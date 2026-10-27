use crate::cache::{CacheKey, ExecutionCache, TaskState};
use crate::domain::{InputNode, ResourceType, TaskId};
use crate::fingerprint::{Fingerprint, Fingerprinter};
use crate::graph::{DependencyGraph, ImpactReport};
use crate::runner::{ProcessExecutor, RunnerError, TaskExecutionResult};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct PipelineExecutionSummary {
    pub results: Vec<TaskExecutionResult>,
    pub total_duration_ms: u64,
    pub success: bool,
}

pub struct TaskScheduler<'a> {
    dag: &'a DependencyGraph,
    workspace_root: PathBuf,
    cache: &'a mut ExecutionCache,
    fail_fast: bool,
}

impl<'a> TaskScheduler<'a> {
    pub fn new(
        dag: &'a DependencyGraph,
        workspace_root: impl AsRef<Path>,
        cache: &'a mut ExecutionCache,
    ) -> Self {
        Self {
            dag,
            workspace_root: workspace_root.as_ref().to_path_buf(),
            cache,
            fail_fast: false,
        }
    }

    pub fn with_fail_fast(mut self, fail_fast: bool) -> Self {
        self.fail_fast = fail_fast;
        self
    }

    /// Ejecuta únicamente el conjunto de tareas afectadas por un ImpactReport.
    pub async fn execute_impact(
        &mut self,
        impact: &ImpactReport,
    ) -> Result<PipelineExecutionSummary, RunnerError> {
        self.execute_tasks(&impact.affected_tasks).await
    }

    /// Ejecuta una secuencia ordenada de tareas respetando caché y propagación de bloqueos.
    pub async fn execute_tasks(
        &mut self,
        task_ids: &[TaskId],
    ) -> Result<PipelineExecutionSummary, RunnerError> {
        let start = std::time::Instant::now();
        let mut results = Vec::new();
        let mut artifact_fingerprints: HashMap<String, Fingerprint> = HashMap::new();
        let mut blocked_tasks: HashSet<TaskId> = HashSet::new();
        let mut overall_success = true;

        let mut sorted_task_ids = task_ids.to_vec();
        if let Ok(topo_order) = petgraph::algo::toposort(&self.dag.graph, None) {
            let topo_rank: HashMap<petgraph::graph::NodeIndex, usize> = topo_order
                .into_iter()
                .enumerate()
                .map(|(rank, idx)| (idx, rank))
                .collect();

            sorted_task_ids.sort_by_key(|t| {
                self.dag
                    .task_indices
                    .get(t)
                    .and_then(|idx| topo_rank.get(idx))
                    .copied()
                    .unwrap_or(usize::MAX)
            });
        }

        // 1. Mapear qué tarea produce cada artefacto para determinar dependencias directas
        let mut artifact_producers: HashMap<crate::domain::ArtifactId, TaskId> = HashMap::new();
        for t_id in &sorted_task_ids {
            if let Some(t) = self.dag.get_task(t_id) {
                for out in &t.outputs {
                    artifact_producers.insert(out.clone(), t.id.clone());
                }
            }
        }

        // 2. Agrupar tareas en oleadas (waves) por nivel topológico para paralelización real
        let mut task_depth: HashMap<TaskId, usize> = HashMap::new();
        for t_id in &sorted_task_ids {
            let mut depth = 0;
            if let Some(t) = self.dag.get_task(t_id) {
                for inp in &t.inputs {
                    if let InputNode::Artifact(a_id) = inp {
                        if let Some(producer) = artifact_producers.get(a_id) {
                            if let Some(&p_depth) = task_depth.get(producer) {
                                depth = depth.max(p_depth + 1);
                            }
                        }
                    }
                }
            }
            task_depth.insert(t_id.clone(), depth);
        }

        let max_depth = task_depth.values().copied().max().unwrap_or(0);
        let mut waves: Vec<Vec<TaskId>> = vec![Vec::new(); max_depth + 1];
        for t_id in &sorted_task_ids {
            let d = task_depth.get(t_id).copied().unwrap_or(0);
            waves[d].push(t_id.clone());
        }

        // 3. Ejecutar oleada por oleada de forma concurrente
        let mut executed_results: HashMap<TaskId, TaskExecutionResult> = HashMap::new();

        for wave in waves {
            if self.fail_fast && !overall_success {
                for task_id in &wave {
                    if !executed_results.contains_key(task_id) {
                        executed_results.insert(
                            task_id.clone(),
                            TaskExecutionResult {
                                task_id: task_id.clone(),
                                state: TaskState::Blocked,
                                exit_code: None,
                                stdout: String::new(),
                                stderr: "Aborted due to prior task failure (--fail-fast enabled)"
                                    .to_string(),
                                duration_ms: 0,
                                cache_key: None,
                            },
                        );
                    }
                }
                continue;
            }

            let mut tasks_to_run_parallel = Vec::new();

            for task_id in &wave {
                let task = match self.dag.get_task(task_id) {
                    Some(t) => t,
                    None => continue,
                };

                // Si está bloqueada por fallo upstream
                if blocked_tasks.contains(task_id) {
                    executed_results.insert(
                        task.id.clone(),
                        TaskExecutionResult {
                            task_id: task.id.clone(),
                            state: TaskState::Blocked,
                            exit_code: None,
                            stdout: String::new(),
                            stderr: "Blocked due to upstream failure".to_string(),
                            duration_ms: 0,
                            cache_key: None,
                        },
                    );
                    overall_success = false;
                    continue;
                }

                // Resolver huellas de entrada
                let mut input_fps = HashMap::new();
                for input in &task.inputs {
                    match input {
                        InputNode::Resource(r_id) => {
                            if let Some(res) = self.dag.get_resource(r_id) {
                                if let ResourceType::File { path, .. } = &res.resource_type {
                                    let full_path = self.workspace_root.join(path);
                                    if let Ok(fp) = Fingerprinter::compute_path(&full_path) {
                                        input_fps.insert(r_id.0.clone(), fp);
                                    }
                                }
                            }
                        }
                        InputNode::Artifact(a_id) => {
                            if let Some(fp) = artifact_fingerprints.get(a_id.as_str()) {
                                input_fps.insert(a_id.0.clone(), *fp);
                            }
                        }
                    }
                }

                // Evaluar Cache Key
                let cache_key = CacheKey::compute(task, &self.workspace_root, &input_fps);

                let mut is_hit = self.cache.check_hit(&task.id, &cache_key);

                // Integridad física: Si hay cache hit pero la tarea declaró cache_paths de salida,
                // verificamos si existen en disco o los restauramos desde el almacén CAS.
                if is_hit && !task.cache_paths.is_empty() {
                    let mut missing_on_disk = false;
                    for rel_path in &task.cache_paths {
                        let full_path = if rel_path.is_absolute() {
                            rel_path.clone()
                        } else {
                            self.workspace_root.join(rel_path)
                        };
                        if !full_path.exists() {
                            missing_on_disk = true;
                            break;
                        }
                    }

                    if missing_on_disk {
                        let restored = Self::restore_cas_outputs(
                            &self.workspace_root,
                            &cache_key,
                            &task.cache_paths,
                        );
                        if !restored {
                            is_hit = false;
                        }
                    }
                }

                if is_hit {
                    // CACHE HIT
                    executed_results.insert(
                        task.id.clone(),
                        TaskExecutionResult {
                            task_id: task.id.clone(),
                            state: TaskState::Cached,
                            exit_code: Some(0),
                            stdout: String::new(),
                            stderr: String::new(),
                            duration_ms: 0,
                            cache_key: Some(cache_key.clone()),
                        },
                    );

                    for out_art in &task.outputs {
                        artifact_fingerprints.insert(
                            out_art.as_str().to_string(),
                            Fingerprinter::compute_bytes(cache_key.0.as_bytes()),
                        );
                    }
                } else {
                    // CACHE MISS -> Programar para ejecución concurrente
                    tasks_to_run_parallel.push((task.clone(), cache_key));
                }
            }

            // Ejecución concurrente de tareas independientes de la oleada mediante Tokio JoinSet
            if !tasks_to_run_parallel.is_empty() {
                let mut join_set = tokio::task::JoinSet::new();

                for (task, cache_key) in tasks_to_run_parallel {
                    let root = self.workspace_root.clone();
                    let key_opt = Some(cache_key);
                    join_set.spawn(async move {
                        let mut attempts = 0;
                        let max_attempts = task.retries.saturating_add(1);
                        let mut final_res = None;

                        while attempts < max_attempts {
                            attempts += 1;
                            let res = ProcessExecutor::execute(&task, &root, key_opt.clone()).await;
                            match &res {
                                Ok(r) if r.state == TaskState::Success => {
                                    final_res = Some(res);
                                    break;
                                }
                                Ok(_) if attempts < max_attempts => {
                                    tokio::time::sleep(std::time::Duration::from_millis(
                                        40 * attempts as u64,
                                    ))
                                    .await;
                                }
                                _ => {
                                    final_res = Some(res);
                                    break;
                                }
                            }
                        }

                        let res = final_res.unwrap_or_else(|| {
                            // Fallback directo en caso extremo
                            Ok(TaskExecutionResult {
                                task_id: task.id.clone(),
                                state: TaskState::Failed,
                                exit_code: Some(1),
                                stdout: String::new(),
                                stderr: "Failed after retry budget exhausted".to_string(),
                                duration_ms: 0,
                                cache_key: key_opt,
                            })
                        });
                        (task, res)
                    });
                }

                while let Some(join_res) = join_set.join_next().await {
                    let (task, exec_res) = join_res
                        .map_err(|e| RunnerError::Io(std::io::Error::other(e.to_string())))?;
                    let exec_result = exec_res?;

                    if exec_result.state == TaskState::Success {
                        if let Some(key) = &exec_result.cache_key {
                            if !task.cache_paths.is_empty() {
                                Self::save_cas_outputs(
                                    &self.workspace_root,
                                    key,
                                    &task.cache_paths,
                                );
                            }

                            self.cache.record_success(
                                &task.id,
                                key.clone(),
                                exec_result.duration_ms,
                            );

                            for out_art in &task.outputs {
                                artifact_fingerprints.insert(
                                    out_art.as_str().to_string(),
                                    Fingerprinter::compute_bytes(key.0.as_bytes()),
                                );
                            }
                        }
                    } else {
                        overall_success = false;
                        self.cache.invalidate(&task.id);

                        if self.fail_fast {
                            // En modo fail-fast, bloquear todas las tareas restantes del workspace
                            for remaining_id in &sorted_task_ids {
                                if remaining_id != &task.id
                                    && !executed_results.contains_key(remaining_id)
                                {
                                    blocked_tasks.insert(remaining_id.clone());
                                }
                            }
                        } else {
                            // En modo causal (keep-going), bloquear únicamente a los descendientes transitivos reales en el DAG
                            if let Some(&task_idx) = self.dag.task_indices.get(&task.id) {
                                let mut bfs = petgraph::visit::Bfs::new(&self.dag.graph, task_idx);
                                while let Some(visited_idx) = bfs.next(&self.dag.graph) {
                                    if let Some(crate::graph::dag::GraphNode::Task(
                                        downstream_task,
                                    )) = self.dag.graph.node_weight(visited_idx)
                                    {
                                        if downstream_task.id != task.id {
                                            blocked_tasks.insert(downstream_task.id.clone());
                                        }
                                    }
                                }
                            }
                        }
                    }

                    executed_results.insert(task.id.clone(), exec_result);
                }
            }
        }

        // Reconstruir lista ordenada final preservando orden topológico determinista
        for task_id in &sorted_task_ids {
            if let Some(res) = executed_results.remove(task_id) {
                results.push(res);
            }
        }

        let total_duration_ms = start.elapsed().as_millis() as u64;

        Ok(PipelineExecutionSummary {
            results,
            total_duration_ms,
            success: overall_success,
        })
    }

    fn restore_cas_outputs(workspace_root: &Path, cache_key: &CacheKey, paths: &[PathBuf]) -> bool {
        let cas_dir = workspace_root
            .join(".onaji")
            .join("cache")
            .join("cas")
            .join(cache_key.as_str());
        if !cas_dir.exists() {
            return false;
        }

        for rel_path in paths {
            let src = cas_dir.join(rel_path);
            let dst = workspace_root.join(rel_path);
            if !src.exists() {
                return false;
            }
            if let Some(parent) = dst.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if src.is_dir() {
                if copy_dir_all(&src, &dst).is_err() {
                    return false;
                }
            } else if std::fs::copy(&src, &dst).is_err() {
                return false;
            }
        }
        true
    }

    fn save_cas_outputs(workspace_root: &Path, cache_key: &CacheKey, paths: &[PathBuf]) {
        let cas_dir = workspace_root
            .join(".onaji")
            .join("cache")
            .join("cas")
            .join(cache_key.as_str());
        for rel_path in paths {
            let src = workspace_root.join(rel_path);
            let dst = cas_dir.join(rel_path);
            if src.exists() {
                if let Some(parent) = dst.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                if src.is_dir() {
                    let _ = copy_dir_all(&src, &dst);
                } else {
                    let _ = std::fs::copy(&src, &dst);
                }
            }
        }
    }
}

fn copy_dir_all(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let dest_child = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_all(&entry.path(), &dest_child)?;
        } else {
            std::fs::copy(entry.path(), dest_child)?;
        }
    }
    Ok(())
}

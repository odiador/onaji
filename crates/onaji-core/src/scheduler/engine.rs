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
        }
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

        for task_id in &sorted_task_ids {
            let task = match self.dag.get_task(task_id) {
                Some(t) => t,
                None => continue,
            };

            // 1. Si está bloqueada por fallo upstream
            if blocked_tasks.contains(task_id) {
                results.push(TaskExecutionResult {
                    task_id: task.id.clone(),
                    state: TaskState::Blocked,
                    exit_code: None,
                    stdout: String::new(),
                    stderr: "Blocked due to upstream failure".to_string(),
                    duration_ms: 0,
                    cache_key: None,
                });
                overall_success = false;
                continue;
            }

            // 2. Resolver huellas de entrada
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

            // 3. Evaluar Cache Key
            let cache_key = CacheKey::compute(task, &self.workspace_root, &input_fps);

            if self.cache.check_hit(&task.id, &cache_key) {
                // CACHE HIT
                results.push(TaskExecutionResult {
                    task_id: task.id.clone(),
                    state: TaskState::Cached,
                    exit_code: Some(0),
                    stdout: String::new(),
                    stderr: String::new(),
                    duration_ms: 0,
                    cache_key: Some(cache_key.clone()),
                });

                // Propagamos huella sintética a los artefactos producidos
                for out_art in &task.outputs {
                    artifact_fingerprints.insert(
                        out_art.as_str().to_string(),
                        Fingerprinter::compute_bytes(cache_key.0.as_bytes()),
                    );
                }
                continue;
            }

            // 4. CACHE MISS -> Ejecución real del proceso
            let exec_result =
                ProcessExecutor::execute(task, &self.workspace_root, Some(cache_key.clone()))
                    .await?;

            if exec_result.state == TaskState::Success {
                self.cache
                    .record_success(&task.id, cache_key.clone(), exec_result.duration_ms);

                // Asignamos huella para los artefactos producidos
                for out_art in &task.outputs {
                    artifact_fingerprints.insert(
                        out_art.as_str().to_string(),
                        Fingerprinter::compute_bytes(cache_key.0.as_bytes()),
                    );
                }
            } else {
                overall_success = false;
                self.cache.invalidate(&task.id);

                // Bloqueamos todas las tareas posteriores que dependan de esta rama
                for remaining_id in task_ids {
                    if remaining_id != &task.id {
                        blocked_tasks.insert(remaining_id.clone());
                    }
                }
            }

            results.push(exec_result);
        }

        let total_duration_ms = start.elapsed().as_millis() as u64;

        Ok(PipelineExecutionSummary {
            results,
            total_duration_ms,
            success: overall_success,
        })
    }
}

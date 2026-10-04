use super::dag::DependencyGraph;
use crate::domain::{ArtifactId, ResourceId, TaskId};
use petgraph::visit::Bfs;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImpactReport {
    pub root_resource: ResourceId,
    pub affected_tasks: Vec<TaskId>,
    pub affected_artifacts: Vec<ArtifactId>,
    pub unaffected_tasks: Vec<TaskId>,
}

pub struct ImpactAnalyzer<'a> {
    dag: &'a DependencyGraph,
}

impl<'a> ImpactAnalyzer<'a> {
    pub fn new(dag: &'a DependencyGraph) -> Self {
        Self { dag }
    }

    /// Calcula la clausura transitiva hacia adelante a partir de un recurso modificado.
    pub fn analyze_resource_change(&self, resource_id: &ResourceId) -> Option<ImpactReport> {
        let root_idx = *self.dag.resource_indices.get(resource_id)?;

        let mut bfs = Bfs::new(&self.dag.graph, root_idx);
        let mut affected_node_indices = HashSet::new();

        while let Some(visited_idx) = bfs.next(&self.dag.graph) {
            affected_node_indices.insert(visited_idx);
        }

        let mut affected_tasks = Vec::new();
        let mut affected_artifacts = Vec::new();
        let mut all_tasks = HashSet::new();

        for (task_id, idx) in &self.dag.task_indices {
            all_tasks.insert(task_id.clone());
            if affected_node_indices.contains(idx) {
                affected_tasks.push(task_id.clone());
            }
        }

        for (art_id, idx) in &self.dag.artifact_indices {
            if affected_node_indices.contains(idx) {
                affected_artifacts.push(art_id.clone());
            }
        }

        // Ordenamos las tareas afectadas preservando orden topológico real
        if let Ok(topo_order) = petgraph::algo::toposort(&self.dag.graph, None) {
            let topo_rank: std::collections::HashMap<petgraph::graph::NodeIndex, usize> =
                topo_order
                    .into_iter()
                    .enumerate()
                    .map(|(rank, idx)| (idx, rank))
                    .collect();

            affected_tasks.sort_by_key(|t| {
                self.dag
                    .task_indices
                    .get(t)
                    .and_then(|idx| topo_rank.get(idx))
                    .copied()
                    .unwrap_or(usize::MAX)
            });
        }

        let mut unaffected_tasks: Vec<TaskId> = all_tasks
            .into_iter()
            .filter(|t| !affected_tasks.contains(t))
            .collect();
        unaffected_tasks.sort();

        Some(ImpactReport {
            root_resource: resource_id.clone(),
            affected_tasks,
            affected_artifacts,
            unaffected_tasks,
        })
    }

    /// Analyzes the cumulative impact of multiple changed resources, returning a merged ImpactReport.
    pub fn analyze_multiple_resources(&self, resource_ids: &[ResourceId]) -> ImpactReport {
        let mut affected_tasks_set = HashSet::new();
        let mut affected_artifacts_set = HashSet::new();

        for res_id in resource_ids {
            if let Some(report) = self.analyze_resource_change(res_id) {
                for t in report.affected_tasks {
                    affected_tasks_set.insert(t);
                }
                for a in report.affected_artifacts {
                    affected_artifacts_set.insert(a);
                }
            }
        }

        let mut affected_tasks: Vec<TaskId> = affected_tasks_set.into_iter().collect();
        if let Ok(topo_order) = petgraph::algo::toposort(&self.dag.graph, None) {
            let topo_rank: HashMap<petgraph::graph::NodeIndex, usize> = topo_order
                .into_iter()
                .enumerate()
                .map(|(rank, idx)| (idx, rank))
                .collect();

            affected_tasks.sort_by_key(|t| {
                self.dag
                    .task_indices
                    .get(t)
                    .and_then(|idx| topo_rank.get(idx))
                    .copied()
                    .unwrap_or(usize::MAX)
            });
        }

        let mut unaffected_tasks: Vec<TaskId> = self
            .dag
            .task_indices
            .keys()
            .filter(|t| !affected_tasks.contains(t))
            .cloned()
            .collect();
        unaffected_tasks.sort();

        let mut affected_artifacts: Vec<ArtifactId> = affected_artifacts_set.into_iter().collect();
        affected_artifacts.sort();

        let root_res = if resource_ids.is_empty() {
            ResourceId::new("none")
        } else {
            resource_ids[0].clone()
        };

        ImpactReport {
            root_resource: root_res,
            affected_tasks,
            affected_artifacts,
            unaffected_tasks,
        }
    }

    /// Finds all registered resources whose file paths match any of the provided changed files.
    pub fn match_changed_files(
        &self,
        workspace_root: &std::path::Path,
        changed_files: &[std::path::PathBuf],
    ) -> Vec<ResourceId> {
        let mut matched = Vec::new();
        let canonical_ws = workspace_root
            .canonicalize()
            .unwrap_or_else(|_| workspace_root.to_path_buf());
        let git_root = find_git_root(&canonical_ws)
            .and_then(|r| r.canonicalize().ok())
            .unwrap_or_else(|| canonical_ws.clone());

        for res_id in self.dag.resource_indices.keys() {
            if let Some(res) = self.dag.get_resource(res_id) {
                if let crate::domain::ResourceType::File { path, .. } = &res.resource_type {
                    let clean_res_path = path.strip_prefix("./").unwrap_or(path);
                    let repo_res_path = if let Ok(rel_ws) = canonical_ws.strip_prefix(&git_root) {
                        rel_ws.join(clean_res_path)
                    } else {
                        clean_res_path.to_path_buf()
                    };

                    for changed in changed_files {
                        let clean_changed = changed.strip_prefix("./").unwrap_or(changed);

                        if clean_changed == clean_res_path
                            || clean_changed == repo_res_path
                            || clean_changed.starts_with(&repo_res_path)
                            || repo_res_path.starts_with(clean_changed)
                            || clean_changed.starts_with(clean_res_path)
                            || clean_res_path.starts_with(clean_changed)
                        {
                            if !matched.contains(res_id) {
                                matched.push(res_id.clone());
                            }
                            break;
                        }
                    }
                }
            }
        }

        matched
    }
}

/// Locates the top-level root of the enclosing git repository.
pub fn find_git_root(start_dir: &std::path::Path) -> Option<std::path::PathBuf> {
    let out = std::process::Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .current_dir(start_dir)
        .output()
        .ok()?;
    if out.status.success() {
        let text = String::from_utf8_lossy(&out.stdout);
        let trimmed = text.trim();
        if !trimmed.is_empty() {
            return Some(std::path::PathBuf::from(trimmed));
        }
    }
    None
}

/// Discovers files modified between a git reference and current HEAD/working tree.
pub fn detect_git_changes(
    workspace_root: &std::path::Path,
    since_ref: &str,
    include_uncommitted: bool,
) -> Result<Vec<std::path::PathBuf>, String> {
    let mut files = HashSet::new();

    // 1. Try git diff --name-only <since_ref>...HEAD
    let mut diff_cmd = std::process::Command::new("git");
    diff_cmd.args(["diff", "--name-only", &format!("{}...HEAD", since_ref)]);
    diff_cmd.current_dir(workspace_root);

    let output = match diff_cmd.output() {
        Ok(out) if out.status.success() => out,
        _ => {
            let mut fb1 = std::process::Command::new("git");
            fb1.args(["diff", "--name-only", since_ref]);
            fb1.current_dir(workspace_root);
            match fb1.output() {
                Ok(out) if out.status.success() => out,
                _ => {
                    let mut fb2 = std::process::Command::new("git");
                    fb2.args(["diff", "--name-only", "HEAD~1"]);
                    fb2.current_dir(workspace_root);
                    fb2.output()
                        .map_err(|e| format!("Failed to execute git: {}", e))?
                }
            }
        }
    };

    if output.status.success() {
        let text = String::from_utf8_lossy(&output.stdout);
        for line in text.lines().map(|s| s.trim()).filter(|s| !s.is_empty()) {
            files.insert(std::path::PathBuf::from(line));
        }
    }

    if include_uncommitted {
        let mut uncommitted_cmd = std::process::Command::new("git");
        uncommitted_cmd.args(["diff", "--name-only", "HEAD"]);
        uncommitted_cmd.current_dir(workspace_root);
        if let Ok(out) = uncommitted_cmd.output() {
            if out.status.success() {
                let text = String::from_utf8_lossy(&out.stdout);
                for line in text.lines().map(|s| s.trim()).filter(|s| !s.is_empty()) {
                    files.insert(std::path::PathBuf::from(line));
                }
            }
        }

        let mut status_cmd = std::process::Command::new("git");
        status_cmd.args(["status", "--porcelain"]);
        status_cmd.current_dir(workspace_root);
        if let Ok(out) = status_cmd.output() {
            if out.status.success() {
                let text = String::from_utf8_lossy(&out.stdout);
                for line in text.lines() {
                    if line.len() > 3 {
                        let path_str = line[3..].trim();
                        if !path_str.is_empty() {
                            files.insert(std::path::PathBuf::from(path_str));
                        }
                    }
                }
            }
        }
    }

    let mut result: Vec<std::path::PathBuf> = files.into_iter().collect();
    result.sort();
    Ok(result)
}

impl ImpactReport {
    /// Filters the impacted tasks to only those that are test/validation tasks
    /// and their upstream causal prerequisites in the DAG, skipping unneeded branches.
    pub fn filter_for_tests(&self, dag: &DependencyGraph) -> Self {
        let test_tasks: Vec<TaskId> = self
            .affected_tasks
            .iter()
            .filter(|tid| {
                if let Some(t) = dag.get_task(tid) {
                    t.is_test()
                } else {
                    false
                }
            })
            .cloned()
            .collect();

        if test_tasks.is_empty() {
            let mut unaffected = self.unaffected_tasks.clone();
            for tid in &self.affected_tasks {
                if !unaffected.contains(tid) {
                    unaffected.push(tid.clone());
                }
            }
            unaffected.sort();

            return Self {
                root_resource: self.root_resource.clone(),
                affected_tasks: Vec::new(),
                affected_artifacts: Vec::new(),
                unaffected_tasks: unaffected,
            };
        }

        let mut required_node_indices = HashSet::new();
        for test_task_id in &test_tasks {
            if let Some(node_idx) = dag.task_indices.get(test_task_id) {
                required_node_indices.insert(*node_idx);
                let mut dfs =
                    petgraph::visit::Dfs::new(&petgraph::visit::Reversed(&dag.graph), *node_idx);
                while let Some(ancestor) = dfs.next(&petgraph::visit::Reversed(&dag.graph)) {
                    required_node_indices.insert(ancestor);
                }
            }
        }

        let filtered_affected: Vec<TaskId> = self
            .affected_tasks
            .iter()
            .filter(|tid| {
                dag.task_indices
                    .get(tid)
                    .map(|idx| required_node_indices.contains(idx))
                    .unwrap_or(false)
            })
            .cloned()
            .collect();

        let filtered_artifacts: Vec<ArtifactId> = self
            .affected_artifacts
            .iter()
            .filter(|aid| {
                dag.artifact_indices
                    .get(aid)
                    .map(|idx| required_node_indices.contains(idx))
                    .unwrap_or(false)
            })
            .cloned()
            .collect();

        let mut unaffected = self.unaffected_tasks.clone();
        for tid in &self.affected_tasks {
            if !filtered_affected.contains(tid) && !unaffected.contains(tid) {
                unaffected.push(tid.clone());
            }
        }
        unaffected.sort();

        Self {
            root_resource: self.root_resource.clone(),
            affected_tasks: filtered_affected,
            affected_artifacts: filtered_artifacts,
            unaffected_tasks: unaffected,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{Action, Artifact, InputNode, Resource, Task};
    use crate::graph::builder::GraphBuilder;

    #[test]
    fn test_smart_test_filtering_causal_ancestors() {
        // Pipeline:
        // schema (resource) -> codegen (task) -> types (artifact)
        // types -> typecheck (task tagged 'test')
        // types -> deploy_docs (task, not test)
        let res = Resource::new(
            "schema",
            crate::domain::ResourceType::File {
                path: std::path::PathBuf::from("schema.sql"),
                debounce_ms: 100,
            },
        );
        let art = Artifact::new("types");

        let codegen = Task::new(
            "codegen",
            vec![InputNode::Resource(res.id.clone())],
            vec![art.id.clone()],
            Action::Command {
                run: "echo codegen".into(),
                working_dir: None,
                env: Default::default(),
            },
        );

        let typecheck = Task::new(
            "typecheck",
            vec![InputNode::Artifact(art.id.clone())],
            vec![],
            Action::Command {
                run: "echo check".into(),
                working_dir: None,
                env: Default::default(),
            },
        )
        .with_tags(vec!["test".into()]);

        let deploy_docs = Task::new(
            "deploy_docs",
            vec![InputNode::Artifact(art.id.clone())],
            vec![],
            Action::Command {
                run: "echo docs".into(),
                working_dir: None,
                env: Default::default(),
            },
        );

        let dag = GraphBuilder::new()
            .add_resource(res)
            .add_artifact(art)
            .add_task(codegen)
            .add_task(typecheck)
            .add_task(deploy_docs)
            .build()
            .expect("DAG build failed");

        let analyzer = ImpactAnalyzer::new(&dag);
        let raw_impact = analyzer
            .analyze_resource_change(&ResourceId::new("schema"))
            .expect("Impact analysis failed");

        assert_eq!(raw_impact.affected_tasks.len(), 3);

        let test_impact = raw_impact.filter_for_tests(&dag);
        // Should keep codegen (upstream prerequisite) and typecheck (test), but drop deploy_docs
        assert_eq!(test_impact.affected_tasks.len(), 2);
        assert!(test_impact.affected_tasks.contains(&TaskId::new("codegen")));
        assert!(test_impact
            .affected_tasks
            .contains(&TaskId::new("typecheck")));
        assert!(!test_impact
            .affected_tasks
            .contains(&TaskId::new("deploy_docs")));
        assert!(test_impact
            .unaffected_tasks
            .contains(&TaskId::new("deploy_docs")));
    }

    #[test]
    fn test_multiple_resources_and_path_matching() {
        let r1 = Resource::new(
            "api_spec",
            crate::domain::ResourceType::File {
                path: std::path::PathBuf::from("openapi.yaml"),
                debounce_ms: 100,
            },
        );
        let r2 = Resource::new(
            "client_code",
            crate::domain::ResourceType::File {
                path: std::path::PathBuf::from("src/client.ts"),
                debounce_ms: 100,
            },
        );
        let t1 = Task::new(
            "gen",
            vec![InputNode::Resource(r1.id.clone())],
            vec![],
            Action::Command {
                run: "echo gen".into(),
                working_dir: None,
                env: Default::default(),
            },
        );
        let t2 = Task::new(
            "check",
            vec![InputNode::Resource(r2.id.clone())],
            vec![],
            Action::Command {
                run: "echo check".into(),
                working_dir: None,
                env: Default::default(),
            },
        );

        let dag = GraphBuilder::new()
            .add_resource(r1)
            .add_resource(r2)
            .add_task(t1)
            .add_task(t2)
            .build()
            .unwrap();

        let analyzer = ImpactAnalyzer::new(&dag);
        let changed = vec![std::path::PathBuf::from("openapi.yaml")];
        let matched = analyzer.match_changed_files(std::path::Path::new("."), &changed);
        assert_eq!(matched, vec![ResourceId::new("api_spec")]);

        let report = analyzer.analyze_multiple_resources(&matched);
        assert_eq!(report.affected_tasks, vec![TaskId::new("gen")]);
        assert_eq!(report.unaffected_tasks, vec![TaskId::new("check")]);
    }
}

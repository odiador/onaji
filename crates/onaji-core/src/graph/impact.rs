use super::dag::DependencyGraph;
use crate::domain::{ArtifactId, ResourceId, TaskId};
use petgraph::visit::Bfs;
use std::collections::HashSet;

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

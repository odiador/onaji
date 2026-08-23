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

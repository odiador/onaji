use super::definition::{Workspace, WorkspaceId};
use crate::domain::{ArtifactId, InputNode, ResourceId, TaskId};
use crate::graph::{GraphBuildError, GraphNode, ImpactAnalyzer};
use petgraph::algo::is_cyclic_directed;
use petgraph::graph::{DiGraph, NodeIndex};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossWorkspaceImpact {
    pub workspace_id: WorkspaceId,
    pub affected_tasks: Vec<TaskId>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiWorkspaceImpactReport {
    pub root_workspace: WorkspaceId,
    pub root_resource: ResourceId,
    pub local_affected_tasks: Vec<TaskId>,
    pub cross_workspace_impacts: Vec<CrossWorkspaceImpact>,
}

pub struct MultiWorkspaceGraph {
    pub workspaces: HashMap<WorkspaceId, Workspace>,
    meta_graph: DiGraph<WorkspaceId, ArtifactId>,
    workspace_nodes: HashMap<WorkspaceId, NodeIndex>,
}

impl Default for MultiWorkspaceGraph {
    fn default() -> Self {
        Self::new()
    }
}

impl MultiWorkspaceGraph {
    pub fn new() -> Self {
        Self {
            workspaces: HashMap::new(),
            meta_graph: DiGraph::new(),
            workspace_nodes: HashMap::new(),
        }
    }

    pub fn add_workspace(&mut self, workspace: Workspace) -> Result<(), GraphBuildError> {
        let ws_id = workspace.id.clone();
        if self.workspaces.contains_key(&ws_id) {
            return Err(GraphBuildError::DuplicateNodeId {
                id: ws_id.to_string(),
                kind: "workspace",
            });
        }

        let node_idx = self.meta_graph.add_node(ws_id.clone());
        self.workspace_nodes.insert(ws_id.clone(), node_idx);
        self.workspaces.insert(ws_id, workspace);
        Ok(())
    }

    pub fn connect_dependencies(&mut self) -> Result<(), GraphBuildError> {
        // Build meta-graph edges
        for (consumer_id, ws) in &self.workspaces {
            let consumer_idx = *self.workspace_nodes.get(consumer_id).unwrap();
            for dep in &ws.dependencies {
                let producer_idx = self
                    .workspace_nodes
                    .get(&dep.workspace)
                    .copied()
                    .ok_or_else(|| GraphBuildError::UnknownInput {
                        task_id: TaskId::new(format!("workspace:{}", consumer_id)),
                        input_id: format!("{}:{}", dep.workspace, dep.artifact),
                    })?;

                // Edge from producer workspace to consumer workspace
                self.meta_graph
                    .add_edge(producer_idx, consumer_idx, dep.artifact.clone());
            }
        }

        // Verify no cycles between workspaces
        if is_cyclic_directed(&self.meta_graph) {
            return Err(GraphBuildError::CyclicDependency {
                cycle_node: "cross-workspace cycle".to_string(),
            });
        }

        Ok(())
    }

    pub fn analyze_impact(
        &self,
        root_ws_id: &WorkspaceId,
        resource_id: &ResourceId,
    ) -> Option<MultiWorkspaceImpactReport> {
        let root_ws = self.workspaces.get(root_ws_id)?;
        let analyzer = ImpactAnalyzer::new(&root_ws.dag);
        let local_report = analyzer.analyze_resource_change(resource_id)?;

        let mut cross_impacts: Vec<CrossWorkspaceImpact> = Vec::new();
        let mut produced_artifacts: HashSet<ArtifactId> = HashSet::new();

        // 1. Identify which artifacts are produced by the local affected tasks
        for task_id in &local_report.affected_tasks {
            if let Some(task_idx) = root_ws.dag.task_indices.get(task_id) {
                if let GraphNode::Task(task) = &root_ws.dag.graph[*task_idx] {
                    for out_art in &task.outputs {
                        produced_artifacts.insert(out_art.clone());
                    }
                }
            }
        }

        // 2. Cascade across dependent workspaces that consume any produced artifacts
        for (consumer_id, consumer_ws) in &self.workspaces {
            if consumer_id == root_ws_id {
                continue;
            }

            let mut affected_downstream: Vec<TaskId> = Vec::new();
            for dep in &consumer_ws.dependencies {
                if dep.workspace == *root_ws_id && produced_artifacts.contains(&dep.artifact) {
                    // Find tasks in consumer_ws that consume this artifact (or "producer:artifact")
                    let artifact_key = dep.artifact.as_str();
                    let qualified_key = format!("{}:{}", dep.workspace, dep.artifact);

                    for (task_id, task_idx) in &consumer_ws.dag.task_indices {
                        if let GraphNode::Task(task) = &consumer_ws.dag.graph[*task_idx] {
                            let consumes = task.inputs.iter().any(|inp| match inp {
                                InputNode::Artifact(a) => {
                                    a.as_str() == artifact_key || a.as_str() == qualified_key
                                }
                                _ => false,
                            });
                            if consumes && !affected_downstream.contains(task_id) {
                                affected_downstream.push(task_id.clone());
                            }
                        }
                    }
                }
            }

            if !affected_downstream.is_empty() {
                cross_impacts.push(CrossWorkspaceImpact {
                    workspace_id: consumer_id.clone(),
                    affected_tasks: affected_downstream,
                });
            }
        }

        Some(MultiWorkspaceImpactReport {
            root_workspace: root_ws_id.clone(),
            root_resource: resource_id.clone(),
            local_affected_tasks: local_report.affected_tasks,
            cross_workspace_impacts: cross_impacts,
        })
    }
}

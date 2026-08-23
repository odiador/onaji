use crate::domain::{Artifact, ArtifactId, Resource, ResourceId, Task, TaskId};
use petgraph::graph::{DiGraph, NodeIndex};
use std::collections::HashMap;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GraphNode {
    Resource(Resource),
    Artifact(Artifact),
    Task(Task),
}

impl GraphNode {
    pub fn id_str(&self) -> &str {
        match self {
            GraphNode::Resource(r) => r.id.as_str(),
            GraphNode::Artifact(a) => a.id.as_str(),
            GraphNode::Task(t) => t.id.as_str(),
        }
    }

    pub fn kind_str(&self) -> &'static str {
        match self {
            GraphNode::Resource(_) => "Resource",
            GraphNode::Artifact(_) => "Artifact",
            GraphNode::Task(_) => "Task",
        }
    }
}

impl fmt::Display for GraphNode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GraphNode::Resource(r) => write!(f, "Resource({})", r.id),
            GraphNode::Artifact(a) => write!(f, "Artifact({})", a.id),
            GraphNode::Task(t) => write!(f, "Task({})", t.id),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct DependencyGraph {
    pub graph: DiGraph<GraphNode, ()>,
    pub node_indices: HashMap<String, NodeIndex>,
    pub resource_indices: HashMap<ResourceId, NodeIndex>,
    pub artifact_indices: HashMap<ArtifactId, NodeIndex>,
    pub task_indices: HashMap<TaskId, NodeIndex>,
}

impl DependencyGraph {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn node_count(&self) -> usize {
        self.graph.node_count()
    }

    pub fn edge_count(&self) -> usize {
        self.graph.edge_count()
    }

    pub fn get_node(&self, index: NodeIndex) -> Option<&GraphNode> {
        self.graph.node_weight(index)
    }

    pub fn get_task(&self, id: &TaskId) -> Option<&Task> {
        self.task_indices.get(id).and_then(|idx| {
            if let Some(GraphNode::Task(t)) = self.graph.node_weight(*idx) {
                Some(t)
            } else {
                None
            }
        })
    }

    pub fn get_resource(&self, id: &ResourceId) -> Option<&Resource> {
        self.resource_indices.get(id).and_then(|idx| {
            if let Some(GraphNode::Resource(r)) = self.graph.node_weight(*idx) {
                Some(r)
            } else {
                None
            }
        })
    }

    pub fn get_artifact(&self, id: &ArtifactId) -> Option<&Artifact> {
        self.artifact_indices.get(id).and_then(|idx| {
            if let Some(GraphNode::Artifact(a)) = self.graph.node_weight(*idx) {
                Some(a)
            } else {
                None
            }
        })
    }
}

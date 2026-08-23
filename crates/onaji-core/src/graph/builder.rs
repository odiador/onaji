use super::dag::{DependencyGraph, GraphNode};
use crate::domain::{Artifact, ArtifactId, InputNode, Resource, Task, TaskId};
use petgraph::algo::toposort;
use std::collections::HashMap;
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum GraphBuildError {
    #[error("Graph is empty: at least one resource and task must be defined")]
    EmptyGraph,

    #[error("Duplicate node ID detected: '{id}' is already registered as a {kind}")]
    DuplicateNodeId { id: String, kind: &'static str },

    #[error("Task '{task_id}' references unknown input '{input_id}'")]
    UnknownInput { task_id: TaskId, input_id: String },

    #[error("Task '{task_id}' references unknown output artifact '{artifact_id}'")]
    UnknownOutput {
        task_id: TaskId,
        artifact_id: ArtifactId,
    },

    #[error("Conflicting artifact producers: artifact '{artifact_id}' is produced by multiple tasks ('{first_task}' and '{second_task}')")]
    ConflictingArtifactProducer {
        artifact_id: ArtifactId,
        first_task: TaskId,
        second_task: TaskId,
    },

    #[error("Cyclic dependency detected in task graph: cycle involves node '{cycle_node}'")]
    CyclicDependency { cycle_node: String },
}

#[derive(Debug, Default)]
pub struct GraphBuilder {
    resources: Vec<Resource>,
    artifacts: Vec<Artifact>,
    tasks: Vec<Task>,
}

impl GraphBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_resource(mut self, resource: Resource) -> Self {
        self.resources.push(resource);
        self
    }

    pub fn add_resources(mut self, resources: impl IntoIterator<Item = Resource>) -> Self {
        self.resources.extend(resources);
        self
    }

    pub fn add_artifact(mut self, artifact: Artifact) -> Self {
        self.artifacts.push(artifact);
        self
    }

    pub fn add_artifacts(mut self, artifacts: impl IntoIterator<Item = Artifact>) -> Self {
        self.artifacts.extend(artifacts);
        self
    }

    pub fn add_task(mut self, task: Task) -> Self {
        self.tasks.push(task);
        self
    }

    pub fn add_tasks(mut self, tasks: impl IntoIterator<Item = Task>) -> Self {
        self.tasks.extend(tasks);
        self
    }

    pub fn build(self) -> Result<DependencyGraph, GraphBuildError> {
        if self.resources.is_empty() && self.tasks.is_empty() {
            return Err(GraphBuildError::EmptyGraph);
        }

        let mut dag = DependencyGraph::new();
        let mut seen_ids: HashMap<String, &'static str> = HashMap::new();

        // 1. Insert Resources
        for resource in self.resources {
            let id_str = resource.id.0.clone();
            if let Some(existing_kind) = seen_ids.get(&id_str) {
                return Err(GraphBuildError::DuplicateNodeId {
                    id: id_str,
                    kind: existing_kind,
                });
            }
            seen_ids.insert(id_str.clone(), "Resource");

            let res_id = resource.id.clone();
            let idx = dag.graph.add_node(GraphNode::Resource(resource));
            dag.node_indices.insert(id_str, idx);
            dag.resource_indices.insert(res_id, idx);
        }

        // 2. Insert Artifacts
        for artifact in self.artifacts {
            let id_str = artifact.id.0.clone();
            if let Some(existing_kind) = seen_ids.get(&id_str) {
                return Err(GraphBuildError::DuplicateNodeId {
                    id: id_str,
                    kind: existing_kind,
                });
            }
            seen_ids.insert(id_str.clone(), "Artifact");

            let art_id = artifact.id.clone();
            let idx = dag.graph.add_node(GraphNode::Artifact(artifact));
            dag.node_indices.insert(id_str, idx);
            dag.artifact_indices.insert(art_id, idx);
        }

        // 3. Insert Tasks
        let mut artifact_producers: HashMap<ArtifactId, TaskId> = HashMap::new();

        for task in &self.tasks {
            let id_str = task.id.0.clone();
            if let Some(existing_kind) = seen_ids.get(&id_str) {
                return Err(GraphBuildError::DuplicateNodeId {
                    id: id_str,
                    kind: existing_kind,
                });
            }
            seen_ids.insert(id_str.clone(), "Task");

            // Check single producer rule for artifacts
            for output in &task.outputs {
                if let Some(first_task) = artifact_producers.get(output) {
                    return Err(GraphBuildError::ConflictingArtifactProducer {
                        artifact_id: output.clone(),
                        first_task: first_task.clone(),
                        second_task: task.id.clone(),
                    });
                }
                artifact_producers.insert(output.clone(), task.id.clone());
            }

            let task_id = task.id.clone();
            let idx = dag.graph.add_node(GraphNode::Task(task.clone()));
            dag.node_indices.insert(id_str, idx);
            dag.task_indices.insert(task_id, idx);
        }

        // 4. Create Edges
        for task in &self.tasks {
            let task_idx = *dag.task_indices.get(&task.id).unwrap();

            // Inputs -> Task
            for input in &task.inputs {
                match input {
                    InputNode::Resource(r_id) => {
                        let res_idx = dag.resource_indices.get(r_id).ok_or_else(|| {
                            GraphBuildError::UnknownInput {
                                task_id: task.id.clone(),
                                input_id: r_id.0.clone(),
                            }
                        })?;
                        dag.graph.add_edge(*res_idx, task_idx, ());
                    }
                    InputNode::Artifact(a_id) => {
                        let art_idx = dag.artifact_indices.get(a_id).ok_or_else(|| {
                            GraphBuildError::UnknownInput {
                                task_id: task.id.clone(),
                                input_id: a_id.0.clone(),
                            }
                        })?;
                        dag.graph.add_edge(*art_idx, task_idx, ());
                    }
                }
            }

            // Task -> Outputs (Artifacts)
            for output in &task.outputs {
                let art_idx = dag.artifact_indices.get(output).ok_or_else(|| {
                    GraphBuildError::UnknownOutput {
                        task_id: task.id.clone(),
                        artifact_id: output.clone(),
                    }
                })?;
                dag.graph.add_edge(task_idx, *art_idx, ());
            }
        }

        // 5. Detect Cycles
        if let Err(cycle) = toposort(&dag.graph, None) {
            let cycle_node = dag
                .graph
                .node_weight(cycle.node_id())
                .map(|n| n.id_str().to_string())
                .unwrap_or_else(|| "unknown".to_string());
            return Err(GraphBuildError::CyclicDependency { cycle_node });
        }

        Ok(dag)
    }
}

use super::schema::{RawManifest, RawResource};
use crate::domain::{
    Action, Artifact, ArtifactId, InputNode, Resource, ResourceId, ResourceType, Task,
};
use crate::graph::{DependencyGraph, GraphBuildError, GraphBuilder};
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Failed to read config file at '{path}': {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("Failed to parse YAML manifest: {0}")]
    Yaml(#[from] serde_yaml::Error),

    #[error("Graph validation failed: {0}")]
    Graph(#[from] GraphBuildError),
}

pub struct ConfigLoader;

impl ConfigLoader {
    pub fn load_from_str(yaml_str: &str) -> Result<(RawManifest, DependencyGraph), ConfigError> {
        let manifest: RawManifest = serde_yaml::from_str(yaml_str)?;
        let dag = Self::build_dag_from_manifest(&manifest)?;
        Ok((manifest, dag))
    }

    pub fn load_from_file(
        path: impl AsRef<Path>,
    ) -> Result<(RawManifest, DependencyGraph), ConfigError> {
        let path_ref = path.as_ref();
        let content = std::fs::read_to_string(path_ref).map_err(|source| ConfigError::Io {
            path: path_ref.to_path_buf(),
            source,
        })?;
        Self::load_from_str(&content)
    }

    pub fn build_dag_from_manifest(
        manifest: &RawManifest,
    ) -> Result<DependencyGraph, GraphBuildError> {
        let mut builder = GraphBuilder::new();

        // 1. Transform Resources
        for (id, raw_res) in &manifest.resources {
            let res_type = match raw_res {
                RawResource::File {
                    path, debounce_ms, ..
                } => ResourceType::File {
                    path: PathBuf::from(path),
                    debounce_ms: *debounce_ms,
                },
                RawResource::Http {
                    url,
                    interval_ms,
                    headers,
                    ..
                } => ResourceType::Http {
                    url: url.clone(),
                    interval_ms: *interval_ms,
                    headers: headers.clone(),
                },
                RawResource::Process { command, .. } => ResourceType::Process {
                    command: command.clone(),
                },
                RawResource::Git { path, branch, .. } => ResourceType::Git {
                    path: PathBuf::from(path),
                    branch: branch.clone(),
                },
            };

            let description = match raw_res {
                RawResource::File { description, .. }
                | RawResource::Http { description, .. }
                | RawResource::Process { description, .. }
                | RawResource::Git { description, .. } => description.clone(),
            };

            let mut resource = Resource::new(id, res_type);
            if let Some(desc) = description {
                resource = resource.with_description(desc);
            }
            builder = builder.add_resource(resource);
        }

        // 2. Transform Artifacts
        for (id, raw_art) in &manifest.artifacts {
            let mut artifact = Artifact::new(id);
            if let Some(desc) = &raw_art.description {
                artifact = artifact.with_description(desc);
            }
            artifact.metadata = raw_art.metadata.clone();
            builder = builder.add_artifact(artifact);
        }

        // 3. Transform Tasks
        for (id, raw_task) in &manifest.tasks {
            let inputs: Vec<InputNode> = raw_task
                .inputs
                .iter()
                .map(|inp| {
                    if manifest.resources.contains_key(inp) {
                        InputNode::Resource(ResourceId::new(inp))
                    } else {
                        InputNode::Artifact(ArtifactId::new(inp))
                    }
                })
                .collect();

            let outputs: Vec<ArtifactId> = raw_task.outputs.iter().map(ArtifactId::new).collect();

            let action = Action::Command {
                run: raw_task.run.clone(),
                working_dir: raw_task.working_dir.as_ref().map(PathBuf::from),
                env: raw_task.env.clone(),
            };

            let mut task = Task::new(id, inputs, outputs, action);
            if let Some(desc) = &raw_task.description {
                task = task.with_description(desc);
            }
            if let Some(cache) = &raw_task.cache {
                task = task.with_cache_paths(cache.paths.iter().map(PathBuf::from).collect());
            }
            if let Some(prio_str) = &raw_task.priority {
                task.priority = match prio_str.to_lowercase().as_str() {
                    "critical" => crate::domain::TaskPriority::Critical,
                    "high" => crate::domain::TaskPriority::High,
                    "low" => crate::domain::TaskPriority::Low,
                    "background" => crate::domain::TaskPriority::Background,
                    _ => crate::domain::TaskPriority::Normal,
                };
            }

            builder = builder.add_task(task);
        }

        builder.build()
    }
}

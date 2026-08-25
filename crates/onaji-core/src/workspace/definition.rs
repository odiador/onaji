use crate::config::RawManifest;
use crate::domain::ArtifactId;
use crate::graph::DependencyGraph;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct WorkspaceId(String);

impl WorkspaceId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for WorkspaceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrossWorkspaceDep {
    pub workspace: WorkspaceId,
    pub artifact: ArtifactId,
}

#[derive(Debug, Clone)]
pub struct Workspace {
    pub id: WorkspaceId,
    pub root_dir: PathBuf,
    pub manifest_path: PathBuf,
    pub manifest: RawManifest,
    pub dag: DependencyGraph,
    pub dependencies: Vec<CrossWorkspaceDep>,
}

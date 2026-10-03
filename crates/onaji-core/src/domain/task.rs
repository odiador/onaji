use super::artifact::ArtifactId;
use super::resource::ResourceId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct TaskId(pub String);

impl TaskId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for TaskId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<&str> for TaskId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl From<&String> for TaskId {
    fn from(s: &String) -> Self {
        Self(s.clone())
    }
}

impl From<String> for TaskId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum InputNode {
    Resource(ResourceId),
    Artifact(ArtifactId),
}

impl fmt::Display for InputNode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InputNode::Resource(r) => write!(f, "resource:{}", r),
            InputNode::Artifact(a) => write!(f, "artifact:{}", a),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    Command {
        run: String,
        working_dir: Option<PathBuf>,
        env: HashMap<String, String>,
    },
    Builtin {
        adapter: String,
        config: HashMap<String, String>,
    },
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default,
)]
pub enum TaskPriority {
    Background = 10,
    Low = 25,
    #[default]
    Normal = 50,
    High = 75,
    Critical = 100,
}

impl TaskPriority {
    pub fn score(&self) -> u32 {
        *self as u32
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Task {
    pub id: TaskId,
    pub description: Option<String>,
    pub inputs: Vec<InputNode>,
    pub outputs: Vec<ArtifactId>,
    pub action: Action,
    pub cache_paths: Vec<PathBuf>,
    pub priority: TaskPriority,
    pub tags: Vec<String>,
}

impl Task {
    pub fn new(
        id: impl Into<TaskId>,
        inputs: Vec<InputNode>,
        outputs: Vec<ArtifactId>,
        action: Action,
    ) -> Self {
        Self {
            id: id.into(),
            description: None,
            inputs,
            outputs,
            action,
            cache_paths: Vec::new(),
            priority: TaskPriority::Normal,
            tags: Vec::new(),
        }
    }

    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    pub fn with_cache_paths(mut self, paths: Vec<PathBuf>) -> Self {
        self.cache_paths = paths;
        self
    }

    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }

    pub fn is_test(&self) -> bool {
        self.tags.iter().any(|t| {
            let lower = t.to_lowercase();
            lower == "test"
                || lower == "tests"
                || lower == "check"
                || lower == "typecheck"
                || lower == "lint"
                || lower == "contract"
        }) || {
            let id = self.id.as_str().to_lowercase();
            id.contains("test") || id.contains("check") || id.contains("lint")
        }
    }
}

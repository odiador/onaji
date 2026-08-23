use super::task::TaskId;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PipelineId(pub String);

impl PipelineId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for PipelineId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<&str> for PipelineId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl From<String> for PipelineId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pipeline {
    pub id: PipelineId,
    pub description: Option<String>,
    pub tasks: Vec<TaskId>,
}

impl Pipeline {
    pub fn new(id: impl Into<PipelineId>, tasks: Vec<TaskId>) -> Self {
        Self {
            id: id.into(),
            description: None,
            tasks,
        }
    }

    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }
}

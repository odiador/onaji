use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ResourceId(pub String);

impl ResourceId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ResourceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<&str> for ResourceId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl From<&String> for ResourceId {
    fn from(s: &String) -> Self {
        Self(s.clone())
    }
}

impl From<String> for ResourceId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResourceType {
    File {
        path: PathBuf,
        debounce_ms: u64,
    },
    Http {
        url: String,
        interval_ms: u64,
        headers: HashMap<String, String>,
    },
    Process {
        command: String,
    },
    Git {
        path: PathBuf,
        branch: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resource {
    pub id: ResourceId,
    pub resource_type: ResourceType,
    pub description: Option<String>,
}

impl Resource {
    pub fn new(id: impl Into<ResourceId>, resource_type: ResourceType) -> Self {
        Self {
            id: id.into(),
            resource_type,
            description: None,
        }
    }

    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }
}

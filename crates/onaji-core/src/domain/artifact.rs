use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ArtifactId(pub String);

impl ArtifactId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ArtifactId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<&str> for ArtifactId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl From<&String> for ArtifactId {
    fn from(s: &String) -> Self {
        Self(s.clone())
    }
}

impl From<String> for ArtifactId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Artifact {
    pub id: ArtifactId,
    pub description: Option<String>,
    pub metadata: HashMap<String, String>,
}

impl Artifact {
    pub fn new(id: impl Into<ArtifactId>) -> Self {
        Self {
            id: id.into(),
            description: None,
            metadata: HashMap::new(),
        }
    }

    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }
}

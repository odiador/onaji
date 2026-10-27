use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawManifest {
    pub version: String,
    pub name: String,
    #[serde(default)]
    pub resources: HashMap<String, RawResource>,
    #[serde(default)]
    pub artifacts: HashMap<String, RawArtifact>,
    #[serde(default)]
    pub tasks: HashMap<String, RawTask>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum RawResource {
    #[serde(rename = "file")]
    File {
        path: String,
        #[serde(default = "default_debounce")]
        debounce_ms: u64,
        description: Option<String>,
    },
    #[serde(rename = "http")]
    Http {
        url: String,
        #[serde(default = "default_interval")]
        interval_ms: u64,
        #[serde(default)]
        headers: HashMap<String, String>,
        description: Option<String>,
    },
    #[serde(rename = "process")]
    Process {
        command: String,
        description: Option<String>,
    },
    #[serde(rename = "git")]
    Git {
        path: String,
        branch: Option<String>,
        description: Option<String>,
    },
}

fn default_debounce() -> u64 {
    300
}

fn default_interval() -> u64 {
    2000
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RawArtifact {
    pub description: Option<String>,
    #[serde(default)]
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawTask {
    pub description: Option<String>,
    #[serde(default)]
    pub inputs: Vec<String>,
    #[serde(default)]
    pub outputs: Vec<String>,
    pub run: String,
    pub working_dir: Option<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default)]
    pub cache: Option<RawTaskCache>,
    #[serde(default)]
    pub priority: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub timeout_secs: Option<u64>,
    #[serde(default)]
    pub retries: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RawTaskCache {
    #[serde(default)]
    pub paths: Vec<String>,
}

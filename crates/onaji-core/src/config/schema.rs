use serde::{Deserialize, Deserializer, Serialize};
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
    #[serde(default)]
    pub cluster: Option<RawCluster>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RawCluster {
    #[serde(default)]
    pub workers: Vec<RawWorkerConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawWorkerConfig {
    pub id: String,
    pub address: String,
    #[serde(default, deserialize_with = "deserialize_toolchains")]
    pub toolchains: HashMap<String, String>,
}

fn deserialize_toolchains<'de, D>(deserializer: D) -> Result<HashMap<String, String>, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum ToolchainsInput {
        Map(HashMap<String, String>),
        List(Vec<String>),
    }

    match ToolchainsInput::deserialize(deserializer)? {
        ToolchainsInput::Map(m) => Ok(m),
        ToolchainsInput::List(list) => Ok(list
            .into_iter()
            .map(|item| (item, "installed".to_string()))
            .collect()),
    }
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

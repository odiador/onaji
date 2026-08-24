use crate::adapter::Capability;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginPermissions {
    #[serde(default)]
    pub workspace_filesystem: bool,
    #[serde(default)]
    pub network: bool,
    #[serde(default)]
    pub environment: Vec<String>,
}

impl Default for PluginPermissions {
    fn default() -> Self {
        Self {
            workspace_filesystem: true,
            network: false,
            environment: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginRuntime {
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: std::collections::HashMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginManifest {
    pub name: String,
    pub version: String,
    #[serde(default = "default_protocol_version")]
    pub protocol: String,
    #[serde(default)]
    pub capabilities: Vec<Capability>,
    #[serde(default)]
    pub resources: Vec<String>,
    pub runtime: PluginRuntime,
    #[serde(default)]
    pub permissions: PluginPermissions,
    #[serde(skip)]
    pub manifest_path: Option<PathBuf>,
}

fn default_protocol_version() -> String {
    "1".to_string()
}

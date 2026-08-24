use crate::adapter::Capability;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginInitializeParams {
    pub protocol_version: String,
    pub workspace_root: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PluginInitializeResult {
    pub name: String,
    pub version: String,
    pub capabilities: Vec<Capability>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginContextDto {
    pub workspace_root: String,
    pub resource_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginExecuteParams {
    pub context: PluginContextDto,
    pub target: String,
}

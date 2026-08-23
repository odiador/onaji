use serde::{Deserialize, Serialize};

/// Versión del protocolo JSON-RPC de Onaji
pub const PROTOCOL_VERSION: &str = "0.1.0";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InitializeParams {
    pub workspace_root: String,
    pub client_info: ClientInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ClientInfo {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InitializeResult {
    pub engine_version: String,
    pub active_pipelines: usize,
    pub registered_resources: usize,
}

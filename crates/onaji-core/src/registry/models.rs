use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegistryVersionMeta {
    pub version: String,
    pub protocol_version: String,
    pub runtime: String,
    pub sha256: String,
    pub signature_hex: String,
    pub public_key: String,
    pub content_address: String,
    pub size_bytes: u64,
    pub published_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegistryPluginMeta {
    pub name: String,
    pub description: String,
    pub author: String,
    pub latest_version: String,
    pub versions: HashMap<String, RegistryVersionMeta>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublishRequest {
    pub name: String,
    pub version: String,
    pub protocol_version: String,
    pub runtime: String,
    pub sha256: String,
    pub signature_hex: String,
    pub public_key: String,
    pub description: String,
    pub author: String,
    pub artifact_base64: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublishResponse {
    pub status: String,
    pub name: String,
    pub version: String,
    pub content_address: String,
    pub message: String,
}

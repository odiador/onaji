use super::key::CacheKey;
use super::store::TaskState;
use crate::domain::TaskId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheMetadata {
    pub created_at_epoch_s: u64,
    pub onaji_version: String,
    pub protocol_version: String,
    pub platform: String,
}

impl Default for CacheMetadata {
    fn default() -> Self {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        Self {
            created_at_epoch_s: timestamp,
            onaji_version: env!("CARGO_PKG_VERSION").to_string(),
            protocol_version: onaji_protocol::PROTOCOL_VERSION.to_string(),
            platform: std::env::consts::OS.to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEntry {
    pub key: CacheKey,
    pub task_id: TaskId,
    pub state: TaskState,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub duration_ms: u64,
    pub artifacts: HashMap<String, String>,
    pub metadata: CacheMetadata,
}

impl CacheEntry {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        key: CacheKey,
        task_id: TaskId,
        state: TaskState,
        exit_code: Option<i32>,
        stdout: String,
        stderr: String,
        duration_ms: u64,
        artifacts: HashMap<String, String>,
    ) -> Self {
        Self {
            key,
            task_id,
            state,
            exit_code,
            stdout,
            stderr,
            duration_ms,
            artifacts,
            metadata: CacheMetadata::default(),
        }
    }
}

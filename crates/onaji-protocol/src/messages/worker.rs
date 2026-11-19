use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkerRegistration {
    pub worker_id: String,
    pub os: String,
    pub arch: String,
    pub toolchains: HashMap<String, String>,
    pub max_parallel_tasks: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskAssignment {
    pub task_id: String,
    pub workspace_id: String,
    pub action_cmd: String,
    pub env: HashMap<String, String>,
    pub cache_key: String,
    pub timeout_ms: u64,
    #[serde(default)]
    pub cache_paths: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth_token: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inputs_tar_zst_hex: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskExecutionReport {
    pub task_id: String,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub duration_ms: u64,
    pub success: bool,
    #[serde(default)]
    pub worker_sandbox: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifacts_tar_zst_hex: Option<String>,
    #[serde(default)]
    pub transport_error: bool,
}

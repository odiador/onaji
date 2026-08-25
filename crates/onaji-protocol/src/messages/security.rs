use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuthCredentials {
    Token(String),
    Certificate {
        subject: String,
        fingerprint: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecurityPolicy {
    pub allow_commands: Vec<String>,
    pub deny_commands: Vec<String>,
    pub allow_network: bool,
    pub allowed_env: Vec<String>,
    pub memory_limit_mb: Option<u64>,
    pub timeout_secs: Option<u64>,
}

impl Default for SecurityPolicy {
    fn default() -> Self {
        Self {
            allow_commands: vec![
                "cargo".to_string(),
                "gradle".to_string(),
                "npm".to_string(),
                "node".to_string(),
                "echo".to_string(),
                "sh".to_string(),
            ],
            deny_commands: vec![
                "sudo".to_string(),
                "shutdown".to_string(),
                "reboot".to_string(),
                "rm -rf /".to_string(),
            ],
            allow_network: false,
            allowed_env: vec![
                "PATH".to_string(),
                "JAVA_HOME".to_string(),
                "ANDROID_HOME".to_string(),
                "RUST_LOG".to_string(),
                "HOME".to_string(),
            ],
            memory_limit_mb: Some(4096),
            timeout_secs: Some(300),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ViolationType {
    ForbiddenCommand,
    PathTraversal,
    NetworkDenied,
    TimeoutExceeded,
    UnauthorizedWorker,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecurityViolation {
    pub violation_type: ViolationType,
    pub task_id: Option<String>,
    pub details: String,
    pub timestamp: u64,
}

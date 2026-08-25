use onaji_protocol::messages::security::{SecurityPolicy, SecurityViolation, ViolationType};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default)]
pub struct PolicyChecker {
    pub policy: SecurityPolicy,
}

impl PolicyChecker {
    pub fn new(policy: SecurityPolicy) -> Self {
        Self { policy }
    }

    pub fn validate_command(&self, cmd: &str) -> Result<(), SecurityViolation> {
        let trimmed = cmd.trim();

        // 1. Check explicitly forbidden commands
        for denied in &self.policy.deny_commands {
            if trimmed.starts_with(denied) || trimmed.contains(&format!(" {} ", denied)) {
                return Err(SecurityViolation {
                    violation_type: ViolationType::ForbiddenCommand,
                    task_id: None,
                    details: format!("Command contains explicitly denied sequence: '{}'", denied),
                    timestamp: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs(),
                });
            }
        }

        // 2. Check explicitly allowed root commands
        let first_word = trimmed
            .split_whitespace()
            .next()
            .unwrap_or("")
            .trim_start_matches("./")
            .trim_start_matches("../");

        if !self
            .policy
            .allow_commands
            .iter()
            .any(|allowed| allowed == first_word)
        {
            return Err(SecurityViolation {
                violation_type: ViolationType::ForbiddenCommand,
                task_id: None,
                details: format!(
                    "Command executable '{}' is not in allowed_commands list",
                    first_word
                ),
                timestamp: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
            });
        }

        Ok(())
    }

    pub fn validate_path(
        &self,
        sandbox_root: &Path,
        target_path: &Path,
    ) -> Result<PathBuf, SecurityViolation> {
        let combined = if target_path.is_absolute() {
            target_path.to_path_buf()
        } else {
            sandbox_root.join(target_path)
        };

        // Normalize path without escaping sandbox root
        if combined.to_string_lossy().contains("..") {
            return Err(SecurityViolation {
                violation_type: ViolationType::PathTraversal,
                task_id: None,
                details: format!(
                    "Path traversal detected: '{}' contains '..' relative sequence",
                    target_path.display()
                ),
                timestamp: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
            });
        }

        if !combined.starts_with(sandbox_root) {
            return Err(SecurityViolation {
                violation_type: ViolationType::PathTraversal,
                task_id: None,
                details: format!(
                    "Path confinement violation: '{}' escapes sandbox root '{}'",
                    combined.display(),
                    sandbox_root.display()
                ),
                timestamp: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
            });
        }

        Ok(combined)
    }

    pub fn filter_environment(
        &self,
        task_env: &HashMap<String, String>,
    ) -> HashMap<String, String> {
        let mut clean_env = HashMap::new();

        // Pass declared allowed system environment variables if present
        for allowed in &self.policy.allowed_env {
            if let Ok(val) = std::env::var(allowed) {
                clean_env.insert(allowed.clone(), val);
            }
        }

        // Add task specific explicit environment variables
        for (k, v) in task_env {
            clean_env.insert(k.clone(), v.clone());
        }

        clean_env
    }
}

use crate::domain::{Action, Task, TaskId};
use crate::fingerprint::{Fingerprint, Fingerprinter};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskState {
    Pending,
    Ready,
    Running,
    Success,
    Failed,
    Cancelled,
    Skipped,
    Cached,
    Blocked,
}

impl fmt::Display for TaskState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TaskState::Pending => write!(f, "Pending"),
            TaskState::Ready => write!(f, "Ready"),
            TaskState::Running => write!(f, "Running"),
            TaskState::Success => write!(f, "Success"),
            TaskState::Failed => write!(f, "Failed"),
            TaskState::Cancelled => write!(f, "Cancelled"),
            TaskState::Skipped => write!(f, "Skipped"),
            TaskState::Cached => write!(f, "Cached"),
            TaskState::Blocked => write!(f, "Blocked"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CacheKey(pub String);

impl CacheKey {
    pub fn compute(
        task: &Task,
        workspace_root: &Path,
        input_fingerprints: &HashMap<String, Fingerprint>,
    ) -> Self {
        let mut bytes = Vec::new();

        // 1. Task ID
        bytes.extend_from_slice(task.id.as_str().as_bytes());

        // 2. Action content
        match &task.action {
            Action::Command { run, env, .. } => {
                bytes.extend_from_slice(run.as_bytes());
                let mut sorted_env: Vec<(&String, &String)> = env.iter().collect();
                sorted_env.sort();
                for (k, v) in sorted_env {
                    bytes.extend_from_slice(k.as_bytes());
                    bytes.extend_from_slice(v.as_bytes());
                }
            }
            Action::Builtin { adapter, config } => {
                bytes.extend_from_slice(adapter.as_bytes());
                let mut sorted_cfg: Vec<(&String, &String)> = config.iter().collect();
                sorted_cfg.sort();
                for (k, v) in sorted_cfg {
                    bytes.extend_from_slice(k.as_bytes());
                    bytes.extend_from_slice(v.as_bytes());
                }
            }
        }

        // 3. Inputs fingerprints
        let mut sorted_inputs: Vec<(&String, &Fingerprint)> = input_fingerprints.iter().collect();
        sorted_inputs.sort_by_key(|(k, _)| *k);
        for (name, fp) in sorted_inputs {
            bytes.extend_from_slice(name.as_bytes());
            bytes.extend_from_slice(&fp.0.to_le_bytes());
        }

        // 4. Cache paths (e.g. build.gradle.kts, package.json)
        for cache_rel_path in &task.cache_paths {
            let full_path = workspace_root.join(cache_rel_path);
            if let Ok(fp) = Fingerprinter::compute_path(&full_path) {
                bytes.extend_from_slice(cache_rel_path.to_string_lossy().as_bytes());
                bytes.extend_from_slice(&fp.0.to_le_bytes());
            }
        }

        let hash = Fingerprinter::compute_bytes(&bytes);
        Self(hash.to_hex())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskExecutionRecord {
    pub cache_key: CacheKey,
    pub duration_ms: u64,
    pub timestamp_epoch_s: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExecutionCache {
    records: HashMap<String, TaskExecutionRecord>,
}

impl ExecutionCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn check_hit(&self, task_id: &TaskId, key: &CacheKey) -> bool {
        if let Some(record) = self.records.get(task_id.as_str()) {
            record.cache_key == *key
        } else {
            false
        }
    }

    pub fn record_success(&mut self, task_id: &TaskId, key: CacheKey, duration_ms: u64) {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        self.records.insert(
            task_id.as_str().to_string(),
            TaskExecutionRecord {
                cache_key: key,
                duration_ms,
                timestamp_epoch_s: timestamp,
            },
        );
    }

    pub fn invalidate(&mut self, task_id: &TaskId) {
        self.records.remove(task_id.as_str());
    }

    pub fn clear(&mut self) {
        self.records.clear();
    }
}

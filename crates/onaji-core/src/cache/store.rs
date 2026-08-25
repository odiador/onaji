use crate::domain::TaskId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

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

pub use super::key::CacheKey;

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

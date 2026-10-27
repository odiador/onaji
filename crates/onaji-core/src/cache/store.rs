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

    pub fn invalidate(&mut self, task_id: &TaskId) -> Option<CacheKey> {
        self.records.remove(task_id.as_str()).map(|r| r.cache_key)
    }

    pub fn clear(&mut self) {
        self.records.clear();
    }

    /// Ruta predeterminada para el almacenamiento de caché CAS local del workspace (.onaji/cache/execution_cache.json)
    pub fn default_cache_path(workspace_root: &std::path::Path) -> std::path::PathBuf {
        workspace_root
            .join(".onaji")
            .join("cache")
            .join("execution_cache.json")
    }

    /// Carga la caché desde el disco si existe, o retorna una nueva instancia vacía.
    pub fn load_or_new(workspace_root: &std::path::Path) -> Self {
        let path = Self::default_cache_path(workspace_root);
        if path.exists() {
            if let Ok(content) = std::fs::read_to_string(&path) {
                if let Ok(cache) = serde_json::from_str(&content) {
                    return cache;
                }
            }
        }
        Self::new()
    }

    /// Persiste el estado de la caché en el disco de forma atómica mediante un archivo temporal y rename.
    pub fn save(&self, workspace_root: &std::path::Path) -> std::io::Result<()> {
        let path = Self::default_cache_path(workspace_root);
        let parent = match path.parent() {
            Some(p) => p,
            None => workspace_root,
        };
        std::fs::create_dir_all(parent)?;

        let content =
            serde_json::to_string(self).map_err(|e| std::io::Error::other(e.to_string()))?;

        let tmp_path = parent.join(format!(
            ".execution_cache.{}.{}.tmp",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));

        std::fs::write(&tmp_path, content)?;
        std::fs::rename(&tmp_path, &path)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_atomic_cache_save_and_reload() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();

        let mut cache = ExecutionCache::new();
        let task_id = TaskId::new("compile");
        let key = CacheKey::new("0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef");
        cache.record_success(&task_id, key.clone(), 120);

        assert!(cache.check_hit(&task_id, &key));
        cache.save(root).expect("atomic save should succeed");

        let loaded = ExecutionCache::load_or_new(root);
        assert!(loaded.check_hit(&task_id, &key));
    }
}

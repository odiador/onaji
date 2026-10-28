use crate::cache::{CacheKey, TaskState};
use crate::domain::{Action, Task};
use crate::runner::{ProcessExecutor, RunnerError, TaskExecutionResult};
use async_trait::async_trait;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[async_trait]
pub trait ExecutionBackend: Send + Sync {
    async fn execute_task(
        &self,
        task: &Task,
        cache_key: Option<CacheKey>,
        env: &HashMap<String, String>,
    ) -> Result<TaskExecutionResult, RunnerError>;
}

pub struct LocalExecutionBackend<'a> {
    workspace_root: &'a Path,
}

impl<'a> LocalExecutionBackend<'a> {
    pub fn new(workspace_root: &'a Path) -> Self {
        Self { workspace_root }
    }
}

#[async_trait]
impl<'a> ExecutionBackend for LocalExecutionBackend<'a> {
    async fn execute_task(
        &self,
        task: &Task,
        cache_key: Option<CacheKey>,
        _env: &HashMap<String, String>,
    ) -> Result<TaskExecutionResult, RunnerError> {
        ProcessExecutor::execute(task, self.workspace_root, cache_key).await
    }
}

fn copy_dir_all(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir_all(&from, &to)?;
        } else {
            std::fs::copy(&from, &to)?;
        }
    }
    Ok(())
}

pub struct RemoteExecutionBackend {
    coordinator: Arc<super::coordinator::ClusterCoordinator>,
    workspace_root: Option<PathBuf>,
}

impl RemoteExecutionBackend {
    pub fn new(coordinator: Arc<super::coordinator::ClusterCoordinator>) -> Self {
        Self {
            coordinator,
            workspace_root: None,
        }
    }

    pub fn with_workspace_root(mut self, root: impl Into<PathBuf>) -> Self {
        self.workspace_root = Some(root.into());
        self
    }
}

#[async_trait]
impl ExecutionBackend for RemoteExecutionBackend {
    async fn execute_task(
        &self,
        task: &Task,
        cache_key: Option<CacheKey>,
        env: &HashMap<String, String>,
    ) -> Result<TaskExecutionResult, RunnerError> {
        let cmd = match &task.action {
            Action::Command { run, .. } => run.clone(),
            Action::Builtin { adapter, .. } => format!("builtin:{}", adapter),
        };

        let timeout_ms = task
            .timeout_secs
            .map(|s| s.saturating_mul(1000))
            .unwrap_or(30000);

        let assignment = onaji_protocol::messages::worker::TaskAssignment {
            task_id: task.id.to_string(),
            workspace_id: "default".to_string(),
            action_cmd: cmd,
            env: env.clone(),
            cache_key: cache_key
                .as_ref()
                .map(|k| k.to_string())
                .unwrap_or_default(),
            timeout_ms,
            cache_paths: task
                .cache_paths
                .iter()
                .map(|p| p.to_string_lossy().to_string())
                .collect(),
        };

        let report = self.coordinator.dispatch_task(assignment).await?;
        let state = if report.success {
            TaskState::Success
        } else {
            TaskState::Failed
        };

        // Sincronizar artefactos del sandbox del worker hacia el workspace local
        if report.success {
            if let (Some(ws_root), Some(sandbox_str)) =
                (&self.workspace_root, &report.worker_sandbox)
            {
                let sandbox = Path::new(sandbox_str);
                if sandbox != ws_root.as_path() && sandbox.exists() {
                    for rel in &task.cache_paths {
                        let src = sandbox.join(rel);
                        let dst = ws_root.join(rel);
                        if let Some(parent) = dst.parent() {
                            let _ = std::fs::create_dir_all(parent);
                        }
                        if src.is_dir() {
                            let _ = copy_dir_all(&src, &dst);
                        } else if src.is_file() {
                            let _ = std::fs::copy(&src, &dst);
                        }
                    }
                }
            }
        }

        Ok(TaskExecutionResult {
            task_id: task.id.clone(),
            state,
            exit_code: report.exit_code,
            stdout: report.stdout,
            stderr: report.stderr,
            duration_ms: report.duration_ms,
            cache_key,
        })
    }
}

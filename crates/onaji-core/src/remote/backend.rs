use crate::cache::{CacheKey, TaskState};
use crate::domain::{Action, Task};
use crate::runner::{ProcessExecutor, RunnerError, TaskExecutionResult};
use async_trait::async_trait;
use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

#[async_trait]
pub trait ExecutionBackend: Send + Sync {
    async fn execute_task(
        &self,
        task: &Task,
        cache_key: Option<CacheKey>,
        env: &HashMap<String, String>,
    ) -> Result<TaskExecutionResult, RunnerError>;

    async fn execute_task_with_inputs(
        &self,
        task: &Task,
        cache_key: Option<CacheKey>,
        env: &HashMap<String, String>,
        _input_paths: &[PathBuf],
    ) -> Result<TaskExecutionResult, RunnerError> {
        self.execute_task(task, cache_key, env).await
    }
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

fn copy_dir_all_confined(src: &Path, dst: &Path, sandbox_canon: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        let meta = std::fs::symlink_metadata(&from)?;
        if meta.file_type().is_symlink() {
            let target = from.canonicalize().map_err(|_| {
                std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    format!(
                        "Security violation: unsafe symlink '{}' in worker sandbox",
                        from.display()
                    ),
                )
            })?;
            if !target.starts_with(sandbox_canon) || target.is_dir() {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    format!(
                        "Security violation: unsafe symlink '{}' in worker sandbox",
                        from.display()
                    ),
                ));
            }
            std::fs::copy(&target, &to)?;
        } else if meta.is_dir() {
            copy_dir_all_confined(&from, &to, sandbox_canon)?;
        } else if meta.is_file() {
            std::fs::copy(&from, &to)?;
        }
    }
    Ok(())
}

fn sync_sandbox_artifacts(
    sandbox: &Path,
    ws_root: &Path,
    cache_paths: &[PathBuf],
) -> Result<(), RunnerError> {
    let sandbox_canon = sandbox.canonicalize()?;
    let ws_canon = ws_root.canonicalize()?;
    if sandbox_canon == ws_canon {
        return Ok(());
    }

    for rel in cache_paths {
        if rel.is_absolute()
            || rel.components().any(|c| {
                matches!(
                    c,
                    Component::ParentDir | Component::RootDir | Component::Prefix(_)
                )
            })
        {
            return Err(RunnerError::ExecutionFailed {
                code: Some(126),
                stderr: format!(
                    "Security violation: unsafe cache_path '{}' in worker artifact sync",
                    rel.display()
                ),
            });
        }

        let src = sandbox_canon.join(rel);
        let dst = ws_canon.join(rel);

        match std::fs::symlink_metadata(&src) {
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => return Err(RunnerError::Io(e)),
        }

        let src_canon = src
            .canonicalize()
            .map_err(|_| RunnerError::ExecutionFailed {
                code: Some(126),
                stderr: format!(
                    "Security violation: unsafe symlink '{}' in worker sandbox",
                    src.display()
                ),
            })?;
        if !src_canon.starts_with(&sandbox_canon) {
            return Err(RunnerError::ExecutionFailed {
                code: Some(126),
                stderr: format!(
                    "Security violation: cache_path '{}' resolves outside worker sandbox",
                    rel.display()
                ),
            });
        }

        if let Some(parent) = dst.parent() {
            std::fs::create_dir_all(parent)?;
        }
        if src_canon.is_dir() {
            copy_dir_all_confined(&src_canon, &dst, &sandbox_canon).map_err(|e| {
                if e.kind() == std::io::ErrorKind::PermissionDenied {
                    RunnerError::ExecutionFailed {
                        code: Some(126),
                        stderr: e.to_string(),
                    }
                } else {
                    RunnerError::Io(e)
                }
            })?;
        } else if src_canon.is_file() {
            std::fs::copy(&src_canon, &dst)?;
        }
    }

    Ok(())
}

fn unpack_remote_artifacts_safely(ws_root: &Path, archive_bytes: &[u8]) -> Result<(), RunnerError> {
    super::worker::unpack_tar_zst_safely(ws_root, archive_bytes).map_err(|msg| {
        RunnerError::ExecutionFailed {
            code: Some(126),
            stderr: msg,
        }
    })
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
        self.execute_task_with_inputs(task, cache_key, env, &[])
            .await
    }

    async fn execute_task_with_inputs(
        &self,
        task: &Task,
        cache_key: Option<CacheKey>,
        env: &HashMap<String, String>,
        input_paths: &[PathBuf],
    ) -> Result<TaskExecutionResult, RunnerError> {
        let cmd = match &task.action {
            Action::Command { run, .. } => run.clone(),
            Action::Builtin { adapter, .. } => format!("builtin:{}", adapter),
        };

        let timeout_ms = task
            .timeout_secs
            .map(|s| s.saturating_mul(1000))
            .unwrap_or(30000);

        let inputs_tar_zst_hex = if !input_paths.is_empty() {
            if let Some(ws_root) = &self.workspace_root {
                let input_strs: Vec<String> = input_paths
                    .iter()
                    .map(|p| p.to_string_lossy().to_string())
                    .collect();
                let packed = super::worker::pack_paths_to_tar_zst_safely(ws_root, &input_strs)
                    .map_err(|msg| RunnerError::ExecutionFailed {
                        code: Some(126),
                        stderr: msg,
                    })?;
                packed.map(hex::encode)
            } else {
                None
            }
        } else {
            None
        };

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
            tags: task.tags.clone(),
            auth_token: None,
            inputs_tar_zst_hex,
        };

        let report = self.coordinator.dispatch_task(assignment).await?;
        let state = if report.success {
            TaskState::Success
        } else {
            TaskState::Failed
        };

        // Sincronizar artefactos desde reporte en red o sandbox local hacia el workspace local
        if report.success {
            if let Some(ws_root) = &self.workspace_root {
                if let Some(hex_archive) = &report.artifacts_tar_zst_hex {
                    let bytes =
                        hex::decode(hex_archive).map_err(|err| RunnerError::ExecutionFailed {
                            code: Some(1),
                            stderr: format!(
                                "Failed to decode remote artifact archive hex: {}",
                                err
                            ),
                        })?;
                    unpack_remote_artifacts_safely(ws_root, &bytes)?;
                } else if let Some(sandbox_str) = &report.worker_sandbox {
                    let sandbox = Path::new(sandbox_str);
                    if sandbox.exists() {
                        sync_sandbox_artifacts(sandbox, ws_root, &task.cache_paths)?;
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

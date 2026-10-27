use crate::cache::{CacheKey, TaskState};
use crate::domain::{Action, Task, TaskId};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::Instant;
use thiserror::Error;
use tokio::process::Command;

#[derive(Debug, Error)]
pub enum RunnerError {
    #[error("Failed to spawn process: {0}")]
    Io(#[from] std::io::Error),

    #[error("Command failed with non-zero exit code ({code:?}): {stderr}")]
    ExecutionFailed { code: Option<i32>, stderr: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskExecutionResult {
    pub task_id: TaskId,
    pub state: TaskState,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub duration_ms: u64,
    pub cache_key: Option<CacheKey>,
}

pub struct ProcessExecutor;

impl ProcessExecutor {
    pub async fn execute(
        task: &Task,
        workspace_root: &Path,
        cache_key: Option<CacheKey>,
    ) -> Result<TaskExecutionResult, RunnerError> {
        let start = Instant::now();

        match &task.action {
            Action::Command {
                run,
                working_dir,
                env,
            } => {
                // 1. Validar y confinar working_dir contra path traversal
                let cwd = match working_dir {
                    Some(rel) => {
                        let candidate = if rel.is_absolute() {
                            rel.to_path_buf()
                        } else {
                            workspace_root.join(rel)
                        };
                        if let (Ok(canon_cwd), Ok(canon_root)) =
                            (candidate.canonicalize(), workspace_root.canonicalize())
                        {
                            if !canon_cwd.starts_with(&canon_root) {
                                return Err(RunnerError::ExecutionFailed {
                                    code: Some(126),
                                    stderr: format!(
                                        "Security violation: path traversal detected outside workspace root for '{}'",
                                        rel.display()
                                    ),
                                });
                            }
                        }
                        candidate
                    }
                    None => workspace_root.to_path_buf(),
                };

                // 2. Bloqueo de comandos destructivos del sistema
                let trimmed = run.trim();
                let dangerous_patterns = [
                    "rm -rf /",
                    "sudo ",
                    "shutdown ",
                    "reboot ",
                    "mkfs ",
                    ":(){ :|:& };:",
                ];
                for pattern in dangerous_patterns {
                    if trimmed == pattern
                        || trimmed.starts_with(&format!("{pattern} "))
                        || trimmed.contains(&format!(" {pattern}"))
                        || trimmed.contains(&format!(";{pattern}"))
                    {
                        return Err(RunnerError::ExecutionFailed {
                            code: Some(126),
                            stderr: format!("Security violation: forbidden destructive command pattern '{pattern}' detected"),
                        });
                    }
                }

                let mut cmd = if cfg!(target_os = "windows") {
                    let mut c = Command::new("cmd");
                    c.args(["/C", run]);
                    c
                } else {
                    let mut c = Command::new("sh");
                    c.args(["-c", run]);
                    c
                };

                cmd.current_dir(cwd);
                cmd.kill_on_drop(true);
                for (k, v) in env {
                    cmd.env(k, v);
                }

                let effective_timeout = task.timeout_secs.or_else(|| {
                    std::env::var("ONAJI_DEFAULT_TIMEOUT_SECS")
                        .ok()
                        .and_then(|v| v.parse().ok())
                });

                let output = match effective_timeout {
                    Some(secs) => {
                        match tokio::time::timeout(
                            std::time::Duration::from_secs(secs),
                            cmd.output(),
                        )
                        .await
                        {
                            Ok(res) => res?,
                            Err(_) => {
                                let duration_ms = start.elapsed().as_millis() as u64;
                                return Ok(TaskExecutionResult {
                                    task_id: task.id.clone(),
                                    state: TaskState::Failed,
                                    exit_code: Some(124),
                                    stdout: String::new(),
                                    stderr: format!(
                                        "Execution timeout: task exceeded deadline of {} seconds",
                                        secs
                                    ),
                                    duration_ms,
                                    cache_key,
                                });
                            }
                        }
                    }
                    None => cmd.output().await?,
                };

                let duration_ms = start.elapsed().as_millis() as u64;

                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                let exit_code = output.status.code();

                if output.status.success() {
                    Ok(TaskExecutionResult {
                        task_id: task.id.clone(),
                        state: TaskState::Success,
                        exit_code,
                        stdout,
                        stderr,
                        duration_ms,
                        cache_key,
                    })
                } else {
                    Ok(TaskExecutionResult {
                        task_id: task.id.clone(),
                        state: TaskState::Failed,
                        exit_code,
                        stdout,
                        stderr,
                        duration_ms,
                        cache_key,
                    })
                }
            }
            Action::Builtin { adapter, .. } => {
                // Builtin mock for MVP
                let duration_ms = start.elapsed().as_millis() as u64;
                Ok(TaskExecutionResult {
                    task_id: task.id.clone(),
                    state: TaskState::Success,
                    exit_code: Some(0),
                    stdout: format!("Executed builtin adapter: {}", adapter),
                    stderr: String::new(),
                    duration_ms,
                    cache_key,
                })
            }
        }
    }
}

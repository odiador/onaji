use crate::cache::{CacheKey, TaskState};
use crate::domain::{Action, Task, TaskId};
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

#[derive(Debug, Clone)]
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
                let cwd = match working_dir {
                    Some(rel) => workspace_root.join(rel),
                    None => workspace_root.to_path_buf(),
                };

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
                for (k, v) in env {
                    cmd.env(k, v);
                }

                let output = cmd.output().await?;
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

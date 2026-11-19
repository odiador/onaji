use crate::cache::{CacheKey, TaskState};
use crate::domain::{Action, Task, TaskId};
use serde::{Deserialize, Serialize};
use std::ffi::OsString;
use std::path::{Component, Path, PathBuf};
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

fn normalize_lexical(path: &Path) -> PathBuf {
    let mut parts: Vec<Component<'_>> = Vec::new();
    let has_root = path.has_root();
    for comp in path.components() {
        match comp {
            Component::CurDir => {}
            Component::ParentDir => {
                if matches!(parts.last(), Some(Component::Normal(_))) {
                    parts.pop();
                } else if !has_root {
                    parts.push(comp);
                }
            }
            other => parts.push(other),
        }
    }
    parts.iter().collect()
}

fn resolve_for_confinement(path: &Path) -> PathBuf {
    let norm = normalize_lexical(path);
    if let Ok(canon) = norm.canonicalize() {
        return canon;
    }
    let mut missing: Vec<OsString> = Vec::new();
    let mut cursor = norm.as_path();
    while !cursor.exists() {
        if let Some(name) = cursor.file_name() {
            missing.push(name.to_os_string());
        } else {
            break;
        }
        match cursor.parent() {
            Some(p) if !p.as_os_str().is_empty() => cursor = p,
            _ => {
                if !norm.has_root() {
                    cursor = Path::new(".");
                }
                break;
            }
        }
    }
    let mut resolved = cursor
        .canonicalize()
        .unwrap_or_else(|_| cursor.to_path_buf());
    for seg in missing.into_iter().rev() {
        resolved.push(seg);
    }
    resolved
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
                        let canon_root = workspace_root
                            .canonicalize()
                            .unwrap_or_else(|_| resolve_for_confinement(workspace_root));

                        if !rel.is_absolute() {
                            let mut prefix = canon_root.clone();
                            for comp in rel.components() {
                                match comp {
                                    Component::CurDir => {}
                                    Component::ParentDir => {
                                        if !prefix.pop() || !prefix.starts_with(&canon_root) {
                                            return Err(RunnerError::ExecutionFailed {
                                                code: Some(126),
                                                stderr: format!(
                                                    "Security violation: path traversal detected outside workspace root for '{}'",
                                                    rel.display()
                                                ),
                                            });
                                        }
                                    }
                                    Component::Normal(seg) => {
                                        prefix.push(seg);
                                        if std::fs::symlink_metadata(&prefix)
                                            .map(|m| m.file_type().is_symlink())
                                            .unwrap_or(false)
                                        {
                                            let target = resolve_for_confinement(&prefix);
                                            if !target.starts_with(&canon_root) {
                                                return Err(RunnerError::ExecutionFailed {
                                                    code: Some(126),
                                                    stderr: format!(
                                                        "Security violation: path traversal detected outside workspace root for '{}'",
                                                        rel.display()
                                                    ),
                                                });
                                            }
                                            prefix = target;
                                        }
                                    }
                                    Component::RootDir | Component::Prefix(_) => {}
                                }
                            }
                        }

                        let candidate = if rel.is_absolute() {
                            rel.to_path_buf()
                        } else {
                            workspace_root.join(rel)
                        };

                        let resolved_candidate = resolve_for_confinement(&candidate);
                        if !resolved_candidate.starts_with(&canon_root) {
                            return Err(RunnerError::ExecutionFailed {
                                code: Some(126),
                                stderr: format!(
                                    "Security violation: path traversal detected outside workspace root for '{}'",
                                    rel.display()
                                ),
                            });
                        }

                        if let Ok(canon_cwd) = candidate.canonicalize() {
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

                // Inyectar variables contextuales de ejecución ONΛJI
                cmd.env("ONAJI_TASK_ID", task.id.as_str());
                let canon_ws = workspace_root
                    .canonicalize()
                    .unwrap_or_else(|_| workspace_root.to_path_buf());
                cmd.env("ONAJI_WORKSPACE_ROOT", canon_ws);
                if let Some(key) = &cache_key {
                    cmd.env("ONAJI_CACHE_KEY", &key.0);
                }
                let mode =
                    std::env::var("ONAJI_EXECUTION_MODE").unwrap_or_else(|_| "local".to_string());
                cmd.env("ONAJI_EXECUTION_MODE", mode);

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

use super::policy::PolicyChecker;
use crate::domain::Action;
use crate::runner::RunnerError;
use std::collections::HashMap;
use std::path::Path;
use std::process::Command;
use std::time::Instant;

pub struct SandboxedProcessRunner {
    pub checker: PolicyChecker,
    pub sandbox_root: std::path::PathBuf,
}

impl SandboxedProcessRunner {
    pub fn new(sandbox_root: impl AsRef<Path>, checker: PolicyChecker) -> Self {
        Self {
            sandbox_root: sandbox_root.as_ref().to_path_buf(),
            checker,
        }
    }

    pub fn execute_action(
        &self,
        action: &Action,
        extra_env: &HashMap<String, String>,
    ) -> Result<(i32, String, String, u64), RunnerError> {
        let start = Instant::now();
        std::fs::create_dir_all(&self.sandbox_root).ok();

        match action {
            Action::Command {
                run,
                working_dir,
                env,
            } => {
                // 1. Authorize command
                if let Err(violation) = self.checker.validate_command(run) {
                    return Err(RunnerError::ExecutionFailed {
                        code: Some(126),
                        stderr: format!(
                            "Security Violation [{}]: {}",
                            violation.violation_type as u8, violation.details
                        ),
                    });
                }

                // 2. Validate and confine working directory
                let cwd = match working_dir {
                    Some(dir) => self
                        .checker
                        .validate_path(&self.sandbox_root, dir)
                        .map_err(|v| RunnerError::ExecutionFailed {
                            code: Some(126),
                            stderr: format!("Security Violation: {}", v.details),
                        })?,
                    None => self.sandbox_root.clone(),
                };

                // 3. Scrub and isolate environment
                let mut merged_env = env.clone();
                for (k, v) in extra_env {
                    merged_env.insert(k.clone(), v.clone());
                }
                let scrubbed_env = self.checker.filter_environment(&merged_env);

                #[cfg(unix)]
                let mut cmd = Command::new("sh");
                #[cfg(unix)]
                cmd.arg("-c").arg(run);

                #[cfg(windows)]
                let mut cmd = Command::new("cmd");
                #[cfg(windows)]
                cmd.arg("/C").arg(run);

                cmd.current_dir(cwd);
                cmd.env_clear();
                for (k, v) in scrubbed_env {
                    cmd.env(k, v);
                }

                match cmd.output() {
                    Ok(output) => {
                        let duration_ms = start.elapsed().as_millis() as u64;
                        let exit_code = output.status.code().unwrap_or(1);
                        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

                        if output.status.success() {
                            Ok((exit_code, stdout, stderr, duration_ms))
                        } else {
                            Err(RunnerError::ExecutionFailed {
                                code: Some(exit_code),
                                stderr,
                            })
                        }
                    }
                    Err(err) => Err(RunnerError::Io(err)),
                }
            }
            Action::Builtin { adapter, .. } => Ok((
                0,
                format!("Executed builtin adapter '{}'", adapter),
                String::new(),
                1,
            )),
        }
    }
}

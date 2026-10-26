use onaji_protocol::messages::worker::{TaskAssignment, TaskExecutionReport, WorkerRegistration};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

pub struct WorkerDaemon {
    pub registration: WorkerRegistration,
    pub sandbox_root: PathBuf,
}

impl WorkerDaemon {
    pub fn new(
        worker_id: impl Into<String>,
        sandbox_root: impl AsRef<Path>,
        toolchains: HashMap<String, String>,
    ) -> Self {
        let registration = WorkerRegistration {
            worker_id: worker_id.into(),
            os: std::env::consts::OS.to_string(),
            arch: std::env::consts::ARCH.to_string(),
            toolchains,
            max_parallel_tasks: 4,
        };

        Self {
            registration,
            sandbox_root: sandbox_root.as_ref().to_path_buf(),
        }
    }

    pub fn execute_assignment(&self, assignment: &TaskAssignment) -> TaskExecutionReport {
        let start = Instant::now();
        std::fs::create_dir_all(&self.sandbox_root).ok();

        let mut cmd = if let Some(image) = assignment.env.get("ONAJI_CONTAINER_IMAGE") {
            let engine =
                std::env::var("ONAJI_CONTAINER_ENGINE").unwrap_or_else(|_| "docker".to_string());
            let mut c = Command::new(&engine);
            c.arg("run")
                .arg("--rm")
                .arg("-i")
                .arg("-v")
                .arg(format!("{}:/workspace", self.sandbox_root.display()))
                .arg("-w")
                .arg("/workspace");
            for (k, v) in &assignment.env {
                if k != "ONAJI_CONTAINER_IMAGE" {
                    c.arg("-e").arg(format!("{}={}", k, v));
                }
            }
            c.arg(image);
            c.arg("sh").arg("-c").arg(&assignment.action_cmd);
            c
        } else {
            #[cfg(unix)]
            let mut c = {
                let mut cmd = Command::new("sh");
                cmd.arg("-c").arg(&assignment.action_cmd);
                cmd
            };

            #[cfg(windows)]
            let mut c = {
                let mut cmd = Command::new("cmd");
                cmd.arg("/C").arg(&assignment.action_cmd);
                cmd
            };

            c.current_dir(&self.sandbox_root);
            for (k, v) in &assignment.env {
                c.env(k, v);
            }
            c
        };

        match cmd.output() {
            Ok(output) => {
                let duration_ms = start.elapsed().as_millis() as u64;
                let exit_code = output.status.code();
                let success = output.status.success();
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();

                TaskExecutionReport {
                    task_id: assignment.task_id.clone(),
                    exit_code,
                    stdout,
                    stderr,
                    duration_ms,
                    success,
                }
            }
            Err(err) => {
                let duration_ms = start.elapsed().as_millis() as u64;
                TaskExecutionReport {
                    task_id: assignment.task_id.clone(),
                    exit_code: Some(1),
                    stdout: String::new(),
                    stderr: format!("Failed to spawn process on worker: {}", err),
                    duration_ms,
                    success: false,
                }
            }
        }
    }
}

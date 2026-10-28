use async_trait::async_trait;
use onaji_protocol::messages::worker::{TaskAssignment, TaskExecutionReport, WorkerRegistration};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;
use tokio::process::Command;

#[async_trait]
pub trait WorkerHandle: Send + Sync {
    fn registration(&self) -> &WorkerRegistration;
    async fn execute_assignment(&self, assignment: &TaskAssignment) -> TaskExecutionReport;
}

pub struct WorkerDaemon {
    pub registration: WorkerRegistration,
    pub sandbox_root: PathBuf,
}

#[async_trait]
impl WorkerHandle for WorkerDaemon {
    fn registration(&self) -> &WorkerRegistration {
        &self.registration
    }

    async fn execute_assignment(&self, assignment: &TaskAssignment) -> TaskExecutionReport {
        self.execute_assignment(assignment).await
    }
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

    pub async fn execute_assignment(&self, assignment: &TaskAssignment) -> TaskExecutionReport {
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

        cmd.kill_on_drop(true);

        let wait_result = if assignment.timeout_ms > 0 {
            match tokio::time::timeout(
                std::time::Duration::from_millis(assignment.timeout_ms),
                cmd.output(),
            )
            .await
            {
                Ok(res) => res,
                Err(_) => {
                    let duration_ms = start.elapsed().as_millis() as u64;
                    return TaskExecutionReport {
                        task_id: assignment.task_id.clone(),
                        exit_code: Some(124),
                        stdout: String::new(),
                        stderr: format!("Remote task timed out after {}ms", assignment.timeout_ms),
                        duration_ms,
                        success: false,
                    };
                }
            }
        } else {
            cmd.output().await
        };

        match wait_result {
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
                    stderr: format!("Process execution error on worker: {}", err),
                    duration_ms,
                    success: false,
                }
            }
        }
    }

    /// Inicia un servidor TCP para recibir y ejecutar asignaciones de tareas de forma distribuida
    pub async fn run_server(self: Arc<Self>, bind_addr: &str) -> std::io::Result<()> {
        use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
        use tokio::net::TcpListener;

        let listener = TcpListener::bind(bind_addr).await?;

        loop {
            let (stream, _) = listener.accept().await?;
            let worker = Arc::clone(&self);

            tokio::spawn(async move {
                let (reader, mut writer) = tokio::io::split(stream);
                let mut lines = BufReader::new(reader).lines();

                while let Ok(Some(line)) = lines.next_line().await {
                    let line = line.trim();
                    if line.is_empty() {
                        continue;
                    }

                    let assignment = match serde_json::from_str::<TaskAssignment>(line) {
                        Ok(a) => a,
                        Err(err) => {
                            let report = TaskExecutionReport {
                                task_id: "unknown".to_string(),
                                exit_code: Some(1),
                                stdout: String::new(),
                                stderr: format!("Invalid TaskAssignment JSON: {}", err),
                                duration_ms: 0,
                                success: false,
                            };
                            if let Ok(mut resp_str) = serde_json::to_string(&report) {
                                resp_str.push('\n');
                                let _ = writer.write_all(resp_str.as_bytes()).await;
                                let _ = writer.flush().await;
                            }
                            continue;
                        }
                    };

                    let report = worker.execute_assignment(&assignment).await;
                    if let Ok(mut resp_str) = serde_json::to_string(&report) {
                        resp_str.push('\n');
                        let _ = writer.write_all(resp_str.as_bytes()).await;
                        let _ = writer.flush().await;
                    }
                }
            });
        }
    }
}

/// Cliente de red para comunicarse y despachar tareas a daemons de workers remotos sobre TCP
#[derive(Clone, Debug)]
pub struct TcpWorkerClient {
    pub registration: WorkerRegistration,
    pub address: String,
}

impl TcpWorkerClient {
    pub fn new(
        worker_id: impl Into<String>,
        address: impl Into<String>,
        toolchains: HashMap<String, String>,
    ) -> Self {
        let registration = WorkerRegistration {
            worker_id: worker_id.into(),
            os: "remote".to_string(),
            arch: "remote".to_string(),
            toolchains,
            max_parallel_tasks: 4,
        };
        Self {
            registration,
            address: address.into(),
        }
    }

    pub fn with_registration(registration: WorkerRegistration, address: impl Into<String>) -> Self {
        Self {
            registration,
            address: address.into(),
        }
    }

    pub async fn ping(&self) -> bool {
        use tokio::net::TcpStream;
        TcpStream::connect(&self.address).await.is_ok()
    }
}

#[async_trait]
impl WorkerHandle for TcpWorkerClient {
    fn registration(&self) -> &WorkerRegistration {
        &self.registration
    }

    async fn execute_assignment(&self, assignment: &TaskAssignment) -> TaskExecutionReport {
        use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
        use tokio::net::TcpStream;

        let start = Instant::now();
        let stream = match TcpStream::connect(&self.address).await {
            Ok(s) => s,
            Err(err) => {
                let duration_ms = start.elapsed().as_millis() as u64;
                return TaskExecutionReport {
                    task_id: assignment.task_id.clone(),
                    exit_code: Some(1),
                    stdout: String::new(),
                    stderr: format!(
                        "Failed to connect to remote worker at {}: {}",
                        self.address, err
                    ),
                    duration_ms,
                    success: false,
                };
            }
        };

        let (reader, mut writer) = tokio::io::split(stream);
        let mut lines = BufReader::new(reader).lines();

        let mut req_str = match serde_json::to_string(assignment) {
            Ok(s) => s,
            Err(err) => {
                let duration_ms = start.elapsed().as_millis() as u64;
                return TaskExecutionReport {
                    task_id: assignment.task_id.clone(),
                    exit_code: Some(1),
                    stdout: String::new(),
                    stderr: format!("Failed to serialize TaskAssignment: {}", err),
                    duration_ms,
                    success: false,
                };
            }
        };
        req_str.push('\n');

        if let Err(err) = writer.write_all(req_str.as_bytes()).await {
            let duration_ms = start.elapsed().as_millis() as u64;
            return TaskExecutionReport {
                task_id: assignment.task_id.clone(),
                exit_code: Some(1),
                stdout: String::new(),
                stderr: format!("Failed to transmit task to remote worker: {}", err),
                duration_ms,
                success: false,
            };
        }
        let _ = writer.flush().await;

        match lines.next_line().await {
            Ok(Some(line)) => match serde_json::from_str::<TaskExecutionReport>(&line) {
                Ok(report) => report,
                Err(err) => {
                    let duration_ms = start.elapsed().as_millis() as u64;
                    TaskExecutionReport {
                        task_id: assignment.task_id.clone(),
                        exit_code: Some(1),
                        stdout: String::new(),
                        stderr: format!("Malformed response from remote worker: {}", err),
                        duration_ms,
                        success: false,
                    }
                }
            },
            Ok(None) => {
                let duration_ms = start.elapsed().as_millis() as u64;
                TaskExecutionReport {
                    task_id: assignment.task_id.clone(),
                    exit_code: Some(1),
                    stdout: String::new(),
                    stderr: "Remote worker connection closed before report received".to_string(),
                    duration_ms,
                    success: false,
                }
            }
            Err(err) => {
                let duration_ms = start.elapsed().as_millis() as u64;
                TaskExecutionReport {
                    task_id: assignment.task_id.clone(),
                    exit_code: Some(1),
                    stdout: String::new(),
                    stderr: format!("Error reading from remote worker: {}", err),
                    duration_ms,
                    success: false,
                }
            }
        }
    }
}

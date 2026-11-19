use async_trait::async_trait;
use onaji_protocol::messages::worker::{TaskAssignment, TaskExecutionReport, WorkerRegistration};
use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;
use tokio::process::Command;

pub const MAX_REMOTE_ARTIFACT_ARCHIVE_BYTES: usize = 32 * 1024 * 1024;

#[async_trait]
pub trait WorkerHandle: Send + Sync {
    fn registration(&self) -> &WorkerRegistration;
    async fn execute_assignment(&self, assignment: &TaskAssignment) -> TaskExecutionReport;
    async fn ping_latency(&self, _timeout: std::time::Duration) -> Option<u64> {
        Some(0)
    }
}

fn append_confined_dir_to_tar<W: std::io::Write>(
    builder: &mut tar::Builder<W>,
    rel_prefix: &Path,
    abs_dir: &Path,
    sandbox_canon: &Path,
) -> Result<(), String> {
    let entries = std::fs::read_dir(abs_dir)
        .map_err(|e| format!("Failed reading sandbox dir '{}': {}", abs_dir.display(), e))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("Failed dir entry: {}", e))?;
        let from = entry.path();
        let entry_rel = rel_prefix.join(entry.file_name());
        let meta = std::fs::symlink_metadata(&from)
            .map_err(|e| format!("Failed metadata for '{}': {}", from.display(), e))?;
        if meta.file_type().is_symlink() {
            let target = from.canonicalize().map_err(|_| {
                format!(
                    "Security violation: unsafe symlink '{}' in worker sandbox",
                    from.display()
                )
            })?;
            if !target.starts_with(sandbox_canon) || target.is_dir() {
                return Err(format!(
                    "Security violation: unsafe symlink '{}' in worker sandbox",
                    from.display()
                ));
            }
            let mut f = std::fs::File::open(&target).map_err(|e| {
                format!(
                    "Failed opening symlink target '{}': {}",
                    target.display(),
                    e
                )
            })?;
            builder
                .append_file(&entry_rel, &mut f)
                .map_err(|e| format!("Failed packing '{}': {}", entry_rel.display(), e))?;
        } else if meta.is_dir() {
            append_confined_dir_to_tar(builder, &entry_rel, &from, sandbox_canon)?;
        } else if meta.is_file() {
            let mut f = std::fs::File::open(&from)
                .map_err(|e| format!("Failed opening '{}': {}", from.display(), e))?;
            builder
                .append_file(&entry_rel, &mut f)
                .map_err(|e| format!("Failed packing '{}': {}", entry_rel.display(), e))?;
        }
    }
    Ok(())
}

fn pack_sandbox_artifacts_safely(
    sandbox: &Path,
    cache_paths: &[String],
) -> Result<Option<Vec<u8>>, String> {
    if cache_paths.is_empty() {
        return Ok(None);
    }

    let sandbox_canon = sandbox
        .canonicalize()
        .map_err(|e| format!("Failed canonicalizing worker sandbox: {}", e))?;

    let mut validated: Vec<(PathBuf, PathBuf)> = Vec::new();
    for rel_str in cache_paths {
        let rel_path = Path::new(rel_str);
        if rel_path.is_absolute()
            || rel_path.components().any(|c| {
                matches!(
                    c,
                    Component::ParentDir | Component::RootDir | Component::Prefix(_)
                )
            })
        {
            return Err(format!(
                "Security violation: unsafe cache_path '{}' in worker artifact pack",
                rel_str
            ));
        }

        let src = sandbox_canon.join(rel_path);
        match std::fs::symlink_metadata(&src) {
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => {
                return Err(format!(
                    "Failed reading metadata for '{}': {}",
                    src.display(),
                    e
                ))
            }
        }

        let src_canon = src.canonicalize().map_err(|_| {
            format!(
                "Security violation: unsafe symlink '{}' in worker sandbox",
                src.display()
            )
        })?;
        if !src_canon.starts_with(&sandbox_canon) {
            return Err(format!(
                "Security violation: cache_path '{}' resolves outside worker sandbox",
                rel_str
            ));
        }
        let rel_clean = rel_path
            .strip_prefix("./")
            .unwrap_or(rel_path)
            .to_path_buf();
        validated.push((rel_clean, src_canon));
    }

    if validated.is_empty() {
        return Ok(None);
    }

    let mut buf = Vec::new();
    {
        let encoder = zstd::Encoder::new(&mut buf, 3)
            .map_err(|e| format!("Failed creating zstd encoder: {}", e))?
            .auto_finish();
        let mut builder = tar::Builder::new(encoder);

        for (rel_clean, src_canon) in validated {
            if src_canon.is_dir() {
                append_confined_dir_to_tar(&mut builder, &rel_clean, &src_canon, &sandbox_canon)?;
            } else if src_canon.is_file() {
                let mut f = std::fs::File::open(&src_canon)
                    .map_err(|e| format!("Failed opening '{}': {}", src_canon.display(), e))?;
                builder
                    .append_file(&rel_clean, &mut f)
                    .map_err(|e| format!("Failed packing '{}': {}", rel_clean.display(), e))?;
            }
        }
        builder
            .finish()
            .map_err(|e| format!("Failed finishing artifact tar archive: {}", e))?;
    }

    if buf.len() > MAX_REMOTE_ARTIFACT_ARCHIVE_BYTES {
        return Err(format!(
            "Artifact archive exceeds max remote transfer size ({} bytes)",
            MAX_REMOTE_ARTIFACT_ARCHIVE_BYTES
        ));
    }

    Ok(Some(buf))
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
                        worker_sandbox: Some(self.sandbox_root.to_string_lossy().to_string()),
                        artifacts_tar_zst_hex: None,
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
                    worker_sandbox: Some(self.sandbox_root.to_string_lossy().to_string()),
                    artifacts_tar_zst_hex: None,
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
                    worker_sandbox: Some(self.sandbox_root.to_string_lossy().to_string()),
                    artifacts_tar_zst_hex: None,
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
                                worker_sandbox: None,
                                artifacts_tar_zst_hex: None,
                            };
                            if let Ok(mut resp_str) = serde_json::to_string(&report) {
                                resp_str.push('\n');
                                let _ = writer.write_all(resp_str.as_bytes()).await;
                                let _ = writer.flush().await;
                            }
                            continue;
                        }
                    };

                    let mut report = worker.execute_assignment(&assignment).await;
                    if report.success && !assignment.cache_paths.is_empty() {
                        match pack_sandbox_artifacts_safely(
                            &worker.sandbox_root,
                            &assignment.cache_paths,
                        ) {
                            Ok(Some(bytes)) => {
                                report.artifacts_tar_zst_hex = Some(hex::encode(bytes));
                                report.worker_sandbox = None;
                            }
                            Ok(None) => {
                                report.worker_sandbox = None;
                            }
                            Err(err_msg) => {
                                report.success = false;
                                report.exit_code = Some(126);
                                report.stderr = err_msg;
                                report.worker_sandbox = None;
                                report.artifacts_tar_zst_hex = None;
                            }
                        }
                    }
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

    pub async fn ping_latency(&self, timeout: std::time::Duration) -> Option<u64> {
        use tokio::net::TcpStream;
        let start = Instant::now();
        match tokio::time::timeout(timeout, TcpStream::connect(&self.address)).await {
            Ok(Ok(_)) => Some(start.elapsed().as_millis() as u64),
            _ => None,
        }
    }

    pub async fn ping(&self) -> bool {
        self.ping_latency(std::time::Duration::from_secs(5))
            .await
            .is_some()
    }
}

#[async_trait]
impl WorkerHandle for TcpWorkerClient {
    fn registration(&self) -> &WorkerRegistration {
        &self.registration
    }

    async fn ping_latency(&self, timeout: std::time::Duration) -> Option<u64> {
        TcpWorkerClient::ping_latency(self, timeout).await
    }

    async fn execute_assignment(&self, assignment: &TaskAssignment) -> TaskExecutionReport {
        use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
        use tokio::net::TcpStream;

        let start = Instant::now();
        let connect_timeout = if assignment.timeout_ms > 0 {
            std::time::Duration::from_millis(assignment.timeout_ms.min(5000))
        } else {
            std::time::Duration::from_secs(5)
        };

        let stream =
            match tokio::time::timeout(connect_timeout, TcpStream::connect(&self.address)).await {
                Ok(Ok(s)) => s,
                Ok(Err(err)) => {
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
                        worker_sandbox: None,
                        artifacts_tar_zst_hex: None,
                    };
                }
                Err(_) => {
                    let duration_ms = start.elapsed().as_millis() as u64;
                    return TaskExecutionReport {
                        task_id: assignment.task_id.clone(),
                        exit_code: Some(124),
                        stdout: String::new(),
                        stderr: format!(
                            "Timed out connecting to remote worker at {} after {}ms",
                            self.address,
                            connect_timeout.as_millis()
                        ),
                        duration_ms,
                        success: false,
                        worker_sandbox: None,
                        artifacts_tar_zst_hex: None,
                    };
                }
            };

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
                    worker_sandbox: None,
                    artifacts_tar_zst_hex: None,
                };
            }
        };
        req_str.push('\n');

        let io_timeout = if assignment.timeout_ms > 0 {
            std::time::Duration::from_millis(assignment.timeout_ms.saturating_add(500))
        } else {
            std::time::Duration::from_secs(300)
        };

        let io_result = tokio::time::timeout(io_timeout, async {
            let (reader, mut writer) = tokio::io::split(stream);
            let mut lines = BufReader::new(reader).lines();

            if let Err(err) = writer.write_all(req_str.as_bytes()).await {
                let duration_ms = start.elapsed().as_millis() as u64;
                return TaskExecutionReport {
                    task_id: assignment.task_id.clone(),
                    exit_code: Some(1),
                    stdout: String::new(),
                    stderr: format!("Failed to transmit task to remote worker: {}", err),
                    duration_ms,
                    success: false,
                    worker_sandbox: None,
                    artifacts_tar_zst_hex: None,
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
                            worker_sandbox: None,
                            artifacts_tar_zst_hex: None,
                        }
                    }
                },
                Ok(None) => {
                    let duration_ms = start.elapsed().as_millis() as u64;
                    TaskExecutionReport {
                        task_id: assignment.task_id.clone(),
                        exit_code: Some(1),
                        stdout: String::new(),
                        stderr: "Remote worker connection closed before report received"
                            .to_string(),
                        duration_ms,
                        success: false,
                        worker_sandbox: None,
                        artifacts_tar_zst_hex: None,
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
                        worker_sandbox: None,
                        artifacts_tar_zst_hex: None,
                    }
                }
            }
        })
        .await;

        match io_result {
            Ok(report) => report,
            Err(_) => {
                let duration_ms = start.elapsed().as_millis() as u64;
                TaskExecutionReport {
                    task_id: assignment.task_id.clone(),
                    exit_code: Some(124),
                    stdout: String::new(),
                    stderr: format!(
                        "Timed out waiting for response from remote worker at {} after {}ms",
                        self.address,
                        io_timeout.as_millis()
                    ),
                    duration_ms,
                    success: false,
                    worker_sandbox: None,
                    artifacts_tar_zst_hex: None,
                }
            }
        }
    }
}

use super::dispatcher::Dispatcher;
use onaji_protocol::error::{ErrorCode, RpcError};
use onaji_protocol::messages::{RpcRequest, RpcResponse};
use std::path::Path;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
#[cfg(unix)]
use tokio::net::UnixListener;

pub struct DaemonServer {
    dispatcher: Arc<Dispatcher>,
}

impl DaemonServer {
    pub fn new(workspace_root: impl AsRef<Path>) -> Self {
        let root = workspace_root.as_ref().to_path_buf();
        Self {
            dispatcher: Arc::new(Dispatcher::new(&root)),
        }
    }

    /// Ejecuta el servidor JSON-RPC sobre Stdin/Stdout (LSP-style stream)
    pub async fn run_stdio(&mut self) -> Result<(), std::io::Error> {
        let stdin = tokio::io::stdin();
        let mut stdout = tokio::io::stdout();
        let mut reader = BufReader::new(stdin).lines();

        while let Some(line) = reader.next_line().await? {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            let response = match serde_json::from_str::<RpcRequest>(line) {
                Ok(req) => self.dispatcher.dispatch_request(&req).await,
                Err(err) => RpcResponse::error(
                    None,
                    RpcError::new(
                        ErrorCode::ParseError,
                        format!("Invalid JSON-RPC request: {}", err),
                    ),
                ),
            };

            let mut out_str = serde_json::to_string(&response)?;
            out_str.push('\n');
            stdout.write_all(out_str.as_bytes()).await?;
            stdout.flush().await?;
        }

        Ok(())
    }

    /// Ejecuta el servidor JSON-RPC sobre un Unix Domain Socket con soporte multi-cliente concurrente
    #[cfg(unix)]
    pub async fn run_unix_socket(
        &mut self,
        socket_path: impl AsRef<Path>,
    ) -> Result<(), std::io::Error> {
        let path = socket_path.as_ref();
        if path.exists() {
            // Verificar si hay otro daemon escuchando activamente en este socket
            if tokio::net::UnixStream::connect(path).await.is_ok() {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::AddrInUse,
                    format!(
                        "Another Onaji daemon is already listening on '{}'",
                        path.display()
                    ),
                ));
            }
            // Socket huérfano / proceso previo caído: limpiamos de forma segura
            let _ = std::fs::remove_file(path);
        }

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let listener = UnixListener::bind(path)?;
        tracing::info!("Onaji Daemon listening on UDS: {}", path.display());

        while let Ok((stream, _)) = listener.accept().await {
            let dispatcher = Arc::clone(&self.dispatcher);

            tokio::spawn(async move {
                let (reader, mut writer) = tokio::io::split(stream);
                let mut lines = BufReader::new(reader).lines();

                while let Ok(Some(line)) = lines.next_line().await {
                    let line = line.trim();
                    if line.is_empty() {
                        continue;
                    }

                    let response = match serde_json::from_str::<RpcRequest>(line) {
                        Ok(req) => dispatcher.dispatch_request(&req).await,
                        Err(err) => RpcResponse::error(
                            None,
                            RpcError::new(
                                ErrorCode::ParseError,
                                format!("Invalid JSON-RPC request: {}", err),
                            ),
                        ),
                    };

                    let mut out_str = match serde_json::to_string(&response) {
                        Ok(s) => s,
                        Err(_) => continue,
                    };
                    out_str.push('\n');
                    if writer.write_all(out_str.as_bytes()).await.is_err() {
                        break;
                    }
                    if writer.flush().await.is_err() {
                        break;
                    }
                }
            });
        }

        Ok(())
    }
}

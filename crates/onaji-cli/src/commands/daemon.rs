use crate::util::resolve_workspace_root;
use std::path::{Path, PathBuf};

pub async fn handle_daemon(config_path: &Path, socket: Option<PathBuf>, _stdio: bool) {
    let workspace_root = resolve_workspace_root(config_path);
    let mut server = onaji_core::DaemonServer::new(&workspace_root);

    if let Some(sock_path) = socket {
        #[cfg(unix)]
        {
            println!("Starting Onaji Daemon on socket: {}", sock_path.display());
            if let Err(err) = server.run_unix_socket(sock_path).await {
                eprintln!("Daemon error: {}", err);
                std::process::exit(1);
            }
        }
        #[cfg(not(unix))]
        {
            eprintln!("Unix domain socket is only supported on Unix systems");
            std::process::exit(1);
        }
    } else if let Err(err) = server.run_stdio().await {
        eprintln!("Daemon stdio error: {}", err);
        std::process::exit(1);
    }
}

pub async fn handle_rpc(
    config_path: &Path,
    method: String,
    params_str: Option<String>,
    socket: Option<PathBuf>,
) {
    let workspace_root = resolve_workspace_root(config_path);
    let parsed_params: Option<serde_json::Value> =
        params_str.and_then(|s| serde_json::from_str(&s).ok());
    let req = onaji_protocol::messages::RpcRequest::new(1, method, parsed_params);

    #[cfg(unix)]
    {
        let target_sock = socket.or_else(|| {
            let default_sock = workspace_root.join(".onaji").join("daemon.sock");
            if default_sock.exists() {
                Some(default_sock)
            } else {
                None
            }
        });

        if let Some(sock_path) = target_sock {
            if sock_path.exists() {
                if let Ok(stream) = tokio::net::UnixStream::connect(&sock_path).await {
                    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
                    let (reader, mut writer) = tokio::io::split(stream);
                    let mut lines = BufReader::new(reader).lines();
                    let mut req_str = serde_json::to_string(&req).unwrap_or_default();
                    req_str.push('\n');
                    if writer.write_all(req_str.as_bytes()).await.is_ok()
                        && writer.flush().await.is_ok()
                    {
                        if let Ok(Some(resp_line)) = lines.next_line().await {
                            if let Ok(resp) = serde_json::from_str::<
                                onaji_protocol::messages::RpcResponse,
                            >(&resp_line)
                            {
                                println!("{}", serde_json::to_string_pretty(&resp).unwrap());
                                return;
                            }
                        }
                    }
                }
            }
        }
    }

    let dispatcher = onaji_core::Dispatcher::new(&workspace_root);
    let resp = dispatcher.dispatch_request(&req).await;
    println!("{}", serde_json::to_string_pretty(&resp).unwrap());
}

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
    _socket: Option<PathBuf>,
) {
    let workspace_root = resolve_workspace_root(config_path);
    let dispatcher = onaji_core::Dispatcher::new(&workspace_root);

    let parsed_params: Option<serde_json::Value> =
        params_str.and_then(|s| serde_json::from_str(&s).ok());
    let req = onaji_protocol::messages::RpcRequest::new(1, method, parsed_params);

    let resp = dispatcher.dispatch_request(&req).await;
    println!("{}", serde_json::to_string_pretty(&resp).unwrap());
}

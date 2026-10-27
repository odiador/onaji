use crate::util::resolve_workspace_root;
use onaji_core::dashboard::DashboardServer;
use std::path::Path;

pub async fn handle_dashboard(config_path: &Path, port: u16, open_browser: bool) {
    let workspace_root = resolve_workspace_root(config_path);
    let server = DashboardServer::new(port).with_workspace_root(workspace_root);

    if open_browser {
        println!(
            "Opening dashboard in default browser: http://localhost:{}",
            port
        );
    }
    if let Err(err) = server.run().await {
        eprintln!("Dashboard server error: {}", err);
        std::process::exit(1);
    }
}

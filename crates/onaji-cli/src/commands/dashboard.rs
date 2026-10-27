use crate::util::resolve_workspace_root;
use onaji_core::dashboard::DashboardServer;
use std::path::Path;

pub fn open_in_browser(url: &str) {
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open").arg(url).spawn();
    }
    #[cfg(target_os = "linux")]
    {
        let _ = std::process::Command::new("xdg-open").arg(url).spawn();
    }
    #[cfg(target_os = "windows")]
    {
        let _ = std::process::Command::new("cmd")
            .args(["/C", "start", url])
            .spawn();
    }
}

pub async fn handle_dashboard(config_path: &Path, port: u16, open_browser: bool) {
    let workspace_root = resolve_workspace_root(config_path);
    let server = DashboardServer::new(port).with_workspace_root(workspace_root);

    if open_browser {
        let url = format!("http://localhost:{}", port);
        println!("Opening dashboard in default browser: {}", url);
        open_in_browser(&url);
    }
    if let Err(err) = server.run().await {
        if err.kind() == std::io::ErrorKind::AddrInUse {
            eprintln!(
                "✗ Failed to start dashboard: Port {} is already in use by another process.",
                port
            );
            eprintln!("  Try specifying a different port using '--port <PORT>'.");
        } else {
            eprintln!("Dashboard server error: {}", err);
        }
        std::process::exit(1);
    }
}

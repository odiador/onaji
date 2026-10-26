use onaji_core::dashboard::DashboardServer;

pub async fn handle_dashboard(port: u16, open_browser: bool) {
    let server = DashboardServer::new(port);
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

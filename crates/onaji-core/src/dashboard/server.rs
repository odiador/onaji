use super::ui::DASHBOARD_HTML;
use std::io::Result as IoResult;
use std::net::SocketAddr;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

pub struct DashboardServer {
    pub port: u16,
}

impl DashboardServer {
    pub fn new(port: u16) -> Self {
        Self { port }
    }

    pub async fn run(&self) -> IoResult<()> {
        let addr = SocketAddr::from(([0, 0, 0, 0], self.port));
        let listener = TcpListener::bind(addr).await?;
        println!(
            "🚀 Onaji Web Dashboard active at http://localhost:{}",
            self.port
        );

        loop {
            let (mut socket, _) = listener.accept().await?;
            tokio::spawn(async move {
                let mut buffer = [0; 2048];
                if let Ok(n) = socket.read(&mut buffer).await {
                    if n > 0 {
                        let request_str = String::from_utf8_lossy(&buffer[..n]);
                        let (status_line, content_type, body) = if request_str
                            .starts_with("GET /api/status")
                        {
                            (
                                "HTTP/1.1 200 OK",
                                "application/json",
                                r#"{"status":"ok","version":"0.1.0","workers_online":2,"cache_hit_ratio":0.884}"#,
                            )
                        } else if request_str.starts_with("GET /api/graph") {
                            (
                                "HTTP/1.1 200 OK",
                                "application/json",
                                r#"{"nodes":[{"id":"openapi_spec","type":"resource"},{"id":"validate_openapi","type":"task"},{"id":"openapi_contract","type":"artifact"},{"id":"generate_kotlin","type":"task"}],"edges":[["openapi_spec","validate_openapi"],["validate_openapi","openapi_contract"],["openapi_contract","generate_kotlin"]]}"#,
                            )
                        } else if request_str.starts_with("GET /api/explain") {
                            (
                                "HTTP/1.1 200 OK",
                                "application/json",
                                r#"{"task_id":"compile_android","root_cause":"openapi_spec","trigger_type":"content_hash_change","cache_lookup":"miss","assigned_worker":"worker-android-02","duration_ms":8240}"#,
                            )
                        } else {
                            (
                                "HTTP/1.1 200 OK",
                                "text/html; charset=utf-8",
                                DASHBOARD_HTML,
                            )
                        };

                        let response = format!(
                            "{}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                            status_line,
                            content_type,
                            body.len(),
                            body
                        );
                        let _ = socket.write_all(response.as_bytes()).await;
                    }
                }
            });
        }
    }
}

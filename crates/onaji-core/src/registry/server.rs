use super::db::RegistryDatabase;
use super::models::PublishRequest;
use super::storage::ContentAddressedStorage;
use std::io::Result as IoResult;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

pub struct RegistryServer {
    pub port: u16,
    pub db: Arc<RegistryDatabase>,
    pub storage: Arc<ContentAddressedStorage>,
}

impl RegistryServer {
    pub fn new(
        port: u16,
        db: Arc<RegistryDatabase>,
        storage: Arc<ContentAddressedStorage>,
    ) -> Self {
        Self { port, db, storage }
    }

    pub async fn run(&self) -> IoResult<()> {
        let addr = SocketAddr::from(([0, 0, 0, 0], self.port));
        let listener = TcpListener::bind(addr).await?;
        let db = self.db.clone();
        let storage = self.storage.clone();

        loop {
            let (mut socket, _) = listener.accept().await?;
            let db = db.clone();
            let storage = storage.clone();

            tokio::spawn(async move {
                let mut buffer = vec![0; 65536];
                if let Ok(n) = socket.read(&mut buffer).await {
                    if n > 0 {
                        let request_str = String::from_utf8_lossy(&buffer[..n]);
                        let (status_line, content_type, body) =
                            if request_str.starts_with("POST /v1/plugins/publish") {
                                // Extract body
                                let body_start =
                                    request_str.find("\r\n\r\n").map(|idx| idx + 4).unwrap_or(n);
                                let json_body = &request_str[body_start..];
                                match serde_json::from_str::<PublishRequest>(json_body) {
                                    Ok(req) => match db.publish(req, &storage) {
                                        Ok(res) => (
                                            "HTTP/1.1 201 Created",
                                            "application/json",
                                            serde_json::to_string(&res).unwrap(),
                                        ),
                                        Err(err) => (
                                            "HTTP/1.1 400 Bad Request",
                                            "application/json",
                                            format!(r#"{{"error":"{}"}}"#, err),
                                        ),
                                    },
                                    Err(_) => (
                                        "HTTP/1.1 400 Bad Request",
                                        "application/json",
                                        r#"{"error":"Invalid PublishRequest JSON"}"#.to_string(),
                                    ),
                                }
                            } else if request_str.starts_with("GET /v1/plugins") {
                                // List or Search
                                let plugins = db.search("");
                                (
                                    "HTTP/1.1 200 OK",
                                    "application/json",
                                    serde_json::to_string(&plugins).unwrap(),
                                )
                            } else {
                                (
                                    "HTTP/1.1 404 Not Found",
                                    "application/json",
                                    r#"{"error":"Route not found"}"#.to_string(),
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

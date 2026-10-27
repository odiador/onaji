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
                let mut buffer = Vec::new();
                let mut chunk = [0u8; 8192];
                let mut content_length: Option<usize> = None;
                let mut body_start: Option<usize> = None;

                while let Ok(n) = socket.read(&mut chunk).await {
                    if n == 0 {
                        break;
                    }
                    buffer.extend_from_slice(&chunk[..n]);

                    if body_start.is_none() {
                        if let Some(pos) = buffer.windows(4).position(|w| w == b"\r\n\r\n") {
                            body_start = Some(pos + 4);
                            let header_str = String::from_utf8_lossy(&buffer[..pos]);
                            for line in header_str.lines() {
                                if line.to_lowercase().starts_with("content-length:") {
                                    if let Some(val) = line.split(':').nth(1) {
                                        content_length = val.trim().parse().ok();
                                    }
                                }
                            }
                        }
                    }

                    if let (Some(b_start), Some(cl)) = (body_start, content_length) {
                        if buffer.len() >= b_start + cl {
                            break;
                        }
                    } else if body_start.is_some() && content_length.is_none() {
                        break;
                    }
                }

                if !buffer.is_empty() {
                    let request_str = String::from_utf8_lossy(&buffer);
                    let (status_line, content_type, body) =
                        if request_str.starts_with("POST /v1/plugins/publish") {
                            let b_start = body_start.unwrap_or(0);
                            let json_body = if b_start < buffer.len() {
                                String::from_utf8_lossy(&buffer[b_start..])
                            } else {
                                std::borrow::Cow::Borrowed("")
                            };
                            match serde_json::from_str::<PublishRequest>(&json_body) {
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
            });
        }
    }
}

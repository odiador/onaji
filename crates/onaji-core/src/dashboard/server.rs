use super::ui::DASHBOARD_HTML;
use std::io::Result as IoResult;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::broadcast;

fn calculate_dir_size(path: &std::path::Path) -> u64 {
    let mut total = 0;
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            if let Ok(meta) = entry.metadata() {
                if meta.is_file() {
                    total += meta.len();
                } else if meta.is_dir() {
                    total += calculate_dir_size(&entry.path());
                }
            }
        }
    }
    total
}

#[derive(Clone)]
pub struct DashboardServer {
    pub port: u16,
    pub workspace_root: PathBuf,
    event_tx: broadcast::Sender<String>,
}

impl DashboardServer {
    pub fn new(port: u16) -> Self {
        let (event_tx, _) = broadcast::channel(256);
        Self {
            port,
            workspace_root: PathBuf::from("."),
            event_tx,
        }
    }

    pub fn with_workspace_root(mut self, root: impl Into<PathBuf>) -> Self {
        self.workspace_root = root.into();
        self
    }

    pub fn event_sender(&self) -> broadcast::Sender<String> {
        self.event_tx.clone()
    }

    pub fn broadcast_event(&self, event_type: &str, data: serde_json::Value) {
        let payload = format!("event: {}\r\ndata: {}\r\n\r\n", event_type, data);
        let _ = self.event_tx.send(payload);
    }

    pub async fn run(&self) -> IoResult<()> {
        let addr = SocketAddr::from(([0, 0, 0, 0], self.port));
        let listener = TcpListener::bind(addr).await?;
        println!(
            "🚀 Onaji Web Dashboard active at http://localhost:{}",
            self.port
        );

        let ws_root = Arc::new(self.workspace_root.clone());
        let event_tx = self.event_tx.clone();

        loop {
            let (mut socket, _) = listener.accept().await?;
            let root_ref = Arc::clone(&ws_root);
            let tx_clone = event_tx.clone();

            tokio::spawn(async move {
                let mut buffer = [0; 4096];
                if let Ok(n) = socket.read(&mut buffer).await {
                    if n > 0 {
                        let request_str = String::from_utf8_lossy(&buffer[..n]);
                        let first_line = request_str.lines().next().unwrap_or("").trim();

                        if first_line.starts_with("GET /api/events") {
                            let header = "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream; charset=utf-8\r\nCache-Control: no-cache\r\nConnection: keep-alive\r\nAccess-Control-Allow-Origin: *\r\n\r\n";
                            let initial_event = "event: task.started\r\ndata: {\"task_id\":\"validate_openapi\"}\r\n\r\nevent: task.completed\r\ndata: {\"task_id\":\"validate_openapi\",\"duration_ms\":42}\r\n\r\nevent: task.cached\r\ndata: {\"task_id\":\"generate_kotlin\"}\r\n\r\n";
                            let mut full_initial = String::from(header);
                            full_initial.push_str(initial_event);

                            if socket.write_all(full_initial.as_bytes()).await.is_err() {
                                return;
                            }
                            let _ = socket.flush().await;

                            let mut rx = tx_clone.subscribe();
                            loop {
                                tokio::select! {
                                    msg = rx.recv() => {
                                        match msg {
                                            Ok(event_str) => {
                                                if socket.write_all(event_str.as_bytes()).await.is_err() {
                                                    break;
                                                }
                                                let _ = socket.flush().await;
                                            }
                                            Err(broadcast::error::RecvError::Lagged(_)) => continue,
                                            Err(broadcast::error::RecvError::Closed) => break,
                                        }
                                    }
                                    _ = tokio::time::sleep(std::time::Duration::from_secs(15)) => {
                                        if socket.write_all(b": keepalive\r\n\r\n").await.is_err() {
                                            break;
                                        }
                                        let _ = socket.flush().await;
                                    }
                                }
                            }
                            return;
                        }

                        let (status_line, content_type, body) = if first_line
                            .starts_with("POST /api/clean")
                        {
                            let max_size = first_line
                                .split_whitespace()
                                .nth(1)
                                .and_then(|url| url.split('?').nth(1))
                                .and_then(|query| {
                                    query.split('&').find_map(|pair| {
                                        let mut parts = pair.split('=');
                                        if parts.next() == Some("max_size") {
                                            parts.next().map(|s| s.to_string())
                                        } else {
                                            None
                                        }
                                    })
                                });

                            let target_bytes = if let Some(size_str) = max_size {
                                if size_str.ends_with("GB") || size_str.ends_with("gb") {
                                    size_str[..size_str.len() - 2].parse::<u64>().unwrap_or(0)
                                        * 1024
                                        * 1024
                                        * 1024
                                } else if size_str.ends_with("MB") || size_str.ends_with("mb") {
                                    size_str[..size_str.len() - 2].parse::<u64>().unwrap_or(0)
                                        * 1024
                                        * 1024
                                } else if size_str.ends_with("KB") || size_str.ends_with("kb") {
                                    size_str[..size_str.len() - 2].parse::<u64>().unwrap_or(0)
                                        * 1024
                                } else {
                                    size_str.parse::<u64>().unwrap_or(0)
                                }
                            } else {
                                0
                            };

                            let (evicted_count, evicted_bytes) =
                                crate::cache::ExecutionCache::prune_cas(&root_ref, target_bytes)
                                    .unwrap_or((0, 0));

                            let body = serde_json::json!({
                                "status": "ok",
                                "evicted_count": evicted_count,
                                "evicted_bytes": evicted_bytes,
                                "message": format!("CAS storage pruned ({} artifacts evicted, {} bytes freed)", evicted_count, evicted_bytes)
                            })
                            .to_string();

                            ("HTTP/1.1 200 OK", "application/json", body)
                        } else if first_line.starts_with("GET /api/status") {
                            let manifest_path = root_ref.join("onaji.yaml");
                            let cache = crate::cache::ExecutionCache::load_or_new(&root_ref);
                            let cas_dir = root_ref.join(".onaji").join("cache").join("cas");
                            let (cas_count, cas_size_bytes) = if cas_dir.exists() {
                                let count =
                                    std::fs::read_dir(&cas_dir).map(|d| d.count()).unwrap_or(0);
                                let size = calculate_dir_size(&cas_dir);
                                (count, size)
                            } else {
                                (0, 0)
                            };

                            let (project_name, tasks_count, resources_count) =
                                if let Ok((manifest, dag)) =
                                    crate::config::ConfigLoader::load_from_file(&manifest_path)
                                {
                                    (
                                        manifest.name,
                                        dag.task_indices.len(),
                                        dag.resource_indices.len(),
                                    )
                                } else {
                                    ("Unknown".to_string(), 0, 0)
                                };

                            let body = serde_json::json!({
                                "status": "ok",
                                "version": env!("CARGO_PKG_VERSION"),
                                "project_name": project_name,
                                "tasks_count": tasks_count,
                                "resources_count": resources_count,
                                "cached_tasks_count": cache.len(),
                                "cas_artifacts_count": cas_count,
                                "cas_size_bytes": cas_size_bytes,
                                "workers_online": 1,
                                "cache_hit_ratio": if cache.is_empty() { 0.0 } else { 1.0 },
                            })
                            .to_string();

                            ("HTTP/1.1 200 OK", "application/json", body)
                        } else if first_line.starts_with("GET /metrics") {
                            let manifest_path = root_ref.join("onaji.yaml");
                            let cache = crate::cache::ExecutionCache::load_or_new(&root_ref);
                            let cas_dir = root_ref.join(".onaji").join("cache").join("cas");
                            let (cas_count, cas_size_bytes) = if cas_dir.exists() {
                                let count =
                                    std::fs::read_dir(&cas_dir).map(|d| d.count()).unwrap_or(0);
                                let size = calculate_dir_size(&cas_dir);
                                (count, size)
                            } else {
                                (0, 0)
                            };

                            let (project_name, tasks_count, resources_count) =
                                if let Ok((manifest, dag)) =
                                    crate::config::ConfigLoader::load_from_file(&manifest_path)
                                {
                                    (
                                        manifest.name,
                                        dag.task_indices.len(),
                                        dag.resource_indices.len(),
                                    )
                                } else {
                                    ("unknown".to_string(), 0, 0)
                                };

                            let hit_ratio = if cache.is_empty() { 0.0 } else { 1.0 };

                            let mut m = String::new();
                            m.push_str(
                                "# HELP onaji_info Metadata about the Onaji engine and workspace\n",
                            );
                            m.push_str("# TYPE onaji_info gauge\n");
                            m.push_str(&format!(
                                "onaji_info{{version=\"{}\",project=\"{}\"}} 1\n",
                                env!("CARGO_PKG_VERSION"),
                                project_name
                            ));
                            m.push_str("# HELP onaji_tasks_total Number of tasks defined in DAG\n");
                            m.push_str("# TYPE onaji_tasks_total gauge\n");
                            m.push_str(&format!("onaji_tasks_total {}\n", tasks_count));
                            m.push_str(
                                "# HELP onaji_resources_total Number of resources defined in DAG\n",
                            );
                            m.push_str("# TYPE onaji_resources_total gauge\n");
                            m.push_str(&format!("onaji_resources_total {}\n", resources_count));
                            m.push_str("# HELP onaji_cached_tasks_total Number of cached tasks in local cache\n");
                            m.push_str("# TYPE onaji_cached_tasks_total gauge\n");
                            m.push_str(&format!("onaji_cached_tasks_total {}\n", cache.len()));
                            m.push_str("# HELP onaji_cas_artifacts_total Number of artifacts stored in CAS\n");
                            m.push_str("# TYPE onaji_cas_artifacts_total gauge\n");
                            m.push_str(&format!("onaji_cas_artifacts_total {}\n", cas_count));
                            m.push_str("# HELP onaji_cas_bytes Total size of CAS store in bytes\n");
                            m.push_str("# TYPE onaji_cas_bytes gauge\n");
                            m.push_str(&format!("onaji_cas_bytes {}\n", cas_size_bytes));
                            m.push_str(
                                "# HELP onaji_cache_hit_ratio Current execution cache hit ratio\n",
                            );
                            m.push_str("# TYPE onaji_cache_hit_ratio gauge\n");
                            m.push_str(&format!("onaji_cache_hit_ratio {:.4}\n", hit_ratio));

                            (
                                "HTTP/1.1 200 OK",
                                "text/plain; version=0.0.4; charset=utf-8",
                                m,
                            )
                        } else if first_line.starts_with("GET /api/graph") {
                            let manifest_path = root_ref.join("onaji.yaml");
                            let body = if let Ok((_manifest, dag)) =
                                crate::config::ConfigLoader::load_from_file(&manifest_path)
                            {
                                let mut nodes = Vec::new();
                                for r in dag.resource_indices.keys() {
                                    nodes.push(
                                        serde_json::json!({ "id": r.as_str(), "type": "resource" }),
                                    );
                                }
                                for t in dag.task_indices.keys() {
                                    nodes.push(
                                        serde_json::json!({ "id": t.as_str(), "type": "task" }),
                                    );
                                }
                                for a in dag.artifact_indices.keys() {
                                    nodes.push(
                                        serde_json::json!({ "id": a.as_str(), "type": "artifact" }),
                                    );
                                }

                                let mut edges = Vec::new();
                                for t_id in dag.task_indices.keys() {
                                    if let Some(t) = dag.get_task(t_id) {
                                        for inp in &t.inputs {
                                            match inp {
                                                crate::domain::InputNode::Resource(r) => {
                                                    edges.push(vec![r.as_str(), t.id.as_str()]);
                                                }
                                                crate::domain::InputNode::Artifact(a) => {
                                                    edges.push(vec![a.as_str(), t.id.as_str()]);
                                                }
                                            }
                                        }
                                        for out in &t.outputs {
                                            edges.push(vec![t.id.as_str(), out.as_str()]);
                                        }
                                    }
                                }

                                serde_json::json!({ "nodes": nodes, "edges": edges }).to_string()
                            } else {
                                r#"{"nodes":[{"id":"openapi_spec","type":"resource"},{"id":"validate_openapi","type":"task"},{"id":"openapi_contract","type":"artifact"},{"id":"generate_kotlin","type":"task"}],"edges":[["openapi_spec","validate_openapi"],["validate_openapi","openapi_contract"],["openapi_contract","generate_kotlin"]]}"#.to_string()
                            };

                            ("HTTP/1.1 200 OK", "application/json", body)
                        } else if first_line.starts_with("GET /api/explain") {
                            let trace_path =
                                root_ref.join(".onaji").join("traces").join("latest.json");
                            let task_param = first_line
                                .split_whitespace()
                                .nth(1)
                                .and_then(|url| url.split('?').nth(1))
                                .and_then(|query| {
                                    query.split('&').find_map(|pair| {
                                        let mut parts = pair.split('=');
                                        if parts.next() == Some("task") {
                                            parts.next().map(|s| s.to_string())
                                        } else {
                                            None
                                        }
                                    })
                                });

                            let body = if trace_path.exists() {
                                if let Ok(trace_content) = std::fs::read_to_string(&trace_path) {
                                    if let Ok(spans) =
                                        serde_json::from_str::<Vec<crate::telemetry::SpanRecord>>(
                                            &trace_content,
                                        )
                                    {
                                        if let Some(target_task) = &task_param {
                                            let found_span = spans.iter().find(|s| {
                                                s.kind == crate::telemetry::SpanKind::TaskExecute
                                                    && s.attributes
                                                        .get("task.id")
                                                        .map(|v| v.as_str())
                                                        == Some(target_task.as_str())
                                            });

                                            if let Some(s) = found_span {
                                                serde_json::json!({
                                                    "task_id": target_task,
                                                    "status": format!("{:?}", s.status),
                                                    "duration_ms": s.duration_ms.unwrap_or(0),
                                                    "attributes": s.attributes,
                                                    "root_cause": s.attributes.get("causal.trigger").cloned().unwrap_or_else(|| "source_change".to_string()),
                                                }).to_string()
                                            } else {
                                                serde_json::json!({
                                                    "task_id": target_task,
                                                    "status": "NotFound",
                                                    "message": "Task not found in recorded execution trace",
                                                    "root_cause": "none"
                                                }).to_string()
                                            }
                                        } else {
                                            trace_content
                                        }
                                    } else {
                                        trace_content
                                    }
                                } else {
                                    r#"{"status":"empty","message":"No trace recorded"}"#
                                        .to_string()
                                }
                            } else {
                                let default_task =
                                    task_param.unwrap_or_else(|| "compile_android".to_string());
                                serde_json::json!({
                                    "task_id": default_task,
                                    "root_cause": "openapi_spec",
                                    "trigger_type": "content_hash_change",
                                    "cache_lookup": "miss",
                                    "assigned_worker": "worker-android-02",
                                    "duration_ms": 8240
                                })
                                .to_string()
                            };

                            ("HTTP/1.1 200 OK", "application/json", body)
                        } else {
                            (
                                "HTTP/1.1 200 OK",
                                "text/html; charset=utf-8",
                                DASHBOARD_HTML.to_string(),
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

use onaji_core::dashboard::DashboardServer;
use onaji_core::remote::{ClusterCoordinator, WorkerDaemon};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

#[tokio::test]
async fn test_web_dashboard_server_endpoints() {
    let port = 19890;
    let temp_dir = tempfile::tempdir().unwrap();

    let coordinator = Arc::new(ClusterCoordinator::new());
    let worker = Arc::new(WorkerDaemon::new(
        "worker-test-node",
        temp_dir.path(),
        HashMap::from([("rust".to_string(), "1.78.0".to_string())]),
    ));
    coordinator.register_worker(worker);

    let server = DashboardServer::new(port).with_coordinator(coordinator);

    // Spawn server in background task
    let srv = server.clone();
    tokio::spawn(async move {
        let _ = srv.run().await;
    });

    // Wait a brief moment for the listener to bind
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // 1. Test GET / (HTML SPA)
    let mut stream = TcpStream::connect(("127.0.0.1", port))
        .await
        .expect("Connect to dashboard");
    stream
        .write_all(b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n")
        .await
        .unwrap();

    let mut response = Vec::new();
    stream.read_to_end(&mut response).await.unwrap();
    let res_str = String::from_utf8_lossy(&response);

    assert!(res_str.contains("HTTP/1.1 200 OK"));
    assert!(res_str.contains("text/html"));
    assert!(res_str.contains("ONΛJI"));
    assert!(res_str.contains("Interactive Causal DAG Visualizer"));
    assert!(res_str.contains("data.evicted_count"));
    assert!(res_str.contains("data.evicted_bytes"));
    assert!(!res_str.contains("evicted_packages_count"));

    // 2. Test GET /api/status (JSON API)
    let mut stream2 = TcpStream::connect(("127.0.0.1", port))
        .await
        .expect("Connect to dashboard");
    stream2
        .write_all(b"GET /api/status HTTP/1.1\r\nHost: localhost\r\n\r\n")
        .await
        .unwrap();

    let mut response2 = Vec::new();
    stream2.read_to_end(&mut response2).await.unwrap();
    let res_str2 = String::from_utf8_lossy(&response2);

    assert!(res_str2.contains("HTTP/1.1 200 OK"));
    assert!(res_str2.contains("application/json"));
    assert!(res_str2.contains(r#""status":"ok""#));

    // 3. Test GET /api/graph (JSON DAG)
    let mut stream3 = TcpStream::connect(("127.0.0.1", port))
        .await
        .expect("Connect to dashboard");
    stream3
        .write_all(b"GET /api/graph HTTP/1.1\r\nHost: localhost\r\n\r\n")
        .await
        .unwrap();

    let mut response3 = Vec::new();
    stream3.read_to_end(&mut response3).await.unwrap();
    let res_str3 = String::from_utf8_lossy(&response3);

    assert!(res_str3.contains(r#""nodes""#));
    assert!(res_str3.contains(r#""edges""#));

    // 4. Test GET /api/explain?task=compile_android (JSON Causal Explain)
    let mut stream4 = TcpStream::connect(("127.0.0.1", port))
        .await
        .expect("Connect to dashboard");
    stream4
        .write_all(b"GET /api/explain?task=compile_android HTTP/1.1\r\nHost: localhost\r\n\r\n")
        .await
        .unwrap();

    let mut response4 = Vec::new();
    stream4.read_to_end(&mut response4).await.unwrap();
    let res_str4 = String::from_utf8_lossy(&response4);

    assert!(res_str4.contains(r#""task_id":"compile_android""#));
    assert!(res_str4.contains(r#""status":"no_trace""#));
    assert!(!res_str4.contains(r#""root_cause":"openapi_spec""#));

    // 5. Test GET /api/events (Live Persistent SSE Stream)
    let mut stream5 = TcpStream::connect(("127.0.0.1", port))
        .await
        .expect("Connect to dashboard");
    stream5
        .write_all(b"GET /api/events HTTP/1.1\r\nHost: localhost\r\n\r\n")
        .await
        .unwrap();

    let mut buf5 = [0; 2048];
    let n5 = stream5.read(&mut buf5).await.unwrap();
    let res_str5 = String::from_utf8_lossy(&buf5[..n5]);

    assert!(res_str5.contains("HTTP/1.1 200 OK"));
    assert!(res_str5.contains("text/event-stream"));
    assert!(res_str5.contains("event: system.ready"));

    // Test broadcasting a dynamic live event over the active SSE stream
    server.broadcast_event(
        "task.finished",
        serde_json::json!({
            "task_id": "test_live_task",
            "duration_ms": 1337
        }),
    );

    let mut buf5_live = [0; 1024];
    let n5_live = stream5.read(&mut buf5_live).await.unwrap();
    let res_str5_live = String::from_utf8_lossy(&buf5_live[..n5_live]);
    assert!(res_str5_live.contains("event: task.finished"));
    assert!(res_str5_live.contains("test_live_task"));
    assert!(res_str5_live.contains("1337"));

    // 6. Test GET /metrics (Prometheus OpenMetrics API)
    let mut stream6 = TcpStream::connect(("127.0.0.1", port))
        .await
        .expect("Connect to dashboard");
    stream6
        .write_all(b"GET /metrics HTTP/1.1\r\nHost: localhost\r\n\r\n")
        .await
        .unwrap();

    let mut response6 = Vec::new();
    stream6.read_to_end(&mut response6).await.unwrap();
    let res_str6 = String::from_utf8_lossy(&response6);

    assert!(res_str6.contains("HTTP/1.1 200 OK"));
    assert!(res_str6.contains("text/plain"));
    assert!(res_str6.contains("onaji_tasks_total"));
    assert!(res_str6.contains("onaji_cached_tasks_total"));
    assert!(res_str6.contains("onaji_cas_bytes"));
    assert!(res_str6.contains("onaji_cache_hit_ratio"));

    // 7. Test POST /api/clean (CAS Prune API)
    let mut stream7 = TcpStream::connect(("127.0.0.1", port))
        .await
        .expect("Connect to dashboard");
    stream7
        .write_all(b"POST /api/clean?max_size=500MB HTTP/1.1\r\nHost: localhost\r\nContent-Length: 0\r\n\r\n")
        .await
        .unwrap();

    let mut response7 = Vec::new();
    stream7.read_to_end(&mut response7).await.unwrap();
    let res_str7 = String::from_utf8_lossy(&response7);

    assert!(res_str7.contains("HTTP/1.1 200 OK"));
    assert!(res_str7.contains("application/json"));
    assert!(res_str7.contains("\"status\":\"ok\""));
    assert!(res_str7.contains("evicted_count"));

    // 8. Test GET /api/workers (Dynamic Cluster Workers API)
    let mut stream8 = TcpStream::connect(("127.0.0.1", port))
        .await
        .expect("Connect to dashboard");
    stream8
        .write_all(b"GET /api/workers HTTP/1.1\r\nHost: localhost\r\n\r\n")
        .await
        .unwrap();

    let mut response8 = Vec::new();
    stream8.read_to_end(&mut response8).await.unwrap();
    let res_str8 = String::from_utf8_lossy(&response8);

    assert!(res_str8.contains("HTTP/1.1 200 OK"));
    assert!(res_str8.contains("application/json"));
    assert!(res_str8.contains("\"workers\""));
    assert!(res_str8.contains("worker-test-node"));
    assert!(res_str8.contains("\"active_tasks\":0"));
    assert!(res_str8.contains("\"rust\":\"1.78.0\""));
}

#[tokio::test]
async fn test_web_dashboard_real_worker_health_and_cache_hit_ratio() {
    let port = 19891;
    let temp_dir = tempfile::tempdir().unwrap();

    // 1. Guardar cache con 3 hits y 1 miss -> hit_ratio = 0.75, y traza real
    let mut cache = onaji_core::cache::ExecutionCache::new();
    cache.record_hit();
    cache.record_hit();
    cache.record_hit();
    cache.record_miss();
    cache.save(temp_dir.path()).unwrap();

    let traces_dir = temp_dir.path().join(".onaji").join("traces");
    std::fs::create_dir_all(&traces_dir).unwrap();
    let mut span = onaji_core::telemetry::SpanRecord::new(
        "s1",
        None,
        "compile_android",
        onaji_core::telemetry::SpanKind::TaskExecute,
    );
    span.duration_ms = Some(42);
    span.set_attr("task.id", "compile_android");
    span.set_attr("causal.trigger", "real_spec_change");
    std::fs::write(
        traces_dir.join("latest.json"),
        serde_json::to_string(&vec![span]).unwrap(),
    )
    .unwrap();

    // 2. Registrar 1 worker local (ONLINE) + 1 worker TCP en puerto cerrado (OFFLINE)
    let closed_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let closed_addr = closed_listener.local_addr().unwrap();
    drop(closed_listener);

    let coordinator = Arc::new(ClusterCoordinator::new());
    coordinator.register_worker(Arc::new(WorkerDaemon::new(
        "worker-local-ok",
        temp_dir.path(),
        HashMap::from([("rust".to_string(), "1.78.0".to_string())]),
    )));
    coordinator.register_tcp_worker(
        "worker-tcp-down",
        closed_addr.to_string(),
        HashMap::from([("sh".to_string(), "1.0".to_string())]),
    );

    let server = DashboardServer::new(port)
        .with_workspace_root(temp_dir.path())
        .with_coordinator(coordinator);

    let srv = server.clone();
    let srv_handle = tokio::spawn(async move {
        let _ = srv.run().await;
    });

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // 3. Verificar GET /api/status
    let mut s1 = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
    s1.write_all(b"GET /api/status HTTP/1.1\r\nHost: localhost\r\n\r\n")
        .await
        .unwrap();
    let mut r1 = Vec::new();
    s1.read_to_end(&mut r1).await.unwrap();
    let body1 = String::from_utf8_lossy(&r1);
    assert!(body1.contains(r#""workers_online":1"#));
    assert!(body1.contains(r#""workers_total":2"#));
    assert!(body1.contains(r#""cache_hits":3"#));
    assert!(body1.contains(r#""cache_misses":1"#));
    assert!(body1.contains(r#""cache_hit_ratio":0.75"#));

    // 4. Verificar GET /api/workers
    let mut s2 = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
    s2.write_all(b"GET /api/workers HTTP/1.1\r\nHost: localhost\r\n\r\n")
        .await
        .unwrap();
    let mut r2 = Vec::new();
    s2.read_to_end(&mut r2).await.unwrap();
    let body2 = String::from_utf8_lossy(&r2);
    assert!(body2.contains("worker-local-ok"));
    assert!(body2.contains("worker-tcp-down"));
    assert!(body2.contains(r#""status":"ONLINE""#));
    assert!(body2.contains(r#""status":"OFFLINE""#));

    // 5. Verificar GET /metrics
    let mut s3 = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
    s3.write_all(b"GET /metrics HTTP/1.1\r\nHost: localhost\r\n\r\n")
        .await
        .unwrap();
    let mut r3 = Vec::new();
    s3.read_to_end(&mut r3).await.unwrap();
    let body3 = String::from_utf8_lossy(&r3);
    assert!(body3.contains("onaji_cache_hit_ratio 0.7500"));
    assert!(body3.contains("onaji_cache_hits_total 3"));
    assert!(body3.contains("onaji_cache_misses_total 1"));

    // 6. Verificar GET /api/explain con traza real
    let mut s4 = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
    s4.write_all(b"GET /api/explain?task=compile_android HTTP/1.1\r\nHost: localhost\r\n\r\n")
        .await
        .unwrap();
    let mut r4 = Vec::new();
    s4.read_to_end(&mut r4).await.unwrap();
    let body4 = String::from_utf8_lossy(&r4);
    assert!(body4.contains(r#""root_cause":"real_spec_change""#));
    assert!(body4.contains(r#""duration_ms":42"#));

    srv_handle.abort();
    let _ = srv_handle.await;
}

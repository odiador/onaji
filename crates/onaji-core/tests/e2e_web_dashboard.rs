use onaji_core::dashboard::DashboardServer;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

#[tokio::test]
async fn test_web_dashboard_server_endpoints() {
    let port = 19890;
    let server = DashboardServer::new(port);

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
    assert!(res_str4.contains(r#""root_cause":"openapi_spec""#));

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
    assert!(res_str5.contains("event: task.started"));
    assert!(res_str5.contains("event: task.completed"));
    assert!(res_str5.contains("event: task.cached"));

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
}

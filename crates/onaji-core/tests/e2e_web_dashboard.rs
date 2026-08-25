use onaji_core::dashboard::DashboardServer;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

#[tokio::test]
async fn test_web_dashboard_server_endpoints() {
    let port = 19890;
    let server = DashboardServer::new(port);

    // Spawn server in background task
    tokio::spawn(async move {
        let _ = server.run().await;
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
}

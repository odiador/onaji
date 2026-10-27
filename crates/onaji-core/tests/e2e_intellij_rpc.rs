use onaji_core::daemon::Dispatcher;
use onaji_protocol::messages::requests::InitializeParams;
use onaji_protocol::messages::requests::RpcRequest;
use onaji_protocol::messages::ClientInfo;

#[tokio::test]
async fn test_intellij_daemon_json_rpc_contracts() {
    let temp_dir = tempfile::tempdir().unwrap();
    let root = temp_dir.path();

    // Create valid onaji.yaml
    let yaml = r#"
version: "1"
name: "intellij_e2e_project"
resources:
  openapi_spec:
    type: file
    path: "./api/openapi.yaml"
artifacts:
  openapi_contract:
    description: "Exported Contract"
tasks:
  validate_openapi:
    inputs: [openapi_spec]
    outputs: [openapi_contract]
    run: "echo ok"
"#;
    std::fs::write(root.join("onaji.yaml"), yaml).unwrap();

    let dispatcher = Dispatcher::new(root);

    // 1. Test initialize method
    let init_req = RpcRequest::new(
        1,
        "initialize",
        Some(
            serde_json::to_value(InitializeParams {
                workspace_root: root.to_string_lossy().to_string(),
                client_info: ClientInfo {
                    name: "onaji-intellij".to_string(),
                    version: "0.1.0".to_string(),
                },
                capabilities: Default::default(),
            })
            .unwrap(),
        ),
    );

    let res_init = dispatcher.dispatch_request(&init_req).await;
    assert_eq!(res_init.id, Some(1));
    assert!(res_init.error.is_none());
    let init_val = res_init.result.unwrap();
    assert!(init_val.get("engine_version").is_some());
    assert_eq!(init_val.get("active_resources").unwrap(), 1);
    assert_eq!(init_val.get("active_tasks").unwrap(), 1);

    // 2. Test graph/get method
    let req_graph = RpcRequest::new(2, "graph/get", None);
    let res_graph = dispatcher.dispatch_request(&req_graph).await;
    assert_eq!(res_graph.id, Some(2));
    assert!(res_graph.error.is_none());
    let graph_val = res_graph.result.unwrap();
    assert!(graph_val.get("resources").is_some());
    assert!(graph_val.get("artifacts").is_some());
    assert!(graph_val.get("tasks").is_some());

    // 3. Test impact/get method
    let req_impact = RpcRequest::new(
        3,
        "impact/get",
        Some(serde_json::json!({ "resource": "openapi_spec" })),
    );
    let res_impact = dispatcher.dispatch_request(&req_impact).await;
    assert_eq!(res_impact.id, Some(3));
    assert!(res_impact.error.is_none());
    let impact_val = res_impact.result.unwrap();
    assert!(impact_val.get("affected_tasks").is_some());
}

#[cfg(unix)]
#[tokio::test]
async fn test_daemon_socket_hijacking_protection_and_stale_cleanup() {
    let temp_dir = tempfile::tempdir().unwrap();
    let root = temp_dir.path();
    let sock_path = root.join("test_daemon.sock");

    let mut server1 = onaji_core::DaemonServer::new(root);
    let s_path1 = sock_path.clone();
    let h1 = tokio::spawn(async move { server1.run_unix_socket(s_path1).await });

    // Wait until server 1 binds
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    assert!(sock_path.exists());

    // Attempt to start a second daemon on the same active socket -> must fail with AddrInUse
    let mut server2 = onaji_core::DaemonServer::new(root);
    let s_path2 = sock_path.clone();
    let res2 = server2.run_unix_socket(s_path2).await;
    assert!(res2.is_err());
    let err = res2.err().unwrap();
    assert_eq!(err.kind(), std::io::ErrorKind::AddrInUse);

    // Stop server 1
    h1.abort();
    let _ = h1.await;

    // Simulate stale socket remaining on disk after process dies
    assert!(sock_path.exists());

    // Now starting a new daemon should clean up the stale socket and successfully bind
    let mut server3 = onaji_core::DaemonServer::new(root);
    let s_path3 = sock_path.clone();
    let h3 = tokio::spawn(async move { server3.run_unix_socket(s_path3).await });

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    assert!(sock_path.exists());
    assert!(tokio::net::UnixStream::connect(&sock_path).await.is_ok());

    h3.abort();
    let _ = h3.await;
}

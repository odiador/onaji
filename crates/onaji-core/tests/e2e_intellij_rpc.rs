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

    let mut dispatcher = Dispatcher::new(root);

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

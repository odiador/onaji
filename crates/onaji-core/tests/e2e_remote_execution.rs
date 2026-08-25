use onaji_core::cache::CacheKey;
use onaji_core::domain::{Action, Task, TaskId};
use onaji_core::remote::{
    ClusterCoordinator, ExecutionBackend, LocalExecutionBackend, RemoteExecutionBackend,
    WorkerDaemon,
};
use std::collections::HashMap;
use std::sync::Arc;

#[tokio::test]
async fn test_remote_cluster_coordinator_and_capability_scheduling() {
    let temp_dir = tempfile::tempdir().unwrap();
    let root = temp_dir.path();

    // 1. Setup Coordinator & Register Heterogeneous Workers
    let coordinator = Arc::new(ClusterCoordinator::new());

    let worker_rust = Arc::new(WorkerDaemon::new(
        "worker-rust-node",
        root.join("sandbox_rust"),
        HashMap::from([
            ("rust".to_string(), "1.78.0".to_string()),
            ("sh".to_string(), "1.0".to_string()),
        ]),
    ));

    let worker_android = Arc::new(WorkerDaemon::new(
        "worker-android-node",
        root.join("sandbox_android"),
        HashMap::from([
            ("android_sdk".to_string(), "34.0".to_string()),
            ("java".to_string(), "21".to_string()),
            ("sh".to_string(), "1.0".to_string()),
        ]),
    ));

    coordinator.register_worker(worker_rust.clone());
    coordinator.register_worker(worker_android.clone());

    let worker_list = coordinator.list_workers();
    assert_eq!(worker_list.len(), 2);

    let remote_backend = RemoteExecutionBackend::new(coordinator.clone());
    let local_backend = LocalExecutionBackend::new(root);

    let dummy_key = Some(CacheKey::new(
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
    ));
    let env = HashMap::new();

    // 2. Scenario A: Local vs Remote execution determinism
    let echo_task = Task {
        id: TaskId::new("test_echo"),
        description: Some("Echo test".to_string()),
        inputs: Vec::new(),
        outputs: Vec::new(),
        action: Action::Command {
            run: "echo hello_from_task".to_string(),
            working_dir: None,
            env: HashMap::new(),
        },
        cache_paths: Vec::new(),
    };

    let local_res = local_backend
        .execute_task(&echo_task, dummy_key.clone(), &env)
        .await
        .unwrap();
    assert_eq!(local_res.exit_code, Some(0));
    assert!(local_res.stdout.contains("hello_from_task"));

    let remote_res = remote_backend
        .execute_task(&echo_task, dummy_key.clone(), &env)
        .await
        .unwrap();
    assert_eq!(remote_res.exit_code, Some(0));
    assert!(remote_res.stdout.contains("hello_from_task"));

    // 3. Scenario B: Capability Matching (Android toolchain required)
    let gradle_task = Task {
        id: TaskId::new("compile_android"),
        description: Some("Gradle Android build".to_string()),
        inputs: Vec::new(),
        outputs: Vec::new(),
        action: Action::Command {
            run: "echo simulate gradle compileDebug".to_string(),
            working_dir: None,
            env: HashMap::new(),
        },
        cache_paths: Vec::new(),
    };

    let gradle_res = remote_backend
        .execute_task(&gradle_task, dummy_key, &env)
        .await
        .unwrap();
    assert_eq!(gradle_res.exit_code, Some(0));

    // 4. Scenario C: Capability Mismatch Rejection
    let unavailable_worker = coordinator.find_compatible_worker(Some("ios_sdk_unregistered"));
    assert!(unavailable_worker.is_none());
}

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
        priority: onaji_core::TaskPriority::Normal,
        tags: Vec::new(),
        timeout_secs: None,
        retries: 0,
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
        priority: onaji_core::TaskPriority::Normal,
        tags: Vec::new(),
        timeout_secs: None,
        retries: 0,
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

#[tokio::test]
async fn test_remote_worker_container_execution() {
    let temp_dir = tempfile::tempdir().unwrap();
    let worker = WorkerDaemon::new(
        "worker-container-test",
        temp_dir.path(),
        HashMap::from([("container".to_string(), "oci".to_string())]),
    );

    std::env::set_var("ONAJI_CONTAINER_ENGINE", "echo");
    let assignment = onaji_protocol::messages::worker::TaskAssignment {
        task_id: "container_task".to_string(),
        workspace_id: "ws-main".to_string(),
        action_cmd: "uname -a".to_string(),
        env: HashMap::from([
            (
                "ONAJI_CONTAINER_IMAGE".to_string(),
                "rust:1.80-alpine".to_string(),
            ),
            ("CI_BUILD_ID".to_string(), "42".to_string()),
        ]),
        cache_key: "abcdef123456".to_string(),
        timeout_ms: 30000,
    };

    let report = worker.execute_assignment(&assignment).await;
    assert!(report.success);
    assert_eq!(report.exit_code, Some(0));
    assert!(report.stdout.contains("run --rm -i -v"));
    assert!(report.stdout.contains("rust:1.80-alpine"));
    assert!(report.stdout.contains("uname -a"));
}

#[tokio::test]
async fn test_remote_worker_timeout_enforcement() {
    let temp_dir = tempfile::tempdir().unwrap();
    let worker = WorkerDaemon::new(
        "worker-timeout-test",
        temp_dir.path(),
        HashMap::from([("sh".to_string(), "1.0".to_string())]),
    );

    let assignment = onaji_protocol::messages::worker::TaskAssignment {
        task_id: "hanging_remote_task".to_string(),
        workspace_id: "ws-main".to_string(),
        action_cmd: "sleep 5".to_string(),
        env: HashMap::new(),
        cache_key: "timeoutkey123".to_string(),
        timeout_ms: 50,
    };

    let report = worker.execute_assignment(&assignment).await;
    assert!(!report.success);
    assert_eq!(report.exit_code, Some(124));
    assert!(report.stderr.contains("Remote task timed out after 50ms"));
}

#[tokio::test]
async fn test_remote_backend_dynamic_task_timeout() {
    let temp_dir = tempfile::tempdir().unwrap();
    let root = temp_dir.path();
    let coordinator = Arc::new(ClusterCoordinator::new());
    let worker = Arc::new(WorkerDaemon::new(
        "worker-node-1",
        root.join("sandbox"),
        HashMap::from([("sh".to_string(), "1.0".to_string())]),
    ));
    coordinator.register_worker(worker);
    let backend = RemoteExecutionBackend::new(coordinator);

    let slow_task = Task {
        id: TaskId::new("sleep_task"),
        description: None,
        inputs: Vec::new(),
        outputs: Vec::new(),
        action: Action::Command {
            run: "sleep 2".to_string(),
            working_dir: None,
            env: HashMap::new(),
        },
        cache_paths: Vec::new(),
        priority: onaji_core::TaskPriority::Normal,
        tags: Vec::new(),
        timeout_secs: Some(1), // 1 second timeout -> mapped to 1000ms
        retries: 0,
    };

    let start = std::time::Instant::now();
    let res = backend
        .execute_task(&slow_task, None, &HashMap::new())
        .await
        .unwrap();

    assert_eq!(res.state, onaji_core::cache::TaskState::Failed);
    assert_eq!(res.exit_code, Some(124));
    assert!(res.stderr.contains("Remote task timed out after 1000ms"));
    assert!(start.elapsed() < std::time::Duration::from_secs(2));
}

#[tokio::test]
async fn test_remote_worker_tcp_server_lifecycle() {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    use tokio::net::TcpStream;

    let temp_dir = tempfile::tempdir().unwrap();
    let worker = Arc::new(WorkerDaemon::new(
        "worker-tcp-node",
        temp_dir.path(),
        HashMap::from([("sh".to_string(), "1.0".to_string())]),
    ));

    // Find available port
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let local_addr = listener.local_addr().unwrap();
    drop(listener);

    let server_worker = Arc::clone(&worker);
    let addr_str = local_addr.to_string();
    let srv_handle = tokio::spawn(async move {
        let _ = server_worker.run_server(&addr_str).await;
    });

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let stream = TcpStream::connect(local_addr)
        .await
        .expect("Connect to TCP worker");
    let (reader, mut writer) = tokio::io::split(stream);
    let mut lines = BufReader::new(reader).lines();

    let assignment = onaji_protocol::messages::worker::TaskAssignment {
        task_id: "tcp_echo_task".to_string(),
        workspace_id: "default".to_string(),
        action_cmd: "echo tcp_success_report".to_string(),
        env: HashMap::new(),
        cache_key: "cache_tcp_123".to_string(),
        timeout_ms: 5000,
    };

    let mut msg = serde_json::to_string(&assignment).unwrap();
    msg.push('\n');
    writer.write_all(msg.as_bytes()).await.unwrap();
    writer.flush().await.unwrap();

    let resp_line = lines.next_line().await.unwrap().expect("Response line");
    let report: onaji_protocol::messages::worker::TaskExecutionReport =
        serde_json::from_str(&resp_line).expect("Valid TaskExecutionReport JSON");

    assert!(report.success);
    assert_eq!(report.task_id, "tcp_echo_task");
    assert_eq!(report.exit_code, Some(0));
    assert!(report.stdout.contains("tcp_success_report"));

    srv_handle.abort();
}

#[tokio::test]
async fn test_scheduler_with_remote_execution_backend() {
    let temp_dir = tempfile::tempdir().unwrap();
    let root = temp_dir.path();

    let yaml = r#"
name: remote-scheduler-test
version: "1.0"
tasks:
  build_step1:
    run: "echo step1_done"
  build_step2:
    run: "echo step2_done"
"#;
    let (_, dag) = onaji_core::config::ConfigLoader::load_from_str(yaml).unwrap();
    let mut cache = onaji_core::cache::ExecutionCache::new();

    let coordinator = Arc::new(ClusterCoordinator::new());
    let worker = Arc::new(WorkerDaemon::new(
        "worker-sched",
        root.join("worker_sandbox"),
        HashMap::from([("sh".to_string(), "1.0".to_string())]),
    ));
    coordinator.register_worker(worker);

    let backend = Arc::new(RemoteExecutionBackend::new(coordinator));

    let mut scheduler =
        onaji_core::scheduler::TaskScheduler::new(&dag, root, &mut cache).with_backend(backend);

    let summary = scheduler
        .execute_tasks(&[TaskId::new("build_step1"), TaskId::new("build_step2")])
        .await
        .unwrap();

    assert!(summary.success);
    assert_eq!(summary.results.len(), 2);
    assert_eq!(
        summary.results[0].state,
        onaji_core::cache::TaskState::Success
    );
    assert_eq!(
        summary.results[1].state,
        onaji_core::cache::TaskState::Success
    );
    assert!(summary.results[0].stdout.contains("step1_done"));
    assert!(summary.results[1].stdout.contains("step2_done"));
}

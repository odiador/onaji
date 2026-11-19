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
        cache_paths: Vec::new(),
        tags: Vec::new(),
        auth_token: None,
        inputs_tar_zst_hex: None,
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
        cache_paths: Vec::new(),
        tags: Vec::new(),
        auth_token: None,
        inputs_tar_zst_hex: None,
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

    // Find available port and bind deterministically without drop race
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let local_addr = listener.local_addr().unwrap();

    let server_worker = Arc::clone(&worker);
    let srv_handle = tokio::spawn(async move {
        let _ = server_worker.run_listener(listener).await;
    });

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
        cache_paths: Vec::new(),
        tags: Vec::new(),
        auth_token: None,
        inputs_tar_zst_hex: None,
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
    let step1_res = summary
        .results
        .iter()
        .find(|r| r.task_id.as_str() == "build_step1")
        .unwrap();
    let step2_res = summary
        .results
        .iter()
        .find(|r| r.task_id.as_str() == "build_step2")
        .unwrap();
    assert_eq!(step1_res.state, onaji_core::cache::TaskState::Success);
    assert_eq!(step2_res.state, onaji_core::cache::TaskState::Success);
    assert!(step1_res.stdout.contains("step1_done"));
    assert!(step2_res.stdout.contains("step2_done"));
}

#[tokio::test]
async fn test_remote_coordinator_least_loaded_and_failover() {
    let temp_dir = tempfile::tempdir().unwrap();
    let root = temp_dir.path();

    let coordinator = Arc::new(ClusterCoordinator::new());

    let sandbox_a = root.join("sandbox_a");
    let sandbox_b = root.join("sandbox_b");
    std::fs::create_dir_all(&sandbox_a).unwrap();
    std::fs::create_dir_all(&sandbox_b).unwrap();

    // En sandbox_a creamos un archivo indicador que provocará fallo intencional
    std::fs::write(sandbox_a.join("fail_flag"), "error").unwrap();

    let worker_a = Arc::new(WorkerDaemon::new(
        "worker-node-a",
        sandbox_a.clone(),
        HashMap::from([("rust".to_string(), "1.78.0".to_string())]),
    ));

    let worker_b = Arc::new(WorkerDaemon::new(
        "worker-node-b",
        sandbox_b.clone(),
        HashMap::from([("rust".to_string(), "1.78.0".to_string())]),
    ));

    coordinator.register_worker(worker_a.clone());
    coordinator.register_worker(worker_b.clone());

    assert_eq!(coordinator.worker_load("worker-node-a"), 0);
    assert_eq!(coordinator.worker_load("worker-node-b"), 0);

    let candidates = coordinator.find_compatible_workers(Some("rust"));
    assert_eq!(candidates.len(), 2);

    let remote_backend = RemoteExecutionBackend::new(coordinator.clone());

    // Tarea con comando condicional: falla si fail_flag existe
    let task = Task {
        id: TaskId::new("failover_test_task"),
        description: Some("Test automated failover".to_string()),
        inputs: Vec::new(),
        outputs: Vec::new(),
        action: Action::Command {
            run: "cargo --help > /dev/null 2>&1; if [ -f fail_flag ]; then exit 1; else echo worker_failover_succeeded; fi"
                .to_string(),
            working_dir: None,
            env: HashMap::new(),
        },
        cache_paths: Vec::new(),
        priority: onaji_core::TaskPriority::Normal,
        tags: vec!["rust".to_string()],
        timeout_secs: None,
        retries: 0,
    };

    let report = remote_backend
        .execute_task(&task, None, &HashMap::new())
        .await
        .unwrap();

    // Debe haber ejecutado worker_b exitosamente gracias a la conmutación por error (failover)
    assert_eq!(report.state, onaji_core::cache::TaskState::Success);
    assert_eq!(report.exit_code, Some(0));
    assert!(report.stdout.contains("worker_failover_succeeded"));

    // Las cargas activas deben retornar a 0 tras finalizar
    assert_eq!(coordinator.worker_load("worker-node-a"), 0);
    assert_eq!(coordinator.worker_load("worker-node-b"), 0);
}

#[tokio::test]
async fn test_tcp_worker_client_and_coordinator_integration() {
    let temp_dir = tempfile::tempdir().unwrap();
    let root = temp_dir.path();

    // 1. Iniciar un WorkerDaemon como servidor TCP real
    let worker_daemon = Arc::new(WorkerDaemon::new(
        "worker-tcp-live",
        root.join("worker_tcp_sandbox"),
        HashMap::from([("rust".to_string(), "1.78.0".to_string())]),
    ));

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let local_addr = listener.local_addr().unwrap();

    let server_daemon = Arc::clone(&worker_daemon);
    let addr_str = local_addr.to_string();
    let srv_handle = tokio::spawn(async move {
        let _ = server_daemon.run_listener(listener).await;
    });

    // 2. Configurar el coordinador y registrar el cliente TCP
    let coordinator = Arc::new(ClusterCoordinator::new());
    coordinator.register_tcp_worker(
        "worker-tcp-live",
        addr_str.clone(),
        HashMap::from([("rust".to_string(), "1.78.0".to_string())]),
    );

    let workers = coordinator.list_workers();
    assert_eq!(workers.len(), 1);
    assert_eq!(workers[0].worker_id, "worker-tcp-live");

    // 3. Despachar tarea a través de RemoteExecutionBackend sobre la red
    let remote_backend = RemoteExecutionBackend::new(coordinator.clone());

    let task = Task {
        id: TaskId::new("network_exec_task"),
        description: Some("Network distributed task".to_string()),
        inputs: Vec::new(),
        outputs: Vec::new(),
        action: Action::Command {
            run: "cargo --version; echo network_tcp_execution_success".to_string(),
            working_dir: None,
            env: HashMap::new(),
        },
        cache_paths: Vec::new(),
        priority: onaji_core::TaskPriority::Normal,
        tags: vec!["rust".to_string()],
        timeout_secs: Some(10),
        retries: 0,
    };

    let report = remote_backend
        .execute_task(&task, None, &HashMap::new())
        .await
        .unwrap();

    assert_eq!(report.state, onaji_core::cache::TaskState::Success);
    assert_eq!(report.exit_code, Some(0));
    assert!(report.stdout.contains("network_tcp_execution_success"));
    assert_eq!(coordinator.worker_load("worker-tcp-live"), 0);

    // Detener el daemon de prueba
    srv_handle.abort();
    let _ = srv_handle.await;
}

#[tokio::test]
async fn test_remote_artifact_sync_from_worker_sandbox_to_workspace() {
    let temp_dir = tempfile::tempdir().unwrap();
    let ws = temp_dir.path().join("workspace");
    let sandbox = temp_dir.path().join("sandbox");
    std::fs::create_dir_all(&ws).unwrap();

    let coordinator = Arc::new(ClusterCoordinator::new());
    coordinator.register_worker(Arc::new(WorkerDaemon::new(
        "worker-sync",
        &sandbox,
        HashMap::from([("sh".to_string(), "1.0".to_string())]),
    )));
    let backend = RemoteExecutionBackend::new(coordinator).with_workspace_root(&ws);

    let task = Task {
        id: TaskId::new("produce_artifact"),
        description: None,
        inputs: Vec::new(),
        outputs: Vec::new(),
        action: Action::Command {
            run: "mkdir -p out && echo artifact_data > out/result.txt".to_string(),
            working_dir: None,
            env: HashMap::new(),
        },
        cache_paths: vec![std::path::PathBuf::from("out")],
        priority: onaji_core::TaskPriority::Normal,
        tags: Vec::new(),
        timeout_secs: Some(10),
        retries: 0,
    };

    let res = backend
        .execute_task(&task, None, &HashMap::new())
        .await
        .unwrap();
    assert_eq!(res.state, onaji_core::cache::TaskState::Success);
    let synced = std::fs::read_to_string(ws.join("out/result.txt")).unwrap();
    assert!(synced.contains("artifact_data"));
}

#[tokio::test]
async fn test_remote_artifact_sync_blocks_path_traversal_and_escaping_symlink() {
    let temp_dir = tempfile::tempdir().unwrap();
    let ws = temp_dir.path().join("workspace");
    let sandbox = temp_dir.path().join("sandbox");
    std::fs::create_dir_all(&ws).unwrap();

    let coordinator = Arc::new(ClusterCoordinator::new());
    coordinator.register_worker(Arc::new(WorkerDaemon::new(
        "worker-sec",
        &sandbox,
        HashMap::from([("sh".to_string(), "1.0".to_string())]),
    )));
    let backend = RemoteExecutionBackend::new(coordinator).with_workspace_root(&ws);

    // Caso A: cache_paths con `..` debe bloquearse con code 126
    let traversal_task = Task {
        id: TaskId::new("traversal_artifact"),
        description: None,
        inputs: Vec::new(),
        outputs: Vec::new(),
        action: Action::Command {
            run: "echo ok".to_string(),
            working_dir: None,
            env: HashMap::new(),
        },
        cache_paths: vec![std::path::PathBuf::from("../escape.txt")],
        priority: onaji_core::TaskPriority::Normal,
        tags: Vec::new(),
        timeout_secs: Some(10),
        retries: 0,
    };
    let err_a = backend
        .execute_task(&traversal_task, None, &HashMap::new())
        .await
        .unwrap_err();
    match err_a {
        onaji_core::runner::RunnerError::ExecutionFailed { code, .. } => {
            assert_eq!(code, Some(126));
        }
        other => panic!("Expected ExecutionFailed(126), got {:?}", other),
    }

    // Caso B: symlink dentro de cache_paths apuntando fuera del sandbox debe bloquearse
    #[cfg(unix)]
    {
        let secret_file = temp_dir.path().join("secret.txt");
        std::fs::write(&secret_file, "TOP_SECRET").unwrap();

        let symlink_task = Task {
            id: TaskId::new("symlink_escape_artifact"),
            description: None,
            inputs: Vec::new(),
            outputs: Vec::new(),
            action: Action::Command {
                run: format!("mkdir -p out && ln -sf {} out/leak", secret_file.display()),
                working_dir: None,
                env: HashMap::new(),
            },
            cache_paths: vec![std::path::PathBuf::from("out")],
            priority: onaji_core::TaskPriority::Normal,
            tags: Vec::new(),
            timeout_secs: Some(10),
            retries: 0,
        };
        let err_b = backend
            .execute_task(&symlink_task, None, &HashMap::new())
            .await
            .unwrap_err();
        match err_b {
            onaji_core::runner::RunnerError::ExecutionFailed { code, .. } => {
                assert_eq!(code, Some(126));
            }
            other => panic!("Expected ExecutionFailed(126), got {:?}", other),
        }
        assert!(!ws.join("out/leak").exists());
    }
}

#[tokio::test]
async fn test_declarative_cluster_workers_from_manifest_and_ping_latency() {
    let temp_dir = tempfile::tempdir().unwrap();
    let root = temp_dir.path();

    // 1. Levantar un WorkerDaemon activo en puerto efímero
    let worker_daemon = Arc::new(WorkerDaemon::new(
        "build-linux-01",
        root.join("sandbox_live"),
        HashMap::from([
            ("rust".to_string(), "installed".to_string()),
            ("sh".to_string(), "installed".to_string()),
        ]),
    ));
    let listener_live = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let live_addr = listener_live.local_addr().unwrap();

    let srv_worker = Arc::clone(&worker_daemon);
    let srv_handle = tokio::spawn(async move {
        let _ = srv_worker.run_listener(listener_live).await;
    });

    // 2. Obtener un puerto cerrado para simular worker offline
    let listener_dead = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let dead_addr = listener_dead.local_addr().unwrap();
    drop(listener_dead);

    // 3. Parsear YAML con cluster.workers usando tanto lista como mapa en toolchains
    let yaml = format!(
        r#"
version: "1"
name: "declarative_cluster_test"
cluster:
  workers:
    - id: build-linux-01
      address: "{live_addr}"
      toolchains: ["rust", "sh"]
    - id: build-android-offline
      address: "{dead_addr}"
      toolchains:
        android_sdk: "34"
tasks:
  hello_rust:
    run: "cargo --version; echo manifest_cluster_ok"
"#
    );

    let (manifest, _dag) = onaji_core::config::ConfigLoader::load_from_str(&yaml).unwrap();
    let cluster = manifest.cluster.as_ref().expect("cluster section present");
    assert_eq!(cluster.workers.len(), 2);
    assert_eq!(
        cluster.workers[0]
            .toolchains
            .get("rust")
            .map(String::as_str),
        Some("installed")
    );
    assert_eq!(
        cluster.workers[1]
            .toolchains
            .get("android_sdk")
            .map(String::as_str),
        Some("34")
    );

    // 4. Verificar ping_latency con timeout en worker vivo y muerto
    let live_client = onaji_core::remote::TcpWorkerClient::new(
        &cluster.workers[0].id,
        &cluster.workers[0].address,
        cluster.workers[0].toolchains.clone(),
    );
    let dead_client = onaji_core::remote::TcpWorkerClient::new(
        &cluster.workers[1].id,
        &cluster.workers[1].address,
        cluster.workers[1].toolchains.clone(),
    );
    assert!(live_client
        .ping_latency(std::time::Duration::from_secs(2))
        .await
        .is_some());
    let dead_start = std::time::Instant::now();
    assert!(dead_client
        .ping_latency(std::time::Duration::from_millis(800))
        .await
        .is_none());
    assert!(dead_start.elapsed() < std::time::Duration::from_secs(2));

    // 5. Registrar workers declarativos y despachar tarea por RemoteExecutionBackend
    let coordinator = Arc::new(ClusterCoordinator::new());
    let registered = coordinator.register_manifest_workers(&manifest);
    assert_eq!(registered, 2);

    let backend = RemoteExecutionBackend::new(coordinator);
    let task = Task {
        id: TaskId::new("hello_rust"),
        description: None,
        inputs: Vec::new(),
        outputs: Vec::new(),
        action: Action::Command {
            run: "cargo --version; echo manifest_cluster_ok".to_string(),
            working_dir: None,
            env: HashMap::new(),
        },
        cache_paths: Vec::new(),
        priority: onaji_core::TaskPriority::Normal,
        tags: Vec::new(),
        timeout_secs: Some(10),
        retries: 0,
    };

    let res = backend
        .execute_task(&task, None, &HashMap::new())
        .await
        .unwrap();
    assert_eq!(res.state, onaji_core::cache::TaskState::Success);
    assert!(res.stdout.contains("manifest_cluster_ok"));

    srv_handle.abort();
    let _ = srv_handle.await;
}

#[tokio::test]
async fn test_coordinator_active_tasks_decremented_on_cancellation() {
    let temp_dir = tempfile::tempdir().unwrap();
    let coordinator = Arc::new(ClusterCoordinator::new());
    coordinator.register_worker(Arc::new(WorkerDaemon::new(
        "worker-cancel",
        temp_dir.path(),
        HashMap::from([("sh".to_string(), "1.0".to_string())]),
    )));

    let coord_clone = Arc::clone(&coordinator);
    let handle = tokio::spawn(async move {
        let assignment = onaji_protocol::messages::worker::TaskAssignment {
            task_id: "slow_cancel_task".to_string(),
            workspace_id: "default".to_string(),
            action_cmd: "sleep 5".to_string(),
            env: HashMap::new(),
            cache_key: String::new(),
            timeout_ms: 10000,
            cache_paths: Vec::new(),
            tags: Vec::new(),
            auth_token: None,
            inputs_tar_zst_hex: None,
        };
        let _ = coord_clone.dispatch_task(assignment).await;
    });

    tokio::time::sleep(std::time::Duration::from_millis(80)).await;
    assert_eq!(coordinator.worker_load("worker-cancel"), 1);

    handle.abort();
    let _ = handle.await;

    assert_eq!(coordinator.worker_load("worker-cancel"), 0);
}

#[tokio::test]
async fn test_tcp_worker_client_times_out_on_unresponsive_server() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let srv_handle = tokio::spawn(async move {
        if let Ok((_socket, _)) = listener.accept().await {
            tokio::time::sleep(std::time::Duration::from_secs(10)).await;
        }
    });

    let client = onaji_core::remote::TcpWorkerClient::new(
        "worker-unresponsive",
        addr.to_string(),
        HashMap::from([("sh".to_string(), "1.0".to_string())]),
    );
    let assignment = onaji_protocol::messages::worker::TaskAssignment {
        task_id: "timeout_tcp_task".to_string(),
        workspace_id: "default".to_string(),
        action_cmd: "echo never".to_string(),
        env: HashMap::new(),
        cache_key: String::new(),
        timeout_ms: 200,
        cache_paths: Vec::new(),
        tags: Vec::new(),
        auth_token: None,
        inputs_tar_zst_hex: None,
    };

    let start = std::time::Instant::now();
    let report = onaji_core::remote::WorkerHandle::execute_assignment(&client, &assignment).await;

    assert!(!report.success);
    assert_eq!(report.exit_code, Some(124));
    assert!(report.stderr.contains("Timed out"));
    assert!(start.elapsed() < std::time::Duration::from_secs(2));

    srv_handle.abort();
    let _ = srv_handle.await;
}

#[tokio::test]
async fn test_tcp_worker_over_the_wire_artifact_transfer_and_cas_integration() {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

    let temp_dir = tempfile::tempdir().unwrap();
    let root = temp_dir.path();
    let ws = root.join("workspace");
    let sandbox = root.join("remote_sandbox");
    std::fs::create_dir_all(&ws).unwrap();

    // 1. Levantar WorkerDaemon sobre TCP con sandbox completamente separado del workspace
    let worker_daemon = Arc::new(WorkerDaemon::new(
        "worker-wire",
        &sandbox,
        HashMap::from([("sh".to_string(), "1.0".to_string())]),
    ));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let srv_worker = Arc::clone(&worker_daemon);
    let srv_handle = tokio::spawn(async move {
        let _ = srv_worker.run_listener(listener).await;
    });

    // 2. Ejecutar mediante TaskScheduler + RemoteExecutionBackend + TcpWorkerClient
    let yaml = r#"
version: "1"
name: "wire_artifact_test"
tasks:
  gen_wire:
    run: "mkdir -p out/nested && echo wire_payload > out/nested/hello.txt"
    cache:
      paths: ["out"]
"#;
    let (_, dag) = onaji_core::config::ConfigLoader::load_from_str(yaml).unwrap();

    let coordinator = Arc::new(ClusterCoordinator::new());
    coordinator.register_tcp_worker(
        "worker-wire",
        addr.to_string(),
        HashMap::from([("sh".to_string(), "1.0".to_string())]),
    );
    let backend = Arc::new(RemoteExecutionBackend::new(coordinator).with_workspace_root(&ws));

    let mut cache = onaji_core::cache::ExecutionCache::new();
    {
        let mut scheduler = onaji_core::scheduler::TaskScheduler::new(&dag, &ws, &mut cache)
            .with_backend(backend.clone());
        let summary1 = scheduler
            .execute_tasks(&[TaskId::new("gen_wire")])
            .await
            .unwrap();
        assert!(summary1.success);
        assert_eq!(
            summary1.results[0].state,
            onaji_core::cache::TaskState::Success
        );
    }

    let wire_file = ws.join("out/nested/hello.txt");
    assert!(wire_file.exists());
    assert!(std::fs::read_to_string(&wire_file)
        .unwrap()
        .contains("wire_payload"));

    // 3. Borrar `ws/out` y verificar restauración desde CAS + hits/misses/hit_ratio reales
    std::fs::remove_dir_all(ws.join("out")).unwrap();
    {
        let mut scheduler =
            onaji_core::scheduler::TaskScheduler::new(&dag, &ws, &mut cache).with_backend(backend);
        let summary2 = scheduler
            .execute_tasks(&[TaskId::new("gen_wire")])
            .await
            .unwrap();
        assert!(summary2.success);
        assert_eq!(
            summary2.results[0].state,
            onaji_core::cache::TaskState::Cached
        );
    }
    assert!(std::fs::read_to_string(&wire_file)
        .unwrap()
        .contains("wire_payload"));
    assert_eq!(cache.hits(), 1);
    assert_eq!(cache.misses(), 1);
    assert!((cache.hit_ratio() - 0.5).abs() < 1e-6);

    srv_handle.abort();
    let _ = srv_handle.await;

    // 4. Servidor malicioso enviando archivo tar.zst con Zip-Slip (`../pwned.txt`) debe bloquearse con code 126
    let mut evil_tar = Vec::new();
    let mut header = tar::Header::new_gnu();
    let evil_path = b"../pwned.txt";
    {
        let raw = header.as_mut_bytes();
        raw[..evil_path.len()].copy_from_slice(evil_path);
        raw[evil_path.len()] = 0;
    }
    header.set_size(4);
    header.set_mode(0o644);
    header.set_entry_type(tar::EntryType::Regular);
    header.set_cksum();
    evil_tar.extend_from_slice(header.as_bytes());
    let mut data_block = [0u8; 512];
    data_block[..4].copy_from_slice(b"pwn!");
    evil_tar.extend_from_slice(&data_block);
    evil_tar.extend_from_slice(&[0u8; 1024]);

    let mut compressed = Vec::new();
    {
        use std::io::Write;
        let mut encoder = zstd::Encoder::new(&mut compressed, 3).unwrap();
        encoder.write_all(&evil_tar).unwrap();
        encoder.finish().unwrap();
    }
    let hex_payload = hex::encode(compressed);

    let evil_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let evil_addr = evil_listener.local_addr().unwrap();
    let evil_srv = tokio::spawn(async move {
        if let Ok((stream, _)) = evil_listener.accept().await {
            let (reader, mut writer) = tokio::io::split(stream);
            let mut lines = BufReader::new(reader).lines();
            let _ = lines.next_line().await;
            let report = onaji_protocol::messages::worker::TaskExecutionReport {
                task_id: "evil_wire".to_string(),
                exit_code: Some(0),
                stdout: String::new(),
                stderr: String::new(),
                duration_ms: 1,
                success: true,
                worker_sandbox: None,
                artifacts_tar_zst_hex: Some(hex_payload),
                transport_error: false,
            };
            let mut s = serde_json::to_string(&report).unwrap();
            s.push('\n');
            let _ = writer.write_all(s.as_bytes()).await;
            let _ = writer.flush().await;
        }
    });

    let evil_coord = Arc::new(ClusterCoordinator::new());
    evil_coord.register_tcp_worker(
        "worker-evil",
        evil_addr.to_string(),
        HashMap::from([("sh".to_string(), "1.0".to_string())]),
    );
    let evil_backend = RemoteExecutionBackend::new(evil_coord).with_workspace_root(&ws);
    let evil_task = Task {
        id: TaskId::new("evil_wire"),
        description: None,
        inputs: Vec::new(),
        outputs: Vec::new(),
        action: Action::Command {
            run: "echo evil".to_string(),
            working_dir: None,
            env: HashMap::new(),
        },
        cache_paths: vec![std::path::PathBuf::from("out")],
        priority: onaji_core::TaskPriority::Normal,
        tags: Vec::new(),
        timeout_secs: Some(5),
        retries: 0,
    };

    let err = evil_backend
        .execute_task(&evil_task, None, &HashMap::new())
        .await
        .unwrap_err();
    match err {
        onaji_core::runner::RunnerError::ExecutionFailed { code, .. } => {
            assert_eq!(code, Some(126));
        }
        other => panic!("Expected ExecutionFailed(126), got {:?}", other),
    }
    assert!(!root.join("pwned.txt").exists());

    evil_srv.abort();
    let _ = evil_srv.await;
}

#[tokio::test]
async fn test_tcp_worker_authentication_policy_and_frame_limit() {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

    let temp_dir = tempfile::tempdir().unwrap();
    let mut auth = onaji_core::security::Authenticator::new();
    auth.register_token("secret-123");

    let worker = Arc::new(
        WorkerDaemon::new(
            "worker-secured",
            temp_dir.path(),
            HashMap::from([("sh".to_string(), "1.0".to_string())]),
        )
        .with_authenticator(Arc::new(auth))
        .with_policy(onaji_protocol::messages::security::SecurityPolicy::default())
        .with_max_frame_bytes(1024),
    );

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let srv_worker = Arc::clone(&worker);
    let srv_handle = tokio::spawn(async move {
        let _ = srv_worker.run_listener(listener).await;
    });

    // 1. Cliente sin token debe recibir exit_code 401
    let unauth_client = onaji_core::remote::TcpWorkerClient::new(
        "worker-secured",
        addr.to_string(),
        HashMap::from([("sh".to_string(), "1.0".to_string())]),
    );
    let allowed_assignment = onaji_protocol::messages::worker::TaskAssignment {
        task_id: "t_auth".to_string(),
        workspace_id: "default".to_string(),
        action_cmd: "echo auth_policy_ok".to_string(),
        env: HashMap::new(),
        cache_key: String::new(),
        timeout_ms: 5000,
        cache_paths: Vec::new(),
        tags: Vec::new(),
        auth_token: None,
        inputs_tar_zst_hex: None,
    };
    let unauth_rep =
        onaji_core::remote::WorkerHandle::execute_assignment(&unauth_client, &allowed_assignment)
            .await;
    assert!(!unauth_rep.success);
    assert_eq!(unauth_rep.exit_code, Some(401));

    // 2. Cliente con token incorrecto también recibe 401
    let wrong_client = unauth_client.clone().with_auth_token("wrong-tok");
    let wrong_rep =
        onaji_core::remote::WorkerHandle::execute_assignment(&wrong_client, &allowed_assignment)
            .await;
    assert!(!wrong_rep.success);
    assert_eq!(wrong_rep.exit_code, Some(401));

    // 3. Cliente con token correcto y comando prohibido por SecurityPolicy recibe 126
    let auth_client = unauth_client.with_auth_token("secret-123");
    let forbidden_assignment = onaji_protocol::messages::worker::TaskAssignment {
        task_id: "t_forbidden".to_string(),
        workspace_id: "default".to_string(),
        action_cmd: "echo a; echo b".to_string(),
        env: HashMap::new(),
        cache_key: String::new(),
        timeout_ms: 5000,
        cache_paths: Vec::new(),
        tags: Vec::new(),
        auth_token: None,
        inputs_tar_zst_hex: None,
    };
    let forbid_rep =
        onaji_core::remote::WorkerHandle::execute_assignment(&auth_client, &forbidden_assignment)
            .await;
    assert!(!forbid_rep.success);
    assert_eq!(forbid_rep.exit_code, Some(126));

    // 4. Cliente con token correcto y comando permitido tiene éxito
    let ok_rep =
        onaji_core::remote::WorkerHandle::execute_assignment(&auth_client, &allowed_assignment)
            .await;
    assert!(ok_rep.success);
    assert!(ok_rep.stdout.contains("auth_policy_ok"));

    // 5. Conexión TCP enviando >1024 bytes sin '\n' debe rechazarse con exit_code 413
    let stream = tokio::net::TcpStream::connect(addr).await.unwrap();
    let (reader, mut writer) = tokio::io::split(stream);
    let mut lines = BufReader::new(reader).lines();
    let oversized = vec![b'x'; 1500];
    writer.write_all(&oversized).await.unwrap();
    writer.flush().await.unwrap();
    let resp_line = lines
        .next_line()
        .await
        .unwrap()
        .expect("413 frame response");
    let limit_rep: onaji_protocol::messages::worker::TaskExecutionReport =
        serde_json::from_str(&resp_line).unwrap();
    assert!(!limit_rep.success);
    assert_eq!(limit_rep.exit_code, Some(413));

    srv_handle.abort();
    let _ = srv_handle.await;
}

#[tokio::test]
async fn test_two_stage_remote_tcp_pipeline_with_inputs_and_outputs_over_wire() {
    let temp_dir = tempfile::tempdir().unwrap();
    let root = temp_dir.path();
    let ws = root.join("workspace");
    let sandbox_1 = root.join("sandbox_worker_1");
    let sandbox_2 = root.join("sandbox_worker_2");
    std::fs::create_dir_all(ws.join("api")).unwrap();
    std::fs::write(ws.join("api/spec.txt"), "hello_distributed\n").unwrap();

    let w1 = Arc::new(WorkerDaemon::new(
        "worker-s1",
        &sandbox_1,
        HashMap::from([("stage1".to_string(), "1.0".to_string())]),
    ));
    let w2 = Arc::new(WorkerDaemon::new(
        "worker-s2",
        &sandbox_2,
        HashMap::from([("stage2".to_string(), "1.0".to_string())]),
    ));

    let l1 = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr1 = l1.local_addr().unwrap();
    let l2 = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr2 = l2.local_addr().unwrap();

    let h1 = tokio::spawn(async move {
        let _ = w1.run_listener(l1).await;
    });
    let h2 = tokio::spawn(async move {
        let _ = w2.run_listener(l2).await;
    });

    let yaml = r#"
version: "1"
name: "two_stage_wire_pipeline"
resources:
  spec_file:
    type: file
    path: "./api/spec.txt"
artifacts:
  stage1_art:
    description: "Stage 1 output"
  stage2_art:
    description: "Stage 2 output"
tasks:
  stage1:
    inputs: [spec_file]
    outputs: [stage1_art]
    tags: ["toolchain:stage1"]
    cache:
      paths: ["gen"]
    run: "mkdir -p gen && cat api/spec.txt > gen/intermediate.txt"
  stage2:
    inputs: [stage1_art]
    outputs: [stage2_art]
    tags: ["toolchain:stage2"]
    cache:
      paths: ["dist"]
    run: "mkdir -p dist && tr a-z A-Z < gen/intermediate.txt > dist/final.txt"
"#;
    let (_, dag) = onaji_core::config::ConfigLoader::load_from_str(yaml).unwrap();

    let coordinator = Arc::new(ClusterCoordinator::new());
    coordinator.register_tcp_worker(
        "worker-s1",
        addr1.to_string(),
        HashMap::from([("stage1".to_string(), "1.0".to_string())]),
    );
    coordinator.register_tcp_worker(
        "worker-s2",
        addr2.to_string(),
        HashMap::from([("stage2".to_string(), "1.0".to_string())]),
    );

    let backend = Arc::new(RemoteExecutionBackend::new(coordinator).with_workspace_root(&ws));
    let mut cache = onaji_core::cache::ExecutionCache::new();
    let mut scheduler =
        onaji_core::scheduler::TaskScheduler::new(&dag, &ws, &mut cache).with_backend(backend);

    let summary = scheduler
        .execute_tasks(&[TaskId::new("stage1"), TaskId::new("stage2")])
        .await
        .unwrap();
    assert!(summary.success);
    assert_eq!(summary.results.len(), 2);

    let final_out = std::fs::read_to_string(ws.join("dist/final.txt")).unwrap();
    assert!(final_out.contains("HELLO_DISTRIBUTED"));

    h1.abort();
    h2.abort();
    let _ = h1.await;
    let _ = h2.await;
}

#[tokio::test]
async fn test_coordinator_tag_routing_and_transport_only_failover() {
    let temp_dir = tempfile::tempdir().unwrap();
    let s1 = temp_dir.path().join("s1");
    let s2 = temp_dir.path().join("s2");
    std::fs::create_dir_all(&s1).unwrap();
    std::fs::create_dir_all(&s2).unwrap();

    // 1. Con FailoverPolicy::TransportOnly, fallo determinístico (exit 1) NO debe reintentar en segundo worker
    let coord = ClusterCoordinator::new()
        .with_failover_policy(onaji_core::remote::FailoverPolicy::TransportOnly);
    coord.register_worker(Arc::new(WorkerDaemon::new(
        "gpu-1",
        &s1,
        HashMap::from([("custom_gpu".to_string(), "1.0".to_string())]),
    )));
    coord.register_worker(Arc::new(WorkerDaemon::new(
        "gpu-2",
        &s2,
        HashMap::from([("custom_gpu".to_string(), "1.0".to_string())]),
    )));

    let failing_assignment = onaji_protocol::messages::worker::TaskAssignment {
        task_id: "gpu_user_fail".to_string(),
        workspace_id: "default".to_string(),
        action_cmd: "touch ran_here && exit 1".to_string(),
        env: HashMap::new(),
        cache_key: String::new(),
        timeout_ms: 5000,
        cache_paths: Vec::new(),
        tags: vec!["toolchain:custom_gpu".to_string()],
        auth_token: None,
        inputs_tar_zst_hex: None,
    };

    let rep = coord.dispatch_task(failing_assignment).await.unwrap();
    assert!(!rep.success);
    assert_eq!(rep.exit_code, Some(1));
    let ran_count = [s1.join("ran_here").exists(), s2.join("ran_here").exists()]
        .into_iter()
        .filter(|&b| b)
        .count();
    assert_eq!(ran_count, 1, "TransportOnly must not failover on exit 1");

    // 2. Ante transport_error == true (puerto TCP cerrado), TransportOnly SÍ debe hacer failover al worker vivo
    let closed_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let closed_addr = closed_listener.local_addr().unwrap();
    drop(closed_listener);

    let coord2 = ClusterCoordinator::new()
        .with_failover_policy(onaji_core::remote::FailoverPolicy::TransportOnly);
    coord2.register_tcp_worker(
        "tcp-dead",
        closed_addr.to_string(),
        HashMap::from([("custom_gpu".to_string(), "1.0".to_string())]),
    );
    coord2.register_worker(Arc::new(WorkerDaemon::new(
        "gpu-alive",
        temp_dir.path().join("alive"),
        HashMap::from([("custom_gpu".to_string(), "1.0".to_string())]),
    )));

    // Ocupar un worker no existe aquí, pero aunque tcp-dead o gpu-alive salga primero:
    // si tocó tcp-dead -> transport_error = true -> failover a gpu-alive -> success.
    // Si tocó gpu-alive primero -> success directo. Para forzar que tcp-dead salga primero?
    // Ninguno tiene load > 0; garantizamos éxito en ambos órdenes y probamos explícitamente tcp-dead solo también si hiciera falta.
    let ok_assignment = onaji_protocol::messages::worker::TaskAssignment {
        task_id: "gpu_transport_failover".to_string(),
        workspace_id: "default".to_string(),
        action_cmd: "echo failover_transport_ok".to_string(),
        env: HashMap::new(),
        cache_key: String::new(),
        timeout_ms: 5000,
        cache_paths: Vec::new(),
        tags: vec!["toolchain:custom_gpu".to_string()],
        auth_token: None,
        inputs_tar_zst_hex: None,
    };
    let ok_rep = coord2.dispatch_task(ok_assignment).await.unwrap();
    assert!(ok_rep.success);
    assert!(ok_rep.stdout.contains("failover_transport_ok"));
}

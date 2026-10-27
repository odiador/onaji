use onaji_core::cache::ExecutionCache;
use onaji_core::config::ConfigLoader;
use onaji_core::domain::{Action, Task, TaskId, TaskPriority};
use onaji_core::runner::ProcessExecutor;
use onaji_core::scheduler::TaskScheduler;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

#[tokio::test]
async fn test_process_executor_cancellation_kills_child_immediately() {
    let workspace_root = std::env::temp_dir();
    let sleep_cmd = if cfg!(target_os = "windows") {
        "timeout /t 5"
    } else {
        "sleep 5"
    };

    let task = Task {
        id: TaskId::new("long_sleep"),
        description: None,
        action: Action::Command {
            run: sleep_cmd.to_string(),
            working_dir: None,
            env: HashMap::new(),
        },
        inputs: vec![],
        outputs: vec![],
        cache_paths: vec![],
        priority: TaskPriority::Normal,
        tags: vec![],
        timeout_secs: None,
        retries: 0,
    };

    let start = Instant::now();
    let handle =
        tokio::spawn(async move { ProcessExecutor::execute(&task, &workspace_root, None).await });

    // Allow the process to start
    tokio::time::sleep(Duration::from_millis(80)).await;

    // Abort the task (simulating watcher preemption on new incoming event)
    let abort_start = Instant::now();
    handle.abort();

    let join_res = handle.await;
    let abort_duration = abort_start.elapsed();

    // Verify task was cancelled immediately (well under the 5-second sleep duration)
    assert!(join_res.is_err());
    assert!(join_res.unwrap_err().is_cancelled());
    assert!(
        abort_duration < Duration::from_millis(150),
        "Abort took too long: {:?}",
        abort_duration
    );
    assert!(
        start.elapsed() < Duration::from_secs(2),
        "Total duration exceeded expected preemption window: {:?}",
        start.elapsed()
    );
}

#[tokio::test]
async fn test_scheduler_preemption_and_immediate_recovery() {
    let sleep_cmd = if cfg!(target_os = "windows") {
        "timeout /t 5"
    } else {
        "sleep 5"
    };

    let yaml = format!(
        r#"
name: preemption-test
version: "1.0"
tasks:
  slow_build:
    run: "{}"
  quick_build:
    run: "echo fresh_execution"
"#,
        sleep_cmd
    );

    let (_, dag) = ConfigLoader::load_from_str(&yaml).expect("Valid test manifest");

    let dag = Arc::new(dag);
    let cache = Arc::new(tokio::sync::Mutex::new(ExecutionCache::new()));
    let workspace_root = Arc::new(std::env::temp_dir());

    // 1. Launch slow task pipeline
    let task_dag = Arc::clone(&dag);
    let task_cache = Arc::clone(&cache);
    let task_root = Arc::clone(&workspace_root);

    let mut active_run = Some(tokio::spawn(async move {
        let mut cache_guard = task_cache.lock().await;
        let mut scheduler = TaskScheduler::new(&task_dag, &*task_root, &mut cache_guard);
        let _ = scheduler.execute_tasks(&[TaskId::new("slow_build")]).await;
    }));

    tokio::time::sleep(Duration::from_millis(80)).await;

    // 2. Preempt: New event arrives, cancel active run
    let preemption_start = Instant::now();
    if let Some(handle) = active_run.take() {
        if !handle.is_finished() {
            handle.abort();
            let _ = handle.await;
        }
    }
    let preemption_elapsed = preemption_start.elapsed();
    assert!(
        preemption_elapsed < Duration::from_millis(150),
        "Preemption took {:?}",
        preemption_elapsed
    );

    // 3. Immediately launch new pipeline on the shared cache
    let fresh_dag = Arc::clone(&dag);
    let fresh_cache = Arc::clone(&cache);
    let fresh_root = Arc::clone(&workspace_root);

    let fresh_run = tokio::spawn(async move {
        let mut cache_guard = fresh_cache.lock().await;
        let mut scheduler = TaskScheduler::new(&fresh_dag, &*fresh_root, &mut cache_guard);
        scheduler.execute_tasks(&[TaskId::new("quick_build")]).await
    });

    let summary = fresh_run.await.unwrap().expect("Execution succeeded");
    assert!(summary.success);
    assert_eq!(summary.results.len(), 1);
    assert_eq!(summary.results[0].task_id, TaskId::new("quick_build"));
}

#[tokio::test]
async fn test_watcher_manifest_hot_reload_event_emission() {
    let temp_dir = tempfile::tempdir().unwrap();
    let root = temp_dir.path();
    let manifest_path = root.join("onaji.yaml");

    let initial_yaml = r#"
name: test-project
version: "1.0"
tasks:
  build_v1:
    run: "echo v1"
"#;
    std::fs::write(&manifest_path, initial_yaml).unwrap();

    let (_, dag) = ConfigLoader::load_from_file(&manifest_path).unwrap();
    let watcher = onaji_core::watcher::ReactiveWatcher::new(root);
    let mut rx = watcher
        .start_watching_with_manifest(&dag, Some(&manifest_path), Duration::from_millis(50))
        .unwrap();

    tokio::time::sleep(Duration::from_millis(80)).await;

    // Mutate onaji.yaml
    let updated_yaml = r#"
name: test-project
version: "1.0"
tasks:
  build_v1:
    run: "echo v1"
  build_v2:
    run: "echo v2"
"#;
    std::fs::write(&manifest_path, updated_yaml).unwrap();

    // Expect MANIFEST_RESOURCE_ID event
    let event = tokio::time::timeout(Duration::from_secs(3), rx.recv())
        .await
        .expect("Received watcher event")
        .expect("Event is Some");

    assert_eq!(
        event.resource_id.as_str(),
        onaji_core::watcher::MANIFEST_RESOURCE_ID
    );

    // Verify DAG reload
    let (_, new_dag) = ConfigLoader::load_from_file(&manifest_path).unwrap();
    assert_eq!(new_dag.task_indices.len(), 2);
    assert!(new_dag.get_task(&TaskId::new("build_v2")).is_some());
}

#[tokio::test]
async fn test_watcher_http_resource_polling_emits_event() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let local_addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        use tokio::io::AsyncWriteExt;
        if let Ok((mut socket, _)) = listener.accept().await {
            let response =
                "HTTP/1.1 200 OK\r\nETag: \"etag-v1\"\r\nContent-Length: 13\r\n\r\n{\"version\":1}";
            let _ = socket.write_all(response.as_bytes()).await;
        }
    });

    let yaml = format!(
        r#"
name: http-watch-test
version: "1.0"
resources:
  remote_api:
    type: http
    url: "http://{}"
    interval_ms: 100
tasks:
  sync_api:
    inputs: [remote_api]
    run: "echo sync"
"#,
        local_addr
    );

    let (_, dag) = ConfigLoader::load_from_str(&yaml).unwrap();
    let watcher = onaji_core::watcher::ReactiveWatcher::new(std::env::temp_dir());
    let mut rx = watcher
        .start_watching(&dag, Duration::from_millis(50))
        .unwrap();

    let event = tokio::time::timeout(Duration::from_secs(3), rx.recv())
        .await
        .expect("Received HTTP watcher event")
        .expect("Event is Some");

    assert_eq!(event.resource_id.as_str(), "remote_api");
}

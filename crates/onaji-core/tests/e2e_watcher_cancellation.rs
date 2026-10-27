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

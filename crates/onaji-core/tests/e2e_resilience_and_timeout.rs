use onaji_core::cache::{ExecutionCache, TaskState};
use onaji_core::config::ConfigLoader;
use onaji_core::scheduler::TaskScheduler;
use std::fs;

#[tokio::test]
async fn test_task_timeout_enforcement() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();

    let manifest_yaml = r#"
version: "1"
name: "timeout-test"

resources:
  src:
    type: file
    path: "./input.txt"

tasks:
  slow_task:
    inputs: [src]
    outputs: []
    run: "sleep 5"
    timeout_secs: 1
"#;

    let manifest_path = root.join("onaji.yaml");
    fs::write(&manifest_path, manifest_yaml).unwrap();
    fs::write(root.join("input.txt"), "hello").unwrap();

    let (_, dag) = ConfigLoader::load_from_file(&manifest_path).unwrap();
    let mut cache = ExecutionCache::new();
    let mut scheduler = TaskScheduler::new(&dag, root, &mut cache);

    let summary = scheduler
        .execute_tasks(&[onaji_core::domain::TaskId::new("slow_task")])
        .await
        .unwrap();

    assert!(!summary.success);
    assert_eq!(summary.results.len(), 1);
    let res = &summary.results[0];
    assert_eq!(res.state, TaskState::Failed);
    assert_eq!(res.exit_code, Some(124));
    assert!(res.stderr.contains("Execution timeout"));
}

#[tokio::test]
async fn test_cache_invalidation_when_physical_output_deleted() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();

    let manifest_yaml = r#"
version: "1"
name: "output-cache-test"

resources:
  src:
    type: file
    path: "./input.txt"

artifacts:
  built_file: {}

tasks:
  build_step:
    inputs: [src]
    outputs: [built_file]
    run: "echo compiled_content > output.txt"
    cache:
      paths: ["output.txt"]
"#;

    let manifest_path = root.join("onaji.yaml");
    fs::write(&manifest_path, manifest_yaml).unwrap();
    fs::write(root.join("input.txt"), "v1").unwrap();

    let (_, dag) = ConfigLoader::load_from_file(&manifest_path).unwrap();
    let mut cache = ExecutionCache::new();
    let task_id = onaji_core::domain::TaskId::new("build_step");

    // Run 1: Must execute and produce output.txt
    let summary1 = TaskScheduler::new(&dag, root, &mut cache)
        .execute_tasks(std::slice::from_ref(&task_id))
        .await
        .unwrap();
    assert!(summary1.success);
    assert_eq!(summary1.results[0].state, TaskState::Success);
    assert!(root.join("output.txt").exists());

    // Run 2: Input and output unchanged -> Must be Cached
    let summary2 = TaskScheduler::new(&dag, root, &mut cache)
        .execute_tasks(std::slice::from_ref(&task_id))
        .await
        .unwrap();
    assert!(summary2.success);
    assert_eq!(summary2.results[0].state, TaskState::Cached);

    // Delete output file from disk
    fs::remove_file(root.join("output.txt")).unwrap();
    assert!(!root.join("output.txt").exists());

    // Run 3: Physical output missing -> Restored automatically from CAS!
    let summary3 = TaskScheduler::new(&dag, root, &mut cache)
        .execute_tasks(std::slice::from_ref(&task_id))
        .await
        .unwrap();
    assert!(summary3.success);
    assert_eq!(summary3.results[0].state, TaskState::Cached);
    assert!(root.join("output.txt").exists());

    // Run 4: Delete both output file AND CAS cache -> Must re-execute!
    fs::remove_file(root.join("output.txt")).unwrap();
    let _ = fs::remove_dir_all(root.join(".onaji").join("cache").join("cas"));
    let summary4 = TaskScheduler::new(&dag, root, &mut cache)
        .execute_tasks(std::slice::from_ref(&task_id))
        .await
        .unwrap();
    assert!(summary4.success);
    assert_eq!(summary4.results[0].state, TaskState::Success);
    assert!(root.join("output.txt").exists());
}

#[tokio::test]
async fn test_task_retry_mechanism() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();

    // Script fails on 1st attempt (attempt_file does not exist, so it creates it and exits 1),
    // and succeeds on 2nd attempt (attempt_file exists, exits 0).
    let manifest_yaml = r#"
version: "1"
name: "retry-test"

resources:
  trigger:
    type: file
    path: "./trigger.txt"

tasks:
  flaky_task:
    inputs: [trigger]
    outputs: []
    run: "if [ -f .attempt ]; then echo 'recovered' && exit 0; else touch .attempt && exit 1; fi"
    retries: 2
"#;

    let manifest_path = root.join("onaji.yaml");
    fs::write(&manifest_path, manifest_yaml).unwrap();
    fs::write(root.join("trigger.txt"), "go").unwrap();

    let (_, dag) = ConfigLoader::load_from_file(&manifest_path).unwrap();
    let mut cache = ExecutionCache::new();
    let task_id = onaji_core::domain::TaskId::new("flaky_task");

    let summary = TaskScheduler::new(&dag, root, &mut cache)
        .execute_tasks(&[task_id])
        .await
        .unwrap();

    assert!(summary.success);
    assert_eq!(summary.results[0].state, TaskState::Success);
    assert!(summary.results[0].stdout.contains("recovered"));
}

#[tokio::test]
async fn test_fail_fast_vs_causal_isolation() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();

    let manifest_yaml = r#"
version: "1"
name: "fail-fast-test"

artifacts:
  art_a: {}
  art_seed: {}

tasks:
  task_seed:
    inputs: []
    outputs: [art_seed]
    run: "echo seed"

  task_fail_a:
    inputs: []
    outputs: [art_a]
    run: "exit 1"

  task_downstream_b:
    inputs: [art_a]
    outputs: []
    run: "echo downstream_ran"

  task_independent_c:
    inputs: [art_seed]
    outputs: []
    run: "echo independent_ran"
"#;

    let manifest_path = root.join("onaji.yaml");
    fs::write(&manifest_path, manifest_yaml).unwrap();

    let (_, dag) = ConfigLoader::load_from_file(&manifest_path).unwrap();
    let task_ids = vec![
        onaji_core::domain::TaskId::new("task_seed"),
        onaji_core::domain::TaskId::new("task_fail_a"),
        onaji_core::domain::TaskId::new("task_downstream_b"),
        onaji_core::domain::TaskId::new("task_independent_c"),
    ];

    // Case 1: Default (Keep-going causal mode) -> Independent task C succeeds, only downstream B blocked
    let mut cache1 = ExecutionCache::new();
    let summary1 = TaskScheduler::new(&dag, root, &mut cache1)
        .with_fail_fast(false)
        .execute_tasks(&task_ids)
        .await
        .unwrap();

    assert!(!summary1.success);
    let res_a1 = summary1
        .results
        .iter()
        .find(|r| r.task_id.as_str() == "task_fail_a")
        .unwrap();
    let res_b1 = summary1
        .results
        .iter()
        .find(|r| r.task_id.as_str() == "task_downstream_b")
        .unwrap();
    let res_c1 = summary1
        .results
        .iter()
        .find(|r| r.task_id.as_str() == "task_independent_c")
        .unwrap();
    assert_eq!(res_a1.state, TaskState::Failed);
    assert_eq!(res_b1.state, TaskState::Blocked);
    assert_eq!(res_c1.state, TaskState::Success);

    // Case 2: Fail-Fast mode -> Both downstream B and independent C in later waves are aborted/blocked
    let mut cache2 = ExecutionCache::new();
    let summary2 = TaskScheduler::new(&dag, root, &mut cache2)
        .with_fail_fast(true)
        .execute_tasks(&task_ids)
        .await
        .unwrap();

    assert!(!summary2.success);
    let res_a2 = summary2
        .results
        .iter()
        .find(|r| r.task_id.as_str() == "task_fail_a")
        .unwrap();
    let res_b2 = summary2
        .results
        .iter()
        .find(|r| r.task_id.as_str() == "task_downstream_b")
        .unwrap();
    let res_c2 = summary2
        .results
        .iter()
        .find(|r| r.task_id.as_str() == "task_independent_c")
        .unwrap();
    assert_eq!(res_a2.state, TaskState::Failed);
    assert_eq!(res_b2.state, TaskState::Blocked);
    assert_eq!(res_c2.state, TaskState::Blocked);
}

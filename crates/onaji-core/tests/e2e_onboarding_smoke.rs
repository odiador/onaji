use onaji_core::cache::{ExecutionCache, TaskState};
use onaji_core::config::ConfigLoader;
use onaji_core::domain::TaskId;
use onaji_core::scheduler::TaskScheduler;
use std::fs;

#[tokio::test]
async fn test_onboarding_and_smoke_golden_path() {
    let temp_dir = tempfile::tempdir().unwrap();
    let root = temp_dir.path();

    // 1. Create a raw project with only an OpenAPI spec
    let api_dir = root.join("backend/api");
    fs::create_dir_all(&api_dir).unwrap();
    fs::write(
        api_dir.join("openapi.yaml"),
        "openapi: 3.0.0\ninfo:\n  title: Smoke API\n  version: 1.0.0\npaths: {}\n",
    )
    .unwrap();

    // 2. Simulate Onboarding Scaffolding
    let scaffolded_yaml = r#"
version: "1"
name: "smoke-onboard-project"
resources:
  openapi_spec:
    type: file
    path: "./backend/api/openapi.yaml"
artifacts:
  validated_openapi: {}
tasks:
  validate_openapi:
    inputs: [openapi_spec]
    outputs: [validated_openapi]
    run: "echo onboard_success"
"#;
    let manifest_path = root.join("onaji.yaml");
    fs::write(&manifest_path, scaffolded_yaml).unwrap();

    // 3. Validate Config Loading & Graph
    let (raw_manifest, dag) = ConfigLoader::load_from_file(&manifest_path).expect("Load config");
    assert_eq!(raw_manifest.name, "smoke-onboard-project");
    assert_eq!(dag.resource_indices.len(), 1);
    assert_eq!(dag.task_indices.len(), 1);
    assert_eq!(dag.artifact_indices.len(), 1);

    let task_ids = vec![TaskId::new("validate_openapi")];

    // 4. Execute Pipeline via TaskScheduler (First Run -> Executed / Success)
    let mut cache = ExecutionCache::new();
    let mut scheduler = TaskScheduler::new(&dag, root, &mut cache);
    let summary1 = scheduler
        .execute_tasks(&task_ids)
        .await
        .expect("Execute tasks 1");
    assert!(summary1.success);
    assert_eq!(summary1.results.len(), 1);
    assert_eq!(summary1.results[0].state, TaskState::Success);

    // 5. Execute Pipeline Again (Second Run -> Cached)
    let mut scheduler2 = TaskScheduler::new(&dag, root, &mut cache);
    let summary2 = scheduler2
        .execute_tasks(&task_ids)
        .await
        .expect("Execute tasks 2");
    assert!(summary2.success);
    assert_eq!(summary2.results.len(), 1);
    assert_eq!(summary2.results[0].state, TaskState::Cached);
}

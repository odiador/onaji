use onaji_core::adapter::{Adapter, AdapterContext, OpenApiAdapter};
use onaji_core::cache::{ExecutionCache, TaskState};
use onaji_core::config::ConfigLoader;
use onaji_core::domain::{ResourceId, TaskId};
use onaji_core::graph::ImpactAnalyzer;
use onaji_core::scheduler::TaskScheduler;
use onaji_core::watcher::{EventNormalizer, NormalizationResult};
use std::fs::File;
use std::io::Write;

#[tokio::test]
async fn test_e2e_openapi_kotlin_gradle_all_scenarios() {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!("onaji_e2e_demo_{}", now));
    let backend_dir = temp_dir.join("backend/api");
    let android_dir = temp_dir.join("android/core/network/src/generated");
    let scripts_dir = temp_dir.join("scripts");

    std::fs::create_dir_all(&backend_dir).unwrap();
    std::fs::create_dir_all(&android_dir).unwrap();
    std::fs::create_dir_all(&scripts_dir).unwrap();

    let openapi_file = backend_dir.join("openapi.yaml");
    let mut f = File::create(&openapi_file).unwrap();
    writeln!(
        f,
        r#"openapi: "3.0.0"
info:
  title: "Onaji E2E PetStore API"
  version: "1.0.0"
paths:
  /health:
    get:
      summary: "Health check"
"#
    )
    .unwrap();

    let settings_file = temp_dir.join("android/settings.gradle.kts");
    let mut sf = File::create(&settings_file).unwrap();
    writeln!(sf, r#"include(":app", ":core:network")"#).unwrap();

    let gradlew_file = temp_dir.join("android/gradlew");
    let mut gf = File::create(&gradlew_file).unwrap();
    writeln!(gf, "#!/bin/sh\necho 'BUILD SUCCESSFUL'\nexit 0").unwrap();

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&gradlew_file, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    let gen_script = scripts_dir.join("generate-kotlin-client.sh");
    let mut gsf = File::create(&gen_script).unwrap();
    writeln!(
        gsf,
        "#!/bin/sh\necho 'package dev.odiador.generated' > android/core/network/src/generated/PetStoreClient.kt\nexit 0"
    )
    .unwrap();

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&gen_script, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    let manifest_yaml = format!(
        r#"
version: "1"
name: "e2e-demo"

resources:
  openapi_spec:
    type: file
    path: "{}"

  gradle_config:
    type: file
    path: "{}"

artifacts:
  validated_openapi: {{}}
  kotlin_client_sdk: {{}}
  android_build_classes: {{}}

tasks:
  validate_openapi:
    inputs: [openapi_spec]
    outputs: [validated_openapi]
    run: "test -f backend/api/openapi.yaml"

  generate_kotlin_client:
    inputs: [validated_openapi]
    outputs: [kotlin_client_sdk]
    run: "./scripts/generate-kotlin-client.sh"

  gradle_check:
    inputs: [kotlin_client_sdk]
    outputs: [android_build_classes]
    run: "cd android && ./gradlew test"
"#,
        openapi_file.to_string_lossy(),
        settings_file.to_string_lossy()
    );

    let (_, dag) = ConfigLoader::load_from_str(&manifest_yaml).unwrap();
    let mut cache = ExecutionCache::new();

    // =========================================================================
    // SCENARIO 1: Real Change -> Impact Analysis -> Full Success Pipeline
    // =========================================================================
    let analyzer = ImpactAnalyzer::new(&dag);
    let impact = analyzer
        .analyze_resource_change(&ResourceId::new("openapi_spec"))
        .unwrap();

    assert_eq!(
        impact.affected_tasks,
        vec![
            TaskId::new("validate_openapi"),
            TaskId::new("generate_kotlin_client"),
            TaskId::new("gradle_check"),
        ]
    );

    let mut scheduler = TaskScheduler::new(&dag, &temp_dir, &mut cache);
    let summary1 = scheduler.execute_impact(&impact).await.unwrap();

    assert!(summary1.success);
    assert_eq!(summary1.results.len(), 3);
    assert_eq!(summary1.results[0].state, TaskState::Success);
    assert_eq!(summary1.results[1].state, TaskState::Success);
    assert_eq!(summary1.results[2].state, TaskState::Success);

    // Verify Kotlin generated file exists
    let generated_kt = temp_dir.join("android/core/network/src/generated/PetStoreClient.kt");
    assert!(generated_kt.exists());

    // =========================================================================
    // SCENARIO 2: `touch` on openapi.yaml -> EventNormalizer ignores unchanged
    // =========================================================================
    let mut normalizer = EventNormalizer::new(&temp_dir);
    normalizer.register_resources(std::iter::once(
        dag.get_resource(&ResourceId::new("openapi_spec")).unwrap(),
    ));

    let touch_res = normalizer.normalize_fs_event(&openapi_file);
    assert_eq!(
        touch_res,
        NormalizationResult::IgnoredUnchanged {
            resource_id: ResourceId::new("openapi_spec")
        }
    );

    // =========================================================================
    // SCENARIO 3: Cache Hit on Second Run
    // =========================================================================
    let mut scheduler2 = TaskScheduler::new(&dag, &temp_dir, &mut cache);
    let summary2 = scheduler2.execute_impact(&impact).await.unwrap();

    assert!(summary2.success);
    assert_eq!(summary2.results.len(), 3);
    assert_eq!(summary2.results[0].state, TaskState::Cached);
    assert_eq!(summary2.results[1].state, TaskState::Cached);
    assert_eq!(summary2.results[2].state, TaskState::Cached);

    // =========================================================================
    // SCENARIO 4: Invalid OpenAPI -> Failure -> Downstream Blocked
    // =========================================================================
    let invalid_manifest = format!(
        r#"
version: "1"
name: "e2e-demo-invalid"

resources:
  openapi_spec:
    type: file
    path: "{}"

artifacts:
  validated_openapi: {{}}
  kotlin_client_sdk: {{}}
  android_build_classes: {{}}

tasks:
  validate_openapi:
    inputs: [openapi_spec]
    outputs: [validated_openapi]
    run: "exit 1"

  generate_kotlin_client:
    inputs: [validated_openapi]
    outputs: [kotlin_client_sdk]
    run: "./scripts/generate-kotlin-client.sh"

  gradle_check:
    inputs: [kotlin_client_sdk]
    outputs: [android_build_classes]
    run: "cd android && ./gradlew test"
"#,
        openapi_file.to_string_lossy()
    );

    let (_, dag_invalid) = ConfigLoader::load_from_str(&invalid_manifest).unwrap();
    let mut cache_invalid = ExecutionCache::new();

    let analyzer_inv = ImpactAnalyzer::new(&dag_invalid);
    let impact_inv = analyzer_inv
        .analyze_resource_change(&ResourceId::new("openapi_spec"))
        .unwrap();

    let mut scheduler3 = TaskScheduler::new(&dag_invalid, &temp_dir, &mut cache_invalid);
    let summary3 = scheduler3.execute_impact(&impact_inv).await.unwrap();

    assert!(!summary3.success);
    assert_eq!(summary3.results.len(), 3);
    assert_eq!(summary3.results[0].state, TaskState::Failed);
    assert_eq!(summary3.results[1].state, TaskState::Blocked);
    assert_eq!(summary3.results[2].state, TaskState::Blocked);

    // =========================================================================
    // Semantic OpenAPI Validation via Adapter
    // =========================================================================
    let adapter = OpenApiAdapter::new();
    let adapter_ctx = AdapterContext::new(&temp_dir).with_resource_path(&openapi_file);
    let val_res = adapter.validate(&adapter_ctx).unwrap();
    assert!(val_res.is_valid);

    // Cleanup
    let _ = std::fs::remove_dir_all(&temp_dir);
}

use onaji_core::cache::{ExecutionCache, TaskState};
use onaji_core::config::ConfigLoader;
use onaji_core::domain::{ResourceId, TaskId};
use onaji_core::graph::ImpactAnalyzer;
use onaji_core::http::{ContentState, HttpPoller, ResourceAvailability};
use onaji_core::scheduler::TaskScheduler;
use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn test_e2e_rust_axum_android_pipeline_and_isolation() {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!("onaji_e2e_axum_{}", now));
    let backend_src = temp_dir.join("backend/src");
    let backend_api = temp_dir.join("backend/api");
    let android_gen = temp_dir.join("android/core/network/src/generated");
    let scripts_dir = temp_dir.join("scripts");

    std::fs::create_dir_all(&backend_src).unwrap();
    std::fs::create_dir_all(&backend_api).unwrap();
    std::fs::create_dir_all(&android_gen).unwrap();
    std::fs::create_dir_all(&scripts_dir).unwrap();

    let rust_main = backend_src.join("main.rs");
    let mut rf = File::create(&rust_main).unwrap();
    writeln!(rf, "fn main() {{ println!(\"Axum service\"); }}").unwrap();

    let openapi_yaml = backend_api.join("openapi.yaml");
    let mut of = File::create(&openapi_yaml).unwrap();
    writeln!(
        of,
        r#"openapi: "3.0.0"
info:
  title: "Axum API"
  version: "1.0.0"
paths:
  /users:
    get:
      summary: "List users"
"#
    )
    .unwrap();

    let gradlew_file = temp_dir.join("android/gradlew");
    let mut gf = File::create(&gradlew_file).unwrap();
    writeln!(gf, "#!/bin/sh\necho 'GRADLE BUILD SUCCESS'\nexit 0").unwrap();

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&gradlew_file, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    let gen_script = scripts_dir.join("generate-kotlin-client.sh");
    let mut gsf = File::create(&gen_script).unwrap();
    writeln!(
        gsf,
        "#!/bin/sh\necho 'class AxumClient' > android/core/network/src/generated/AxumApiClient.kt\nexit 0"
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
name: "axum-android-test"

resources:
  backend_source:
    type: file
    path: "{}"

  openapi_spec:
    type: file
    path: "{}"

artifacts:
  backend_binary: {{}}
  validated_openapi: {{}}
  kotlin_client_sdk: {{}}
  android_build_classes: {{}}

tasks:
  build_backend:
    inputs: [backend_source]
    outputs: [backend_binary]
    run: "test -f backend/src/main.rs"

  validate_openapi:
    inputs: [openapi_spec]
    outputs: [validated_openapi]
    run: "test -f backend/api/openapi.yaml"

  generate_kotlin_client:
    inputs: [validated_openapi]
    outputs: [kotlin_client_sdk]
    run: "./scripts/generate-kotlin-client.sh"

  compile_android:
    inputs: [kotlin_client_sdk]
    outputs: [android_build_classes]
    run: "cd android && ./gradlew test"
"#,
        rust_main.to_string_lossy(),
        openapi_yaml.to_string_lossy()
    );

    let (_, dag) = ConfigLoader::load_from_str(&manifest_yaml).unwrap();
    let mut cache = ExecutionCache::new();
    let analyzer = ImpactAnalyzer::new(&dag);

    // =========================================================================
    // 1. Rust Source Change Isolation -> ONLY build_backend is affected
    // =========================================================================
    let rust_impact = analyzer
        .analyze_resource_change(&ResourceId::new("backend_source"))
        .unwrap();

    assert_eq!(
        rust_impact.affected_tasks,
        vec![TaskId::new("build_backend")]
    );
    assert_eq!(
        rust_impact.unaffected_tasks,
        vec![
            TaskId::new("compile_android"),
            TaskId::new("generate_kotlin_client"),
            TaskId::new("validate_openapi")
        ]
    );

    let mut scheduler1 = TaskScheduler::new(&dag, &temp_dir, &mut cache);
    let sum1 = scheduler1.execute_impact(&rust_impact).await.unwrap();
    assert!(sum1.success);
    assert_eq!(sum1.results.len(), 1);
    assert_eq!(sum1.results[0].task_id, TaskId::new("build_backend"));
    assert_eq!(sum1.results[0].state, TaskState::Success);

    // =========================================================================
    // 2. OpenAPI Spec Change -> Triggers Pipeline (Validate -> Gen -> Gradle)
    // =========================================================================
    let spec_impact = analyzer
        .analyze_resource_change(&ResourceId::new("openapi_spec"))
        .unwrap();

    assert_eq!(
        spec_impact.affected_tasks,
        vec![
            TaskId::new("validate_openapi"),
            TaskId::new("generate_kotlin_client"),
            TaskId::new("compile_android")
        ]
    );
    assert_eq!(
        spec_impact.unaffected_tasks,
        vec![TaskId::new("build_backend")]
    );

    let mut scheduler2 = TaskScheduler::new(&dag, &temp_dir, &mut cache);
    let sum2 = scheduler2.execute_impact(&spec_impact).await.unwrap();
    assert!(sum2.success);
    assert_eq!(sum2.results.len(), 3);
    assert_eq!(sum2.results[0].state, TaskState::Success);
    assert_eq!(sum2.results[1].state, TaskState::Success);
    assert_eq!(sum2.results[2].state, TaskState::Success);

    // Generated Kotlin file exists
    assert!(temp_dir
        .join("android/core/network/src/generated/AxumApiClient.kt")
        .exists());

    // =========================================================================
    // 3. HTTP Availability Health Check Decoupled from Content Changes
    // =========================================================================
    let mock_server = MockServer::start().await;

    // Available (200 OK)
    Mock::given(method("GET"))
        .and(path("/health"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({ "status": "ok" })),
        )
        .mount(&mock_server)
        .await;

    let mut poller = HttpPoller::new();
    let health_url = format!("{}/health", mock_server.uri());
    let headers = HashMap::new();
    let res_id = ResourceId::new("backend_health");

    let outcome_ok = poller.poll_resource(&res_id, &health_url, &headers).await;
    assert_eq!(outcome_ok.availability, ResourceAvailability::Available);
    assert!(matches!(
        outcome_ok.content_state,
        ContentState::Modified { .. }
    ));

    // Degraded (500 Server Error)
    mock_server.reset().await;
    Mock::given(method("GET"))
        .and(path("/health"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&mock_server)
        .await;

    let outcome_deg = poller.poll_resource(&res_id, &health_url, &headers).await;
    assert!(matches!(
        outcome_deg.availability,
        ResourceAvailability::Degraded {
            status: Some(500),
            ..
        }
    ));
    assert_eq!(outcome_deg.content_state, ContentState::Unknown);

    let _ = std::fs::remove_dir_all(&temp_dir);
}

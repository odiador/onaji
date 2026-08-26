use onaji_core::cache::{ExecutionCache, TaskState};
use onaji_core::config::ConfigLoader;
use onaji_core::domain::TaskId;
use onaji_core::registry::{ContentAddressedStorage, PublishRequest, RegistryDatabase};
use onaji_core::release::{ChecksumVerifier, SignatureVerifier};
use onaji_core::scheduler::TaskScheduler;
use onaji_core::security::PolicyChecker;
use onaji_core::telemetry::{InMemoryTraceCollector, SpanKind, SpanStatus, TelemetryCollector};
use std::fs;

#[tokio::test]
async fn test_release_candidate_end_to_end_stabilization() {
    let temp_dir = tempfile::tempdir().unwrap();
    let root = temp_dir.path();

    // =========================================================================
    // 1. DAG & Incremental CAS Caching
    // =========================================================================
    let api_dir = root.join("api");
    fs::create_dir_all(&api_dir).unwrap();
    fs::write(api_dir.join("openapi.yaml"), "openapi: 3.0.0\n").unwrap();

    let manifest_yaml = r#"
version: "1"
name: "rc1-polyglot-test"
resources:
  openapi_spec:
    type: file
    path: "./api/openapi.yaml"
artifacts:
  validated_contract: {}
tasks:
  validate_openapi:
    inputs: [openapi_spec]
    outputs: [validated_contract]
    run: "echo rc1_contract_validated"
"#;
    let manifest_path = root.join("onaji.yaml");
    fs::write(&manifest_path, manifest_yaml).unwrap();

    let (_, dag) = ConfigLoader::load_from_file(&manifest_path).expect("Load DAG");
    let mut cache = ExecutionCache::new();
    let task_ids = vec![TaskId::new("validate_openapi")];

    // First execution -> Miss -> Success
    let mut scheduler1 = TaskScheduler::new(&dag, root, &mut cache);
    let summary1 = scheduler1
        .execute_tasks(&task_ids)
        .await
        .expect("First run");
    assert!(summary1.success);
    assert_eq!(summary1.results[0].state, TaskState::Success);

    // Second execution -> Hit -> Cached
    let mut scheduler2 = TaskScheduler::new(&dag, root, &mut cache);
    let summary2 = scheduler2
        .execute_tasks(&task_ids)
        .await
        .expect("Second run");
    assert!(summary2.success);
    assert_eq!(summary2.results[0].state, TaskState::Cached);

    // =========================================================================
    // 2. Security Sandbox & Injection Defense
    // =========================================================================
    let policy = PolicyChecker::default();
    assert!(policy.validate_command("echo hello").is_ok());
    assert!(policy.validate_command("echo hello; rm -rf /").is_err());
    assert!(policy.validate_command("sh -c 'whoami'").is_err());

    // =========================================================================
    // 3. Cryptographic Release Signature Verification
    // =========================================================================
    let raw_key: [u8; 32] = [88u8; 32];
    let payload = b"ONAJI_RC1_OFFICIAL_DISTRIBUTION_BINARY";
    let (sig_hex, pub_hex) = SignatureVerifier::sign_ed25519(payload, &raw_key);
    assert!(SignatureVerifier::verify_ed25519(payload, &sig_hex, &pub_hex).unwrap());

    // =========================================================================
    // 4. Content-Addressed Plugin Registry & CAS Storage
    // =========================================================================
    let reg_db = RegistryDatabase::new();
    let reg_storage = ContentAddressedStorage::new();
    let sha256 = ChecksumVerifier::compute_sha256(payload);

    let pub_req = PublishRequest {
        name: "@onaji/rc-plugin".to_string(),
        version: "1.0.0".to_string(),
        protocol_version: "0.1.0".to_string(),
        runtime: "wasm".to_string(),
        sha256: sha256.clone(),
        signature_hex: sig_hex,
        public_key: pub_hex,
        description: "Release Candidate Verified Plugin".to_string(),
        author: "Onaji Core Team".to_string(),
        artifact_base64: hex::encode(payload),
    };

    let pub_res = reg_db
        .publish(pub_req, &reg_storage)
        .expect("Publish to CAS");
    assert_eq!(pub_res.content_address, sha256);
    assert_eq!(
        reg_storage.get(&pub_res.content_address).unwrap(),
        payload.to_vec()
    );

    // =========================================================================
    // 5. Causal Telemetry Spans & Metric Collector
    // =========================================================================
    let collector = InMemoryTraceCollector::new();
    let span_id = collector.start_span(
        "task_validate_openapi",
        None,
        "onaji.task.execute",
        SpanKind::TaskExecute,
    );
    collector.set_attribute(&span_id, "task.id", "validate_openapi");
    collector.set_attribute(&span_id, "causal.trigger", "openapi_spec");
    collector.finish_span(&span_id, SpanStatus::Ok);

    let explanation = collector.explain_task("validate_openapi");
    assert!(explanation.contains("validate_openapi"));
    assert!(explanation.contains("openapi_spec"));
}

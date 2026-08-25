use onaji_core::cache::{
    CacheBackend, CacheCoordinator, CacheEntry, CacheKey, HttpRemoteCacheBackend,
    InMemoryCacheBackend, LocalDiskCacheBackend, TaskState,
};
use onaji_core::domain::TaskId;
use std::collections::HashMap;
use std::sync::Arc;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn test_distributed_cache_coordinator_and_multi_machine_simulation() {
    // 1. Setup Mock Remote HTTP Server (Simulating S3 / Remote Cache Cluster)
    let mock_server = MockServer::start().await;

    let test_key =
        CacheKey::new("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
    let test_entry = CacheEntry::new(
        test_key.clone(),
        TaskId::new("validate_openapi"),
        TaskState::Success,
        Some(0),
        "OpenAPI validated cleanly".to_string(),
        "".to_string(),
        42,
        HashMap::from([("openapi.yaml".to_string(), "hash_abc123".to_string())]),
    );

    // Mock PUT /cache/{key} (Machine A uploading entry)
    Mock::given(method("PUT"))
        .and(path(format!("/cache/{}", test_key.as_str())))
        .respond_with(ResponseTemplate::new(200))
        .mount(&mock_server)
        .await;

    // Mock GET /cache/{key} (Machine B downloading entry)
    Mock::given(method("GET"))
        .and(path(format!("/cache/{}", test_key.as_str())))
        .respond_with(ResponseTemplate::new(200).set_body_json(&test_entry))
        .mount(&mock_server)
        .await;

    // -----------------------------------------------------------------------
    // Machine A: Local Execution + Upload to Remote Cache
    // -----------------------------------------------------------------------
    let machine_a_local = Arc::new(InMemoryCacheBackend::new());
    let machine_a_remote = Arc::new(HttpRemoteCacheBackend::new(mock_server.uri(), None));
    let machine_a_coordinator =
        CacheCoordinator::new(machine_a_local.clone(), Some(machine_a_remote.clone()));

    // Machine A checks cache for unknown key -> MISS
    let unknown_key =
        CacheKey::new("0000000000000000000000000000000000000000000000000000000000000000");
    let initial_lookup = machine_a_coordinator.get(&unknown_key).await.unwrap();
    assert!(initial_lookup.is_none());

    // Machine A executes task and saves to cache
    machine_a_coordinator.put(&test_entry).await.unwrap();
    assert!(machine_a_local.has(&test_key).await.unwrap());

    // -----------------------------------------------------------------------
    // Machine B (Clean Environment): Local MISS -> Remote HIT -> Tier 1 Populated
    // -----------------------------------------------------------------------
    let temp_dir_b = tempfile::tempdir().unwrap();
    let machine_b_local = Arc::new(LocalDiskCacheBackend::new(temp_dir_b.path()));
    let machine_b_remote = Arc::new(HttpRemoteCacheBackend::new(mock_server.uri(), None));
    let machine_b_coordinator =
        CacheCoordinator::new(machine_b_local.clone(), Some(machine_b_remote.clone()));

    // Verify Machine B initially has no local entry on disk
    assert!(!machine_b_local.has(&test_key).await.unwrap());

    // Machine B queries Coordinator -> Local MISS -> Remote HIT -> Populates Local Disk
    let restored_entry = machine_b_coordinator.get(&test_key).await.unwrap();
    assert!(restored_entry.is_some());

    let entry = restored_entry.unwrap();
    assert_eq!(entry.key, test_key);
    assert_eq!(entry.task_id, TaskId::new("validate_openapi"));
    assert_eq!(entry.state, TaskState::Success);
    assert_eq!(entry.stdout, "OpenAPI validated cleanly");

    // Verify Tier 1 local disk cache now has the entry populated
    assert!(machine_b_local.has(&test_key).await.unwrap());
    let local_disk_entry = machine_b_local.get(&test_key).await.unwrap();
    assert!(local_disk_entry.is_some());
    assert_eq!(local_disk_entry.unwrap().key, test_key);
}

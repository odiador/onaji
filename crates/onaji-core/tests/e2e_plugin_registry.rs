use onaji_core::plugin::{
    OnajiLockfile, PluginInstaller, PluginPackageInfo, PluginRegistryClient, PluginRelease,
};
use sha2::{Digest, Sha256};
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn test_plugin_registry_and_deterministic_lockfile() {
    let mock_server = MockServer::start().await;

    let package_payload = b"console.log('Onaji OpenAPI Plugin v1.2.0');";
    let package_sha256 = hex::encode(Sha256::digest(package_payload));

    let release_1_2 = PluginRelease {
        name: "@onaji/openapi".to_string(),
        version: "1.2.0".to_string(),
        protocol_version: "0.1.0".to_string(),
        capabilities: vec![
            "Detect".to_string(),
            "Validate".to_string(),
            "Inspect".to_string(),
        ],
        sha256: package_sha256.clone(),
        download_url: format!(
            "{}/v1/plugins/@onaji/openapi/1.2.0/download",
            mock_server.uri()
        ),
        description: Some("Official OpenAPI 3.0 adapter for Onaji".to_string()),
    };

    let package_info = PluginPackageInfo {
        name: "@onaji/openapi".to_string(),
        description: "Official OpenAPI 3.0 adapter for Onaji".to_string(),
        latest_version: "1.2.0".to_string(),
        versions: vec![release_1_2.clone()],
    };

    // 1. Mock Search endpoint: GET /v1/plugins?q=openapi
    Mock::given(method("GET"))
        .and(path("/v1/plugins"))
        .and(query_param("q", "openapi"))
        .respond_with(ResponseTemplate::new(200).set_body_json(vec![package_info.clone()]))
        .mount(&mock_server)
        .await;

    // 2. Mock Info endpoint: GET /v1/plugins/@onaji/openapi
    Mock::given(method("GET"))
        .and(path("/v1/plugins/@onaji/openapi"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&package_info))
        .mount(&mock_server)
        .await;

    // 3. Mock Release endpoint: GET /v1/plugins/@onaji/openapi/1.2.0
    Mock::given(method("GET"))
        .and(path("/v1/plugins/@onaji/openapi/1.2.0"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&release_1_2))
        .mount(&mock_server)
        .await;

    // 4. Mock Download endpoint: GET /v1/plugins/@onaji/openapi/1.2.0/download
    Mock::given(method("GET"))
        .and(path("/v1/plugins/@onaji/openapi/1.2.0/download"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(package_payload.to_vec()))
        .mount(&mock_server)
        .await;

    let client = PluginRegistryClient::new(mock_server.uri(), None);
    let temp_workspace = tempfile::tempdir().unwrap();
    let workspace_root = temp_workspace.path();

    // -----------------------------------------------------------------------
    // Scenario A: Search & Introspection
    // -----------------------------------------------------------------------
    let search_results = client.search("openapi").await.unwrap();
    assert_eq!(search_results.len(), 1);
    assert_eq!(search_results[0].name, "@onaji/openapi");

    let info = client.get_info("@onaji/openapi").await.unwrap();
    assert_eq!(info.latest_version, "1.2.0");

    // -----------------------------------------------------------------------
    // Scenario B: Install Plugin & Generate Deterministic onaji.lock
    // -----------------------------------------------------------------------
    let locked = PluginInstaller::install("@onaji/openapi@1.2.0", workspace_root, &client)
        .await
        .unwrap();

    assert_eq!(locked.name, "@onaji/openapi");
    assert_eq!(locked.version, "1.2.0");
    assert_eq!(locked.sha256, package_sha256);

    // Verify onaji.lock file on disk
    let lockfile = OnajiLockfile::load_from_dir(workspace_root).unwrap();
    assert_eq!(lockfile.plugins.len(), 1);
    assert_eq!(lockfile.plugins[0].name, "@onaji/openapi");
    assert_eq!(lockfile.plugins[0].sha256, package_sha256);

    // Verify binary saved in .onaji/plugins/@onaji_openapi/1.2.0/package.bin
    let installed_bin = workspace_root
        .join(".onaji")
        .join("plugins")
        .join("@onaji_openapi")
        .join("1.2.0")
        .join("package.bin");
    assert!(installed_bin.exists());
    let installed_content = std::fs::read(&installed_bin).unwrap();
    assert_eq!(installed_content, package_payload);

    // Verify Checksum Integrity
    lockfile
        .verify_checksum("@onaji/openapi", &installed_content)
        .unwrap();

    // Verify Tamper Detection: altered bytes must fail integrity verification
    let tampered_payload = b"malicious injected code";
    let tamper_res = lockfile.verify_checksum("@onaji/openapi", tampered_payload);
    assert!(tamper_res.is_err());

    // -----------------------------------------------------------------------
    // Scenario C: Uninstall Plugin & Update Lockfile
    // -----------------------------------------------------------------------
    let uninstalled = PluginInstaller::uninstall("@onaji/openapi", workspace_root).unwrap();
    assert!(uninstalled);

    let updated_lockfile = OnajiLockfile::load_from_dir(workspace_root).unwrap();
    assert!(updated_lockfile.plugins.is_empty());
    assert!(!installed_bin.exists());
}

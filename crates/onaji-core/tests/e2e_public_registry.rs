use onaji_core::registry::{
    ContentAddressedStorage, PublishRequest, RegistryDatabase, RegistryError,
};
use onaji_core::release::{ChecksumVerifier, SignatureVerifier};

#[test]
fn test_public_plugin_registry_lifecycle_and_rejections() {
    let db = RegistryDatabase::new();
    let storage = ContentAddressedStorage::new();

    let raw_key: [u8; 32] = [77u8; 32];
    let wasm_payload = b"OFFICIAL_WASM_PLUGIN_BINARY_BYTES";
    let sha256 = ChecksumVerifier::compute_sha256(wasm_payload);
    let (sig_hex, pub_key_hex) = SignatureVerifier::sign_ed25519(wasm_payload, &raw_key);

    // 1. Successful Publish
    let publish_req = PublishRequest {
        name: "@onaji/openapi".to_string(),
        version: "1.2.0".to_string(),
        protocol_version: "0.1.0".to_string(),
        runtime: "wasm".to_string(),
        sha256: sha256.clone(),
        signature_hex: sig_hex.clone(),
        public_key: pub_key_hex.clone(),
        description: "Official OpenAPI Contract Validator Plugin".to_string(),
        author: "Onaji Core Team".to_string(),
        artifact_base64: hex::encode(wasm_payload),
    };

    let publish_res = db
        .publish(publish_req.clone(), &storage)
        .expect("Publish should succeed");
    assert_eq!(publish_res.status, "published");
    assert_eq!(publish_res.name, "@onaji/openapi");
    assert_eq!(publish_res.version, "1.2.0");
    assert_eq!(publish_res.content_address, sha256);

    // 2. Query & Search Metadata
    let plugin_meta = db.get_plugin("@onaji/openapi").expect("Find plugin");
    assert_eq!(plugin_meta.latest_version, "1.2.0");

    let version_meta = db
        .get_version("@onaji/openapi", "1.2.0")
        .expect("Find version");
    assert_eq!(version_meta.sha256, sha256);
    assert_eq!(version_meta.signature_hex, sig_hex);

    let search_results = db.search("openapi");
    assert_eq!(search_results.len(), 1);
    assert_eq!(search_results[0].name, "@onaji/openapi");

    // 3. Verify Content Addressed Storage Retrieval
    let stored_bytes = storage
        .get(&publish_res.content_address)
        .expect("Retrieve from CAS");
    assert_eq!(stored_bytes, wasm_payload);

    // 4. Invariant: Duplicate Version Rejection
    let duplicate_err = db
        .publish(publish_req, &storage)
        .expect_err("Duplicate version must be rejected");
    assert!(matches!(duplicate_err, RegistryError::DuplicateVersion(_)));

    // 5. Invariant: Checksum Mismatch Rejection
    let tampered_sha_req = PublishRequest {
        name: "@onaji/openapi".to_string(),
        version: "1.2.1".to_string(),
        protocol_version: "0.1.0".to_string(),
        runtime: "wasm".to_string(),
        sha256: "0000000000000000000000000000000000000000000000000000000000000000".to_string(),
        signature_hex: sig_hex.clone(),
        public_key: pub_key_hex.clone(),
        description: "Tampered Hash".to_string(),
        author: "Tester".to_string(),
        artifact_base64: hex::encode(wasm_payload),
    };
    let checksum_err = db
        .publish(tampered_sha_req, &storage)
        .expect_err("Checksum mismatch must be rejected");
    assert!(matches!(checksum_err, RegistryError::ChecksumMismatch));

    // 6. Invariant: Invalid Signature Rejection
    let invalid_sig_req = PublishRequest {
        name: "@onaji/openapi".to_string(),
        version: "1.2.2".to_string(),
        protocol_version: "0.1.0".to_string(),
        runtime: "wasm".to_string(),
        sha256: sha256.clone(),
        signature_hex: "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff".to_string(),
        public_key: pub_key_hex,
        description: "Invalid Sig".to_string(),
        author: "Tester".to_string(),
        artifact_base64: hex::encode(wasm_payload),
    };
    let sig_err = db
        .publish(invalid_sig_req, &storage)
        .expect_err("Invalid signature must be rejected");
    assert!(matches!(sig_err, RegistryError::InvalidSignature));

    // 7. Invariant: Incompatible Protocol Version Rejection
    let (_, new_pub) = SignatureVerifier::sign_ed25519(wasm_payload, &raw_key);
    let bad_proto_req = PublishRequest {
        name: "@onaji/openapi".to_string(),
        version: "1.2.3".to_string(),
        protocol_version: "9.9.9".to_string(),
        runtime: "wasm".to_string(),
        sha256,
        signature_hex: sig_hex,
        public_key: new_pub,
        description: "Incompatible".to_string(),
        author: "Tester".to_string(),
        artifact_base64: hex::encode(wasm_payload),
    };
    let proto_err = db
        .publish(bad_proto_req, &storage)
        .expect_err("Incompatible protocol must be rejected");
    assert!(matches!(proto_err, RegistryError::IncompatibleProtocol(_)));
}

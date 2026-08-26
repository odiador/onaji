use onaji_core::release::{ChecksumVerifier, ReleaseArtifact, ReleaseManifest};

#[test]
fn test_production_packaging_and_checksum_verification() {
    // 1. Create Sample Release Manifest
    let mut manifest = ReleaseManifest::new("0.1.0", "0.1.0");

    let darwin_bytes = b"ONAJI_BINARY_DARWIN_ARM64_PAYLOAD_CONTENT";
    let darwin_sha = ChecksumVerifier::compute_sha256(darwin_bytes);

    let linux_bytes = b"ONAJI_BINARY_LINUX_X86_64_PAYLOAD_CONTENT";
    let linux_sha = ChecksumVerifier::compute_sha256(linux_bytes);

    manifest.add_artifact(ReleaseArtifact {
        filename: "onaji-v0.1.0-darwin-arm64.tar.gz".to_string(),
        os: "darwin".to_string(),
        arch: "arm64".to_string(),
        sha256: darwin_sha.clone(),
        size_bytes: darwin_bytes.len() as u64,
        signature: None,
    });

    manifest.add_artifact(ReleaseArtifact {
        filename: "onaji-v0.1.0-linux-x86_64.tar.gz".to_string(),
        os: "linux".to_string(),
        arch: "x86_64".to_string(),
        sha256: linux_sha.clone(),
        size_bytes: linux_bytes.len() as u64,
        signature: None,
    });

    // 2. Validate JSON Serialization and Roundtrip
    let json = manifest.to_json().expect("Serialize manifest");
    assert!(json.contains("onaji-v0.1.0-darwin-arm64.tar.gz"));
    assert!(json.contains(&darwin_sha));

    let parsed = ReleaseManifest::from_json(&json).expect("Deserialize manifest");
    assert_eq!(parsed.version, "0.1.0");
    assert_eq!(parsed.artifacts.len(), 2);

    // 3. Validate Target Lookup
    let found_darwin = parsed.find_artifact("darwin", "arm64").unwrap();
    assert_eq!(found_darwin.filename, "onaji-v0.1.0-darwin-arm64.tar.gz");
    assert_eq!(found_darwin.sha256, darwin_sha);

    let found_linux = parsed.find_artifact("linux", "x86_64").unwrap();
    assert_eq!(found_linux.filename, "onaji-v0.1.0-linux-x86_64.tar.gz");

    assert!(parsed.find_artifact("solaris", "sparc").is_none());

    // 4. Validate Checksum Verification
    assert!(ChecksumVerifier::verify(darwin_bytes, &darwin_sha));
    assert!(ChecksumVerifier::verify(linux_bytes, &linux_sha));

    // Tampered payload must be rejected
    let tampered_bytes = b"TAMPERED_PAYLOAD";
    assert!(!ChecksumVerifier::verify(tampered_bytes, &darwin_sha));

    // 5. Validate SHA256SUMS format parsing & generation
    let entries = vec![
        (
            "onaji-v0.1.0-darwin-arm64.tar.gz".to_string(),
            darwin_sha.clone(),
        ),
        (
            "onaji-v0.1.0-linux-x86_64.tar.gz".to_string(),
            linux_sha.clone(),
        ),
    ];
    let formatted = ChecksumVerifier::format_sha256sums(&entries);
    assert!(formatted.contains(&darwin_sha));
    assert!(formatted.contains("onaji-v0.1.0-darwin-arm64.tar.gz"));

    let parsed_sums = ChecksumVerifier::parse_sha256sums(&formatted);
    assert_eq!(
        parsed_sums.get("onaji-v0.1.0-darwin-arm64.tar.gz").unwrap(),
        &darwin_sha
    );
    assert_eq!(
        parsed_sums.get("onaji-v0.1.0-linux-x86_64.tar.gz").unwrap(),
        &linux_sha
    );
}

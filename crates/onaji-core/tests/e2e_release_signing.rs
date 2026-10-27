use onaji_core::release::{
    ArtifactSignature, ChecksumVerifier, ReleaseArtifact, ReleaseManifest, SignatureVerifier,
};

#[test]
fn test_release_signing_and_cryptographic_verification() {
    let raw_signing_key: [u8; 32] = [42u8; 32];
    let payload = b"OFFICIAL_ONAJI_COMPILED_RELEASE_BINARY_PAYLOAD";

    // 1. Compute SHA-256 and Sign
    let sha256 = ChecksumVerifier::compute_sha256(payload);
    let (sig_hex, pub_key_hex) = SignatureVerifier::sign_ed25519(payload, &raw_signing_key);

    let signature = ArtifactSignature {
        sig_type: "ed25519".to_string(),
        signature_hex: sig_hex.clone(),
        public_key_hint: Some(pub_key_hex.clone()),
    };

    // 2. Build Signed Release Manifest
    let mut manifest = ReleaseManifest::new("0.1.0", "0.1.0");
    manifest.add_artifact(ReleaseArtifact {
        filename: "onaji-v0.1.0-linux-x86_64.tar.gz".to_string(),
        os: "linux".to_string(),
        arch: "x86_64".to_string(),
        sha256: sha256.clone(),
        size_bytes: payload.len() as u64,
        signature: Some(signature.clone()),
    });

    let manifest_json = manifest.to_json().expect("Serialize manifest");
    assert!(manifest_json.contains(&sig_hex));
    assert!(manifest_json.contains(&pub_key_hex));

    // 3. Verify Valid Signature
    let is_valid = SignatureVerifier::verify_artifact(payload, &signature, &pub_key_hex);
    assert!(is_valid, "Valid signature must be verified as true");

    // 4. Verify Tampered Payload is Rejected
    let tampered_payload = b"TAMPERED_MALICIOUS_RELEASE_BINARY";
    let tampered_valid =
        SignatureVerifier::verify_artifact(tampered_payload, &signature, &pub_key_hex);
    assert!(
        !tampered_valid,
        "Tampered payload must be rejected by signature verifier"
    );

    // 5. Verify Mismatched Public Key is Rejected
    let other_signing_key: [u8; 32] = [99u8; 32];
    let (_, other_pub_key) = SignatureVerifier::sign_ed25519(b"test", &other_signing_key);
    let wrong_key_valid = SignatureVerifier::verify_artifact(payload, &signature, &other_pub_key);
    assert!(
        !wrong_key_valid,
        "Signature verified with wrong key must be rejected"
    );
}

#[test]
fn test_release_command_sign_and_verify_e2e() {
    let temp_dir = tempfile::tempdir().unwrap();
    let root = temp_dir.path();
    let artifact_path = root.join("artifact.bin");
    let sig_path = root.join("artifact.bin.sig");
    std::fs::write(&artifact_path, b"test artifact binary content").unwrap();

    let raw_key = [7u8; 32];
    let (sig_hex, pub_hex) =
        SignatureVerifier::sign_ed25519(b"test artifact binary content", &raw_key);
    std::fs::write(&sig_path, &sig_hex).unwrap();

    let loaded_bytes = std::fs::read(&artifact_path).unwrap();
    let loaded_sig = std::fs::read_to_string(&sig_path).unwrap();

    let verified = SignatureVerifier::verify_ed25519(&loaded_bytes, &loaded_sig, &pub_hex).unwrap();
    assert!(verified);
}

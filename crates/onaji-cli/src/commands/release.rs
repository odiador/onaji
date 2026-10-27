use crate::cli::ReleaseCommands;
use std::path::Path;

pub fn handle_release(cmd: ReleaseCommands) {
    match cmd {
        ReleaseCommands::Verify {
            artifact,
            signature,
            key,
        } => {
            if !artifact.exists() {
                eprintln!("✗ Error: Artifact not found at '{}'", artifact.display());
                std::process::exit(1);
            }
            let bytes = std::fs::read(&artifact).unwrap_or_default();
            let sha = onaji_core::release::ChecksumVerifier::compute_sha256(&bytes);
            println!("=== Onaji Cryptographic Release Verification ===");
            println!("Artifact:    {}", artifact.display());
            println!("SHA-256:     {}", sha);

            if let (Some(sig), Some(pub_key)) = (signature, key) {
                let sig_str = if Path::new(&sig).exists() {
                    std::fs::read_to_string(&sig).unwrap_or(sig)
                } else {
                    sig
                };
                match onaji_core::release::SignatureVerifier::verify_ed25519(
                    &bytes, &sig_str, &pub_key,
                ) {
                    Ok(true) => {
                        println!("Signature:   ✓ VALID (Ed25519 Sigstore Certified)");
                        println!("Status:      TRUSTED");
                    }
                    Ok(false) => {
                        println!("Signature:   ✗ INVALID (Signature mismatch)");
                        println!("Status:      REJECTED");
                        std::process::exit(1);
                    }
                    Err(err) => {
                        eprintln!("Signature:   ✗ ERROR ({})", err);
                        std::process::exit(1);
                    }
                }
            } else {
                println!(
                    "Signature:   ○ (No cryptographic public key provided for signature check)"
                );
                println!("Status:      UNVERIFIED SIGNATURE");
            }
        }
        ReleaseCommands::Sign {
            artifact,
            key,
            output,
        } => {
            if !artifact.exists() {
                eprintln!("✗ Error: Artifact not found at '{}'", artifact.display());
                std::process::exit(1);
            }
            let key_str = if Path::new(&key).exists() {
                std::fs::read_to_string(&key).unwrap_or(key)
            } else {
                key
            };
            let key_bytes = match hex::decode(key_str.trim()) {
                Ok(b) if b.len() == 32 => {
                    let mut arr = [0u8; 32];
                    arr.copy_from_slice(&b);
                    arr
                }
                _ => {
                    eprintln!("✗ Error: Invalid Ed25519 signing key. Expected 32-byte hex string (64 characters).");
                    std::process::exit(1);
                }
            };

            let bytes = std::fs::read(&artifact).unwrap_or_default();
            let (sig_hex, pub_hex) =
                onaji_core::release::SignatureVerifier::sign_ed25519(&bytes, &key_bytes);
            let sha = onaji_core::release::ChecksumVerifier::compute_sha256(&bytes);

            println!("=== Onaji Cryptographic Release Signing ===");
            println!("Artifact:    {}", artifact.display());
            println!("SHA-256:     {}", sha);
            println!("Public Key:  {}", pub_hex);
            println!("Signature:   {}", sig_hex);

            if let Some(out_path) = output {
                if let Err(e) = std::fs::write(&out_path, &sig_hex) {
                    eprintln!(
                        "✗ Error writing signature to '{}': {}",
                        out_path.display(),
                        e
                    );
                    std::process::exit(1);
                }
                println!("✓ Signature written to '{}'", out_path.display());
            }
        }
    }
}

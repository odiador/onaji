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
    }
}

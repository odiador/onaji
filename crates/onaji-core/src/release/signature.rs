use super::manifest::ArtifactSignature;
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SignatureError {
    #[error("Invalid hex encoding: {0}")]
    HexDecode(#[from] hex::FromHexError),
    #[error("Invalid public key format")]
    InvalidPublicKey,
    #[error("Invalid signature format")]
    InvalidSignature,
}

pub struct SignatureVerifier;

impl SignatureVerifier {
    /// Verify an Ed25519 signature in hex format against a message and public key
    pub fn verify_ed25519(
        message: &[u8],
        signature_hex: &str,
        public_key_hex: &str,
    ) -> Result<bool, SignatureError> {
        let pub_bytes = hex::decode(public_key_hex.trim())?;
        let pub_array: [u8; 32] = pub_bytes
            .try_into()
            .map_err(|_| SignatureError::InvalidPublicKey)?;
        let verifying_key =
            VerifyingKey::from_bytes(&pub_array).map_err(|_| SignatureError::InvalidPublicKey)?;

        let sig_bytes = hex::decode(signature_hex.trim())?;
        let sig_array: [u8; 64] = sig_bytes
            .try_into()
            .map_err(|_| SignatureError::InvalidSignature)?;
        let signature = Signature::from_bytes(&sig_array);

        Ok(verifying_key.verify(message, &signature).is_ok())
    }

    /// Sign a message using a raw 32-byte secret key and return (signature_hex, public_key_hex)
    pub fn sign_ed25519(message: &[u8], signing_key_bytes: &[u8; 32]) -> (String, String) {
        let signing_key = SigningKey::from_bytes(signing_key_bytes);
        let signature = signing_key.sign(message);
        let verifying_key = signing_key.verifying_key();

        (
            hex::encode(signature.to_bytes()),
            hex::encode(verifying_key.to_bytes()),
        )
    }

    /// Verify an artifact's cryptographic signature against its binary payload
    pub fn verify_artifact(
        artifact_bytes: &[u8],
        signature: &ArtifactSignature,
        public_key_hex: &str,
    ) -> bool {
        Self::verify_ed25519(artifact_bytes, &signature.signature_hex, public_key_hex)
            .unwrap_or(false)
    }
}

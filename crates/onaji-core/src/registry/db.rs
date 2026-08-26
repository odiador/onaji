use super::models::{PublishRequest, PublishResponse, RegistryPluginMeta, RegistryVersionMeta};
use super::storage::ContentAddressedStorage;
use crate::release::{ChecksumVerifier, SignatureVerifier};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RegistryError {
    #[error("Plugin version '{0}' already exists and is immutable")]
    DuplicateVersion(String),
    #[error("Declared SHA-256 does not match artifact payload")]
    ChecksumMismatch,
    #[error("Cryptographic signature verification failed")]
    InvalidSignature,
    #[error("Incompatible protocol version: {0}")]
    IncompatibleProtocol(String),
    #[error("Invalid base64 payload: {0}")]
    InvalidPayload(String),
}

#[derive(Clone, Default)]
pub struct RegistryDatabase {
    plugins: Arc<RwLock<HashMap<String, RegistryPluginMeta>>>,
}

impl RegistryDatabase {
    pub fn new() -> Self {
        Self {
            plugins: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn publish(
        &self,
        req: PublishRequest,
        storage: &ContentAddressedStorage,
    ) -> Result<PublishResponse, RegistryError> {
        // 1. Verify Protocol Compatibility
        if req.protocol_version != onaji_protocol::PROTOCOL_VERSION {
            return Err(RegistryError::IncompatibleProtocol(req.protocol_version));
        }

        // 2. Decode Payload
        let payload_bytes = hex::decode(req.artifact_base64.trim())
            .or_else(|_| {
                // If not hex, try raw bytes string
                Ok::<Vec<u8>, hex::FromHexError>(req.artifact_base64.as_bytes().to_vec())
            })
            .map_err(|e| RegistryError::InvalidPayload(e.to_string()))?;

        // 3. Verify SHA-256 Hash
        if !ChecksumVerifier::verify(&payload_bytes, &req.sha256) {
            return Err(RegistryError::ChecksumMismatch);
        }

        // 4. Verify Cryptographic Signature
        let sig_valid =
            SignatureVerifier::verify_ed25519(&payload_bytes, &req.signature_hex, &req.public_key)
                .unwrap_or(false);
        if !sig_valid {
            return Err(RegistryError::InvalidSignature);
        }

        // 5. Store in Content Addressed Storage
        let content_address = storage.store(&payload_bytes);

        // 6. Check Duplicate Version and Insert
        let mut map = self.plugins.write().unwrap();
        let plugin_entry = map
            .entry(req.name.clone())
            .or_insert_with(|| RegistryPluginMeta {
                name: req.name.clone(),
                description: req.description.clone(),
                author: req.author.clone(),
                latest_version: req.version.clone(),
                versions: HashMap::new(),
            });

        if plugin_entry.versions.contains_key(&req.version) {
            return Err(RegistryError::DuplicateVersion(req.version));
        }

        let version_meta = RegistryVersionMeta {
            version: req.version.clone(),
            protocol_version: req.protocol_version,
            runtime: req.runtime,
            sha256: req.sha256,
            signature_hex: req.signature_hex,
            public_key: req.public_key,
            content_address: content_address.clone(),
            size_bytes: payload_bytes.len() as u64,
            published_at: "2026-08-26T11:45:00Z".to_string(),
        };

        plugin_entry.latest_version = req.version.clone();
        plugin_entry
            .versions
            .insert(req.version.clone(), version_meta);

        Ok(PublishResponse {
            status: "published".to_string(),
            name: req.name,
            version: req.version,
            content_address,
            message: "Plugin successfully verified and registered into CAS".to_string(),
        })
    }

    pub fn get_plugin(&self, name: &str) -> Option<RegistryPluginMeta> {
        let map = self.plugins.read().unwrap();
        map.get(name).cloned()
    }

    pub fn get_version(&self, name: &str, version: &str) -> Option<RegistryVersionMeta> {
        let map = self.plugins.read().unwrap();
        map.get(name).and_then(|p| p.versions.get(version).cloned())
    }

    pub fn search(&self, query: &str) -> Vec<RegistryPluginMeta> {
        let map = self.plugins.read().unwrap();
        let q = query.to_lowercase();
        map.values()
            .filter(|p| {
                p.name.to_lowercase().contains(&q) || p.description.to_lowercase().contains(&q)
            })
            .cloned()
            .collect()
    }
}

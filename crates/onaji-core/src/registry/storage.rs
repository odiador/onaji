use crate::release::ChecksumVerifier;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

#[derive(Clone, Default)]
pub struct ContentAddressedStorage {
    objects: Arc<RwLock<HashMap<String, Vec<u8>>>>,
}

impl ContentAddressedStorage {
    pub fn new() -> Self {
        Self {
            objects: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Stores raw artifact bytes and returns its canonical content address (SHA-256)
    pub fn store(&self, bytes: &[u8]) -> String {
        let sha256 = ChecksumVerifier::compute_sha256(bytes);
        let mut map = self.objects.write().unwrap();
        map.insert(sha256.clone(), bytes.to_vec());
        sha256
    }

    /// Retrieves an artifact by its content address (SHA-256)
    pub fn get(&self, content_address: &str) -> Option<Vec<u8>> {
        let map = self.objects.read().unwrap();
        map.get(content_address).cloned()
    }
}

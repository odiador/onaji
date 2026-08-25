use super::backend::{CacheBackend, CacheError};
use super::entry::CacheEntry;
use super::key::CacheKey;
use std::sync::Arc;

pub struct CacheCoordinator {
    local_backend: Arc<dyn CacheBackend>,
    remote_backend: Option<Arc<dyn CacheBackend>>,
}

impl CacheCoordinator {
    pub fn new(
        local_backend: Arc<dyn CacheBackend>,
        remote_backend: Option<Arc<dyn CacheBackend>>,
    ) -> Self {
        Self {
            local_backend,
            remote_backend,
        }
    }

    pub fn local_only(local_backend: Arc<dyn CacheBackend>) -> Self {
        Self {
            local_backend,
            remote_backend: None,
        }
    }

    pub async fn get(&self, key: &CacheKey) -> Result<Option<CacheEntry>, CacheError> {
        // 1. Tier 1: Local Cache Check
        if let Some(entry) = self.local_backend.get(key).await? {
            return Ok(Some(entry));
        }

        // 2. Tier 2: Remote Cache Check (if configured)
        if let Some(remote) = &self.remote_backend {
            if let Some(entry) = remote.get(key).await? {
                // Populate local cache from remote for future hits
                self.local_backend.put(&entry).await.ok();
                return Ok(Some(entry));
            }
        }

        Ok(None)
    }

    pub async fn put(&self, entry: &CacheEntry) -> Result<(), CacheError> {
        // 1. Write to local cache
        self.local_backend.put(entry).await?;

        // 2. Write to remote cache (if configured)
        if let Some(remote) = &self.remote_backend {
            remote.put(entry).await.ok();
        }

        Ok(())
    }

    pub async fn has(&self, key: &CacheKey) -> Result<bool, CacheError> {
        if self.local_backend.has(key).await? {
            return Ok(true);
        }

        if let Some(remote) = &self.remote_backend {
            return remote.has(key).await;
        }

        Ok(false)
    }
}

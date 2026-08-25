use super::entry::CacheEntry;
use super::key::CacheKey;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::RwLock;

#[derive(Error, Debug)]
pub enum CacheError {
    #[error("I/O error in cache backend: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization/Deserialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("HTTP remote cache error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("Remote cache returned status code {status}: {message}")]
    RemoteStatus { status: u16, message: String },
}

#[async_trait::async_trait]
pub trait CacheBackend: Send + Sync {
    async fn has(&self, key: &CacheKey) -> Result<bool, CacheError>;
    async fn get(&self, key: &CacheKey) -> Result<Option<CacheEntry>, CacheError>;
    async fn put(&self, entry: &CacheEntry) -> Result<(), CacheError>;
    async fn delete(&self, key: &CacheKey) -> Result<bool, CacheError>;
}

// ---------------------------------------------------------------------------
// 1. In-Memory Backend (High-speed unit tests)
// ---------------------------------------------------------------------------
#[derive(Debug, Default, Clone)]
pub struct InMemoryCacheBackend {
    entries: Arc<RwLock<HashMap<CacheKey, CacheEntry>>>,
}

impl InMemoryCacheBackend {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait::async_trait]
impl CacheBackend for InMemoryCacheBackend {
    async fn has(&self, key: &CacheKey) -> Result<bool, CacheError> {
        let lock = self.entries.read().await;
        Ok(lock.contains_key(key))
    }

    async fn get(&self, key: &CacheKey) -> Result<Option<CacheEntry>, CacheError> {
        let lock = self.entries.read().await;
        Ok(lock.get(key).cloned())
    }

    async fn put(&self, entry: &CacheEntry) -> Result<(), CacheError> {
        let mut lock = self.entries.write().await;
        lock.insert(entry.key.clone(), entry.clone());
        Ok(())
    }

    async fn delete(&self, key: &CacheKey) -> Result<bool, CacheError> {
        let mut lock = self.entries.write().await;
        Ok(lock.remove(key).is_some())
    }
}

// ---------------------------------------------------------------------------
// 2. Local Disk Cache Backend (.onaji/cache/entries/<key>.json)
// ---------------------------------------------------------------------------
#[derive(Debug, Clone)]
pub struct LocalDiskCacheBackend {
    root_dir: PathBuf,
}

impl LocalDiskCacheBackend {
    pub fn new(root_dir: impl Into<PathBuf>) -> Self {
        let root = root_dir.into();
        std::fs::create_dir_all(&root).ok();
        Self { root_dir: root }
    }

    pub fn default_for_workspace(workspace_root: &Path) -> Self {
        Self::new(workspace_root.join(".onaji").join("cache").join("entries"))
    }

    fn entry_path(&self, key: &CacheKey) -> PathBuf {
        self.root_dir.join(format!("{}.json", key.as_str()))
    }
}

#[async_trait::async_trait]
impl CacheBackend for LocalDiskCacheBackend {
    async fn has(&self, key: &CacheKey) -> Result<bool, CacheError> {
        let path = self.entry_path(key);
        Ok(path.exists())
    }

    async fn get(&self, key: &CacheKey) -> Result<Option<CacheEntry>, CacheError> {
        let path = self.entry_path(key);
        if !path.exists() {
            return Ok(None);
        }
        let content = tokio::fs::read_to_string(&path).await?;
        let entry: CacheEntry = serde_json::from_str(&content)?;
        Ok(Some(entry))
    }

    async fn put(&self, entry: &CacheEntry) -> Result<(), CacheError> {
        tokio::fs::create_dir_all(&self.root_dir).await?;
        let path = self.entry_path(&entry.key);
        let content = serde_json::to_string_pretty(entry)?;
        tokio::fs::write(&path, content).await?;
        Ok(())
    }

    async fn delete(&self, key: &CacheKey) -> Result<bool, CacheError> {
        let path = self.entry_path(key);
        if path.exists() {
            tokio::fs::remove_file(&path).await?;
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

// ---------------------------------------------------------------------------
// 3. HTTP Remote Cache Backend (REST / S3 compatible)
// ---------------------------------------------------------------------------
#[derive(Debug, Clone)]
pub struct HttpRemoteCacheBackend {
    endpoint_url: String,
    auth_token: Option<String>,
    client: reqwest::Client,
}

impl HttpRemoteCacheBackend {
    pub fn new(endpoint_url: impl Into<String>, auth_token: Option<String>) -> Self {
        Self {
            endpoint_url: endpoint_url.into().trim_end_matches('/').to_string(),
            auth_token,
            client: reqwest::Client::builder().build().unwrap(),
        }
    }

    fn key_url(&self, key: &CacheKey) -> String {
        format!("{}/cache/{}", self.endpoint_url, key.as_str())
    }
}

#[async_trait::async_trait]
impl CacheBackend for HttpRemoteCacheBackend {
    async fn has(&self, key: &CacheKey) -> Result<bool, CacheError> {
        let mut req = self.client.head(self.key_url(key));
        if let Some(token) = &self.auth_token {
            req = req.bearer_auth(token);
        }

        let resp = req.send().await?;
        Ok(resp.status().is_success())
    }

    async fn get(&self, key: &CacheKey) -> Result<Option<CacheEntry>, CacheError> {
        let mut req = self.client.get(self.key_url(key));
        if let Some(token) = &self.auth_token {
            req = req.bearer_auth(token);
        }

        let resp = req.send().await?;
        if resp.status().as_u16() == 404 {
            return Ok(None);
        }

        if !resp.status().is_success() {
            return Err(CacheError::RemoteStatus {
                status: resp.status().as_u16(),
                message: resp.text().await.unwrap_or_default(),
            });
        }

        let entry: CacheEntry = resp.json().await?;
        Ok(Some(entry))
    }

    async fn put(&self, entry: &CacheEntry) -> Result<(), CacheError> {
        let mut req = self.client.put(self.key_url(&entry.key)).json(entry);
        if let Some(token) = &self.auth_token {
            req = req.bearer_auth(token);
        }

        let resp = req.send().await?;
        if !resp.status().is_success() {
            return Err(CacheError::RemoteStatus {
                status: resp.status().as_u16(),
                message: resp.text().await.unwrap_or_default(),
            });
        }

        Ok(())
    }

    async fn delete(&self, key: &CacheKey) -> Result<bool, CacheError> {
        let mut req = self.client.delete(self.key_url(key));
        if let Some(token) = &self.auth_token {
            req = req.bearer_auth(token);
        }

        let resp = req.send().await?;
        Ok(resp.status().is_success())
    }
}

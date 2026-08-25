use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum RegistryError {
    #[error("HTTP error communicating with plugin registry: {0}")]
    Http(#[from] reqwest::Error),

    #[error("JSON serialization error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Registry error ({status}): {message}")]
    RemoteStatus { status: u16, message: String },

    #[error("Plugin '{0}' was not found in registry")]
    NotFound(String),

    #[error("Integrity error: Downloaded archive SHA-256 '{actual}' does not match manifest hash '{expected}'")]
    ChecksumMismatch { expected: String, actual: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginRelease {
    pub name: String,
    pub version: String,
    pub protocol_version: String,
    pub capabilities: Vec<String>,
    pub sha256: String,
    pub download_url: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginPackageInfo {
    pub name: String,
    pub description: String,
    pub latest_version: String,
    pub versions: Vec<PluginRelease>,
}

#[derive(Debug, Clone)]
pub struct PluginRegistryClient {
    registry_url: String,
    auth_token: Option<String>,
    client: reqwest::Client,
}

impl PluginRegistryClient {
    pub const DEFAULT_REGISTRY: &'static str = "https://registry.onaji.dev";

    pub fn new(registry_url: impl Into<String>, auth_token: Option<String>) -> Self {
        Self {
            registry_url: registry_url.into().trim_end_matches('/').to_string(),
            auth_token,
            client: reqwest::Client::builder().build().unwrap(),
        }
    }

    pub fn default_client() -> Self {
        Self::new(Self::DEFAULT_REGISTRY, None)
    }

    pub async fn search(&self, query: &str) -> Result<Vec<PluginPackageInfo>, RegistryError> {
        let url = format!("{}/v1/plugins?q={}", self.registry_url, query);
        let mut req = self.client.get(&url);
        if let Some(token) = &self.auth_token {
            req = req.bearer_auth(token);
        }

        let resp = req.send().await?;
        if !resp.status().is_success() {
            return Err(RegistryError::RemoteStatus {
                status: resp.status().as_u16(),
                message: resp.text().await.unwrap_or_default(),
            });
        }

        let list: Vec<PluginPackageInfo> = resp.json().await?;
        Ok(list)
    }

    pub async fn get_info(&self, name: &str) -> Result<PluginPackageInfo, RegistryError> {
        let url = format!("{}/v1/plugins/{}", self.registry_url, name);
        let mut req = self.client.get(&url);
        if let Some(token) = &self.auth_token {
            req = req.bearer_auth(token);
        }

        let resp = req.send().await?;
        if resp.status().as_u16() == 404 {
            return Err(RegistryError::NotFound(name.to_string()));
        }

        if !resp.status().is_success() {
            return Err(RegistryError::RemoteStatus {
                status: resp.status().as_u16(),
                message: resp.text().await.unwrap_or_default(),
            });
        }

        let info: PluginPackageInfo = resp.json().await?;
        Ok(info)
    }

    pub async fn get_release(
        &self,
        name: &str,
        version: &str,
    ) -> Result<PluginRelease, RegistryError> {
        let url = format!("{}/v1/plugins/{}/{}", self.registry_url, name, version);
        let mut req = self.client.get(&url);
        if let Some(token) = &self.auth_token {
            req = req.bearer_auth(token);
        }

        let resp = req.send().await?;
        if resp.status().as_u16() == 404 {
            return Err(RegistryError::NotFound(format!("{}@{}", name, version)));
        }

        if !resp.status().is_success() {
            return Err(RegistryError::RemoteStatus {
                status: resp.status().as_u16(),
                message: resp.text().await.unwrap_or_default(),
            });
        }

        let release: PluginRelease = resp.json().await?;
        Ok(release)
    }

    pub async fn download_and_verify(
        &self,
        release: &PluginRelease,
    ) -> Result<Vec<u8>, RegistryError> {
        let mut req = self.client.get(&release.download_url);
        if let Some(token) = &self.auth_token {
            req = req.bearer_auth(token);
        }

        let resp = req.send().await?;
        if !resp.status().is_success() {
            return Err(RegistryError::RemoteStatus {
                status: resp.status().as_u16(),
                message: resp.text().await.unwrap_or_default(),
            });
        }

        let bytes = resp.bytes().await?.to_vec();

        // Strict SHA-256 verification
        use sha2::{Digest, Sha256};
        let computed_digest = hex::encode(Sha256::digest(&bytes));
        if computed_digest != release.sha256 {
            return Err(RegistryError::ChecksumMismatch {
                expected: release.sha256.clone(),
                actual: computed_digest,
            });
        }

        Ok(bytes)
    }

    pub async fn publish(
        &self,
        release: &PluginRelease,
        archive_bytes: &[u8],
    ) -> Result<(), RegistryError> {
        let url = format!("{}/v1/plugins", self.registry_url);
        let form = reqwest::multipart::Form::new()
            .text("manifest", serde_json::to_string(release)?)
            .part(
                "package",
                reqwest::multipart::Part::bytes(archive_bytes.to_vec())
                    .file_name(format!("{}-{}.tar.gz", release.name, release.version)),
            );

        let mut req = self.client.post(&url).multipart(form);
        if let Some(token) = &self.auth_token {
            req = req.bearer_auth(token);
        }

        let resp = req.send().await?;
        if !resp.status().is_success() {
            return Err(RegistryError::RemoteStatus {
                status: resp.status().as_u16(),
                message: resp.text().await.unwrap_or_default(),
            });
        }

        Ok(())
    }
}

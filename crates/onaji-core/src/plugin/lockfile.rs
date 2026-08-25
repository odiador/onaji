use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum LockfileError {
    #[error("I/O error reading or writing onaji.lock: {0}")]
    Io(#[from] std::io::Error),

    #[error("YAML error in onaji.lock: {0}")]
    Yaml(#[from] serde_yaml::Error),

    #[error("Integrity check failed for plugin '{plugin}': expected SHA-256 '{expected}', found '{actual}'")]
    ChecksumMismatch {
        plugin: String,
        expected: String,
        actual: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LockedPlugin {
    pub name: String,
    pub version: String,
    pub resolved: String,
    pub sha256: String,
    pub protocol: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OnajiLockfile {
    pub version: String,
    pub plugins: Vec<LockedPlugin>,
}

impl Default for OnajiLockfile {
    fn default() -> Self {
        Self {
            version: "1".to_string(),
            plugins: Vec::new(),
        }
    }
}

impl OnajiLockfile {
    pub fn load_from_dir(workspace_root: &Path) -> Result<Self, LockfileError> {
        let lockfile_path = Self::path_in_dir(workspace_root);
        if !lockfile_path.exists() {
            return Ok(Self::default());
        }

        let content = std::fs::read_to_string(&lockfile_path)?;
        let lockfile: Self = serde_yaml::from_str(&content)?;
        Ok(lockfile)
    }

    pub fn save_to_dir(&self, workspace_root: &Path) -> Result<(), LockfileError> {
        let lockfile_path = Self::path_in_dir(workspace_root);
        let content = serde_yaml::to_string(self)?;
        std::fs::write(&lockfile_path, content)?;
        Ok(())
    }

    pub fn path_in_dir(workspace_root: &Path) -> PathBuf {
        workspace_root.join("onaji.lock")
    }

    pub fn add_or_update(&mut self, locked: LockedPlugin) {
        if let Some(existing) = self.plugins.iter_mut().find(|p| p.name == locked.name) {
            *existing = locked;
        } else {
            self.plugins.push(locked);
        }
        self.plugins.sort_by(|a, b| a.name.cmp(&b.name));
    }

    pub fn remove(&mut self, plugin_name: &str) -> bool {
        let initial_len = self.plugins.len();
        self.plugins.retain(|p| p.name != plugin_name);
        self.plugins.len() < initial_len
    }

    pub fn find(&self, plugin_name: &str) -> Option<&LockedPlugin> {
        self.plugins.iter().find(|p| p.name == plugin_name)
    }

    pub fn verify_checksum(
        &self,
        plugin_name: &str,
        content_bytes: &[u8],
    ) -> Result<(), LockfileError> {
        if let Some(locked) = self.find(plugin_name) {
            use sha2::{Digest, Sha256};
            let digest = hex::encode(Sha256::digest(content_bytes));
            if digest != locked.sha256 {
                return Err(LockfileError::ChecksumMismatch {
                    plugin: plugin_name.to_string(),
                    expected: locked.sha256.clone(),
                    actual: digest,
                });
            }
        }
        Ok(())
    }
}

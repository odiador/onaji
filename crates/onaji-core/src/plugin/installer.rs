use super::lockfile::{LockedPlugin, OnajiLockfile};
use super::registry_client::{PluginRegistryClient, RegistryError};
use std::path::Path;

pub struct PluginInstaller;

impl PluginInstaller {
    pub async fn install(
        spec: &str,
        workspace_root: &Path,
        client: &PluginRegistryClient,
    ) -> Result<LockedPlugin, RegistryError> {
        let (name, version_req) = if let Some(last_at_idx) = spec.rfind('@') {
            if last_at_idx > 0 {
                (&spec[..last_at_idx], Some(&spec[last_at_idx + 1..]))
            } else {
                (spec, None)
            }
        } else {
            (spec, None)
        };

        // 1. Resolve Release from Registry
        let release = if let Some(ver) = version_req {
            client.get_release(name, ver).await?
        } else {
            let pkg_info = client.get_info(name).await?;
            client.get_release(name, &pkg_info.latest_version).await?
        };

        // 2. Download and Verify SHA-256
        let package_bytes = client.download_and_verify(&release).await?;

        // 3. Write plugin to local workspace store: .onaji/plugins/<name>/<version>/
        let safe_name = name.replace('/', "_");
        let plugin_dir = workspace_root
            .join(".onaji")
            .join("plugins")
            .join(&safe_name)
            .join(&release.version);
        std::fs::create_dir_all(&plugin_dir).map_err(|e| RegistryError::RemoteStatus {
            status: 500,
            message: format!("Failed to create plugin directory: {}", e),
        })?;

        let plugin_package_file = plugin_dir.join("package.bin");
        std::fs::write(&plugin_package_file, &package_bytes).map_err(|e| {
            RegistryError::RemoteStatus {
                status: 500,
                message: format!("Failed to write plugin package: {}", e),
            }
        })?;

        // 4. Update onaji.lock
        let locked = LockedPlugin {
            name: release.name.clone(),
            version: release.version.clone(),
            resolved: release.download_url.clone(),
            sha256: release.sha256.clone(),
            protocol: release.protocol_version.clone(),
        };

        let mut lockfile = OnajiLockfile::load_from_dir(workspace_root).unwrap_or_default();
        lockfile.add_or_update(locked.clone());
        lockfile
            .save_to_dir(workspace_root)
            .map_err(|e| RegistryError::RemoteStatus {
                status: 500,
                message: format!("Failed to save onaji.lock: {}", e),
            })?;

        Ok(locked)
    }

    pub fn uninstall(plugin_name: &str, workspace_root: &Path) -> Result<bool, RegistryError> {
        let mut lockfile = OnajiLockfile::load_from_dir(workspace_root).map_err(|e| {
            RegistryError::RemoteStatus {
                status: 500,
                message: format!("Failed to load onaji.lock: {}", e),
            }
        })?;

        let removed = lockfile.remove(plugin_name);
        if removed {
            lockfile
                .save_to_dir(workspace_root)
                .map_err(|e| RegistryError::RemoteStatus {
                    status: 500,
                    message: format!("Failed to save onaji.lock: {}", e),
                })?;

            let safe_name = plugin_name.replace('/', "_");
            let plugin_dir = workspace_root
                .join(".onaji")
                .join("plugins")
                .join(&safe_name);
            if plugin_dir.exists() {
                std::fs::remove_dir_all(&plugin_dir).ok();
            }
        }

        Ok(removed)
    }
}

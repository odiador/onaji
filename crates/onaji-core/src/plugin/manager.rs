use super::manifest::PluginManifest;
use super::subprocess::SubprocessPluginAdapter;
use crate::adapter::{AdapterError, AdapterRegistry};
use std::path::Path;

pub struct PluginManager {
    loaded_plugins: Vec<PluginManifest>,
}

impl PluginManager {
    pub fn new() -> Self {
        Self {
            loaded_plugins: Vec::new(),
        }
    }

    /// Registra un plugin externo por subproceso en el AdapterRegistry de Onaji
    pub fn register_subprocess_plugin(
        &mut self,
        manifest: PluginManifest,
        registry: &mut AdapterRegistry,
        workspace_root: &Path,
    ) -> Result<(), AdapterError> {
        let mut adapter = SubprocessPluginAdapter::new(manifest.clone());
        adapter.initialize(workspace_root)?;
        registry.register(Box::new(adapter));
        self.loaded_plugins.push(manifest);
        Ok(())
    }

    pub fn loaded_plugins(&self) -> &[PluginManifest] {
        &self.loaded_plugins
    }
}

impl Default for PluginManager {
    fn default() -> Self {
        Self::new()
    }
}

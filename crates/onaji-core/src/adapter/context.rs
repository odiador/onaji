use std::collections::HashMap;
use std::path::Path;

pub struct AdapterContext<'a> {
    pub workspace_root: &'a Path,
    pub resource_path: Option<&'a Path>,
    pub config: &'a HashMap<String, String>,
}

impl<'a> AdapterContext<'a> {
    pub fn new(workspace_root: &'a Path) -> Self {
        static EMPTY_CONFIG: std::sync::OnceLock<HashMap<String, String>> =
            std::sync::OnceLock::new();
        let empty = EMPTY_CONFIG.get_or_init(HashMap::new);

        Self {
            workspace_root,
            resource_path: None,
            config: empty,
        }
    }

    pub fn with_resource_path(mut self, path: &'a Path) -> Self {
        self.resource_path = Some(path);
        self
    }

    pub fn with_config(mut self, config: &'a HashMap<String, String>) -> Self {
        self.config = config;
        self
    }
}

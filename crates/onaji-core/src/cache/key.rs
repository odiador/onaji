use crate::domain::{Action, Task};
use crate::fingerprint::Fingerprint;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fmt;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CacheKey(pub String);

impl fmt::Display for CacheKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl CacheKey {
    pub fn new(hash_hex: impl Into<String>) -> Self {
        Self(hash_hex.into())
    }

    pub fn compute(
        task: &Task,
        workspace_root: &Path,
        input_fingerprints: &HashMap<String, Fingerprint>,
    ) -> Self {
        Self::compute_sha256(task, workspace_root, input_fingerprints)
    }

    pub fn compute_sha256(
        task: &Task,
        workspace_root: &Path,
        input_fingerprints: &HashMap<String, Fingerprint>,
    ) -> Self {
        let mut hasher = Sha256::new();

        // 1. Engine & Protocol Version
        hasher.update(env!("CARGO_PKG_VERSION").as_bytes());
        hasher.update(onaji_protocol::PROTOCOL_VERSION.as_bytes());

        // 2. Task ID
        hasher.update(task.id.as_str().as_bytes());

        // 3. Action Definition & Sorted Environment
        match &task.action {
            Action::Command {
                run,
                env,
                working_dir,
            } => {
                hasher.update(b"command:");
                hasher.update(run.as_bytes());
                if let Some(c) = working_dir {
                    hasher.update(c.to_string_lossy().as_bytes());
                }
                let mut sorted_env: Vec<(&String, &String)> = env.iter().collect();
                sorted_env.sort();
                for (k, v) in sorted_env {
                    hasher.update(k.as_bytes());
                    hasher.update(b"=");
                    hasher.update(v.as_bytes());
                    hasher.update(b";");
                }
            }
            Action::Builtin { adapter, config } => {
                hasher.update(b"builtin:");
                hasher.update(adapter.as_bytes());
                let mut sorted_cfg: Vec<(&String, &String)> = config.iter().collect();
                sorted_cfg.sort();
                for (k, v) in sorted_cfg {
                    hasher.update(k.as_bytes());
                    hasher.update(b"=");
                    hasher.update(v.as_bytes());
                    hasher.update(b";");
                }
            }
        }

        // 4. Sorted Inputs Fingerprints
        let mut sorted_inputs: Vec<(&String, &Fingerprint)> = input_fingerprints.iter().collect();
        sorted_inputs.sort_by_key(|(k, _)| *k);
        for (name, fp) in sorted_inputs {
            hasher.update(name.as_bytes());
            hasher.update(b":");
            hasher.update(fp.0.to_le_bytes());
            hasher.update(b",");
        }

        // 5. Sorted Cache Paths Fingerprints (e.g. build.gradle.kts, package.json)
        for cache_rel_path in &task.cache_paths {
            let full_path = workspace_root.join(cache_rel_path);
            if full_path.exists() {
                if let Ok(content) = std::fs::read(&full_path) {
                    hasher.update(cache_rel_path.to_string_lossy().as_bytes());
                    hasher.update(Sha256::digest(&content));
                }
            }
        }

        let result = hasher.finalize();
        Self(hex::encode(result))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

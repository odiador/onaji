use crate::cli::Cli;
use crate::util::resolve_workspace_root;
use clap::CommandFactory;
use onaji_core::cache::ExecutionCache;
use onaji_core::domain::TaskId;
use std::path::{Path, PathBuf};

pub fn handle_completions(shell: clap_complete::Shell) {
    let mut cmd = Cli::command();
    clap_complete::generate(shell, &mut cmd, "onaji", &mut std::io::stdout());
}

pub fn handle_clean(config_path: &Path, all: bool, task_id_filter: Option<String>) {
    let workspace_root = resolve_workspace_root(config_path);

    if let Some(task_id_str) = task_id_filter {
        let task_id = TaskId::new(task_id_str);
        let mut cache = ExecutionCache::load_or_new(&workspace_root);
        let removed_key = cache.invalidate(&task_id);
        let _ = cache.save(&workspace_root);

        let mut cas_freed = 0;
        if let Some(key) = removed_key {
            let cas_task_dir = workspace_root
                .join(".onaji")
                .join("cache")
                .join("cas")
                .join(&key.0);
            if cas_task_dir.exists() {
                cas_freed = dir_size(&cas_task_dir);
                let _ = std::fs::remove_dir_all(&cas_task_dir);
            }
        }

        if cas_freed > 0 {
            println!(
                "✓ Invalidated cache record and purged {} of CAS artifacts for task '{}'.",
                format_bytes(cas_freed),
                task_id
            );
        } else {
            println!(
                "✓ Invalidated cache record for task '{}' in workspace.",
                task_id
            );
        }
        return;
    }

    let local_cache = workspace_root.join(".onaji").join("cache");
    if local_cache.exists() {
        let size = dir_size(&local_cache);
        let _ = std::fs::remove_dir_all(&local_cache);
        println!(
            "✓ Purged {} of workspace cache at '{}'",
            format_bytes(size),
            local_cache.display()
        );
    } else {
        println!("○ No local cache found at '{}'", local_cache.display());
    }

    if all {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let global_cache = PathBuf::from(home).join(".onaji").join("cache");
        if global_cache.exists() {
            let size = dir_size(&global_cache);
            let _ = std::fs::remove_dir_all(&global_cache);
            println!(
                "✓ Purged {} of global cache at '{}'",
                format_bytes(size),
                global_cache.display()
            );
        }
    }
}

fn dir_size(path: &Path) -> u64 {
    let mut total = 0;
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            if let Ok(meta) = entry.metadata() {
                if meta.is_dir() {
                    total += dir_size(&entry.path());
                } else {
                    total += meta.len();
                }
            }
        }
    }
    total
}

fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;
    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

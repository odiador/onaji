use crate::cli::Cli;
use crate::util::{dir_size, format_bytes, parse_size, resolve_workspace_root};
use clap::CommandFactory;
use onaji_core::cache::ExecutionCache;
use onaji_core::domain::TaskId;
use std::path::{Path, PathBuf};

pub fn handle_completions(shell: clap_complete::Shell) {
    let mut cmd = Cli::command();
    clap_complete::generate(shell, &mut cmd, "onaji", &mut std::io::stdout());
}

pub fn handle_clean(
    config_path: &Path,
    all: bool,
    traces: bool,
    task_id_filter: Option<String>,
    max_size_limit: Option<String>,
) {
    let workspace_root = resolve_workspace_root(config_path);

    if traces || all {
        let traces_dir = workspace_root.join(".onaji").join("traces");
        if traces_dir.exists() {
            let size = dir_size(&traces_dir);
            let _ = std::fs::remove_dir_all(&traces_dir);
            println!(
                "✓ Purged {} of execution traces at '{}'",
                format_bytes(size),
                traces_dir.display()
            );
        } else if traces && !all {
            println!("○ No execution traces found at '{}'", traces_dir.display());
        }
        if traces && !all && task_id_filter.is_none() && max_size_limit.is_none() {
            return;
        }
    }

    if let Some(limit_str) = max_size_limit {
        let max_bytes = match parse_size(&limit_str) {
            Some(b) => b,
            None => {
                eprintln!(
                    "✗ Invalid size format: '{}'. Examples: '500MB', '2GB', '100KB'.",
                    limit_str
                );
                std::process::exit(1);
            }
        };

        match ExecutionCache::prune_cas(&workspace_root, max_bytes) {
            Ok((evicted_count, evicted_bytes)) if evicted_count > 0 => {
                let remaining = dir_size(&workspace_root.join(".onaji").join("cache").join("cas"));
                println!(
                    "✓ Pruned {} across {} old CAS artifacts to meet quota of {} (remaining: {}).",
                    format_bytes(evicted_bytes),
                    evicted_count,
                    format_bytes(max_bytes),
                    format_bytes(remaining)
                );
            }
            Ok(_) => {
                let current = dir_size(&workspace_root.join(".onaji").join("cache").join("cas"));
                println!(
                    "○ CAS storage is {} (under quota limit of {}). No eviction needed.",
                    format_bytes(current),
                    format_bytes(max_bytes)
                );
            }
            Err(err) => {
                eprintln!("✗ Failed to prune CAS store: {}", err);
            }
        }
        return;
    }

    if let Some(task_id_str) = task_id_filter {
        let task_id = TaskId::new(task_id_str);
        let mut cache = ExecutionCache::load_or_new(&workspace_root);
        let removed_key = cache.invalidate(&task_id);
        let _ = cache.save(&workspace_root);

        let mut cas_freed = 0;
        if let Some(key) = removed_key {
            let cas_base = workspace_root.join(".onaji").join("cache").join("cas");
            let archive_file = cas_base.join(format!("{}.tar.zst", key.0));
            if archive_file.is_file() {
                if let Ok(m) = archive_file.metadata() {
                    cas_freed += m.len();
                }
                let _ = std::fs::remove_file(&archive_file);
            }
            let cas_task_dir = cas_base.join(&key.0);
            if cas_task_dir.is_dir() {
                cas_freed += dir_size(&cas_task_dir);
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

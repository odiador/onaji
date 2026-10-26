use crate::cli::Cli;
use crate::util::resolve_workspace_root;
use clap::CommandFactory;
use std::path::{Path, PathBuf};

pub fn handle_completions(shell: clap_complete::Shell) {
    let mut cmd = Cli::command();
    clap_complete::generate(shell, &mut cmd, "onaji", &mut std::io::stdout());
}

pub fn handle_clean(config_path: &Path, all: bool) {
    let workspace_root = resolve_workspace_root(config_path);
    let local_cache = workspace_root.join(".onaji").join("cache");
    if local_cache.exists() {
        let _ = std::fs::remove_dir_all(&local_cache);
        println!("✓ Purged workspace cache at '{}'", local_cache.display());
    } else {
        println!("○ No local cache found at '{}'", local_cache.display());
    }

    if all {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let global_cache = PathBuf::from(home).join(".onaji").join("cache");
        if global_cache.exists() {
            let _ = std::fs::remove_dir_all(&global_cache);
            println!("✓ Purged global cache at '{}'", global_cache.display());
        }
    }
}

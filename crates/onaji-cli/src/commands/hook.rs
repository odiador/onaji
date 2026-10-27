use crate::cli::HookCommands;
use crate::util::resolve_workspace_root;
use std::path::Path;

pub fn handle_hook(config_path: &Path, cmd: HookCommands) {
    let workspace_root = resolve_workspace_root(config_path);
    let git_dir = workspace_root.join(".git");

    if !git_dir.exists() {
        eprintln!(
            "✗ Error: '{}' is not a git repository root.",
            workspace_root.display()
        );
        std::process::exit(1);
    }

    let git_hooks_dir = git_dir.join("hooks");
    let pre_commit_path = git_hooks_dir.join("pre-commit");

    match cmd {
        HookCommands::Install => {
            if let Err(e) = std::fs::create_dir_all(&git_hooks_dir) {
                eprintln!("✗ Failed to create git hooks directory: {}", e);
                std::process::exit(1);
            }

            let hook_script = r#"#!/usr/bin/env sh
# ONΛJI Automated Pre-Commit Selective Testing Hook
if command -v onaji >/dev/null 2>&1; then
    ONAJI_BIN="onaji"
elif [ -f "$HOME/.cargo/bin/onaji" ]; then
    ONAJI_BIN="$HOME/.cargo/bin/onaji"
elif [ -f "./target/release/onaji" ]; then
    ONAJI_BIN="./target/release/onaji"
elif [ -f "./target/debug/onaji" ]; then
    ONAJI_BIN="./target/debug/onaji"
else
    ONAJI_BIN="cargo run -q -p onaji-cli --"
fi

echo "⚡ [onaji-hook] Running selective impact tests for staged/uncommitted changes..."
$ONAJI_BIN affected --since HEAD --uncommitted --run --tests
STATUS=$?
if [ $STATUS -ne 0 ]; then
    echo "✗ [onaji-hook] Pre-commit validation failed. Fix failing tasks before committing."
    exit $STATUS
fi
exit 0
"#;
            if let Err(e) = std::fs::write(&pre_commit_path, hook_script) {
                eprintln!("✗ Failed to write pre-commit hook: {}", e);
                std::process::exit(1);
            }

            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let perms = std::fs::Permissions::from_mode(0o755);
                let _ = std::fs::set_permissions(&pre_commit_path, perms);
            }

            println!(
                "✓ Installed Onaji Git pre-commit hook at '{}'",
                pre_commit_path.display()
            );
            println!("  Only affected tests and causal prerequisites will run before git commits.");
        }
        HookCommands::Uninstall => {
            if pre_commit_path.exists() {
                let _ = std::fs::remove_file(&pre_commit_path);
                println!("✓ Uninstalled Onaji Git pre-commit hook.");
            } else {
                println!(
                    "○ No pre-commit hook found at '{}'.",
                    pre_commit_path.display()
                );
            }
        }
    }
}

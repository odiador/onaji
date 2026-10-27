use crate::util::{dir_size, format_bytes, resolve_workspace_root};
use onaji_core::config::ConfigLoader;
use std::path::Path;
use std::process::Command;

pub fn handle_doctor(config_path: &Path) {
    println!("=== Onaji Doctor Diagnostics ===\n");
    println!("Core:");
    println!(
        "  ✓ Onaji Engine ............. v{}",
        env!("CARGO_PKG_VERSION")
    );
    println!(
        "  ✓ Protocol Version ......... v{}",
        onaji_protocol::PROTOCOL_VERSION
    );

    println!("\nToolchains:");
    check_tool("rustc", &["--version"]);
    check_tool("cargo", &["--version"]);
    check_tool("node", &["--version"]);
    check_tool("java", &["-version"]);
    check_tool("gradle", &["--version"]);

    // Check Android SDK
    let android_home = std::env::var("ANDROID_HOME").or_else(|_| std::env::var("ANDROID_SDK_ROOT"));
    match android_home {
        Ok(path) => println!("  ✓ Android SDK .............. {}", path),
        Err(_) => println!("  ○ Android SDK .............. (not set in environment)"),
    }

    // Check Container Runtime
    match Command::new("docker").arg("--version").output() {
        Ok(out) if out.status.success() => {
            let ver = String::from_utf8_lossy(&out.stdout)
                .lines()
                .next()
                .unwrap_or("")
                .trim()
                .to_string();
            println!("  ✓ Container Engine ......... docker ({})", ver);
        }
        _ => match Command::new("podman").arg("--version").output() {
            Ok(out) if out.status.success() => {
                let ver = String::from_utf8_lossy(&out.stdout)
                    .lines()
                    .next()
                    .unwrap_or("")
                    .trim()
                    .to_string();
                println!("  ✓ Container Engine ......... podman ({})", ver);
            }
            _ => {
                println!("  ○ Container Engine ......... (neither docker nor podman detected)");
            }
        },
    }

    println!("\nAdapters & Plugins:");
    let registry = onaji_core::adapter::AdapterRegistry::with_builtins();
    for adapter in registry.list() {
        let caps: Vec<String> = adapter
            .capabilities()
            .iter()
            .map(|c| c.to_string())
            .collect();
        println!("  ✓ [native] {:<18} [{}]", adapter.id(), caps.join(", "));
    }

    println!("\nWorkspace & Automation:");
    let workspace_root = resolve_workspace_root(config_path);

    // Git Hook status
    let git_hook_path = workspace_root.join(".git").join("hooks").join("pre-commit");
    if git_hook_path.exists() {
        if let Ok(hook_body) = std::fs::read_to_string(&git_hook_path) {
            if hook_body.contains("onaji") {
                println!("  ✓ Git Pre-commit Hook ...... installed (.git/hooks/pre-commit)");
            } else {
                println!("  ⚠ Git Pre-commit Hook ...... foreign script present (run `onaji hook install` to overwrite)");
            }
        } else {
            println!("  ✓ Git Pre-commit Hook ...... present");
        }
    } else if workspace_root.join(".git").exists() {
        println!("  ○ Git Pre-commit Hook ...... not installed (run `onaji hook install`)");
    } else {
        println!("  ○ Git Repository ........... none detected in current workspace");
    }

    // Storage and CAS status
    let cache_dir = workspace_root.join(".onaji").join("cache");
    let cache_file = cache_dir.join("execution_cache.json");
    if cache_file.exists() {
        let cache = onaji_core::cache::ExecutionCache::load_or_new(&workspace_root);
        println!(
            "  ✓ Execution Cache .......... {} cached task records",
            cache.len()
        );
    } else {
        println!("  ○ Execution Cache .......... empty");
    }

    let cas_dir = cache_dir.join("cas");
    if cas_dir.exists() {
        let size = dir_size(&cas_dir);
        let count = std::fs::read_dir(&cas_dir).map(|d| d.count()).unwrap_or(0);
        println!(
            "  ✓ CAS Storage .............. {} across {} artifact trees",
            format_bytes(size),
            count
        );
    } else {
        println!("  ○ CAS Storage .............. empty");
    }

    if config_path.exists() {
        match ConfigLoader::load_from_file(config_path) {
            Ok((manifest, dag)) => {
                println!("  ✓ Project Name ............. {}", manifest.name);
                println!(
                    "  ✓ Manifest ................. {} (valid syntax)",
                    config_path.display()
                );
                println!("  ✓ Dependency DAG ........... valid (no cycles, single producers)");
                println!(
                    "  ✓ Registered Resources ..... {}",
                    dag.resource_indices.len()
                );
                println!("  ✓ Tracked Tasks ............ {}", dag.task_indices.len());
                println!(
                    "  ✓ Monitored Artifacts ...... {}",
                    dag.artifact_indices.len()
                );
                println!("\nStatus: Ready for reactive orchestration.");
            }
            Err(err) => {
                println!(
                    "  ✗ {} ............. invalid: {}",
                    config_path.display(),
                    err
                );
            }
        }
    } else {
        println!(
            "  ⚠ {} ............. not found (run `onaji init` to generate)",
            config_path.display()
        );
    }
}

pub fn check_tool(bin: &str, args: &[&str]) {
    match Command::new(bin).args(args).output() {
        Ok(out) if out.status.success() => {
            let first_line = String::from_utf8_lossy(&out.stdout)
                .lines()
                .next()
                .unwrap_or("")
                .trim()
                .to_string();
            let summary = if first_line.is_empty() {
                String::from_utf8_lossy(&out.stderr)
                    .lines()
                    .next()
                    .unwrap_or("")
                    .trim()
                    .to_string()
            } else {
                first_line
            };
            println!("  ✓ {:<12} ..... installed ({})", bin, summary);
        }
        _ => {
            println!("  ⚠ {:<12} ..... not available / not in PATH", bin);
        }
    }
}

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

    println!("\nWorkspace:");
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

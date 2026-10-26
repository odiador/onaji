use crate::cli::WorkspaceCommands;
use crate::util::resolve_workspace_root;
use std::path::Path;

pub fn handle_workspace(config_path: &Path, cmd: WorkspaceCommands) {
    let workspace_root = resolve_workspace_root(config_path);

    // Try loading monorepo workspaces
    let root_content = match std::fs::read_to_string(config_path) {
        Ok(c) => c,
        Err(err) => {
            eprintln!(
                "✗ Error reading root manifest '{}': {}",
                config_path.display(),
                err
            );
            std::process::exit(1);
        }
    };

    let root_yaml: serde_yaml::Value =
        serde_yaml::from_str(&root_content).unwrap_or(serde_yaml::Value::Null);
    let workspace_dirs: Vec<String> = root_yaml
        .get("workspaces")
        .and_then(|w| w.as_sequence())
        .map(|seq| {
            seq.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();

    if workspace_dirs.is_empty() {
        println!("Single workspace project (no 'workspaces' declared in root onaji.yaml).");
        return;
    }

    let mut multi_graph = onaji_core::workspace::MultiWorkspaceGraph::new();

    for ws_dir_str in &workspace_dirs {
        let ws_root = workspace_root.join(ws_dir_str);
        let ws_manifest_path = ws_root.join("onaji.yaml");
        if ws_manifest_path.exists() {
            if let Ok((manifest, dag)) =
                onaji_core::config::ConfigLoader::load_from_file(&ws_manifest_path)
            {
                let ws_id = onaji_core::workspace::WorkspaceId::new(ws_dir_str.clone());
                let ws = onaji_core::workspace::Workspace {
                    id: ws_id,
                    root_dir: ws_root,
                    manifest_path: ws_manifest_path,
                    manifest,
                    dag,
                    dependencies: Vec::new(),
                };
                multi_graph.add_workspace(ws).ok();
            }
        }
    }

    match cmd {
        WorkspaceCommands::List => {
            println!(
                "=== Monorepo Workspaces ({}) ===",
                multi_graph.workspaces.len()
            );
            for (id, ws) in &multi_graph.workspaces {
                println!(
                    "  • {:<20} ({} tasks, {} resources) -> {}",
                    id,
                    ws.dag.task_indices.len(),
                    ws.dag.resource_indices.len(),
                    ws.root_dir.display()
                );
            }
        }
        WorkspaceCommands::Check => match multi_graph.connect_dependencies() {
            Ok(_) => {
                println!(
                    "✓ All {} workspaces and cross-workspace contracts are valid.",
                    multi_graph.workspaces.len()
                );
            }
            Err(err) => {
                eprintln!("✗ Cross-workspace validation error: {}", err);
                std::process::exit(2);
            }
        },
        WorkspaceCommands::Graph { global, workspace } => {
            if global || workspace.is_none() {
                println!("=== Global Monorepo Workspace Meta-Graph ===");
                for id in multi_graph.workspaces.keys() {
                    println!("  Workspace [{}]", id);
                }
            } else if let Some(ws_name) = workspace {
                let ws_id = onaji_core::workspace::WorkspaceId::new(ws_name);
                if let Some(ws) = multi_graph.workspaces.get(&ws_id) {
                    println!("=== Workspace [{}] DAG ===", ws_id);
                    println!("Tasks: {}", ws.dag.task_indices.len());
                    for t in ws.dag.task_indices.keys() {
                        println!("  - Task: {}", t);
                    }
                } else {
                    eprintln!("✗ Workspace '{}' not found.", ws_id);
                    std::process::exit(1);
                }
            }
        }
        WorkspaceCommands::Impact {
            workspace,
            resource,
        } => {
            let ws_id = onaji_core::workspace::WorkspaceId::new(workspace);
            let res_id = onaji_core::domain::ResourceId::new(resource);

            match multi_graph.analyze_impact(&ws_id, &res_id) {
                Some(report) => {
                    println!("=== Cross-Workspace Impact Analysis ===");
                    println!(
                        "Trigger: [{}] -> {}",
                        report.root_workspace, report.root_resource
                    );
                    println!("\nLocal Affected Tasks in [{}]:", report.root_workspace);
                    for t in report.local_affected_tasks {
                        println!("  ✓ {}", t);
                    }
                    if report.cross_workspace_impacts.is_empty() {
                        println!("\nNo cross-workspace downstream tasks affected (Workspace Isolation Preserved).");
                    } else {
                        println!("\nCross-Workspace Downstream Impacts:");
                        for cross in report.cross_workspace_impacts {
                            println!("  ↳ Workspace [{}]:", cross.workspace_id);
                            for t in cross.affected_tasks {
                                println!("      • {}", t);
                            }
                        }
                    }
                }
                None => {
                    eprintln!("✗ Resource or workspace not found in graph.");
                    std::process::exit(1);
                }
            }
        }
    }
}

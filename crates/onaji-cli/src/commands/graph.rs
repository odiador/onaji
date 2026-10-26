use crate::cli::GraphFormat;
use crate::util::sanitize_id;
use onaji_core::config::ConfigLoader;
use std::path::Path;

pub fn handle_graph(config_path: &Path, format: GraphFormat) {
    let (manifest, dag) = match ConfigLoader::load_from_file(config_path) {
        Ok(res) => res,
        Err(err) => {
            eprintln!("✗ Error loading '{}': {}", config_path.display(), err);
            std::process::exit(1);
        }
    };

    match format {
        GraphFormat::Text => {
            println!("=== Onaji Dependency Graph: {} ===", manifest.name);
            println!("Resources ({}):", dag.resource_indices.len());
            for res_id in dag.resource_indices.keys() {
                if let Some(res) = dag.get_resource(res_id) {
                    println!("  • {} ({:?})", res.id, res.resource_type);
                }
            }

            println!("\nTasks ({}):", dag.task_indices.len());
            for task_id in dag.task_indices.keys() {
                if let Some(task) = dag.get_task(task_id) {
                    let inputs: Vec<String> = task.inputs.iter().map(|i| i.to_string()).collect();
                    let outputs: Vec<String> = task
                        .outputs
                        .iter()
                        .map(|o| o.as_str().to_string())
                        .collect();
                    println!("  ┌── [Task: {}]", task.id);
                    println!("  │   Inputs:  [{}]", inputs.join(", "));
                    println!("  │   Outputs: [{}]", outputs.join(", "));
                    if let onaji_core::domain::Action::Command { run, .. } = &task.action {
                        println!("  │   Run:     {}", run);
                    }
                    println!("  └───");
                }
            }

            println!("\nArtifacts ({}):", dag.artifact_indices.len());
            for art_id in dag.artifact_indices.keys() {
                if let Some(art) = dag.get_artifact(art_id) {
                    println!("  ◆ {}", art.id);
                }
            }
        }
        GraphFormat::Mermaid => {
            println!("```mermaid");
            println!("graph TD");
            for res_id in dag.resource_indices.keys() {
                println!(
                    "    res_{}[\"📄 Resource: {}\"]",
                    sanitize_id(res_id.as_str()),
                    res_id
                );
            }
            for task_id in dag.task_indices.keys() {
                println!(
                    "    task_{}[\"⚙️ Task: {}\"]",
                    sanitize_id(task_id.as_str()),
                    task_id
                );
            }
            for art_id in dag.artifact_indices.keys() {
                println!(
                    "    art_{}[\"📦 Artifact: {}\"]",
                    sanitize_id(art_id.as_str()),
                    art_id
                );
            }

            for task_id in dag.task_indices.keys() {
                if let Some(task) = dag.get_task(task_id) {
                    let t_safe = sanitize_id(task.id.as_str());
                    for input in &task.inputs {
                        match input {
                            onaji_core::domain::InputNode::Resource(r) => {
                                println!("    res_{} --> task_{}", sanitize_id(r.as_str()), t_safe);
                            }
                            onaji_core::domain::InputNode::Artifact(a) => {
                                println!("    art_{} --> task_{}", sanitize_id(a.as_str()), t_safe);
                            }
                        }
                    }
                    for output in &task.outputs {
                        println!(
                            "    task_{} --> art_{}",
                            t_safe,
                            sanitize_id(output.as_str())
                        );
                    }
                }
            }
            println!("```");
        }
        GraphFormat::Json => {
            let json = serde_json::json!({
                "name": manifest.name,
                "version": manifest.version,
                "node_count": dag.node_count(),
                "edge_count": dag.edge_count(),
                "resources": dag.resource_indices.keys().map(|r| r.as_str()).collect::<Vec<_>>(),
                "artifacts": dag.artifact_indices.keys().map(|a| a.as_str()).collect::<Vec<_>>(),
                "tasks": dag.task_indices.keys().map(|t| t.as_str()).collect::<Vec<_>>(),
            });
            println!("{}", serde_json::to_string_pretty(&json).unwrap());
        }
    }
}

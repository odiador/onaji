use crate::cli::OutputFormat;
use onaji_core::config::ConfigLoader;
use std::path::Path;

pub fn handle_check(config_path: &Path, _ci: bool, output: OutputFormat) {
    match ConfigLoader::load_from_file(config_path) {
        Ok((manifest, dag)) => match output {
            OutputFormat::Github => {
                println!(
                    "::notice title=Onaji Check::Manifest '{}' and DAG ({} nodes, {} edges) are valid",
                    config_path.display(),
                    dag.node_count(),
                    dag.edge_count()
                );
            }
            OutputFormat::Json => {
                let json = serde_json::json!({
                    "valid": true,
                    "name": manifest.name,
                    "nodes": dag.node_count(),
                    "edges": dag.edge_count(),
                    "tasks": manifest.tasks.len()
                });
                println!("{}", serde_json::to_string_pretty(&json).unwrap());
            }
            OutputFormat::Text => {
                println!("✓ Manifest '{}' is valid.", config_path.display());
                println!(
                    "✓ Graph successfully built: {} nodes, {} edges across {} tasks.",
                    dag.node_count(),
                    dag.edge_count(),
                    manifest.tasks.len()
                );
            }
        },
        Err(err) => {
            match output {
                OutputFormat::Github => {
                    println!(
                        "::error title=Onaji Check::Manifest or DAG is invalid: {}",
                        err
                    );
                }
                OutputFormat::Json => {
                    let json = serde_json::json!({
                        "valid": false,
                        "error": err.to_string(),
                        "exit_code": 2
                    });
                    println!("{}", serde_json::to_string_pretty(&json).unwrap());
                }
                OutputFormat::Text => {
                    eprintln!("✗ Validation failed: {}", err);
                }
            }
            std::process::exit(2);
        }
    }
}

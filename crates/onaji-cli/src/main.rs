use clap::{Parser, Subcommand, ValueEnum};
use onaji_core::cache::{ExecutionCache, TaskState};
use onaji_core::config::ConfigLoader;
use onaji_core::domain::ResourceId;
use onaji_core::graph::ImpactAnalyzer;
use onaji_core::scheduler::TaskScheduler;
use onaji_core::watcher::ReactiveWatcher;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

#[derive(Parser)]
#[command(
    name = "onaji",
    about = "Universal reactive automation engine for software engineering workflows",
    version
)]
struct Cli {
    #[arg(
        short,
        long,
        default_value = "onaji.yaml",
        help = "Path to the configuration manifest"
    )]
    config: PathBuf,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the reactive daemon watcher loop
    Watch {
        #[arg(
            short,
            long,
            default_value = "250",
            help = "Debounce duration in milliseconds"
        )]
        debounce: u64,
    },

    /// Run tasks or trigger a pipeline manually
    Run {
        #[arg(
            short,
            long,
            help = "Optional resource ID to simulate change and run impact for"
        )]
        resource: Option<String>,
    },

    /// Inspect and render the Semantic Automation DAG
    Graph {
        #[arg(
            short,
            long,
            default_value = "text",
            help = "Output format: text, mermaid, or json"
        )]
        format: GraphFormat,
    },

    /// Run environment, toolchain, and manifest health checks
    Doctor,

    /// Validate the manifest syntax and verify DAG invariants
    Check,

    /// Preview which tasks and artifacts are affected by a resource change
    Impact {
        #[arg(short, long, help = "ID of the resource to simulate change for")]
        resource: String,
    },
}

#[derive(Clone, Copy, ValueEnum, PartialEq, Eq)]
enum GraphFormat {
    Text,
    Mermaid,
    Json,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Watch { debounce } => handle_watch(&cli.config, debounce).await,
        Commands::Run { resource } => handle_run(&cli.config, resource).await,
        Commands::Graph { format } => handle_graph(&cli.config, format),
        Commands::Doctor => handle_doctor(&cli.config),
        Commands::Check => handle_check(&cli.config),
        Commands::Impact { resource } => handle_impact(&cli.config, &resource),
    }
}

fn resolve_workspace_root(config_path: &Path) -> PathBuf {
    match config_path.parent() {
        Some(p) if !p.as_os_str().is_empty() => p.to_path_buf(),
        _ => std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
    }
}

async fn handle_watch(config_path: &Path, debounce_ms: u64) {
    let workspace_root = resolve_workspace_root(config_path);
    let (manifest, dag) = match ConfigLoader::load_from_file(config_path) {
        Ok(res) => res,
        Err(err) => {
            eprintln!("✗ Error loading '{}': {}", config_path.display(), err);
            std::process::exit(1);
        }
    };

    println!("╔═══════════════════════════════════════════════════════════════╗");
    println!("║                 ONAJI REACTIVE DAEMON                         ║");
    println!("╚═══════════════════════════════════════════════════════════════╝");
    println!("Project:    {}", manifest.name);
    println!(
        "Workspace:  {}",
        workspace_root
            .canonicalize()
            .unwrap_or_else(|_| workspace_root.to_path_buf())
            .display()
    );
    println!("Resources:  {} registered", dag.resource_indices.len());
    println!("Tasks:      {} configured in DAG", dag.task_indices.len());
    println!("Debounce:   {} ms\n", debounce_ms);
    println!("Watching for changes... (Press Ctrl+C to stop)\n");

    let watcher = ReactiveWatcher::new(&workspace_root);
    let mut rx = match watcher.start_watching(&dag, Duration::from_millis(debounce_ms)) {
        Ok(receiver) => receiver,
        Err(err) => {
            eprintln!("✗ Failed to start filesystem watcher: {}", err);
            std::process::exit(1);
        }
    };

    let mut cache = ExecutionCache::new();

    while let Some(event) = rx.recv().await {
        println!("---------------------------------------------------------------");
        println!(
            "⚡ [watch] Resource '{}' {:?} (payload hash: {})",
            event.resource_id,
            event.event_type,
            event
                .payload_hash
                .map(|h| format!("{:016x}", h))
                .unwrap_or_else(|| "none".to_string())
        );

        let analyzer = ImpactAnalyzer::new(&dag);
        if let Some(impact) = analyzer.analyze_resource_change(&event.resource_id) {
            println!(
                "🎯 [impact] {} affected tasks, {} unaffected",
                impact.affected_tasks.len(),
                impact.unaffected_tasks.len()
            );

            let mut scheduler = TaskScheduler::new(&dag, &workspace_root, &mut cache);
            match scheduler.execute_impact(&impact).await {
                Ok(summary) => {
                    println!("\nExecution Plan:");
                    for res in summary.results {
                        match res.state {
                            TaskState::Success => {
                                println!("  ✓ {:<26} {:>6} ms", res.task_id, res.duration_ms);
                            }
                            TaskState::Cached => {
                                println!("  ⚡ {:<26}   cached", res.task_id);
                            }
                            TaskState::Blocked => {
                                println!("  ○ {:<26}  blocked", res.task_id);
                            }
                            TaskState::Failed => {
                                println!(
                                    "  ✗ {:<26}   failed (exit code: {:?})",
                                    res.task_id, res.exit_code
                                );
                                if !res.stderr.is_empty() {
                                    for line in res.stderr.lines() {
                                        println!("      | {}", line);
                                    }
                                }
                            }
                            _ => {
                                println!("  ? {:<26}  {:?}", res.task_id, res.state);
                            }
                        }
                    }
                    for unaffected in &impact.unaffected_tasks {
                        println!("  ○ {:<26}  skipped (unaffected)", unaffected);
                    }
                    println!(
                        "\nTotal Time: {} ms | Status: {}\n",
                        summary.total_duration_ms,
                        if summary.success { "PASSED" } else { "FAILED" }
                    );
                }
                Err(err) => {
                    eprintln!("✗ Execution error: {}", err);
                }
            }
        }
    }
}

async fn handle_run(config_path: &Path, resource_filter: Option<String>) {
    let workspace_root = resolve_workspace_root(config_path);
    let (_manifest, dag) = match ConfigLoader::load_from_file(config_path) {
        Ok(res) => res,
        Err(err) => {
            eprintln!("✗ Error loading '{}': {}", config_path.display(), err);
            std::process::exit(1);
        }
    };

    let mut cache = ExecutionCache::new();
    let mut scheduler = TaskScheduler::new(&dag, &workspace_root, &mut cache);

    let tasks_to_run: Vec<onaji_core::domain::TaskId> = if let Some(res_str) = resource_filter {
        let res_id = ResourceId::new(res_str);
        let analyzer = ImpactAnalyzer::new(&dag);
        match analyzer.analyze_resource_change(&res_id) {
            Some(report) => report.affected_tasks,
            None => {
                eprintln!("✗ Resource '{}' not found in DAG.", res_id);
                std::process::exit(1);
            }
        }
    } else {
        dag.task_indices.keys().cloned().collect()
    };

    println!("Running {} tasks...", tasks_to_run.len());
    match scheduler.execute_tasks(&tasks_to_run).await {
        Ok(summary) => {
            for res in summary.results {
                match res.state {
                    TaskState::Success => {
                        println!("  ✓ {:<26} {:>6} ms", res.task_id, res.duration_ms)
                    }
                    TaskState::Cached => println!("  ⚡ {:<26}   cached", res.task_id),
                    TaskState::Blocked => println!("  ○ {:<26}  blocked", res.task_id),
                    TaskState::Failed => println!(
                        "  ✗ {:<26}   failed (exit code: {:?})",
                        res.task_id, res.exit_code
                    ),
                    _ => println!("  ? {:<26}  {:?}", res.task_id, res.state),
                }
            }
            println!("\nCompleted in {} ms", summary.total_duration_ms);
        }
        Err(err) => {
            eprintln!("✗ Run failed: {}", err);
            std::process::exit(1);
        }
    }
}

fn handle_check(config_path: &Path) {
    match ConfigLoader::load_from_file(config_path) {
        Ok((manifest, dag)) => {
            println!("✓ Manifest '{}' is valid.", config_path.display());
            println!(
                "✓ Graph successfully built: {} nodes, {} edges across {} tasks.",
                dag.node_count(),
                dag.edge_count(),
                manifest.tasks.len()
            );
        }
        Err(err) => {
            eprintln!("✗ Validation failed: {}", err);
            std::process::exit(1);
        }
    }
}

fn handle_graph(config_path: &Path, format: GraphFormat) {
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

fn handle_doctor(config_path: &Path) {
    println!("=== Onaji Doctor Diagnostics ===\n");
    println!("Toolchain Check:");
    check_tool("rustc", &["--version"]);
    check_tool("cargo", &["--version"]);
    check_tool("node", &["--version"]);
    check_tool("pnpm", &["--version"]);
    check_tool("java", &["-version"]);
    check_tool("gradle", &["--version"]);
    check_tool("docker", &["--version"]);

    println!("\nProject Manifest & DAG:");
    if config_path.exists() {
        match ConfigLoader::load_from_file(config_path) {
            Ok((_manifest, dag)) => {
                println!("  ✓ {} ............. valid syntax", config_path.display());
                println!("  ✓ Dependency DAG ........... valid (no cycles, single producers)");
                println!("  ✓ Discovered Tasks ......... {}", dag.task_indices.len());
                println!(
                    "  ✓ Registered Resources ..... {}",
                    dag.resource_indices.len()
                );
                println!(
                    "  ✓ Tracked Artifacts ........ {}",
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
            "  ⚠ {} ............. not found in current directory",
            config_path.display()
        );
    }
}

fn handle_impact(config_path: &Path, resource_id_str: &str) {
    let (_, dag) = match ConfigLoader::load_from_file(config_path) {
        Ok(res) => res,
        Err(err) => {
            eprintln!("✗ Error loading '{}': {}", config_path.display(), err);
            std::process::exit(1);
        }
    };

    let res_id = ResourceId::new(resource_id_str);
    let analyzer = ImpactAnalyzer::new(&dag);
    match analyzer.analyze_resource_change(&res_id) {
        Some(report) => {
            println!(
                "=== Impact Analysis for Resource: '{}' ===",
                resource_id_str
            );
            println!("\nAffected Tasks ({}):", report.affected_tasks.len());
            for (idx, task) in report.affected_tasks.iter().enumerate() {
                println!("  {}. [Task: {}]", idx + 1, task);
            }

            println!(
                "\nAffected Artifacts ({}):",
                report.affected_artifacts.len()
            );
            for art in &report.affected_artifacts {
                println!("  • {}", art);
            }

            println!("\nUnaffected Tasks ({}):", report.unaffected_tasks.len());
            for task in &report.unaffected_tasks {
                println!("  • [Task: {}]", task);
            }
        }
        None => {
            eprintln!(
                "✗ Resource '{}' is not registered in the graph.",
                resource_id_str
            );
            std::process::exit(1);
        }
    }
}

fn check_tool(bin: &str, args: &[&str]) {
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

fn sanitize_id(id: &str) -> String {
    id.chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .collect()
}

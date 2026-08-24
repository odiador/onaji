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
    /// Initialize a new onaji.yaml in the current workspace by auto-detecting project structure
    Init {
        #[arg(short, long, help = "Force overwrite if onaji.yaml already exists")]
        force: bool,
    },

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

    /// Inspect a resource or workspace using native adapters
    Inspect {
        #[arg(
            help = "Resource ID, path, or adapter name (e.g. 'openapi', 'gradle', './backend/api/openapi.yaml')"
        )]
        target: Option<String>,
    },

    /// Validate a resource against its semantic specification
    Validate {
        #[arg(help = "Resource ID or path to validate (e.g. './backend/api/openapi.yaml')")]
        target: Option<String>,
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

    /// Start the JSON-RPC 2.0 Daemon server (for IDE extensions / remote clients)
    Daemon {
        #[arg(
            long,
            help = "Unix domain socket path (macOS/Linux) or Named Pipe (Windows)"
        )]
        socket: Option<PathBuf>,

        #[arg(long, default_value = "true", help = "Run over standard I/O streams")]
        stdio: bool,
    },

    /// Send a JSON-RPC request to the Onaji daemon
    Rpc {
        #[arg(help = "RPC method name (e.g. 'initialize', 'graph/get', 'pipeline/run')")]
        method: String,

        #[arg(short, long, help = "Optional JSON parameters string")]
        params: Option<String>,

        #[arg(long, help = "Optional socket path to connect to")]
        socket: Option<PathBuf>,
    },

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
        Commands::Init { force } => handle_init(&cli.config, force),
        Commands::Watch { debounce } => handle_watch(&cli.config, debounce).await,
        Commands::Run { resource } => handle_run(&cli.config, resource).await,
        Commands::Daemon { socket, stdio } => handle_daemon(&cli.config, socket, stdio).await,
        Commands::Rpc {
            method,
            params,
            socket,
        } => handle_rpc(&cli.config, method, params, socket).await,
        Commands::Inspect { target } => handle_inspect(&cli.config, target),
        Commands::Validate { target } => handle_validate(&cli.config, target),
        Commands::Graph { format } => handle_graph(&cli.config, format),
        Commands::Doctor => handle_doctor(&cli.config),
        Commands::Check => handle_check(&cli.config),
        Commands::Impact { resource } => handle_impact(&cli.config, &resource),
    }
}

async fn handle_daemon(config_path: &Path, socket: Option<PathBuf>, _stdio: bool) {
    let workspace_root = resolve_workspace_root(config_path);
    let mut server = onaji_core::DaemonServer::new(&workspace_root);

    if let Some(sock_path) = socket {
        #[cfg(unix)]
        {
            println!("Starting Onaji Daemon on socket: {}", sock_path.display());
            if let Err(err) = server.run_unix_socket(sock_path).await {
                eprintln!("Daemon error: {}", err);
                std::process::exit(1);
            }
        }
        #[cfg(not(unix))]
        {
            eprintln!("Unix domain socket is only supported on Unix systems");
            std::process::exit(1);
        }
    } else {
        if let Err(err) = server.run_stdio().await {
            eprintln!("Daemon stdio error: {}", err);
            std::process::exit(1);
        }
    }
}

async fn handle_rpc(
    config_path: &Path,
    method: String,
    params_str: Option<String>,
    _socket: Option<PathBuf>,
) {
    let workspace_root = resolve_workspace_root(config_path);
    let mut dispatcher = onaji_core::Dispatcher::new(&workspace_root);

    let parsed_params: Option<serde_json::Value> =
        params_str.and_then(|s| serde_json::from_str(&s).ok());
    let req = onaji_protocol::messages::RpcRequest::new(1, method, parsed_params);

    let resp = dispatcher.dispatch_request(&req).await;
    println!("{}", serde_json::to_string_pretty(&resp).unwrap());
}

fn handle_inspect(config_path: &Path, target: Option<String>) {
    let workspace_root = resolve_workspace_root(config_path);
    let registry = onaji_core::adapter::AdapterRegistry::with_builtins();

    let target_path = target.as_ref().map(Path::new);
    let ctx = onaji_core::adapter::AdapterContext::new(&workspace_root);
    let ctx = if let Some(p) = target_path {
        ctx.with_resource_path(p)
    } else {
        ctx
    };

    if let Some(target_id) = &target {
        if let Some(adapter) = registry.get(target_id) {
            match adapter.inspect(&ctx) {
                Ok(res) => {
                    print_inspection_result(&res);
                    return;
                }
                Err(err) => {
                    eprintln!("✗ Error inspecting with '{}': {}", target_id, err);
                    std::process::exit(1);
                }
            }
        }
    }

    let detections = registry.detect_all(&ctx);
    if detections.is_empty() {
        println!("No compatible adapter detected for target: {:?}", target);
        return;
    }

    for (id, det) in detections {
        if let Some(adapter) = registry.get(&id) {
            println!(
                "=== [{}] {} (Confidence: {:.0}%) ===",
                adapter.id(),
                adapter.name(),
                det.confidence * 100.0
            );
            if let Ok(res) = adapter.inspect(&ctx) {
                print_inspection_result(&res);
            }
            println!();
        }
    }
}

fn handle_validate(config_path: &Path, target: Option<String>) {
    let workspace_root = resolve_workspace_root(config_path);
    let registry = onaji_core::adapter::AdapterRegistry::with_builtins();

    let target_path = target.as_ref().map(Path::new);
    let ctx = onaji_core::adapter::AdapterContext::new(&workspace_root);
    let ctx = if let Some(p) = target_path {
        ctx.with_resource_path(p)
    } else {
        ctx
    };

    let mut found = false;
    let detections = registry.detect_all(&ctx);

    for (id, _) in detections {
        if let Some(adapter) = registry.get(&id) {
            found = true;
            println!("Validating with [{}] {}...", adapter.id(), adapter.name());
            match adapter.validate(&ctx) {
                Ok(val) => {
                    if val.is_valid {
                        println!("  ✓ Resource is structurally valid.");
                    } else {
                        println!("  ✗ Validation failed:");
                        for e in val.errors {
                            println!("      • Error: {}", e);
                        }
                    }
                    for w in val.warnings {
                        println!("      ⚠ Warning: {}", w);
                    }
                }
                Err(err) => {
                    println!("  ✗ Adapter validation error: {}", err);
                }
            }
        }
    }

    if !found {
        println!(
            "No compatible validator adapter found for target: {:?}",
            target
        );
    }
}

fn print_inspection_result(res: &onaji_core::adapter::InspectionResult) {
    println!("---------------------------------------------------------------");
    println!("Title:   {}", res.title);
    if let Some(ver) = &res.version {
        println!("Version: {}", ver);
    }
    println!("Summary: {}", res.summary);
    println!("\nProperties:");
    for prop in &res.properties {
        println!("  • {:<24}: {}", prop.key, prop.value);
    }
    if !res.children.is_empty() {
        println!("\nComponents / Modules:");
        for c in &res.children {
            println!("  ◆ {}", c);
        }
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

fn handle_init(config_path: &Path, force: bool) {
    let workspace_root = resolve_workspace_root(config_path);
    let target_manifest = workspace_root.join("onaji.yaml");

    if target_manifest.exists() && !force {
        println!(
            "✗ File '{}' already exists. Use --force to overwrite.",
            target_manifest.display()
        );
        return;
    }

    println!("Scanning workspace for detectable toolchains and resources...");
    let mut resources = Vec::new();
    let mut tasks = Vec::new();
    let mut artifacts = Vec::new();

    let openapi_candidates = [
        "backend/api/openapi.yaml",
        "api/openapi.yaml",
        "openapi.yaml",
        "openapi.json",
        "swagger.json",
    ];
    let mut has_openapi = false;
    for cand in &openapi_candidates {
        if workspace_root.join(cand).exists() {
            resources.push(format!(
                "  openapi_spec:\n    type: file\n    path: \"./{}\"\n    debounce_ms: 200\n    description: \"OpenAPI REST contract\"",
                cand
            ));
            artifacts.push("  validated_openapi: {}\n  kotlin_client_sdk: {}".to_string());
            tasks.push(format!(
                "  validate_openapi:\n    inputs: [openapi_spec]\n    outputs: [validated_openapi]\n    run: \"onaji validate ./{}\"",
                cand
            ));
            has_openapi = true;
            break;
        }
    }

    if workspace_root.join("settings.gradle.kts").exists()
        || workspace_root.join("settings.gradle").exists()
    {
        resources.push("  gradle_workspace:\n    type: file\n    path: \"./settings.gradle.kts\"\n    debounce_ms: 300\n    description: \"Gradle multi-module build settings\"".to_string());
        if has_openapi {
            tasks.push("  build_and_test:\n    inputs: [validated_openapi]\n    outputs: [app_binaries]\n    run: \"./gradlew check\"".to_string());
            artifacts.push("  app_binaries: {}".to_string());
        }
    }

    if workspace_root.join("Cargo.toml").exists() {
        resources.push("  cargo_workspace:\n    type: file\n    path: \"./Cargo.toml\"\n    debounce_ms: 250\n    description: \"Rust Cargo workspace manifest\"".to_string());
    }

    if workspace_root.join("package.json").exists() {
        resources.push("  node_package:\n    type: file\n    path: \"./package.json\"\n    debounce_ms: 200\n    description: \"Node.js package manifest\"".to_string());
    }

    if resources.is_empty() {
        resources.push("  main_source:\n    type: file\n    path: \"./src/\"\n    debounce_ms: 250\n    description: \"Source code directory\"".to_string());
        tasks.push("  build_task:\n    inputs: [main_source]\n    outputs: [build_output]\n    run: \"echo 'Building project...'\"".to_string());
        artifacts.push("  build_output: {}".to_string());
    }

    let project_name = workspace_root
        .file_name()
        .unwrap_or_default()
        .to_string_lossy();
    let content = format!(
        r#"version: "1"
name: "{}"

resources:
{}

artifacts:
{}

tasks:
{}
"#,
        project_name,
        resources.join("\n\n"),
        artifacts.join("\n"),
        tasks.join("\n\n")
    );

    match std::fs::write(&target_manifest, content) {
        Ok(_) => {
            println!(
                "✓ Created '{}' with auto-detected configuration.",
                target_manifest.display()
            );
            println!("Run `onaji graph` to inspect the DAG, or `onaji watch` to start reactive automation.");
        }
        Err(err) => {
            eprintln!("✗ Failed to write '{}': {}", target_manifest.display(), err);
        }
    }
}

fn handle_doctor(config_path: &Path) {
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

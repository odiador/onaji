use crate::cli::OutputFormat;
use crate::commands::check::handle_check;
use crate::commands::doctor::handle_doctor;
use crate::util::resolve_workspace_root;
use std::path::Path;

pub fn handle_onboard(config_path: &Path) {
    let workspace_root = resolve_workspace_root(config_path);
    let target_manifest = workspace_root.join("onaji.yaml");

    println!("================================================================================");
    println!("             ONΛJI — Interactive Workspace Onboarding Guide                    ");
    println!("================================================================================\n");

    println!(
        "🔍 Step 1: Inspecting workspace at: {}",
        workspace_root.display()
    );
    if target_manifest.exists() {
        println!("  ✓ Found existing configuration manifest: onaji.yaml");
    } else {
        println!("  ○ No onaji.yaml found. Generating starter configuration...");
        handle_init(config_path, false);
    }

    println!("\n🩺 Step 2: Running Environment & Toolchain Diagnostics...");
    handle_doctor(config_path);

    println!("\n📐 Step 3: Checking Semantic DAG Integrity...");
    handle_check(config_path, false, OutputFormat::Text);

    println!("\n🎯 Step 4: Golden Path Workflow Commands:");
    println!("  1. onaji check          — Validate semantic DAG and dependency integrity");
    println!("  2. onaji run            — Execute DAG pipeline locally or with caching");
    println!("  3. onaji dashboard      — Launch real-time visualizer at http://localhost:8080");
    println!("  4. onaji watch          — Start reactive background watcher daemon");
    println!("\n✨ Onboarding complete! You are ready to automate your polyglot workflows.\n");
}

pub fn handle_init(config_path: &Path, force: bool) {
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

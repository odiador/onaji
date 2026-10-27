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

    // 1. OpenAPI REST Contract Detection
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
            artifacts.push("  validated_openapi: {}".to_string());
            tasks.push(format!(
                "  validate_openapi:\n    inputs: [openapi_spec]\n    outputs: [validated_openapi]\n    run: \"onaji validate ./{}\"",
                cand
            ));
            has_openapi = true;
            break;
        }
    }

    // 2. Rust Cargo Workspace
    if workspace_root.join("Cargo.toml").exists() {
        resources.push("  cargo_manifest:\n    type: file\n    path: \"./Cargo.toml\"\n    debounce_ms: 250\n    description: \"Rust Cargo workspace manifest\"".to_string());
        if workspace_root.join("src").exists() {
            resources.push("  rust_sources:\n    type: file\n    path: \"./src/\"\n    debounce_ms: 200\n    description: \"Rust source code directory\"".to_string());
            artifacts.push("  cargo_binaries: {}".to_string());
            artifacts.push("  cargo_test_reports: {}".to_string());
            tasks.push("  cargo_check:\n    inputs: [cargo_manifest, rust_sources]\n    outputs: [cargo_binaries]\n    run: \"cargo check --workspace\"".to_string());
            tasks.push("  cargo_test:\n    inputs: [cargo_binaries]\n    outputs: [cargo_test_reports]\n    run: \"cargo test --workspace\"\n    cache:\n      paths: [\"target\"]".to_string());
        } else {
            artifacts.push("  cargo_binaries: {}".to_string());
            tasks.push("  cargo_check:\n    inputs: [cargo_manifest]\n    outputs: [cargo_binaries]\n    run: \"cargo check --workspace\"".to_string());
        }
    }

    // 3. Node.js / NPM / Frontend
    if workspace_root.join("package.json").exists() {
        resources.push("  node_package:\n    type: file\n    path: \"./package.json\"\n    debounce_ms: 200\n    description: \"Node.js package manifest\"".to_string());
        artifacts.push("  node_dependencies: {}".to_string());
        tasks.push("  npm_install:\n    inputs: [node_package]\n    outputs: [node_dependencies]\n    run: \"npm install\"\n    cache:\n      paths: [\"node_modules\"]".to_string());
        if workspace_root.join("src").exists() {
            artifacts.push("  frontend_bundle: {}".to_string());
            tasks.push("  npm_build:\n    inputs: [node_dependencies]\n    outputs: [frontend_bundle]\n    run: \"npm run build --if-present\"\n    cache:\n      paths: [\"dist\", \"build\"]".to_string());
        }
    }

    // 4. Gradle / Android
    if workspace_root.join("settings.gradle.kts").exists()
        || workspace_root.join("settings.gradle").exists()
        || workspace_root.join("build.gradle.kts").exists()
        || workspace_root.join("build.gradle").exists()
    {
        let gradle_file = if workspace_root.join("settings.gradle.kts").exists() {
            "settings.gradle.kts"
        } else if workspace_root.join("settings.gradle").exists() {
            "settings.gradle"
        } else if workspace_root.join("build.gradle.kts").exists() {
            "build.gradle.kts"
        } else {
            "build.gradle"
        };
        resources.push(format!(
            "  gradle_build:\n    type: file\n    path: \"./{}\"\n    debounce_ms: 300\n    description: \"Gradle build configuration\"",
            gradle_file
        ));
        artifacts.push("  gradle_classes: {}".to_string());
        let inputs_str = if has_openapi {
            "[validated_openapi, gradle_build]"
        } else {
            "[gradle_build]"
        };
        tasks.push(format!(
            "  gradle_check:\n    inputs: {}\n    outputs: [gradle_classes]\n    run: \"./gradlew check --daemon\"\n    cache:\n      paths: [\"build\"]",
            inputs_str
        ));
    }

    // 5. Go Modules
    if workspace_root.join("go.mod").exists() {
        resources.push("  go_module:\n    type: file\n    path: \"./go.mod\"\n    debounce_ms: 200\n    description: \"Go module definition\"".to_string());
        artifacts.push("  go_test_summary: {}".to_string());
        tasks.push("  go_test:\n    inputs: [go_module]\n    outputs: [go_test_summary]\n    run: \"go test ./...\"".to_string());
    }

    // 6. Python
    if workspace_root.join("pyproject.toml").exists()
        || workspace_root.join("requirements.txt").exists()
    {
        let py_file = if workspace_root.join("pyproject.toml").exists() {
            "pyproject.toml"
        } else {
            "requirements.txt"
        };
        resources.push(format!(
            "  python_deps:\n    type: file\n    path: \"./{}\"\n    debounce_ms: 200\n    description: \"Python dependencies definition\"",
            py_file
        ));
        artifacts.push("  pytest_summary: {}".to_string());
        tasks.push("  pytest_check:\n    inputs: [python_deps]\n    outputs: [pytest_summary]\n    run: \"pytest\"".to_string());
    }

    // Fallback if nothing detected
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

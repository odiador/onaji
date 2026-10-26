use crate::util::resolve_workspace_root;
use std::path::Path;

pub fn handle_inspect(config_path: &Path, target: Option<String>) {
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

pub fn handle_validate(config_path: &Path, target: Option<String>) {
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

pub fn print_inspection_result(res: &onaji_core::adapter::InspectionResult) {
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

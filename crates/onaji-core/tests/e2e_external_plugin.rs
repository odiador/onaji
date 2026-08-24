use onaji_core::adapter::{AdapterContext, AdapterRegistry, Capability};
use onaji_core::plugin::{PluginManager, PluginManifest, PluginPermissions, PluginRuntime};
use std::fs::File;
use std::io::Write;

#[test]
fn test_e2e_subprocess_node_plugin_lifecycle() {
    // Verify node is installed on the system
    let node_check = std::process::Command::new("node").arg("--version").output();
    if node_check.is_err() {
        eprintln!("Node.js is not available in environment, skipping test");
        return;
    }

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!("onaji_e2e_plugin_{}", now));
    let backend_api = temp_dir.join("backend/api");
    std::fs::create_dir_all(&backend_api).unwrap();

    let openapi_file = backend_api.join("openapi.yaml");
    let mut f = File::create(&openapi_file).unwrap();
    writeln!(
        f,
        r#"openapi: "3.0.0"
info:
  title: "Node Plugin Demo API"
  version: "2.4.0"
paths:
  /users:
    get:
      summary: "List users"
  /orders:
    post:
      summary: "Create order"
"#
    )
    .unwrap();

    // Create a standalone Node.js plugin script in temp dir
    let plugin_script = temp_dir.join("plugin.js");
    let mut pf = File::create(&plugin_script).unwrap();
    writeln!(
        pf,
        r#"const fs = require('fs');
const readline = require('readline');

const rl = readline.createInterface({{ input: process.stdin, terminal: false }});

rl.on('line', (line) => {{
  if (!line.trim()) return;
  const req = JSON.parse(line);
  if (req.method === 'initialize') {{
    process.stdout.write(JSON.stringify({{
      jsonrpc: '2.0',
      id: req.id,
      result: {{
        name: 'node-openapi-test',
        version: '1.0.0',
        capabilities: ['Detect', 'Inspect', 'Validate']
      }}
    }}) + '\n');
  }} else if (req.method === 'plugin/detect') {{
    const exists = fs.existsSync(req.params.resource_path);
    process.stdout.write(JSON.stringify({{
      jsonrpc: '2.0',
      id: req.id,
      result: {{ matched: exists, confidence: 1.0, detected_paths: [req.params.resource_path] }}
    }}) + '\n');
  }} else if (req.method === 'plugin/validate') {{
    const exists = fs.existsSync(req.params.resource_path);
    process.stdout.write(JSON.stringify({{
      jsonrpc: '2.0',
      id: req.id,
      result: {{ is_valid: exists, errors: [], warnings: [] }}
    }}) + '\n');
  }} else if (req.method === 'plugin/inspect') {{
    const content = fs.readFileSync(req.params.resource_path, 'utf-8');
    process.stdout.write(JSON.stringify({{
      jsonrpc: '2.0',
      id: req.id,
      result: {{
        title: 'OpenAPI Schema (via Node Process)',
        version: '2.4.0',
        summary: 'Extracted by external node plugin',
        properties: [
          {{ key: 'API Title', value: 'Node Plugin Demo API' }},
          {{ key: 'Total Endpoints', value: '2' }}
        ],
        children: []
      }}
    }}) + '\n');
  }}
}});"#,
    )
    .unwrap();

    let manifest = PluginManifest {
        name: "node-openapi-test".to_string(),
        version: "1.0.0".to_string(),
        protocol: "1".to_string(),
        capabilities: vec![
            Capability::Detect,
            Capability::Inspect,
            Capability::Validate,
        ],
        resources: vec!["openapi".to_string()],
        runtime: PluginRuntime {
            command: "node".to_string(),
            args: vec![plugin_script.to_string_lossy().to_string()],
            env: std::collections::HashMap::new(),
        },
        permissions: PluginPermissions::default(),
        manifest_path: None,
    };

    let mut registry = AdapterRegistry::new();
    let mut manager = PluginManager::new();

    // 1. Initialize & Register external plugin into registry
    manager
        .register_subprocess_plugin(manifest, &mut registry, &temp_dir)
        .unwrap();

    assert_eq!(manager.loaded_plugins().len(), 1);
    let adapter = registry.get("node-openapi-test").unwrap();
    assert_eq!(adapter.id(), "node-openapi-test");
    assert_eq!(
        adapter.capabilities(),
        &[
            Capability::Detect,
            Capability::Inspect,
            Capability::Validate
        ]
    );

    let ctx = AdapterContext::new(&temp_dir).with_resource_path(&openapi_file);

    // 2. Detect through AdapterRegistry
    let detections = registry.detect_all(&ctx);
    assert_eq!(detections.len(), 1);
    assert_eq!(detections[0].0, "node-openapi-test");
    assert!(detections[0].1.matched);

    // 3. Validate through AdapterRegistry
    let val_res = registry.validate("node-openapi-test", &ctx).unwrap();
    assert!(val_res.is_valid);
    assert!(val_res.errors.is_empty());

    // 4. Inspect through AdapterRegistry
    let inspect_res = registry.inspect("node-openapi-test", &ctx).unwrap();
    assert_eq!(inspect_res.version.as_deref(), Some("2.4.0"));
    assert_eq!(
        inspect_res
            .properties
            .iter()
            .find(|p| p.key == "API Title")
            .map(|p| p.value.as_str()),
        Some("Node Plugin Demo API")
    );
    assert_eq!(
        inspect_res
            .properties
            .iter()
            .find(|p| p.key == "Total Endpoints")
            .map(|p| p.value.as_str()),
        Some("2")
    );

    let _ = std::fs::remove_dir_all(&temp_dir);
}

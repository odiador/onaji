use onaji_core::adapter::{Adapter, AdapterContext, Capability};
use onaji_core::plugin::manifest::{PluginManifest, PluginPermissions, PluginRuntime};
use onaji_core::plugin::wasm::{WasmPluginAdapter, WasmSecurityPolicy};

// Real WebAssembly module in WAT format that implements the Onaji Host ABI
const MOCK_WASM_PLUGIN_WAT: &str = r#"
(module
  (memory (export "memory") 1)

  (data (i32.const 0) "{\"matched\":true,\"confidence\":1.0,\"detected_paths\":[\"openapi.yaml\"]}")
  (data (i32.const 100) "{\"is_valid\":true,\"errors\":[],\"warnings\":[]}")
  (data (i32.const 200) "{\"is_valid\":false,\"errors\":[\"Missing openapi or swagger root field\"],\"warnings\":[]}")
  (data (i32.const 300) "{\"title\":\"OpenAPI WASM Adapter\",\"version\":\"1.0.0\",\"summary\":\"Sandboxed Wasmtime Execution\",\"properties\":[{\"key\":\"Sandbox\",\"value\":\"Wasmtime\"},{\"key\":\"Memory\",\"value\":\"64MB\"}],\"children\":[]}")

  (func (export "onaji_malloc") (param $size i32) (result i32)
    i32.const 500
  )

  (func (export "onaji_free") (param $ptr i32) (param $size i32)
    ;; No-op for test harness
  )

  (func (export "onaji_call") (param $method i32) (param $req_ptr i32) (param $req_len i32) (result i64)
    ;; If method == 2 (detect) -> return data at 0 (len 67)
    local.get $method
    i32.const 2
    i32.eq
    if (result i64)
      i64.const 67 ;; (0 << 32) | 67
    else
      ;; If method == 3 (validate)
      local.get $method
      i32.const 3
      i32.eq
      if (result i64)
        ;; For mock: return valid at offset 100, len 43
        i64.const 429496729643 ;; (100 << 32) | 43
      else
        ;; If method == 4 (inspect) -> return data at offset 300, len 189
        i64.const 1288490188989 ;; (300 << 32) | 189
      end
    end
  )
)
"#;

#[test]
fn test_wasm_plugin_sandbox_lifecycle_and_validation() {
    let manifest = PluginManifest {
        name: "openapi-wasm-plugin".to_string(),
        version: "1.0.0".to_string(),
        protocol: "1".to_string(),
        capabilities: vec![
            Capability::Detect,
            Capability::Inspect,
            Capability::Validate,
        ],
        resources: vec![],
        runtime: PluginRuntime {
            command: "wasm".to_string(),
            args: vec![],
            env: std::collections::HashMap::new(),
        },
        permissions: PluginPermissions {
            workspace_filesystem: true,
            network: false,
            environment: vec![],
        },
        manifest_path: None,
    };

    let policy = WasmSecurityPolicy {
        max_memory_bytes: 64 * 1024 * 1024,
        timeout_ms: 5000,
        allow_filesystem: true,
        allow_network: false,
    };

    let adapter = WasmPluginAdapter::from_wat(MOCK_WASM_PLUGIN_WAT, manifest, policy)
        .expect("WASM adapter builds");

    assert_eq!(adapter.id(), "openapi-wasm-plugin");
    assert_eq!(adapter.name(), "openapi-wasm-plugin");
    assert_eq!(adapter.capabilities().len(), 3);

    let temp_workspace = tempfile::tempdir().unwrap();
    let spec_path = temp_workspace.path().join("openapi.yaml");
    std::fs::write(
        &spec_path,
        "openapi: 3.0.0\ninfo:\n  title: Test API\n  version: 1.0.0\n",
    )
    .unwrap();

    let ctx = AdapterContext::new(temp_workspace.path())
        .with_resource_path(std::path::Path::new("openapi.yaml"));

    // 1. Test Detect
    let detection = adapter
        .detect(&ctx)
        .expect("Detect succeeds in WASM sandbox");
    assert!(detection.matched);
    assert_eq!(detection.confidence, 1.0);

    // 2. Test Validate
    let validation = adapter
        .validate(&ctx)
        .expect("Validate succeeds in WASM sandbox");
    assert!(validation.is_valid);
    assert!(validation.errors.is_empty());

    // 3. Test Inspect
    let inspection = adapter
        .inspect(&ctx)
        .expect("Inspect succeeds in WASM sandbox");
    assert_eq!(inspection.title, "OpenAPI WASM Adapter");
    assert_eq!(inspection.version, Some("1.0.0".to_string()));
    assert_eq!(inspection.properties.len(), 2);
    assert_eq!(inspection.properties[0].key, "Sandbox");
    assert_eq!(inspection.properties[0].value, "Wasmtime");
}

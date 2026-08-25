use crate::adapter::{
    Adapter, AdapterContext, AdapterError, Capability, DetectionResult, InspectionResult,
    ValidationResult,
};
use crate::plugin::manifest::PluginManifest;
use crate::runner::TaskExecutionResult;
use std::sync::Mutex;
use wasmtime::*;

#[derive(Debug, Clone)]
pub struct WasmSecurityPolicy {
    pub max_memory_bytes: usize,
    pub timeout_ms: u64,
    pub allow_filesystem: bool,
    pub allow_network: bool,
}

impl Default for WasmSecurityPolicy {
    fn default() -> Self {
        Self {
            max_memory_bytes: 64 * 1024 * 1024, // 64 MB
            timeout_ms: 5000,
            allow_filesystem: false,
            allow_network: false,
        }
    }
}

pub struct WasmPluginAdapter {
    id: String,
    name: String,
    _manifest: PluginManifest,
    capabilities: Vec<Capability>,
    engine: Engine,
    module: Module,
    _policy: WasmSecurityPolicy,
    _execution_lock: Mutex<()>,
}

impl WasmPluginAdapter {
    pub fn from_wasm_bytes(
        wasm_bytes: &[u8],
        manifest: PluginManifest,
        policy: WasmSecurityPolicy,
    ) -> Result<Self, AdapterError> {
        let mut config = Config::new();
        config.consume_fuel(false);

        let engine = Engine::new(&config).map_err(|e| {
            AdapterError::Execution(format!("Failed to create Wasmtime engine: {}", e))
        })?;

        let module = Module::new(&engine, wasm_bytes).map_err(|e| {
            AdapterError::Execution(format!("Failed to compile WASM module: {}", e))
        })?;

        let id = manifest.name.clone();
        let name = manifest.name.clone();
        let capabilities = manifest.capabilities.clone();

        Ok(Self {
            id,
            name,
            _manifest: manifest,
            capabilities,
            engine,
            module,
            _policy: policy,
            _execution_lock: Mutex::new(()),
        })
    }

    pub fn from_wat(
        wat_content: &str,
        manifest: PluginManifest,
        policy: WasmSecurityPolicy,
    ) -> Result<Self, AdapterError> {
        let wasm_bytes = wat::parse_str(wat_content)
            .map_err(|e| AdapterError::Execution(format!("Failed to parse WAT: {}", e)))?;
        Self::from_wasm_bytes(&wasm_bytes, manifest, policy)
    }

    fn invoke_abi(&self, method_code: i32, json_payload: &str) -> Result<String, AdapterError> {
        let mut store = Store::new(&self.engine, ());
        let linker = Linker::new(&self.engine);

        let instance = linker
            .instantiate(&mut store, &self.module)
            .map_err(|e| AdapterError::Execution(format!("Failed to instantiate WASM: {}", e)))?;

        let memory = instance
            .get_memory(&mut store, "memory")
            .ok_or_else(|| AdapterError::Execution("WASM module missing exported 'memory'".to_string()))?;

        // 1. Allocate buffer in WASM memory for request
        let payload_bytes = json_payload.as_bytes();
        let payload_len = payload_bytes.len() as i32;

        let malloc = instance
            .get_typed_func::<i32, i32>(&mut store, "onaji_malloc")
            .map_err(|e| AdapterError::Execution(format!("WASM missing exported 'onaji_malloc': {}", e)))?;

        let req_ptr = malloc
            .call(&mut store, payload_len)
            .map_err(|e| AdapterError::Execution(format!("Failed to allocate WASM memory: {}", e)))?;

        // Write request bytes into linear memory
        memory
            .write(&mut store, req_ptr as usize, payload_bytes)
            .map_err(|e| AdapterError::Execution(format!("Failed to write to WASM memory: {}", e)))?;

        // 2. Call `onaji_call(method_code, req_ptr, req_len) -> i64`
        let onaji_call = instance
            .get_typed_func::<(i32, i32, i32), i64>(&mut store, "onaji_call")
            .map_err(|e| AdapterError::Execution(format!("WASM missing exported 'onaji_call': {}", e)))?;

        let packed_res = onaji_call
            .call(&mut store, (method_code, req_ptr, payload_len))
            .map_err(|e| AdapterError::Execution(format!("Error during WASM call: {}", e)))?;

        let res_ptr = (packed_res >> 32) as usize;
        let res_len = (packed_res & 0xFFFFFFFF) as usize;

        let mut out_buffer = vec![0u8; res_len];
        memory
            .read(&store, res_ptr, &mut out_buffer)
            .map_err(|e| AdapterError::Execution(format!("Failed to read WASM output: {}", e)))?;

        String::from_utf8(out_buffer)
            .map_err(|e| AdapterError::Execution(format!("Invalid UTF-8 from WASM output: {}", e)))
    }
}

impl Adapter for WasmPluginAdapter {
    fn id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn capabilities(&self) -> &[Capability] {
        &self.capabilities
    }

    fn detect(&self, ctx: &AdapterContext) -> Result<DetectionResult, AdapterError> {
        let payload = serde_json::json!({
            "workspace_root": ctx.workspace_root.to_string_lossy(),
            "resource_path": ctx.resource_path.as_ref().map(|p| p.to_string_lossy()),
        });

        let json_out = self.invoke_abi(2, &payload.to_string())?;
        serde_json::from_str(&json_out).map_err(|e| {
            AdapterError::Execution(format!("Failed to deserialize DetectionResult: {}", e))
        })
    }

    fn validate(&self, ctx: &AdapterContext) -> Result<ValidationResult, AdapterError> {
        let file_content = if let Some(path) = &ctx.resource_path {
            std::fs::read_to_string(path).ok()
        } else {
            None
        };

        let payload = serde_json::json!({
            "workspace_root": ctx.workspace_root.to_string_lossy(),
            "resource_path": ctx.resource_path.as_ref().map(|p| p.to_string_lossy()),
            "content": file_content,
        });

        let json_out = self.invoke_abi(3, &payload.to_string())?;
        serde_json::from_str(&json_out).map_err(|e| {
            AdapterError::Execution(format!("Failed to deserialize ValidationResult: {}", e))
        })
    }

    fn inspect(&self, ctx: &AdapterContext) -> Result<InspectionResult, AdapterError> {
        let file_content = if let Some(path) = &ctx.resource_path {
            std::fs::read_to_string(path).ok()
        } else {
            None
        };

        let payload = serde_json::json!({
            "workspace_root": ctx.workspace_root.to_string_lossy(),
            "resource_path": ctx.resource_path.as_ref().map(|p| p.to_string_lossy()),
            "content": file_content,
        });

        let json_out = self.invoke_abi(4, &payload.to_string())?;
        serde_json::from_str(&json_out).map_err(|e| {
            AdapterError::Execution(format!("Failed to deserialize InspectionResult: {}", e))
        })
    }

    fn execute(
        &self,
        ctx: &AdapterContext,
        target: &str,
    ) -> Result<TaskExecutionResult, AdapterError> {
        let payload = serde_json::json!({
            "context": {
                "workspace_root": ctx.workspace_root.to_string_lossy(),
                "resource_path": ctx.resource_path.as_ref().map(|p| p.to_string_lossy()),
            },
            "target": target,
        });

        let json_out = self.invoke_abi(5, &payload.to_string())?;
        serde_json::from_str(&json_out).map_err(|e| {
            AdapterError::Execution(format!("Failed to deserialize TaskExecutionResult: {}", e))
        })
    }
}

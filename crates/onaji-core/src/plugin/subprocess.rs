use super::manifest::PluginManifest;
use super::protocol::{
    PluginContextDto, PluginExecuteParams, PluginInitializeParams, PluginInitializeResult,
};
use crate::adapter::{
    Adapter, AdapterContext, AdapterError, Capability, DetectionResult, InspectionResult,
    ValidationResult,
};
use crate::runner::TaskExecutionResult;
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};

pub struct SubprocessPluginAdapter {
    manifest: PluginManifest,
    cached_capabilities: Vec<Capability>,
}

impl SubprocessPluginAdapter {
    pub fn new(manifest: PluginManifest) -> Self {
        let caps = manifest.capabilities.clone();
        Self {
            manifest,
            cached_capabilities: caps,
        }
    }

    /// Inicializa el plugin externo e interroga sus capacidades dinámicas
    pub fn initialize(
        &mut self,
        workspace_root: &std::path::Path,
    ) -> Result<PluginInitializeResult, AdapterError> {
        let params = PluginInitializeParams {
            protocol_version: "1".to_string(),
            workspace_root: workspace_root.to_string_lossy().to_string(),
        };

        let res: PluginInitializeResult =
            self.rpc_call("initialize", serde_json::to_value(params).unwrap())?;
        self.cached_capabilities = res.capabilities.clone();
        Ok(res)
    }

    fn rpc_call<T: serde::de::DeserializeOwned>(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<T, AdapterError> {
        let mut cmd = Command::new(&self.manifest.runtime.command);
        cmd.args(&self.manifest.runtime.args);
        cmd.stdin(Stdio::piped());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        for (k, v) in &self.manifest.runtime.env {
            cmd.env(k, v);
        }

        let mut child = cmd.spawn().map_err(|e| {
            AdapterError::Execution(format!(
                "Failed to spawn plugin '{}': {}",
                self.manifest.name, e
            ))
        })?;

        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": method,
            "params": params,
        });

        if let Some(mut stdin) = child.stdin.take() {
            let req_str = serde_json::to_string(&request)
                .map_err(|e| AdapterError::Execution(format!("Serialization error: {}", e)))?;
            writeln!(stdin, "{}", req_str).map_err(|e| {
                AdapterError::Execution(format!("Failed writing to plugin stdin: {}", e))
            })?;
        }

        let stdout = child.stdout.take().ok_or_else(|| {
            AdapterError::Execution("Failed to capture plugin stdout".to_string())
        })?;

        let mut reader = BufReader::new(stdout);
        let mut response_line = String::new();
        reader
            .read_line(&mut response_line)
            .map_err(|e| AdapterError::Execution(format!("Failed reading plugin output: {}", e)))?;

        let _ = child.wait();

        let response_val: serde_json::Value =
            serde_json::from_str(&response_line).map_err(|e| {
                AdapterError::Parse(format!(
                    "Invalid JSON-RPC response from plugin '{}': {} (Output was: '{}')",
                    self.manifest.name, e, response_line
                ))
            })?;

        if let Some(err) = response_val.get("error") {
            let msg = err
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("Unknown RPC error");
            return Err(AdapterError::Execution(format!(
                "Plugin RPC error: {}",
                msg
            )));
        }

        let result_val = response_val.get("result").ok_or_else(|| {
            AdapterError::Execution("Missing 'result' in plugin RPC response".to_string())
        })?;

        serde_json::from_value(result_val.clone())
            .map_err(|e| AdapterError::Parse(format!("Failed deserializing plugin result: {}", e)))
    }

    fn to_context_dto(&self, ctx: &AdapterContext) -> PluginContextDto {
        PluginContextDto {
            workspace_root: ctx.workspace_root.to_string_lossy().to_string(),
            resource_path: ctx.resource_path.map(|p| p.to_string_lossy().to_string()),
        }
    }
}

impl Adapter for SubprocessPluginAdapter {
    fn id(&self) -> &str {
        &self.manifest.name
    }

    fn name(&self) -> &str {
        &self.manifest.name
    }

    fn capabilities(&self) -> &[Capability] {
        &self.cached_capabilities
    }

    fn detect(&self, ctx: &AdapterContext) -> Result<DetectionResult, AdapterError> {
        if !self.cached_capabilities.contains(&Capability::Detect) {
            return Err(AdapterError::UnsupportedCapability(Capability::Detect));
        }
        let dto = self.to_context_dto(ctx);
        self.rpc_call("plugin/detect", serde_json::to_value(dto).unwrap())
    }

    fn validate(&self, ctx: &AdapterContext) -> Result<ValidationResult, AdapterError> {
        if !self.cached_capabilities.contains(&Capability::Validate) {
            return Err(AdapterError::UnsupportedCapability(Capability::Validate));
        }
        let dto = self.to_context_dto(ctx);
        self.rpc_call("plugin/validate", serde_json::to_value(dto).unwrap())
    }

    fn inspect(&self, ctx: &AdapterContext) -> Result<InspectionResult, AdapterError> {
        if !self.cached_capabilities.contains(&Capability::Inspect) {
            return Err(AdapterError::UnsupportedCapability(Capability::Inspect));
        }
        let dto = self.to_context_dto(ctx);
        self.rpc_call("plugin/inspect", serde_json::to_value(dto).unwrap())
    }

    fn execute(
        &self,
        ctx: &AdapterContext,
        target: &str,
    ) -> Result<TaskExecutionResult, AdapterError> {
        if !self.cached_capabilities.contains(&Capability::Execute) {
            return Err(AdapterError::UnsupportedCapability(Capability::Execute));
        }
        let params = PluginExecuteParams {
            context: self.to_context_dto(ctx),
            target: target.to_string(),
        };
        self.rpc_call("plugin/execute", serde_json::to_value(params).unwrap())
    }
}

use super::base::{Adapter, AdapterError};
use super::capability::Capability;
use super::context::AdapterContext;
use super::types::{DetectionResult, InspectionResult, ValidationResult};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub struct OpenApiAdapter;

impl Default for OpenApiAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl OpenApiAdapter {
    pub fn new() -> Self {
        Self
    }

    fn find_spec_path(ctx: &AdapterContext) -> Option<PathBuf> {
        if let Some(res_path) = ctx.resource_path {
            let full = if res_path.is_absolute() {
                res_path.to_path_buf()
            } else {
                ctx.workspace_root.join(res_path)
            };
            if full.exists() {
                return Some(full);
            }
        }

        let candidates = [
            "openapi.yaml",
            "openapi.yml",
            "openapi.json",
            "swagger.yaml",
            "swagger.json",
            "api/openapi.yaml",
            "backend/api/openapi.yaml",
        ];

        for candidate in candidates {
            let p = ctx.workspace_root.join(candidate);
            if p.exists() {
                return Some(p);
            }
        }

        None
    }

    fn parse_spec_content(path: &Path) -> Result<Value, AdapterError> {
        let content = std::fs::read_to_string(path)?;
        if path.extension().and_then(|e| e.to_str()) == Some("json") {
            serde_json::from_str(&content).map_err(|e| AdapterError::Parse(e.to_string()))
        } else {
            serde_yaml::from_str(&content).map_err(|e| AdapterError::Parse(e.to_string()))
        }
    }
}

impl Adapter for OpenApiAdapter {
    fn id(&self) -> &'static str {
        "openapi"
    }

    fn name(&self) -> &'static str {
        "OpenAPI Specification Adapter"
    }

    fn capabilities(&self) -> &[Capability] {
        &[
            Capability::Detect,
            Capability::Inspect,
            Capability::Validate,
        ]
    }

    fn detect(&self, ctx: &AdapterContext) -> Result<DetectionResult, AdapterError> {
        if let Some(spec_path) = Self::find_spec_path(ctx) {
            Ok(DetectionResult::matched(
                vec![spec_path],
                1.0,
                "Detected OpenAPI/Swagger contract specification file",
            ))
        } else {
            Ok(DetectionResult::none())
        }
    }

    fn validate(&self, ctx: &AdapterContext) -> Result<ValidationResult, AdapterError> {
        let spec_path = match Self::find_spec_path(ctx) {
            Some(p) => p,
            None => {
                return Ok(ValidationResult::failure(vec![
                    "No OpenAPI specification file found at target path".to_string(),
                ]))
            }
        };

        let val = match Self::parse_spec_content(&spec_path) {
            Ok(v) => v,
            Err(err) => return Ok(ValidationResult::failure(vec![err.to_string()])),
        };

        let mut errors = Vec::new();
        let mut warnings = Vec::new();

        // 1. Check version format
        let has_openapi = val.get("openapi").and_then(|v| v.as_str());
        let has_swagger = val.get("swagger").and_then(|v| v.as_str());

        if has_openapi.is_none() && has_swagger.is_none() {
            errors.push("Missing root 'openapi' or 'swagger' version declaration".to_string());
        }

        // 2. Check info object
        let info = val.get("info");
        if info.is_none() {
            errors.push("Missing required root 'info' object".to_string());
        } else if let Some(info_obj) = info {
            if info_obj.get("title").is_none() {
                errors.push("Missing 'info.title' string".to_string());
            }
            if info_obj.get("version").is_none() {
                errors.push("Missing 'info.version' string".to_string());
            }
        }

        // 3. Check paths
        if val.get("paths").is_none() {
            warnings.push("No 'paths' object defined in OpenAPI specification".to_string());
        }

        if errors.is_empty() {
            let mut res = ValidationResult::ok();
            for w in warnings {
                res = res.with_warning(w);
            }
            Ok(res)
        } else {
            Ok(ValidationResult::failure(errors))
        }
    }

    fn inspect(&self, ctx: &AdapterContext) -> Result<InspectionResult, AdapterError> {
        let spec_path = match Self::find_spec_path(ctx) {
            Some(p) => p,
            None => {
                return Err(AdapterError::Execution(
                    "No OpenAPI specification file found to inspect".to_string(),
                ))
            }
        };

        let val = Self::parse_spec_content(&spec_path)?;

        let spec_ver = val
            .get("openapi")
            .or_else(|| val.get("swagger"))
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");

        let title = val
            .get("info")
            .and_then(|i| i.get("title"))
            .and_then(|t| t.as_str())
            .unwrap_or("Untitled API");

        let api_version = val
            .get("info")
            .and_then(|i| i.get("version"))
            .and_then(|v| v.as_str())
            .unwrap_or("1.0.0");

        let mut method_counts: BTreeMap<&'static str, usize> = BTreeMap::new();
        let mut endpoint_count = 0;

        if let Some(paths) = val.get("paths").and_then(|p| p.as_object()) {
            endpoint_count = paths.len();
            for (_path_str, methods) in paths {
                if let Some(m_obj) = methods.as_object() {
                    for (m, _) in m_obj {
                        let m_upper = m.to_uppercase();
                        match m_upper.as_str() {
                            "GET" => *method_counts.entry("GET").or_default() += 1,
                            "POST" => *method_counts.entry("POST").or_default() += 1,
                            "PUT" => *method_counts.entry("PUT").or_default() += 1,
                            "DELETE" => *method_counts.entry("DELETE").or_default() += 1,
                            "PATCH" => *method_counts.entry("PATCH").or_default() += 1,
                            "OPTIONS" => *method_counts.entry("OPTIONS").or_default() += 1,
                            "HEAD" => *method_counts.entry("HEAD").or_default() += 1,
                            _ => (),
                        }
                    }
                }
            }
        }

        let schema_count = val
            .get("components")
            .and_then(|c| c.get("schemas"))
            .or_else(|| val.get("definitions"))
            .and_then(|s| s.as_object())
            .map(|s| s.len())
            .unwrap_or(0);

        let mut result = InspectionResult::new(
            "OpenAPI Specification",
            format!(
                "OpenAPI {} contract with {} endpoints",
                spec_ver, endpoint_count
            ),
        )
        .with_version(api_version)
        .add_property("Specification Version", spec_ver)
        .add_property("API Title", title)
        .add_property("API Version", api_version)
        .add_property("Total Endpoints", endpoint_count.to_string())
        .add_property("Total Schemas", schema_count.to_string());

        for (method, count) in method_counts {
            result = result.add_property(format!("Method {}", method), count.to_string());
        }

        Ok(result)
    }
}

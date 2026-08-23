use super::base::{Adapter, AdapterError};
use super::capability::Capability;
use super::context::AdapterContext;
use super::types::{DetectionResult, InspectionResult, ValidationResult};
use crate::cache::TaskState;
use crate::runner::TaskExecutionResult;
use std::path::{Path, PathBuf};
use std::process::Command;

pub struct GradleAdapter;

impl Default for GradleAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl GradleAdapter {
    pub fn new() -> Self {
        Self
    }

    fn find_gradle_root(ctx: &AdapterContext) -> Option<PathBuf> {
        let candidates = [
            "settings.gradle.kts",
            "settings.gradle",
            "build.gradle.kts",
            "build.gradle",
            "gradlew",
        ];

        for c in candidates {
            let p = ctx.workspace_root.join(c);
            if p.exists() {
                return Some(ctx.workspace_root.to_path_buf());
            }
        }

        None
    }

    fn extract_wrapper_version(root: &Path) -> Option<String> {
        let props_path = root.join("gradle/wrapper/gradle-wrapper.properties");
        if let Ok(content) = std::fs::read_to_string(props_path) {
            for line in content.lines() {
                if line.starts_with("distributionUrl") {
                    // e.g. distributionUrl=https\://services.gradle.org/distributions/gradle-8.14-bin.zip
                    if let Some(pos) = line.find("gradle-") {
                        let rest = &line[pos + 7..];
                        if let Some(end) = rest.find("-") {
                            return Some(rest[..end].to_string());
                        }
                    }
                }
            }
        }
        None
    }

    fn parse_modules_from_settings(root: &Path) -> Vec<String> {
        let mut modules = Vec::new();
        let settings_files = ["settings.gradle.kts", "settings.gradle"];

        for s_file in settings_files {
            let p = root.join(s_file);
            if let Ok(content) = std::fs::read_to_string(p) {
                for line in content.lines() {
                    let trimmed = line.trim();
                    if trimmed.starts_with("include(") || trimmed.starts_with("include ") {
                        // Extract quoted module names e.g. ":app", ":core:network"
                        let mut current_quote = None;
                        let mut current_str = String::new();
                        for ch in trimmed.chars() {
                            if ch == '"' || ch == '\'' {
                                if current_quote.is_some() {
                                    if !current_str.is_empty() {
                                        modules.push(current_str.clone());
                                        current_str.clear();
                                    }
                                    current_quote = None;
                                } else {
                                    current_quote = Some(ch);
                                }
                            } else if current_quote.is_some() {
                                current_str.push(ch);
                            }
                        }
                    }
                }
            }
        }

        if modules.is_empty() {
            // Default single root module
            modules.push(":root".to_string());
        }

        modules
    }
}

impl Adapter for GradleAdapter {
    fn id(&self) -> &'static str {
        "gradle"
    }

    fn name(&self) -> &'static str {
        "Gradle Build Tool Adapter"
    }

    fn capabilities(&self) -> &[Capability] {
        &[
            Capability::Detect,
            Capability::Inspect,
            Capability::Validate,
            Capability::Execute,
        ]
    }

    fn detect(&self, ctx: &AdapterContext) -> Result<DetectionResult, AdapterError> {
        if let Some(root) = Self::find_gradle_root(ctx) {
            Ok(DetectionResult::matched(
                vec![root],
                0.95,
                "Detected Gradle workspace build configuration",
            ))
        } else {
            Ok(DetectionResult::none())
        }
    }

    fn validate(&self, ctx: &AdapterContext) -> Result<ValidationResult, AdapterError> {
        let root = match Self::find_gradle_root(ctx) {
            Some(r) => r,
            None => {
                return Ok(ValidationResult::failure(vec![
                    "No Gradle build configuration found in workspace".to_string(),
                ]))
            }
        };

        let mut warnings = Vec::new();
        let gradlew = root.join("gradlew");

        if !gradlew.exists() {
            warnings.push(
                "Missing 'gradlew' wrapper script. Running builds may depend on system Gradle."
                    .to_string(),
            );
        }

        let mut res = ValidationResult::ok();
        for w in warnings {
            res = res.with_warning(w);
        }
        Ok(res)
    }

    fn inspect(&self, ctx: &AdapterContext) -> Result<InspectionResult, AdapterError> {
        let root = match Self::find_gradle_root(ctx) {
            Some(r) => r,
            None => {
                return Err(AdapterError::Execution(
                    "No Gradle workspace found to inspect".to_string(),
                ))
            }
        };

        let wrapper_ver =
            Self::extract_wrapper_version(&root).unwrap_or_else(|| "System / Unknown".to_string());
        let modules = Self::parse_modules_from_settings(&root);

        let mut res = InspectionResult::new(
            "Gradle Project",
            format!("Gradle project with {} detected modules", modules.len()),
        )
        .with_version(&wrapper_ver)
        .add_property("Gradle Wrapper Version", &wrapper_ver)
        .add_property("Detected Modules Count", modules.len().to_string());

        for m in modules {
            res = res.add_child(format!("Module: {}", m));
        }

        Ok(res)
    }

    fn execute(
        &self,
        ctx: &AdapterContext,
        target_task: &str,
    ) -> Result<TaskExecutionResult, AdapterError> {
        let root = Self::find_gradle_root(ctx).unwrap_or_else(|| ctx.workspace_root.to_path_buf());
        let gradlew = root.join("gradlew");

        let start = std::time::Instant::now();

        let mut cmd = if gradlew.exists() {
            Command::new(&gradlew)
        } else {
            Command::new("gradle")
        };

        cmd.arg(target_task).current_dir(&root);

        let output = cmd.output()?;
        let duration_ms = start.elapsed().as_millis() as u64;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let exit_code = output.status.code();

        let state = if output.status.success() {
            TaskState::Success
        } else {
            TaskState::Failed
        };

        Ok(TaskExecutionResult {
            task_id: crate::domain::TaskId::new(format!("gradle_{}", target_task)),
            state,
            exit_code,
            stdout,
            stderr,
            duration_ms,
            cache_key: None,
        })
    }
}

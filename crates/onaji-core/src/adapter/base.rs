use super::capability::Capability;
use super::context::AdapterContext;
use super::types::{DetectionResult, InspectionResult, ValidationResult};
use crate::runner::TaskExecutionResult;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AdapterError {
    #[error("Capability '{0}' is not supported by this adapter")]
    UnsupportedCapability(Capability),

    #[error("Adapter execution failed: {0}")]
    Execution(String),

    #[error("IO error in adapter: {0}")]
    Io(#[from] std::io::Error),

    #[error("Failed to parse resource content: {0}")]
    Parse(String),
}

pub trait Adapter: Send + Sync {
    fn id(&self) -> &'static str;
    fn name(&self) -> &'static str;
    fn capabilities(&self) -> &[Capability];

    fn has_capability(&self, cap: Capability) -> bool {
        self.capabilities().contains(&cap)
    }

    fn detect(&self, _ctx: &AdapterContext) -> Result<DetectionResult, AdapterError> {
        Err(AdapterError::UnsupportedCapability(Capability::Detect))
    }

    fn validate(&self, _ctx: &AdapterContext) -> Result<ValidationResult, AdapterError> {
        Err(AdapterError::UnsupportedCapability(Capability::Validate))
    }

    fn inspect(&self, _ctx: &AdapterContext) -> Result<InspectionResult, AdapterError> {
        Err(AdapterError::UnsupportedCapability(Capability::Inspect))
    }

    fn execute(
        &self,
        _ctx: &AdapterContext,
        _target: &str,
    ) -> Result<TaskExecutionResult, AdapterError> {
        Err(AdapterError::UnsupportedCapability(Capability::Execute))
    }
}

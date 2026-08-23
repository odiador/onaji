pub mod base;
pub mod capability;
pub mod context;
pub mod gradle;
pub mod openapi;
pub mod registry;
pub mod types;

pub use base::{Adapter, AdapterError};
pub use capability::Capability;
pub use context::AdapterContext;
pub use gradle::GradleAdapter;
pub use openapi::OpenApiAdapter;
pub use registry::AdapterRegistry;
pub use types::{DetectionResult, InspectionProperty, InspectionResult, ValidationResult};

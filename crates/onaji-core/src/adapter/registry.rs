use super::base::{Adapter, AdapterError};
use super::context::AdapterContext;
use super::gradle::GradleAdapter;
use super::openapi::OpenApiAdapter;
use super::types::{DetectionResult, InspectionResult, ValidationResult};
use std::collections::HashMap;

pub struct AdapterRegistry {
    adapters: HashMap<&'static str, Box<dyn Adapter>>,
}

impl Default for AdapterRegistry {
    fn default() -> Self {
        Self::with_builtins()
    }
}

impl AdapterRegistry {
    pub fn new() -> Self {
        Self {
            adapters: HashMap::new(),
        }
    }

    pub fn with_builtins() -> Self {
        let mut reg = Self::new();
        reg.register(Box::new(OpenApiAdapter::new()));
        reg.register(Box::new(GradleAdapter::new()));
        reg
    }

    pub fn register(&mut self, adapter: Box<dyn Adapter>) {
        self.adapters.insert(adapter.id(), adapter);
    }

    pub fn get(&self, id: &str) -> Option<&dyn Adapter> {
        self.adapters.get(id).map(|a| a.as_ref())
    }

    pub fn list(&self) -> Vec<&dyn Adapter> {
        self.adapters.values().map(|a| a.as_ref()).collect()
    }

    pub fn detect_all(&self, ctx: &AdapterContext) -> Vec<(&'static str, DetectionResult)> {
        let mut results = Vec::new();
        for (id, adapter) in &self.adapters {
            if let Ok(det) = adapter.detect(ctx) {
                if det.matched {
                    results.push((*id, det));
                }
            }
        }
        results
    }

    pub fn inspect(
        &self,
        id: &str,
        ctx: &AdapterContext,
    ) -> Result<InspectionResult, AdapterError> {
        match self.get(id) {
            Some(adapter) => adapter.inspect(ctx),
            None => Err(AdapterError::Execution(format!(
                "Adapter '{}' not registered",
                id
            ))),
        }
    }

    pub fn validate(
        &self,
        id: &str,
        ctx: &AdapterContext,
    ) -> Result<ValidationResult, AdapterError> {
        match self.get(id) {
            Some(adapter) => adapter.validate(ctx),
            None => Err(AdapterError::Execution(format!(
                "Adapter '{}' not registered",
                id
            ))),
        }
    }
}

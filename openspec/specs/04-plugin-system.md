# OpenSpec 04: Plugin & Adapter Architecture

**Status**: Approved  
**Package**: `dev.odiador.onaji.plugins`  
**Domain**: Extensibility, Built-in Adapters, External Process Runners  

---

## 1. MVP Strategy: No Premature WASM

Para evitar sobreingeniería de ABI, sandboxing y distribución en el MVP:
* **Fase 1 (MVP)**:
  1. `Built-in Adapters` compilados en el core (`Filesystem`, `OpenAPI`, `Gradle`).
  2. `Command Adapters` genéricos mediante subprocesos (`tokio::process`).
* **Fase 2 (Post-MVP)**:
  * Plugins externos vía WebAssembly (WASI) o procesos externos JSON-RPC.

---

## 2. Core Rust Trait Specification

```rust
pub trait Plugin: Send + Sync {
    fn name(&self) -> &'static str;
    fn version(&self) -> &'static str;
    
    /// Diagnóstico para `onaji doctor`
    fn check_health(&self) -> HealthReport;
    
    /// Detección automática en el workspace
    fn detect(&self, root: &std::path::Path) -> Option<Vec<DetectedPipeline>>;
    
    /// Validador de inputs / recursos
    fn validate(&self, input: &Artifact) -> Result<(), ValidationError>;
}
```

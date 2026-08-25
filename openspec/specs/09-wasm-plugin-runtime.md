# 09 — Onaji WebAssembly (WASM) Plugin Runtime & Sandbox Specification

## 1. Abstract
The Onaji WASM Plugin Runtime enables high-performance, untrusted third-party automation plugins to execute within a cryptographically verified, isolated Wasmtime sandbox with strict resource limits and zero implicit host permissions.

---

## 2. Sandbox Security Architecture

```text
 ┌──────────────────────────────────────────────────────────────┐
 │                      Onaji Core Engine                       │
 └──────────────────────────────┬───────────────────────────────┘
                                │ JSON ABI Call
                                ▼
 ┌──────────────────────────────────────────────────────────────┐
 │                    Wasmtime Isolation Layer                  │
 │   • Memory Limit: 64 MB (strictly enforced)                  │
 │   • CPU Epoch Interruption / Execution Timeout (e.g. 5000ms) │
 │   • Network Access: Denied by default                        │
 │   • Filesystem: Virtual Workspace Mount only (or Denied)     │
 │   • Environment Variables: Explicitly allowed whitelist only │
 └──────────────────────────────┬───────────────────────────────┘
                                │ Linear Memory Buffer
                                ▼
 ┌──────────────────────────────────────────────────────────────┐
 │                  Untrusted WASM Plugin Module                │
 │                  (Compiled C/Rust/Go/Zig/TS)                 │
 └──────────────────────────────────────────────────────────────┘
```

---

## 3. Host-Plugin ABI Specification

WASM plugin modules export standard entrypoints:
* `onaji_malloc(size: i32) -> i32`: Allocates bytes in plugin linear memory for request payload.
* `onaji_free(ptr: i32, size: i32)`: Deallocates payload buffer.
* `onaji_call(method_code: i32, req_ptr: i32, req_len: i32) -> i64`:
  * `method_code`:
    * `1`: `initialize`
    * `2`: `detect`
    * `3`: `validate`
    * `4`: `inspect`
    * `5`: `execute`
  * Returns `(res_ptr << 32) | (res_len & 0xFFFFFFFF)` pointing to JSON response string in linear memory.

---

## 4. Manifest Runtime Declaration

```yaml
name: "openapi-wasm-validator"
version: "1.0.0"
protocol: "1"

runtime:
  type: "wasm"
  module: "plugin.wasm"

permissions:
  workspace_filesystem: true
  network: false
  max_memory_mb: 64
  timeout_ms: 5000
```

# 12 — Onaji Remote Task Execution & Worker Cluster Specification

## 1. Abstract
The Onaji Remote Execution Cluster distributes task graph processing across heterogeneous worker nodes via JSON-RPC, matching task resource/toolchain requirements to worker capabilities while coordinating artifact inputs and outputs through Content-Addressable Storage (CAS).

---

## 2. Cluster Architecture

```text
 ┌───────────────────────────────────────────────────────────┐
 │                   Coordinator (onaji run --remote)        │
 │   • DAG Resolution & Impact Analysis                      │
 │   • Capability-Based Worker Matching                      │
 │   • Failover, Retries & Distributed Telemetry             │
 └─────────────────────────────┬─────────────────────────────┘
                               │ TaskAssignment (JSON-RPC)
        ┌──────────────────────┼──────────────────────┐
        ▼                      ▼                      ▼
 ┌──────────────┐       ┌──────────────┐       ┌──────────────┐
 │   Worker A   │       │   Worker B   │       │   Worker C   │
 │ Rust / Cargo │       │ Android/JDK  │       │ Node / WASM  │
 └──────┬───────┘       └──────┬───────┘       └──────┬───────┘
        │                      │                      │
        └──────────────────────┼──────────────────────┘
                               │ Fetch Inputs / Push Artifacts
                               ▼
 ┌───────────────────────────────────────────────────────────┐
 │        Distributed Content-Addressable Cache / CAS        │
 └───────────────────────────────────────────────────────────┘
```

---

## 3. Worker Protocol Contract

### Registration & Capability Announcement (`worker/register`):
```json
{
  "worker_id": "worker-linux-01",
  "os": "linux",
  "arch": "x86_64",
  "toolchains": {
    "rust": "1.78.0",
    "java": "21.0.2",
    "android_sdk": "34.0.0"
  },
  "max_parallel_tasks": 4
}
```

### Task Assignment (`task/assign`):
```json
{
  "task_id": "compile_android",
  "workspace_id": "mobile",
  "action": {
    "command": "./gradlew assembleDebug",
    "env": { "GRADLE_OPTS": "-Xmx4g" }
  },
  "cache_key": "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
  "timeout_ms": 30000
}
```

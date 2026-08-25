# 10 — Onaji Structural Observability & OpenTelemetry Tracing Specification

## 1. Abstract
Onaji Structural Observability establishes a causal tracing and metrics architecture that records semantic execution graphs across DAG resolution, impact analysis, multi-tier caching, external plugins, and subshell processes.

---

## 2. Causal Span Hierarchy

```text
onaji.pipeline.run [resource.trigger, workspace.root, duration_ms]
  │
  ├── onaji.impact.analysis [resource.changed, affected_tasks.count]
  │
  ├── onaji.task.execute [task.id, task.action, task.state]
  │     ├── onaji.cache.lookup [cache.key, cache.tier, cache.hit]
  │     ├── onaji.plugin.execute [plugin.name, plugin.runtime, plugin.capability]
  │     └── onaji.artifact.write [artifact.id, artifact.sha256]
  │
  └── onaji.pipeline.summary [tasks.success, tasks.cached, tasks.failed, tasks.blocked]
```

---

## 3. Standard Semantic Attributes

| Attribute | Type | Description |
| :--- | :--- | :--- |
| `task.id` | string | Unique task identifier |
| `task.state` | string | `success`, `cached`, `failed`, `blocked` |
| `task.duration_ms` | integer | Execution latency in milliseconds |
| `cache.hit` | boolean | Whether cache lookup succeeded |
| `cache.tier` | string | `local`, `remote`, `memory` |
| `cache.key` | string | Cryptographic SHA-256 cache key |
| `plugin.name` | string | Registered plugin name (e.g. `@onaji/openapi`) |
| `plugin.runtime` | string | `native`, `subprocess`, `wasm` |
| `causal.trigger` | string | ID of resource or change initiating the run |

---

## 4. CLI Tracing Contract

```bash
onaji run --trace
onaji run --trace --trace-format json --trace-output ./trace.json
```

Environment variable support:
* `ONAJI_TRACE=1`: Enables tracing for any command execution.
* `OTEL_EXPORTER_OTLP_ENDPOINT`: Sets target for OTLP export.

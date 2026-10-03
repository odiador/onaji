# Contract-Driven Reactive Pipeline

This example demonstrates a local-first, zero-infrastructure reactive test and codegen pipeline orchestrated by **ONΛJI**.

## Architecture

```
openapi.yaml (Source of Truth)
       │
       ▼ (codegen)
src/generated/api.d.ts
       │
       ├────────────────────────┐
       ▼ (typecheck)            ▼ (build)
  src/client.ts            dist/client.js
                                │
                                ▼ (test)
                       node --test (Contract Suite)
```

## How It Works

1. **Contract Integrity**:
   - `openapi.yaml` defines the REST API contract (`Order`, `CreateOrderRequest`).
   - Any modification to endpoints, parameters, or schemas causes `codegen` to run.
2. **Selective Invalidation**:
   - If you edit `openapi.yaml`: `codegen` runs, invalidating downstream `typecheck`, `build`, and `test`.
   - If you edit only `src/client.ts`: `codegen` is marked `⚡ CACHED` (zero unnecessary overhead), and only `typecheck`, `build`, and `test` execute.
   - If you run tests twice without edits: all tasks are resolved instantly from CAS (`⚡ CACHED`).

## Running the Pipeline

### 1. Direct Execution
```bash
onaji run
```

### 2. Reactive Watcher Mode
```bash
onaji watch
```
Try modifying `openapi.yaml` (e.g. rename a field or change a type) and observe instant causal compilation and test failure detection.

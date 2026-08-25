# 11 — Onaji Multi-Workspace & Monorepo Architecture Specification

## 1. Abstract
The Onaji Multi-Workspace architecture extends single-project semantic DAGs to distributed multi-package monorepos, enforcing strict subproject boundary isolation while orchestrating explicit cross-workspace artifact dependencies.

---

## 2. Monorepo Root Manifest (`onaji.yaml`)

```yaml
version: "1"
name: "my-enterprise-monorepo"

workspaces:
  - backend
  - mobile
  - web
  - packages/shared
```

---

## 3. Subproject Workspace Schema & Cross-Workspace Artifacts

### Backend Workspace (`backend/onaji.yaml`):
```yaml
version: "1"
name: "backend"

resources:
  openapi_spec:
    type: file
    path: "./api/openapi.yaml"

artifacts:
  openapi_contract:
    description: "Exported OpenAPI 3.0 contract"

tasks:
  export_contract:
    inputs: [openapi_spec]
    outputs: [openapi_contract]
    run: "cargo run --bin export-openapi"
```

### Mobile Workspace (`mobile/onaji.yaml`):
```yaml
version: "1"
name: "mobile"

dependencies:
  - workspace: "backend"
    artifact: "openapi_contract"

artifacts:
  kotlin_client:
    description: "Generated Kotlin API bindings"

tasks:
  generate_client:
    inputs: ["backend:openapi_contract"]
    outputs: [kotlin_client]
    run: "openapi-generator-cli generate"
```

---

## 4. Invariants & Isolation Rules

1. **Strict Subproject Isolation**: Modifications inside `web/` NEVER trigger tasks in `backend/` or `mobile/` unless an explicit artifact dependency links them.
2. **Causal Propagation**: When `backend/api/openapi.yaml` changes, the `MultiWorkspaceImpactAnalyzer` computes the local impact on `backend:export_contract` and cascades across the workspace boundary to trigger `mobile:generate_client`.
3. **Workspace-Namespaced Cache & Telemetry**: Every cache key and telemetry span incorporates the logical `workspace_id`, ensuring zero key collisions and transparent multi-tier resolution.

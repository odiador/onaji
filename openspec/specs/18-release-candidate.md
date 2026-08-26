# 18 — Onaji v0.1.0-rc1 Release Candidate & Stabilization Specification

## 1. Abstract
This specification governs the Feature Freeze, Quality Assurance criteria, Supply-Chain verification gates, and Release Engineering checklist for **Onaji v0.1.0-rc1**.

---

## 2. Feature Freeze Policy
Effective as of `v0.1.0-rc1`, the Onaji codebase enters strict Feature Freeze. Only the following categories of changes are permitted:
* Critical bug fixes and regression remedies.
* Security vulnerabilities and supply-chain integrity enhancements.
* Documentation and developer onboarding clarity.
* Multi-platform release engineering and CI fixes.

---

## 3. Integrated Subsystem Validation Matrix

```text
 ┌───────────────────────────────────┬───────────────────────────────────────────────────────┐
 │ Subsystem                         │ Validation Criteria & Invariant                       │
 ├───────────────────────────────────┼───────────────────────────────────────────────────────┤
 │ Causal DAG Engine                 │ Zero-cycle guarantee, topological ordering, debounce   │
 │ Multi-Tier Cache (CAS)            │ Exact SHA-256 cache key, deterministic hit on second run│
 │ Multi-Workspace Monorepo          │ Cross-workspace dependency propagation, isolated DAGs │
 │ Remote Execution Cluster          │ Capability matching, worker daemon, backpressure      │
 │ Distributed Scheduler             │ Priority aging, capacity bounded, work-stealing       │
 │ Sandboxed Security Runtime        │ Env scrubbed, path traversal rejection, shell blocked │
 │ WASM & Subprocess Plugins         │ Sandboxed execution, onaji.lock SHA-256 pin           │
 │ Public Plugin Registry            │ CAS storage, duplicate rejection, signature check     │
 │ Real-Time Web Dashboard           │ Glassmorphism SPA, REST endpoints, live SSE stream    │
 │ IDE Clients (VS Code & IntelliJ)  │ JSON-RPC 2.0 daemon client, tool windows, actions     │
 │ Supply-Chain Security             │ Ed25519 & Cosign signatures, SHA256SUMS verification  │
 │ Universal Installer (install.sh)  │ SHA-256 pre-install check, POSIX target detection     │
 └───────────────────────────────────┴───────────────────────────────────────────────────────┘
```

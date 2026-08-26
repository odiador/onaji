# Changelog

All notable changes to **ONΛJI** will be documented in this file.
The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [0.1.0-rc1] - 2026-08-26

### Added
- **Reactive Causal DAG Engine**: Graph-based task execution with content-addressed inputs/outputs, topological ordering, and file watching.
- **Multi-Tier Content-Addressed Cache (CAS)**: Local Tier 1 CAS and Remote Tier 2 CAS with deterministic SHA-256 cache keys.
- **Multi-Workspace Monorepo Architecture**: Cross-workspace DAG dependencies with strict boundary isolation and cycle rejection.
- **Remote Execution Cluster & Workers**: Worker daemon, capability matching (OS/Arch/Toolchains), and remote backend.
- **Distributed Scheduler**: Priority queues with dynamic aging, strict slot backpressure, capability-aware work-stealing, and automatic worker loss recovery.
- **WASM & Subprocess Plugin Runtimes**: Sandboxed WASM plugins and JSON-RPC subprocess plugins with pinned `onaji.lock`.
- **Public Plugin Registry (`registry.onaji.dev`)**: Content-addressed object store, REST API, publisher authentication, search, and immutability controls.
- **Real-Time Web Dashboard**: Embedded zero-dependency dark-mode SPA with interactive DAG canvas, worker nodes grid, CAS hit ratio meters, and live SSE event streaming.
- **IDE Integrations**:
  - VS Code Extension with status bar, DAG preview, and task execution.
  - IntelliJ IDEA & Android Studio Plugin with reactive tool window and JSON-RPC client.
- **Developer Onboarding**:
  - `onaji onboard` interactive project scanner and config scaffolding.
  - Comprehensive documentation in `docs/` and reproducible Golden Path example in `examples/polyglot-workspace/`.

### Security & Supply-Chain
- **Cryptographic Release Signing**: Ed25519 digital signatures and Sigstore/Cosign validation for official binary archives.
- **Deterministic Checksumming**: `SHA256SUMS` generation and pre-installation verification in `install.sh`.
- **Command & Argument Hardening**: Subshell and chaining injection blocker (`;`, `|`, `&`, `$()`, `` ` ``), with shells (`sh`, `bash`, `zsh`) explicitly disabled by default.
- **Worker Sandboxing**: Empty environment scrubbing (`env_clear`), path traversal confinement, and CPU/timeout enforcement.
- **Cluster Authentication**: Worker bearer token and mTLS X.509 certificate verification.

### Developer Experience
- Subcommands: `onboard`, `init`, `check`, `run`, `explain`, `watch`, `dashboard`, `doctor`, `worker`, `security`, `workspace`, `plugin`, `release`.

# ONΛJI (同じ)

> **Universal Reactive Build Engine & Causal DAG Scheduler for Polyglot Monorepos**

[![Version](https://img.shields.io/badge/version-0.1.0--rc1-blue.svg)](CHANGELOG.md)
[![License](https://img.shields.io/badge/license-Apache--2.0%20%7C%20MIT-green.svg)](Cargo.toml)
[![Rust](https://img.shields.io/badge/rust-2021%20edition-orange.svg)](https://www.rust-lang.org)
[![Platform](https://img.shields.io/badge/platform-macOS%20%7C%20Linux%20%7C%20Windows-lightgrey.svg)](install.sh)

**ONΛJI** (from Japanese *同じ*, "the same") is a high-performance, deterministic task orchestration and caching engine engineered in Rust. It unifies polyglot software repositories (Rust, TypeScript/Node, Kotlin/Gradle, OpenAPI contracts, Python, WASM) through a reactive, content-addressed Directed Acyclic Graph (DAG).

With sub-millisecond graph evaluation, multi-tier Content-Addressed Storage (CAS), sandboxed remote execution, and bidirectional IDE communication, ONΛJI guarantees that you **never rebuild, test, or re-verify the same code twice**.

---

## ⚡ Key Capabilities

### 1. 🔄 Reactive Causal DAG Engine
- **Topologically Sorted Execution**: Automatically resolves causal dependencies among resources, tasks, and artifacts.
- **Smart Change Detection**: Integrates file-system watchers (`notify`) with debounced hash comparison. Only tasks whose direct inputs or upstream artifacts changed are scheduled.
- **Cycle Detection**: Strictly verifies acyclicity during graph building, rejecting circular dependencies at load time.

### 2. 🗄️ Multi-Tier Content-Addressed Storage (CAS)
- **Deterministic Hashing**: Inputs, arguments, and environment configurations resolve to canonical SHA-256 cache keys.
- **Tier 1 (Local CAS)**: Ultra-fast local artifact store with atomic writes and LRU eviction.
- **Tier 2 (Remote CAS)**: Shared team/cluster cache backend, delivering instant cache hits across CI/CD and developer machines.

### 3. 🎯 Smart Test Selection & Git Impact
- **Git Diff Targeting**: Discover exactly what tasks are affected by a git branch or commit range (`onaji affected --since main`).
- **Targeted Test Execution**: Run only test suites directly connected to modified resources in the causal graph (`onaji run --tests`).

### 4. 🌐 Polyglot Adapters & Extensibility
- **Built-in First-Party Adapters**:
  - **OpenAPI**: Contract validation, breaking change detection, and mock servers.
  - **Gradle / Kotlin**: Gradle task inspection, daemon communication, and build outputs.
  - **Rust / Cargo**: Workspace and crate discovery, linting (`clippy`), and compilation.
- **Extensible Plugin Ecosystem**:
  - Sandboxed **WASM Plugins** (`wasmtime`) with strict host capability boundaries.
  - Out-of-process **JSON-RPC Plugins** authored in TypeScript via `@onaji/plugin-sdk`.
  - Pinned dependencies via `onaji.lock` and public registry support (`registry.onaji.dev`).

### 5. 🚀 Distributed Cluster & Remote Execution
- **Worker Daemon**: Distributed execution node with hardware capability matching (OS, Architecture, Toolchains).
- **Work-Stealing Scheduler**: Dynamic aging priority queue, slot-based backpressure, and worker loss recovery.
- **Hardened Sandboxing**: Command injection rejection (no arbitrary subshell chaining), directory traversal containment, and environment scrubbing (`env_clear`).
- **Cluster Security**: Mutual TLS (mTLS) with X.509 certificates and bearer token authorization.

### 6. 📊 Real-Time Web Dashboard & IDE Tooling
- **Embedded Web UI**: Zero-dependency dark-mode visualizer with interactive DAG canvas, worker grid, CAS hit ratios, and live Server-Sent Events (SSE) streaming (`onaji dashboard`).
- **VS Code Extension**: Status bar indicator, interactive DAG tree view, and direct task dispatch (`editors/vscode`).
- **IntelliJ & Android Studio Plugin**: Native tool window and JSON-RPC daemon client (`editors/intellij`).

---

## 🚀 Quickstart (0 to 1 in 3 Minutes)

### Installation

#### Automated Installer (macOS & Linux)
```bash
curl -fsSL https://raw.githubusercontent.com/odiador/onaji/main/install.sh | bash
```

#### Build from Source (Cargo)
```bash
git clone https://github.com/odiador/onaji.git
cd onaji
cargo install --path crates/onaji-cli
```

---

### Step-by-Step Usage

#### 1. Interactive Onboarding
Run onboarding in any existing polyglot repository:
```bash
onaji onboard
```
`onaji onboard` scans your codebase, detects OpenAPI contracts, Cargo crates, Gradle builds, and TypeScript workspaces, checks toolchains via `doctor`, and scaffolds `onaji.yaml`.

#### 2. Verify Graph Integrity
```bash
onaji check
```

#### 3. Execute Incremental Pipeline
```bash
onaji run
```

Subsequent runs without changes will hit Tier 1/Tier 2 Cache instantly:
```bash
onaji run
# ✓ validate_openapi ............. (cached)
# ✓ generate_kotlin_client ....... (cached)
# ✓ compile_android .............. (cached)
```

#### 4. Launch the Visual Dashboard
```bash
onaji dashboard --open
```
Opens the real-time DAG visualizer at `http://127.0.0.1:4040`.

---

## 📄 Configuration Example (`onaji.yaml`)

ONΛJI models your project as **Resources** (inputs/specs), **Artifacts** (produced outputs), and **Tasks** (executable units):

```yaml
version: "1"
name: "polyglot-microservices"

resources:
  openapi_spec:
    type: file
    path: "./backend/api/openapi.yaml"
    debounce_ms: 250
    description: "OpenAPI v3 contract specification"

  docs_source:
    type: file
    path: "./docs"
    debounce_ms: 500
    description: "Markdown documentation source"

artifacts:
  validated_contract:
    description: "Linted OpenAPI schema"

  kotlin_client_sdk:
    description: "Generated Kotlin Multiplatform client SDK"

  android_build_classes:
    description: "Compiled Kotlin Android binaries"

  docs_html:
    description: "Rendered documentation site"

tasks:
  validate_contract:
    description: "Validate OpenAPI contract syntax and breaking changes"
    inputs:
      - openapi_spec
    outputs:
      - validated_contract
    run: "test -f ./backend/api/openapi.yaml && echo 'Contract valid'"

  generate_kotlin:
    description: "Generate Kotlin SDK models and API client"
    inputs:
      - validated_contract
    outputs:
      - kotlin_client_sdk
    run: "./scripts/generate-kotlin-client.sh"

  compile_android:
    description: "Compile and execute Android unit tests"
    inputs:
      - kotlin_client_sdk
    outputs:
      - android_build_classes
    run: "./gradlew :androidApp:assembleDebug"

  build_docs:
    description: "Generate static documentation site"
    inputs:
      - docs_source
    outputs:
      - docs_html
    run: "echo 'Building static docs... OK'"
```

### 🧬 Causal DAG Visualization

Rendered dynamically via `onaji graph --format mermaid`:

```mermaid
graph TD
    res_openapi_spec["📄 Resource: openapi_spec"]
    res_docs_source["📄 Resource: docs_source"]
    task_compile_android["⚙️ Task: compile_android"]
    task_generate_kotlin["⚙️ Task: generate_kotlin"]
    task_build_docs["⚙️ Task: build_docs"]
    task_validate_contract["⚙️ Task: validate_contract"]
    art_docs_html["📦 Artifact: docs_html"]
    art_android_build_classes["📦 Artifact: android_build_classes"]
    art_kotlin_client_sdk["📦 Artifact: kotlin_client_sdk"]
    art_validated_contract["📦 Artifact: validated_contract"]
    art_kotlin_client_sdk --> task_compile_android
    task_compile_android --> art_android_build_classes
    art_validated_contract --> task_generate_kotlin
    task_generate_kotlin --> art_kotlin_client_sdk
    res_docs_source --> task_build_docs
    task_build_docs --> art_docs_html
    res_openapi_spec --> task_validate_contract
    task_validate_contract --> art_validated_contract
```

---

## ⚡ Performance Benchmarks

Micro-benchmarks executed on Apple Silicon (`--release` profile, `opt-level = 3`):

| Target Component | Workload | Throughput | Latency |
|---|---|---|---|
| **DAG Graph Build & Toposort** | 500 tasks + 1,000 dependency edges | ~2,250 DAGs/sec | **443 µs** |
| **Deterministic `CacheKey` (SHA-256)** | 10,000 compound composite keys | **1,190,925 keys/sec** | **0.84 µs / key** |
| **CAS Memory Cache Lookup** | 50,000 concurrent hit queries | **91,095,422 lookups/sec** | **11 nanoseconds** |
| **JSON-RPC Daemon Dispatch** | 1,000 `graph/get` request cycles | **1,244,877 reqs/sec** | **0.80 µs / req** |
| **Content Hashing (`xxHash3`)** | Streaming 64 KB buffers (1,000 MB payload) | **41.08 GB/sec** | - |

---

## 🛠️ CLI Command Reference

| Command | Description |
|---|---|
| `onaji onboard` | Interactive project detection, adapter discovery, doctor diagnostics, and scaffolding |
| `onaji init [--force]` | Auto-detect toolchains and initialize a starter `onaji.yaml` |
| `onaji check` | Validate syntax, acyclicity, and dependency graph completeness |
| `onaji run [options]` | Execute affected DAG tasks with caching, topological scheduling, and CI flags |
| `onaji affected [options]` | Discover or execute only tasks affected by git diff changes (`--since <ref>`, `--output json`) |
| `onaji explain <task>` | Print causal explanation, trigger reason, and cache status for a given task |
| `onaji watch [options]` | Run reactive file-watcher daemon (optional Smart Test Selection with `--tests`) |
| `onaji dashboard [--open]` | Start embedded real-time web UI and DAG visualizer (SSE streaming) |
| `onaji doctor` | Run diagnostics on installed toolchains, adapters, permissions, and paths |
| `onaji worker <start\|status>`| Manage worker cluster nodes and remote executors |
| `onaji security check` | Audit authentication, command sandboxing, and execution policies |
| `onaji workspace <list\|check>`| Multi-workspace monorepo inspection and cross-impact analysis |
| `onaji plugin <list\|add\|...>`| Manage external WASM / JSON-RPC plugins and lockfile |
| `onaji clean [--all]` | Clear workspace or global Content-Addressed Storage (CAS) caches |
| `onaji completions <shell>` | Generate shell autocomplete scripts (bash, zsh, fish, powershell, elvish) |

---

## 📁 Repository Structure

```
onaji/
├── crates/
│   ├── onaji-core/          # Causal DAG, CAS cache, scheduler, watcher, cluster, adapters
│   ├── onaji-cli/           # Main command-line interface binary (`onaji`)
│   └── onaji-protocol/      # JSON-RPC schemas, DTOs, and protocol specifications
├── packages/
│   ├── create-onaji-plugin/ # Scaffolding CLI for community plugins
│   └── plugin-sdk/          # TypeScript SDK for writing out-of-process JSON-RPC plugins
├── editors/
│   ├── vscode/              # VS Code extension (status bar, graph webview, task runner)
│   └── intellij/            # IntelliJ IDEA & Android Studio plugin
├── backend/
│   └── api/                 # Sample contract definitions (OpenAPI specifications)
├── docs/                    # Architectural guides, concepts, and reference documentation
├── examples/                # Golden path examples (polyglot-workspace, rust-axum-android, etc.)
└── install.sh               # Self-contained installer script for macOS & Linux
```

---

## 🤖 Code Intelligence with CodeGraph

This repository is indexed with **[CodeGraph](https://github.com/codegraph-dev/codegraph)** to provide AI coding assistants (Antigravity IDE, Claude Code, Cursor) with full-graph awareness:

- **Local Index**: Built in `.codegraph/codegraph.db` with SQLite WAL and FTS5 search.
- **Dynamic Dispatch**: Automatically resolves Rust trait implementations (`Adapter`, `CacheBackend`, `ExecutionBackend`) and cross-language boundaries.
- **MCP Integration**: Run `codegraph install` or start `codegraph serve --mcp` to expose symbols, call paths (`codegraph_callers`, `codegraph_callees`), and blast radius impact analysis (`codegraph_impact`) to your AI pair programmer.

---

## 📜 License

ONΛJI is dual-licensed under either:

- **Apache License, Version 2.0** ([LICENSE-APACHE](http://www.apache.org/licenses/LICENSE-2.0))
- **MIT License** ([LICENSE-MIT](http://opensource.org/licenses/MIT))

at your option.

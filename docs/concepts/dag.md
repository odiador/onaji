# Core Architecture Concepts

## 1. Resources, Artifacts, and Tasks
* **Resource**: Input source of truth (files, directories, git trees, HTTP endpoints). Tracked by content hashing (SHA-256) and ETags.
* **Task**: Deterministic execution unit consuming inputs (resources or artifacts) and producing outputs (artifacts).
* **Artifact**: Immutably named intermediate or final output produced by a task.

## 2. Distributed Content-Addressed Caching (CAS)
* Every task execution is assigned a cryptographic Cache Key:
  $$\text{CacheKey} = \text{SHA256}(\text{TaskID} \parallel \text{InputsHash} \parallel \text{Command} \parallel \text{Env})$$
* **Tier 1 (Local CAS)**: Stored in `.onaji/cache/cas/`.
* **Tier 2 (Remote CAS)**: Shared across CI and cluster workers via HTTP/S3/JSON-RPC.

## 3. Multi-Workspace Monorepos
* Subprojects declare their own isolated `onaji.yaml`.
* Root monorepo orchestrates cross-boundary dependencies (`workspace:subproject/artifact`).
* Topological sorting detects and rejects cyclic dependencies across workspace borders.

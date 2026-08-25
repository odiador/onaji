# 07 — Onaji Distributed Cache Protocol Specification

## 1. Abstract
The Onaji Distributed Cache Protocol establishes a content-addressable, multi-tiered caching architecture for deterministic task execution across local developer environments and remote CI/CD clusters.

---

## 2. Hashing Separation of Concerns

Onaji strictly decouples change detection from cryptographic content identity:

1. **`xxHash3` (Fast Change Detection)**:
   * Used by reactive watchers and the DAG Impact Engine to determine if files or HTTP responses have changed in microsecond latencies.
2. **`SHA-256` (Cryptographic Content Identity & Transport Integrity)**:
   * Used for `CacheKey` derivation, remote artifact storage, and cache entry verification across machines.

---

## 3. Cache Key Derivation Specification

The `CacheKey` is deterministically computed as the SHA-256 digest of:
```text
CacheKey = SHA256(
    onaji_engine_version,
    onaji_protocol_version,
    task_id,
    action_type,
    action_payload,
    environment_variables_sorted,
    toolchain_identity,
    input_content_digests_sorted,
    cache_paths_digests_sorted
)
```

---

## 4. Cache Entry Data Model

```json
{
  "key": "a1b2c3d4...",
  "task_id": "validate_openapi",
  "state": "Success",
  "exit_code": 0,
  "stdout": "",
  "stderr": "",
  "duration_ms": 42,
  "artifacts": {
    "generated/client.kt": "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
  },
  "metadata": {
    "created_at_epoch_s": 1724600000,
    "onaji_version": "0.1.0",
    "protocol_version": "0.1.0",
    "platform": "x86_64-unknown-linux-gnu"
  }
}
```

---

## 5. Storage Tiering & Cache Coordinator

```text
               Task Execution Pipeline
                         │
                         ▼
                  CacheCoordinator
                         │
         ┌───────────────┴───────────────┐
         ▼                               ▼
    Tier 1 (Local)                Tier 2 (Remote)
   • In-Memory / Disk             • HTTP REST / S3
   • Microsecond lookup           • Team-wide sharing
```

### Retrieval Order:
1. **Tier 1 (Local Disk / Memory)**:
   * If `HIT` $\to$ Restore task output artifacts immediately.
2. **Tier 2 (Remote HTTP/S3)**:
   * If `HIT` $\to$ Download artifact blobs, populate Tier 1 local disk, restore artifacts.
3. **MISS**:
   * Execute task $\to$ On Success, write `CacheEntry` to Tier 1 and asynchronously push to Tier 2.

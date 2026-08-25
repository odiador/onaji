# 08 — Onaji Plugin Registry & Package Management Specification

## 1. Abstract
The Onaji Plugin Registry establishes a decentralized, content-addressable distribution protocol for third-party automation adapters, guaranteeing deterministic dependency resolution via cryptographic SHA-256 lockfiles (`onaji.lock`).

---

## 2. Registry HTTP REST Protocol (v1)

Registries (public, private, or self-hosted) implement the following endpoints:

| Method | Endpoint | Description |
| :--- | :--- | :--- |
| `GET` | `/v1/plugins?q={query}` | Search plugins by keyword/tag |
| `GET` | `/v1/plugins/{name}` | Fetch plugin metadata and available versions |
| `GET` | `/v1/plugins/{name}/{version}` | Fetch metadata for a specific release |
| `GET` | `/v1/plugins/{name}/{version}/download` | Download plugin distribution archive |
| `POST` | `/v1/plugins` | Publish a new plugin release package |

---

## 3. Package Metadata & Integrity

### Release Manifest Schema:
```json
{
  "name": "@onaji/openapi",
  "version": "1.0.0",
  "protocol_version": "0.1.0",
  "runtime": {
    "command": "node",
    "entrypoint": "dist/index.js"
  },
  "capabilities": ["Detect", "Inspect", "Validate", "Generate"],
  "permissions": {
    "workspace_filesystem": true,
    "network": false
  },
  "sha256": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
  "download_url": "https://registry.onaji.dev/v1/plugins/@onaji/openapi/1.0.0/download"
}
```

---

## 4. Deterministic Lockfile (`onaji.lock`)

When installing plugins with `onaji plugin add`, Onaji writes an immutable `onaji.lock`:

```yaml
version: "1"
plugins:
  - name: "@onaji/openapi"
    version: "1.0.0"
    resolved: "https://registry.onaji.dev/v1/plugins/@onaji/openapi/1.0.0"
    sha256: "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    protocol: "0.1.0"
```

### Invariants:
1. **Deterministic CI Installs**: `onaji check --ci` and `onaji run --ci` strictly verify that on-disk plugin binaries match the SHA-256 digest recorded in `onaji.lock`.
2. **Tamper Rejection**: Any mismatch between downloaded bytes and the lockfile SHA-256 immediately aborts with exit code `2`.

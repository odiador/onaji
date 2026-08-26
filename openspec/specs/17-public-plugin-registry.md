# 17 — Onaji Public Plugin Registry Specification

## 1. Abstract
The Onaji Public Plugin Registry (`registry.onaji.dev`) provides a secure, content-addressed distribution system for external native, Node.js, and WASM plugins. Every published package requires semantic versioning, deterministic SHA-256 hashing, and cryptographic signature verification before admission into the catalog.

---

## 2. Architecture & Content Addressing

```text
 Developer                     Registry API                       CAS Object Store
    │                               │                                     │
    │  onaji plugin publish         │                                     │
    ├──────────────────────────────►│ 1. Validate manifest & protocol     │
    │                               │ 2. Verify SHA-256 & Signature       │
    │                               │ 3. Store raw artifact               │
    │                               ├────────────────────────────────────►│ objects/sha256/ab/cdef...
    │                               │                                     │
    │                               │ 4. Insert metadata & content_addr   │
    │  onaji plugin add @org/plug   │                                     │
    ├──────────────────────────────►│ 5. Resolve version & return meta    │
    │                               │                                     │
    │  Download artifact            │                                     │
    ├──────────────────────────────►├────────────────────────────────────►│ Read stream
    │◄──────────────────────────────┴─────────────────────────────────────┤
    │ 6. Verify local SHA-256 + Sig │
    │ 7. Pin to onaji.lock          │
```

---

## 3. REST API Contract

| Route | Method | Description |
|---|---|---|
| `/v1/plugins?q=<query>` | `GET` | Search published plugins |
| `/v1/plugins/:name` | `GET` | Get plugin metadata and all available versions |
| `/v1/plugins/:name/:version` | `GET` | Get specific version metadata, SHA-256, and signature |
| `/v1/plugins/publish` | `POST` | Authenticated upload of signed plugin package |
| `/v1/plugins/:name/:version/download` | `GET` | Download immutable plugin artifact |

---

## 4. Rejection Invariants
1. **Duplicate Version Rejection**: Once a version (`@org/plugin@1.2.0`) is published, it is immutable and cannot be overwritten.
2. **Hash Mismatch Rejection**: Declared SHA-256 must match the computed digest of payload bytes.
3. **Invalid Signature Rejection**: Signatures must verify against declared publisher public key.
4. **Protocol Incompatibility Rejection**: Plugin protocol version must be compatible with engine protocol `0.1.0`.

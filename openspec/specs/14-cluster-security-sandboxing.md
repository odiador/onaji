# 14 — Onaji Cluster Security, Authentication & Sandboxing Specification

## 1. Abstract
The Onaji Cluster Security Architecture establishes cryptographically verified worker identity, mutual authentication (Token & mTLS), granular command authorization policies, environment isolation, path jail confinement, and process resource enforcement.

> **Security Note**: In non-virtualized Unix runtimes, filesystem and command checks provide *policy enforcement*. For hostile untrusted code execution, dedicated container runtimes (cgroups/seccomp/namespaces) or sandboxed WASM (Hito 15) must be employed.

---

## 2. Authentication & Worker Identity

```text
               ┌─────────────────────────────────────────┐
               │         Coordinator (Auth Engine)       │
               └────────────────────┬────────────────────┘
                                    │
                     ┌──────────────┴──────────────┐
                     ▼                             ▼
              [Bearer Token]                [mTLS X.509]
             Development Mode              Production Mode
```

* **Token Mode**: Shared secret validation via constant-time comparison (`HMAC-SHA256`).
* **mTLS Mode**: X.509 certificate exchange with certificate fingerprint pinning and expiration verification.

---

## 3. Execution Policy & Authorization

```yaml
security:
  execution:
    allow_commands:
      - cargo
      - gradle
      - npm
      - node
      - echo
      - sh
    deny_commands:
      - sudo
      - shutdown
      - rm -rf /
      - curl
    network: false
    allowed_env:
      - PATH
      - JAVA_HOME
      - ANDROID_HOME
      - RUST_LOG
    limits:
      memory_mb: 4096
      timeout_secs: 300
```

---

## 4. Sandboxing & Isolation Rules

1. **Filesystem Jail**: Working directories are strictly confined within the designated sandbox root. Any attempt to traverse (`../`, root `/`, absolute paths outside sandbox) triggers an immediate security violation.
2. **Environment Scrubbing**: Task execution inherits an empty environment, populated strictly with declared `allowed_env` variables and task-specific explicit properties. Worker host variables are completely shielded.
3. **Timeout & Resource Limits**: Processes running beyond `timeout_secs` are forcibly terminated (`SIGKILL`).

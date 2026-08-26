# 16 — Onaji Production Packaging, Release Engineering & Distribution Specification

## 1. Abstract
The Onaji Release Engineering & Distribution Architecture establishes deterministic multi-target compilation, cryptographic SHA-256 verification, unified release manifests, IDE extension packaging, and an automated zero-privilege installer (`install.sh`).

---

## 2. Multi-Target Artifact Matrix

```text
 ┌───────────────────────────────┬────────────────────────────────────────────┐
 │ Target Platform               │ Artifact Archive                           │
 ├───────────────────────────────┼────────────────────────────────────────────┤
 │ macOS Apple Silicon (arm64)   │ onaji-v0.1.0-darwin-arm64.tar.gz          │
 │ macOS Intel (x86_64)          │ onaji-v0.1.0-darwin-x86_64.tar.gz         │
 │ Linux ARM64 (aarch64)         │ onaji-v0.1.0-linux-arm64.tar.gz           │
 │ Linux x86_64 (amd64)          │ onaji-v0.1.0-linux-x86_64.tar.gz          │
 │ Windows x86_64                │ onaji-v0.1.0-windows-x86_64.zip           │
 │ VS Code Extension             │ onaji-vscode-0.1.0.vsix                   │
 │ IntelliJ / Android Studio     │ onaji-intellij-0.1.0.zip                  │
 │ TypeScript Plugin SDK         │ onaji-plugin-sdk-0.1.0.tgz                │
 └───────────────────────────────┴────────────────────────────────────────────┘
```

---

## 3. Cryptographic Verification & Release Manifest

Each GitHub Release publishes `SHA256SUMS` and `release-manifest.json`:

```json
{
  "version": "0.1.0",
  "protocol_version": "0.1.0",
  "release_date": "2026-08-25T19:45:00Z",
  "artifacts": [
    {
      "filename": "onaji-v0.1.0-darwin-arm64.tar.gz",
      "os": "darwin",
      "arch": "arm64",
      "sha256": "4a6f8b9c0d1e2f3a4b5c6d7e8f9a0b1c2d3e4f5a6b7c8d9e0f1a2b3c4d5e6f7a",
      "size_bytes": 12845610
    }
  ]
}
```

---

## 4. Deterministic Installation Flow (`install.sh`)

```text
 ┌────────────────┐     ┌──────────────────┐     ┌────────────────┐
 │ Detect OS/Arch ├────►│ Download Archive ├────►│ Verify SHA-256 │
 └────────────────┘     └──────────────────┘     └───────┬────────┘
                                                         │ Valid
                                                         ▼
 ┌────────────────┐     ┌──────────────────┐     ┌────────────────┐
 │  onaji doctor  │◄────┤  Set PATH Hint   │◄────┤ Install to Dir │
 └────────────────┘     └──────────────────┘     └────────────────┘
```

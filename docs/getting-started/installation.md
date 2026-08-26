# Installation & Setup

## One-Line Universal Installer (macOS, Linux, Windows WSL)

```bash
curl -fsSL https://onaji.dev/install.sh | bash
```

The installer verifies SHA-256 cryptographic signatures against official release checksums and places the binary in `~/.onaji/bin/onaji`.

## Cargo / Source Installation

```bash
cargo install --path crates/onaji-cli
```

## Verify Installation

```bash
onaji --version
# Output: onaji 0.1.0 (protocol v0.1.0)

onaji doctor
```

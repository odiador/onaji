#!/usr/bin/env bash
set -euo pipefail

# Onaji Universal Reactive Engine Installer
# Usage: curl -fsSL https://get.onaji.dev | bash

INSTALL_DIR="${HOME}/.onaji/bin"
BINARY_NAME="onaji"

echo "================================================="
echo " Installing Onaji Reactive Automation Engine"
echo "================================================="

mkdir -p "${INSTALL_DIR}"

if command -v cargo >/dev/null 2>&1; then
    echo ">>> Building Onaji from source via Cargo..."
    cargo install --path crates/onaji-cli --root "${HOME}/.onaji"
else
    echo ">>> Error: Cargo is required for local build installation."
    exit 1
fi

echo ""
echo "✓ Onaji successfully installed to: ${INSTALL_DIR}/${BINARY_NAME}"
echo ""
echo "Ensure ${INSTALL_DIR} is in your PATH:"
echo "  export PATH=\"\$HOME/.onaji/bin:\$PATH\""
echo ""
echo "Verify installation:"
echo "  onaji doctor"

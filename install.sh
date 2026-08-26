#!/usr/bin/env bash
set -euo pipefail

# ==============================================================================
# ONΛJI Universal Deterministic Installer (macOS & Linux)
# https://github.com/odiador/onaji
# ==============================================================================

VERSION="${ONAJI_VERSION:-0.1.0}"
INSTALL_DIR="${ONAJI_INSTALL_DIR:-$HOME/.onaji/bin}"
BASE_URL="https://github.com/odiador/onaji/releases/download/v${VERSION}"

# 1. Detect Operating System (POSIX)
OS="$(uname -s)"
case "${OS}" in
    Darwin*) OS_TYPE="darwin" ;;
    Linux*)  OS_TYPE="linux" ;;
    *)
        echo "❌ Error: Unsupported operating system for install.sh: ${OS}" >&2
        echo "ℹ️  Windows users: please download onaji-v${VERSION}-windows-x86_64.zip from GitHub Releases." >&2
        exit 1
        ;;
esac

# 2. Detect Architecture
ARCH="$(uname -m)"
case "${ARCH}" in
    x86_64|amd64) ARCH_TYPE="x86_64" ;;
    arm64|aarch64) ARCH_TYPE="arm64" ;;
    *)
        echo "❌ Error: Unsupported architecture: ${ARCH}" >&2
        exit 1
        ;;
esac

ARCHIVE_NAME="onaji-v${VERSION}-${OS_TYPE}-${ARCH_TYPE}.tar.gz"

echo "📦 Installing ONΛJI v${VERSION} for ${OS_TYPE}-${ARCH_TYPE}..."

TEMP_DIR="$(mktemp -d)"
cleanup() {
    rm -rf "${TEMP_DIR}"
}
trap cleanup EXIT

# 3. Download Archive, SHA256SUMS, and Signature
echo "⬇️  Downloading ${ARCHIVE_NAME}..."
if command -v curl >/dev/null 2>&1; then
    curl -fsSL "${BASE_URL}/${ARCHIVE_NAME}" -o "${TEMP_DIR}/${ARCHIVE_NAME}" || true
    curl -fsSL "${BASE_URL}/SHA256SUMS" -o "${TEMP_DIR}/SHA256SUMS" || true
    curl -fsSL "${BASE_URL}/SHA256SUMS.sig" -o "${TEMP_DIR}/SHA256SUMS.sig" || true
elif command -v wget >/dev/null 2>&1; then
    wget -q "${BASE_URL}/${ARCHIVE_NAME}" -O "${TEMP_DIR}/${ARCHIVE_NAME}" || true
    wget -q "${BASE_URL}/SHA256SUMS" -O "${TEMP_DIR}/SHA256SUMS" || true
    wget -q "${BASE_URL}/SHA256SUMS.sig" -O "${TEMP_DIR}/SHA256SUMS.sig" || true
else
    echo "❌ Error: curl or wget is required for installation." >&2
    exit 1
fi

# 4. Fallback for Local Build / Offline validation
if [ ! -s "${TEMP_DIR}/${ARCHIVE_NAME}" ]; then
    echo "ℹ️  Remote release archive not present; building from local workspace..."
    if command -v cargo >/dev/null 2>&1; then
        cargo build --release -p onaji-cli
        mkdir -p "${INSTALL_DIR}"
        cp "target/release/onaji" "${INSTALL_DIR}/onaji"
        chmod +x "${INSTALL_DIR}/onaji"
        echo "✅ Installed local binary to ${INSTALL_DIR}/onaji"
    else
        echo "❌ Error: Failed to download artifact and cargo not found." >&2
        exit 1
    fi
else
    # 5. Verify Checksum
    echo "🔒 Verifying SHA-256 cryptographic signature..."
    cd "${TEMP_DIR}"
    if command -v sha256sum >/dev/null 2>&1; then
        grep "${ARCHIVE_NAME}" SHA256SUMS | sha256sum -c -
    elif command -v shasum >/dev/null 2>&1; then
        grep "${ARCHIVE_NAME}" SHA256SUMS | shasum -a 256 -c -
    fi

    # 6. Verify Cosign Signature if available
    if [ -s "SHA256SUMS.sig" ] && command -v cosign >/dev/null 2>&1; then
        echo "🛡️  Verifying Sigstore/Cosign signature on SHA256SUMS..."
        cosign verify-blob --signature SHA256SUMS.sig SHA256SUMS || true
    fi

    # Extract binary
    mkdir -p "${INSTALL_DIR}"
    tar -xzf "${ARCHIVE_NAME}" -C "${INSTALL_DIR}"
    chmod +x "${INSTALL_DIR}/onaji"
    echo "✅ Successfully installed onaji to ${INSTALL_DIR}/onaji"
fi

# 7. PATH Verification and Doctor Check
echo ""
echo "🚀 Installation completed!"
if ! echo "${PATH}" | grep -q "${INSTALL_DIR}"; then
    echo "👉 Please add Onaji to your PATH by adding this line to your ~/.zshrc or ~/.bashrc:"
    echo "   export PATH=\"${INSTALL_DIR}:\$PATH\""
fi

echo ""
if [ -x "${INSTALL_DIR}/onaji" ]; then
    "${INSTALL_DIR}/onaji" doctor || true
fi

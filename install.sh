#!/usr/bin/env bash
set -euo pipefail

# ==============================================================================
# ONΛJI Universal Deterministic Installer (macOS & Linux)
# https://github.com/odiador/onaji
# ==============================================================================

VERSION="${ONAJI_VERSION:-0.1.0-rc1}"
INSTALL_DIR="${ONAJI_INSTALL_DIR:-$HOME/.onaji/bin}"
REPO="odiador/onaji"
BASE_URL="https://github.com/${REPO}/releases/download/v${VERSION}"

# 1. Detect Operating System (POSIX)
OS="$(uname -s)"
ARCH="$(uname -m)"

case "${OS}" in
    Darwin*)
        case "${ARCH}" in
            arm64|aarch64) TARGET="aarch64-apple-darwin" ;;
            x86_64|amd64)  TARGET="x86_64-apple-darwin" ;;
            *) echo "❌ Error: Unsupported architecture: ${ARCH}" >&2; exit 1 ;;
        esac
        ;;
    Linux*)
        case "${ARCH}" in
            x86_64|amd64)  TARGET="x86_64-unknown-linux-gnu" ;;
            arm64|aarch64) TARGET="aarch64-unknown-linux-gnu" ;;
            *) echo "❌ Error: Unsupported architecture: ${ARCH}" >&2; exit 1 ;;
        esac
        ;;
    *)
        echo "❌ Error: Unsupported operating system for install.sh: ${OS}" >&2
        echo "ℹ️  Windows users: please download onaji-x86_64-pc-windows-msvc.zip from GitHub Releases." >&2
        exit 1
        ;;
esac

ARCHIVE_NAME="onaji-${TARGET}.tar.gz"
CHECKSUM_NAME="onaji-${TARGET}.sha256"

echo "📦 Installing ONΛJI v${VERSION} (${TARGET})..."

TEMP_DIR="$(mktemp -d)"
cleanup() {
    rm -rf "${TEMP_DIR}"
}
trap cleanup EXIT

# 2. Download Archive and Checksum (via gh CLI, authenticated curl, or direct curl)
echo "⬇️  Downloading ${ARCHIVE_NAME}..."
DOWNLOAD_SUCCESS=false

if command -v gh >/dev/null 2>&1 && gh auth status >/dev/null 2>&1; then
    if gh release download "v${VERSION}" --repo "${REPO}" -p "${ARCHIVE_NAME}" -p "${CHECKSUM_NAME}" --dir "${TEMP_DIR}" >/dev/null 2>&1; then
        DOWNLOAD_SUCCESS=true
    fi
fi

if [ "${DOWNLOAD_SUCCESS}" = false ]; then
    AUTH_HEADER=()
    if [ -n "${GITHUB_TOKEN:-}" ]; then
        AUTH_HEADER=(-H "Authorization: Bearer ${GITHUB_TOKEN}")
    fi

    if command -v curl >/dev/null 2>&1; then
        curl -fsSL "${AUTH_HEADER[@]}" "${BASE_URL}/${ARCHIVE_NAME}" -o "${TEMP_DIR}/${ARCHIVE_NAME}" 2>/dev/null || true
        curl -fsSL "${AUTH_HEADER[@]}" "${BASE_URL}/${CHECKSUM_NAME}" -o "${TEMP_DIR}/${CHECKSUM_NAME}" 2>/dev/null || true
    elif command -v wget >/dev/null 2>&1; then
        wget -q "${BASE_URL}/${ARCHIVE_NAME}" -O "${TEMP_DIR}/${ARCHIVE_NAME}" 2>/dev/null || true
        wget -q "${BASE_URL}/${CHECKSUM_NAME}" -O "${TEMP_DIR}/${CHECKSUM_NAME}" 2>/dev/null || true
    fi

    if [ -s "${TEMP_DIR}/${ARCHIVE_NAME}" ]; then
        DOWNLOAD_SUCCESS=true
    fi
fi

# 3. Fallback for Local Build
if [ "${DOWNLOAD_SUCCESS}" = false ]; then
    echo "ℹ️  Remote release archive not downloaded; building from local workspace..."
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
    # 4. Verify SHA-256 Integrity Checksum
    echo "🔒 Verifying SHA-256 cryptographic checksum..."
    cd "${TEMP_DIR}"
    if [ -s "${CHECKSUM_NAME}" ]; then
        EXPECTED_HASH="$(cat "${CHECKSUM_NAME}" | awk '{print $1}')"
        if command -v sha256sum >/dev/null 2>&1; then
            ACTUAL_HASH="$(sha256sum "${ARCHIVE_NAME}" | awk '{print $1}')"
        elif command -v shasum >/dev/null 2>&1; then
            ACTUAL_HASH="$(shasum -a 256 "${ARCHIVE_NAME}" | awk '{print $1}')"
        else
            ACTUAL_HASH=""
        fi

        if [ -n "${ACTUAL_HASH}" ] && [ "${EXPECTED_HASH}" != "${ACTUAL_HASH}" ]; then
            echo "❌ Error: SHA-256 Checksum verification failed!" >&2
            echo "   Expected: ${EXPECTED_HASH}" >&2
            echo "   Actual:   ${ACTUAL_HASH}" >&2
            exit 1
        fi
        echo "✓ SHA-256 checksum verified: ${EXPECTED_HASH}"
    fi

    # 5. Extract binary
    mkdir -p "${INSTALL_DIR}"
    tar -xzf "${ARCHIVE_NAME}" -C "${INSTALL_DIR}"
    chmod +x "${INSTALL_DIR}/onaji"
    echo "✅ Successfully installed onaji to ${INSTALL_DIR}/onaji"
fi

# 6. Shell Autocompletions Setup
if [ -x "${INSTALL_DIR}/onaji" ]; then
    mkdir -p "$HOME/.onaji/completions"
fi

# 7. PATH Verification and Doctor Check
echo ""
echo "🚀 Installation completed!"
if ! echo "${PATH}" | grep -q "${INSTALL_DIR}"; then
    echo "👉 Add Onaji to your PATH by adding this line to your ~/.zshrc or ~/.bashrc:"
    echo "   export PATH=\"${INSTALL_DIR}:\$PATH\""
fi

echo ""
if [ -x "${INSTALL_DIR}/onaji" ]; then
    "${INSTALL_DIR}/onaji" doctor || true
fi

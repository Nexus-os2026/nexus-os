#!/bin/bash
set -euo pipefail

# Nexus Code (nx) installer
VERSION="${NX_VERSION:-latest}"
INSTALL_DIR="${NX_INSTALL_DIR:-$HOME/.local/bin}"

OS=$(uname -s | tr '[:upper:]' '[:lower:]')
ARCH=$(uname -m)

case "$OS" in
    linux) PLATFORM="linux" ;;
    darwin) PLATFORM="macos" ;;
    *) echo "Unsupported OS: $OS"; exit 1 ;;
esac

case "$ARCH" in
    x86_64|amd64) ARCH="x86_64" ;;
    aarch64|arm64) ARCH="aarch64" ;;
    *) echo "Unsupported architecture: $ARCH"; exit 1 ;;
esac

BINARY_NAME="nx-${PLATFORM}-${ARCH}"
echo "Installing Nexus Code (nx) for ${PLATFORM}/${ARCH}..."

if [ "$VERSION" = "latest" ]; then
    DOWNLOAD_URL="https://github.com/nexaiceo/nexus-os/releases/latest/download/${BINARY_NAME}.tar.gz"
else
    DOWNLOAD_URL="https://github.com/nexaiceo/nexus-os/releases/download/${VERSION}/${BINARY_NAME}.tar.gz"
fi

mkdir -p "$INSTALL_DIR"
TMP_DIR=$(mktemp -d)
trap "rm -rf $TMP_DIR" EXIT

echo "Downloading..."
curl -sSL "$DOWNLOAD_URL" -o "${TMP_DIR}/${BINARY_NAME}.tar.gz"
tar xzf "${TMP_DIR}/${BINARY_NAME}.tar.gz" -C "$TMP_DIR"
mv "${TMP_DIR}/nx" "${INSTALL_DIR}/nx"
chmod +x "${INSTALL_DIR}/nx"

echo ""
echo "Installed to ${INSTALL_DIR}/nx"
echo ""
if ! echo "$PATH" | grep -q "$INSTALL_DIR"; then
    echo "Add to PATH: export PATH=\"${INSTALL_DIR}:\$PATH\""
    echo ""
fi
echo "Get started: nx doctor"

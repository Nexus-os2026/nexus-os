#!/usr/bin/env bash
set -euo pipefail

VERSION="${1:-1.0.0}"
OUT_DIR="target/package/macos"

mkdir -p "${OUT_DIR}"

echo "[macos] building nexus-cli release binary"
cargo build --release -p nexus-cli

echo "[macos] collecting Homebrew formula and launchd plist"
cp packaging/macos/homebrew/nexus-os.rb "${OUT_DIR}/nexus-os.rb"
cp packaging/macos/com.nexusos.agent.plist "${OUT_DIR}/com.nexusos.agent.plist"
cp target/release/nexus-cli "${OUT_DIR}/nexus-cli"

tar -C "${OUT_DIR}" -czf "target/package/nexus-os_${VERSION}_macos.tar.gz" .
echo "[macos] output: target/package/nexus-os_${VERSION}_macos.tar.gz"

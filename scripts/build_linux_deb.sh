#!/usr/bin/env bash
set -euo pipefail

VERSION="${1:-1.0.0}"
ARCH="${2:-amd64}"
PKG_NAME="nexus-os"
PKG_ROOT="target/package/${PKG_NAME}_${VERSION}_${ARCH}"
DEBIAN_DIR="${PKG_ROOT}/DEBIAN"

echo "[linux] building release binary"
cargo build --release -p nexus-cli

echo "[linux] assembling package tree at ${PKG_ROOT}"
rm -rf "${PKG_ROOT}"
mkdir -p "${DEBIAN_DIR}" \
  "${PKG_ROOT}/usr/bin" \
  "${PKG_ROOT}/lib/systemd/system"

cat > "${DEBIAN_DIR}/control" <<EOF
Package: ${PKG_NAME}
Version: ${VERSION}
Section: utils
Priority: optional
Architecture: ${ARCH}
Maintainer: NEXUS OS Team <release@nexus-os.dev>
Description: NEXUS OS governed agent runtime
EOF

cp target/release/nexus-cli "${PKG_ROOT}/usr/bin/nexus-cli"
cp packaging/linux/nexus-os.service "${PKG_ROOT}/lib/systemd/system/nexus-os.service"

if command -v dpkg-deb >/dev/null 2>&1; then
  echo "[linux] creating .deb via dpkg-deb"
  dpkg-deb --build "${PKG_ROOT}" "target/package/${PKG_NAME}_${VERSION}_${ARCH}.deb"
  echo "[linux] output: target/package/${PKG_NAME}_${VERSION}_${ARCH}.deb"
else
  echo "[linux] dpkg-deb not found; creating tarball fallback"
  tar -C target/package -czf "target/package/${PKG_NAME}_${VERSION}_${ARCH}.tar.gz" "$(basename "${PKG_ROOT}")"
  echo "[linux] output: target/package/${PKG_NAME}_${VERSION}_${ARCH}.tar.gz"
fi

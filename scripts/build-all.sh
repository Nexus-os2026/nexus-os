#!/bin/bash
echo "Building NexusOS for all platforms..."

# Build Rust workspace
cargo build --release

# Build Tauri app
cd app
npm install
npm run tauri build

echo "Build artifacts:"
echo "  Linux:   app/src-tauri/target/release/bundle/deb/"
echo "  macOS:   app/src-tauri/target/release/bundle/dmg/"
echo "  Windows: app/src-tauri/target/release/bundle/nsis/"
echo "           app/src-tauri/target/release/bundle/msi/"

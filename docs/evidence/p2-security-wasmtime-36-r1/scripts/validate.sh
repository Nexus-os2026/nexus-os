#!/usr/bin/env bash
# P2 security closure R1: local validation of the Wasmtime 36.0.17 candidate,
# in an isolated snapshot (the base plus the candidate's changed files, in a
# fresh scratch repository). Mirrors ci.yml's test-linux, test-frontend and
# test-python lanes; the security lane runs separately (scripts/security-audit.sh).
# Each command's output goes to its own file; commands.txt records every exit.
# Deviation: the workspace test run excludes nexus-verifier-sandbox, whose
# phase2_live_sandbox target would run live sandbox cases (transient systemd
# scopes) on this host; its other targets run individually and the live
# harness is built only. nexus-verifier-sandbox is not in Wasmtime's graph.
# Usage: validate.sh <snapshot> <output directory> <target directory> <node bin directory>
set -u
snap="$1"; out="$2"; export CARGO_TARGET_DIR="$3"; nodebin="$4"
mkdir -p "$out"
log="$out/commands.txt"
: > "$log"
cd "$snap" || exit 2
step() {
  local name="$1"; shift
  local started; started="$(date -u +%FT%TZ)"
  ( "$@" ) > "$out/$name.txt" 2>&1
  local rc=$?
  echo "$name exit=$rc started=$started :: $*" >> "$log"
}
step toolchain bash -c 'rustc --version; cargo --version; cargo fmt --version; cargo clippy --version'
step fmt-check cargo fmt --all -- --check
step clippy cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
step workspace-tests cargo test --workspace --locked --exclude nexus-verifier-sandbox
step verifier-sandbox-non-live cargo test -p nexus-verifier-sandbox --locked --lib --bins \
  --test phase2_cleanup_observation --test phase2_custody_codec --test phase2_custody_core \
  --test phase2_custody_store --test phase2_never_populated --test phase2_package_layout
step verifier-live-harness-build-only cargo test -p nexus-verifier-sandbox --locked \
  --features development-toolchain --test phase2_live_sandbox --no-run
step sdk-tests-all-features cargo test -p nexus-sdk --locked --all-features
step protocols-tests cargo test -p nexus-protocols --locked
step benchmarks-build cargo bench -p nexus-benchmarks --locked --no-run
step wasmtime-guard cargo test -p nexus-desktop-backend --locked --lib -- \
  phase0_surface::fg_reliability::tests::p0_fg_dep_wasmtime_uses_no_dynamic_component_val_api --exact
step phase-guards cargo test -p nexus-desktop-backend --locked --lib -- phase0_surface phase1_tests phase2_tests coding_flow::verification
step kernel-phase2 cargo test -p nexus-kernel --locked --lib -- p2b_ p2e_input p2h_
step webview-live-dev bash -c 'set -o pipefail; xvfb-run -a -s "-screen 0 1280x1024x24" cargo test -p nexus-desktop-backend --locked --test webview_boundary_live -- --ignored 2>&1 | tee "$0/webview-live-dev.log"; rc=$?; grep -q "^test webview_boundary_live ... ok$" "$0/webview-live-dev.log" && exit $rc; exit 1' "$out"
step builder-assemble env PATH="$nodebin:$PATH" node packaging/builder-toolchain/scripts/assemble.mjs \
  --target x86_64-unknown-linux-gnu --out app/src-tauri/builder-toolchain
step builder-entry-tests app/src-tauri/builder-toolchain/node/node --test packaging/builder-toolchain/test/entry.test.mjs
step builder-packaged-toolchain bash -c 'set -o pipefail; NEXUS_BUILDER_TOOLCHAIN=packaged cargo test -p nexus-desktop-backend --locked --lib -- p0_002c4d2_assembled_ p0_002c4c3_packaged_ 2>&1 | tee "$0/packaged-toolchain.log"; rc=$?; grep -q "test result: ok. 13 passed" "$0/packaged-toolchain.log" && exit $rc; exit 1' "$out"
step frontend-install bash -c 'cd app && npm ci'
step frontend-tsc bash -c 'cd app && npx tsc --noEmit'
step frontend-tests bash -c 'cd app && npm test'
step frontend-build bash -c 'cd app && npx vite build'
step webview-live-release bash -c 'set -o pipefail; (cd app && npm run build) || exit 1; xvfb-run -a -s "-screen 0 1280x1024x24" cargo test -p nexus-desktop-backend --locked --features tauri/custom-protocol --test webview_boundary_live -- --ignored 2>&1 | tee "$0/webview-live-release.log"; rc=$?; grep -q "^\[live\] tauri profile: release" "$0/webview-live-release.log" && grep -q "^test webview_boundary_live ... ok$" "$0/webview-live-release.log" && exit $rc; exit 1' "$out"
step python-voice bash -c 'set -euo pipefail; python3 --version; python3 -m venv "$0/py" && "$0/py/bin/python" -m pip install -q pip==26.2.1 && "$0/py/bin/python" -m pip install -q --require-hashes --no-deps -r voice/requirements-linux-py311.lock && "$0/py/bin/python" -m pip check && cd voice && "$0/py/bin/python" -m pytest -v' "$out"
cat "$log"

#!/usr/bin/env bash
# P2-V1-R3B-I4-Q1 non-live validation (mission section 19), in order, each
# command with its exit status and complete output; the toolchain; the
# identity of the checkout; the SHA-256 of every source it validated. The
# live harness is built, never run: the authoritative live execution is the
# exact-SHA workflow. Nothing here contacts a systemd bus, creates a cgroup,
# runs a live sandbox, opens a real store or uses a runner.
# Usage: validate.sh <checkout> <output directory>   (CARGO_TARGET_DIR set by the caller)
set -u
checkout="$1"
out="$2"
mkdir -p "$out"
cd "$checkout" || exit 2
summary="$out/commands.txt"
: > "$summary"
{
  echo "rustc: $(rustc -V)"
  echo "cargo: $(cargo -V)"
  echo "clippy: $(cargo clippy -V)"
  echo "rustfmt: $(rustfmt -V)"
  echo "python3: $(python3 -V 2>&1)"
  echo "host kernel (uname -sr): $(uname -sr)"
  echo "HEAD: $(git rev-parse HEAD)"
  echo "HEAD tree: $(git rev-parse 'HEAD^{tree}')"
  echo "status entries: $(git status --porcelain=v1 --untracked-files=all | wc -l)"
  echo "CARGO_TARGET_DIR: ${CARGO_TARGET_DIR:-(unset)}"
} > "$out/toolchain.txt"
{
  find crates/nexus-verifier-sandbox/src crates/nexus-verifier-sandbox/tests -type f -name '*.rs' -print
  printf '%s\n' \
    crates/nexus-verifier-sandbox/Cargo.toml \
    .github/workflows/ci-phase2-linux-sandbox.yml \
    scripts/ci/test_phase2_cleanup_check.py \
    app/src-tauri/src/phase2_tests.rs \
    app/src-tauri/src/coding_flow/verification.rs \
    docs/security/phase2-governed-verification.md \
    Cargo.toml Cargo.lock rust-toolchain.toml AGENTS.md CLAUDE.md
  find docs/evidence/p2-v1-r3b-i4-q1-host-live-qualification/scripts -type f -print
} | LC_ALL=C sort -u | xargs sha256sum > "$out/sources.SHA256SUMS"
run() {
  local name="$1"
  shift
  local started
  started=$(date -u +%Y-%m-%dT%H:%M:%SZ)
  "$@" > "$out/$name.txt" 2>&1
  local status=$?
  printf '%s exit=%d started=%s :: %s\n' "$name" "$status" "$started" "$*" >> "$summary"
  return 0
}
run fmt-check cargo fmt --all -- --check
run diff-check git diff --check
run clippy cargo clippy --locked -p nexus-verifier-sandbox --lib --tests -- -D warnings
run lib-tests cargo test --locked -p nexus-verifier-sandbox --lib
run store-tests cargo test --locked -p nexus-verifier-sandbox --test phase2_custody_store
run codec-tests cargo test --locked -p nexus-verifier-sandbox --test phase2_custody_codec
run core-tests cargo test --locked -p nexus-verifier-sandbox --test phase2_custody_core
run cleanup-observation cargo test --locked -p nexus-verifier-sandbox --test phase2_cleanup_observation
run package-layout cargo test --locked -p nexus-verifier-sandbox --test phase2_package_layout
run live-harness-build-only cargo test --locked -p nexus-verifier-sandbox --features development-toolchain --test phase2_live_sandbox --no-run
run live-step-fixture-controls python3 scripts/ci/test_phase2_cleanup_check.py
run desktop-check cargo check --locked -p nexus-desktop-backend --lib --tests
run desktop-phase2-guards cargo test --locked -p nexus-desktop-backend --lib -- phase2_tests::
run lib-test-list cargo test --locked -p nexus-verifier-sandbox --lib -- --list
cat "$summary"

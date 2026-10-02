#!/usr/bin/env bash
# P2-V1-R3B-I4 validation: the mission's section 23 commands, in order, each
# with its exit status and complete output; the toolchain; the identity of
# the checkout; and the SHA-256 of every source it validated.
# Usage: validate.sh <checkout> <output directory>
# It builds and runs the sandbox crate's unit tests (over the deterministic
# simulation of the user manager and the kernel) and the named integration
# targets; the live harness is built, never run (--no-run). Nothing here
# contacts a systemd bus, creates a cgroup, runs a live sandbox, opens a
# real store, or uses a runner.
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
# The validated sources, by SHA-256 (paths relative to the checkout).
{
  find crates/nexus-verifier-sandbox/src -type f -name '*.rs' -print
  find crates/nexus-verifier-sandbox/tests -type f -name '*.rs' -print
  printf '%s\n' \
    crates/nexus-verifier-sandbox/Cargo.toml \
    crates/nexus-verifier-sandbox/build.rs \
    docs/security/phase2-governed-verification.md \
    Cargo.toml Cargo.lock rust-toolchain.toml AGENTS.md CLAUDE.md
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
run lib-tests cargo test --locked -p nexus-verifier-sandbox --lib
run store-tests cargo test --locked -p nexus-verifier-sandbox --test phase2_custody_store
run codec-tests cargo test --locked -p nexus-verifier-sandbox --test phase2_custody_codec
run core-tests cargo test --locked -p nexus-verifier-sandbox --test phase2_custody_core
run cleanup-observation cargo test --locked -p nexus-verifier-sandbox --test phase2_cleanup_observation
run package-layout cargo test --locked -p nexus-verifier-sandbox --test phase2_package_layout
run live-harness-build-only cargo test --locked -p nexus-verifier-sandbox --test phase2_live_sandbox --no-run
run clippy cargo clippy --locked -p nexus-verifier-sandbox --lib --test phase2_custody_store --test phase2_live_sandbox -- -D warnings
run fmt-check cargo fmt --all -- --check
run diff-check git diff --check
# Beyond section 23: the unit-test code itself under clippy, and the full
# unit-test list (names only).
run clippy-unit-tests cargo clippy --locked -p nexus-verifier-sandbox --lib --tests -- -D warnings
run lib-test-list cargo test --locked -p nexus-verifier-sandbox --lib -- --list
cat "$summary"

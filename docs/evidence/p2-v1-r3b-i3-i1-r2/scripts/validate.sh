#!/usr/bin/env bash
# P2-V1-R3B-I3-I1-R2 validation: the mission's section 17 commands, in
# order, each with its exit status and complete output; the toolchain; the
# identity of the checkout; and the SHA-256 of every source it validated.
# Usage: validate.sh <checkout> <output directory>
# It builds and runs only the named custody targets and the cleanup
# observation target; the live harness is built, never run (--no-run).
# Nothing here opens a real store, qualifies a host or device, mounts,
# performs privileged I/O, uses a bus or a runner, or runs a live sandbox.
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
} > "$out/toolchain.txt"
# The validated sources, by SHA-256 (paths relative to the checkout).
{
  find crates/nexus-verifier-sandbox/tests/support/custody -type f -name '*.rs' -print
  printf '%s\n' \
    crates/nexus-verifier-sandbox/tests/phase2_custody_store.rs \
    crates/nexus-verifier-sandbox/tests/phase2_custody_core.rs \
    crates/nexus-verifier-sandbox/tests/phase2_custody_codec.rs \
    crates/nexus-verifier-sandbox/tests/phase2_cleanup_observation.rs \
    crates/nexus-verifier-sandbox/tests/support/cleanup_observation.rs \
    crates/nexus-verifier-sandbox/tests/phase2_live_sandbox.rs \
    crates/nexus-verifier-sandbox/Cargo.toml \
    Cargo.toml Cargo.lock rust-toolchain.toml
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
run store-tests cargo test --locked -p nexus-verifier-sandbox --test phase2_custody_store
run codec-tests cargo test --locked -p nexus-verifier-sandbox --test phase2_custody_codec
run core-tests cargo test --locked -p nexus-verifier-sandbox --test phase2_custody_core
run clippy cargo clippy --locked -p nexus-verifier-sandbox --test phase2_custody_store --test phase2_custody_codec --test phase2_custody_core -- -D warnings
run cleanup-observation cargo test --locked -p nexus-verifier-sandbox --test phase2_cleanup_observation
run live-harness-build-only cargo test --locked -p nexus-verifier-sandbox --test phase2_live_sandbox --no-run
run fmt-check cargo fmt --all -- --check
run diff-check git diff --check
run store-tests-nocapture cargo test --locked -p nexus-verifier-sandbox --test phase2_custody_store -- --nocapture --test-threads=1
cat "$summary"

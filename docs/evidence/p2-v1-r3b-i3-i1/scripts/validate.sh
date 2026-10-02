#!/usr/bin/env bash
# P2-V1-R3B-I3-I1 validation: the mission's section 8 commands, in order,
# each with its exit status and complete output, and the toolchain.
# Usage: validate.sh <checkout> <output directory>
# It builds and runs only the named custody targets and the cleanup
# observation target; the live harness is built, never run (--no-run).
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
  echo "branch: $(git rev-parse --abbrev-ref HEAD)"
} > "$out/toolchain.txt"
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

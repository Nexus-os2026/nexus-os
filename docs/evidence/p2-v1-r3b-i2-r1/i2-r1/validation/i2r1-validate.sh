#!/usr/bin/env bash
# Run the I2-R1 required validation from the repository root of a worktree
# whose HEAD must be the given candidate, logging every command with its exit
# status. The package's own build artifacts are removed first (dependencies
# are kept), so every target is compiled and linted from the checked-out
# sources rather than reused. The live harness is built, never executed.
#
# Usage: validate.sh <worktree> <expected HEAD commit> <log file>
set -u
worktree=$1
expected=$2
log=$3

run() {
  echo "=== \$ $*"
  "$@"
  local status=$?
  echo "=== exit $status"
  [ "$status" -eq 0 ] || failed=$((failed + 1))
}

cd "$worktree" || exit 2
failed=0
{
  echo "# validation  $(date -u +%Y-%m-%dT%H:%M:%SZ)"
  echo "repository root: $(git rev-parse --show-toplevel)"
  echo "branch: $(git rev-parse --abbrev-ref HEAD)"
  echo "HEAD: $(git rev-parse HEAD)  tree: $(git rev-parse 'HEAD^{tree}')"
  echo "expected HEAD: $expected"
  if [ "$(git rev-parse HEAD)" != "$expected" ]; then
    echo "RESULT: HEAD is not the expected candidate; nothing run"
    exit 2
  fi
  echo "worktree entries before (status --porcelain, untracked included): $(git status --porcelain=v1 --untracked-files=all | wc -l)"
  echo "toolchain: $(rustc -V) | $(cargo -V) | $(cargo clippy -V) | $(rustfmt -V)"
  echo "host: $(uname -srm)"
  run cargo clean --locked -p nexus-verifier-sandbox
  run cargo test --locked -p nexus-verifier-sandbox --test phase2_custody_codec
  run cargo test --locked -p nexus-verifier-sandbox --test phase2_custody_core
  run cargo clippy --locked -p nexus-verifier-sandbox --test phase2_custody_codec --test phase2_custody_core -- -D warnings
  run cargo test --locked -p nexus-verifier-sandbox --test phase2_cleanup_observation
  run cargo test --locked -p nexus-verifier-sandbox --test phase2_live_sandbox --no-run
  run cargo fmt --all -- --check
  run git diff --check
  echo "worktree entries after (status --porcelain, untracked included): $(git status --porcelain=v1 --untracked-files=all | wc -l)"
  [ -z "$(git status --porcelain=v1 --untracked-files=all)" ] || failed=$((failed + 1))
  echo "HEAD after: $(git rev-parse HEAD)"
  [ "$(git rev-parse HEAD)" = "$expected" ] || failed=$((failed + 1))
  echo "RESULT: $failed failure(s)"
  [ "$failed" -eq 0 ]
} 2>&1 | tee "$log"
exit "${PIPESTATUS[0]}"

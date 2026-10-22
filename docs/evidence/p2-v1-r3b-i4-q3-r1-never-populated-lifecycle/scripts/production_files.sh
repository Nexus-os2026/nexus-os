#!/usr/bin/env bash
# P2-V1-R3B-I4-Q3-R1: every production and governance file, the worktree
# against the base, by SHA-256 (the mission's production change envelope:
# none expected). Production: every non-test source of the sandbox crate
# (src/, without scope/tests.rs and execution/tests.rs), its manifest, the
# workspace manifest and lock file, the toolchain pin, the live workflow and
# its step's fixture controls, the desktop's verification module, the live
# harness's probe and checked observation, AGENTS.md and CLAUDE.md.
# Usage: production_files.sh <repository> <base> <output file>
set -eu
repo="$1"; base="$2"; out="$3"
cd "$repo"
{
  echo "P2-V1-R3B-I4-Q3-R1: production and governance files, worktree vs the base $(git rev-parse "$base") ($(date -u +%FT%TZ))"
  {
    git ls-tree -r --name-only "$base" crates/nexus-verifier-sandbox/src \
      | grep -v -e '^crates/nexus-verifier-sandbox/src/scope/tests\.rs$' -e '^crates/nexus-verifier-sandbox/src/execution/tests\.rs$'
    printf '%s\n' crates/nexus-verifier-sandbox/Cargo.toml crates/nexus-verifier-sandbox/build.rs Cargo.toml Cargo.lock \
      rust-toolchain.toml .github/workflows/ci-phase2-linux-sandbox.yml scripts/ci/test_phase2_cleanup_check.py \
      app/src-tauri/src/coding_flow/verification.rs \
      crates/nexus-verifier-sandbox/tests/support/host_qualification.rs \
      crates/nexus-verifier-sandbox/tests/support/cleanup_observation.rs AGENTS.md CLAUDE.md
  } | LC_ALL=C sort -u | while IFS= read -r path; do
    [ -e "$path" ] || git cat-file -e "$base:$path" 2>/dev/null || continue
    b=$(git show "$base:$path" 2>/dev/null | sha256sum | cut -d' ' -f1)
    w=$(sha256sum < "$path" | cut -d' ' -f1)
    if [ "$b" = "$w" ]; then echo "same-as-base $w $path"; else echo "CHANGED base=$b worktree=$w $path"; fi
  done
} > "$out"
echo "$(grep -c '^same-as-base' "$out") same as base, $(grep -c '^CHANGED' "$out") changed"

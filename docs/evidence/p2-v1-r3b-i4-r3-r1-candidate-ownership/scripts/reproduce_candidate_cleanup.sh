#!/usr/bin/env bash
# P2-V1-R3B-I4-R3-R1: the candidate-cleanup defect, reproduced on the base's
# own deterministic model and production code. A scratch checkout of <base>
# gets reproduction/candidate_cleanup_on_the_base.rs appended to
# crates/nexus-verifier-sandbox/src/execution/tests.rs; only its tests run
# (the model only: no bus, no cgroup, no live sandbox); the file is written
# back byte for byte and the checkout's status must be clean again.
# Usage: reproduce_candidate_cleanup.sh <repository> <base> <reproduction.rs> <scratch (must not exist)> <output file>
set -u
repo="$1"; base="$2"; repro="$3"; scratch="$4"; out="$5"
[ ! -e "$scratch" ] || { echo "exists: $scratch" >&2; exit 2; }
mkdir -p "$scratch/checkout" "$scratch/target"
git -C "$repo" archive "$base" | tar -x -C "$scratch/checkout"
cd "$scratch/checkout" || exit 2
git init -q && git add -f -A && git -c user.name=scratch -c user.email=scratch@localhost commit -q -m "base snapshot"
file=crates/nexus-verifier-sandbox/src/execution/tests.rs
before=$(sha256sum "$file" | cut -d' ' -f1)
cp -p "$file" "$scratch/tests.rs.orig"
{
  echo "reproduction of the P2-V1-R3B-I4-R3-R1 candidate-cleanup defect, $(date -u +%FT%TZ)"
  echo "base: $base (snapshot tree $(git rev-parse 'HEAD^{tree}'))"
  echo "production pending.rs sha256: $(sha256sum crates/nexus-verifier-sandbox/src/scope/pending.rs | cut -d' ' -f1)"
  echo "reproduction source sha256: $(sha256sum "$repro" | cut -d' ' -f1)"
  echo "execution/tests.rs sha256 before: $before"
} > "$out"
cat "$repro" >> "$file"
CARGO_TARGET_DIR="$scratch/target" cargo test --locked -p nexus-verifier-sandbox --lib -- \
  execution::tests::r3r1_reproduction --nocapture --test-threads=1 >> "$out" 2>&1
status=$?
cp -p "$scratch/tests.rs.orig" "$file"
after=$(sha256sum "$file" | cut -d' ' -f1)
{
  echo "cargo test exit: $status"
  echo "execution/tests.rs sha256 after restore: $after (identical: $([ "$before" = "$after" ] && echo yes || echo NO))"
  echo "checkout status entries after restore: $(git status --porcelain=v1 --untracked-files=all | wc -l)"
} >> "$out"
tail -4 "$out"
[ "$status" -eq 0 ] && [ "$before" = "$after" ] && [ -z "$(git status --porcelain=v1 --untracked-files=all)" ]

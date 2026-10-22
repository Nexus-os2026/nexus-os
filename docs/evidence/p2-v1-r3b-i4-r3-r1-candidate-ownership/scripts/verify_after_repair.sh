#!/usr/bin/env bash
# P2-V1-R3B-I4-R3-R1: the before-repair reproduction, run unchanged against
# the candidate. reproduction/candidate_cleanup_on_the_base.rs is appended to
# crates/nexus-verifier-sandbox/src/execution/tests.rs of an isolated clean
# checkout of the candidate (a snapshot), only its tests run (the model only:
# no bus, no cgroup, no live sandbox), and the file is written back byte for
# byte; the checkout's status must be clean again. On the candidate both
# tests must fail at their own "not reproduced" assertions (A1, B1): the
# printed fates show what happened to the foreign cgroups instead.
# Usage: verify_after_repair.sh <clean candidate checkout> <reproduction.rs> <target dir (must not exist)> <output file>
set -u
checkout="$(cd "$1" && pwd)"; repro="$2"; target="$3"; out="$4"
[ ! -e "$target" ] || { echo "exists: $target" >&2; exit 2; }
[ -z "$(git -C "$checkout" status --porcelain=v1 --untracked-files=all)" ] || { echo "not clean: $checkout" >&2; exit 2; }
mkdir -p "$target"
cd "$checkout" || exit 2
file=crates/nexus-verifier-sandbox/src/execution/tests.rs
before=$(sha256sum "$file" | cut -d' ' -f1)
original="$target/tests.rs.orig"
cp -p "$file" "$original"
{
  echo "the P2-V1-R3B-I4-R3-R1 reproduction against the candidate, $(date -u +%FT%TZ)"
  echo "candidate snapshot: HEAD $(git rev-parse HEAD), tree $(git rev-parse 'HEAD^{tree}')"
  echo "production pending.rs sha256: $(sha256sum crates/nexus-verifier-sandbox/src/scope/pending.rs | cut -d' ' -f1)"
  echo "reproduction source sha256: $(sha256sum "$repro" | cut -d' ' -f1)"
  echo "execution/tests.rs sha256 before: $before"
} > "$out"
cat "$repro" >> "$file"
CARGO_TARGET_DIR="$target" cargo test --locked -p nexus-verifier-sandbox --lib -- \
  execution::tests::r3r1_reproduction --nocapture --test-threads=1 >> "$out" 2>&1
status=$?
cp -p "$original" "$file"
after=$(sha256sum "$file" | cut -d' ' -f1)
a=$(grep -c 'A1 not reproduced' "$out")
b=$(grep -c 'B1 not reproduced' "$out")
{
  echo "cargo test exit: $status (expected 101: both tests fail)"
  echo "A fails at its own assertion (\"A1 not reproduced\"): $([ "$a" -ge 1 ] && echo yes || echo NO)"
  echo "B fails at its own assertion (\"B1 not reproduced\"): $([ "$b" -ge 1 ] && echo yes || echo NO)"
  echo "test result line: $(grep '^test result' "$out" | tail -1)"
  echo "execution/tests.rs sha256 after restore: $after (identical: $([ "$before" = "$after" ] && echo yes || echo NO))"
  echo "checkout status entries after restore: $(git status --porcelain=v1 --untracked-files=all | wc -l)"
} >> "$out"
tail -6 "$out"
[ "$status" -eq 101 ] && [ "$a" -ge 1 ] && [ "$b" -ge 1 ] \
  && grep -q '^test result: FAILED. 0 passed; 2 failed' "$out" \
  && [ "$before" = "$after" ] && [ -z "$(git status --porcelain=v1 --untracked-files=all)" ]

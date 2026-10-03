#!/usr/bin/env bash
# P2-V1-R3B-I4-R2: the defect, shown by the candidate's own tests against the
# unrepaired production code. A scratch checkout of the base (c3a12b2b: the
# production code of I4-R1, unchanged since ea475eac) gets exactly the
# candidate's two test files (the scope model and the execution tests);
# every production source stays the base's. The R2 tests and the three tests
# R2 tightened run there; the defect shows as their failures (a stop by the
# unit's name without a recorded start reply). Nothing here contacts a bus or
# creates a cgroup.
# Usage: reproduce_before_repair.sh <repository> <base> <candidate checkout> <scratch (must not exist)> <output file>
set -u
repo="$1"; base="$2"; candidate="$3"; scratch="$4"; out="$5"
[ ! -e "$scratch" ] || { echo "exists: $scratch" >&2; exit 2; }
mkdir -p "$scratch/checkout" "$scratch/target"
git -C "$repo" archive "$base" | tar -x -C "$scratch/checkout"
cd "$scratch/checkout" || exit 2
git init -q && git add -f -A && git -c user.name=scratch -c user.email=scratch@localhost commit -q -m "base snapshot"
tests="crates/nexus-verifier-sandbox/src/scope/tests.rs crates/nexus-verifier-sandbox/src/execution/tests.rs"
for f in $tests; do cp -p "$candidate/$f" "$f"; done
{
  echo "the R2 tests against the unrepaired production code, $(date -u +%FT%TZ)"
  echo "base: $base (production code unchanged since ea475eac)"
  for f in $tests; do echo "test file from the candidate: $f sha256=$(sha256sum "$f" | cut -d' ' -f1)"; done
  echo "production pending.rs: sha256=$(sha256sum crates/nexus-verifier-sandbox/src/scope/pending.rs | cut -d' ' -f1) (the base's)"
  echo "changed files vs the base: $(git status --porcelain=v1 --untracked-files=all | wc -l) ($(git status --porcelain=v1 | cut -c4- | tr '\n' ' '))"
} > "$out"
CARGO_TARGET_DIR="$scratch/target" cargo test --locked -p nexus-verifier-sandbox --lib -- \
  i4r2_ i4r1_01_ i4r1_12_ i4q1r1_nc2_ >> "$out" 2>&1
echo "cargo test exit: $?" >> "$out"
grep -E "^test .* (ok|FAILED)$|^test result" "$out" | sort

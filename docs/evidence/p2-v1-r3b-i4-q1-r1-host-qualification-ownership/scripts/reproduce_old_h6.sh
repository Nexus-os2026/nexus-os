#!/usr/bin/env bash
# P2-V1-R3B-I4-Q1-R1: reproduce the Architect's H6 finding on the
# deterministic scope model, against the exact base the finding is about.
#
# A scratch checkout of <base> (git archive, one scratch commit, so its
# status is clean) gets reproduction/old_h6_on_the_model.rs appended to
# crates/nexus-verifier-sandbox/src/scope/tests.rs; only its tests run (the
# model only: no bus, no cgroup, no live sandbox); the file is then written
# back byte for byte and the checkout's status must be clean again.
# Usage: reproduce_old_h6.sh <repository> <base> <reproduction.rs> <scratch dir (must not exist)> <output file>
set -u
repo="$1"; base="$2"; repro="$3"; scratch="$4"; out="$5"
[ ! -e "$scratch" ] || { echo "exists: $scratch" >&2; exit 2; }
mkdir -p "$scratch/checkout" "$scratch/target"
git -C "$repo" archive "$base" | tar -x -C "$scratch/checkout"
cd "$scratch/checkout" || exit 2
git init -q && git add -f -A && git -c user.name=scratch -c user.email=scratch@localhost commit -q -m "base snapshot"
file=crates/nexus-verifier-sandbox/src/scope/tests.rs
before=$(sha256sum "$file" | cut -d' ' -f1)
cp -p "$file" "$scratch/tests.rs.orig"
{
  echo "reproduction of the P2-V1-R3B-I4-Q1 H6 finding, $(date -u +%FT%TZ)"
  echo "base: $base (snapshot tree $(git rev-parse 'HEAD^{tree}'))"
  echo "reproduction source sha256: $(sha256sum "$repro" | cut -d' ' -f1)"
  echo "scope/tests.rs sha256 before: $before"
} > "$out"
cat "$repro" >> "$file"
# Scratch-only model extension for the residual (E): a collision's reply can
# be lost like any other (the default, a delivered reply, is unchanged).
python3 - "$file" <<'EOF'
import sys
path = sys.argv[1]
text = open(path).read()
anchor = "            return Started::Collision;\n        }\n        assert!(\n"
assert text.count(anchor) == 1, "model anchor"
text = text.replace(anchor,
    "            // Scratch-only (reproduction E): a collision's reply can be lost.\n"
    "            return match state.start_reply {\n"
    "                Reply::Delivered => Started::Collision,\n"
    "                reply => Started::Uncertain(reply.reason(\"StartTransientUnit\")),\n"
    "            };\n        }\n        assert!(\n", 1)
open(path, "w").write(text)
EOF
echo "scratch model extension applied (reproduction E)" >> "$out"
CARGO_TARGET_DIR="$scratch/target" cargo test --locked -p nexus-verifier-sandbox --lib -- \
  scope::tests::q1r1_reproduction --nocapture --test-threads=1 >> "$out" 2>&1
status=$?
cp -p "$scratch/tests.rs.orig" "$file"
after=$(sha256sum "$file" | cut -d' ' -f1)
{
  echo "cargo test exit: $status"
  echo "scope/tests.rs sha256 after restore: $after (identical: $([ "$before" = "$after" ] && echo yes || echo NO))"
  echo "checkout status entries after restore: $(git status --porcelain=v1 --untracked-files=all | wc -l)"
} >> "$out"
tail -4 "$out"
[ "$status" -eq 0 ] && [ "$before" = "$after" ] && [ -z "$(git status --porcelain=v1 --untracked-files=all)" ]

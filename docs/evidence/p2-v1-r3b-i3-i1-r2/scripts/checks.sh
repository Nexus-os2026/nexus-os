#!/usr/bin/env bash
# P2-V1-R3B-I3-I1-R2 repository checks, run in the repository itself (they
# read Git objects and history; they write nothing there): the R5 design
# model and reference self-check (bytecode writes disabled, the cited Linux
# v6.17 files verified by SHA-256), every historical evidence manifest
# against its own snapshot (R1's evidence included), and the R2 scope check
# against the base.
# Usage: checks.sh <repository> <output directory> <kernel directory>
set -u
repo="$1"
out="$2"
kernel="$3"
mkdir -p "$out"
cd "$repo" || exit 2
summary="$out/checks.txt"
: > "$summary"
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
export PYTHONDONTWRITEBYTECODE=1
run r5-design-checks python3 -B docs/evidence/p2-v1-r3b-i3-p-r5/design_checks.py --json "$out/design_checks.json"
run r5-design-checks-compare cmp "$out/design_checks.json" docs/evidence/p2-v1-r3b-i3-p-r5/coverage.json
run r5-reference-check python3 -B docs/evidence/p2-v1-r3b-i3-p-r5/reference_check.py --self-test --kernel "$kernel"
run historical-manifests python3 -B docs/evidence/p2-v1-r3b-i3-i1/scripts/manifests.py "$repo" \
  docs/evidence/p2-v1-r3b-i3-p-r1 docs/evidence/p2-v1-r3b-i3-p-r2 docs/evidence/p2-v1-r3b-i3-p-r3 \
  docs/evidence/p2-v1-r3b-i3-p-r4 docs/evidence/p2-v1-r3b-i3-p-r5 docs/evidence/p2-v1-r3b-i3-i1 \
  docs/evidence/p2-v1-r3b-i3-i1-r1
run scope python3 -B docs/evidence/p2-v1-r3b-i3-i1-r2/scripts/scope.py "$repo" ed7c7088247badf87b3b1d483ee57360f867e2f1
cat "$summary"

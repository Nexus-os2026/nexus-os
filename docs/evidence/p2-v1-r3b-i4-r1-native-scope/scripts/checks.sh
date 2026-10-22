#!/usr/bin/env bash
# P2-V1-R3B-I4-R1 repository checks, run in the repository itself (they read
# Git objects, history and files; they write nothing there):
#   - every historical evidence manifest against its own snapshot (R3's and
#     I4's included), with I3-I1's runner unchanged; an entry inside its
#     directory must also equal the working file, so the I4 evidence is
#     unchanged;
#   - the I4-R1 scope check against the base;
#   - the accepted custody store's immutable-hash verification (I4's
#     script, unchanged);
#   - the reruns of the accepted suites (from the candidate runs) compared
#     with their as-run results when R3 was accepted (I4's and R3's
#     comparison scripts, unchanged).
# Usage: checks.sh <repository> <candidate runs directory> <output directory>
set -u
repo="$1"
runs="$2"
out="$3"
mkdir -p "$out"
cd "$repo" || exit 2
E=docs/evidence/p2-v1-r3b-i4-r1-native-scope/scripts
I4=docs/evidence/p2-v1-r3b-i4-native-scope/scripts
R3=docs/evidence/p2-v1-r3b-i3-i1-r3
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
run historical-manifests python3 -B docs/evidence/p2-v1-r3b-i3-i1/scripts/manifests.py "$repo" \
  docs/evidence/p2-v1-r3b-i3-p-r1 docs/evidence/p2-v1-r3b-i3-p-r2 docs/evidence/p2-v1-r3b-i3-p-r3 \
  docs/evidence/p2-v1-r3b-i3-p-r4 docs/evidence/p2-v1-r3b-i3-p-r5 docs/evidence/p2-v1-r3b-i3-i1 \
  docs/evidence/p2-v1-r3b-i3-i1-r1 docs/evidence/p2-v1-r3b-i3-i1-r2 docs/evidence/p2-v1-r3b-i3-i1-r3 \
  docs/evidence/p2-v1-r3b-i4-native-scope
run scope python3 -B "$E/scope.py" "$repo" bb0dfec0338273ddf187b1dfcb22cd4b8eb8b2f9
run custody-hashes python3 -B "$I4/custody_hashes.py" "$repo"
run r3-store-comparison python3 -B "$I4/compare_reruns.py" store \
  "$R3/controls/store/summary.json" "$runs/controls/r3-store/summary.json"
run i2r1-comparison python3 -B "$R3/scripts/compare_i2r1.py" \
  "$R3/controls/i2r1-rerun/summary.json" "$runs/controls/i2r1-rerun/summary.json"
run r3-api-comparison python3 -B "$I4/compare_reruns.py" api \
  "$R3/controls/api-probes/api_probes.json" "$runs/controls/r3-api-probes/api_probes.json"
cat "$summary"

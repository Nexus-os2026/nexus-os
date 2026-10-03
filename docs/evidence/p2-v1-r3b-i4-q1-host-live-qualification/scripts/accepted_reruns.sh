#!/usr/bin/env bash
# P2-V1-R3B-I4-Q1: the accepted suites, rerun unchanged on the candidate, each
# with its own empty target directory, in one clean checkout of the candidate
# (nothing else may build, test or edit there):
#   1. the I4-R1 behavioural controls (38: the 21 accepted I4 controls and the
#      17 I4-R1 controls; they carry section 18's items 1-5, 8, 9 and 11);
#   2. the I4-R1 harness-build API/type guards (27);
#   3. the I4-R1 normal-build API guards (9; the "removed" self-checks build
#      against bb0dfec0, where the removed items existed);
#   4. the I4-R1 source guards (14, with self-tests);
#   5. the accepted custody store's immutable-hash verification (I4's script);
#   6. the accepted R3 store controls (179), R3's runner unchanged;
#   7. the I2-R1 control rerun (32 + 4), I3-I1's runner unchanged;
#   8. R3's API probes (28), R3's runner unchanged.
# Usage: accepted_reruns.sh <clean checkout> <output directory> <targets directory (must not exist)> <repository>
set -u
checkout="$(cd "$1" && pwd)"
out="$2"
targets="$3"
repo="$4"
[ ! -e "$targets" ] || { echo "exists: $targets" >&2; exit 2; }
mkdir -p "$out" "$targets"
out="$(cd "$out" && pwd)"
targets="$(cd "$targets" && pwd)"
r1="$checkout/docs/evidence/p2-v1-r3b-i4-r1-native-scope/scripts"
i4="$checkout/docs/evidence/p2-v1-r3b-i4-native-scope/scripts"
r3="$checkout/docs/evidence/p2-v1-r3b-i3-i1-r3/scripts"
export PYTHONDONTWRITEBYTECODE=1
log="$out/runs.txt"
: > "$log"
state() {
  printf '%s status entries: %s\n' "$1" \
    "$(git -C "$checkout" status --porcelain=v1 --untracked-files=all | wc -l)" >> "$log"
}
step() {
  local name="$1"
  shift
  local started
  started=$(date -u +%Y-%m-%dT%H:%M:%SZ)
  "$@" > "$out/$name.out" 2>&1
  local status=$?
  printf '%s exit=%d started=%s :: %s\n' "$name" "$status" "$started" "$*" >> "$log"
  state "after $name"
}
{
  echo "checkout: $checkout"
  echo "HEAD: $(git -C "$checkout" rev-parse HEAD)"
  echo "HEAD tree: $(git -C "$checkout" rev-parse 'HEAD^{tree}')"
  echo "targets: $targets (each step's own, empty before it)"
} >> "$log"
mkdir -p "$targets/base-bb0dfec0"
git -C "$repo" archive bb0dfec0338273ddf187b1dfcb22cd4b8eb8b2f9 | tar -x -C "$targets/base-bb0dfec0"
state "before"
step i4r1-scope-controls python3 "$r1/scope_controls.py" "$checkout" "$out/i4r1-scope-controls" \
  --target-dir "$targets/i4r1-scope-controls"
step i4r1-api-guards python3 "$r1/api_guards.py" "$checkout" "$out/i4r1-api-guards" \
  --target-dir "$targets/i4r1-api-guards"
step i4r1-normal-api-probes python3 "$r1/normal_api_probes.py" "$checkout" "$targets/base-bb0dfec0" \
  "$targets/normal-api-probes-scratch" "$out/i4r1-normal-api-probes" \
  --target-dir "$targets/i4r1-normal-api-probes"
step i4r1-source-guards python3 "$r1/source_guards.py" "$checkout" "$out/i4r1-source-guards.json"
step custody-hashes python3 "$i4/custody_hashes.py" "$checkout"
mkdir -p "$targets/r3-store" "$targets/i2r1" "$targets/r3-api"
step r3-store-controls env CARGO_TARGET_DIR="$targets/r3-store" \
  python3 "$r3/store_controls.py" "$checkout" "$out/r3-store"
step i2r1-rerun env CARGO_TARGET_DIR="$targets/i2r1" \
  python3 "$checkout/docs/evidence/p2-v1-r3b-i3-i1/scripts/i2r1_controls_rerun.py" "$checkout" "$out/i2r1-rerun"
step r3-api-probes env CARGO_TARGET_DIR="$targets/r3-api" \
  python3 "$r3/api_probes.py" "$checkout" "$out/r3-api-probes"
step r3-store-comparison python3 "$i4/compare_reruns.py" store \
  "$checkout/docs/evidence/p2-v1-r3b-i3-i1-r3/controls/store/summary.json" "$out/r3-store/summary.json"
step i2r1-comparison python3 "$r3/compare_i2r1.py" \
  "$checkout/docs/evidence/p2-v1-r3b-i3-i1-r3/controls/i2r1-rerun/summary.json" "$out/i2r1-rerun/summary.json"
step r3-api-comparison python3 "$i4/compare_reruns.py" api \
  "$checkout/docs/evidence/p2-v1-r3b-i3-i1-r3/controls/api-probes/api_probes.json" "$out/r3-api-probes/api_probes.json"
cat "$log"

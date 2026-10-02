#!/usr/bin/env bash
# P2-V1-R3B-I3-I1-R2 candidate runs, serially and alone in one clean Git
# checkout of the candidate (nothing else may build, test or edit there):
#   1. validate.sh (the section 11 commands);
#   2. store_controls.py (the counted store controls);
#   3. the I2-R1 control rerun, with I3-I1's runner unchanged (the six
#      files it pins are byte-identical to I3-I1's);
#   4. api_probes.py (the compile-time API/type guards).
# Each mutating step restores the checkout byte for byte and verifies it;
# the checkout's status is recorded before and after every step.
# Usage: candidate_runs.sh <clean checkout> <output directory>
set -u
checkout="$(cd "$1" && pwd)"
out="$2"
mkdir -p "$out"
out="$(cd "$out" && pwd)"
scripts="$checkout/docs/evidence/p2-v1-r3b-i3-i1-r2/scripts"
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
} >> "$log"
state "before"
step validation bash "$scripts/validate.sh" "$checkout" "$out/validation"
step store-controls python3 "$scripts/store_controls.py" "$checkout" "$out/controls/store"
step i2r1-rerun python3 "$checkout/docs/evidence/p2-v1-r3b-i3-i1/scripts/i2r1_controls_rerun.py" \
  "$checkout" "$out/controls/i2r1-rerun"
step api-probes python3 "$scripts/api_probes.py" "$checkout" "$out/api-probes"
cat "$log"

#!/usr/bin/env bash
# P2-V1-R3B-I4 candidate runs, serially and alone in one clean Git checkout
# of the candidate (nothing else may build, test or edit there):
#   1. validate.sh (the section 23 commands);
#   2. scope_controls.py (the behavioural NC-I4-* controls);
#   3. api_guards.py (the compile-time API/type guards);
#   4. source_guards.py (the static source guards, with self-tests);
#   5. the accepted R3 store controls, R3's runner unchanged;
#   6. the I2-R1 control rerun, I3-I1's runner unchanged;
#   7. the accepted R3 API probes, R3's runner unchanged;
#   8. stability: the unit-test binary run repeatedly;
#   9. the no-live-systemd proof: the unit-test binary run under strace;
#  10. the production caller: the desktop backend type-checked (library and
#      tests) against the candidate, and its Phase Two guard tests (static
#      reads of the sandbox sources, manifests and packaging) run.
# Every mutating step gets its own empty target directory (beneath
# <targets>), restores the checkout byte for byte and verifies it; the
# checkout's status is recorded before and after every step.
# Usage: candidate_runs.sh <clean checkout> <output directory> <targets directory (must not exist)>
set -u
checkout="$(cd "$1" && pwd)"
out="$2"
targets="$3"
[ ! -e "$targets" ] || { echo "exists: $targets" >&2; exit 2; }
mkdir -p "$out" "$targets"
out="$(cd "$out" && pwd)"
targets="$(cd "$targets" && pwd)"
scripts="$checkout/docs/evidence/p2-v1-r3b-i4-native-scope/scripts"
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
state "before"
step validation env CARGO_TARGET_DIR="$targets/validation" \
  bash "$scripts/validate.sh" "$checkout" "$out/validation"
step scope-controls python3 "$scripts/scope_controls.py" "$checkout" "$out/controls/scope" \
  --target-dir "$targets/scope-controls"
step api-guards python3 "$scripts/api_guards.py" "$checkout" "$out/controls/api-guards" \
  --target-dir "$targets/api-guards"
step source-guards python3 "$scripts/source_guards.py" "$checkout" "$out/controls/source-guards.json"
mkdir -p "$targets/r3-store" "$targets/i2r1" "$targets/r3-api"
step r3-store-controls env CARGO_TARGET_DIR="$targets/r3-store" \
  python3 "$r3/store_controls.py" "$checkout" "$out/controls/r3-store"
step i2r1-rerun env CARGO_TARGET_DIR="$targets/i2r1" \
  python3 "$checkout/docs/evidence/p2-v1-r3b-i3-i1/scripts/i2r1_controls_rerun.py" \
  "$checkout" "$out/controls/i2r1-rerun"
step r3-api-probes env CARGO_TARGET_DIR="$targets/r3-api" \
  python3 "$r3/api_probes.py" "$checkout" "$out/controls/r3-api-probes"
step stability bash "$scripts/stability.sh" "$checkout" "$targets/validation" "$out/stability" 25
step no-live bash "$scripts/no_live.sh" "$checkout" "$targets/validation" "$out/no-live"
step desktop-callers env CARGO_TARGET_DIR="$targets/desktop" bash -c "cd '$checkout' && \
  cargo check --locked -p nexus-desktop-backend --lib --tests && \
  cargo test --locked -p nexus-desktop-backend --lib phase2_tests::"
cat "$log"

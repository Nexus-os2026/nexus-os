#!/usr/bin/env bash
# P2-V1-R3B-I4-R3-R1: the first compatibility run. The accepted control and
# guard runners R3-R1 may affect, invoked exactly as P2-V1-R3B-I4-R3
# accepted them (its run_controls.sh and accepted_reruns.sh steps: R3's own
# runner, R3's adapters of the R2, Q1-R1, I4/I4-R1 controls and of the
# source guards, the Q1 runner, and the I4-R1 API guards and normal-build
# probes, which R3 ran unadapted), unmodified, on one isolated clean
# checkout of the R3-R1 candidate: each runner's anchor check first, then
# its full run with its own empty target directory. They are expected to
# fail where R3-R1 changed what they anchor on or assert: the outputs are
# kept as evidence of exactly where, before any adaptation. The runners
# restore every file they mutate (a runner that stops on a missing anchor
# stops before mutating anything); this script records the checkout's
# status after each. P2-V1-R3B-I4-R3's compat_run1.sh, but for the runners.
# Usage: compat_run1.sh <clean checkout> <output directory> <targets directory (must not exist)> <repository>
set -u
checkout="$(cd "$1" && pwd)"
out="$2"
targets="$3"
repo="$4"
[ ! -e "$targets" ] || { echo "exists: $targets" >&2; exit 2; }
mkdir -p "$out" "$targets"
out="$(cd "$out" && pwd)"
targets="$(cd "$targets" && pwd)"
export PYTHONDONTWRITEBYTECODE=1
r3e="$checkout/docs/evidence/p2-v1-r3b-i4-r3-no-name-actuation/scripts"
q1="$checkout/docs/evidence/p2-v1-r3b-i4-q1-host-live-qualification/scripts"
r1="$checkout/docs/evidence/p2-v1-r3b-i4-r1-native-scope/scripts"
log="$out/runs.txt"
{
  echo "checkout: $checkout"
  echo "HEAD: $(git -C "$checkout" rev-parse HEAD)"
  echo "HEAD tree: $(git -C "$checkout" rev-parse 'HEAD^{tree}')"
  echo "status entries before: $(git -C "$checkout" status --porcelain=v1 --untracked-files=all | wc -l)"
  for f in "$r3e/r3_controls.py" "$r3e/accepted_r2_controls_r3.py" "$r3e/accepted_r1_controls_r3.py" \
           "$r3e/accepted_scope_controls_r3.py" "$r3e/accepted_source_guards_r3.py" "$q1/q1_controls.py" \
           "$checkout/docs/evidence/p2-v1-r3b-i4-r2-uncertain-stop-authority/scripts/r2_controls.py" \
           "$checkout/docs/evidence/p2-v1-r3b-i4-q1-r1-host-qualification-ownership/scripts/r1_controls.py" \
           "$checkout/docs/evidence/p2-v1-r3b-i4-r1-native-scope/scripts/scope_controls.py" \
           "$checkout/docs/evidence/p2-v1-r3b-i4-r1-native-scope/scripts/source_guards.py" \
           "$r1/api_guards.py" "$r1/normal_api_probes.py"; do
    echo "sha256 $(sha256sum "$f" | cut -d' ' -f1) ${f#"$checkout"/}"
  done
} > "$log"
step() {
  local name="$1"
  shift
  local started
  started=$(date -u +%Y-%m-%dT%H:%M:%SZ)
  "$@" > "$out/$name.out" 2>&1
  local status=$?
  printf '%s exit=%d started=%s status-entries-after=%s :: %s\n' "$name" "$status" "$started" \
    "$(git -C "$checkout" status --porcelain=v1 --untracked-files=all | wc -l)" "$*" >> "$log"
}
step r3-controls-anchors python3 "$r3e/r3_controls.py" "$checkout" --check-anchors
step r2-controls-anchors python3 "$r3e/accepted_r2_controls_r3.py" "$checkout" --check-anchors
step q1r1-controls-anchors python3 "$r3e/accepted_r1_controls_r3.py" "$checkout" --check-anchors
step q1-controls-anchors python3 "$q1/q1_controls.py" "$checkout" --check-anchors
step i4r1-scope-controls-anchors python3 "$r3e/accepted_scope_controls_r3.py" "$checkout" --check-anchors
step i4r1-source-guards python3 "$r3e/accepted_source_guards_r3.py" "$checkout" "$out/i4r1-source-guards.json"
step r3-controls python3 "$r3e/r3_controls.py" "$checkout" "$out/r3" --target-dir "$targets/r3"
step r2-controls python3 "$r3e/accepted_r2_controls_r3.py" "$checkout" "$out/r2" --target-dir "$targets/r2"
step q1r1-controls python3 "$r3e/accepted_r1_controls_r3.py" "$checkout" "$out/q1r1" --target-dir "$targets/q1r1"
step q1-controls python3 "$q1/q1_controls.py" "$checkout" "$out/q1" --target-dir "$targets/q1"
step i4r1-scope-controls python3 "$r3e/accepted_scope_controls_r3.py" "$checkout" "$out/i4r1-scope-controls" \
  --target-dir "$targets/i4r1-scope-controls"
step i4r1-api-guards python3 "$r1/api_guards.py" "$checkout" "$out/i4r1-api-guards" \
  --target-dir "$targets/i4r1-api-guards"
mkdir -p "$targets/base-bb0dfec0"
git -C "$repo" archive bb0dfec0338273ddf187b1dfcb22cd4b8eb8b2f9 | tar -x -C "$targets/base-bb0dfec0"
step i4r1-normal-api-probes python3 "$r1/normal_api_probes.py" "$checkout" "$targets/base-bb0dfec0" \
  "$targets/normal-api-probes-scratch" "$out/i4r1-normal-api-probes" \
  --target-dir "$targets/i4r1-normal-api-probes"
cat "$log"

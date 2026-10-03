#!/usr/bin/env bash
# P2-V1-R3B-I4-R3: the first compatibility run. The accepted control and
# guard runners R3 adapts, invoked exactly as P2-V1-R3B-I4-R2 accepted them
# (its run_controls.sh and accepted_reruns.sh steps), unmodified, on one
# isolated clean checkout of the R3 candidate; each runner's anchor check
# first, then its full run with its own empty target directory. They are
# expected to fail where R3 removed what they anchor on: the outputs are
# kept as evidence of exactly where, before any adaptation. The runners
# restore every file they mutate (a runner that stops on a missing anchor
# stops before mutating anything); this script records the checkout's
# status after each.
# Usage: compat_run1.sh <clean checkout> <output directory> <targets directory (must not exist)>
set -u
checkout="$(cd "$1" && pwd)"
out="$2"
targets="$3"
[ ! -e "$targets" ] || { echo "exists: $targets" >&2; exit 2; }
mkdir -p "$out" "$targets"
out="$(cd "$out" && pwd)"
targets="$(cd "$targets" && pwd)"
export PYTHONDONTWRITEBYTECODE=1
r2="$checkout/docs/evidence/p2-v1-r3b-i4-r2-uncertain-stop-authority/scripts"
r1="$checkout/docs/evidence/p2-v1-r3b-i4-r1-native-scope/scripts"
log="$out/runs.txt"
{
  echo "checkout: $checkout"
  echo "HEAD: $(git -C "$checkout" rev-parse HEAD)"
  echo "HEAD tree: $(git -C "$checkout" rev-parse 'HEAD^{tree}')"
  echo "status entries before: $(git -C "$checkout" status --porcelain=v1 --untracked-files=all | wc -l)"
  for f in "$r2/accepted_scope_controls_r2.py" "$r2/r2_controls.py" "$r2/accepted_r1_controls_r2.py" \
           "$r1/scope_controls.py" "$r1/source_guards.py" \
           "$checkout/docs/evidence/p2-v1-r3b-i4-q1-r1-host-qualification-ownership/scripts/r1_controls.py"; do
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
step i4r1-scope-controls-anchors python3 "$r2/accepted_scope_controls_r2.py" "$checkout" --check-anchors
step r2-controls-anchors python3 "$r2/r2_controls.py" "$checkout" --check-anchors
step q1r1-controls-anchors python3 "$r2/accepted_r1_controls_r2.py" "$checkout" --check-anchors
step i4r1-source-guards python3 "$r1/source_guards.py" "$checkout" "$out/i4r1-source-guards.json"
step i4r1-scope-controls python3 "$r2/accepted_scope_controls_r2.py" "$checkout" "$out/i4r1-scope-controls" \
  --target-dir "$targets/i4r1-scope-controls"
step r2-controls python3 "$r2/r2_controls.py" "$checkout" "$out/r2" --target-dir "$targets/r2"
step q1r1-controls python3 "$r2/accepted_r1_controls_r2.py" "$checkout" "$out/q1r1" --target-dir "$targets/q1r1"
cat "$log"

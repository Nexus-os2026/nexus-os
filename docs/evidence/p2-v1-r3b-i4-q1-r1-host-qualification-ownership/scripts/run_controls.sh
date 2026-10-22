#!/usr/bin/env bash
# P2-V1-R3B-I4-Q1-R1: the mutation controls, in one isolated clean checkout
# of the candidate (nothing else may build, test or edit there), one after
# the other, each with its own empty target directory: first the fifteen R1
# controls (r1_controls.py), then the ten Q1 controls, Q1's script
# unchanged. Each restores every file it mutates and checks the checkout's
# Git status; this script records the status between them.
# Usage: run_controls.sh <clean checkout> <output directory> <targets directory (must not exist)>
set -u
checkout="$(cd "$1" && pwd)"
out="$2"
targets="$3"
[ ! -e "$targets" ] || { echo "exists: $targets" >&2; exit 2; }
mkdir -p "$out" "$targets"
out="$(cd "$out" && pwd)"
export PYTHONDONTWRITEBYTECODE=1
log="$out/runs.txt"
{
  echo "checkout: $checkout"
  echo "HEAD: $(git -C "$checkout" rev-parse HEAD)"
  echo "HEAD tree: $(git -C "$checkout" rev-parse 'HEAD^{tree}')"
  echo "status entries before: $(git -C "$checkout" status --porcelain=v1 --untracked-files=all | wc -l)"
} > "$log"
python3 "$checkout/docs/evidence/p2-v1-r3b-i4-q1-r1-host-qualification-ownership/scripts/r1_controls.py" \
  "$checkout" "$out/r1" --target-dir "$targets/r1" > "$out/r1.out" 2>&1
echo "r1 exit=$? status entries after: $(git -C "$checkout" status --porcelain=v1 --untracked-files=all | wc -l)" >> "$log"
python3 "$checkout/docs/evidence/p2-v1-r3b-i4-q1-host-live-qualification/scripts/q1_controls.py" \
  "$checkout" "$out/q1" --target-dir "$targets/q1" > "$out/q1.out" 2>&1
echo "q1 exit=$? status entries after: $(git -C "$checkout" status --porcelain=v1 --untracked-files=all | wc -l)" >> "$log"
cat "$log"

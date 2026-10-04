#!/usr/bin/env bash
# P2-V1-R3B-I4-Q3-R1: the mutation controls, in one isolated clean checkout
# of the candidate (nothing else may build, test or edit there), one after
# the other, each with its own empty target directory: the Q3-R1 controls
# (q3r1_controls.py), then the accepted R3-R1 controls (R3-R1's
# r3r1_controls.py, unchanged), the accepted R3, R2 and Q1-R1 controls
# (R3-R1's adapters, unchanged) and the accepted Q1 controls (their runner
# unchanged). Each restores every file it mutates and checks the checkout's
# Git status; this script records the status between. P2-V1-R3B-I4-R3-R1's
# run_controls.sh, but for the Q3-R1 controls added first.
# Usage: run_controls.sh <clean checkout> <output directory> <targets directory (must not exist)>
set -u
checkout="$(cd "$1" && pwd)"
out="$2"
targets="$3"
[ ! -e "$targets" ] || { echo "exists: $targets" >&2; exit 2; }
mkdir -p "$out" "$targets"
out="$(cd "$out" && pwd)"
export PYTHONDONTWRITEBYTECODE=1
Q="$checkout/docs/evidence/p2-v1-r3b-i4-q3-r1-never-populated-lifecycle/scripts"
E="$checkout/docs/evidence/p2-v1-r3b-i4-r3-r1-candidate-ownership/scripts"
log="$out/runs.txt"
{
  echo "checkout: $checkout"
  echo "HEAD: $(git -C "$checkout" rev-parse HEAD)"
  echo "HEAD tree: $(git -C "$checkout" rev-parse 'HEAD^{tree}')"
  echo "status entries before: $(git -C "$checkout" status --porcelain=v1 --untracked-files=all | wc -l)"
} > "$log"
python3 "$Q/q3r1_controls.py" "$checkout" "$out/q3r1" --target-dir "$targets/q3r1" > "$out/q3r1.out" 2>&1
echo "q3r1 exit=$? status entries after: $(git -C "$checkout" status --porcelain=v1 --untracked-files=all | wc -l)" >> "$log"
python3 "$E/r3r1_controls.py" "$checkout" "$out/r3r1" --target-dir "$targets/r3r1" > "$out/r3r1.out" 2>&1
echo "r3r1 exit=$? status entries after: $(git -C "$checkout" status --porcelain=v1 --untracked-files=all | wc -l)" >> "$log"
python3 "$E/accepted_r3_controls_r3r1.py" "$checkout" "$out/r3" --target-dir "$targets/r3" > "$out/r3.out" 2>&1
echo "r3 exit=$? status entries after: $(git -C "$checkout" status --porcelain=v1 --untracked-files=all | wc -l)" >> "$log"
python3 "$E/accepted_r2_controls_r3r1.py" "$checkout" "$out/r2" --target-dir "$targets/r2" > "$out/r2.out" 2>&1
echo "r2 exit=$? status entries after: $(git -C "$checkout" status --porcelain=v1 --untracked-files=all | wc -l)" >> "$log"
python3 "$E/accepted_r1_controls_r3r1.py" "$checkout" "$out/q1r1" --target-dir "$targets/q1r1" > "$out/q1r1.out" 2>&1
echo "q1r1 exit=$? status entries after: $(git -C "$checkout" status --porcelain=v1 --untracked-files=all | wc -l)" >> "$log"
python3 "$checkout/docs/evidence/p2-v1-r3b-i4-q1-host-live-qualification/scripts/q1_controls.py" \
  "$checkout" "$out/q1" --target-dir "$targets/q1" > "$out/q1.out" 2>&1
echo "q1 exit=$? status entries after: $(git -C "$checkout" status --porcelain=v1 --untracked-files=all | wc -l)" >> "$log"
cat "$log"

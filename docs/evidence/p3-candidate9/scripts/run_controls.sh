#!/usr/bin/env bash
# Phase Three Candidate 9: the mutation controls (c9_controls.py), in one
# run, in one isolated Git checkout of the candidate (nothing else may
# build, test or edit there while they run). The controls restore every
# file they mutate and check the checkout's Git status; this script records
# the candidate's identity and the status before and after.
# Usage: run_controls.sh <checkout> <output directory> <target directory>
set -u
[ $# -eq 3 ] || { echo "usage: $0 <checkout> <output directory> <target directory>" >&2; exit 2; }
checkout="$(cd "$1" && pwd)"
out="$2"
target="$3"
mkdir -p "$out" "$target"
out="$(cd "$out" && pwd)"
export PYTHONDONTWRITEBYTECODE=1 CI=1
unset DISPLAY
script="$checkout/docs/evidence/p3-candidate9/scripts/c9_controls.py"
log="$out/runs.txt"
{
  echo "checkout: $checkout"
  echo "HEAD: $(git -C "$checkout" rev-parse HEAD)"
  echo "HEAD tree: $(git -C "$checkout" rev-parse 'HEAD^{tree}')"
  echo "script sha256: $(sha256sum "$script" | cut -d' ' -f1)"
  echo "status entries before: $(git -C "$checkout" status --porcelain=v1 --untracked-files=all | wc -l)"
  echo "started: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
} > "$log"
python3 -I -B "$script" "$checkout" "$out/c9" --target-dir "$target" > "$out/c9.out" 2>&1
code=$?
{
  echo "c9 exit=$code"
  echo "status entries after: $(git -C "$checkout" status --porcelain=v1 --untracked-files=all | wc -l)"
  echo "finished: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
} >> "$log"
cat "$log"
exit "$code"

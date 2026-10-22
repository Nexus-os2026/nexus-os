#!/usr/bin/env bash
# P2-V1-R3B-I4 evidence assembly. Copies every raw output (kept in the work
# directory, outside the repository) into this evidence directory through
# normalize.py, which records each copy's raw and normalized SHA-256 in
# validation/normalization.tsv. Run from the repository root.
# Usage: assemble.sh <work directory>
set -eu
E=docs/evidence/p2-v1-r3b-i4-native-scope
W="$1"
RUNS=$W/candidate-runs
N="python3 -B $E/scripts/normalize.py $E/validation/normalization.tsv"
[ ! -e "$E/validation/normalization.tsv" ] || { echo "already assembled" >&2; exit 2; }
pairs=()
add() { pairs+=("$1" "$2"); }
# baseline/: B-I4-1..5 at the base, and the source-derived model.
for f in "$W"/baseline/*; do add "$f" "$E/baseline/$(basename "$f")"; done
# validation/: the section 23 commands, stability, the desktop callers, the
# run log, the snapshot's identity, the repository checks, the review
# searches and the citations.
for f in "$RUNS"/validation/*; do add "$f" "$E/validation/candidate/$(basename "$f")"; done
for f in "$RUNS"/stability/*; do add "$f" "$E/validation/stability/$(basename "$f")"; done
for f in "$RUNS"/no-live/*; do add "$f" "$E/validation/no-live/$(basename "$f")"; done
add "$RUNS/desktop-callers.out" "$E/validation/desktop-callers.txt"
add "$RUNS/runs.txt" "$E/validation/candidate-runs.txt"
add "$W/candidate-runs.console" "$E/validation/candidate-runs.console.txt"
add "$W/candidate-snapshot.txt" "$E/validation/candidate-snapshot.txt"
add "$W/candidate-snapshot-files.SHA256SUMS" "$E/validation/candidate-snapshot-files.SHA256SUMS"
for f in "$W"/checks/*; do add "$f" "$E/validation/checks/$(basename "$f")"; done
add "$W/review-searches.txt" "$E/validation/review-searches.txt"
add "$W/citations.txt" "$E/validation/citations.txt"
# controls/
for d in scope api-guards r3-store i2r1-rerun r3-api-probes; do
  for f in "$RUNS/controls/$d"/*; do add "$f" "$E/controls/$d/$(basename "$f")"; done
done
add "$RUNS/scope-controls.out" "$E/controls/scope/run.out"
add "$RUNS/api-guards.out" "$E/controls/api-guards/run.out"
add "$RUNS/r3-store-controls.out" "$E/controls/r3-store/run.out"
add "$RUNS/i2r1-rerun.out" "$E/controls/i2r1-rerun/run.out"
add "$RUNS/r3-api-probes.out" "$E/controls/r3-api-probes/run.out"
add "$RUNS/controls/source-guards.json" "$E/controls/source-guards.json"
add "$RUNS/source-guards.out" "$E/controls/source-guards.run.out"
add "$W/restoration-summary.md" "$E/controls/restoration-summary.md"
# matrices/ (generated; the hand-written ones are already in place)
add "$W/matrices/test-control-coverage.md" "$E/matrices/test-control-coverage.md"
add "$W/matrices/test-control-coverage.json" "$E/matrices/test-control-coverage.json"
# scope/
for f in "$W"/scope/*; do add "$f" "$E/scope/$(basename "$f")"; done
$N "${pairs[@]}"
echo "copied $(( ${#pairs[@]} / 2 )) files"

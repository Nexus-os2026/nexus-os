#!/usr/bin/env bash
# P2-V1-R3B-I3-I1-R3 evidence assembly. Copies every raw output (kept in the
# work directory, outside the repository) into this evidence directory
# through normalize.py, which records each copy's raw and normalized
# SHA-256 in validation/normalization.tsv. Run from the repository root.
# Usage: assemble.sh <work directory>
set -eu
E=docs/evidence/p2-v1-r3b-i3-i1-r3
W="$1"
RUNS=$W/candidate-runs
N="python3 -B $E/scripts/normalize.py $E/validation/normalization.tsv"
pairs=()
add() { pairs+=("$1" "$2"); }
# baseline/: the R3-B1 to R3-B8 reproduction at the base.
for f in "$W"/baseline/*; do add "$f" "$E/baseline/$(basename "$f")"; done
# validation/: the candidate runs' validation, the run log, the repository
# checks, and the snapshot's identity.
for f in "$RUNS"/validation/*; do add "$f" "$E/validation/candidate/$(basename "$f")"; done
add "$RUNS/runs.txt" "$E/validation/candidate-runs.txt"
add "$W/candidate-runs.console" "$E/validation/candidate-runs.console.txt"
add "$W/candidate-snapshot.txt" "$E/validation/candidate-snapshot.txt"
add "$W/candidate-snapshot-worktree-status.txt" "$E/validation/candidate-snapshot-worktree-status.txt"
add "$W/candidate-snapshot-files.SHA256SUMS" "$E/validation/candidate-snapshot-files.SHA256SUMS"
for f in "$W"/checks-final/*; do add "$f" "$E/validation/checks/$(basename "$f")"; done
add "$W/adaptation-diff-apply.txt" "$E/validation/checks/adaptation-diff-apply.txt"
# controls/
for f in "$RUNS"/controls/store/*; do add "$f" "$E/controls/store/$(basename "$f")"; done
add "$RUNS/store-controls.out" "$E/controls/store/run.out"
for f in "$RUNS"/controls/i2r1-rerun/*; do add "$f" "$E/controls/i2r1-rerun/$(basename "$f")"; done
add "$RUNS/i2r1-rerun.out" "$E/controls/i2r1-rerun/run.out"
add "$W/i2r1-comparison.txt" "$E/controls/i2r1-rerun/comparison-with-r2.txt"
for f in "$RUNS"/api-probes/*; do add "$f" "$E/controls/api-probes/$(basename "$f")"; done
add "$RUNS/api-probes.out" "$E/controls/api-probes/run.out"
# controls/earlier-attempt/: the complete run before the final one (snapshot
# tree ad142aaf...), which found two R2 controls masked by R3 checks and
# three probe self-checks that needed the returned type reopened.
A=$W/candidate-runs-attempt3
add "$A/runs.txt" "$E/controls/earlier-attempt/runs.txt"
add "$A/snapshot.txt" "$E/controls/earlier-attempt/snapshot.txt"
add "$A/controls/store/summary.json" "$E/controls/earlier-attempt/store-summary.json"
for id in NC-SUCC-MUTATE-BEFORE-GATE NC-SUCC-SAME-ROOT; do
  add "$A/controls/store/$id.log" "$E/controls/earlier-attempt/$id.log"
done
add "$A/api-probes/api_probes.json" "$E/controls/earlier-attempt/api_probes.json"
for id in P-R3-KEPT-COPY P-R3-INSPECT P-R3-REVOCATION-STATE; do
  add "$A/api-probes/$id.reopened.log" "$E/controls/earlier-attempt/$id.reopened.log"
done
# matrices/ (generated)
for f in "$W"/matrices/*; do add "$f" "$E/matrices/$(basename "$f")"; done
add "$W/crash-model-tallies.md" "$E/matrices/crash-model-tallies.md"
# scope/
for f in "$W"/scope/*; do add "$f" "$E/scope/$(basename "$f")"; done
# scripts/: the adaptation diff (R2's scripts to this directory's).
add "$W/raw/scripts.adaptation.diff" "$E/scripts/scripts.adaptation.diff"
$N "${pairs[@]}"
echo "copied $(( ${#pairs[@]} / 2 )) files"

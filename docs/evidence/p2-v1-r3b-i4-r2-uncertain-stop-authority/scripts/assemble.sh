#!/usr/bin/env bash
# P2-V1-R3B-I4-R2 evidence assembly: compact. Copies the before-repair
# reproduction, the non-live validation, the R2 and accepted mutation
# controls and the accepted suites' reruns (kept complete in the work
# directory, outside the repository) into this evidence directory through
# Q1's normalize.py (unchanged), which records each copy's raw and normalized
# SHA-256 in validation/normalization.tsv. Full logs stay in the work
# directory. Run from the repository root.
# Usage: assemble.sh <work directory>
set -eu
E=docs/evidence/p2-v1-r3b-i4-r2-uncertain-stop-authority
W="$1"
N="python3 -B docs/evidence/p2-v1-r3b-i4-q1-host-live-qualification/scripts/normalize.py $E/validation/normalization.tsv"
[ ! -e "$E/validation/normalization.tsv" ] || { echo "already assembled" >&2; exit 2; }
pairs=()
add() { pairs+=("$1" "$2"); }
add "$W/reproduction/before-repair.out" "$E/reproduction/before-repair.out"
for f in commands.txt toolchain.txt sources.SHA256SUMS lib-tests.txt store-tests.txt codec-tests.txt \
         core-tests.txt cleanup-observation.txt package-layout.txt live-harness-build-only.txt \
         live-step-fixture-controls.txt desktop-check.txt desktop-clippy.txt desktop-phase2-guards.txt \
         clippy.txt fmt-check.txt diff-check.txt lib-test-list.txt desktop-guard-list.txt; do
  add "$W/validation/$f" "$E/validation/$f"
done
add "$W/validation.console" "$E/validation/console.txt"
add "$W/controls/runs.txt" "$E/controls/runs.txt"
add "$W/controls/r2/summary.json" "$E/controls/r2/summary.json"
add "$W/controls/r2.out" "$E/controls/r2/run.out"
add "$W/controls/q1r1/summary.json" "$E/controls/q1r1/summary.json"
add "$W/controls/q1r1.out" "$E/controls/q1r1/run.out"
add "$W/controls/q1/summary.json" "$E/controls/q1/summary.json"
add "$W/controls/q1.out" "$E/controls/q1/run.out"
add "$W/accepted/runs.txt" "$E/controls/accepted/runs.txt"
add "$W/accepted/i4r1-scope-controls/summary.json" "$E/controls/accepted/i4r1-scope-controls.summary.json"
add "$W/accepted/i4r1-scope-controls.out" "$E/controls/accepted/i4r1-scope-controls.run.out"
add "$W/accepted/i4r1-api-guards/summary.json" "$E/controls/accepted/i4r1-api-guards.summary.json"
add "$W/accepted/i4r1-normal-api-probes/summary.json" "$E/controls/accepted/i4r1-normal-api-probes.summary.json"
add "$W/accepted/i4r1-source-guards.json" "$E/controls/accepted/i4r1-source-guards.json"
add "$W/accepted/custody-hashes.out" "$E/controls/accepted/custody-hashes.txt"
add "$W/accepted/r3-store/summary.json" "$E/controls/accepted/r3-store.summary.json"
add "$W/accepted/i2r1-rerun/summary.json" "$E/controls/accepted/i2r1-rerun.summary.json"
add "$W/accepted/r3-api-probes/api_probes.json" "$E/controls/accepted/r3-api-probes.json"
add "$W/accepted/r3-store-comparison.out" "$E/controls/accepted/r3-store-comparison.txt"
add "$W/accepted/i2r1-comparison.out" "$E/controls/accepted/i2r1-comparison.txt"
add "$W/accepted/r3-api-comparison.out" "$E/controls/accepted/r3-api-comparison.txt"
# The first full rerun of the accepted I4 and I4-R1 controls, which found
# NC-I4-X-COLLISION-CLAIMED missing its marker (kept: the diagnosis).
add "$W/run1/accepted/i4r1-scope-controls/summary.json" "$E/controls/accepted/first-run-i4r1-scope-controls.summary.json"
add "$W/run1/accepted/i4r1-scope-controls/NC-I4-X-COLLISION-CLAIMED.log" "$E/controls/accepted/first-run-NC-I4-X-COLLISION-CLAIMED.log"
for f in "$W"/scope/*; do add "$f" "$E/scope/$(basename "$f")"; done
$N "${pairs[@]}"
echo "copied $(( ${#pairs[@]} / 2 )) files"

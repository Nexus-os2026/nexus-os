#!/usr/bin/env bash
# P2-V1-R3B-I4-Q1-R1 evidence assembly: compact. Copies the reproduction,
# the summaries and key outputs of the non-live validation, the R1 and Q1
# mutation controls and the accepted suites' reruns (kept complete in the
# work directory, outside the repository) into this evidence directory
# through Q1's normalize.py (unchanged), which records each copy's raw and
# normalized SHA-256 in validation/normalization.tsv. Full control logs stay
# in the work directory. Run from the repository root.
# Usage: assemble.sh <work directory>
set -eu
E=docs/evidence/p2-v1-r3b-i4-q1-r1-host-qualification-ownership
W="$1"
N="python3 -B docs/evidence/p2-v1-r3b-i4-q1-host-live-qualification/scripts/normalize.py $E/validation/normalization.tsv"
[ ! -e "$E/validation/normalization.tsv" ] || { echo "already assembled" >&2; exit 2; }
pairs=()
add() { pairs+=("$1" "$2"); }
# reproduction/: the finding, reproduced against 3d46b4d on the model.
add "$W/reproduction/old_h6_on_the_model.rs" "$E/reproduction/old_h6_on_the_model.rs"
add "$W/reproduction/run.out" "$E/reproduction/run.out"
# validation/: the section 13 commands, their results, the toolchain and the
# validated sources' hashes; the full outputs of the test runs.
for f in commands.txt toolchain.txt sources.SHA256SUMS lib-tests.txt store-tests.txt codec-tests.txt \
         core-tests.txt cleanup-observation.txt package-layout.txt live-harness-build-only.txt \
         live-step-fixture-controls.txt desktop-check.txt desktop-clippy.txt desktop-phase2-guards.txt \
         clippy.txt fmt-check.txt diff-check.txt lib-test-list.txt desktop-guard-list.txt; do
  add "$W/validation/$f" "$E/validation/$f"
done
add "$W/validation.console" "$E/validation/console.txt"
# controls/: the R1 and Q1 mutation controls (summaries and run logs) and the
# accepted suites' summaries and comparisons.
add "$W/controls/runs.txt" "$E/controls/runs.txt"
add "$W/controls/r1/summary.json" "$E/controls/r1/summary.json"
add "$W/controls/r1.out" "$E/controls/r1/run.out"
add "$W/controls/q1/summary.json" "$E/controls/q1/summary.json"
add "$W/controls/q1.out" "$E/controls/q1/run.out"
add "$W/accepted/runs.txt" "$E/controls/accepted/runs.txt"
add "$W/accepted/i4r1-scope-controls/summary.json" "$E/controls/accepted/i4r1-scope-controls.summary.json"
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
# scope/
for f in "$W"/scope/*; do add "$f" "$E/scope/$(basename "$f")"; done
$N "${pairs[@]}"
echo "copied $(( ${#pairs[@]} / 2 )) files"

#!/usr/bin/env bash
# P2-V1-R3B-I4-Q3-R1 evidence assembly: copies the preserved live baseline,
# the primary-source excerpts, the non-live validation, the Q3-R1 and accepted
# mutation controls, the accepted suites' reruns and the scope records (kept
# complete in the work directory, outside the repository) into this evidence
# directory through Q1's normalize.py (unchanged), which records each copy's
# raw and normalized SHA-256 in validation/normalization.tsv. Full logs stay
# in the work directory. P2-V1-R3B-I4-R3-R1's assemble.sh, but for the paths
# and the runs this mission makes. Run from the repository root.
# Usage: assemble.sh <work directory> <final run directory>
set -eu
E=docs/evidence/p2-v1-r3b-i4-q3-r1-never-populated-lifecycle
W="$1"
F="$2"
N="python3 -B docs/evidence/p2-v1-r3b-i4-q1-host-live-qualification/scripts/normalize.py $E/validation/normalization.tsv"
[ ! -e "$E/validation/normalization.tsv" ] || { echo "already assembled" >&2; exit 2; }
mkdir -p "$E/validation"
pairs=()
add() { pairs+=("$1" "$2"); }
for f in result.txt run.json jobs.json expected-cases.txt run-log.sha256.txt live-step-excerpt.txt \
         journal-excerpt.txt host-residue.txt; do
  add "$W/baseline/$f" "$E/baseline/$f"
done
add "$W/primary/never-populated-excerpts.txt" "$E/primary/never-populated-excerpts.txt"
add "$W/primary/linux-v6.17.identity.txt" "$E/primary/linux-v6.17.identity.txt"
for f in commands.txt toolchain.txt sources.SHA256SUMS lib-tests.txt store-tests.txt codec-tests.txt \
         core-tests.txt cleanup-observation.txt package-layout.txt live-harness-build-only.txt \
         never-populated-fixtures.txt live-step-fixture-controls.txt desktop-check.txt desktop-clippy.txt \
         desktop-phase2-guards.txt clippy.txt fmt-check.txt diff-check.txt lib-test-list.txt \
         desktop-guard-list.txt never-populated-fixture-list.txt; do
  add "$F/validation/$f" "$E/validation/$f"
done
add "$F/validation.console" "$E/validation/console.txt"
add "$F/controls/runs.txt" "$E/controls/runs.txt"
for c in q3r1 r3r1 r3 r2 q1r1 q1; do
  add "$F/controls/$c/summary.json" "$E/controls/$c/summary.json"
  add "$F/controls/$c.out" "$E/controls/$c/run.out"
done
add "$F/accepted/runs.txt" "$E/controls/accepted/runs.txt"
add "$F/accepted/i4r1-scope-controls/summary.json" "$E/controls/accepted/i4r1-scope-controls.summary.json"
add "$F/accepted/i4r1-scope-controls.out" "$E/controls/accepted/i4r1-scope-controls.run.out"
add "$F/accepted/i4r1-api-guards/summary.json" "$E/controls/accepted/i4r1-api-guards.summary.json"
add "$F/accepted/i4r1-api-guards.out" "$E/controls/accepted/i4r1-api-guards.run.out"
add "$F/accepted/i4r1-normal-api-probes/summary.json" "$E/controls/accepted/i4r1-normal-api-probes.summary.json"
add "$F/accepted/i4r1-source-guards.json" "$E/controls/accepted/i4r1-source-guards.json"
add "$F/accepted/i4r1-source-guards.out" "$E/controls/accepted/i4r1-source-guards.run.out"
add "$F/accepted/custody-hashes.out" "$E/controls/accepted/custody-hashes.txt"
add "$F/accepted/r3-store/summary.json" "$E/controls/accepted/r3-store.summary.json"
add "$F/accepted/i2r1-rerun/summary.json" "$E/controls/accepted/i2r1-rerun.summary.json"
add "$F/accepted/r3-api-probes/api_probes.json" "$E/controls/accepted/r3-api-probes.json"
add "$F/accepted/r3-store-comparison.out" "$E/controls/accepted/r3-store-comparison.txt"
add "$F/accepted/i2r1-comparison.out" "$E/controls/accepted/i2r1-comparison.txt"
add "$F/accepted/r3-api-comparison.out" "$E/controls/accepted/r3-api-comparison.txt"
for f in "$W"/scope/*; do add "$f" "$E/scope/$(basename "$f")"; done
$N "${pairs[@]}"
echo "copied $(( ${#pairs[@]} / 2 )) files"

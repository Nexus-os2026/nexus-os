#!/usr/bin/env bash
# P2-V1-R3B-I4-R3-R1 evidence assembly: compact. Copies the before- and
# after-repair reproductions, the primary-source excerpts, the non-live
# validation, the first compatibility run, the R3-R1 and accepted mutation
# controls and the accepted suites' reruns (kept complete in the work
# directory, outside the repository) into this evidence directory through
# Q1's normalize.py (unchanged), which records each copy's raw and
# normalized SHA-256 in validation/normalization.tsv. Full logs stay in the
# work directory. P2-V1-R3B-I4-R3's assemble.sh, but for the paths. Run
# from the repository root.
# Usage: assemble.sh <work directory> <final run directory>
set -eu
E=docs/evidence/p2-v1-r3b-i4-r3-r1-candidate-ownership
W="$1"
F="$2"
N="python3 -B docs/evidence/p2-v1-r3b-i4-q1-host-live-qualification/scripts/normalize.py $E/validation/normalization.tsv"
[ ! -e "$E/validation/normalization.tsv" ] || { echo "already assembled" >&2; exit 2; }
pairs=()
add() { pairs+=("$1" "$2"); }
add "$W/reproduction/before-repair.out" "$E/reproduction/before-repair.out"
add "$F/reproduction/after-repair.out" "$E/reproduction/after-repair.out"
add "$W/primary/systemd-255.4-excerpts.txt" "$E/primary/systemd-255.4-excerpts.txt"
for f in commands.txt toolchain.txt sources.SHA256SUMS lib-tests.txt store-tests.txt codec-tests.txt \
         core-tests.txt cleanup-observation.txt package-layout.txt live-harness-build-only.txt \
         live-step-fixture-controls.txt desktop-check.txt desktop-clippy.txt desktop-phase2-guards.txt \
         clippy.txt fmt-check.txt diff-check.txt lib-test-list.txt desktop-guard-list.txt; do
  add "$F/validation/$f" "$E/validation/$f"
done
add "$F/validation.console" "$E/validation/console.txt"
# The first compatibility run: the accepted runners and R3's adapters, unadapted.
add "$F/compat-run1/runs.txt" "$E/controls/compat-run1/runs.txt"
for f in "$F"/compat-run1/*.out "$F"/compat-run1/*.json; do
  [ -e "$f" ] && add "$f" "$E/controls/compat-run1/$(basename "$f")"
done
for d in r3 r2 q1r1 q1 i4r1-scope-controls i4r1-api-guards; do
  [ -e "$F/compat-run1/$d/summary.json" ] && add "$F/compat-run1/$d/summary.json" "$E/controls/compat-run1/$d.summary.json"
done
add "$F/controls/runs.txt" "$E/controls/runs.txt"
for c in r3r1 r3 r2 q1r1 q1; do
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
for f in "$F"/scope/*; do add "$f" "$E/scope/$(basename "$f")"; done
$N "${pairs[@]}"
echo "copied $(( ${#pairs[@]} / 2 )) files"

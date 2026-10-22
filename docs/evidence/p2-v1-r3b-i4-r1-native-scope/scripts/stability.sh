#!/usr/bin/env bash
# P2-V1-R3B-I4-R1 stability (I4's script, unchanged but for this label): the sandbox crate's unit-test binary (already
# built by the validation, in the given target directory) run repeatedly:
# <runs> times with the default test threads, then <runs>/5 times with one
# thread. Each run's result line is recorded; any failing run keeps its
# whole output. The tests run over the deterministic simulation only.
# Usage: stability.sh <checkout> <target directory> <output directory> <runs>
set -u
checkout="$1"; target="$2"; out="$3"; runs="$4"
mkdir -p "$out"
cd "$checkout" || exit 2
binary=$(CARGO_TARGET_DIR="$target" cargo test --locked -p nexus-verifier-sandbox --lib --no-run \
  --message-format=json 2>/dev/null | python3 -c '
import json, sys
for line in sys.stdin:
    message = json.loads(line)
    if message.get("reason") == "compiler-artifact" and message.get("executable") \
            and message["target"]["name"] == "nexus_verifier_sandbox" and message["profile"]["test"]:
        print(message["executable"])
')
[ -x "$binary" ] || { echo "no unit-test binary" >&2; exit 2; }
{
  echo "binary: $binary"
  echo "sha256: $(sha256sum "$binary" | cut -d' ' -f1)"
} > "$out/stability.txt"
pass=0; fail=0
for i in $(seq 1 "$runs"); do
  if "$binary" -q > "$out/run.tmp" 2>&1; then pass=$((pass + 1)); else fail=$((fail + 1)); cp "$out/run.tmp" "$out/failed-default-$i.txt"; fi
  echo "default threads run $i: $(grep 'test result' "$out/run.tmp")" >> "$out/stability.txt"
done
single=$(( runs / 5 ))
spass=0; sfail=0
for i in $(seq 1 "$single"); do
  if "$binary" -q --test-threads=1 > "$out/run.tmp" 2>&1; then spass=$((spass + 1)); else sfail=$((sfail + 1)); cp "$out/run.tmp" "$out/failed-single-$i.txt"; fi
  echo "one thread run $i: $(grep 'test result' "$out/run.tmp")" >> "$out/stability.txt"
done
rm -f "$out/run.tmp"
echo "default threads: $pass passed, $fail failed of $runs; one thread: $spass passed, $sfail failed of $single" | tee -a "$out/stability.txt"
[ "$fail" -eq 0 ] && [ "$sfail" -eq 0 ]

#!/usr/bin/env bash
# P2-V1-R3B-I4-Q3-R1: the failed live baseline, preserved as it was recorded
# (never rewritten): run 37163435032 of the Phase Two live workflow at the
# base candidate d16d7ca3. Copies the run's bounded extraction (made by the Q2
# host-qualification work's read-only extractor), hashes the complete job log,
# excerpts its host-layer and live-step lines, and records the user manager's
# journal for the case-18 scope and the host's verifier residue now. Reads
# only: GitHub (API GETs), the journal and /sys/fs/cgroup. Nothing is
# dispatched, rerun, stopped or cleaned.
# Usage: baseline.sh <extraction directory of run 37163435032> <output directory>
set -eu
X="$1"; out="$2"
RUN=37163435032
UNIT=nexus-verifier-deb565108b334be56fbff5e9df044ead.scope
mkdir -p "$out"
for f in result.txt run.json jobs.json expected-cases.txt; do cp -p "$X/$f" "$out/$f"; done
{
  echo "run $RUN job log: $(wc -c < "$X/run.log") bytes, sha256 $(sha256sum "$X/run.log" | cut -d' ' -f1)"
  echo "(the complete log stays in the Q2 work directory; the API serves it while GitHub retains it)"
} > "$out/run-log.sha256.txt"
{
  echo "== run $RUN, step 7 (required host layers) and step 12 (the live suite), as logged"
  grep -aE $'\tRequired host layers|\tLive Phase Two isolation, escape and cleanup suite' "$X/run.log" | cut -f3- \
    | grep -avE '^\S+Z (\s*Compiling|\s*Fresh|\s*Checking|\s*Downloaded|\s*Finished `test` profile)' || true
} > "$out/live-step-excerpt.txt"
{
  echo "== user manager journal (uid 1001, user@1001.service) for the case-18 scope and the job, read-only, $(date -u +%FT%TZ)"
  journalctl --since '2026-10-04 00:58:00' --until '2026-10-04 01:03:10' -o short-iso-precise --no-pager \
    _SYSTEMD_UNIT=user@1001.service _UID=1001 | grep -aE "$UNIT|nexus-verifier-e8cfd1a2|Job phase2-linux-sandbox|Running job" || true
} > "$out/journal-excerpt.txt"
{
  echo "== host verifier residue now (read-only), $(date -u +%FT%TZ)"
  echo "nexus-verifier cgroups anywhere under /sys/fs/cgroup: $(find /sys/fs/cgroup -name 'nexus-verifier*' 2>/dev/null | wc -l)"
  echo "user-manager journal lines naming the case-18 scope after its timeout: $(journalctl --since '2026-10-04 01:03:00' -o cat --no-pager _SYSTEMD_UNIT=user@1001.service _UID=1001 | grep -ac "$UNIT" || true)"
  echo "workflow runs of ci-phase2-linux-sandbox.yml (all): $(gh run list --repo Nexus-os2026/nexus-os --workflow ci-phase2-linux-sandbox.yml --limit 20 --json databaseId --jq '[.[].databaseId]|join(",")')"
} > "$out/host-residue.txt"
ls -la "$out"

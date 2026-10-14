#!/usr/bin/env bash
# Freeze record, mission P3-C11-DR01-D2A1-FINAL-SOURCE-PROOF-READINESS-01. Written once, before the final gate.
# Usage: freeze.sh <packet-dir>
# Records the sha256 of every tool, every authored document, both matrices and the development receipts:
# receipts/00-FREEZE.sha256 (checked by the gate at its start and its end) and receipts/00-FREEZE.txt.
set -eu
PKT=$1
cd "$PKT"
test ! -e receipts/00-FREEZE.sha256
find . -type f ! -path './receipts/00-FREEZE.*' | LC_ALL=C sort | sed 's|^\./||' | xargs sha256sum > receipts/00-FREEZE.sha256
{
  echo "FREEZE — P3-C11-DR01-D2A1-FINAL-SOURCE-PROOF-READINESS-01"
  echo "date_utc=$(date -u +%Y-%m-%dT%H:%M:%SZ)"
  echo "python=$(python3 --version 2>&1) git=$(git --version)"
  echo "Frozen before the final gate: the files listed in receipts/00-FREEZE.sha256 ($(wc -l < receipts/00-FREEZE.sha256) files):"
  echo "the tools (tools/, with tools/revisions/), the authored review documents, both matrices and the"
  echo "development receipts (receipts/development/). The gate verifies this list at its start and its end."
  echo "Files written after the freeze: the gate's receipts (receipts/0[1-9]*, receipts/1*, receipts/2*,"
  echo "receipts/COMMANDS.log, receipts/GATE-SUMMARY.txt) and SHA256SUMS."
  echo
  cat receipts/00-FREEZE.sha256
} > receipts/00-FREEZE.txt
echo "frozen: $(wc -l < receipts/00-FREEZE.sha256) files"

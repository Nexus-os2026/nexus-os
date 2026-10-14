#!/usr/bin/env bash
# P3-C11-DR01-D2A1-AD1R3-READINESS-01 — the declared final mechanical gate.
# Runs only after receipts/03-freeze.txt exists. Verifies every frozen input against it first; a mismatch stops.
# Then produces the final draft, the R2->R3 diff and every check output, recording each exact command and exit code
# in receipts/04-final-gate.txt. Any unexpected exit code is a gate failure: the script reports GATE FAILED and the
# mission stops (no checker change, no rerun). Builds, runs or mutates nothing of Nexus.
set -u
M=/home/nexus/nexus-audits/claude-architect/p3-c11-dr01-d2a1-ad1r3-readiness-01
A=/home/nexus/nexus-audits/claude-architect
CD=$A/p3-c11-dr01-d2a1-ad1r1-mut-readiness-01/contract
CON=$CD/P3-C11-D2A1-CONTRACT-02-R1.md
R02=$CD/P3-C11-R3-AMENDMENT-01-RESPONSE-02-REVIEW-02.md
R03=$CD/P3-C11-R3-AMENDMENT-01-RESPONSE-03-REVIEW-01.md
R2=$A/p3-c11-dr01-d2a1-ad1r2-contract-evidence-01/drafts/P3-C11-D2A1-CONTRACT-02-R1-ADDENDUM-AD1-R2-DRAFT.md
DEFS=$A/p3-c11-dr01-d2a1-campaign-01/tools/stage5-prepared-NOT-EXECUTED/controls_defs.py
REPO=/home/nexus/NEXUS/nexus-os
SRC=42dd1bd1cf21ffdea5968b256d2983f6575fab5b
R3=$M/drafts/P3-C11-D2A1-CONTRACT-02-R1-ADDENDUM-AD1-R3-DRAFT.md
OUT=$M/receipts/04-final-gate.txt
cd "$M" || exit 2
test -e "$OUT" && { echo "REFUSED: $OUT exists (the gate runs once)"; exit 2; }
fail=0
{
echo "P3-C11-DR01-D2A1-AD1R3-READINESS-01 — FINAL MECHANICAL GATE   $(date -Iseconds)"
echo "gate script sha256 $(sha256sum "$0" | cut -d' ' -f1)"
echo
echo "[F] frozen inputs vs receipts/03-freeze.txt (sha256sum -c)"
grep -E '^[0-9a-f]{64}  ' receipts/03-freeze.txt > /tmp/.ad1r3-freeze.$$ || true
if sha256sum --quiet -c /tmp/.ad1r3-freeze.$$; then echo "    exit 0: every frozen input matches ($(wc -l < /tmp/.ad1r3-freeze.$$) entries)"; else echo "    MISMATCH: STOP"; fail=1; fi
rm -f /tmp/.ad1r3-freeze.$$
for f in "$CON:cac6535115fb333e68e6c3c045e444342c950ab7a735b7b7deea21cb2cb53b3c" "$R02:6dfb14560a8b2d22fe4e6d517891491b926423941179d0965eb38bc52aeecdce" "$R03:1f964d7515d94f3d21e55d53f23b67fb99a67cb6307b6aacef2c585dcd39fb35" "$R2:53cdb3d6ea8683a11ccaf90a6671a37b42dd5db178b599cfeb421b898a089ebc" "$DEFS:ef0420a1a69a756d4e71b3a746e39f7e29933643c121645f44dce32c7e4a108f"; do
  p=${f%%:*}; h=${f##*:}; a=$(sha256sum "$p" | cut -d' ' -f1)
  [ "$a" = "$h" ] && echo "    OK  $(basename "$p") $h" || { echo "    MISMATCH $(basename "$p") $a != $h: STOP"; fail=1; }
done
[ $fail = 0 ] || { echo; echo "GATE FAILED (provenance)"; exit 1; }
step() { # name expected-exit command...
  local name=$1 want=$2; shift 2
  echo; echo "[$name] \$ $*"
  "$@"; local rc=$?
  if [ "$rc" = "$want" ]; then echo "    exit $rc (expected $want): OK"; else echo "    exit $rc (expected $want): GATE FAILURE"; fail=1; fi
}
step G1-fill 0 python3 tools/fill-ad1r3.py tools/ad1r3-template.md "$CON" "$R02" "$R03" "$R3"
echo "    AD-1-R3 sha256 $(sha256sum "$R3" | cut -d' ' -f1) bytes $(stat -c %s "$R3") lines $(wc -l < "$R3")"
step G2-diff 1 sh -c "diff -u --label AD-1-R2/P3-C11-D2A1-CONTRACT-02-R1-ADDENDUM-AD1-R2-DRAFT.md --label AD-1-R3/P3-C11-D2A1-CONTRACT-02-R1-ADDENDUM-AD1-R3-DRAFT.md '$R2' '$R3' > drafts/AD1-R2-to-AD1-R3.diff"
echo "    diff sha256 $(sha256sum drafts/AD1-R2-to-AD1-R3.diff | cut -d' ' -f1); +$(grep -c '^+[^+]' drafts/AD1-R2-to-AD1-R3.diff) -$(grep -c '^-[^-]' drafts/AD1-R2-to-AD1-R3.diff) lines"
step G3-check 0 sh -c "python3 tools/check-ad1r3.py '$R3' '$CON' '$R02' '$R03' '$REPO' $SRC '$A' > checks/01-ad1r3-mechanical-checks.txt 2>&1"
echo "    $(tail -1 checks/01-ad1r3-mechanical-checks.txt)"
step G4-negative-controls 0 sh -c "python3 tools/checker-negative-controls.py tools/check-ad1r3.py '$R3' '$R2' '$CON' '$R02' '$R03' '$REPO' $SRC '$A' > checks/06-checker-negative-controls.txt 2>&1"
echo "    $(tail -1 checks/06-checker-negative-controls.txt)"
step G5-classification-cases 0 sh -c "python3 tools/classify-cases.py > checks/05-classification-cases.txt 2>&1"
echo "    $(tail -1 checks/05-classification-cases.txt)"
step G6-defs-anchor 0 sh -c "python3 tools/defs-anchor-check.py '$DEFS' '$REPO' $SRC C9 C10 C24 C4 > checks/03-defs-anchor-check.txt 2>&1"
echo "    $(tail -1 checks/03-defs-anchor-check.txt)"
step G7-preflight 0 sh -c "python3 tools/check-preflight.py preflight/C9-C10-C24-C4-ATTRIBUTION-PREFLIGHT.md '$CON' '$REPO' $SRC '$A' > checks/04-preflight-checks.txt 2>&1"
echo "    $(tail -1 checks/04-preflight-checks.txt)"
step G8-obligation-scan 0 sh -c "python3 tools/r2-r3-obligation-scan.py '$R2' '$R3' > checks/07-r2-r3-obligation-scan.txt 2>&1"
echo "    $(head -1 checks/07-r2-r3-obligation-scan.txt)"
step G8b-scan-count-matches-review 0 grep -q '; 30 obligation-bearing sentences not verbatim in AD-1-R3$' checks/07-r2-r3-obligation-scan.txt
step G9-inert-copy 0 sh -c "cp '$DEFS' inert/controls_defs.py.txt && cmp '$DEFS' inert/controls_defs.py.txt"
echo "    inert/controls_defs.py.txt sha256 $(sha256sum inert/controls_defs.py.txt | cut -d' ' -f1)"
step G10-authored-text-hygiene 0 sh -c "! grep -n '[[:space:]]\$' '$R3' preflight/*.md checks/02-r2-to-r3-diff-review.txt && ! grep -c -P '\x00' '$R3' preflight/*.md >/dev/null"
echo
if [ $fail = 0 ]; then echo "FINAL GATE: PASS (every step exited as expected)"; else echo "GATE FAILED: STOP"; fi
} > "$OUT" 2>&1
cat "$OUT"
exit $fail

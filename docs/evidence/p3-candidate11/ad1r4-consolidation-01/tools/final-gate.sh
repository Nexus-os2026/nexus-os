#!/usr/bin/env bash
# P3-C11-DR01-D2A1-AD1R4-CONSOLIDATION-01 — the declared final gate. Derived from AD-1-R3's final-gate.sh.
# Runs only after receipts/03-freeze.txt exists, and only once (it refuses if receipts/04-final-gate.txt exists).
# Verifies every frozen file and every authenticated input first; a mismatch stops. Then produces AD-1-R4, the
# R3 -> R4 diff, every check output and the rendered matrix, recording each exact command and exit code in
# receipts/04-final-gate.txt. Any unexpected exit code is a gate failure: GATE FAILED, and the mission stops (no tool,
# template or matrix is changed afterwards, and the gate is not rerun). Builds, runs or mutates nothing of Nexus.
#     final-gate.sh <evidence-dir>   (the evidence dir is the mission directory; the run must be from it)
set -u
M=/home/nexus/nexus-audits/claude-architect/p3-c11-dr01-d2a1-ad1r4-consolidation-01
[ "${1:-$M}" = "$M" ] || { echo "REFUSED: evidence dir must be $M"; exit 2; }
A=/home/nexus/nexus-audits/claude-architect
REPO=/home/nexus/NEXUS/nexus-os
SRC=42dd1bd1cf21ffdea5968b256d2983f6575fab5b
B=a464a36621523f539d2554a6b176d678c7f885ea
I=$M/inputs
CON=$I/CONTRACT-02-R1.md; R02=$I/REVIEW-02.md; R03=$I/REVIEW-03-01.md; R3=$I/AD1-R3.md; DISP=$I/DISPOSITION-01.md
PROTO=$I/protocol.rs.42dd1bd1; DEFS=$I/controls_defs.py.txt
R4=$M/drafts/P3-C11-D2A1-CONTRACT-02-R1-ADDENDUM-AD1-R4-DRAFT.md
OUT=$M/receipts/04-final-gate.txt
cd "$M" || exit 2
test -e receipts/03-freeze.txt || { echo "REFUSED: no freeze record"; exit 2; }
test -e "$OUT" && { echo "REFUSED: $OUT exists (the gate runs once)"; exit 2; }
test -e drafts || mkdir drafts
test -e "$R4" && { echo "REFUSED: $R4 exists"; exit 2; }
fail=0
{
echo "P3-C11-DR01-D2A1-AD1R4-CONSOLIDATION-01 — FINAL GATE   $(date -Iseconds)"
echo "gate script sha256 $(sha256sum "$0" | cut -d' ' -f1)"
echo
echo "[F] frozen files vs receipts/03-freeze.txt (sha256sum -c)"
grep -E '^[0-9a-f]{64}  ' receipts/03-freeze.txt > "$M/work/.freeze-list.$$" || true
if sha256sum --quiet -c "$M/work/.freeze-list.$$"; then echo "    exit 0: every frozen file matches ($(wc -l < "$M/work/.freeze-list.$$") entries)"; else echo "    MISMATCH: STOP"; fail=1; fi
rm -f "$M/work/.freeze-list.$$"
for f in "$CON:cac6535115fb333e68e6c3c045e444342c950ab7a735b7b7deea21cb2cb53b3c" "$R02:6dfb14560a8b2d22fe4e6d517891491b926423941179d0965eb38bc52aeecdce" \
         "$R03:1f964d7515d94f3d21e55d53f23b67fb99a67cb6307b6aacef2c585dcd39fb35" "$R3:485d9256932220644d7368d64b7b9b4d8df8a3a5c65aba4a1c5fcdf390ae286e" \
         "$DISP:a8a6d29376cf6f393a7eaa9047923719a339377caa554527a8878760cbdcf11c" "$PROTO:a5454944368df70fa4ce1c7cc4191d283e0a54b371137338fb999e59960aa296" \
         "$DEFS:ef0420a1a69a756d4e71b3a746e39f7e29933643c121645f44dce32c7e4a108f"; do
  p=${f%%:*}; h=${f##*:}; a=$(sha256sum "$p" | cut -d' ' -f1)
  [ "$a" = "$h" ] && echo "    OK  $(basename "$p") $h" || { echo "    MISMATCH $(basename "$p") $a != $h: STOP"; fail=1; }
done
g=$(git -C $REPO show $SRC:crates/nexus-governed-control/src/ledger/protocol.rs | sha256sum | cut -d' ' -f1)
[ "$g" = a5454944368df70fa4ce1c7cc4191d283e0a54b371137338fb999e59960aa296 ] && echo "    OK  protocol.rs at $SRC (git object) = inputs copy" || { echo "    MISMATCH protocol.rs git object: STOP"; fail=1; }
cmp -s "$DEFS" $A/p3-c11-dr01-d2a1-campaign-01/tools/stage5-prepared-NOT-EXECUTED/controls_defs.py && echo "    OK  controls_defs.py copy = CAMPAIGN-01 sealed original" || { echo "    MISMATCH controls_defs.py: STOP"; fail=1; }
[ "$(git -C $REPO rev-parse $B^{tree})" = e1f3e48f5b5fc7c792d99fb21e274f5c5e3af2b3 ] && echo "    OK  evidence commit B tree e1f3e48f" || { echo "    MISMATCH B: STOP"; fail=1; }
[ $fail = 0 ] || { echo; echo "GATE FAILED (provenance)"; exit 1; }
step() { # name expected-exit command...
  local name=$1 want=$2; shift 2
  echo; echo "[$name] \$ $*"
  "$@"; local rc=$?
  if [ "$rc" = "$want" ]; then echo "    exit $rc (expected $want): OK"; else echo "    exit $rc (expected $want): GATE FAILURE"; fail=1; fi
}
step G1-fill 0 python3 tools/fill-ad1r4.py tools/ad1r4-template.md "$CON" "$R02" "$R03" "$PROTO" "$DEFS" "$R4"
echo "    AD-1-R4 sha256 $(sha256sum "$R4" | cut -d' ' -f1) bytes $(stat -c %s "$R4") lines $(wc -l < "$R4")"
step G2-diff 1 sh -c "diff -u --label AD-1-R3/P3-C11-D2A1-CONTRACT-02-R1-ADDENDUM-AD1-R3-DRAFT.md --label AD-1-R4/P3-C11-D2A1-CONTRACT-02-R1-ADDENDUM-AD1-R4-DRAFT.md '$R3' '$R4' > drafts/AD1-R3-to-AD1-R4.diff"
echo "    diff sha256 $(sha256sum drafts/AD1-R3-to-AD1-R4.diff | cut -d' ' -f1); +$(grep -c '^+[^+]' drafts/AD1-R3-to-AD1-R4.diff) -$(grep -c '^-[^-]' drafts/AD1-R3-to-AD1-R4.diff) lines"
step G3-check 0 sh -c "python3 tools/check-ad1r4.py '$R4' '$CON' '$R02' '$R03' '$PROTO' '$DEFS' '$REPO' $SRC '$A' $B > checks/01-check-ad1r4.txt 2>&1"
echo "    $(tail -1 checks/01-check-ad1r4.txt)"
step G4-contradiction-scan 0 sh -c "python3 tools/contradiction-scan.py '$R4' > checks/02-contradiction-scan.txt 2>&1"
echo "    $(tail -1 checks/02-contradiction-scan.txt)"
step G5-matrix 0 sh -c "python3 tools/check-matrix.py matrix/CONSOLIDATION-MATRIX.tsv '$R3' '$DISP' '$R4' tools/obligation-scan.py > checks/03-check-matrix.txt 2>&1"
echo "    $(tail -1 checks/03-check-matrix.txt)"
step G6-classification-cases 0 sh -c "python3 tools/classify-cases-r4.py '$R4' > checks/04-classification-cases.txt 2>&1"
echo "    $(tail -1 checks/04-classification-cases.txt)"
step G7-negative-controls 0 sh -c "python3 tools/negative-controls-r4.py tools '$R4' matrix/CONSOLIDATION-MATRIX.tsv '$R3' '$DISP' '$CON' '$R02' '$R03' '$PROTO' '$DEFS' '$REPO' $SRC '$A' $B > checks/05-negative-controls.txt 2>&1"
echo "    $(tail -1 checks/05-negative-controls.txt)"
step G8-obligation-scan-r3 0 sh -c "python3 tools/obligation-scan.py '$R3' '$R4' R3 > checks/06-obligation-scan-r3.txt 2>&1"
echo "    $(head -1 checks/06-obligation-scan-r3.txt)"
step G8b-obligation-scan-disposition 0 sh -c "python3 tools/obligation-scan.py '$DISP' '$R4' DISP all > checks/07-obligation-scan-disposition.txt 2>&1"
echo "    $(head -1 checks/07-obligation-scan-disposition.txt)"
step G9-review-coverage 0 sh -c "python3 tools/check-review.py '$R3' '$R4' review/R3-TO-R4-SEMANTIC-REVIEW.md > checks/08-review-coverage.txt 2>&1"
echo "    $(tail -1 checks/08-review-coverage.txt)"
step G10-defs-anchor 0 sh -c "python3 tools/defs-anchor-check.py '$DEFS' '$REPO' $SRC C10 C24 C9 C4 > checks/09-defs-anchor-check.txt 2>&1"
echo "    $(tail -1 checks/09-defs-anchor-check.txt)"
step G11-render-matrix 0 python3 tools/render-matrix.py matrix/CONSOLIDATION-MATRIX.tsv checks/06-obligation-scan-r3.txt checks/07-obligation-scan-disposition.txt matrix/CONSOLIDATION-MATRIX.md
step G12-authored-text-hygiene 0 sh -c "! grep -n '[[:space:]]\$' '$R4' review/R3-TO-R4-SEMANTIC-REVIEW.md matrix/CONSOLIDATION-MATRIX.tsv matrix/CONSOLIDATION-MATRIX.md matrix/README.md && ! grep -c -P '\x00' '$R4' review/*.md matrix/* >/dev/null"
echo
if [ $fail = 0 ]; then echo "FINAL GATE: PASS (every step exited as expected)"; else echo "GATE FAILED: STOP"; fi
} > "$OUT" 2>&1
cat "$OUT"
exit $fail

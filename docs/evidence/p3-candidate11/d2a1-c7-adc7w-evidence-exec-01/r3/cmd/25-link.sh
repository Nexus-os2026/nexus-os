# Stage 9 — the (D) link of the accepted restored-side receipts 44-full-F-PT and 43-full-F-3 (read from git objects at f023cc3d; never rerun).
. /home/nexus/nexus-audits/claude-architect/p3-c11-dr01-d2a1-c7-w-evidence-exec-01/r3/cmd/00-gates.sh 25-link 8589934592
grep -q '^RESTORATION COMPLETE: EXACTLY_W$' $EV/receipts/50-restoration/restoration.txt || { echo "PRECONDITION FAILED: restoration is not complete"; exit 92; }
$PY $C/c7link.py link $EV/receipts/60-link $WH $WM $EV/receipts/30-bwc/B-W-C.json $EV/receipts/12-pin $EV/preflight/sonnet-7b42496f/mission.txt
echo "exit=$?"

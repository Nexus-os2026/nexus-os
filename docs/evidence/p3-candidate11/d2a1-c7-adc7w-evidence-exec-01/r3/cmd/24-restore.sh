# Stage 8 — preserve evidence first, then the ONE authenticated restoration of the mutant worktree to EXACTLY_W
# (7.3A R-1..R-8 inside c7wt restore; git apply -R once, 60 s), then E-W7's sha256 after restoration (7.4).
# Used identically after a successful sequence and after any stop at or after the mutant patch.
. /home/nexus/nexus-audits/claude-architect/p3-c11-dr01-d2a1-c7-w-evidence-exec-01/r3/cmd/00-gates.sh 24-restore 8589934592
EXE=$(/usr/bin/python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["executable"]["path"])' $EV/receipts/12-pin/E-W7.json)
SHA=$(/usr/bin/python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["executable"]["sha256"])' $EV/receipts/12-pin/E-W7.json)
O=$EV/receipts/50-restoration
$PY $C/c7wt.py preserve $EV $O/pre-restore
echo "preserve exit=$?"
$PY $C/c7wt.py restore $EV $O
rc=$?
echo "restore exit=$rc"
now=$(sha256sum "$EXE" | cut -d' ' -f1)
echo "E-W7 $EXE sha256 after restoration: $now (pinned $SHA): $([ "$now" = "$SHA" ] && echo MATCH || echo MISMATCH)" | tee $O/E-W7-sha256-after-restoration.txt
$PY $C/c7wt.py verify H W $O/healthy-EXACTLY_W-after.txt
echo "healthy verify exit=$?"
exit $rc

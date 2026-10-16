# Stage 3: the eight supervised 6A listings of E-W (each: 2700 s, 16 GiB, zero swap; one at a time).
. /home/nexus/nexus-audits/claude-architect/p3-c11-dr01-d2a1-c7-w-evidence-exec-01/r3/cmd/00-gates.sh 17-list-E-W 8589934592
[ -f $EV/receipts/12-pin/E-W.json ] && [ -f $EV/receipts/12-pin/E-W7.json ] || { echo "PRECONDITION FAILED: both executables must be pinned"; exit 92; }
$PY $C/c7bwc.py list $EV E-W
echo "exit=$?"

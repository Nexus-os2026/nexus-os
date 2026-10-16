# Stage 2a: create the separately owned mutant worktree at exact W-COMMIT (AD-C7-W 13.3 step 4).
. /home/nexus/nexus-audits/claude-architect/p3-c11-dr01-d2a1-c7-w-evidence-exec-01/r3/cmd/00-gates.sh 13-create-M 8589934592
[ -f $EV/receipts/12-pin/E-W.json ] || { echo "PRECONDITION FAILED: E-W is not pinned"; exit 92; }
$PY $C/c7wt.py create M $EV
echo "exit=$?"

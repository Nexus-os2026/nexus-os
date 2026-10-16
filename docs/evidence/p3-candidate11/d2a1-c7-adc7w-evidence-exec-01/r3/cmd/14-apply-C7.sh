# Stage 2b: apply the frozen C7 patch ONCE (worktree only, never staged) and verify EXACTLY_W7 (mutant blob 0d4ab992...).
. /home/nexus/nexus-audits/claude-architect/p3-c11-dr01-d2a1-c7-w-evidence-exec-01/r3/cmd/00-gates.sh 14-apply-C7 8589934592
[ -f $EV/receipts/00-worktrees/creation-M.json ] || { echo "PRECONDITION FAILED: no creation record for M"; exit 92; }
$PY $C/c7wt.py apply $EV $EV/receipts/00-worktrees/apply-C7.txt
echo "exit=$?"

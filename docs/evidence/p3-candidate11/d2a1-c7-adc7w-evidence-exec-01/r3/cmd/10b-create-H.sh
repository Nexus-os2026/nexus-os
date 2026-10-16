# Stage 1a (second gate attempt; the first, 10-create-H, failed closed at its v1 process gate and started nothing):
# create the owned, isolated healthy worktree at exact W-COMMIT (AD-C7-W 13.3 step 3).
. /home/nexus/nexus-audits/claude-architect/p3-c11-dr01-d2a1-c7-w-evidence-exec-01/r3/cmd/00-gates.sh 10b-create-H 8589934592
$PY $C/c7wt.py create H $EV
echo "exit=$?"

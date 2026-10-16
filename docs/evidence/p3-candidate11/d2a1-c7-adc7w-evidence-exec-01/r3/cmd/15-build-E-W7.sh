# Stage 2c: the ONE build of E-W7 from W7-SRC in the mutant worktree (AD-C7-W 7.1). Disk gate as for E-W.
. /home/nexus/nexus-audits/claude-architect/p3-c11-dr01-d2a1-c7-w-evidence-exec-01/r3/cmd/00-gates.sh 15-build-E-W7 10737418240
grep -q '^EXACTLY_W7 after the patch: OK$' $EV/receipts/00-worktrees/apply-C7.txt || { echo "PRECONDITION FAILED: W7-SRC not verified"; exit 92; }
$PY $C/c7sup.py build $EV/receipts/11-build-E-W7 $WM E-W7
rc=$?
echo "exit=$rc"
echo "disk available after build: $(( $(stat -f -c '%a * %S' /home/nexus/NEXUS) )) bytes (floor 8589934592)"
exit $rc

# Stage 1b: the ONE build of E-W from W-SRC in the healthy worktree (AD-C7-W 7.1; c7sup: 2 jobs, 900 s, 16 GiB, zero swap).
# Disk gate: 8 GiB floor + 2 GiB projected build footprint (precedent target ~0.77 GB), so the floor is maintained after the build.
. /home/nexus/nexus-audits/claude-architect/p3-c11-dr01-d2a1-c7-w-evidence-exec-01/r3/cmd/00-gates.sh 11-build-E-W 10737418240
$PY $C/c7sup.py build $EV/receipts/10-build-E-W $WH E-W
rc=$?
echo "exit=$rc"
echo "disk available after build: $(( $(stat -f -c '%a * %S' /home/nexus/NEXUS) )) bytes (floor 8589934592)"
exit $rc

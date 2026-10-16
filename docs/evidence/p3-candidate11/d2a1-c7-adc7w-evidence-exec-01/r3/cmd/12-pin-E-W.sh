# Stage 1c: pin E-W from its build receipt (c7bwc pin re-checks the build receipt and re-hashes the executable).
. /home/nexus/nexus-audits/claude-architect/p3-c11-dr01-d2a1-c7-w-evidence-exec-01/r3/cmd/00-gates.sh 12-pin-E-W 8589934592
$PY $C/c7bwc.py pin $EV E-W
echo "exit=$?"

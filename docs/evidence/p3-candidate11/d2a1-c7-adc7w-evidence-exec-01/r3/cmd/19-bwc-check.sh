# Stage 3: judge NI-6 (build-time clause), NI-7, NI-8 and the 6A four-way agreement; record B-W/C (O-6). Exit 0 = MECHANICALLY SATISFIED.
. /home/nexus/nexus-audits/claude-architect/p3-c11-dr01-d2a1-c7-w-evidence-exec-01/r3/cmd/00-gates.sh 19-bwc-check 8589934592
$PY $C/c7bwc.py check $EV
echo "exit=$?"

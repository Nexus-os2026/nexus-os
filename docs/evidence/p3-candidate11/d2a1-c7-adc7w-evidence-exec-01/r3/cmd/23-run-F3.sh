# Stage 7 — INFORMATIONAL F-3 on the SAME E-W7, A4 off, 30,000,000-state cap (AD-C7-W section 10; 13.3 step 10). Never a kill receipt.
. /home/nexus/nexus-audits/claude-architect/p3-c11-dr01-d2a1-c7-w-evidence-exec-01/r3/cmd/00-gates.sh 23-run-F3 8589934592
[ "$(/usr/bin/python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["verdict"])' $EV/receipts/42-run-B-mutant-witness/CLASSIFICATION.json)" = ADMISSIBLE_TWO_CANONICAL ] || { echo "PRECONDITION FAILED: (B) is not ADMISSIBLE_TWO_CANONICAL"; exit 92; }
EXE=$(/usr/bin/python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["executable"]["path"])' $EV/receipts/12-pin/E-W7.json)
SHA=$(/usr/bin/python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["executable"]["sha256"])' $EV/receipts/12-pin/E-W7.json)
R=$EV/receipts/43-run-F3-informational
$PY $C/c7sup.py invoke $R $WM F3-full_f3-E-W7 E-W7 "$EXE" "$SHA" 2700 -- ledger::protocol::model_tests::full_f3 --exact --ignored --nocapture --test-threads=1
echo "invoke exit=$?"
$PY $C/c7class.py classify F3 $R $EV/receipts/12-pin
echo "classify exit=$?"

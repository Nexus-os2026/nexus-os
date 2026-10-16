# Stage 6 — (B) MUTANT WITNESS on the SAME E-W7 (AD-C7-W 7.2; 13.3 step 9), then classification.
. /home/nexus/nexus-audits/claude-architect/p3-c11-dr01-d2a1-c7-w-evidence-exec-01/r3/cmd/00-gates.sh 22-run-B 8589934592
[ "$(/usr/bin/python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["verdict"])' $EV/receipts/41-run-A-full_fpt/CLASSIFICATION.json)" = FAIL_E11_ADMISSIBLE ] || { echo "PRECONDITION FAILED: (A) is not FAIL_E11_ADMISSIBLE"; exit 92; }
EXE=$(/usr/bin/python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["executable"]["path"])' $EV/receipts/12-pin/E-W7.json)
SHA=$(/usr/bin/python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["executable"]["sha256"])' $EV/receipts/12-pin/E-W7.json)
R=$EV/receipts/42-run-B-mutant-witness
$PY $C/c7sup.py invoke $R $WM B-mutant-witness-E-W7 E-W7 "$EXE" "$SHA" 2700 -- ledger::protocol::model_tests::c7_witness::c7_witness_p6_after_settlement_on_fpt --exact --nocapture --test-threads=1
echo "invoke exit=$?"
$PY $C/c7class.py classify B $R $EV/receipts/12-pin
echo "classify exit=$?"

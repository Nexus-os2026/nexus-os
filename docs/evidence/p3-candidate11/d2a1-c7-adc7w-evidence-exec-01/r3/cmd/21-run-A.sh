# Stage 5 — (A) ORIGINAL F-PT EXPLORER on E-W7, A4 on, 30,000,000-state cap (AD-C7-W 7.2; 13.3 step 8), then classification.
. /home/nexus/nexus-audits/claude-architect/p3-c11-dr01-d2a1-c7-w-evidence-exec-01/r3/cmd/00-gates.sh 21-run-A 8589934592
[ "$(/usr/bin/python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["verdict"])' $EV/receipts/40-run-C-healthy-witness/CLASSIFICATION.json)" = PASS_ADMISSIBLE ] || { echo "PRECONDITION FAILED: (C) is not PASS_ADMISSIBLE"; exit 92; }
EXE=$(/usr/bin/python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["executable"]["path"])' $EV/receipts/12-pin/E-W7.json)
SHA=$(/usr/bin/python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["executable"]["sha256"])' $EV/receipts/12-pin/E-W7.json)
R=$EV/receipts/41-run-A-full_fpt
$PY $C/c7sup.py invoke $R $WM A-full_fpt-E-W7 E-W7 "$EXE" "$SHA" 2700 -- ledger::protocol::model_tests::full_fpt --exact --ignored --nocapture --test-threads=1
echo "invoke exit=$?"
$PY $C/c7class.py classify A $R $EV/receipts/12-pin
echo "classify exit=$?"

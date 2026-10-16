# Stage 4 — (C) HEALTHY WITNESS on E-W (AD-C7-W 7.2; 13.3 step 7), then the independent classification (8.2).
. /home/nexus/nexus-audits/claude-architect/p3-c11-dr01-d2a1-c7-w-evidence-exec-01/r3/cmd/00-gates.sh 20-run-C 8589934592
/usr/bin/python3 -c 'import json,sys; j=json.load(open(sys.argv[1])); sys.exit(0 if j["satisfied"] is True and j["verdict"].startswith("B-W/C: MECHANICALLY SATISFIED") else 1)' $EV/receipts/30-bwc/B-W-C.json || { echo "PRECONDITION FAILED: B-W/C not mechanically satisfied"; exit 92; }
EXE=$(/usr/bin/python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["executable"]["path"])' $EV/receipts/12-pin/E-W.json)
SHA=$(/usr/bin/python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["executable"]["sha256"])' $EV/receipts/12-pin/E-W.json)
R=$EV/receipts/40-run-C-healthy-witness
$PY $C/c7sup.py invoke $R $WH C-healthy-witness-E-W E-W "$EXE" "$SHA" 2700 -- ledger::protocol::model_tests::c7_witness::c7_witness_p6_after_settlement_on_fpt --exact --nocapture --test-threads=1
echo "invoke exit=$?"
$PY $C/c7class.py classify C $R $EV/receipts/12-pin
echo "classify exit=$?"

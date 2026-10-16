#!/bin/bash
# P3-C11-DR01-D2A1-C7-COMPLIANT-EXEC-02 — stage runner. Authored under P3-C11-DR01-D2A1-C7-TOOL-REPAIR-RESUME-03 (EXEC-02 left
# cmd/ empty); modelled on EXEC-01's r3/cmd stage files.
#   run-stage.sh <stage>
# One call = one stage of the frozen AD-C7-W 13.3 sequence (the STAGES list of tools/c7tools/final.py). Before anything runs:
#   1. final.py precondition — the exact stage sequence so far (every earlier stage GATE PASS then END rc=0, in order). The only
#      exception is S10-restore on the 7.3A stop path. A stage never runs twice and nothing is retried.
#   2. stagegate.py — frozen tools (TOOLS.sha256), no cargo/rustc/test/runner process by executable identity, no campaign scope,
#      MemAvailable >= 18 GiB, free disk minus the stage's projected growth >= the 16 GiB floor, primary checkout unchanged,
#      destination ref absent, mutant blob absent. It appends the stage's GATE line to SEQUENCE.log.
# Then the stage's commands, then one END line (UTC, nanoseconds) with the stage's return code. Console: console/<stage>.txt.
# Nothing here kills, stops, cleans, resets, checks out or deletes anything. Every file it writes is created exclusively.
set -u -o pipefail -o noclobber
EV=/home/nexus/nexus-audits/claude-architect/p3-c11-dr01-d2a1-c7-compliant-exec-02
C=$EV/tools/c7tools
WH=/home/nexus/NEXUS/nexus-os-p3-c11-c7w02-healthy
WM=/home/nexus/NEXUS/nexus-os-p3-c11-c7w02-mutant
PY="/usr/bin/env PYTHONPYCACHEPREFIX=/nonexistent/c7w02-no-pycache PYTHONDONTWRITEBYTECODE=1 /usr/bin/python3 -B"
WITNESS=ledger::protocol::model_tests::c7_witness::c7_witness_p6_after_settlement_on_fpt
FPT=ledger::protocol::model_tests::full_fpt
F3=ledger::protocol::model_tests::full_f3
MIB=1048576
STAGE=${1:-}
case "$STAGE" in
  S01-authorisation-gate|S02-preflight-repeat|S03c-pin-E-W|S03e-stage3-gate|S04b-apply-C7|S04d-pin-E-W7|S05-bwc|S10-restore|S11-link|S12-final) PROJ=0 ;;
  S03a-create-H|S04a-create-M) PROJ=$((256 * MIB)) ;;
  S03b-build-E-W|S04c-build-E-W7) PROJ=$((3072 * MIB)) ;;
  S03d-list-E-W|S04e-list-E-W7|S06-run-C|S07-run-A|S08-run-B|S09-run-F3) PROJ=$((512 * MIB)) ;;
  *) echo "REFUSED: unknown stage '$STAGE'"; exit 2 ;;
esac
CON=$EV/console/$STAGE.txt
[ -e "$CON" ] && { echo "REFUSED: $CON exists (one stage, one run)"; exit 2; }

utc() { date -u +%Y-%m-%dT%H:%M:%S.%NZ; }
pinv() { $PY -c 'import json,sys; print(json.load(open(sys.argv[1]))["executable"][sys.argv[2]])' "$EV/receipts/12-pin/$1.json" "$2"; }

run_test() {  # <kind> <receipt-dir> <E-W|E-W7> <worktree> <libtest args...>: one supervised invocation, then its classification
  local kind=$1 dir=$EV/receipts/$2 role=$3 wt=$4 exe sha
  shift 4
  exe=$(pinv "$role" path) && sha=$(pinv "$role" sha256) || return 93
  $PY $C/c7sup.py invoke "$dir" "$wt" "$kind-$role" "$role" "$exe" "$sha" 2700 -- "$@" || return $?
  $PY $C/c7class.py classify "$kind" "$dir" "$EV/receipts/12-pin"
}

restore() {  # 7.3A: preserve evidence first; R-1..R-8 and one `git apply -R` inside c7wt restore; then E-W7 re-hashed (7.4)
  local O=$EV/receipts/50-restoration rc=0 exe sha now
  $PY $C/c7wt.py preserve $EV $O/pre-restore || return $?
  $PY $C/c7wt.py restore $EV $O || rc=$?
  if [ -f $EV/receipts/12-pin/E-W7.json ]; then
    exe=$(pinv E-W7 path) && sha=$(pinv E-W7 sha256) || return 93
    now=$(sha256sum "$exe" | cut -d' ' -f1)
    echo "E-W7 $exe sha256 after restoration: $now (pinned $sha): $([ "$now" = "$sha" ] && echo MATCH || echo MISMATCH)" > $O/E-W7-sha256-after-restoration.txt
    if [ "$now" != "$sha" ] && [ $rc -eq 0 ]; then rc=94; fi
  else
    echo "E-W7 was never pinned (stop before S04d-pin-E-W7): no executable to re-hash" > $O/E-W7-sha256-after-restoration.txt
  fi
  if ! $PY $C/c7wt.py verify H W $O/healthy-EXACTLY_W-after.txt && [ $rc -eq 0 ]; then rc=95; fi
  return $rc
}

stage_body() {
  case "$STAGE" in
    S01-authorisation-gate) $PY $C/gate.py $EV/gates/AUTH-S01.txt ;;
    S02-preflight-repeat) $PY $C/gate.py $EV/gates/AUTH-S02.txt && $PY $C/c7sup.py selftest $EV/gates/SUP-S02 ;;
    S03a-create-H) $PY $C/c7wt.py create H $EV ;;
    S03b-build-E-W) $PY $C/c7sup.py build $EV/receipts/10-build-E-W $WH E-W ;;
    S03c-pin-E-W) $PY $C/c7bwc.py pin $EV E-W ;;
    S03d-list-E-W) $PY $C/c7bwc.py list $EV E-W ;;
    S03e-stage3-gate) $PY $C/c7bwc.py stage3 $EV ;;
    S04a-create-M) $PY $C/c7wt.py create M $EV ;;
    S04b-apply-C7) $PY $C/c7wt.py apply $EV $EV/receipts/00-worktrees/apply-C7.txt ;;
    S04c-build-E-W7) $PY $C/c7sup.py build $EV/receipts/11-build-E-W7 $WM E-W7 ;;
    S04d-pin-E-W7) $PY $C/c7bwc.py pin $EV E-W7 ;;
    S04e-list-E-W7) $PY $C/c7bwc.py list $EV E-W7 ;;
    S05-bwc) $PY $C/c7bwc.py check $EV ;;
    S06-run-C) run_test C 40-run-C-healthy-witness E-W $WH $WITNESS --exact --nocapture --test-threads=1 ;;
    S07-run-A) run_test A 41-run-A-full_fpt E-W7 $WM $FPT --exact --ignored --nocapture --test-threads=1 ;;
    S08-run-B) run_test B 42-run-B-mutant-witness E-W7 $WM $WITNESS --exact --nocapture --test-threads=1 ;;
    S09-run-F3) run_test F3 43-run-F3-informational E-W7 $WM $F3 --exact --ignored --nocapture --test-threads=1 ;;
    S10-restore) restore ;;
    S11-link) $PY $C/c7link.py link $EV/receipts/60-link $WH $WM $EV/receipts/30-bwc/B-W-C.json $EV/receipts/12-pin $EV/MISSION.txt ;;
    S12-final) $PY $C/final.py classify $EV ;;
  esac
}

main() {
  echo "# stage $STAGE; runner sha256 $(sha256sum "${BASH_SOURCE[0]}" | cut -d' ' -f1); started $(utc)"
  $PY $C/final.py precondition $EV "$STAGE" || { echo "STAGE $STAGE NOT STARTED (the 13.3 sequence precondition refused)"; return 92; }
  $PY $C/stagegate.py "$STAGE" "$PROJ" || { echo "STAGE $STAGE NOT STARTED (the stage gate failed)"; return 91; }
  local rc=0
  stage_body || rc=$?
  echo "$(utc) END $STAGE rc=$rc" >> $EV/SEQUENCE.log
  echo "# stage $STAGE finished $(utc); rc=$rc"
  return $rc
}

main 2>&1 | tee "$CON"

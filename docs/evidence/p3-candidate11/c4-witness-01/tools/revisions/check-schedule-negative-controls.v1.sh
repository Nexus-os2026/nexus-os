#!/usr/bin/env bash
# Negative controls for check-schedule.py: each altered SCRATCH copy of the witness source must be rejected.
# Nothing is built or run; the copies never enter a worktree.
set -u
EV=/home/nexus/nexus-audits/claude-architect/p3-c11-dr01-d2a1-c4-witness-01
SRC="$1"; tmp=$(mktemp -d); fail=0
nc() { # label sed-expression
  sed "$2" "$SRC" > "$tmp/x.rs"
  if cmp -s "$SRC" "$tmp/x.rs"; then echo "  BROKEN CONTROL (no change made): $1"; fail=1; return; fi
  if python3 $EV/tools/check-schedule.py "$tmp/x.rs" > "$tmp/out" 2>&1; then echo "  NOT DETECTED: $1"; fail=1
  else echo "  detected: $1 — $(grep -m1 -E 'FAIL|not found' "$tmp/out" | sed 's/^ *//')"; fi
}
nc "an option count changed in C4_CONFLICT" '0,/consumed: &\[(0, 2), (0, 4), (0, 4), (0, 3)\]/s//consumed: \&[(0, 2), (0, 4), (0, 4), (0, 4)]/'
nc "the P8 changed end's branch changed" 's/branch: "append_acknowledgement: Committed",/branch: "append_acknowledgement: Committed(4)",/'
nc "the P4 offered stream extended" 's/offered: &\[0, 0, 0, 1, 0, 0\],/offered: \&[0, 0, 0, 1, 0, 0, 0],/'
nc "an injected ledger row" 's/^    w.faults = oracle.faults;$/    w.faults = oracle.faults;\n    w.rows.push(SimRow::Gating(0));/'
nc "a forced core state" 's/^    hook::take_counts();$/    hook::take_counts();\n    w.core.phase = Phase::Healthy;/'
nc "an Oracle::next enumeration" 's/^    let counts = hook::take_counts();$/    let counts = hook::take_counts();\n    let _ = oracle.next();/'
nc "a byte of the baseline prefix changed" '5s/explores/Explores/'
nc "an #[ignore] on the new test" 's/^fn c4_witness_p4_and_p8_on_f8() {$/#[ignore]\nfn c4_witness_p4_and_p8_on_f8() {/'
nc "an environment switch" 's/^    let mut lines = Vec::new();$/    let mut lines = Vec::new();\n    let _m = std::env::var("C4").is_ok();/'
nc "a 33-step scenario's step count (C4_SNAPSHOT removed from P8)" '0,/        C4_CONFIRM,\n        C4_WK_QUIET,/s/x/x/; /^const C4_P8/,/^};/{s/^        C4_SNAPSHOT,$//}'
rm -rf "$tmp"
[ $fail = 0 ] && echo "NEGATIVE CONTROLS: ALL DETECTED" || echo "NEGATIVE CONTROLS: FAILURES"
exit $fail

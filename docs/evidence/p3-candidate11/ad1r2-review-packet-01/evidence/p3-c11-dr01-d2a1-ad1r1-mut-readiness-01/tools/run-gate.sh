#!/usr/bin/env bash
# One cheap gate of P3-C11-DR01-D2A1-AD1R1-MUT-READINESS-01, recorded as one receipt.
# Copied from PT-FB-EXPERIMENT-01 tools/run-gate.sh. Changes: worktree, evidence, commit and pin; also records the pinned
# executable's sha256 before and after (a cargo gate must not rebuild it); nice -n 10 is in the command itself.
# Refuses an existing receipt; refuses to start unless HEAD is the mission commit and the worktree is clean.
#   run-gate.sh <receipt-path-relative-to-receipts> <command...>
set -u
name="$1"; shift
WORKTREE=/home/nexus/NEXUS/nexus-os-p3-c11-ad1r1-witness
EVIDENCE=/home/nexus/nexus-audits/claude-architect/p3-c11-dr01-d2a1-ad1r1-mut-readiness-01
EXPECT_COMMIT=42dd1bd1cf21ffdea5968b256d2983f6575fab5b
EXE=$WORKTREE/target/release/deps/nexus_governed_control-2faa0b9718d3e6a3
PIN=0983a19fc380b71250bb908844752a5b1121e29277a7181192906047e77d4f53
export CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=8 CARGO_TARGET_DIR="$WORKTREE/target"
receipt="$EVIDENCE/receipts/$name.txt"
if [ -e "$receipt" ]; then echo "REFUSED: $receipt exists"; exit 9; fi
mkdir -p "$(dirname "$receipt")"
cd "$WORKTREE" || exit 2
b=$(sha256sum "$EXE" | cut -d' ' -f1)
{
  echo "GATE $name"
  echo "commit: $(git rev-parse HEAD)  tree: $(git rev-parse 'HEAD^{tree}')  HEAD: $(git rev-parse --abbrev-ref HEAD) (experimental branch expected)"
  echo "worktree porcelain: $(git status --porcelain | wc -l)"
  echo "compiler: $(rustc --version)"
  echo "environment: CARGO_INCREMENTAL=${CARGO_INCREMENTAL} CARGO_BUILD_JOBS=${CARGO_BUILD_JOBS} CARGO_TARGET_DIR=${CARGO_TARGET_DIR}; cwd $(pwd)"
  printf 'command:'; printf ' %q' "$@"; echo
  echo "pinned executable before: $b $([ "$b" = "$PIN" ] && echo MATCH || echo MISMATCH)"
  echo "started: $(date -Is)"
  echo "--------------------------------------------------------------------------------"
} > "$receipt"
if [ "$(git rev-parse HEAD)" != "$EXPECT_COMMIT" ]; then echo "OUTCOME: NOT STARTED (commit)" >> "$receipt"; cat "$receipt"; exit 7; fi
if [ -n "$(git status --porcelain)" ]; then echo "OUTCOME: NOT STARTED (worktree dirty)" >> "$receipt"; cat "$receipt"; exit 6; fi
/usr/bin/time -f "peak RSS (KiB, /usr/bin/time): %M; wall: %e s; user %U s; sys %S s" "$@" >> "$receipt" 2>&1
status=$?
a=$(sha256sum "$EXE" | cut -d' ' -f1)
{
  echo "--------------------------------------------------------------------------------"
  echo "exit status: $status"
  echo "finished: $(date -Is)"
  echo "pinned executable after: $a $([ "$a" = "$PIN" ] && echo MATCH || echo MISMATCH)"
  echo "worktree porcelain after: $(git status --porcelain | wc -l)"
} >> "$receipt"
tail -5 "$receipt"
exit $status

#!/usr/bin/env bash
# One witness run of P3-C11-DR01-D2A1-AD1R1-MUT-READINESS-01, recorded as one receipt.
# Records the worktree's commit and porcelain, the blobs of model_tests.rs and protocol.rs as built
# (git hash-object of the working files), the executable's sha256 before and after, the exact command and its
# whole output. Refuses an existing receipt. Selects exactly one test with --exact.
#   run-witness.sh <receipt-path> <worktree> <executable> <test-name>
set -u
receipt="$1"; worktree="$2"; exe="$3"; name="$4"
if [ -e "$receipt" ]; then echo "REFUSED: $receipt exists"; exit 9; fi
mkdir -p "$(dirname "$receipt")"
cd "$worktree/crates/nexus-governed-control" || exit 2
b=$(sha256sum "$exe" | cut -d' ' -f1)
{
  echo "WITNESS RUN $name"
  echo "worktree: $worktree"
  echo "HEAD: $(git rev-parse HEAD)  tree: $(git rev-parse 'HEAD^{tree}')  branch: $(git rev-parse --abbrev-ref HEAD)"
  echo "porcelain:"; git status --porcelain | sed 's/^/  /'
  echo "model_tests.rs as built: blob $(git hash-object src/ledger/protocol/model_tests.rs) sha256 $(sha256sum src/ledger/protocol/model_tests.rs | cut -d' ' -f1)"
  echo "protocol.rs as built:    blob $(git hash-object src/ledger/protocol.rs) sha256 $(sha256sum src/ledger/protocol.rs | cut -d' ' -f1)"
  echo "compiler: $(rustc --version)"
  echo "executable: $exe"
  echo "executable sha256 before: $b ($(stat -c %s "$exe") bytes)"
  echo "test listing for the selector (--exact):"; "$exe" "ledger::protocol::model_tests::$name" --exact --list 2>&1 | sed 's/^/  /'
  printf 'command (cwd %s):' "$(pwd)"; printf ' %q' "$exe" "ledger::protocol::model_tests::$name" --exact --nocapture --test-threads=1; echo
  echo "started: $(date -Is)"
  echo "--------------------------------------------------------------------------------"
} > "$receipt"
RUST_BACKTRACE=0 "$exe" "ledger::protocol::model_tests::$name" --exact --nocapture --test-threads=1 >> "$receipt" 2>&1
status=$?
a=$(sha256sum "$exe" | cut -d' ' -f1)
{
  echo "--------------------------------------------------------------------------------"
  echo "exit status: $status"
  echo "finished: $(date -Is)"
  echo "executable sha256 after: $a $([ "$a" = "$b" ] && echo MATCH || echo MISMATCH)"
} >> "$receipt"
exit $status

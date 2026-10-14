#!/usr/bin/env bash
# The launcher for the authorised gc_symmetry_fpt run (Stage 3) and gc_symmetry_fb run (Stage 4) of
# P3-C11-DR01-D2A1-PT-FB-EXPERIMENT-01. Copied from GC-SYM-F3-01 tools/run-gc-symmetry-f3.sh. It has the same
# memory, swap and time enforcement and the same sampler. Changes: paths and pin; one argument, fpt or fb, picks the test;
# fb refuses to start unless the F-PT receipt exists with OUTCOME FPT_PASS. The outcome classification maps to this
# mission's statuses and is fixed here, before either run:
#   "symmetry inapplicable" anywhere in the output        -> BLOCKED (symmetry guard / mirror check failed)
#   "quotient inapplicable" (admission-quotient guard)    -> BLOCKED (admission-quotient precondition failed)
#   cgroup OOM kill, or exit 124/137 (timeout/kill)        -> RESOURCE_BLOCKED
#   outcome PASS and exit 0                               -> FPT_PASS / FB_PASS (both -> EXPERIMENTAL_PT_FB_PASS)
#   outcome FAIL                                          -> FPT_PROPERTY_FAILURE / FB_PROPERTY_FAILURE
#   outcome INCOMPLETE, the explorer's 45-minute time cap -> RESOURCE_BLOCKED
#   outcome INCOMPLETE, the 30,000,000 key cap            -> FPT_INCOMPLETE / FB_INCOMPLETE
#   anything else                                         -> BLOCKED (unexplained)
# It never invokes cargo and never retries. It refuses to run if its receipt already exists.
#
#   run-gc-symmetry-full.sh fpt|fb
set -u
WORKTREE=/home/nexus/NEXUS/nexus-os-p3-c11-pt-fb-experiment
CRATE="$WORKTREE/crates/nexus-governed-control"
EXE="$WORKTREE/target/release/deps/nexus_governed_control-2faa0b9718d3e6a3"
PIN=5cb3bf4ce78260ccddaf3dc06ca2e3b3c82b202b040ee71199c9d08965dba4af
COMMIT=2d83b74b3e9f0bfb6ca4763df56d57f3c044bcda
EVIDENCE=/home/nexus/nexus-audits/claude-architect/p3-c11-dr01-d2a1-pt-fb-experiment-01
case "${1:-}" in
  fpt) TEST=ledger::protocol::model_tests::gc_symmetry_fpt; CFG=F-PT; TAG=FPT; OUT="$EVIDENCE/receipts/stage3";;
  fb)  TEST=ledger::protocol::model_tests::gc_symmetry_fb;  CFG=F-B;  TAG=FB;  OUT="$EVIDENCE/receipts/stage4";;
  *) echo "usage: $0 fpt|fb"; exit 64;;
esac
name=gc-symmetry-full-$CFG
receipt="$OUT/$name.txt"
samples="$OUT/$name.cgroup-samples.txt"
need_kib=$(( (16 + 2) * 1024 * 1024 ))
time_cap=2700
unit="p3c11-ptfb-$1-$(date +%s)"
mkdir -p "$OUT"
if [ -e "$receipt" ]; then echo "REFUSED: $receipt exists; exactly one run is authorised"; exit 9; fi
if [ "$1" = fb ]; then
  fpt_receipt="$EVIDENCE/receipts/stage3/gc-symmetry-full-F-PT.txt"
  if ! grep -qx 'OUTCOME: FPT_PASS' "$fpt_receipt" 2>/dev/null; then
    echo "REFUSED: F-B runs only after an F-PT PASS ($fpt_receipt: $(grep '^OUTCOME:' "$fpt_receipt" 2>/dev/null || echo absent))"; exit 8
  fi
fi
cd "$WORKTREE" || exit 2
{
  echo "RECEIPT $name (gating-caller symmetry quotient composed with the admission observer quotient; experimental;"
  echo "         not the contract's explorer)"
  echo "started: $(date -Is)"
  echo "commit: $(git rev-parse HEAD)  tree: $(git rev-parse 'HEAD^{tree}')  HEAD: $(git rev-parse --abbrev-ref HEAD)"
  echo "worktree clean: $(test -z "$(git status --porcelain)" && echo yes || echo NO)"
  echo "model_tests.rs blob: $(git hash-object crates/nexus-governed-control/src/ledger/protocol/model_tests.rs)"
  echo "compiler (that built the executable): $(rustc --version)"
  echo "executable: $EXE"
  echo "executable size: $(stat -c %s "$EXE") bytes; mtime $(stat -c %y "$EXE")"
} > "$receipt"
if [ "$(git rev-parse HEAD)" != "$COMMIT" ]; then echo "OUTCOME: NOT STARTED (commit)" >> "$receipt"; cat "$receipt"; exit 7; fi
got=$(sha256sum "$EXE" | awk '{print $1}')
echo "executable sha256: $got" >> "$receipt"
if [ "$got" != "$PIN" ]; then
  echo "pinned sha256: $PIN — MISMATCH" >> "$receipt"; echo "OUTCOME: NOT STARTED (executable differs)" >> "$receipt"; cat "$receipt"; exit 5
fi
echo "pinned sha256: $PIN — MATCH" >> "$receipt"
if [ -n "$(git status --porcelain)" ]; then echo "OUTCOME: NOT STARTED (worktree dirty)" >> "$receipt"; cat "$receipt"; exit 6; fi

# ---- selection: exactly one test --------------------------------------------------------------------
cd "$CRATE" || exit 2
sel=$("$EXE" "$TEST" --exact --ignored --list --format terse 2>&1)
nsel=$(printf '%s\n' "$sel" | grep -c ': test$')
{
  echo "selection: $EXE $TEST --exact --ignored --list --format terse"
  printf '%s\n' "$sel" | sed 's/^/  selected: /'
  echo "selected test count: $nsel"
} >> "$receipt"
if [ "$nsel" -ne 1 ] || [ "$(printf '%s\n' "$sel" | grep ': test$')" != "$TEST: test" ]; then
  echo "OUTCOME: NOT STARTED (selection is not exactly $TEST)" >> "$receipt"; cat "$receipt"; exit 7
fi

# ---- pre-flight: headroom and conflicts -------------------------------------------------------------
avail=$(awk '/MemAvailable/ {print $2}' /proc/meminfo)
running=$(pgrep -c -f '^[^ ]*/deps/nexus_governed_control-[0-9a-f]{16}( |$)')  # argv[0] is the test executable (comm is truncated)
{
  echo "pre-flight $(date -Is):"
  echo "  MemTotal $(awk '/MemTotal/ {print $2}' /proc/meminfo) KiB; MemAvailable ${avail} KiB (needed ${need_kib} KiB = 16 GiB cap + 2 GiB margin)"
  echo "  SwapTotal $(awk '/SwapTotal/ {print $2}' /proc/meminfo) KiB; SwapFree $(awk '/SwapFree/ {print $2}' /proc/meminfo) KiB (the run's scope has swap disabled)"
  echo "  load average: $(cut -d' ' -f1-3 /proc/loadavg); online CPUs: $(nproc)"
  echo "  disk free on the worktree filesystem: $(df -h --output=avail "$WORKTREE" | tail -1 | tr -d ' ')"
  echo "  cargo/rustc/clippy processes: $(ps -C cargo,rustc,clippy-driver --no-headers | wc -l); governed-control test executables running: $running; other p3c11 scopes: $(systemctl --user list-units --type=scope --no-legend 'p3c11*' | wc -l)"
  echo "  ollama processes: $(pgrep -c -x ollama) (not touched)"
  echo "  largest resident processes (comm only; none is stopped by this launcher):"
  ps -eo rss=,comm= --sort=-rss | head -8 | awk '{printf "    %10d KiB  %s\n", $1, $2}'
} >> "$receipt"
if [ "$avail" -lt "$need_kib" ]; then
  echo "OUTCOME: NOT STARTED (not enough free memory; no workload was stopped)" >> "$receipt"; cat "$receipt"; exit 3
fi
if [ "$running" -ne 0 ] || [ "$(systemctl --user list-units --type=scope --no-legend 'p3c11*' | wc -l)" -ne 0 ]; then
  echo "OUTCOME: NOT STARTED (a conflicting exploration or scope is running)" >> "$receipt"; cat "$receipt"; exit 3
fi

# ---- run ---------------------------------------------------------------------------------------------
args=("$TEST" --exact --nocapture --test-threads=1 --ignored)
{
  echo "scope unit: $unit.scope"
  echo "cwd: $CRATE"
  echo "command: systemd-run --user --scope --unit=$unit -p MemoryMax=16G -p MemorySwapMax=0 -p RuntimeMaxSec=2710 --quiet /usr/bin/time -v timeout -s KILL $time_cap nice -n 10 $EXE ${args[*]}"
  echo "--------------------------------------------------------------------------------"
} >> "$receipt"

sampler() {
  local cg="" i
  for i in $(seq 1 100); do
    cg=$(systemctl --user show -p ControlGroup --value "$unit.scope" 2>/dev/null)
    [ -n "$cg" ] && [ -d "/sys/fs/cgroup$cg" ] && break
    sleep 0.1
  done
  if [ -z "$cg" ] || [ ! -d "/sys/fs/cgroup$cg" ]; then
    echo "sampler: scope cgroup not found" >> "$samples"; return
  fi
  local d="/sys/fs/cgroup$cg"
  {
    echo "scope cgroup: $d"
    echo "memory.max: $(cat "$d/memory.max")"
    echo "memory.swap.max: $(cat "$d/memory.swap.max")"
    echo "RuntimeMaxUSec (live): $(systemctl --user show -p RuntimeMaxUSec --value "$unit.scope" 2>/dev/null)"
    echo "procs at start: $(tr '\n' ' ' < "$d/cgroup.procs")"
    echo "time  memory.current  memory.peak  memory.swap.current  oom  oom_kill  host.MemAvailable(KiB)  cpu.usage_usec"
  } >> "$samples"
  while [ -d "$d" ]; do
    local cur peak swp oom oomk cpu
    cur=$(cat "$d/memory.current" 2>/dev/null) || break
    peak=$(cat "$d/memory.peak" 2>/dev/null)
    swp=$(cat "$d/memory.swap.current" 2>/dev/null)
    oom=$(awk '$1=="oom"{print $2}' "$d/memory.events" 2>/dev/null)
    oomk=$(awk '$1=="oom_kill"{print $2}' "$d/memory.events" 2>/dev/null)
    cpu=$(awk '$1=="usage_usec"{print $2}' "$d/cpu.stat" 2>/dev/null)
    echo "$(date +%T) $cur $peak $swp ${oom:-?} ${oomk:-?} $(awk '/MemAvailable/ {print $2}' /proc/meminfo) ${cpu:-?}" >> "$samples"
    sleep 2
  done
  echo "sampler: scope cgroup gone at $(date -Is)" >> "$samples"
}
: > "$samples"
sampler &
sampler_pid=$!

systemd-run --user --scope --unit="$unit" -p MemoryMax=16G -p MemorySwapMax=0 -p RuntimeMaxSec=2710 --quiet \
  /usr/bin/time -v timeout -s KILL "$time_cap" nice -n 10 "$EXE" "${args[@]}" >> "$receipt" 2>&1
status=$?
cd "$WORKTREE" || exit 2
wait "$sampler_pid" 2>/dev/null

{
  echo "--------------------------------------------------------------------------------"
  echo "exit status: $status"
  echo "finished: $(date -Is)"
  echo "scope after the run: $(systemctl --user show -p Result -p ActiveState "$unit.scope" 2>/dev/null | tr '\n' ' ')"
  echo "executable sha256 after the run: $(sha256sum "$EXE" | awk '{print $1}')"
  echo "worktree clean after the run: $(test -z "$(git status --porcelain)" && echo yes || echo NO)"
} >> "$receipt"
peak=$(grep -E 'Maximum resident set size' "$receipt" | tail -1 | awk '{print $NF}')
wall=$(grep -E 'Elapsed \(wall clock\) time' "$receipt" | tail -1 | awk '{print $NF}')
cg_peak=$(awk 'NF==8 && $3 ~ /^[0-9]+$/ {p=$3} END{print p}' "$samples")
cg_oomk=$(awk 'NF==8 && $6 ~ /^[0-9]+$/ {k=$6} END{print k}' "$samples")
cg_swmax=$(awk 'NF==8 && $4 ~ /^[0-9]+$/ && $4>m {m=$4} END{print m+0}' "$samples")
{
  echo "peak memory (maximum resident set size, KiB, as /usr/bin/time reports it): ${peak:-unknown}"
  echo "peak memory of the scope cgroup (memory.peak, last sample, bytes): ${cg_peak:-unknown}"
  echo "maximum memory.swap.current over all samples (bytes): ${cg_swmax}"
  echo "cgroup oom_kill events (last sample): ${cg_oomk:-unknown}"
  echo "wall clock (as /usr/bin/time reports it): ${wall:-unknown}"
} >> "$receipt"
scope_result=$(systemctl --user show -p Result --value "$unit.scope" 2>/dev/null)
outcome_line=$(grep -E '^  outcome: ' "$receipt" | tail -1 | awk '{print $2}')
if grep -q 'symmetry inapplicable' "$receipt"; then
  verdict="BLOCKED (symmetry guard: $(grep -m1 -o 'symmetry inapplicable[^"]*' "$receipt"); exit $status)"
elif grep -q 'quotient inapplicable' "$receipt"; then
  verdict="BLOCKED (an admission-quotient precondition failed: $(grep -m1 -o 'quotient inapplicable[^"]*' "$receipt"); exit $status)"
elif [ "$status" -ne 0 ] && { [ "$scope_result" = "oom-kill" ] || { [ -n "$cg_oomk" ] && [ "$cg_oomk" != "0" ]; }; }; then
  verdict="RESOURCE_BLOCKED (memory cap: cgroup OOM kill; exit $status)"
elif [ "$status" -eq 124 ] || [ "$status" -eq 137 ]; then
  verdict="RESOURCE_BLOCKED (killed: time or memory cap; exit $status)"
elif [ "$outcome_line" = "PASS" ] && [ "$status" -eq 0 ]; then
  verdict="${TAG}_PASS"
elif [ "$outcome_line" = "FAIL" ]; then
  verdict="${TAG}_PROPERTY_FAILURE ($(grep -E '^  FAIL: ' "$receipt" | head -1 | sed 's/^  FAIL: //'))"
elif [ "$outcome_line" = "INCOMPLETE" ] && grep -q '^  INCOMPLETE: the time cap' "$receipt"; then
  verdict="RESOURCE_BLOCKED (the explorer's own 45-minute time cap: $(grep -E '^  INCOMPLETE: ' "$receipt" | head -1 | sed 's/^  INCOMPLETE: //'))"
elif [ "$outcome_line" = "INCOMPLETE" ]; then
  verdict="${TAG}_INCOMPLETE ($(grep -E '^  INCOMPLETE: ' "$receipt" | head -1 | sed 's/^  INCOMPLETE: //'))"
else
  verdict="BLOCKED (exit $status; no recognised exploration outcome)"
fi
echo "OUTCOME: $verdict" >> "$receipt"
tail -12 "$receipt"

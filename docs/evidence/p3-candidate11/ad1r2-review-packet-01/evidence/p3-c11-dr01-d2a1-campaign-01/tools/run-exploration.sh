#!/usr/bin/env bash
# The launcher for one exploration run of P3-C11-DR01-D2A1-CAMPAIGN-01.
# Copied from SOURCE-R2 tools/run-exploration.sh (itself from SOURCE-R1 and CAP60-01). Same caps, same
# enforcement, same verdict rules. Changes: paths; WORKTREE and OUT may be overridden by the environment
# (mutation worktree); CARGO_INCREMENTAL=0 and -j 8 for the build; the build's "fresh" flag is recorded;
# if EXPECT_EXE_SHA256 is set and the built executable differs, the run does not start (NOT STARTED).
#
#   run-exploration.sh <receipt-name> <test-filter> <release|dev> [--ignored]
#
# Build: cargo builds the lib test executable first (--no-run); the time cap
#   covers the run only. The exact executable is recorded with its sha256 and
#   run directly (not through cargo), with the crate directory as cwd, as
#   cargo test would, and one test thread.
# Before the run: needs MemAvailable >= 16 GiB + 2 GiB margin; otherwise the
#   run does not start and is reported NOT STARTED. No workload is stopped.
# During the run, enforced here (not reported afterwards):
#   memory  a transient systemd user scope, MemoryMax=16G, MemorySwapMax=0;
#           the kernel's cgroup OOM killer ends the run at the cap.
#   time    timeout -s KILL 2700 (45 minutes) around the executable; the
#           scope's RuntimeMaxSec=2710 is a backstop only.
#   A sampler reads the live scope cgroup every 2 s: memory.max, memory.swap.max,
#   memory.current, memory.peak, memory.events (oom, oom_kill).
# After the run: PASS only if the test exited 0 and the explorer printed
#   "outcome: PASS". A cap reached, a killed run, an OOM kill or a missing
#   outcome line is INCOMPLETE. A property failure is FAIL.
set -u
name="$1"; filter="$2"; profile="$3"; shift 3
ignored_flag=""
for arg in "$@"; do
  case "$arg" in
    --ignored) ignored_flag="--ignored" ;;
  esac
done
case "$profile" in
  release) profile_flag="--release" ;;
  dev) profile_flag="" ;;
  *) echo "profile must be release or dev"; exit 2 ;;
esac
WORKTREE="${WORKTREE:-/home/nexus/NEXUS/nexus-os-p3-c11-d2a1-campaign-01}"
export CARGO_INCREMENTAL=0
CRATE="$WORKTREE/crates/nexus-governed-control"
EVIDENCE=/home/nexus/nexus-audits/claude-architect/p3-c11-dr01-d2a1-campaign-01
OUT="${OUT:-$EVIDENCE/receipts/explorations}"
mkdir -p "$OUT"
receipt="$OUT/$name.txt"
samples="$OUT/$name.cgroup-samples.txt"
need_kib=$(( (16 + 2) * 1024 * 1024 ))
time_cap=2700
unit="p3c11-camp01-$(echo "$name" | tr -c 'a-zA-Z0-9-' '-' | cut -c1-40)-$(date +%s)"
cd "$WORKTREE" || exit 2
{
  echo "RECEIPT $name"
  echo "started: $(date -Is)"
  echo "commit: $(git rev-parse HEAD)  tree: $(git rev-parse 'HEAD^{tree}')"
  echo "branch: $(git rev-parse --abbrev-ref HEAD)"
  echo "worktree clean: $(test -z "$(git status --porcelain)" && echo yes || echo NO)"
  git status --porcelain | sed 's/^/  dirty: /'
  echo "compiler: $(rustc --version)"
  rustc -vV | sed 's/^/  rustc -vV: /'
  echo "cargo: $(cargo --version)"
  echo "environment: CARGO_INCREMENTAL=${CARGO_INCREMENTAL} CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-<unset: worktree target/>} build jobs 8"
  echo "profile: $profile ${profile_flag}"
  if [ "$profile" = release ]; then
    echo "  [profile.release] in Cargo.toml: $(awk '/^\[profile.release\]/{p=1;next} /^\[/{p=0} p&&NF' Cargo.toml | tr '\n' ';')"
  fi
} > "$receipt"

cargo test -p nexus-governed-control --locked $profile_flag --lib --no-run -j 8 --message-format=json \
  2> "$receipt.build" > "$receipt.build.json" || {
  echo "OUTCOME: BUILD FAILED (see $name.txt.build)" >> "$receipt"; cat "$receipt"; exit 4; }
exe=$(python3 -c '
import json, sys
exes = []
for line in open(sys.argv[1]):
    try: m = json.loads(line)
    except ValueError: continue
    if m.get("reason") == "compiler-artifact" and m.get("executable") and m["target"]["name"] == "nexus_governed_control" and m["profile"]["test"]:
        exes.append(m["executable"])
print(exes[-1] if len(exes) == 1 else "")
' "$receipt.build.json")
fresh=$(python3 -c '
import json, sys
for line in open(sys.argv[1]):
    try: m = json.loads(line)
    except ValueError: continue
    if m.get("reason") == "compiler-artifact" and m.get("executable") and m["target"]["name"] == "nexus_governed_control" and m["profile"]["test"]:
        print(m.get("fresh"))
' "$receipt.build.json")
if [ -z "$exe" ] || [ ! -x "$exe" ]; then
  echo "OUTCOME: BUILD FAILED (no unique test executable found)" >> "$receipt"; cat "$receipt"; exit 4
fi
{
  echo "executable: $exe"
  echo "executable sha256: $(sha256sum "$exe" | awk '{print $1}')"
  echo "executable size: $(stat -c %s "$exe") bytes; mtime $(stat -c %y "$exe")"
  echo "build fresh (no recompilation of the test executable by this call): ${fresh}"
} >> "$receipt"
if [ -n "${EXPECT_EXE_SHA256:-}" ]; then
  got=$(sha256sum "$exe" | awk '{print $1}')
  if [ "$got" != "$EXPECT_EXE_SHA256" ]; then
    echo "expected executable sha256: $EXPECT_EXE_SHA256 — MISMATCH" >> "$receipt"
    echo "OUTCOME: NOT STARTED (executable differs from the expected one)" >> "$receipt"; cat "$receipt"; exit 5
  fi
  echo "expected executable sha256: $EXPECT_EXE_SHA256 — MATCH" >> "$receipt"
fi

# ---- pre-flight: headroom ------------------------------------------------------------------------
avail=$(awk '/MemAvailable/ {print $2}' /proc/meminfo)
{
  echo "pre-flight $(date -Is):"
  echo "  MemTotal $(awk '/MemTotal/ {print $2}' /proc/meminfo) KiB; MemAvailable ${avail} KiB (needed ${need_kib} KiB = 16 GiB cap + 2 GiB margin)"
  echo "  SwapTotal $(awk '/SwapTotal/ {print $2}' /proc/meminfo) KiB; SwapFree $(awk '/SwapFree/ {print $2}' /proc/meminfo) KiB (the run's scope has swap disabled)"
  echo "  load average: $(cut -d' ' -f1-3 /proc/loadavg); online CPUs: $(nproc)"
  echo "  disk free on the worktree filesystem: $(df -h --output=avail "$WORKTREE" | tail -1 | tr -d ' ')"
  echo "  launcher scheduling: nice $(nice); CPU affinity: $(taskset -cp $$ | sed 's/.*: //')"
  echo "  largest resident processes (comm only; none is stopped by this launcher):"
  ps -eo rss=,comm= --sort=-rss | head -8 | awk '{printf "    %10d KiB  %s\n", $1, $2}'
} >> "$receipt"
if [ "$avail" -lt "$need_kib" ]; then
  echo "OUTCOME: NOT STARTED (not enough free memory; no workload was stopped)" >> "$receipt"
  cat "$receipt"
  exit 3
fi

# ---- run -----------------------------------------------------------------------------------------
args=("$filter" --exact --nocapture --test-threads=1)
[ -n "$ignored_flag" ] && args+=("$ignored_flag")
{
  echo "scope unit: $unit.scope"
  echo "cwd: $CRATE"
  echo "command: systemd-run --user --scope --unit=$unit -p MemoryMax=16G -p MemorySwapMax=0 -p RuntimeMaxSec=2710 --quiet /usr/bin/time -v timeout -s KILL $time_cap $exe ${args[*]}"
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
    echo "procs at start: $(tr '\n' ' ' < "$d/cgroup.procs")"
    echo "time  memory.current  memory.peak  memory.swap.current  oom  oom_kill  host.MemAvailable(KiB)"
  } >> "$samples"
  while [ -d "$d" ]; do
    local cur peak swp oom oomk
    cur=$(cat "$d/memory.current" 2>/dev/null) || break
    peak=$(cat "$d/memory.peak" 2>/dev/null)
    swp=$(cat "$d/memory.swap.current" 2>/dev/null)
    oom=$(awk '$1=="oom"{print $2}' "$d/memory.events" 2>/dev/null)
    oomk=$(awk '$1=="oom_kill"{print $2}' "$d/memory.events" 2>/dev/null)
    echo "$(date +%T) $cur $peak $swp ${oom:-?} ${oomk:-?} $(awk '/MemAvailable/ {print $2}' /proc/meminfo)" >> "$samples"
    sleep 2
  done
  echo "sampler: scope cgroup gone at $(date -Is)" >> "$samples"
}
: > "$samples"
sampler &
sampler_pid=$!

cd "$CRATE" || exit 2
systemd-run --user --scope --unit="$unit" -p MemoryMax=16G -p MemorySwapMax=0 -p RuntimeMaxSec=2710 --quiet \
  /usr/bin/time -v timeout -s KILL "$time_cap" "$exe" "${args[@]}" >> "$receipt" 2>&1
status=$?
cd "$WORKTREE" || exit 2
wait "$sampler_pid" 2>/dev/null

{
  echo "--------------------------------------------------------------------------------"
  echo "exit status: $status"
  echo "finished: $(date -Is)"
  echo "scope after the run: $(systemctl --user show -p Result -p ActiveState "$unit.scope" 2>/dev/null | tr '\n' ' ')"
} >> "$receipt"
peak=$(grep -E 'Maximum resident set size' "$receipt" | tail -1 | awk '{print $NF}')
wall=$(grep -E 'Elapsed \(wall clock\) time' "$receipt" | tail -1 | awk '{print $NF}')
cg_peak=$(awk 'NF==7 && $3 ~ /^[0-9]+$/ {p=$3} END{print p}' "$samples")
cg_oomk=$(awk 'NF==7 && $6 ~ /^[0-9]+$/ {k=$6} END{print k}' "$samples")
{
  echo "peak memory (maximum resident set size, KiB, as /usr/bin/time reports it): ${peak:-unknown}"
  echo "peak memory of the scope cgroup (memory.peak, last sample, bytes): ${cg_peak:-unknown}"
  echo "cgroup oom_kill events (last sample): ${cg_oomk:-unknown}"
  echo "wall clock (as /usr/bin/time reports it): ${wall:-unknown}"
} >> "$receipt"
scope_result=$(systemctl --user show -p Result --value "$unit.scope" 2>/dev/null)
outcome_line=$(grep -E '^  outcome: ' "$receipt" | tail -1 | awk '{print $2}')
if [ "$status" -ne 0 ] && { [ "$scope_result" = "oom-kill" ] || { [ -n "$cg_oomk" ] && [ "$cg_oomk" != "0" ]; }; }; then
  verdict="INCOMPLETE (killed: memory cap, cgroup OOM kill; exit $status)"
elif [ "$status" -eq 124 ] || [ "$status" -eq 137 ]; then
  verdict="INCOMPLETE (killed: time or memory cap; exit $status)"
elif [ "$outcome_line" = "PASS" ] && [ "$status" -eq 0 ]; then
  verdict="PASS"
elif [ "$outcome_line" = "FAIL" ]; then
  verdict="FAIL ($(grep -E '^  FAIL: ' "$receipt" | head -1 | sed 's/^  FAIL: //'))"
elif [ "$outcome_line" = "INCOMPLETE" ]; then
  verdict="INCOMPLETE ($(grep -E '^  INCOMPLETE: ' "$receipt" | head -1 | sed 's/^  INCOMPLETE: //'))"
else
  verdict="FAILED (exit $status; no exploration outcome line)"
fi
echo "OUTCOME: $verdict" >> "$receipt"
tail -9 "$receipt"

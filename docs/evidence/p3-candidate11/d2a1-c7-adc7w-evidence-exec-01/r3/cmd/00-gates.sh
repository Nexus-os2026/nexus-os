# P3-C11-DR01-D2A1-C7-W-EVIDENCE-EXEC-01-OPUS-R3 — gate prologue, sourced (never executed alone) by every stage file.
#   . 00-gates.sh <stage-id> <minimum-available-bytes>
# Fail-closed: any failing gate exits the stage before anything runs. Nothing is retried.
set -u
EV=/home/nexus/nexus-audits/claude-architect/p3-c11-dr01-d2a1-c7-w-evidence-exec-01
S=/tmp/claude-1000/-home-nexus-NEXUS-nexus-os/7b42496f-8d31-44a9-b5c7-079991353147/scratchpad
C=$S/c7tools
P=/tmp/claude-1000/-home-nexus-NEXUS-nexus-os/35e734c1-5308-4f19-ae15-39e870b30974/scratchpad/preflight-r2
WH=/home/nexus/NEXUS/nexus-os-p3-c11-c7w-healthy
WM=/home/nexus/NEXUS/nexus-os-p3-c11-c7w-mutant
PY="env PYTHONPYCACHEPREFIX=/tmp/claude-1000/-home-nexus-NEXUS-nexus-os/882e437e-0db3-409e-8b8f-72205c69d44d/scratchpad/pycache-empty PYTHONDONTWRITEBYTECODE=1 /usr/bin/python3 -B"
STAGE=$1
MIN=$2
G=$EV/r3/gates/$STAGE.txt
[ -e "$G" ] && { echo "REFUSED: gate record $G exists (one stage, one run)"; exit 90; }
avail=$(( $(stat -f -c '%a * %S' /home/nexus/NEXUS) ))
ifree=$(stat -f -c '%d' /home/nexus/NEXUS)
# v2: match process NAMES (comm; a test executable's comm is its first 15 characters, `nexus_governed_`), not command
# lines. v1 used `pgrep -f`, which also matched a stale Oct-13 `until ! pgrep -f ...; do sleep 5; done` bash loop
# (PID 2306315, not this mission's, not a test process) that merely mentions an executable name; v1 failed closed.
procs=$( { pgrep -x -l 'cargo|rustc|rustup'; pgrep -x -l 'nexus_governed_'; } | tr '\n' ';')
scopes=$(systemctl --user list-units --state=active --no-legend 'p3c11-*' 'c4w01-*' 'fbk*' 2>/dev/null | wc -l)
if sha256sum --strict -c "$P/TOOLS.sha256" > /dev/null 2>&1; then tools=OK; else tools=FAILED; fi
memavail=$(awk '/^MemAvailable:/{print $2}' /proc/meminfo)
{
  echo "stage $STAGE gate at $(date -Is)"
  echo "disk available to uid $(id -u) (f_bavail*f_bsize, root-reserved excluded): $avail bytes; required >= $MIN"
  echo "inodes free: $ifree; required >= 1000000"
  echo "MemAvailable: $memavail KiB (c7sup additionally requires >= 18874368 KiB)"
  echo "conflicting cargo/rustc/rustup/test processes: '${procs}'"
  echo "active p3c11-*/c4w01-*/fbk* scopes: $scopes"
  echo "frozen helpers vs TOOLS.sha256 (12 entries): $tools"
} > "$G"
ok=1
[ "$avail" -ge "$MIN" ] || ok=0
[ "$ifree" -ge 1000000 ] || ok=0
[ -z "$procs" ] || ok=0
[ "$scopes" -eq 0 ] || ok=0
[ "$tools" = OK ] || ok=0
if [ $ok -ne 1 ]; then echo "GATE: FAILED — stage NOT STARTED" >> "$G"; cat "$G"; exit 91; fi
echo "GATE: PASS" >> "$G"
cat "$G"

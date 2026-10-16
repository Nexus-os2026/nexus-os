#!/bin/sh
# P3-C11-DR01-D2A1-C7-W-EVIDENCE-EXEC-01. Runs as the first process inside the transient systemd user scope.
# Records the scope's own cgroup and its enforced limits, runs the command, then records the cgroup's peak
# memory, peak swap and memory events, and exits with the command's status.
#   scope-wrap.sh <record-file> <command> [args...]
rec="$1"; shift
cg="/sys/fs/cgroup$(sed -n 's/^0:://p' /proc/self/cgroup)"
{
  echo "cgroup: $cg"
  echo "memory.max: $(cat "$cg/memory.max")"
  echo "memory.swap.max: $(cat "$cg/memory.swap.max")"
  echo "memory.high: $(cat "$cg/memory.high")"
  echo "memory.oom.group: $(cat "$cg/memory.oom.group")"
  echo "processes at start: $(tr '\n' ' ' < "$cg/cgroup.procs")"
  echo "started: $(date -Is)"
} > "$rec"
"$@"
rc=$?
{
  echo "finished: $(date -Is)"
  echo "command exit status: $rc"
  echo "memory.peak: $(cat "$cg/memory.peak")"
  echo "memory.swap.peak: $(cat "$cg/memory.swap.peak" 2>/dev/null || echo unavailable)"
  echo "memory.current: $(cat "$cg/memory.current")"
  sed 's/^/memory.events: /' "$cg/memory.events"
  sed 's/^/memory.swap.events: /' "$cg/memory.swap.events" 2>/dev/null
} >> "$rec"
exit $rc

#!/usr/bin/env bash
# P2-V1-R3B-I4-Q3-R1: the primary sources of the never-populated scope
# lifecycle, with each source's identity. systemd: the tree the host's
# systemd 255.4-1ubuntu8.17 was built from (Ubuntu's import tag with its
# debian/patches applied in order: P2-V1-R3B-I4-R3-R1's primary directory),
# each cited file compared with upstream systemd-stable v255.4. Linux:
# kernel/cgroup/cgroup.c at upstream tag v6.17 (the host runs Ubuntu's
# 6.17.0-35-generic, whose patched source is not here: the live run's own
# outcome is that kernel's behaviour). Every excerpt is located by its
# function's own line, never by a fixed number.
# Usage: primary_excerpts.sh <R3-R1 primary directory> <linux v6.17 cgroup.c> <output file>
set -eu
P="$1"; K="$2"; out="$3"
A="$P/ubuntu-applied-255.4-1ubuntu8.17"
S="$P/upstream-systemd-stable-v255.4"
U="$P/ubuntu-systemd-255.4-1ubuntu8.17"
at() { grep -n -m1 -F -- "$2" "$1" | cut -d: -f1; }
ex() { # ex root file anchor count label
  local n; n=$(at "$1/$2" "$3")
  [ -n "$n" ] || { echo "anchor not found: $2: $3" >&2; exit 2; }
  echo; echo "== $5: $2 lines $n-$((n + $4 - 1))"
  sed -n "$n,$((n + $4 - 1))p" "$1/$2" | nl -ba -v "$n" -w5 -s'  '
}
{
  echo "P2-V1-R3B-I4-Q3-R1 primary-source excerpts, $(date -u +%FT%TZ)"
  echo "host: $(systemctl --version | head -1); kernel $(uname -r)"
  echo "systemd (built from): git.launchpad.net/ubuntu/+source/systemd tag import/255.4-1ubuntu8.17 commit $(git -C "$U" rev-parse HEAD), debian/patches applied"
  echo "systemd (compared): github.com/systemd/systemd-stable tag v255.4 commit $(git -C "$S" rev-parse HEAD)"
  echo "linux: github.com/torvalds/linux tag v6.17 kernel/cgroup/cgroup.c sha256 $(sha256sum "$K" | cut -d' ' -f1)"
  echo
  echo "== cited systemd files: applied-tree sha256, and whether Ubuntu's patches change them"
  for f in src/core/scope.c src/core/cgroup.c src/core/dbus-scope.c src/basic/pidref.c src/basic/process-util.c; do
    printf '%s  %s  %s\n' "$(sha256sum "$A/$f" | cut -d' ' -f1)" "$f" \
      "$(cmp -s "$A/$f" "$S/$f" && echo 'identical to upstream v255.4' || echo 'patched by Ubuntu: excerpts compared below')"
  done
  ex "$A" src/core/dbus-scope.c '        if (streq(name, "PIDs")) {' 40 "StartTransientUnit's PIDs: each resolved to a pidref (pidfd) when the request is handled"
  ex "$A" src/basic/process-util.c 'int pidfd_get_pid(int fd, pid_t *ret) {' 14 "pidfd_get_pid: -ESRCH only once the process is reaped"
  ex "$A" src/basic/pidref.c 'int pidref_verify(const PidRef *pidref) {' 22 "pidref_verify: an exited, unreaped process still verifies"
  ex "$A" src/core/scope.c 'static int scope_enter_running(Scope *s) {' 45 "scope_enter_running: the PIDs attached once; refused only if none was attached"
  ex "$A" src/core/cgroup.c 'int unit_attach_pids_to_cgroup(Unit *u, Set *pids, const char *suffix_path) {' 80 "unit_attach_pids_to_cgroup: a successful cgroup.procs write counts as attached"
  ex "$A" src/core/cgroup.c 'int unit_watch_cgroup(Unit *u) {' 50 "unit_watch_cgroup: emptiness only from IN_MODIFY on cgroup.events"
  ex "$A" src/core/cgroup.c 'int unit_synthesize_cgroup_empty_event(Unit *u) {' 24 "no synthesized empty event on the unified hierarchy"
  ex "$A" src/core/cgroup.c 'static int unit_check_cgroup_events(Unit *u) {' 40 "unit_check_cgroup_events: read on a cgroup.events notification"
  ex "$A" src/core/scope.c 'static void scope_notify_cgroup_empty_event(Unit *u) {' 9 "a running scope ends for emptiness only on that notification"
  ex "$A" src/core/scope.c 'static int scope_dispatch_timer(sd_event_source *source, usec_t usec, void *userdata) {' 15 "the runtime backstop (RuntimeMaxUSec): stopping, failed 'timeout'"
  echo; echo "== Ubuntu's patches to the cited files, against upstream v255.4 (only files the table marks patched); the attach logic (pidref_verify, then cg_attach, a successful write counted) is the same in both"
  for f in src/core/scope.c src/core/cgroup.c src/core/dbus-scope.c src/basic/pidref.c src/basic/process-util.c; do
    cmp -s "$A/$f" "$S/$f" || {
      echo "-- $f: $(diff -u "$S/$f" "$A/$f" | grep -c '^[-+][^-+]') changed lines; the cited functions:"
      for fn in 'int unit_attach_pids_to_cgroup(' 'int unit_watch_cgroup(' 'int unit_synthesize_cgroup_empty_event(' 'static int unit_check_cgroup_events('; do
        grep -q -F "$fn" "$A/$f" || continue
        a=$(awk -v f="$fn" 'index($0,f)==1{p=1} p{print} p&&/^}/{exit}' "$A/$f" | sha256sum | cut -d' ' -f1)
        s=$(awk -v f="$fn" 'index($0,f)==1{p=1} p{print} p&&/^}/{exit}' "$S/$f" | sha256sum | cut -d' ' -f1)
        if [ "$a" = "$s" ]; then
          echo "   $fn ... identical"
        else
          echo "   $fn ... differs; the function's diff (upstream -> applied), then the patches touching it:"
          diff <(awk -v f="$fn" 'index($0,f)==1{p=1} p{print} p&&/^}/{exit}' "$S/$f") \
               <(awk -v f="$fn" 'index($0,f)==1{p=1} p{print} p&&/^}/{exit}' "$A/$f") | sed 's/^/      /' || true
          grep -l -F "${fn%%(*}" "$U"/debian/patches/* 2>/dev/null | sed 's|.*/|      patch: |' || true
        fi
      done
    }
  done
  ex "$(dirname "$K")" "$(basename "$K")" 'struct task_struct *cgroup_procs_write_start(char *buf, bool threadgroup,' 45 "Linux: a cgroup.procs write finds the task; -ESRCH only when none (a reaped one)"
  ex "$(dirname "$K")" "$(basename "$K")" 'static void cgroup_migrate_add_task(struct task_struct *task,' 28 "Linux: an exiting task (PF_EXITING) is skipped, silently"
  ex "$(dirname "$K")" "$(basename "$K")" 'static int cgroup_migrate_execute(struct cgroup_mgctx *mgctx)' 70 "Linux: a migration of no task succeeds"
  ex "$(dirname "$K")" "$(basename "$K")" 'void cgroup_exit(struct task_struct *tsk)' 16 "Linux: an exiting task leaves its css_set's tasks (never counted as populated)"
  ex "$(dirname "$K")" "$(basename "$K")" 'static void css_set_move_task(struct task_struct *task,' 40 "Linux: populated is updated as tasks leave"
  ex "$(dirname "$K")" "$(basename "$K")" 'static void cgroup_update_populated(struct cgroup *cgrp, bool populated)' 40 "Linux: cgroup.events is notified only when populated changes"
  ex "$(dirname "$K")" "$(basename "$K")" 'int proc_cgroup_show(struct seq_file *m, struct pid_namespace *ns,' 75 "Linux: /proc/<pid>/cgroup of an exiting task names its (original) cgroup on cgroup v2"
} > "$out"
wc -l "$out"

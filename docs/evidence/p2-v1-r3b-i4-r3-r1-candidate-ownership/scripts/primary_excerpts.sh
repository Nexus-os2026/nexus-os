#!/usr/bin/env bash
# P2-V1-R3B-I4-R3-R1: the primary-source excerpts the unit-instance analysis
# (matrices/unit-instance-identity.md) cites, with each source's identity.
# Sources: Ubuntu's exact systemd package (git-ubuntu import tag
# import/255.4-1ubuntu8.17, patches unapplied), with its debian/patches
# series applied in order (the tree the host's systemd was built from), and
# upstream systemd-stable v255.4 for comparison; the D-Bus specification and
# dbus-daemon's dispatch source at dbus-1.14.10 (the host's dbus-daemon).
# Usage: primary_excerpts.sh <primary sources directory> <output file>
set -eu
P="$1"; out="$2"
U="$P/ubuntu-systemd-255.4-1ubuntu8.17"
A="$P/ubuntu-applied-255.4-1ubuntu8.17"
S="$P/upstream-systemd-stable-v255.4"
{
  echo "P2-V1-R3B-I4-R3-R1 primary-source excerpts, $(date -u +%FT%TZ)"
  echo "host: $(systemctl --version | head -1); dbus-daemon $(dpkg-query -W -f='${Version}' dbus-daemon)"
  echo "ubuntu source: git.launchpad.net/ubuntu/+source/systemd tag import/255.4-1ubuntu8.17 commit $(git -C "$U" rev-parse HEAD)"
  echo "  debian/patches/series sha256 $(sha256sum "$U/debian/patches/series" | cut -d' ' -f1); $(grep -vc '^#' "$U/debian/patches/series") patches applied in order, none failed"
  echo "upstream: github.com/systemd/systemd-stable tag v255.4 commit $(git -C "$S" rev-parse HEAD)"
  echo
  echo "== cited files: applied-tree sha256, and whether Ubuntu's patches change them against upstream v255.4"
  for f in src/core/scope.c src/core/unit.c src/core/job.c src/core/dbus-job.c src/core/dbus-unit.c \
           src/core/dbus-manager.c src/core/dbus.c src/core/cgroup.c src/core/manager.c src/core/unit-serialize.c \
           src/libsystemd/sd-id128/sd-id128.c src/libsystemd/sd-bus/sd-bus.c src/libsystemd/sd-bus/bus-objects.c \
           src/shared/bus-get-properties.c src/basic/cgroup-util.c src/run/run.c \
           src/basic/unit-def.c src/basic/bus-label.c src/libsystemd/sd-bus/bus-common-errors.h; do
    printf '%s  %s  %s\n' "$(sha256sum "$A/$f" | cut -d' ' -f1)" "$f" \
      "$(cmp -s "$A/$f" "$S/$f" && echo 'identical to upstream v255.4' || echo 'patched by Ubuntu (see the diff summary below)')"
  done
  ex() { # file first last label
    echo; echo "== $4: $1 lines $2-$3"
    sed -n "$2,$3p" "$A/$1" | nl -ba -v "$2" -w5 -s'  '
  }
  ex src/core/scope.c 415 444 "scope start: the invocation ID is acquired, then the PIDs attached"
  ex src/core/scope.c 460 487 "scope_start"
  ex src/core/unit.c 3465 3500 "unit_set_invocation_id"
  ex src/core/unit.c 5377 5391 "unit_acquire_invocation_id: a fresh random ID on every start"
  ex src/libsystemd/sd-id128/sd-id128.c 326 339 "sd_id128_randomize"
  ex src/core/unit-serialize.c 478 490 "deserialization restores the same ID (daemon-reload, reexec)"
  ex src/core/dbus-unit.c 940 940 "the InvocationID property: ay, EMITS_CHANGE"
  ex src/shared/bus-get-properties.c 57 72 "the property's encoding: an empty array while null, else 16 bytes"
  ex src/core/dbus-unit.c 1557 1558 "the ControlGroup and ControlGroupId properties (the cgroup vtable)"
  ex src/core/dbus.c 512 519 "that vtable is on the unit type's interface: org.freedesktop.systemd1.Scope"
  ex src/core/cgroup.c 2495 2510 "ControlGroupId: the kernel cgroup ID of the unit's directory, read by file handle at realization"
  ex src/basic/cgroup-util.c 1441 1455 "cg_path_get_cgroupid: name_to_handle_at"
  ex src/core/cgroup.c 1043 1057 "a user manager writes no invocation-ID xattr on its cgroups"
  ex src/core/dbus-unit.c 2490 2497 "an unknown property (InvocationID) cannot be set by a caller"
  ex src/core/dbus-manager.c 997 1055 "StartTransientUnit: the unit must be pristine, and is made transient by this request"
  ex src/core/unit.c 5153 5170 "unit_is_pristine: no job, no fragment"
  ex src/core/dbus-manager.c 1097 1134 "method_start_transient_unit"
  ex src/core/dbus-unit.c 1825 1841 "the reply: the job path, after the requester is tracked and JobNew is forced out"
  ex src/core/dbus-unit.c 1915 1919 "the reply is sent before the handler returns"
  ex src/core/manager.c 736 742 "jobs run from a defer event source at idle priority"
  ex src/core/manager.c 2421 2441 "manager_dispatch_run_queue"
  ex src/core/unit.c 433 464 "unit_may_gc: never while a job is installed or change signals are pending"
  ex src/core/unit.c 2636 2670 "a start job finishes when the unit becomes active"
  ex src/core/job.c "$(grep -n '^void job_uninstall' "$A/src/core/job.c" | cut -d: -f1)" "$(( $(grep -n '^void job_uninstall' "$A/src/core/job.c" | cut -d: -f1) + 23 ))" "job_uninstall: JobRemoved is sent before the job is detached from the unit"
  ex src/core/dbus-job.c 271 311 "JobRemoved: the unit's pending change signal is sent first, to the requester's tracking"
  ex src/core/dbus-unit.c 1621 1669 "the unit change signal: the type's interface, then org.freedesktop.systemd1.Unit"
  ex src/core/dbus-unit.c 1671 1693 "bus_unit_send_pending_change_signal"
  ex src/core/dbus-unit.c 1593 1620 "a unit never announced gets UnitNew (no properties) instead of PropertiesChanged"
  ex src/core/manager.c 3268 3290 "the main loop: the D-Bus queue (UnitNew of a new unit) is dispatched before the next event, the run queue's"
  ex src/core/manager.c 2453 2475 "manager_dispatch_dbus_queue: skipped while more than MANAGER_BUS_BUSY_THRESHOLD messages are queued"
  ex src/core/manager.c 113 116 "MANAGER_BUS_BUSY_THRESHOLD, MANAGER_BUS_MESSAGE_BUDGET"
  ex src/core/dbus.c 1118 1149 "bus_foreach_bus: the API bus only for subscribed or tracking clients"
  ex src/core/dbus-manager.c 1367 1398 "Subscribe"
  ex src/libsystemd/sd-bus/bus-common-errors.h 14 15 "NotSubscribed, AlreadySubscribed"
  ex src/basic/unit-def.c 9 19 "unit_dbus_path_from_name: the unit's object path"
  ex src/basic/bus-label.c 10 40 "bus_label_escape: every byte but a letter, or a digit after the first, as _ and two hex digits"
  ex src/libsystemd/sd-bus/bus-objects.c "$(( $(grep -n 'static int emit_properties_changed_on_interface' "$A/src/libsystemd/sd-bus/bus-objects.c" | cut -d: -f1) + 100 ))" "$(( $(grep -n 'static int emit_properties_changed_on_interface' "$A/src/libsystemd/sd-bus/bus-objects.c" | cut -d: -f1) + 128 ))" "PropertiesChanged with no names: every EMITS_CHANGE property, by value"
  ex src/libsystemd/sd-bus/sd-bus.c 1901 1940 "message serials (cookies): incremented in send order, cycled flag past 2^32"
  ex src/run/run.c 1242 1283 "systemd-run reads InvocationID by unit name after the job: an unbound later read"
  echo; echo "== Ubuntu's patches to the cited core files (only these change; none concerns invocation IDs, jobs, transient units or scope start):"
  for f in src/core/dbus-unit.c src/core/dbus-manager.c src/core/cgroup.c src/core/manager.c; do
    echo "-- $f: $(diff -u "$S/$f" "$A/$f" | grep -c '^[-+][^-+]') changed lines"
  done
  echo; echo "== D-Bus specification 1.14.10: the serial and SENDER header fields"
  sed -n 1539,1544p "$P/dbus-specification-1.14.10.xml"
  L=$(grep -n '<literal>SENDER</literal></entry>' "$P/dbus-specification-1.14.10.xml" | head -1 | cut -d: -f1)
  sed -n "$L,$((L+11))p" "$P/dbus-specification-1.14.10.xml"
  echo; echo "== dbus-daemon 1.14.10 bus/dispatch.c: the bus assigns the sender, nothing else"
  sed -n 350,366p "$P/dbus-1.14.10-bus-dispatch.c" | nl -ba -v 350 -w5 -s'  '
  echo "sha256 dbus-specification-1.14.10.xml $(sha256sum "$P/dbus-specification-1.14.10.xml" | cut -d' ' -f1)"
  echo "sha256 dbus-1.14.10-bus-dispatch.c $(sha256sum "$P/dbus-1.14.10-bus-dispatch.c" | cut -d' ' -f1)"
} > "$out"
wc -l "$out"

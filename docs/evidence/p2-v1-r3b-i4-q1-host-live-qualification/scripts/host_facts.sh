#!/usr/bin/env bash
# P2-V1-R3B-I4-Q1 host facts, read-only, as evidence of the machine the
# qualification was prepared on (the implementer's user, not the runner's
# job): versions, the unified hierarchy, which users have a runtime
# directory, and the runner's service user. No systemctl, loginctl, busctl or
# any state change. Never authority.
set -u
echo "taken at: $(date -u +%Y-%m-%dT%H:%M:%SZ) by $(id -un) ($(id -u))"
echo "kernel: $(uname -sr) $(uname -m)"
echo "os: $(. /etc/os-release && echo "$PRETTY_NAME")"
echo "systemd: $(/usr/lib/systemd/systemd --version 2>/dev/null | head -1)"
echo "dbus: $(dbus-daemon --version 2>/dev/null | head -1)"
echo "packages: $(dpkg-query -W -f='${Package} ${Version}\n' systemd dbus 2>/dev/null | tr '\n' ';')"
echo "/sys/fs/cgroup type: $(stat -f -c %T /sys/fs/cgroup)"
echo "this process's cgroup: $(cat /proc/self/cgroup)"
echo "runtime directories: $(ls -1 /run/user 2>/dev/null | tr '\n' ' ')"
echo "lingering users (/var/lib/systemd/linger): $(ls -1 /var/lib/systemd/linger 2>/dev/null | tr '\n' ' ')(none if empty)"
echo "runner service user: $(grep -h '^User=' /etc/systemd/system/actions.runner*.service 2>/dev/null | head -1)"
echo "runner user entry: $(getent passwd github-runner)"
echo "runner process: $(ps -eo user:16,cmd | grep '[R]unner.Listener' | head -1)"

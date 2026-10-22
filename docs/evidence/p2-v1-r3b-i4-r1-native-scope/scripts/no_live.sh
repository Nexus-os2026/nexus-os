#!/usr/bin/env bash
# P2-V1-R3B-I4-R1 no-live-systemd proof (I4's script, unchanged but for this label): the sandbox crate's unit-test binary
# (already built by the validation, in the given target directory) run once
# under `strace -f`, tracing connect, execve and the open calls of the test
# process and every process it starts. The verdict holds only if:
#   - no connect() reaches any socket (no bus, no user manager);
#   - no systemctl, systemd-run, busctl or loginctl is executed;
#   - every access beneath /sys/fs/cgroup is a read-only open of `cpu.max`,
#     and every /proc cgroup file read is /proc/self/cgroup (the standard
#     library's available_parallelism in the test harness); no scope
#     cgroup, cgroup.procs, cgroup.kill or limit file is ever opened.
# The complete trace is kept beside the verdict.
# Usage: no_live.sh <checkout> <target directory> <output directory>
set -u
checkout="$1"; target="$2"; out="$3"
mkdir -p "$out"
cd "$checkout" || exit 2
binary=$(CARGO_TARGET_DIR="$target" cargo test --locked -p nexus-verifier-sandbox --lib --no-run \
  --message-format=json 2>/dev/null | python3 -c '
import json, sys
for line in sys.stdin:
    message = json.loads(line)
    if message.get("reason") == "compiler-artifact" and message.get("executable") \
            and message["target"]["name"] == "nexus_verifier_sandbox" and message["profile"]["test"]:
        print(message["executable"])
')
[ -x "$binary" ] || { echo "no unit-test binary" >&2; exit 2; }
trace="$out/strace.txt"
strace -f -qq -e signal=none -e trace=connect,execve,openat,open,openat2 -o "$trace" \
  "$binary" > "$out/tests.txt" 2>&1
tests_status=$?
python3 - "$trace" "$out/tests.txt" "$tests_status" "$binary" > "$out/verdict.txt" <<'EOF'
import collections
import re
import sys

trace, tests, status, binary = sys.argv[1], sys.argv[2], int(sys.argv[3]), sys.argv[4]
lines = open(trace).read().splitlines()
connects = [l for l in lines if " connect(" in l or l.split(" ", 1)[-1].startswith("connect(")]
execs = collections.Counter(re.sub(r"nexus-i4-helper-\d+-\d+", "nexus-i4-helper-<pid>-<n>", m)
                            for l in lines for m in re.findall(r'execve\("([^"]*)"', l))
cgroup = [l for l in lines if "/sys/fs/cgroup" in l]
proc_cgroup = [l for l in lines if re.search(r'"/proc/[^"]*/cgroup"', l)]
tools = [l for l in lines if re.search(r"execve\(\"[^\"]*(systemctl|systemd-run|busctl|loginctl)", l)]
bad_cgroup = [l for l in cgroup if not (re.search(r'"/sys/fs/cgroup(/[^"]*)?/cpu\.max", O_RDONLY\|O_CLOEXEC', l))]
bad_proc = [l for l in proc_cgroup if '"/proc/self/cgroup"' not in l]
result = open(tests).read()
summary = re.findall(r"^test result: .*$", result, re.M)
ok = status == 0 and not connects and not tools and not bad_cgroup and not bad_proc
print(f"binary: {binary}")
print(f"unit tests under strace: exit {status}; {summary[-1] if summary else 'no result line'}")
print(f"trace lines: {len(lines)}")
print(f"connect() calls: {len(connects)}")
for l in connects:
    print(f"    {l}")
print("programs executed:")
for program, n in sorted(execs.items()):
    print(f"    {n:4} {program}")
print(f"systemd tools executed: {len(tools)}")
print(f"accesses beneath /sys/fs/cgroup: {len(cgroup)} (other than a read-only cpu.max: {len(bad_cgroup)})")
for l in bad_cgroup:
    print(f"    {l}")
print(f"/proc cgroup reads: {len(proc_cgroup)} (other than /proc/self/cgroup: {len(bad_proc)})")
for l in bad_proc:
    print(f"    {l}")
print("RESULT:", "PASS" if ok else "FAIL")
sys.exit(0 if ok else 1)
EOF
verdict=$?
cat "$out/verdict.txt"
exit $verdict

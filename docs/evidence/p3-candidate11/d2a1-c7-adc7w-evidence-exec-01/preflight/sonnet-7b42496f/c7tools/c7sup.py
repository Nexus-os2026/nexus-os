#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-C7-W-EVIDENCE-EXEC-01 supervised launcher. One call = one supervised process = one receipt directory.

  c7sup.py selftest <dir>                                           enforcement self-test with NON-Nexus processes only
  c7sup.py build  <dir> <worktree> <E-W|E-W7>                        the ONE test build of that identity (exact mission command)
  c7sup.py invoke <dir> <worktree> <label> <E-W|E-W7> <exe> <sha256> <limit-seconds> -- <args...>
                                                                     one invocation of a pinned executable, cwd = <worktree>/crates/nexus-governed-control

Enforcement (the existing local mechanism of the earlier missions): a transient systemd user scope with MemoryMax=16G
(17179869184) and MemorySwapMax=0 (cgroup v2: charged memory), the scope's RuntimeMaxSec (limit + 10 s) as a backstop, and
`timeout -s KILL <limit>` as the time limit. scope-wrap.sh, the scope's first process, records the scope's enforced
memory.max / memory.swap.max before the command and memory.peak / memory.swap.peak / memory.events after it. /usr/bin/time -v
records the exit status or terminating signal. A build has at most 2 compiler jobs (cargo -j 2) and 900 s.
Before starting: MemAvailable >= 16 GiB + 2 GiB margin, or NOT STARTED. No other workload is touched. Nothing is retried.
The receipt directory must not exist.
"""
import hashlib
import json
import os
import re
import shlex
import subprocess
import sys
import threading
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import c7wt
from c7lib import *  # noqa

HERE = os.path.dirname(os.path.abspath(__file__))
WRAP = os.path.join(HERE, "scope-wrap.sh")
UNIT = "p3c11-c7w01"
NEED_KIB = (16 + 2) * 1024 * 1024
BUILD_LIMIT = 900
BUILD_ARGV = ["cargo", "test", "-p", "nexus-governed-control", "--locked", "--release", "--lib", "--no-run", "-j", "2"]
BUILD_ENV_FIXED = [("HOME", "/home/nexus"), ("PATH", "/home/nexus/.cargo/bin:/usr/local/bin:/usr/bin:/bin"), ("CARGO_NET_OFFLINE", "true")]
RUN_ENV = [("PATH", "/usr/bin:/bin"), ("LANG", "C.UTF-8"), ("RUST_BACKTRACE", "0")]
CRATE_DIR = "crates/nexus-governed-control"


def meminfo(key):
    for line in open("/proc/meminfo"):
        if line.startswith(key + ":"):
            return int(line.split()[1])


def preflight(rec):
    avail = meminfo("MemAvailable")
    rec.append("pre-flight %s:" % time.strftime("%Y-%m-%dT%H:%M:%S%z"))
    rec.append("  MemTotal %d KiB; MemAvailable %d KiB (needed %d KiB = 16 GiB cap + 2 GiB margin)" % (meminfo("MemTotal"), avail, NEED_KIB))
    rec.append("  SwapTotal %d KiB; SwapFree %d KiB (the scope has swap disabled)" % (meminfo("SwapTotal"), meminfo("SwapFree")))
    rec.append("  load average %s; online CPUs %d" % (open("/proc/loadavg").read().split()[:3], os.cpu_count()))
    top = subprocess.run(["ps", "-eo", "rss=,comm=", "--sort=-rss"], capture_output=True, text=True).stdout.splitlines()[:8]
    rec.append("  largest resident processes (comm only; none is stopped by this launcher):")
    rec += ["    " + l.strip() for l in top]
    st = os.statvfs(os.path.dirname(WT_HEALTHY))
    rec.append("  free disk %.1f GiB" % (st.f_bavail * st.f_frsize / 2**30))
    return avail >= NEED_KIB


def sampler(unit, path, stop):
    with open(path, "w") as out:
        cg = None
        for _ in range(200):
            r = subprocess.run(["systemctl", "--user", "show", "-p", "ControlGroup", "--value", unit + ".scope"],
                               capture_output=True, text=True)
            cand = "/sys/fs/cgroup" + r.stdout.strip()
            if r.stdout.strip() and os.path.isdir(cand):
                cg = cand
                break
            if stop.is_set():
                break
            time.sleep(0.05)
        if not cg:
            out.write("sampler: the scope's cgroup was not found while it ran (a short run; scope-wrap.sh records it)\n")
            return
        out.write("scope cgroup: %s\ntime memory.current memory.peak memory.swap.current oom_kill host.MemAvailable(KiB)\n" % cg)
        while os.path.isdir(cg) and not stop.is_set():
            try:
                cur = open(cg + "/memory.current").read().strip()
                peak = open(cg + "/memory.peak").read().strip()
                swp = open(cg + "/memory.swap.current").read().strip()
                ev = dict(l.split() for l in open(cg + "/memory.events"))
            except OSError:
                break
            out.write("%s %s %s %s %s %s\n" % (time.strftime("%H:%M:%S"), cur, peak, swp, ev.get("oom_kill"), meminfo("MemAvailable")))
            out.flush()
            time.sleep(2)
        out.write("sampler: stopped at %s\n" % time.strftime("%H:%M:%S"))


def parse_time_v(path):
    text = open(path).read() if os.path.exists(path) else ""
    sig = re.search(r"Command terminated by signal (\d+)", text)
    ex = re.search(r"Exit status: (\d+)", text)
    rss = re.search(r"Maximum resident set size \(kbytes\): (\d+)", text)
    wall = re.search(r"Elapsed \(wall clock\) time \(h:mm:ss or m:ss\): (\S+)", text)
    secs = None
    if wall:
        parts = [float(p) for p in wall.group(1).split(":")]
        secs = sum(p * 60 ** (len(parts) - 1 - i) for i, p in enumerate(parts))
    # GNU time also prints "Exit status: 0" for a process ended by a signal; such a process has no exit status.
    return {"signal": int(sig.group(1)) if sig else None, "exit_status": int(ex.group(1)) if ex and not sig else None,
            "max_rss_kib": int(rss.group(1)) if rss else None, "wall_seconds": secs, "text": text}


def parse_scope_record(path):
    d = {}
    if os.path.exists(path):
        for line in open(path):
            k, _, v = line.rstrip("\n").partition(": ")
            if k == "memory.events":
                a, b = v.split()
                d["events." + a] = int(b)
            elif k == "memory.swap.events":
                a, b = v.split()
                d["swap.events." + a] = int(b)
            else:
                d[k] = v
    return d


def supervised(limit, outdir, unit, cwd, inner, env_inner, stdout_path, stderr_path, rec):
    scope_rec = os.path.join(outdir, "scope-record.txt")
    time_v = os.path.join(outdir, "time-v.txt")
    cmd = ["systemd-run", "--user", "--scope", "--unit=%s" % unit, "-p", "MemoryMax=16G", "-p", "MemorySwapMax=0",
           "-p", "RuntimeMaxSec=%d" % (limit + 10), "--quiet", WRAP, scope_rec,
           "/usr/bin/env", "-i", *["%s=%s" % kv for kv in env_inner],
           "/usr/bin/time", "-v", "-o", time_v, "timeout", "-s", "KILL", str(limit), *inner]
    rec.append("scope unit: %s.scope; cwd: %s" % (unit, cwd))
    rec.append("command: " + " ".join(shlex.quote(c) for c in cmd))
    stop = threading.Event()
    th = threading.Thread(target=sampler, args=(unit, os.path.join(outdir, "cgroup-samples.txt"), stop), daemon=True)
    th.start()
    t0 = time.time()
    with open(stdout_path, "wb") as so:
        se = open(stderr_path, "wb") if stderr_path else subprocess.STDOUT
        r = subprocess.run(cmd, cwd=cwd, stdout=so, stderr=se)
        if stderr_path:
            se.close()
    elapsed = time.time() - t0
    stop.set()
    th.join(timeout=10)
    show = subprocess.run(["systemctl", "--user", "show", unit + ".scope", "-p", "LoadState", "-p", "ActiveState", "-p", "Result"],
                          capture_output=True, text=True).stdout.split()
    show = dict(s.split("=", 1) for s in show if "=" in s)
    tv, sr = parse_time_v(time_v), parse_scope_record(scope_rec)
    scope_result = "gone" if show.get("LoadState") == "not-found" else show.get("Result")
    timed_out = tv["exit_status"] in (124, 137) or (tv["wall_seconds"] or 0) >= limit
    sup = {"memory_max": sr.get("memory.max"), "swap_max": sr.get("memory.swap.max"), "memory_high": sr.get("memory.high"),
           "memory_peak_bytes": sr.get("memory.peak"), "swap_peak_bytes": sr.get("memory.swap.peak"),
           "oom": sr.get("events.oom"), "oom_kill": sr.get("events.oom_kill"),
           "swap_events": {k: v for k, v in sr.items() if k.startswith("swap.events.")},
           "completed": all(k in sr for k in ("finished", "command exit status", "events.oom_kill")), "timed_out": timed_out,
           "wall_seconds": tv["wall_seconds"], "time_limit_seconds": limit, "scope_result": scope_result, "scope_show": show,
           "launcher_elapsed_seconds": round(elapsed, 3), "max_rss_kib_largest_process": tv["max_rss_kib"]}
    proc = {"exit_status": tv["exit_status"], "signal": tv["signal"], "supervisor_rc": r.returncode,
            "wrapper_recorded_status": sr.get("command exit status")}
    rec.append("supervisor return code %s; /usr/bin/time: exit status %s, signal %s, wall %s s, max RSS (largest single process) %s KiB"
               % (r.returncode, tv["exit_status"], tv["signal"], tv["wall_seconds"], tv["max_rss_kib"]))
    rec.append("scope (scope-wrap.sh record): memory.max %s, memory.swap.max %s, memory.peak %s B, memory.swap.peak %s B, oom %s, "
               "oom_kill %s, completed %s; systemctl after: %s" % (sup["memory_max"], sup["swap_max"], sup["memory_peak_bytes"],
                                                                    sup["swap_peak_bytes"], sup["oom"], sup["oom_kill"], sup["completed"], show))
    return sup, proc


def selftest(outdir):
    os.makedirs(outdir)
    rec = ["SUPERVISION SELF-TEST %s (non-Nexus processes only)" % time.strftime("%Y-%m-%dT%H:%M:%S%z"),
           "systemd: %s" % subprocess.run(["systemctl", "--user", "--version"], capture_output=True, text=True).stdout.splitlines()[0],
           "kernel: %s; timeout: %s" % (os.uname().release, subprocess.run(["timeout", "--version"], capture_output=True, text=True).stdout.splitlines()[0])]
    ok = True
    py = "/usr/bin/python3"
    ts = int(time.time())
    # 1. limits read back from inside the scope
    d = os.path.join(outdir, "1")
    os.makedirs(d)
    r = ["1. the 16 GiB, zero-swap scope's limits read back from inside it (a 10 MiB allocation)"]
    sup, proc = supervised(60, d, "%s-selftest-1-%d" % (UNIT, ts), outdir, [py, "-c", "b = b'x' * (10 << 20); print('allocated', len(b))"],
                           [("PATH", "/usr/bin:/bin")], os.path.join(d, "out.txt"), None, r)
    good = (sup["memory_max"] == "17179869184" and sup["swap_max"] == "0" and proc["exit_status"] == 0 and sup["completed"]
            and sup["oom_kill"] == 0 and sup["swap_peak_bytes"] == "0")
    ok &= good
    rec += r + ["  RESULT %s" % ("OK" if good else "FAILED"), ""]
    # 2. memory cap enforced
    d = os.path.join(outdir, "2")
    os.makedirs(d)
    unit = "%s-selftest-mem-%d" % (UNIT, ts)
    r = subprocess.run(["systemd-run", "--user", "--scope", "--unit=%s" % unit, "-p", "MemoryMax=64M", "-p", "MemorySwapMax=0", "--quiet",
                        WRAP, os.path.join(d, "scope-record.txt"), py, "-c", "b = b'x' * (256 << 20); print('NOT KILLED: wrote', len(b))"],
                       capture_output=True, text=True)
    show = subprocess.run(["systemctl", "--user", "show", unit + ".scope", "-p", "Result", "-p", "ActiveState"], capture_output=True, text=True).stdout.split()
    sr = parse_scope_record(os.path.join(d, "scope-record.txt"))
    complete = all(k in sr for k in ("finished", "command exit status", "events.oom_kill"))
    good = (r.returncode != 0 and "NOT KILLED" not in r.stdout and "Result=oom-kill" in show
            and (not complete or sr.get("command exit status") == "137") and sr.get("memory.max") == "67108864" and sr.get("memory.swap.max") == "0")
    ok &= good
    rec += ["2. memory cap enforced: a 64 MiB zero-swap scope, the process writes 256 MiB",
            "  supervisor rc %s; stdout %r; scope after: %s; wrapper record keys %s; complete record: %s; command status recorded: %s"
            % (r.returncode, r.stdout.strip(), show, sorted(sr), complete, sr.get("command exit status")), "  RESULT %s" % ("OK" if good else "FAILED"), ""]
    subprocess.run(["systemctl", "--user", "reset-failed", unit + ".scope"], capture_output=True)
    # 3. time limit
    t0 = time.time()
    r = subprocess.run(["systemd-run", "--user", "--scope", "--unit=%s-selftest-time-%d" % (UNIT, ts), "--quiet", "timeout", "-s", "KILL", "2", "sleep", "30"],
                       capture_output=True, text=True)
    el = time.time() - t0
    good = r.returncode in (-9, 137) and el < 10
    ok &= good
    rec += ["3. time limit enforced: timeout -s KILL 2 around a 30 s sleep", "  rc %s; elapsed %.2f s" % (r.returncode, el), "  RESULT %s" % ("OK" if good else "FAILED"), ""]
    # 4. backstop
    t0 = time.time()
    unit = "%s-selftest-backstop-%d" % (UNIT, ts)
    r = subprocess.run(["systemd-run", "--user", "--scope", "--unit=%s" % unit, "-p", "RuntimeMaxSec=2", "--quiet", "sleep", "30"], capture_output=True, text=True)
    el = time.time() - t0
    good = r.returncode != 0 and el < 10
    ok &= good
    rec += ["4. backstop enforced: RuntimeMaxSec=2 around a 30 s sleep", "  rc %s; elapsed %.2f s" % (r.returncode, el), "  RESULT %s" % ("OK" if good else "FAILED"), ""]
    subprocess.run(["systemctl", "--user", "reset-failed", unit + ".scope"], capture_output=True)
    # 5/6. exit 101 and a signal through the whole run chain
    for label, code, want in (("5. a normal exit 101 through the run chain", "import sys; sys.exit(101)",
                               lambda s, p: p["exit_status"] == 101 and p["signal"] is None and s["completed"] and p["supervisor_rc"] == 101),
                              ("6. a process ended by SIGABRT through the run chain", "import os; os.abort()",
                               lambda s, p: p["signal"] == 6 and p["exit_status"] is None and p["supervisor_rc"] == 134)):
        d = os.path.join(outdir, label[0])
        os.makedirs(d)
        r = [label]
        sup, proc = supervised(60, d, "%s-selftest-%s-%d" % (UNIT, label[0], ts), outdir, [py, "-c", code], [("PATH", "/usr/bin:/bin")],
                               os.path.join(d, "out.txt"), None, r)
        good = want(sup, proc)
        ok &= good
        rec += r + ["  process %s" % proc, "  RESULT %s" % ("OK" if good else "FAILED"), ""]
    rec.append("SELF-TEST RESULT: %s" % ("ALL OK" if ok else "FAILED"))
    open(os.path.join(outdir, "SELFTEST.txt"), "x").write("\n".join(rec) + "\n")
    print("\n".join(rec))
    return 0 if ok else 1


def inventory(target):
    """Artifact inventory under <target>/release: every regular file (path, size, sha256) except nothing excluded; fingerprints parsed."""
    files = []
    root = os.path.join(target, "release")
    for dp, dn, fn in os.walk(root):
        dn.sort()
        for n in sorted(fn):
            p = os.path.join(dp, n)
            if os.path.islink(p) or not os.path.isfile(p):
                continue
            files.append({"path": os.path.relpath(p, target), "size": os.path.getsize(p), "sha256": sha256_file(p)})
    return files


def parse_executables(stderr_text):
    """Both cargo forms: non-verbose `  Executable unittests src/lib.rs (target/release/deps/x-hash)` and verbose `... `path``."""
    found = []
    for line in stderr_text.splitlines():
        m = re.match(r"^\s+Executable\s+(.*)$", line)
        if not m:
            continue
        rest = m.group(1)
        pm = re.search(r"\(([^()]+)\)\s*$", rest) or re.search(r"`([^`]+)`\s*$", rest)
        if pm:
            found.append(pm.group(1))
    return found


def build(outdir, wt, role, argv=None, verify_role=None, limit=BUILD_LIMIT, check_source=True, exe_prefix="nexus_governed_control"):
    os.makedirs(outdir)
    argv = list(argv or BUILD_ARGV)
    rec = ["SUPERVISED BUILD (%s) — %s" % (role, MISSION), "started: %s" % time.strftime("%Y-%m-%dT%H:%M:%S%z")]
    srole = verify_role or {"E-W": "W", "E-W7": "W7"}.get(role)
    before_chk = c7wt.verify(wt, srole) if check_source else []
    ok_before = all(c[1] for c in before_chk)
    rec += ["source verification before (%s):" % srole] + ["  %s %s %s" % ("PASS" if c[1] else "FAIL", c[0], c[2]) for c in before_chk]
    target = os.path.join(wt, "target")
    rec.append("target directory %s existed before: %s; entries before: %s" % (target, os.path.exists(target),
                                                                              sorted(os.listdir(target)) if os.path.isdir(target) else None))
    env_inner = BUILD_ENV_FIXED + [("CARGO_TARGET_DIR", target)]
    rec.append("build environment (env -i; only these): " + " ".join("%s=%s" % kv for kv in env_inner))
    rec.append("inherited variables NOT passed (names only; values never recorded): " + " ".join(sorted(k for k in os.environ if k not in dict(env_inner))))
    rec.append("build command: " + " ".join(argv))
    ver = {}
    for name, a in (("rustc -vV", ["rustc", "-vV"]), ("cargo -vV", ["cargo", "-vV"]), ("rustup show active-toolchain", ["rustup", "show", "active-toolchain"])):
        p = subprocess.run(["/usr/bin/env", "-i", *["%s=%s" % kv for kv in BUILD_ENV_FIXED], *a], cwd=wt, capture_output=True, text=True)
        ver[name] = p.stdout
        rec += ["  %s (rc %d):" % (name, p.returncode)] + ["    " + l for l in p.stdout.splitlines()]
    open(os.path.join(outdir, "toolchain-versions.json"), "x").write(json.dumps(ver, indent=1))
    if not ok_before:
        rec.append("OUTCOME: NOT STARTED (the source verification failed)")
        open(os.path.join(outdir, "RECEIPT.txt"), "x").write("\n".join(rec) + "\n")
        print("\n".join(rec))
        return 5
    if not preflight(rec):
        rec.append("OUTCOME: NOT STARTED (not enough free memory; no workload was stopped)")
        open(os.path.join(outdir, "RECEIPT.txt"), "x").write("\n".join(rec) + "\n")
        print("\n".join(rec))
        return 3
    sup, proc = supervised(limit, outdir, "%s-build-%s-%d" % (UNIT, role, int(time.time())), wt, argv, env_inner,
                           os.path.join(outdir, "build.stdout"), os.path.join(outdir, "build.stderr"), rec)
    stderr = open(os.path.join(outdir, "build.stderr"), errors="replace").read()
    stdout = open(os.path.join(outdir, "build.stdout"), errors="replace").read()
    lines = stderr.splitlines()
    warnings = [l for l in lines if re.match(r"^warning", l)]
    errors = [l for l in lines if re.match(r"^error", l)]
    compiling = [l.strip() for l in lines if re.match(r"^\s+Compiling ", l)]
    finished = [l.strip() for l in lines if re.match(r"^\s+Finished ", l)]
    exes = parse_executables(stderr)
    deps = os.path.join(target, "release/deps")
    cands = []
    if os.path.isdir(deps):
        for n in sorted(os.listdir(deps)):
            p = os.path.join(deps, n)
            if re.match(r"^%s-[0-9a-f]{16}$" % re.escape(exe_prefix), n) and os.path.isfile(p) and os.access(p, os.X_OK):
                cands.append(p)
    resolved = [os.path.normpath(os.path.join(wt, e)) for e in exes]
    pinned = []
    for p in resolved:
        if os.path.exists(p):
            pinned.append({"path": p, "sha256": sha256_file(p), "size": os.path.getsize(p)})
    inv = inventory(target) if os.path.isdir(target) else []
    after_chk = c7wt.verify(wt, srole) if check_source else []
    ok_after = all(c[1] for c in after_chk)
    rec.append("cargo stderr: %d lines; compiling lines %d; finished lines %s" % (len(lines), len(compiling), finished))
    rec.append("warnings (lines starting 'warning'): %d; errors (lines starting 'error'): %d" % (len(warnings), len(errors)))
    rec.append("Executable lines parsed: %s -> %s; executables found by the deps scan: %s" % (exes, resolved, cands))
    for p in pinned:
        rec.append("  executable %s sha256 %s size %d" % (p["path"], p["sha256"], p["size"]))
    rec.append("artifact inventory: %d regular files under target/release" % len(inv))
    rec += ["source verification after (%s):" % srole] + ["  %s %s %s" % ("PASS" if c[1] else "FAIL", c[0], c[2]) for c in after_chk]
    rec.append("build stdout (%d bytes): %r" % (len(stdout), stdout[:200]))
    desc = {"id": os.path.basename(outdir), "type": "build", "role": role, "mission": MISSION, "worktree": wt, "argv": argv, "jobs": 2,
            "env": dict(env_inner), "process": proc, "supervisor": sup, "stderr_path": "build.stderr", "stdout_path": "build.stdout",
            "build_stderr_sha256": sha256_file(os.path.join(outdir, "build.stderr")),
            "compiler_messages": {"warnings": len(warnings), "errors": len(errors), "warning_lines": warnings[:20], "error_lines": errors[:20]},
            "compiling": compiling, "finished": finished, "executable_lines": exes, "executables_found": cands,
            "pinned_executables": pinned, "inventory": inv,
            "source_verification_before": [{"id": a, "ok": b, "detail": c} for a, b, c in before_chk],
            "source_verification_after": [{"id": a, "ok": b, "detail": c} for a, b, c in after_chk],
            "toolchain": ver, "finished_at": time.strftime("%Y-%m-%dT%H:%M:%S%z")}
    json.dump(desc, open(os.path.join(outdir, "descriptor.json"), "x"), indent=1)
    good = (proc["exit_status"] == 0 and proc["signal"] is None and not sup["timed_out"] and len(pinned) == 1
            and cands == [p["path"] for p in pinned] and ok_after and sup["completed"] and sup["oom_kill"] == 0
            and sup["swap_peak_bytes"] == "0")
    rec.append("OUTCOME: %s" % ("BUILD COMPLETED; one executable; source unchanged (NI-7/NI-6 are judged by c7bwc from this receipt)" if good else "BUILD NOT CLEAN"))
    open(os.path.join(outdir, "RECEIPT.txt"), "x").write("\n".join(rec) + "\n")
    print("\n".join(rec[-14:]))
    return 0 if good else 4


def invoke(outdir, wt, label, exe, want_sha, limit, args, verify_role=None, cwd=None, env=None):
    os.makedirs(outdir)
    rec = ["SUPERVISED INVOCATION (%s) — %s" % (label, MISSION), "started: %s" % time.strftime("%Y-%m-%dT%H:%M:%S%z")]
    srole = verify_role
    before_chk = c7wt.verify(wt, srole) if srole else []
    rec += ["source verification before (%s):" % srole] + ["  %s %s %s" % ("PASS" if c[1] else "FAIL", c[0], c[2]) for c in before_chk]
    before = sha256_file(exe)
    rec.append("executable %s: sha256 before %s (%d bytes); pinned %s" % (exe, before, os.path.getsize(exe), want_sha))
    if before != want_sha or (srole and not all(c[1] for c in before_chk)):
        rec.append("OUTCOME: NOT STARTED (the executable is not the pinned one, or the source verification failed)")
        open(os.path.join(outdir, "RECEIPT.txt"), "x").write("\n".join(rec) + "\n")
        print("\n".join(rec))
        return 5
    if not preflight(rec):
        rec.append("OUTCOME: NOT STARTED (not enough free memory; no workload was stopped)")
        open(os.path.join(outdir, "RECEIPT.txt"), "x").write("\n".join(rec) + "\n")
        print("\n".join(rec))
        return 3
    argv = [exe, *args]
    env_inner = env or RUN_ENV
    rec.append("environment (env -i; only these): " + " ".join("%s=%s" % kv for kv in env_inner))
    cwd = cwd or os.path.join(wt, CRATE_DIR)
    raw = os.path.join(outdir, "raw-output.txt")
    sup, proc = supervised(limit, outdir, "%s-%s-%d" % (UNIT, re.sub(r"[^A-Za-z0-9_.-]", "-", label), int(time.time())), cwd, argv, env_inner, raw, None, rec)
    after = sha256_file(exe)
    after_chk = c7wt.verify(wt, srole) if srole else []
    rec.append("executable sha256 after: %s %s" % (after, "MATCH" if after == before else "MISMATCH"))
    rec += ["source verification after (%s):" % srole] + ["  %s %s %s" % ("PASS" if c[1] else "FAIL", c[0], c[2]) for c in after_chk]
    rec.append("raw output: %s sha256 %s bytes %d" % (raw, sha256_file(raw), os.path.getsize(raw)))
    desc = {"id": os.path.basename(outdir), "type": "invocation", "label": label, "mission": MISSION, "worktree": wt, "cwd": cwd,
            "argv": argv, "env": dict(env_inner), "limit_seconds": limit,
            "executable": {"path": exe, "sha256_before": before, "sha256_after": after, "size": os.path.getsize(exe)},
            "process": proc, "supervisor": sup, "output_path": "raw-output.txt", "raw_output_sha256": sha256_file(raw),
            "source_verification_before": [{"id": a, "ok": b, "detail": c} for a, b, c in before_chk],
            "source_verification_after": [{"id": a, "ok": b, "detail": c} for a, b, c in after_chk],
            "finished_at": time.strftime("%Y-%m-%dT%H:%M:%S%z")}
    json.dump(desc, open(os.path.join(outdir, "descriptor.json"), "x"), indent=1)
    rec.append("finished: %s" % time.strftime("%Y-%m-%dT%H:%M:%S%z"))
    open(os.path.join(outdir, "RECEIPT.txt"), "x").write("\n".join(rec) + "\n")
    print("\n".join(rec[-6:]))
    return 0


if __name__ == "__main__":
    mode = sys.argv[1]
    if os.path.exists(sys.argv[2]):
        raise SystemExit("REFUSED: %s exists (one call, one receipt; nothing is retried)" % sys.argv[2])
    if mode == "selftest":
        sys.exit(selftest(sys.argv[2]))
    if mode == "build":
        sys.exit(build(sys.argv[2], sys.argv[3], sys.argv[4]))
    if mode == "invoke":
        i = sys.argv.index("--")
        sys.exit(invoke(sys.argv[2], sys.argv[3], sys.argv[4], sys.argv[6], sys.argv[7], int(sys.argv[8]), sys.argv[i + 1:],
                        verify_role={"E-W": "W", "E-W7": "W7"}[sys.argv[5]]))
    raise SystemExit("usage")

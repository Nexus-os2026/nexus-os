#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-FINAL-EVIDENCE-STAGE2-01 supervised launcher. One call = one supervised process = one receipt
directory. Adapted from P3-C11-DR01-D2A1-FINAL-EVIDENCE-STAGE1-01 tools/supervise.py v3 (the same enforcement chain
and self-test); there is no build mode: this mission builds nothing.

  supervise.py selftest <dir>                       enforcement self-test with NON-Nexus processes only
  supervise.py witness <dir> c1|c25|c4              one standalone healthy witness (mission section 7)
  supervise.py full <dir> fg|fb2|f4|fw|f8|f5|f6|f7|fd|fr      one remaining configuration (mission sections 8, 9)

Enforcement: a transient systemd user scope with MemoryMax=16G and MemorySwapMax=0 (cgroup v2), RuntimeMaxSec (limit
+ 10 s) as a backstop, and `timeout -s KILL <limit>` as the wall-time limit (witness 180 s, full 2700 s). scope-wrap.sh,
the scope's first process, records the enforced memory.max and memory.swap.max before the command and memory.peak,
memory.swap.peak and memory.events after it. /usr/bin/time -v records the exit status or terminating signal. A
sampler records the scope's memory every 2 s. `nice -n 10` keeps the Owner's workloads ahead; no other process is
touched.

Fail-closed rules: the receipt directory must not exist; every frozen tool and the frozen vocabulary must equal the
freeze record; the worktree must be exactly C, byte for byte, before and after; the executable must be the pinned one
(path, sha256 and size) before, at selection and after; MemAvailable >= 18 GiB before starting; no governed-control
test executable, no cargo/rustc in this worktree and no scope of these missions may be running; the selection listing
must show exactly the one named test; an invocation starts only when the previous one in the frozen order is
classified PASS. Nothing is retried.
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
import common  # noqa: E402

EV, WT = common.EV, common.WT
CRATE_DIR = os.path.join(WT, "crates/nexus-governed-control")
WRAP = os.path.join(EV, "tools/scope-wrap.sh")
VERIFY = os.path.join(EV, "tools/verify_tree.py")
FREEZE = os.path.join(EV, "receipts/40-FREEZE.json")
NEED_KIB = 18 * 1024 * 1024
LIMITS = {"selftest": 60, "list": 60, "witness": 180, "full": 2700}
UNIT_PREFIX = "p3c11-fbr01"
RUN_ENV = [("PATH", "/usr/bin:/bin"), ("LANG", "C.UTF-8"), ("RUST_BACKTRACE", "0")]
P = common.TEST_PREFIX
STAGE1_NORMALIZED = os.path.join(EV, "normalized/stage1/SUMMARY.txt")
WITNESS = {  # key: (test, receipt directory, the step that must be classified PASS first)
    "c1": (P + "c1_witness_p1_at_a_successful_admission", "receipts/50-witness-C1", None),
    "c25": (P + "c25_witness_p7_on_the_resulting_world", "receipts/51-witness-C25", "receipts/50-witness-C1"),
    "c4": (P + "c4_witness_p4_and_p8_on_f8", "receipts/52-witness-C4", "receipts/51-witness-C25"),
}
ORDER = ["fg", "fb2", "f4", "fw", "f8", "f5", "f6", "f7", "fd", "fr"]
CFG = {"fg": "F-G", "fb2": "F-B2", "f4": "F-4", "fw": "F-W", "f8": "F-8", "f5": "F-5", "f6": "F-6", "f7": "F-7",
       "fd": "F-D", "fr": "F-R"}
FULL = {}
for n, k in enumerate(ORDER):
    prior = "receipts/52-witness-C4" if n == 0 else f"receipts/{60 + n - 1}-full-{CFG[ORDER[n - 1]]}"
    FULL[k] = (P + "full_" + k, f"receipts/{60 + n}-full-{CFG[k]}", prior)


def now():
    return time.strftime("%Y-%m-%dT%H:%M:%S%z")


def sha256(path):
    return common.sha256_file(path)


def meminfo(key):
    for line in open("/proc/meminfo"):
        if line.startswith(key + ":"):
            return int(line.split()[1])


def freeze_check(rec):
    """Every frozen file's sha256 equals the freeze record; returns a problem or None."""
    if not os.path.exists(FREEZE):
        return "no freeze record"
    fr = json.load(open(FREEZE))
    bad = []
    for rel, digest in sorted(fr["files"].items()):
        p = os.path.join(EV, rel)
        if not os.path.exists(p) or sha256(p) != digest:
            bad.append(rel)
    rec.append(f"freeze check: {len(fr['files'])} frozen files (tools, vocabulary, expectations) against "
               f"receipts/40-FREEZE.json: {'ALL EQUAL' if not bad else 'CHANGED: ' + str(bad)}")
    return ("frozen files changed: " + ", ".join(bad)) if bad else None


def exe_identity():
    st = os.stat(common.EXE)
    return dict(path=common.EXE, sha256=sha256(common.EXE), size=st.st_size, mtime_ns=st.st_mtime_ns,
                inode=st.st_ino, symlink=os.path.islink(common.EXE))


def exe_ok(i):
    return i["sha256"] == common.EXE_SHA256 and i["size"] == common.EXE_SIZE and not i["symlink"]


def tree_check(rec, when):
    r = subprocess.run([sys.executable, VERIFY, WT], capture_output=True, text=True)
    d = json.loads(r.stdout)
    rec.append(f"source freeze ({when}): {d['verdict']}; HEAD {d['head']}; tree {d['tree']}; detached {d['detached']}; "
               f"{d['tracked_blobs_compared']} tracked blobs, {d['bytes_compared']} bytes compared; status entries "
               f"{d['status_entries']}; mismatches {d['content_mismatches']}; missing {d['missing']}; "
               f"modes {d['mode_mismatches']}")
    return d


def conflicts():
    """Governed-control test executables anywhere (argv[0] is the executable; comm is truncated), cargo or rustc
    processes working in this mission's worktree, and active scopes of these missions."""
    exes, builds = [], []
    for pid in os.listdir("/proc"):
        if not pid.isdigit() or int(pid) == os.getpid():
            continue
        try:
            argv = open(f"/proc/{pid}/cmdline", "rb").read().split(b"\0")
        except OSError:
            continue
        a0 = argv[0].decode(errors="replace") if argv else ""
        if re.search(r"/deps/nexus_governed_control-[0-9a-f]{16}$", a0):
            exes.append((pid, a0))
        if os.path.basename(a0) in ("cargo", "rustc") and any(WT.encode() in a for a in argv):
            builds.append((pid, a0))
    scopes = subprocess.run(["systemctl", "--user", "list-units", "--type=scope", "--no-legend", "--plain",
                             "p3c11-*"], capture_output=True, text=True).stdout.split("\n")
    scopes = [s for s in scopes if s.strip()]
    return exes, builds, scopes


def preflight(rec):
    avail = meminfo("MemAvailable")
    exes, builds, scopes = conflicts()
    rec.append(f"pre-flight {now()}:")
    rec.append(f"  MemTotal {meminfo('MemTotal')} KiB; MemAvailable {avail} KiB (needed {NEED_KIB} KiB = 18 GiB)")
    rec.append(f"  SwapTotal {meminfo('SwapTotal')} KiB; SwapFree {meminfo('SwapFree')} KiB (the scope has swap disabled)")
    rec.append(f"  load average {open('/proc/loadavg').read().split()[:3]}; online CPUs {os.cpu_count()}")
    st = os.statvfs(WT)
    rec.append(f"  disk free on the worktree filesystem: {st.f_bavail * st.f_frsize >> 30} GiB")
    rec.append(f"  governed-control test executables running: {exes}; cargo/rustc in this worktree: {builds}; "
               f"active p3c11-* scopes: {scopes}")
    top = subprocess.run(["ps", "-eo", "rss=,comm=", "--sort=-rss"], capture_output=True, text=True).stdout.splitlines()[:8]
    rec.append("  largest resident processes (comm only; none is stopped by this launcher):")
    rec += ["    " + l.strip() for l in top]
    problems = []
    if avail < NEED_KIB:
        problems.append("not enough free memory; no workload was stopped")
    if exes or builds or scopes:
        problems.append("a conflicting test executable, build or scope is running")
    return problems


def sampler(unit, path, stop):
    with open(path, "w") as out:
        cg = None
        for _ in range(400):
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
        out.write(f"scope cgroup: {cg}\ntime memory.current memory.peak memory.swap.current oom_kill "
                  f"host.MemAvailable(KiB)\n")
        while os.path.isdir(cg) and not stop.is_set():
            try:
                cur = open(cg + "/memory.current").read().strip()
                peak = open(cg + "/memory.peak").read().strip()
                swp = open(cg + "/memory.swap.current").read().strip()
                ev = dict(l.split() for l in open(cg + "/memory.events"))
            except OSError:
                break
            out.write(f"{time.strftime('%H:%M:%S')} {cur} {peak} {swp} {ev.get('oom_kill')} {meminfo('MemAvailable')}\n")
            out.flush()
            stop.wait(2)
        out.write(f"sampler: stopped at {time.strftime('%H:%M:%S')}\n")


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
            "max_rss_kib": int(rss.group(1)) if rss else None, "wall_seconds": secs}


def parse_scope_record(path):
    d = {}
    if os.path.exists(path):
        for line in open(path):
            k, _, v = line.rstrip("\n").partition(": ")
            if k in ("memory.events", "memory.swap.events"):
                a, b = v.split()
                d[k.replace("memory.", "") + "." + a] = int(b)
            else:
                d[k] = v
    return d


def supervised(mode, outdir, unit, cwd, inner, env_inner, stdout_path, rec, limit=None):
    limit = limit or LIMITS[mode]
    scope_rec = os.path.join(outdir, "scope-record.txt")
    time_v = os.path.join(outdir, "time-v.txt")
    cmd = ["systemd-run", "--user", "--scope", f"--unit={unit}", "-p", "MemoryMax=16G", "-p", "MemorySwapMax=0",
           "-p", f"RuntimeMaxSec={limit + 10}", "--quiet", WRAP, scope_rec,
           "/usr/bin/env", "-i", *[f"{k}={v}" for k, v in env_inner],
           "/usr/bin/time", "-v", "-o", time_v, "timeout", "-s", "KILL", str(limit), "nice", "-n", "10", *inner]
    rec.append(f"scope unit: {unit}.scope; cwd: {cwd}")
    rec.append("command: " + " ".join(shlex.quote(c) for c in cmd))
    stop = threading.Event()
    th = threading.Thread(target=sampler, args=(unit, os.path.join(outdir, "cgroup-samples.txt"), stop), daemon=True)
    th.start()
    t0 = time.time()
    rec.append(f"launched: {now()}")
    with open(stdout_path, "wb") as so:
        r = subprocess.run(cmd, cwd=cwd, stdout=so, stderr=subprocess.STDOUT)
    elapsed = time.time() - t0
    rec.append(f"returned: {now()}")
    stop.set()
    th.join(timeout=10)
    show = subprocess.run(["systemctl", "--user", "show", unit + ".scope", "-p", "LoadState", "-p", "ActiveState",
                           "-p", "Result"], capture_output=True, text=True).stdout.split()
    show = dict(s.split("=", 1) for s in show if "=" in s)
    tv, sr = parse_time_v(time_v), parse_scope_record(scope_rec)
    scope_result = "gone" if show.get("LoadState") == "not-found" else show.get("Result")
    timed_out = tv["exit_status"] in (124, 137) or (tv["wall_seconds"] or 0) >= limit  # Stage 1's predicate
    sup = {"memory_max": sr.get("memory.max"), "swap_max": sr.get("memory.swap.max"),
           "memory_high": sr.get("memory.high"), "memory_peak_bytes": sr.get("memory.peak"),
           "swap_peak_bytes": sr.get("memory.swap.peak"), "oom": sr.get("events.oom"),
           "oom_kill": sr.get("events.oom_kill"), "max_events": sr.get("events.max"),
           "completed": all(k in sr for k in ("finished", "command exit status", "events.oom_kill")),
           "timed_out": timed_out, "wall_seconds": tv["wall_seconds"], "time_limit_seconds": limit,
           "backstop_runtime_max_seconds": limit + 10, "scope_result": scope_result, "scope_show": show,
           "launcher_elapsed_seconds": round(elapsed, 3), "max_rss_kib_largest_process": tv["max_rss_kib"]}
    proc = {"exit_status": tv["exit_status"], "signal": tv["signal"], "supervisor_rc": r.returncode,
            "wrapper_recorded_status": sr.get("command exit status")}
    rec.append(f"supervisor return code {r.returncode}; /usr/bin/time: exit status {tv['exit_status']}, signal "
               f"{tv['signal']}, wall {tv['wall_seconds']} s, max RSS (largest single process) {tv['max_rss_kib']} KiB")
    rec.append(f"scope (scope-wrap.sh record): memory.max {sup['memory_max']}, memory.swap.max {sup['swap_max']}, "
               f"memory.peak {sup['memory_peak_bytes']} B, memory.swap.peak {sup['swap_peak_bytes']} B, oom "
               f"{sup['oom']}, oom_kill {sup['oom_kill']}, completed {sup['completed']}, timed out {timed_out}; "
               f"systemctl after: {show}")
    return sup, proc


def finish(outdir, rec, desc, code):
    rec.append(f"finished: {now()}")
    json.dump(desc, open(os.path.join(outdir, "descriptor.json"), "w"), indent=1)
    open(os.path.join(outdir, "RECEIPT.txt"), "w").write("\n".join(rec) + "\n")
    print("\n".join(rec))
    return code


def not_started(outdir, rec, desc, reason, code):
    rec.append(f"OUTCOME: NOT STARTED ({reason})")
    desc["not_started"] = reason
    return finish(outdir, rec, desc, code)


def selftest(outdir):
    """Unchanged in behaviour from Stage 1's v3 self-test (non-Nexus processes only)."""
    os.makedirs(outdir)
    rec = [f"SUPERVISION SELF-TEST {now()} (non-Nexus processes only)",
           f"systemd: {subprocess.run(['systemctl', '--user', '--version'], capture_output=True, text=True).stdout.splitlines()[0]}",
           f"kernel: {os.uname().release}; timeout: "
           f"{subprocess.run(['timeout', '--version'], capture_output=True, text=True).stdout.splitlines()[0]}"]
    ok = True
    py = "/usr/bin/python3"
    d = os.path.join(outdir, "1")
    os.makedirs(d)
    r = ["1. the 16 GiB, zero-swap scope's limits read back from inside it (a 10 MiB allocation), full chain"]
    sup, proc = supervised("selftest", d, f"{UNIT_PREFIX}-selftest-1-{int(time.time())}", outdir,
                           [py, "-c", "b = b'x' * (10 << 20); print('allocated', len(b))"], [("PATH", "/usr/bin:/bin")],
                           os.path.join(d, "out.txt"), r)
    good = (sup["memory_max"] == "17179869184" and sup["swap_max"] == "0" and proc["exit_status"] == 0
            and sup["completed"] and sup["oom_kill"] == 0)
    ok &= good
    rec += r + [f"  RESULT {'OK' if good else 'FAILED'}", ""]
    d = os.path.join(outdir, "2")
    os.makedirs(d)
    unit = f"{UNIT_PREFIX}-selftest-mem-{int(time.time())}"
    r = subprocess.run(["systemd-run", "--user", "--scope", f"--unit={unit}", "-p", "MemoryMax=64M", "-p",
                        "MemorySwapMax=0", "--quiet", WRAP, os.path.join(d, "scope-record.txt"), py, "-c",
                        "b = b'x' * (256 << 20); print('NOT KILLED: wrote', len(b))"], capture_output=True, text=True)
    show = subprocess.run(["systemctl", "--user", "show", unit + ".scope", "-p", "Result", "-p", "ActiveState"],
                          capture_output=True, text=True).stdout.split()
    sr = parse_scope_record(os.path.join(d, "scope-record.txt"))
    complete = all(k in sr for k in ("finished", "command exit status", "events.oom_kill"))
    good = (r.returncode != 0 and "NOT KILLED" not in r.stdout and "Result=oom-kill" in show
            and (not complete or sr.get("command exit status") == "137")
            and sr.get("memory.max") == "67108864" and sr.get("memory.swap.max") == "0")
    ok &= good
    rec += ["2. memory cap enforced: a 64 MiB zero-swap scope, the process writes 256 MiB",
            f"  supervisor rc {r.returncode}; stdout {r.stdout.strip()!r}; scope after: {show}; wrapper record keys "
            f"{sorted(sr)}; complete record: {complete}; command status recorded: {sr.get('command exit status')}",
            f"  RESULT {'OK' if good else 'FAILED'}", ""]
    subprocess.run(["systemctl", "--user", "reset-failed", unit + ".scope"], capture_output=True)
    d = os.path.join(outdir, "3")
    os.makedirs(d)
    r = ["3. time limit enforced through the full chain: timeout -s KILL 3 around a 30 s sleep"]
    sup, proc = supervised("selftest", d, f"{UNIT_PREFIX}-selftest-3-{int(time.time())}", outdir, ["/bin/sleep", "30"],
                           [("PATH", "/usr/bin:/bin")], os.path.join(d, "out.txt"), r, limit=3)
    good = (sup["timed_out"] and (proc["signal"] == 9 or proc["exit_status"] == 137)
            and (sup["wall_seconds"] or 99) < 10)
    ok &= good
    rec += r + [f"  process {proc}; timed_out {sup['timed_out']}", f"  RESULT {'OK' if good else 'FAILED'}", ""]
    t0 = time.time()
    unit = f"{UNIT_PREFIX}-selftest-backstop-{int(time.time())}"
    r = subprocess.run(["systemd-run", "--user", "--scope", f"--unit={unit}", "-p", "RuntimeMaxSec=2", "--quiet",
                        "sleep", "30"], capture_output=True, text=True)
    el = time.time() - t0
    good = r.returncode != 0 and el < 10
    ok &= good
    rec += ["4. backstop enforced: RuntimeMaxSec=2 around a 30 s sleep", f"  rc {r.returncode}; elapsed {el:.2f} s",
            f"  RESULT {'OK' if good else 'FAILED'}", ""]
    subprocess.run(["systemctl", "--user", "reset-failed", unit + ".scope"], capture_output=True)
    for label, code, want in (("5. a normal exit 101 through the run chain", "import sys; sys.exit(101)",
                               lambda s, p: p["exit_status"] == 101 and p["signal"] is None and s["completed"]
                               and p["supervisor_rc"] == 101),
                              ("6. a process ended by SIGABRT through the run chain", "import os; os.abort()",
                               lambda s, p: p["signal"] == 6 and p["exit_status"] is None and p["supervisor_rc"] == 134)):
        d = os.path.join(outdir, label.split(".")[0])
        os.makedirs(d)
        r = [label]
        sup, proc = supervised("selftest", d, f"{UNIT_PREFIX}-selftest-{label[0]}-{int(time.time())}", outdir,
                               [py, "-c", code], [("PATH", "/usr/bin:/bin")], os.path.join(d, "out.txt"), r)
        good = want(sup, proc)
        ok &= good
        rec += r + [f"  process {proc}", f"  RESULT {'OK' if good else 'FAILED'}", ""]
    rec.append(f"SELF-TEST RESULT: {'ALL OK' if ok else 'FAILED'}")
    open(os.path.join(outdir, "SELFTEST.txt"), "w").write("\n".join(rec) + "\n")
    print("\n".join(rec))
    return 0 if ok else 1


def listing(argv_tail):
    r = subprocess.run(["/usr/bin/env", "-i", *[f"{k}={v}" for k, v in RUN_ENV], common.EXE, *argv_tail],
                       cwd=CRATE_DIR, capture_output=True, text=True)
    return r.returncode, r.stdout, [l for l in r.stdout.splitlines() if l.endswith(": test")]


def invoke(outdir, kind, test, argv_tail, title, prior):
    """One supervised invocation of the pinned executable; cwd is the crate directory, as cargo would run it."""
    outdir_abs = os.path.join(EV, outdir)
    os.makedirs(outdir_abs)
    argv = [common.EXE, *argv_tail]
    rec = [title, f"started: {now()}", f"mission {common.MISSION}; frozen C {common.C}"]
    desc = {"id": os.path.basename(outdir), "type": kind, "test": test, "worktree": WT, "argv": argv,
            "env": dict(RUN_ENV), "cwd": CRATE_DIR, "prior": prior}
    problem = freeze_check(rec)
    if problem:
        return not_started(outdir_abs, rec, desc, problem, 8)
    if prior is None:
        s = open(STAGE1_NORMALIZED).read() if os.path.exists(STAGE1_NORMALIZED) else ""
        ok_prior = "ALL NORMALIZED" in s
        rec.append(f"prior step: the Stage-1 normalization ({'ALL NORMALIZED' if ok_prior else 'NOT COMPLETE'})")
    else:
        cls = os.path.join(EV, prior, "CLASSIFICATION.txt")
        ok_prior = os.path.exists(cls) and open(cls).read().splitlines()[0] == "CLASSIFICATION: PASS"
        rec.append(f"prior step: {prior} {'classified PASS' if ok_prior else 'NOT classified PASS'}")
    if not ok_prior:
        return not_started(outdir_abs, rec, desc, "the previous step is not classified PASS (no run follows a stop)", 9)
    t = tree_check(rec, "before")
    before = exe_identity()
    rec.append(f"executable before: {json.dumps(before)}; pinned sha256 {common.EXE_SHA256} size {common.EXE_SIZE}: "
               f"{'MATCH' if exe_ok(before) else 'MISMATCH'}")
    rec.append("test environment (env -i; exactly these): " + " ".join(f"{k}={v}" for k, v in RUN_ENV))
    desc.update({"source_before": t, "executable": {"path": common.EXE, "pinned_sha256": common.EXE_SHA256,
                                                    "pinned_size": common.EXE_SIZE, "before": before}})
    if t["verdict"] != "EXACTLY_C":
        return not_started(outdir_abs, rec, desc, "the worktree is not exactly C", 7)
    if not exe_ok(before):
        return not_started(outdir_abs, rec, desc, "the executable is not the pinned one", 5)
    problems = preflight(rec)
    if problems:
        return not_started(outdir_abs, rec, desc, "; ".join(problems), 3)
    # Selection: exactly the one named test; for a witness, also not an ignored test.
    sel = {}
    rc, out, lines = listing([test, "--exact", "--ignored", "--list"] if kind == "full" else [test, "--exact", "--list"])
    sel["listing"] = {"argv": [test, "--exact"] + (["--ignored"] if kind == "full" else []) + ["--list"], "exit": rc,
                      "stdout": out, "listed": lines}
    if kind == "witness":
        rc2, out2, lines2 = listing([test, "--exact", "--ignored", "--list"])
        sel["ignored_listing"] = {"argv": [test, "--exact", "--ignored", "--list"], "exit": rc2, "stdout": out2,
                                  "listed": lines2}
    mid = exe_identity()
    sel["executable_after_listing"] = mid
    desc["selection"] = sel
    rec.append(f"selection: {sel['listing']['argv']} -> exit {rc}; listed {lines}" +
               (f"; with --ignored -> exit {sel['ignored_listing']['exit']}, listed {sel['ignored_listing']['listed']}"
                if kind == "witness" else "") + f"; executable after the listing {mid['sha256']} {mid['size']}")
    good_sel = rc == 0 and lines == [f"{test}: test"] and exe_ok(mid) and (
        kind == "full" or (sel["ignored_listing"]["exit"] == 0 and sel["ignored_listing"]["listed"] == []))
    if not good_sel:
        return not_started(outdir_abs, rec, desc, "the selection is not exactly the one named (nonignored) test", 6)
    sup, proc = supervised(kind, outdir_abs, f"{UNIT_PREFIX}-{os.path.basename(outdir)}-{int(time.time())}",
                           CRATE_DIR, argv, RUN_ENV, os.path.join(outdir_abs, "raw-output.txt"), rec)
    after = exe_identity()
    t2 = tree_check(rec, "after")
    raw = os.path.join(outdir_abs, "raw-output.txt")
    rec.append(f"executable after: {json.dumps(after)}: {'MATCH' if exe_ok(after) else 'MISMATCH'}")
    rec.append(f"raw output: raw-output.txt sha256 {sha256(raw)} bytes {os.path.getsize(raw)}")
    desc["executable"]["after"] = after
    desc.update({"process": proc, "supervisor": sup, "output_path": "raw-output.txt", "raw_output_sha256": sha256(raw),
                 "source_after": t2})
    return finish(outdir_abs, rec, desc, 0)


if __name__ == "__main__":
    mode = sys.argv[1]
    if mode == "selftest":
        out = os.path.abspath(sys.argv[2])
        if os.path.exists(out):
            raise SystemExit(f"REFUSED: {out} exists (one call, one receipt; nothing is retried)")
        sys.exit(selftest(out))
    if mode == "witness":
        test, outdir, prior = WITNESS[sys.argv[2]]
        if os.path.exists(os.path.join(EV, outdir)):
            raise SystemExit(f"REFUSED: {outdir} exists (one call, one receipt; nothing is retried)")
        sys.exit(invoke(outdir, "witness", test, [test, "--exact", "--nocapture", "--test-threads=1"],
                        f"SUPERVISED HEALTHY WITNESS {sys.argv[2].upper()} — {test} (type (e3))", prior))
    if mode == "full":
        test, outdir, prior = FULL[sys.argv[2]]
        if os.path.exists(os.path.join(EV, outdir)):
            raise SystemExit(f"REFUSED: {outdir} exists (one call, one receipt; nothing is retried)")
        sys.exit(invoke(outdir, "full", test, [test, "--exact", "--ignored", "--nocapture", "--test-threads=1"],
                        f"SUPERVISED FULL EXPLORATION {CFG[sys.argv[2]]} — {test} (original explorer)", prior))
    raise SystemExit("usage")

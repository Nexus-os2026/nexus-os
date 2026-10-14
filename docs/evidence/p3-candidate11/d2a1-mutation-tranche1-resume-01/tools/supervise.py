#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-MUTATION-TRANCHE1-EXEC-01 supervised launcher. One call = one supervised process = one receipt
directory. Adapted from P3-C11-DR01-D2A1-FINAL-EVIDENCE-STAGE2-01 tools/supervise.py (itself Stage 1's v3): the same
enforcement chain and self-test, with this mission's build mode (Stage 1's, with `-j 2`) and mutant-run mode.

  supervise.py selftest <dir>          enforcement self-test with NON-Nexus processes only
  supervise.py baseline <key>          one standalone baseline receipt on the pinned baseline executable (mission 7):
                                       X-1 (e1), then T-BIND, T-CELL, W6, T-PREC, T-RES (e2), in that order
  (library) build(...)                 one supervised build (healthy or mutant), used by healthy.py and control.py
  (library) run_mutant(...)            one supervised invocation of a pinned mutant executable, used by control.py

Enforcement: a transient systemd user scope with MemoryMax=16G and MemorySwapMax=0 (cgroup v2), RuntimeMaxSec (limit
+ 10 s) as a backstop, and `timeout -s KILL <limit>` as the wall-time limit (test 180 s or the configuration's adopted
time ceiling; build 900 s). scope-wrap.sh (Stage 1's, unchanged), the scope's first process, records the enforced
memory.max and memory.swap.max before the command and memory.peak, memory.swap.peak and memory.events after it.
/usr/bin/time -v records the exit status or terminating signal. A sampler records the scope's memory every 2 s.
`nice -n 10` keeps the Owner's workloads ahead; no other process is touched.

Fail-closed rules: the receipt directory must not exist; every frozen file must equal the freeze record; the source
must be in its expected state (exactly C, or exactly C plus the control's patch), byte for byte, before and after; the
executable must be the pinned one (path, sha256 and size) before, at selection and after; MemAvailable >= 18 GiB
before starting; no governed-control test executable, no cargo/rustc in the worktree and no scope of these missions may
be running; a test selection listing must show exactly the one named test; an invocation starts only when the
previous step in the frozen order is classified as its expected success. Nothing is retried.
"""
import json
import os
import re
import shlex
import subprocess
import sys
import threading
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import common as K  # noqa: E402

EV = K.EV
WRAP = os.path.join(EV, "tools/scope-wrap.sh")
VERIFY = os.path.join(EV, "tools/verify_tree.py")
VERIFY_MUTANT = os.path.join(EV, "tools/verify_mutant.py")
FREEZE = os.path.join(EV, "receipts/05-FREEZE.json")
EXPECT = os.path.join(EV, "receipts/02-EXPECTATIONS.json")


def now():
    return time.strftime("%Y-%m-%dT%H:%M:%S%z")


def meminfo(key):
    for line in open("/proc/meminfo"):
        if line.startswith(key + ":"):
            return int(line.split()[1])


def expectations():
    return json.load(open(EXPECT))


def freeze_check(rec):
    """Every frozen file's sha256 equals the freeze record; returns a problem or None."""
    if not os.path.exists(FREEZE):
        return "no freeze record"
    fr = json.load(open(FREEZE))
    bad = []
    for rel, digest in sorted(fr["files"].items()):
        p = os.path.join(EV, rel)
        if not os.path.exists(p) or K.sha256_file(p) != digest:
            bad.append(rel)
    rec.append(f"freeze check: {len(fr['files'])} frozen files (tools, inputs, expectations) against "
               f"receipts/05-FREEZE.json: {'ALL EQUAL' if not bad else 'CHANGED: ' + str(bad)}")
    return ("frozen files changed: " + ", ".join(bad)) if bad else None


def exe_identity(path):
    st = os.stat(path)
    return dict(path=path, sha256=K.sha256_file(path), size=st.st_size, mtime_ns=st.st_mtime_ns, inode=st.st_ino,
                symlink=os.path.islink(path))


def exe_ok(i, sha, size):
    return i["sha256"] == sha and i["size"] == size and not i["symlink"]


def tree_check(rec, wt, when):
    r = subprocess.run([sys.executable, VERIFY, wt], capture_output=True, text=True)
    d = json.loads(r.stdout)
    rec.append(f"source ({when}): {d['verdict']}; HEAD {d['head']}; tree {d['tree']}; detached {d['detached']}; "
               f"{d['tracked_blobs_compared']} tracked blobs, {d['bytes_compared']} bytes compared; status entries "
               f"{d['status_entries']}; mismatches {d['content_mismatches']}; missing {d['missing']}; "
               f"modes {d['mode_mismatches']}")
    return d


def mutant_check(rec, wt, ctl, when):
    r = subprocess.run([sys.executable, VERIFY_MUTANT, wt, ctl["source_file"], ctl["mutant_blob"],
                        ctl["mutant_sha256"]], capture_output=True, text=True)
    d = json.loads(r.stdout)
    m = d["mutant_file"] or {}
    rec.append(f"source ({when}): {d['verdict']}; HEAD {d['head']}; tree {d['tree']}; detached {d['detached']}; "
               f"{d['tracked_blobs_compared']} tracked blobs, {d['bytes_compared']} bytes compared; status entries "
               f"{d['status_entries']}; other mismatches {d['other_content_mismatches']}; missing {d['missing']}; "
               f"modes {d['mode_mismatches']}; {m.get('path')}: git blob {m.get('git_blob')} sha256 {m.get('sha256')} "
               f"(expected {ctl['mutant_blob']} / {ctl['mutant_sha256']})")
    return d


def conflicts(wt):
    """Governed-control test executables anywhere (argv[0] is the executable; comm is truncated), cargo or rustc
    processes working in this worktree, and active scopes of these missions."""
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
        if os.path.basename(a0) in ("cargo", "rustc") and any(wt.encode() in a for a in argv):
            builds.append((pid, a0))
    scopes = subprocess.run(["systemctl", "--user", "list-units", "--type=scope", "--no-legend", "--plain",
                             "p3c11-*"], capture_output=True, text=True).stdout.split("\n")
    scopes = [s for s in scopes if s.strip()]
    return exes, builds, scopes


def preflight(rec, wt):
    avail = meminfo("MemAvailable")
    exes, builds, scopes = conflicts(wt)
    rec.append(f"pre-flight {now()}:")
    rec.append(f"  MemTotal {meminfo('MemTotal')} KiB; MemAvailable {avail} KiB (needed {K.NEED_KIB} KiB = 18 GiB)")
    rec.append(f"  SwapTotal {meminfo('SwapTotal')} KiB; SwapFree {meminfo('SwapFree')} KiB (the scope has swap disabled)")
    rec.append(f"  load average {open('/proc/loadavg').read().split()[:3]}; online CPUs {os.cpu_count()}")
    st = os.statvfs(wt)
    rec.append(f"  disk free on the worktree filesystem: {st.f_bavail * st.f_frsize >> 30} GiB")
    rec.append(f"  governed-control test executables running: {exes}; cargo/rustc in this worktree: {builds}; "
               f"active p3c11-* scopes: {scopes}")
    top = subprocess.run(["ps", "-eo", "rss=,comm=", "--sort=-rss"], capture_output=True, text=True).stdout.splitlines()[:8]
    rec.append("  largest resident processes (comm only; none is stopped by this launcher):")
    rec += ["    " + l.strip() for l in top]
    problems = []
    if avail < K.NEED_KIB:
        problems.append("not enough free memory; no workload was stopped")
    if (st.f_bavail * st.f_frsize >> 30) < 5:
        problems.append("less than 5 GiB of free disk; nothing was deleted")
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


def supervised(outdir, unit, cwd, inner, env_inner, stdout_path, rec, limit, stderr_path=None):
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
        if stderr_path:
            with open(stderr_path, "wb") as se:
                r = subprocess.run(cmd, cwd=cwd, stdout=so, stderr=se)
        else:
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


def first_line(path):
    return open(path).readline().rstrip("\n") if os.path.exists(path) else None


# ---- self-test ------------------------------------------------------------------------------------------------------

def selftest(outdir):
    """Unchanged in behaviour from Stage 1's v3 self-test (non-Nexus processes only), plus a separate-stderr case."""
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
    sup, proc = supervised(d, f"{K.UNIT_PREFIX}-selftest-1-{int(time.time())}", outdir,
                           [py, "-c", "b = b'x' * (10 << 20); print('allocated', len(b))"], [("PATH", "/usr/bin:/bin")],
                           os.path.join(d, "out.txt"), r, 60)
    good = (sup["memory_max"] == K.MEMORY_MAX and sup["swap_max"] == "0" and proc["exit_status"] == 0
            and sup["completed"] and sup["oom_kill"] == 0)
    ok &= good
    rec += r + [f"  RESULT {'OK' if good else 'FAILED'}", ""]
    d = os.path.join(outdir, "2")
    os.makedirs(d)
    unit = f"{K.UNIT_PREFIX}-selftest-mem-{int(time.time())}"
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
    sup, proc = supervised(d, f"{K.UNIT_PREFIX}-selftest-3-{int(time.time())}", outdir, ["/bin/sleep", "30"],
                           [("PATH", "/usr/bin:/bin")], os.path.join(d, "out.txt"), r, 3)
    good = (sup["timed_out"] and (proc["signal"] == 9 or proc["exit_status"] == 137)
            and (sup["wall_seconds"] or 99) < 10)
    ok &= good
    rec += r + [f"  process {proc}; timed_out {sup['timed_out']}", f"  RESULT {'OK' if good else 'FAILED'}", ""]
    t0 = time.time()
    unit = f"{K.UNIT_PREFIX}-selftest-backstop-{int(time.time())}"
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
        sup, proc = supervised(d, f"{K.UNIT_PREFIX}-selftest-{label[0]}-{int(time.time())}", outdir,
                               [py, "-c", code], [("PATH", "/usr/bin:/bin")], os.path.join(d, "out.txt"), r, 60)
        good = want(sup, proc)
        ok &= good
        rec += r + [f"  process {proc}", f"  RESULT {'OK' if good else 'FAILED'}", ""]
    d = os.path.join(outdir, "7")
    os.makedirs(d)
    r = ["7. separate stdout and stderr files (the build mode), environment exactly as given (env -i)"]
    sup, proc = supervised(d, f"{K.UNIT_PREFIX}-selftest-7-{int(time.time())}", outdir,
                           [py, "-c", "import os, sys; print(sorted(os.environ)); print('to stderr', file=sys.stderr)"],
                           [("PATH", "/usr/bin:/bin"), ("CARGO_NET_OFFLINE", "true")], os.path.join(d, "out.txt"), r, 60,
                           stderr_path=os.path.join(d, "err.txt"))
    o, e = open(os.path.join(d, "out.txt")).read(), open(os.path.join(d, "err.txt")).read()
    # Python's own C-locale coercion (PEP 538) may add LC_CTYPE inside the child; nothing else may appear.
    good = (proc["exit_status"] == 0 and e.strip() == "to stderr" and "to stderr" not in o
            and o.strip() in (str(sorted(["PATH", "CARGO_NET_OFFLINE", "LC_CTYPE"])),
                              str(sorted(["PATH", "CARGO_NET_OFFLINE"]))))
    ok &= good
    rec += r + [f"  stdout {o.strip()!r}; stderr {e.strip()!r}; process {proc}", f"  RESULT {'OK' if good else 'FAILED'}",
                ""]
    rec.append(f"SELF-TEST RESULT: {'ALL OK' if ok else 'FAILED'}")
    open(os.path.join(outdir, "SELFTEST.txt"), "w").write("\n".join(rec) + "\n")
    print("\n".join(rec))
    return 0 if ok else 1


# ---- test selection and invocation ----------------------------------------------------------------------------------

def listing(exe, cwd, argv_tail):
    r = subprocess.run(["/usr/bin/env", "-i", *[f"{k}={v}" for k, v in K.RUN_ENV], exe, *argv_tail],
                       cwd=cwd, capture_output=True, text=True)
    return {"argv": argv_tail, "exit": r.returncode, "stdout": r.stdout, "stderr": r.stderr,
            "listed": [l for l in r.stdout.splitlines() if l.endswith(": test")]}


def select(exe, cwd, test, ignored):
    """The selection evidence: `--exact --list` must show exactly the one test; `--exact --ignored --list` shows it
    exactly when it is an ignored test. Returns (selection record, ok)."""
    a = listing(exe, cwd, [test, "--exact", "--list"])
    b = listing(exe, cwd, [test, "--exact", "--ignored", "--list"])
    one = [f"{test}: test"]
    ok = (a["exit"] == 0 and a["listed"] == one and b["exit"] == 0 and b["listed"] == (one if ignored else []))
    return {"listing": a, "ignored_listing": b, "expected_ignored": ignored}, ok


def invoke(outdir, title, exe, exe_sha, exe_size, cwd, wt, test, ignored, argv_tail, limit, source_ok, kind, prior,
           extra_desc, gate=True, env=None):
    """One supervised invocation of a pinned executable. source_ok(rec, when) -> (verdict dict, ok). gate=False and
    env are for tool_controls.py's synthetic, non-Nexus programs only (no freeze or prior-step gate; extra variables)."""
    env = env or K.RUN_ENV
    os.makedirs(outdir)
    argv = [exe, *argv_tail]
    rec = [title, f"started: {now()}", f"mission {K.MISSION}; frozen C {K.C}"]
    desc = {"id": os.path.basename(outdir), "type": kind, "test": test, "worktree": wt, "argv": argv,
            "env": dict(env), "cwd": cwd, "prior": prior, "limit_seconds": limit}
    desc.update(extra_desc)
    if gate:
        problem = freeze_check(rec)
        if problem:
            return not_started(outdir, rec, desc, problem, 8)
    else:
        rec.append("SYNTHETIC DEVELOPMENT RUN (tool_controls.py): no freeze or prior-step gate; not Nexus evidence")
    for p, want in prior:
        got = first_line(os.path.join(EV, p))
        rec.append(f"prior step: {p}: {got!r} (required {want!r})")
        if got != want:
            return not_started(outdir, rec, desc, "a previous step is not classified as required (no run follows a "
                                                  "stop)", 9)
    src, good_src = source_ok(rec, "before")
    before = exe_identity(exe)
    rec.append(f"executable before: {json.dumps(before)}; pinned sha256 {exe_sha} size {exe_size}: "
               f"{'MATCH' if exe_ok(before, exe_sha, exe_size) else 'MISMATCH'}")
    rec.append("test environment (env -i; exactly these): " + " ".join(f"{k}={v}" for k, v in env))
    desc.update({"source_before": src, "executable": {"path": exe, "pinned_sha256": exe_sha, "pinned_size": exe_size,
                                                      "before": before}})
    if not good_src:
        return not_started(outdir, rec, desc, "the source is not in its expected state", 7)
    if not exe_ok(before, exe_sha, exe_size):
        return not_started(outdir, rec, desc, "the executable is not the pinned one", 5)
    problems = preflight(rec, wt)
    if problems:
        return not_started(outdir, rec, desc, "; ".join(problems), 3)
    sel, good_sel = select(exe, cwd, test, ignored)
    mid = exe_identity(exe)
    sel["executable_after_listing"] = mid
    desc["selection"] = sel
    rec.append(f"selection: {sel['listing']['argv']} -> exit {sel['listing']['exit']}, listed {sel['listing']['listed']}; "
               f"{sel['ignored_listing']['argv']} -> exit {sel['ignored_listing']['exit']}, listed "
               f"{sel['ignored_listing']['listed']} (ignored test expected: {ignored}); executable after the listings "
               f"{mid['sha256']} {mid['size']}")
    if not good_sel or not exe_ok(mid, exe_sha, exe_size):
        return not_started(outdir, rec, desc, "the selection is not exactly the one named test with its ignored status",
                           6)
    sup, proc = supervised(outdir, f"{K.UNIT_PREFIX}-{os.path.basename(outdir)}-{int(time.time())}", cwd, argv,
                           env, os.path.join(outdir, "raw-output.txt"), rec, limit)
    after = exe_identity(exe)
    src2, _ = source_ok(rec, "after")
    raw = os.path.join(outdir, "raw-output.txt")
    rec.append(f"executable after: {json.dumps(after)}: {'MATCH' if exe_ok(after, exe_sha, exe_size) else 'MISMATCH'}")
    rec.append(f"raw output: raw-output.txt sha256 {K.sha256_file(raw)} bytes {os.path.getsize(raw)}")
    desc["executable"]["after"] = after
    desc.update({"process": proc, "supervisor": sup, "output_path": "raw-output.txt",
                 "raw_output_sha256": K.sha256_file(raw), "source_after": src2})
    return finish(outdir, rec, desc, 0)


BASELINE_PRIOR = {"X-1": [("receipts/05-FREEZE-RECORD.txt", "FREEZE: RECORDED")],
                  "T-BIND": [("receipts/10-baseline-X-1/CLASSIFICATION.txt", "CLASSIFICATION: PASS")],
                  "T-CELL": [("receipts/11-baseline-T-BIND/CLASSIFICATION.txt", "CLASSIFICATION: PASS")],
                  "W6": [("receipts/12-baseline-T-CELL/CLASSIFICATION.txt", "CLASSIFICATION: PASS")],
                  "T-PREC": [("receipts/13-baseline-W6/CLASSIFICATION.txt", "CLASSIFICATION: PASS")],
                  "T-RES": [("receipts/14-baseline-T-PREC/CLASSIFICATION.txt", "CLASSIFICATION: PASS")]}


def baseline(key):
    b = next(x for x in expectations()["baselines"] if x["key"] == key)
    outdir = os.path.join(EV, b["dir"])
    if os.path.exists(outdir):
        raise SystemExit(f"REFUSED: {b['dir']} exists (one call, one receipt; nothing is retried)")
    cwd = os.path.join(K.BASE_WT, K.CRATE)

    def source_ok(rec, when):
        d = tree_check(rec, K.BASE_WT, when)
        return d, d["verdict"] == "EXACTLY_C"
    title = (f"SUPERVISED BASELINE {key} — {b['test']} ("
             + ("type (e1) unchanged-core exploration PASS" if b["type"] == "e1" else "type (e2) restored named-unit-test "
                "PASS") + ") on the pinned baseline executable")
    return invoke(outdir, title, K.EXE, K.EXE_SHA256, K.EXE_SIZE, cwd, K.BASE_WT, b["test"], b["ignored"],
                  b["argv_tail"], b["limit_seconds"], source_ok, b["type"], BASELINE_PRIOR[key],
                  {"baseline_key": key, "control": b["control"]})


# ---- builds ---------------------------------------------------------------------------------------------------------

def toolchain_record(rec, wt, env):
    tc = lambda c, *a: subprocess.run([c, *a], cwd=wt, capture_output=True, text=True,
                                      env=dict(env)).stdout.strip()
    rec.append(f"compiler (rustup proxy resolving rust-toolchain.toml, same environment): {tc('rustc', '--version')}")
    rec += ["  rustc -vV: " + l for l in tc("rustc", "-vV").splitlines()]
    rec.append(f"cargo: {tc('cargo', '--version')}")
    rec += ["  cargo -vV: " + l for l in tc("cargo", "-vV").splitlines()]
    rec.append(f"rustup: {tc('rustup', '--version')}; active toolchain: {tc('rustup', 'show', 'active-toolchain')}")
    rec.append(".cargo/config.toml (repository, unchanged): " +
               open(os.path.join(wt, ".cargo/config.toml")).read().replace("\n", " | "))
    others = [f"{d}/.cargo/{n}" for d in ("/home/nexus/NEXUS", "/home/nexus", "/home", "") for n in ("config", "config.toml")
              if os.path.exists(f"{d}/.cargo/{n}")]
    rec.append(f"ancestor and CARGO_HOME Cargo configuration files other than the repository's: {others or 'none'} "
               f"(checked /home/nexus/NEXUS, /home/nexus, /home, / for .cargo/config and .cargo/config.toml)")
    rel = open(os.path.join(wt, "Cargo.toml")).read()
    rec.append("[profile.release] (workspace Cargo.toml, unchanged): " + " | ".join(
        re.search(r"\[profile\.release\]\n(.*?)(?:\n\[|\Z)", rel, re.S).group(1).splitlines()))
    inputs = {}
    for f in ("rust-toolchain.toml", "Cargo.lock", ".cargo/config.toml", "Cargo.toml", K.CRATE + "/Cargo.toml"):
        p = os.path.join(wt, f)
        inputs[f] = {"git_blob": K.git_blob_id(open(p, "rb").read()), "sha256": K.sha256_file(p)}
        rec.append(f"build input {f}: git blob {inputs[f]['git_blob']} sha256 {inputs[f]['sha256']}")
    return {"rustc": tc("rustc", "--version"), "cargo": tc("cargo", "--version"),
            "rustc_vV": tc("rustc", "-vV"), "cargo_vV": tc("cargo", "-vV"),
            "active_toolchain": tc("rustup", "show", "active-toolchain").split(" (overridden")[0],
            "other_cargo_configs": others, "build_inputs": inputs}


def deps_artifacts(target):
    deps = os.path.join(target, "release/deps")
    out = []
    if os.path.isdir(deps):
        for n in sorted(os.listdir(deps)):
            if n.startswith("nexus_governed_control"):
                p = os.path.join(deps, n)
                out.append({"path": p, "sha256": K.sha256_file(p) if os.path.isfile(p) else None,
                            "size": os.path.getsize(p), "executable_bit": bool(os.stat(p).st_mode & 0o100),
                            "test_executable_name": bool(re.fullmatch(r"nexus_governed_control-[0-9a-f]{16}", n))})
    return out


def target_manifest(target, path):
    """Every regular file under the target directory: path, size, sha256 (F.1 (d) and (e4): recorded as found)."""
    n, total = 0, 0
    with open(path, "w") as out:
        out.write("# sha256\tsize\tpath (relative to the target directory)\n")
        for d, dirs, files in os.walk(target):
            dirs.sort()
            for f in sorted(files):
                p = os.path.join(d, f)
                if os.path.islink(p) or not os.path.isfile(p):
                    out.write(f"-\t-\t{os.path.relpath(p, target)} (not a regular file)\n")
                    continue
                s = os.path.getsize(p)
                out.write(f"{K.sha256_file(p)}\t{s}\t{os.path.relpath(p, target)}\n")
                n += 1
                total += s
    return n, total


def build(outdir, title, wt, source_ok, prior, extra_desc, manifest, gate=True):
    """One supervised build in worktree wt with a fresh worktree-local target directory; nothing is classified here.
    gate=False is for tool_controls.py's synthetic, non-Nexus workspace only (no freeze or prior-step gate)."""
    os.makedirs(outdir)
    target = os.path.join(wt, "target")
    env = K.build_env(target)
    rec = [title, f"started: {now()}", f"mission {K.MISSION}; frozen C {K.C}"]
    desc = {"id": os.path.basename(outdir), "type": "build", "worktree": wt, "cwd": wt, "argv": K.BUILD_ARGV,
            "env": dict(env), "target": target, "prior": prior, "limit_seconds": K.BUILD_LIMIT}
    desc.update(extra_desc)
    if gate:
        problem = freeze_check(rec)
        if problem:
            return not_started(outdir, rec, desc, problem, 8)
    else:
        rec.append("SYNTHETIC DEVELOPMENT BUILD (tool_controls.py): no freeze or prior-step gate; not Nexus evidence")
    for p, want in prior:
        got = first_line(os.path.join(EV, p))
        rec.append(f"prior step: {p}: {got!r} (required {want!r})")
        if got != want:
            return not_started(outdir, rec, desc, "a previous step is not classified as required (no build follows a "
                                                  "stop)", 9)
    src, good_src = source_ok(rec, "before")
    desc["source_before"] = src
    desc["toolchain"] = toolchain_record(rec, wt, env)
    rec.append(f"target directory {target} existed before: {os.path.lexists(target)}")
    rec.append("build environment (env -i; exactly these, nothing inherited): " + " ".join(f"{k}={v}" for k, v in env))
    rec.append("  CARGO_TARGET_DIR: the worktree-local fresh target directory. CARGO_TERM_VERBOSE=true: output only, "
               "the equivalent of -v, so that the build's own messages record each rustc invocation's effective flags "
               "(as in the accepted Stage-1 build); it enters no fingerprint and no artifact. CARGO_NET_OFFLINE=true: no "
               "network; with --locked the lock file alone resolves dependencies. -j 2: two compiler jobs.")
    rec.append("  unset by env -i (so not in effect): RUSTFLAGS, CARGO_ENCODED_RUSTFLAGS, RUSTDOCFLAGS, RUSTC, "
               "RUSTC_WRAPPER, RUSTC_WORKSPACE_WRAPPER, RUSTUP_TOOLCHAIN, RUSTUP_HOME, CARGO_HOME, CARGO_BUILD_*, "
               "CARGO_PROFILE_*, CARGO_INCREMENTAL, CARGO_TARGET_*, CARGO_ENCODED_RUSTDOCFLAGS")
    launcher_env = sorted(k for k in os.environ if re.match(r"^(CARGO|RUST|RUSTUP)", k))
    rec.append(f"  CARGO*/RUST* variables in the launcher's own environment (names; none is passed): {launcher_env}")
    if not good_src:
        return not_started(outdir, rec, desc, "the source is not in its expected state", 7)
    if os.path.lexists(target):
        return not_started(outdir, rec, desc, "the target directory already exists: not a fresh build", 7)
    problems = preflight(rec, wt)
    if problems:
        return not_started(outdir, rec, desc, "; ".join(problems), 3)
    sup, proc = supervised(outdir, f"{K.UNIT_PREFIX}-{os.path.basename(os.path.dirname(outdir))}-build-"
                                   f"{int(time.time())}", wt, K.BUILD_ARGV, env, os.path.join(outdir, "build.stdout"),
                           rec, K.BUILD_LIMIT, stderr_path=os.path.join(outdir, "build.stderr"))
    src2, _ = source_ok(rec, "after")
    arts = deps_artifacts(target)
    rec.append(f"nexus_governed_control* artifacts in the target's release/deps: "
               f"{[(os.path.basename(a['path']), a['sha256'], a['size']) for a in arts]}")
    desc.update({"process": proc, "supervisor": sup, "stdout_path": "build.stdout", "stderr_path": "build.stderr",
                 "stdout_sha256": K.sha256_file(os.path.join(outdir, "build.stdout")),
                 "stderr_sha256": K.sha256_file(os.path.join(outdir, "build.stderr")),
                 "source_after": src2, "deps_artifacts": arts})
    if manifest:
        n, total = target_manifest(target, os.path.join(outdir, "TARGET-MANIFEST.tsv"))
        rec.append(f"target manifest: TARGET-MANIFEST.tsv, {n} regular files, {total} bytes, sha256 "
                   f"{K.sha256_file(os.path.join(outdir, 'TARGET-MANIFEST.tsv'))}")
        desc["target_manifest"] = {"path": "TARGET-MANIFEST.tsv", "files": n, "bytes": total,
                                   "sha256": K.sha256_file(os.path.join(outdir, "TARGET-MANIFEST.tsv"))}
    return finish(outdir, rec, desc, 0)


if __name__ == "__main__":
    mode = sys.argv[1]
    if mode == "selftest":
        out = os.path.abspath(sys.argv[2])
        if os.path.exists(out):
            raise SystemExit(f"REFUSED: {out} exists (one call, one receipt; nothing is retried)")
        sys.exit(selftest(out))
    if mode == "baseline":
        sys.exit(baseline(sys.argv[2]))
    raise SystemExit("usage")

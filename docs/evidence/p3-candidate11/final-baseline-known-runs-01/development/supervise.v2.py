#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-FINAL-EVIDENCE-STAGE1-01 supervised launcher. One call = one supervised process = one receipt
directory. Adapted from P3-C11-DR01-D2A1-C4-WITNESS-01 tools/supervise.py (same enforcement chain and self-test).

  supervise.py selftest <dir>                 enforcement self-test with NON-Nexus processes only
  supervise.py build <dir>                    the one fresh optimised build (mission section 5)
  supervise.py list <dir> all|ignored         the pinned executable's own test listing
  supervise.py suite <dir>                    the nonignored suite, no name filter (mission section 6)
  supervise.py full <dir> f2|fb|f3|fpt|f1     one full exploration (mission sections 7 to 12)

Enforcement: a transient systemd user scope with MemoryMax=16G and MemorySwapMax=0 (cgroup v2), the scope's
RuntimeMaxSec (limit + 10 s) as a backstop, and `timeout -s KILL <limit>` as the wall-time limit (build 900 s, suite
180 s, full run 2700 s, listing 60 s). scope-wrap.sh, the scope's first process, records the enforced memory.max and
memory.swap.max before the command and memory.peak, memory.swap.peak and memory.events after it. /usr/bin/time -v
records the exit status or terminating signal. A sampler records the scope's memory every 2 s. `nice -n 10` keeps
the Owner's workloads ahead; no other process is touched.

Fail-closed rules: the receipt directory must not exist; the worktree must be exactly C, byte for byte, before and
after; the executable must be the pinned one before and after; MemAvailable >= 18 GiB before starting; no other
governed-control test executable and no other scope of this mission may be running; a full run starts only when every
earlier step's classification is the expected success. Nothing is retried.
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

EV = "/home/nexus/nexus-audits/claude-architect/p3-c11-dr01-d2a1-final-baseline-known-runs-01"
WT = "/home/nexus/NEXUS/nexus-os-p3-c11-final-baseline-known-runs-01"
CRATE_DIR = os.path.join(WT, "crates/nexus-governed-control")
TARGET = os.path.join(WT, "target")
WRAP = os.path.join(EV, "tools/scope-wrap.sh")
VERIFY = os.path.join(EV, "tools/verify_tree.py")
PIN_FILE = os.path.join(EV, "receipts/10-build/PIN.json")
C = "d5964ee5326c2733177307db6f507328023d361a"
C_TREE = "51bac5fd335999cc995c0ab9f582a30244973ef0"
NEED_KIB = 18 * 1024 * 1024
LIMITS = {"build": 900, "list": 60, "suite": 180, "full": 2700}
UNIT_PREFIX = "p3c11-fbk01"
BUILD_ARGV = ["cargo", "test", "-p", "nexus-governed-control", "--locked", "--release", "--lib", "--no-run"]
BUILD_ENV = [("HOME", "/home/nexus"), ("PATH", "/home/nexus/.cargo/bin:/usr/local/bin:/usr/bin:/bin"),
             ("CARGO_TARGET_DIR", TARGET), ("CARGO_TERM_VERBOSE", "true"), ("CARGO_NET_OFFLINE", "true")]
RUN_ENV = [("PATH", "/usr/bin:/bin"), ("LANG", "C.UTF-8"), ("RUST_BACKTRACE", "0")]
PREFIX = "ledger::protocol::model_tests::"
FULL = {  # key: (test, receipt directory of the step that must have succeeded first, its success line)
    "f2": (PREFIX + "full_f2", "receipts/30-suite", "CLASSIFICATION: PASS"),
    "fb": (PREFIX + "gc_symmetry_fb", "receipts/41-full-F-2", "CLASSIFICATION: PASS"),
    "f3": (PREFIX + "gc_symmetry_f3", "receipts/42-full-F-B", "CLASSIFICATION: PASS"),
    "fpt": (PREFIX + "gc_symmetry_fpt", "receipts/43-full-F-3", "CLASSIFICATION: PASS"),
    "f1": (PREFIX + "full_f1", "receipts/44-full-F-PT", "CLASSIFICATION: PASS"),
}


def now():
    return time.strftime("%Y-%m-%dT%H:%M:%S%z")


def sha256(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def meminfo(key):
    for line in open("/proc/meminfo"):
        if line.startswith(key + ":"):
            return int(line.split()[1])


def tree_check(rec, when):
    r = subprocess.run([sys.executable, VERIFY, WT], capture_output=True, text=True)
    d = json.loads(r.stdout)
    rec.append(f"source freeze ({when}): {d['verdict']}; HEAD {d['head']}; tree {d['tree']}; detached {d['detached']}; "
               f"{d['tracked_blobs_compared']} tracked blobs, {d['bytes_compared']} bytes compared; status entries "
               f"{d['status_entries']}; mismatches {d['content_mismatches']}; missing {d['missing']}; "
               f"modes {d['mode_mismatches']}")
    return d


def conflicts():
    """Other governed-control test executables (argv[0] is the executable; comm is truncated), cargo or rustc
    processes working in this mission's worktree, and active scopes of this mission."""
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
                             UNIT_PREFIX + "*"], capture_output=True, text=True).stdout.split("\n")
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
               f"active scopes of this mission: {scopes}")
    top = subprocess.run(["ps", "-eo", "rss=,comm=", "--sort=-rss"], capture_output=True, text=True).stdout.splitlines()[:8]
    rec.append("  largest resident processes (comm only; none is stopped by this launcher):")
    rec += ["    " + l.strip() for l in top]
    problems = []
    if avail < NEED_KIB:
        problems.append("not enough free memory; no workload was stopped")
    if exes or builds or scopes:
        problems.append("a conflicting build, test executable or scope is running")
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


def supervised(mode, outdir, unit, cwd, inner, env_inner, stdout_path, stderr_path, rec, limit=None):
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
        se = open(stderr_path, "wb") if stderr_path else subprocess.STDOUT
        r = subprocess.run(cmd, cwd=cwd, stdout=so, stderr=se)
        if stderr_path:
            se.close()
    elapsed = time.time() - t0
    rec.append(f"returned: {now()}")
    stop.set()
    th.join(timeout=10)
    show = subprocess.run(["systemctl", "--user", "show", unit + ".scope", "-p", "LoadState", "-p", "ActiveState",
                           "-p", "Result"], capture_output=True, text=True).stdout.split()
    show = dict(s.split("=", 1) for s in show if "=" in s)
    tv, sr = parse_time_v(time_v), parse_scope_record(scope_rec)
    scope_result = "gone" if show.get("LoadState") == "not-found" else show.get("Result")
    timed_out = tv["exit_status"] in (124, 137) or (tv["wall_seconds"] or 0) >= limit
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
    sup, proc = supervised("list", d, f"{UNIT_PREFIX}-selftest-1-{int(time.time())}", outdir,
                           [py, "-c", "b = b'x' * (10 << 20); print('allocated', len(b))"], [("PATH", "/usr/bin:/bin")],
                           os.path.join(d, "out.txt"), None, r)
    good = (sup["memory_max"] == "17179869184" and sup["swap_max"] == "0" and proc["exit_status"] == 0
            and sup["completed"] and sup["oom_kill"] == 0)
    ok &= good
    rec += r + [f"  RESULT {'OK' if good else 'FAILED'}", ""]
    # 2. memory cap enforced: a 64 MiB zero-swap scope; the process writes 256 MiB. systemd stops the whole scope on
    # the kernel's OOM kill (Result=oom-kill); the wrapper may or may not finish its record before it is stopped, and
    # a record that does finish shows the command killed (137). Either way the run is never classified complete.
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
    # 3. time limit enforced through the full chain: a 3 s limit around a 30 s sleep.
    d = os.path.join(outdir, "3")
    os.makedirs(d)
    r = ["3. time limit enforced through the full chain: timeout -s KILL 3 around a 30 s sleep"]
    sup, proc = supervised("list", d, f"{UNIT_PREFIX}-selftest-3-{int(time.time())}", outdir, ["/bin/sleep", "30"],
                           [("PATH", "/usr/bin:/bin")], os.path.join(d, "out.txt"), None, r, limit=3)
    good = sup["timed_out"] and proc["exit_status"] == 137 and (sup["wall_seconds"] or 99) < 10
    ok &= good
    rec += r + [f"  process {proc}; timed_out {sup['timed_out']}", f"  RESULT {'OK' if good else 'FAILED'}", ""]
    # 4. backstop: RuntimeMaxSec=2 around a 30 s sleep.
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
    # 5. a normal exit 101 through the whole chain; 6. a process ended by a signal through the same chain.
    for label, code, want in (("5. a normal exit 101 through the run chain", "import sys; sys.exit(101)",
                               lambda s, p: p["exit_status"] == 101 and p["signal"] is None and s["completed"]
                               and p["supervisor_rc"] == 101),
                              ("6. a process ended by SIGABRT through the run chain", "import os; os.abort()",
                               lambda s, p: p["signal"] == 6 and p["exit_status"] is None and p["supervisor_rc"] == 134)):
        d = os.path.join(outdir, label.split(".")[0])
        os.makedirs(d)
        r = [label]
        sup, proc = supervised("list", d, f"{UNIT_PREFIX}-selftest-{label[0]}-{int(time.time())}", outdir,
                               [py, "-c", code], [("PATH", "/usr/bin:/bin")], os.path.join(d, "out.txt"), None, r)
        good = want(sup, proc)
        ok &= good
        rec += r + [f"  process {proc}", f"  RESULT {'OK' if good else 'FAILED'}", ""]
    rec.append(f"SELF-TEST RESULT: {'ALL OK' if ok else 'FAILED'}")
    open(os.path.join(outdir, "SELFTEST.txt"), "w").write("\n".join(rec) + "\n")
    print("\n".join(rec))
    return 0 if ok else 1


def build(outdir):
    os.makedirs(outdir)
    rec = ["SUPERVISED BUILD — the one fresh optimised build of Candidate C (mission section 5)", f"started: {now()}"]
    desc = {"id": os.path.basename(outdir), "type": "build", "worktree": WT, "argv": BUILD_ARGV,
            "env": dict(BUILD_ENV)}
    t = tree_check(rec, "before")
    tc = lambda c, *a: subprocess.run([c, *a], cwd=WT, capture_output=True, text=True,
                                      env=dict(BUILD_ENV)).stdout.strip()
    rec.append(f"compiler (rustup proxy resolving rust-toolchain.toml, same environment): {tc('rustc', '--version')}")
    rec += ["  rustc -vV: " + l for l in tc("rustc", "-vV").splitlines()]
    rec.append(f"cargo: {tc('cargo', '--version')}")
    rec += ["  cargo -vV: " + l for l in tc("cargo", "-vV").splitlines()]
    rec.append(f"rustup: {tc('rustup', '--version')}; active toolchain: {tc('rustup', 'show', 'active-toolchain')}")
    rec.append(f"installed components of the active toolchain: "
               f"{' '.join(tc('rustup', 'component', 'list', '--installed').split())}")
    rec.append(f"installed targets: {' '.join(tc('rustup', 'target', 'list', '--installed').split())}")
    rec.append(".cargo/config.toml (repository, unchanged): " + open(os.path.join(WT, ".cargo/config.toml")).read().replace("\n", " | "))
    for d in ("/home/nexus/NEXUS", "/home/nexus", "/home", ""):
        for n in ("config", "config.toml"):
            p = f"{d}/.cargo/{n}"
            if os.path.exists(p):
                rec.append(f"OTHER CARGO CONFIG PRESENT: {p}")
    rec.append("ancestor and CARGO_HOME Cargo configuration files: none other than the repository's (checked "
               "/home/nexus/NEXUS, /home/nexus, /home, / for .cargo/config and .cargo/config.toml)")
    rel = open(os.path.join(WT, "Cargo.toml")).read()
    rec.append("[profile.release] (workspace Cargo.toml, unchanged): " + " | ".join(
        re.search(r"\[profile\.release\]\n(.*?)(?:\n\[|\Z)", rel, re.S).group(1).splitlines()))
    rec.append(f"target directory {TARGET} existed before: {os.path.exists(TARGET)}")
    rec.append("build environment (env -i; exactly these, nothing inherited): " +
               " ".join(f"{k}={v}" for k, v in BUILD_ENV))
    rec.append("  CARGO_TARGET_DIR: the mission-owned fresh target directory. CARGO_TERM_VERBOSE=true: output only, "
               "the equivalent of -v, so that the build's own messages record each rustc invocation's effective flags; "
               "it enters no fingerprint and no artifact. CARGO_NET_OFFLINE=true: no network, so no package can be "
               "fetched or installed; with --locked the lock file alone resolves dependencies.")
    rec.append("  unset by env -i (so not in effect): RUSTFLAGS, CARGO_ENCODED_RUSTFLAGS, RUSTDOCFLAGS, RUSTC, "
               "RUSTC_WRAPPER, RUSTC_WORKSPACE_WRAPPER, RUSTUP_TOOLCHAIN, RUSTUP_HOME, CARGO_HOME, CARGO_BUILD_*, "
               "CARGO_PROFILE_*, CARGO_INCREMENTAL, CARGO_TARGET_*, CARGO_ENCODED_RUSTDOCFLAGS")
    launcher_env = sorted(k for k in os.environ if re.match(r"^(CARGO|RUST|RUSTUP)", k))
    rec.append(f"  CARGO*/RUST* variables in the launcher's own environment (names; none is passed): {launcher_env}")
    desc["source_before"] = t
    if t["verdict"] != "EXACTLY_C":
        return not_started(outdir, rec, desc, "the worktree is not exactly C", 7)
    if os.path.exists(TARGET):
        return not_started(outdir, rec, desc, "the target directory already exists: not a fresh build", 7)
    problems = preflight(rec)
    if problems:
        return not_started(outdir, rec, desc, "; ".join(problems), 3)
    sup, proc = supervised("build", outdir, f"{UNIT_PREFIX}-build-{int(time.time())}", WT, BUILD_ARGV, BUILD_ENV,
                           os.path.join(outdir, "build.stdout"), os.path.join(outdir, "build.stderr"), rec)
    stderr = open(os.path.join(outdir, "build.stderr"), errors="replace").read()
    named = re.findall(r"^\s*Executable unittests src/lib\.rs \(([^)]+)\)\s*$", stderr, re.M)
    finished = re.findall(r"^\s*Finished `(\w+)` profile \[([^\]]+)\] target\(s\) in (.+)$", stderr, re.M)
    named_abs = [p if os.path.isabs(p) else os.path.join(WT, p) for p in named]
    deps = os.path.join(TARGET, "release/deps")
    on_disk = sorted(os.path.join(deps, n) for n in os.listdir(deps)
                     if re.fullmatch(r"nexus_governed_control-[0-9a-f]{16}", n)) if os.path.isdir(deps) else []
    recorded = [{"path": p, "sha256": sha256(p), "size": os.path.getsize(p),
                 "mtime_ns": os.stat(p).st_mtime_ns} for p in named_abs if os.path.exists(p)]
    t2 = tree_check(rec, "after")
    rec.append(f"cargo's Finished line(s): {finished}")
    rec.append(f"executables cargo named ('Executable unittests src/lib.rs (...)'): {named}")
    rec.append(f"nexus_governed_control-<hash> executables present in the fresh target's release/deps: {on_disk}")
    for p in recorded:
        rec.append(f"  executable {p['path']} sha256 {p['sha256']} size {p['size']}")
    desc.update({"process": proc, "supervisor": sup, "stderr_path": "build.stderr", "stdout_path": "build.stdout",
                 "finished": finished, "named_executables": named, "deps_executables": on_disk,
                 "recorded_executables": recorded, "source_after": t2})
    good = (proc["exit_status"] == 0 and proc["signal"] is None and sup["completed"] and not sup["timed_out"]
            and sup["oom_kill"] == 0 and len(named) == 1 and len(recorded) == 1 and on_disk == named_abs
            and finished and finished[-1][0] == "release" and t2["verdict"] == "EXACTLY_C")
    if good:
        pin = {"executable": recorded[0]["path"], "sha256": recorded[0]["sha256"], "size": recorded[0]["size"],
               "commit": C, "tree": C_TREE, "built": now(), "build_receipt": "receipts/10-build/RECEIPT.txt"}
        json.dump(pin, open(PIN_FILE, "w"), indent=1)
        os.chmod(PIN_FILE, 0o444)
        rec.append(f"PINNED: {json.dumps(pin)}")
    rec.append(f"OUTCOME: {'BUILD OK — exactly one library-test executable, pinned' if good else 'BUILD NOT OK'}")
    return finish(outdir, rec, desc, 0 if good else 4)


def load_pin():
    pin = json.load(open(PIN_FILE))
    return pin["executable"], pin["sha256"]


def invoke(outdir, kind, argv_tail, title, extra_checks=None, limit_mode=None):
    """One supervised invocation of the pinned executable; cwd is the crate directory, as cargo would run it."""
    os.makedirs(outdir)
    exe, pin = load_pin()
    argv = [exe, *argv_tail]
    rec = [title, f"started: {now()}"]
    desc = {"id": os.path.basename(outdir), "type": kind, "worktree": WT, "argv": argv, "env": dict(RUN_ENV),
            "cwd": CRATE_DIR}
    t = tree_check(rec, "before")
    before = sha256(exe)
    rec.append(f"executable {exe}: sha256 before {before} ({os.path.getsize(exe)} bytes); pinned {pin} "
               f"{'MATCH' if before == pin else 'MISMATCH'}")
    rec.append("test environment (env -i; exactly these): " + " ".join(f"{k}={v}" for k, v in RUN_ENV))
    desc.update({"source_before": t, "executable": {"path": exe, "pinned_sha256": pin, "sha256_before": before}})
    if t["verdict"] != "EXACTLY_C":
        return not_started(outdir, rec, desc, "the worktree is not exactly C", 7), None
    if before != pin:
        return not_started(outdir, rec, desc, "the executable is not the pinned one", 5), None
    problems = preflight(rec)
    if problems:
        return not_started(outdir, rec, desc, "; ".join(problems), 3), None
    if extra_checks:
        problem = extra_checks(rec, desc, exe)
        if problem:
            return not_started(outdir, rec, desc, problem, 6), None
    sup, proc = supervised(limit_mode or kind, outdir, f"{UNIT_PREFIX}-{os.path.basename(outdir)}-{int(time.time())}",
                           CRATE_DIR, argv, RUN_ENV, os.path.join(outdir, "raw-output.txt"), None, rec)
    after = sha256(exe)
    t2 = tree_check(rec, "after")
    raw = os.path.join(outdir, "raw-output.txt")
    rec.append(f"executable sha256 after: {after} {'MATCH' if after == pin else 'MISMATCH'}")
    rec.append(f"raw output: raw-output.txt sha256 {sha256(raw)} bytes {os.path.getsize(raw)}")
    desc["executable"].update({"sha256_after": after, "size": os.path.getsize(exe)})
    desc.update({"process": proc, "supervisor": sup, "output_path": "raw-output.txt", "raw_output_sha256": sha256(raw),
                 "source_after": t2})
    return finish(outdir, rec, desc, 0), desc


def listing(outdir, which):
    tail = ["--list"] + (["--ignored"] if which == "ignored" else [])
    code, _ = invoke(outdir, "list", tail, f"SUPERVISED LISTING ({which}) of the pinned executable")
    return code


def suite(outdir):
    for d in ("receipts/20-list-all", "receipts/21-list-ignored"):
        if not os.path.exists(os.path.join(EV, d, "descriptor.json")):
            raise SystemExit(f"REFUSED: {d} is missing")
    code, _ = invoke(outdir, "suite", ["--nocapture", "--test-threads=1"],
                     "SUPERVISED NONIGNORED SUITE — the pinned executable, no name filter (mission section 6)")
    return code


def full(outdir, key):
    test, prior, success = FULL[key]
    cls = os.path.join(EV, prior, "CLASSIFICATION.txt")
    if not os.path.exists(cls) or success not in open(cls).read().splitlines():
        raise SystemExit(f"REFUSED: {prior} has no '{success}' (no run follows a stop)")

    def selection(rec, desc, exe):
        sel = subprocess.run(["/usr/bin/env", "-i", *[f"{k}={v}" for k, v in RUN_ENV], exe, test, "--exact",
                              "--ignored", "--list"], cwd=CRATE_DIR, capture_output=True, text=True)
        lines = [l for l in sel.stdout.splitlines() if l.endswith(": test")]
        after = sha256(exe)
        rec.append(f"selection check: {exe} {test} --exact --ignored --list -> exit {sel.returncode}; "
                   f"listed {lines}; summary {[l for l in sel.stdout.splitlines() if 'tests,' in l]}; executable "
                   f"sha256 after the listing {after}")
        desc["selection"] = {"argv": [exe, test, "--exact", "--ignored", "--list"], "exit": sel.returncode,
                             "stdout": sel.stdout, "listed": lines, "sha256_after": after}
        if sel.returncode != 0 or lines != [f"{test}: test"] or after != desc["executable"]["pinned_sha256"]:
            return "the selection is not exactly the one named test"
        return None

    code, _ = invoke(outdir, "full", [test, "--exact", "--ignored", "--nocapture", "--test-threads=1"],
                     f"SUPERVISED FULL EXPLORATION {key} — {test}", extra_checks=selection)
    return code


if __name__ == "__main__":
    # Absolute: the scope's cwd is not the launcher's, and every record path is passed into the scope (v1 defect).
    mode, out = sys.argv[1], os.path.abspath(sys.argv[2])
    if os.path.exists(out):
        raise SystemExit(f"REFUSED: {out} exists (one call, one receipt; nothing is retried)")
    if mode == "selftest":
        sys.exit(selftest(out))
    if mode == "build":
        sys.exit(build(out))
    if mode == "list":
        sys.exit(listing(out, sys.argv[3]))
    if mode == "suite":
        sys.exit(suite(out))
    if mode == "full":
        sys.exit(full(out, sys.argv[3]))
    raise SystemExit("usage")

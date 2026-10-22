#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-C4-WITNESS-01 supervised launcher. One call = one supervised process = one receipt directory.

  supervise.py selftest <dir>                                  enforcement self-test with NON-Nexus processes only
  supervise.py build <dir> <worktree> <role>                   one optimised test build (role baseline|mutant)
  supervise.py run <dir> <worktree> <role> <exe> <sha256>      one invocation of the C4 witness test only

Enforcement (existing local mechanism of the earlier missions): a transient systemd user scope with MemoryMax=16G and
MemorySwapMax=0 (cgroup v2: charged memory, not address space), the scope's RuntimeMaxSec as a backstop, and
`timeout -s KILL` as the time limit (build 900 s, run 180 s). scope-wrap.sh, the scope's first process, records the
scope's enforced memory.max / memory.swap.max before the command and memory.peak / memory.swap.peak / memory.events
after it. /usr/bin/time -v records the command's exit status or terminating signal and its maximum resident set size
(the largest single process, not the scope total). A build has at most 2 compiler jobs (cargo -j 2).
Before starting: MemAvailable >= 16 GiB + 2 GiB margin, or NOT STARTED. No other workload is touched.
The receipt directory must not exist. Nothing is retried.
"""
import hashlib, json, os, re, shlex, subprocess, sys, threading, time

EV = "/home/nexus/nexus-audits/claude-architect/p3-c11-dr01-d2a1-c4-witness-01"
WRAP = os.path.join(EV, "tools/scope-wrap.sh")
TEST = "ledger::protocol::model_tests::c4_witness_p4_and_p8_on_f8"
MODEL = "crates/nexus-governed-control/src/ledger/protocol/model_tests.rs"
PROTOCOL = "crates/nexus-governed-control/src/ledger/protocol.rs"
NEED_KIB = (16 + 2) * 1024 * 1024
LIMITS = {"build": 900, "run": 180}
ENV_KEEP = ("HOME", "PATH", "USER", "LOGNAME", "LANG")


def sha256(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def git(wt, *args):
    return subprocess.run(["git", "-C", wt, *args], capture_output=True, text=True, check=True).stdout


def meminfo(key):
    for line in open("/proc/meminfo"):
        if line.startswith(key + ":"):
            return int(line.split()[1])


def source_identity(wt):
    porcelain = [l for l in git(wt, "status", "--porcelain").splitlines()]
    blobs = git(wt, "hash-object", PROTOCOL, MODEL).split()
    files = {}
    for rel in ("Cargo.lock", "rust-toolchain.toml", ".cargo/config.toml", "Cargo.toml",
                "crates/nexus-governed-control/Cargo.toml", PROTOCOL, MODEL):
        files[rel] = sha256(os.path.join(wt, rel))
    return {"head": git(wt, "rev-parse", "HEAD").strip(), "tree_head": git(wt, "rev-parse", "HEAD^{tree}").strip(),
            "branch": git(wt, "rev-parse", "--abbrev-ref", "HEAD").strip(), "porcelain": porcelain,
            "source": {PROTOCOL: blobs[0], MODEL: blobs[1]}, "file_sha256": files}


def preflight(rec):
    avail = meminfo("MemAvailable")
    rec.append(f"pre-flight {time.strftime('%Y-%m-%dT%H:%M:%S%z')}:")
    rec.append(f"  MemTotal {meminfo('MemTotal')} KiB; MemAvailable {avail} KiB (needed {NEED_KIB} KiB = 16 GiB cap + 2 GiB margin)")
    rec.append(f"  SwapTotal {meminfo('SwapTotal')} KiB; SwapFree {meminfo('SwapFree')} KiB (the scope has swap disabled)")
    rec.append(f"  load average {open('/proc/loadavg').read().split()[:3]}; online CPUs {os.cpu_count()}")
    top = subprocess.run(["ps", "-eo", "rss=,comm=", "--sort=-rss"], capture_output=True, text=True).stdout.splitlines()[:8]
    rec.append("  largest resident processes (comm only; none is stopped by this launcher):")
    rec += ["    " + l.strip() for l in top]
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
        out.write(f"scope cgroup: {cg}\ntime memory.current memory.peak memory.swap.current oom_kill host.MemAvailable(KiB)\n")
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
            time.sleep(2)
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
            "max_rss_kib": int(rss.group(1)) if rss else None, "wall_seconds": secs, "text": text}


def parse_scope_record(path):
    d = {}
    if os.path.exists(path):
        for line in open(path):
            k, _, v = line.rstrip("\n").partition(": ")
            if k == "memory.events":
                a, b = v.split()
                d["events." + a] = int(b)
            else:
                d[k] = v
    return d


def supervised(mode, outdir, unit, cwd, inner, env_inner, stdout_path, stderr_path, rec):
    limit = LIMITS.get(mode, 5)
    scope_rec = os.path.join(outdir, "scope-record.txt")
    time_v = os.path.join(outdir, "time-v.txt")
    cmd = ["systemd-run", "--user", "--scope", f"--unit={unit}", "-p", "MemoryMax=16G", "-p", "MemorySwapMax=0",
           "-p", f"RuntimeMaxSec={limit + 10}", "--quiet", WRAP, scope_rec,
           "/usr/bin/env", "-i", *[f"{k}={v}" for k, v in env_inner],
           "/usr/bin/time", "-v", "-o", time_v, "timeout", "-s", "KILL", str(limit), *inner]
    rec.append(f"scope unit: {unit}.scope; cwd: {cwd}")
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
    show = subprocess.run(["systemctl", "--user", "show", unit + ".scope", "-p", "LoadState", "-p", "ActiveState",
                           "-p", "Result"], capture_output=True, text=True).stdout.split()
    show = dict(s.split("=", 1) for s in show if "=" in s)
    tv, sr = parse_time_v(time_v), parse_scope_record(scope_rec)
    scope_result = "gone" if show.get("LoadState") == "not-found" else show.get("Result")
    timed_out = tv["exit_status"] in (124, 137) or (tv["wall_seconds"] or 0) >= limit
    sup = {"memory_max": sr.get("memory.max"), "swap_max": sr.get("memory.swap.max"),
           "memory_high": sr.get("memory.high"), "memory_peak_bytes": sr.get("memory.peak"),
           "swap_peak_bytes": sr.get("memory.swap.peak"), "oom": sr.get("events.oom"), "oom_kill": sr.get("events.oom_kill"),
           "completed": all(k in sr for k in ("finished", "command exit status", "events.oom_kill")), "timed_out": timed_out,
           "wall_seconds": tv["wall_seconds"], "time_limit_seconds": limit, "scope_result": scope_result,
           "scope_show": show, "launcher_elapsed_seconds": round(elapsed, 3),
           "max_rss_kib_largest_process": tv["max_rss_kib"]}
    proc = {"exit_status": tv["exit_status"], "signal": tv["signal"], "supervisor_rc": r.returncode,
            "wrapper_recorded_status": sr.get("command exit status")}
    rec.append(f"supervisor return code {r.returncode}; /usr/bin/time: exit status {tv['exit_status']}, signal {tv['signal']}, "
               f"wall {tv['wall_seconds']} s, max RSS (largest single process) {tv['max_rss_kib']} KiB")
    rec.append(f"scope (scope-wrap.sh record): memory.max {sup['memory_max']}, memory.swap.max {sup['swap_max']}, "
               f"memory.peak {sup['memory_peak_bytes']} B, memory.swap.peak {sup['swap_peak_bytes']} B, oom {sup['oom']}, "
               f"oom_kill {sup['oom_kill']}, completed {sup['completed']}; systemctl after: {show}")
    return sup, proc


def selftest(outdir):
    os.makedirs(outdir)
    rec = [f"SUPERVISION SELF-TEST {time.strftime('%Y-%m-%dT%H:%M:%S%z')} (non-Nexus processes only)",
           f"systemd: {subprocess.run(['systemctl', '--user', '--version'], capture_output=True, text=True).stdout.splitlines()[0]}",
           f"kernel: {os.uname().release}; timeout: {subprocess.run(['timeout', '--version'], capture_output=True, text=True).stdout.splitlines()[0]}"]
    ok = True
    py = "/usr/bin/python3"
    tests = [
        ("1. the 16 GiB, zero-swap scope's limits read back from inside it (a 10 MiB allocation)", "run",
         [py, "-c", "b = b'x' * (10 << 20); print('allocated', len(b))"], lambda s, p: s["memory_max"] == "17179869184"
         and s["swap_max"] == "0" and p["exit_status"] == 0 and s["completed"] and s["oom_kill"] == 0),
    ]
    for label, mode, inner, want in tests:
        d = os.path.join(outdir, label.split(".")[0])
        os.makedirs(d)
        r = [label]
        sup, proc = supervised(mode, d, f"c4w01-selftest-{int(time.time())}", outdir, inner,
                               [("PATH", "/usr/bin:/bin")], os.path.join(d, "out.txt"), None, r)
        good = want(sup, proc)
        ok &= good
        rec += r + [f"  RESULT {'OK' if good else 'FAILED'}", ""]
    # 2. memory cap enforced: a 64 MiB zero-swap scope; the process writes 256 MiB. systemd stops the whole scope on
    # the kernel's OOM kill (Result=oom-kill); the wrapper may or may not finish its record before it is stopped, and
    # a record that does finish shows the command killed (137). Either way the run is never classified complete.
    d = os.path.join(outdir, "2")
    os.makedirs(d)
    unit = f"c4w01-selftest-mem-{int(time.time())}"
    r = subprocess.run(["systemd-run", "--user", "--scope", f"--unit={unit}", "-p", "MemoryMax=64M", "-p", "MemorySwapMax=0",
                        "--quiet", WRAP, os.path.join(d, "scope-record.txt"), py, "-c",
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
    # 3. time limit enforced: timeout -s KILL 2 around a 30 s sleep. timeout signals its own process group, so it
    # is killed too: the parent sees signal 9.
    t0 = time.time()
    r = subprocess.run(["systemd-run", "--user", "--scope", f"--unit=c4w01-selftest-time-{int(time.time())}", "--quiet",
                        "timeout", "-s", "KILL", "2", "sleep", "30"], capture_output=True, text=True)
    el = time.time() - t0
    good = r.returncode in (-9, 137) and el < 10
    ok &= good
    rec += ["3. time limit enforced: timeout -s KILL 2 around a 30 s sleep", f"  rc {r.returncode}; elapsed {el:.2f} s",
            f"  RESULT {'OK' if good else 'FAILED'}", ""]
    # 4. backstop: RuntimeMaxSec=2 around a 30 s sleep.
    t0 = time.time()
    unit = f"c4w01-selftest-backstop-{int(time.time())}"
    r = subprocess.run(["systemd-run", "--user", "--scope", f"--unit={unit}", "-p", "RuntimeMaxSec=2", "--quiet", "sleep", "30"],
                       capture_output=True, text=True)
    el = time.time() - t0
    good = r.returncode != 0 and el < 10
    ok &= good
    rec += ["4. backstop enforced: RuntimeMaxSec=2 around a 30 s sleep", f"  rc {r.returncode}; elapsed {el:.2f} s",
            f"  RESULT {'OK' if good else 'FAILED'}", ""]
    subprocess.run(["systemctl", "--user", "reset-failed", unit + ".scope"], capture_output=True)
    # 5. a normal exit 101 through the whole chain (scope, wrapper, env -i, time, timeout): status 101, no signal,
    # completion recorded. 6. a process ended by a signal through the same chain: the signal is reported.
    for label, code, want in (("5. a normal exit 101 through the run chain", "import sys; sys.exit(101)",
                               lambda s, p: p["exit_status"] == 101 and p["signal"] is None and s["completed"]
                               and p["supervisor_rc"] == 101),
                              ("6. a process ended by SIGABRT through the run chain", "import os; os.abort()",
                               lambda s, p: p["signal"] == 6 and p["exit_status"] is None and p["supervisor_rc"] == 134)):
        d = os.path.join(outdir, label.split(".")[0])
        os.makedirs(d)
        r = [label]
        sup, proc = supervised("run", d, f"c4w01-selftest-{label[0]}-{int(time.time())}", outdir, [py, "-c", code],
                               [("PATH", "/usr/bin:/bin")], os.path.join(d, "out.txt"), None, r)
        good = want(sup, proc)
        ok &= good
        rec += r + [f"  process {proc}", f"  RESULT {'OK' if good else 'FAILED'}", ""]
    rec.append(f"SELF-TEST RESULT: {'ALL OK' if ok else 'FAILED'}")
    open(os.path.join(outdir, "SELFTEST.txt"), "w").write("\n".join(rec) + "\n")
    print("\n".join(rec))
    return 0 if ok else 1


def build(outdir, wt, role):
    os.makedirs(outdir)
    rec = [f"SUPERVISED BUILD ({role}) — P3-C11-DR01-D2A1-C4-WITNESS-01", f"started: {time.strftime('%Y-%m-%dT%H:%M:%S%z')}"]
    ident = source_identity(wt)
    rec.append("source identity before: " + json.dumps(ident))
    rec.append(f"compiler: {subprocess.run(['rustc', '--version'], cwd=wt, capture_output=True, text=True).stdout.strip()}")
    rec += ["  rustc -vV: " + l for l in subprocess.run(["rustc", "-vV"], cwd=wt, capture_output=True, text=True).stdout.splitlines()]
    rec.append(f"cargo: {subprocess.run(['cargo', '--version'], cwd=wt, capture_output=True, text=True).stdout.strip()}")
    rec.append(".cargo/config.toml: " + open(os.path.join(wt, ".cargo/config.toml")).read().replace("\n", " | "))
    rel = open(os.path.join(wt, "Cargo.toml")).read()
    rec.append("[profile.release]: " + " | ".join(re.search(r"\[profile\.release\]\n(.*?)(?:\n\[|\Z)", rel, re.S).group(1).splitlines()))
    target = os.path.join(wt, "target")
    rec.append(f"target directory {target} existed before: {os.path.exists(target)}")
    env_inner = [(k, os.environ[k]) for k in ENV_KEEP if k in os.environ]
    env_inner += [("CARGO_INCREMENTAL", "0"), ("CARGO_TARGET_DIR", target)]
    rec.append("build environment (env -i; only these): " + " ".join(f"{k}={v}" for k, v in env_inner))
    rec.append("inherited variables NOT passed (names only; values never recorded): " +
               " ".join(sorted(k for k in os.environ if k not in ENV_KEEP)))
    rec.append("features: the crate defines none; default features of dependencies as locked; RUSTFLAGS unset")
    if not preflight(rec):
        rec.append("OUTCOME: NOT STARTED (not enough free memory; no workload was stopped)")
        open(os.path.join(outdir, "RECEIPT.txt"), "w").write("\n".join(rec) + "\n")
        print("\n".join(rec))
        return 3
    inner = ["cargo", "test", "-p", "nexus-governed-control", "--locked", "--release", "--lib", "--no-run", "-j", "2",
             "--message-format=json-render-diagnostics"]
    sup, proc = supervised("build", outdir, f"c4w01-build-{role}-{int(time.time())}", wt, inner, env_inner,
                           os.path.join(outdir, "build.json"), os.path.join(outdir, "build.stderr"), rec)
    produced, fresh = [], []
    msgs = {"warning": 0, "error": 0}
    for line in open(os.path.join(outdir, "build.json")):
        try:
            m = json.loads(line)
        except ValueError:
            continue
        if m.get("reason") == "compiler-message":
            lvl = m.get("message", {}).get("level")
            msgs[lvl] = msgs.get(lvl, 0) + 1
        if (m.get("reason") == "compiler-artifact" and m.get("executable") and m["target"]["name"] == "nexus_governed_control"
                and m["profile"]["test"]):
            produced.append(m["executable"])
            fresh.append(m.get("fresh"))
    recorded = [{"path": p, "sha256": sha256(p), "size": os.path.getsize(p)} for p in produced if os.path.exists(p)]
    after = source_identity(wt)
    rec.append(f"compiler messages: {msgs}")
    rec.append(f"produced test executables: {produced}; fresh flags {fresh}")
    for p in recorded:
        rec.append(f"  executable {p['path']} sha256 {p['sha256']} size {p['size']}")
    rec.append("source identity after: " + json.dumps(after))
    rec.append(f"source unchanged by the build: {after == ident}")
    stderr = open(os.path.join(outdir, "build.stderr"), errors="replace").read()
    rec.append("build stderr, last lines: " + " | ".join(stderr.strip().splitlines()[-3:]))
    desc = {"id": os.path.basename(outdir), "type": "build", "role": role, "test": None, "worktree": wt,
            "argv": inner, "jobs": 2, **ident, "process": proc, "supervisor": sup, "stderr_path": "build.stderr",
            "produced_executables": produced, "recorded_executables": recorded, "compiler_messages": msgs,
            "source_unchanged": after == ident}
    json.dump(desc, open(os.path.join(outdir, "descriptor.json"), "w"), indent=1)
    rec.append(f"finished: {time.strftime('%Y-%m-%dT%H:%M:%S%z')}")
    open(os.path.join(outdir, "RECEIPT.txt"), "w").write("\n".join(rec) + "\n")
    print("\n".join(rec[-12:]))
    return 0 if proc["exit_status"] == 0 and len(recorded) == 1 and after == ident else 4


def run(outdir, wt, role, exe, want_sha):
    os.makedirs(outdir)
    rec = [f"SUPERVISED WITNESS INVOCATION ({role}) — P3-C11-DR01-D2A1-C4-WITNESS-01",
           f"started: {time.strftime('%Y-%m-%dT%H:%M:%S%z')}"]
    ident = source_identity(wt)
    rec.append("source identity: " + json.dumps(ident))
    before = sha256(exe)
    rec.append(f"executable {exe}: sha256 before {before} ({os.path.getsize(exe)} bytes); pinned {want_sha}")
    if before != want_sha:
        rec.append("OUTCOME: NOT STARTED (the executable is not the pinned one)")
        open(os.path.join(outdir, "RECEIPT.txt"), "w").write("\n".join(rec) + "\n")
        print("\n".join(rec))
        return 5
    if not preflight(rec):
        rec.append("OUTCOME: NOT STARTED (not enough free memory; no workload was stopped)")
        open(os.path.join(outdir, "RECEIPT.txt"), "w").write("\n".join(rec) + "\n")
        print("\n".join(rec))
        return 3
    argv = [exe, TEST, "--exact", "--nocapture", "--test-threads=1"]
    env_inner = [("PATH", "/usr/bin:/bin"), ("LANG", "C.UTF-8"), ("RUST_BACKTRACE", "0")]
    rec.append("test environment (env -i; only these): " + " ".join(f"{k}={v}" for k, v in env_inner))
    rec.append("selection: exactly one test by --exact; verified from the harness's own output (no separate listing "
               "invocation is made)")
    cwd = os.path.join(wt, "crates/nexus-governed-control")
    sup, proc = supervised("run", outdir, f"c4w01-run-{role}-{int(time.time())}", cwd, argv, env_inner,
                           os.path.join(outdir, "raw-output.txt"), None, rec)
    after = sha256(exe)
    rec.append(f"executable sha256 after: {after} {'MATCH' if after == before else 'MISMATCH'}")
    raw = os.path.join(outdir, "raw-output.txt")
    rec.append(f"raw output: {raw} sha256 {sha256(raw)} bytes {os.path.getsize(raw)}")
    desc = {"id": os.path.basename(outdir), "type": "c4-healthy-witness" if role == "baseline" else "c4-mutant-witness",
            "role": role, "test": TEST, "worktree": wt, "argv": argv, "env": dict(env_inner), **ident,
            "executable": {"path": exe, "sha256_before": before, "sha256_after": after, "size": os.path.getsize(exe)},
            "process": proc, "supervisor": sup, "output_path": "raw-output.txt", "raw_output_sha256": sha256(raw)}
    json.dump(desc, open(os.path.join(outdir, "descriptor.json"), "w"), indent=1)
    rec.append(f"finished: {time.strftime('%Y-%m-%dT%H:%M:%S%z')}")
    open(os.path.join(outdir, "RECEIPT.txt"), "w").write("\n".join(rec) + "\n")
    print("\n".join(rec[-6:]))
    return 0


if __name__ == "__main__":
    mode = sys.argv[1]
    if os.path.exists(sys.argv[2]):
        raise SystemExit(f"REFUSED: {sys.argv[2]} exists (one call, one receipt; nothing is retried)")
    if mode == "selftest":
        sys.exit(selftest(sys.argv[2]))
    if mode == "build":
        sys.exit(build(sys.argv[2], sys.argv[3], sys.argv[4]))
    if mode == "run":
        sys.exit(run(sys.argv[2], sys.argv[3], sys.argv[4], sys.argv[5], sys.argv[6]))
    raise SystemExit("usage")

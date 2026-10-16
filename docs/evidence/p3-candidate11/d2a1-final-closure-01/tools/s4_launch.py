#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-FINAL-CLOSURE-01 Stage 4 supervised launcher. One call = one receipt directory. Nothing retries.

  s4_launch.py selftest <dir>   enforcement self-test with non-Nexus processes only
  s4_launch.py worktree <dir>   create the mission-owned detached Candidate C worktree; run the workflow's own
                                checkout-verification and toolchain/system-library verification commands
  s4_launch.py clippy <dir>     ONE supervised `cargo clippy --workspace --all-targets --all-features --locked --
                                -D warnings` (the exact Fast Local step), only after selftest and worktree PASS

Enforcement chain (the VALIDATION-02 design; scope-wrap.sh reused unchanged): a transient systemd user scope with
MemoryMax=16G and MemorySwapMax=0 (cgroup v2) and RuntimeMaxSec=limit+10 as a backstop; `timeout -s KILL <limit>`;
`nice -n 10`; /usr/bin/time -v; a 2 s memory sampler; a 2 s disk monitor of the root filesystem. Disk: at 17 GiB
available a CONTROLLED STOP (SIGTERM, SIGKILL after 15 s) of this mission's scope only; below the 16 GiB floor SIGKILL
at once; so at least 16 GiB stays available. No other process is ever signalled. Nothing is cleaned up.
"""
import json
import os
import re
import shlex
import subprocess
import sys
import threading
import time

TOOLS = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(TOOLS)
REC = os.path.join(ROOT, "receipts")
REPO = "/home/nexus/NEXUS/nexus-os"
WT = "/home/nexus/NEXUS/nexus-os-p3-c11-d2a1-final-closure-01"
TARGET = os.path.join(WT, "target")
C = "d5964ee5326c2733177307db6f507328023d361a"
C_TREE = "51bac5fd335999cc995c0ab9f582a30244973ef0"
WRAP = os.path.join(TOOLS, "scope-wrap.sh")
TOOLCHAIN = "/home/nexus/.rustup/toolchains/1.94.0-x86_64-unknown-linux-gnu/bin"
RUNNER = "actions.runner.Nexus-os2026-nexus-os.nexus-local-asus.service"
GIB = 1 << 30
DISK_STOP = 17 * GIB
DISK_FLOOR = 16 * GIB
MEM_NEED_KIB = 18 * 1024 * 1024
UNIT_PREFIX = "p3c11-fc01"
LIMIT_CLIPPY = 2700
PROJECTION_CLIPPY = 4 * GIB  # VALIDATION-02's clippy without --all-features consumed 2.28 GB; this is a ceiling estimate
DECLARED_GENERATED = {"!! app/src-tauri/permissions/"}
# env -i, exactly these. The toolchain directory first (no rustup proxy is resolved for cargo, rustc or cargo-clippy;
# ~/.cargo/bin is listed after it only so that cargo does not insert it ahead of PATH for subcommand lookup).
# CARGO_BUILD_JOBS=2 overrides .cargo/config.toml [build] jobs = 8 (mission limit).
BUILD_ENV = [
    ("HOME", "/home/nexus"), ("USER", "nexus"), ("LOGNAME", "nexus"),
    ("PATH", f"{TOOLCHAIN}:/home/nexus/.cargo/bin:/usr/bin:/bin"), ("LANG", "C.UTF-8"),
    ("CARGO_TARGET_DIR", TARGET), ("CARGO_BUILD_JOBS", "2"), ("CARGO_INCREMENTAL", "0"),
    ("CARGO_NET_OFFLINE", "true"), ("CARGO_TERM_COLOR", "never"),
]
CLIPPY_ARGV = ["cargo", "clippy", "--workspace", "--all-targets", "--all-features", "--locked", "--", "-D", "warnings"]


def now():
    return time.strftime("%Y-%m-%dT%H:%M:%S%z")


def run(argv, **kw):
    return subprocess.run(argv, capture_output=True, text=True, **kw)


def meminfo(key):
    for line in open("/proc/meminfo"):
        if line.startswith(key + ":"):
            return int(line.split()[1])


def disk():
    s = os.statvfs("/")
    return {"avail_bytes": s.f_bavail * s.f_frsize, "used_bytes": (s.f_blocks - s.f_bfree) * s.f_frsize}


def gib(b):
    return f"{b / GIB:.3f} GiB"


def du_bytes(path):
    if not os.path.exists(path):
        return 0
    r = run(["du", "-sx", "--block-size=1", path])
    try:
        return int(r.stdout.split()[0])
    except (IndexError, ValueError):
        return None


def write_exclusive(path, text):
    with open(path, "x") as f:
        f.write(text)


def runner_state():
    r = run(["systemctl", "show", RUNNER, "-p", "ActiveState", "-p", "MainPID", "-p", "UnitFileState"])
    show = dict(l.split("=", 1) for l in r.stdout.splitlines() if "=" in l)
    return show.get("ActiveState") == "inactive" and show.get("MainPID") == "0", show


def conflicts():
    found = []
    for pid in os.listdir("/proc"):
        if not pid.isdigit() or int(pid) == os.getpid():
            continue
        try:
            argv = open(f"/proc/{pid}/cmdline", "rb").read().split(b"\0")
        except OSError:
            continue
        a0 = argv[0].decode(errors="replace") if argv and argv[0] else ""
        base = os.path.basename(a0)
        if base in ("cargo", "rustc", "clippy-driver", "cargo-clippy", "rustdoc", "Runner.Worker", "Runner.Listener") or \
                re.search(r"/deps/nexus_governed_control-[0-9a-f]{16}$", a0):
            found.append((pid, b" ".join(argv).decode(errors="replace")[:240]))
    scopes = [s for s in run(["systemctl", "--user", "list-units", "--type=scope", "--state=active", "--no-legend",
                              "--plain", UNIT_PREFIX + "*"]).stdout.splitlines() if s.strip()]
    return found, scopes


def tree_check(allow_generated):
    r = run([sys.executable, os.path.join(TOOLS, "verify_tree.py"), WT])
    d = json.loads(r.stdout)
    tracked_exact = (d["head"] == C and d["tree"] == C_TREE and d["detached"] and not d["index_differs_from_head"]
                     and not d["content_mismatches"] and not d["missing"] and not d["mode_mismatches"])
    extras = d["unexpected_status"]
    if tracked_exact and not extras:
        v = "EXACTLY_C"
    elif tracked_exact and allow_generated and all(e in DECLARED_GENERATED for e in extras):
        v = "SOURCE_EXACT_C_WITH_DECLARED_GENERATED"
    else:
        v = "NOT_C"
    d["tracked_exact"], d["classified_verdict"] = tracked_exact, v
    return d


def tree_line(d, when):
    return (f"source check ({when}): {d['classified_verdict']}; HEAD {d['head']}; tree {d['tree']}; detached "
            f"{d['detached']}; {d['tracked_blobs_compared']} tracked blobs, {d['bytes_compared']} bytes compared; "
            f"status {d['status_entries']}; mismatches {d['content_mismatches']}; missing {d['missing']}; modes "
            f"{d['mode_mismatches']}")


# ------------------------------------------------------------------------------------------------ monitoring
def mem_sampler(unit, path, stop):
    with open(path, "w") as out:
        cg = None
        for _ in range(400):
            r = run(["systemctl", "--user", "show", "-p", "ControlGroup", "--value", unit + ".scope"])
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
        out.write(f"scope cgroup: {cg}\ntime memory.current memory.peak memory.swap.current oom_kill pids.current "
                  f"host.MemAvailable(KiB)\n")
        while os.path.isdir(cg) and not stop.is_set():
            try:
                cur = open(cg + "/memory.current").read().strip()
                peak = open(cg + "/memory.peak").read().strip()
                swp = open(cg + "/memory.swap.current").read().strip()
                ev = dict(l.split() for l in open(cg + "/memory.events"))
                pids = open(cg + "/pids.current").read().strip() if os.path.exists(cg + "/pids.current") else "-"
            except OSError:
                break
            out.write(f"{time.strftime('%H:%M:%S')} {cur} {peak} {swp} {ev.get('oom_kill')} {pids} "
                      f"{meminfo('MemAvailable')}\n")
            out.flush()
            stop.wait(2)
        out.write(f"sampler: stopped at {time.strftime('%H:%M:%S')}\n")


class DiskMonitor(threading.Thread):
    def __init__(self, unit, path, target, stop_at, floor):
        super().__init__(daemon=True)
        self.unit, self.path, self.target, self.stop_at, self.floor = unit, path, target, stop_at, floor
        self.halt = threading.Event()
        self.events, self.triggered, self.min_avail, self.max_target, self.samples = [], None, None, 0, 0
        self.start_disk = disk()
        self.start_target = du_bytes(target) if target else 0

    def kill(self, sig):
        r = run(["systemctl", "--user", "kill", f"--signal={sig}", self.unit + ".scope"])
        self.events.append(f"{now()} systemctl --user kill --signal={sig} {self.unit}.scope -> rc {r.returncode} "
                           f"{r.stderr.strip()}")
        return r.returncode == 0

    def active(self):
        return run(["systemctl", "--user", "is-active", self.unit + ".scope"]).stdout.strip() in (
            "active", "activating", "deactivating")

    def stop_scope(self, first_sig, out):
        seen, accepted, escalated, t_sig = False, False, first_sig == "SIGKILL", None
        while not self.halt.is_set():
            if self.active():
                seen = True
                if not accepted:
                    accepted = self.kill(first_sig)
                    t_sig = time.time() if accepted else None
                elif not escalated and (time.time() - t_sig > 15 or disk()["avail_bytes"] < self.floor):
                    out.write(f"{now()} scope still active after SIGTERM (15 s or floor) -> SIGKILL\n")
                    escalated = self.kill("SIGKILL")
            elif seen:
                break
            time.sleep(0.25)
        self.events.append(f"{now()} stop delivery ended: scope seen {seen}, signal accepted {accepted}, escalated "
                           f"{escalated and first_sig != 'SIGKILL'}, launcher returned {self.halt.is_set()}")

    def run(self):
        last_du, tgt = 0.0, self.start_target
        with open(self.path, "w") as out:
            out.write(f"disk monitor: root filesystem; stop trigger {self.stop_at} B ({gib(self.stop_at)}), floor "
                      f"{self.floor} B ({gib(self.floor)}); target {self.target} start {self.start_target} B; start avail "
                      f"{self.start_disk['avail_bytes']} B\n")
            out.write("time avail_bytes used_delta_bytes target_bytes\n")
            while not self.halt.is_set():
                d = disk()
                if self.target and time.time() - last_du >= 15:
                    v = du_bytes(self.target)
                    tgt = v if v is not None else tgt
                    last_du = time.time()
                self.samples += 1
                self.min_avail = d["avail_bytes"] if self.min_avail is None else min(self.min_avail, d["avail_bytes"])
                self.max_target = max(self.max_target, tgt or 0)
                out.write(f"{time.strftime('%H:%M:%S')} {d['avail_bytes']} {d['used_bytes'] - self.start_disk['used_bytes']} {tgt}\n")
                if d["avail_bytes"] < self.floor and self.triggered != "floor":
                    self.triggered = "floor"
                    out.write(f"{now()} BELOW FLOOR: avail {d['avail_bytes']} B < {self.floor} B -> SIGKILL\n")
                    out.flush()
                    self.stop_scope("SIGKILL", out)
                elif d["avail_bytes"] <= self.stop_at and self.triggered is None:
                    self.triggered = "stop"
                    out.write(f"{now()} CONTROLLED STOP: avail {d['avail_bytes']} B <= {self.stop_at} B -> SIGTERM\n")
                    out.flush()
                    self.stop_scope("SIGTERM", out)
                out.flush()
                self.halt.wait(2)
            d = disk()
            tgt = du_bytes(self.target) if self.target else 0
            self.end_disk, self.end_target = d, tgt
            self.min_avail = d["avail_bytes"] if self.min_avail is None else min(self.min_avail, d["avail_bytes"])
            self.max_target = max(self.max_target, tgt or 0)
            out.write(f"disk monitor: stopped {now()}; end avail {d['avail_bytes']} B; end target {tgt} B; samples "
                      f"{self.samples}; min avail {self.min_avail} B; triggered {self.triggered}\n")

    def summary(self):
        return {"start_avail_bytes": self.start_disk["avail_bytes"], "end_avail_bytes": self.end_disk["avail_bytes"],
                "min_avail_bytes": self.min_avail, "start_target_bytes": self.start_target,
                "end_target_bytes": self.end_target, "max_target_bytes": self.max_target,
                "consumed_bytes": self.start_disk["avail_bytes"] - self.end_disk["avail_bytes"],
                "triggered": self.triggered, "samples": self.samples, "events": self.events,
                "stop_trigger_bytes": self.stop_at, "floor_bytes": self.floor}


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


def supervised(outdir, unit, cwd, inner, env_inner, out_path, rec, limit, target=None, stop_at=DISK_STOP,
               floor=DISK_FLOOR, memory_max="16G"):
    scope_rec = os.path.join(outdir, "scope-record.txt")
    time_v = os.path.join(outdir, "time-v.txt")
    cmd = ["systemd-run", "--user", "--scope", f"--unit={unit}", "-p", f"MemoryMax={memory_max}", "-p",
           "MemorySwapMax=0", "-p", f"RuntimeMaxSec={limit + 10}", "--quiet", WRAP, scope_rec,
           "/usr/bin/env", "-i", *[f"{k}={v}" for k, v in env_inner],
           "/usr/bin/time", "-v", "-o", time_v, "timeout", "-s", "KILL", str(limit), "nice", "-n", "10", *inner]
    rec.append(f"scope unit: {unit}.scope; cwd: {cwd}")
    rec.append("command: " + " ".join(shlex.quote(c) for c in cmd))
    stop = threading.Event()
    th = threading.Thread(target=mem_sampler, args=(unit, os.path.join(outdir, "cgroup-samples.txt"), stop), daemon=True)
    dm = DiskMonitor(unit, os.path.join(outdir, "disk-samples.txt"), target, stop_at, floor)
    th.start()
    dm.start()
    t0 = time.time()
    rec.append(f"launched: {now()}")
    with open(out_path, "wb") as so:
        r = subprocess.run(cmd, cwd=cwd, stdout=so, stderr=subprocess.STDOUT)
    elapsed = time.time() - t0
    rec.append(f"returned: {now()}")
    stop.set()
    dm.halt.set()
    th.join(timeout=10)
    dm.join(timeout=60)
    show = run(["systemctl", "--user", "show", unit + ".scope", "-p", "LoadState", "-p", "ActiveState", "-p",
                "Result"]).stdout.split()
    show = dict(s.split("=", 1) for s in show if "=" in s)
    tv, sr = parse_time_v(time_v), parse_scope_record(scope_rec)
    timed_out = tv["exit_status"] in (124, 137) or (tv["wall_seconds"] or 0) >= limit
    sup = {"memory_max": sr.get("memory.max"), "swap_max": sr.get("memory.swap.max"),
           "memory_high": sr.get("memory.high"), "memory_peak_bytes": sr.get("memory.peak"),
           "swap_peak_bytes": sr.get("memory.swap.peak"), "oom": sr.get("events.oom"),
           "oom_kill": sr.get("events.oom_kill"), "max_events": sr.get("events.max"),
           "completed": all(k in sr for k in ("finished", "command exit status", "events.oom_kill")),
           "timed_out": timed_out, "wall_seconds": tv["wall_seconds"], "time_limit_seconds": limit,
           "backstop_runtime_max_seconds": limit + 10, "scope_show": show,
           "launcher_elapsed_seconds": round(elapsed, 3), "max_rss_kib_largest_process": tv["max_rss_kib"],
           "disk": dm.summary()}
    proc = {"exit_status": tv["exit_status"], "signal": tv["signal"], "supervisor_rc": r.returncode,
            "wrapper_recorded_status": sr.get("command exit status")}
    rec.append(f"supervisor rc {r.returncode}; /usr/bin/time: exit status {tv['exit_status']}, signal {tv['signal']}, "
               f"wall {tv['wall_seconds']} s of {limit} s, max RSS (largest single process) {tv['max_rss_kib']} KiB")
    rec.append(f"scope: memory.max {sup['memory_max']}, memory.swap.max {sup['swap_max']}, memory.peak "
               f"{sup['memory_peak_bytes']} B, memory.swap.peak {sup['swap_peak_bytes']} B, oom {sup['oom']}, oom_kill "
               f"{sup['oom_kill']}, max events {sup['max_events']}, completed {sup['completed']}, timed out {timed_out}; "
               f"systemctl after: {show}")
    ds = sup["disk"]
    rec.append(f"disk: start avail {ds['start_avail_bytes']} B ({gib(ds['start_avail_bytes'])}), end avail "
               f"{ds['end_avail_bytes']} B, min avail {ds['min_avail_bytes']} B ({gib(ds['min_avail_bytes'])}), consumed "
               f"{ds['consumed_bytes']} B ({gib(ds['consumed_bytes'])}); target start {ds['start_target_bytes']} end "
               f"{ds['end_target_bytes']} peak {ds['max_target_bytes']} B; controlled stop {ds['triggered']}; samples "
               f"{ds['samples']}; events {ds['events']}")
    return sup, proc


def finish(outdir, rec, desc, classification, checks):
    rec.append(f"finished: {now()}")
    json.dump(desc, open(os.path.join(outdir, "descriptor.json"), "w"), indent=1)
    write_exclusive(os.path.join(outdir, "CHECKS.txt"), "\n".join(
        f"{'OK  ' if ok else 'FAIL'} {label}: {detail}" for label, ok, detail in checks) + "\n")
    write_exclusive(os.path.join(outdir, "CLASSIFICATION.txt"), f"CLASSIFICATION: {classification}\n"
                    f"checks: {sum(ok for _, ok, _ in checks)} of {len(checks)} hold\n")
    write_exclusive(os.path.join(outdir, "RECEIPT.txt"), "\n".join(rec) + "\n")
    print("\n".join(rec))
    print(f"CLASSIFICATION: {classification}")
    return 0 if classification == "PASS" else 1


def classification_of(sup, proc, checks):
    if sup is None:
        return "NOT_STARTED"
    if sup["disk"]["triggered"]:
        return "STOPPED_DISK"
    if sup["timed_out"] or not sup["completed"] or sup["oom_kill"] not in (0, None) or proc["signal"] is not None:
        return "INCOMPLETE"
    return "PASS" if all(ok for _, ok, _ in checks) else "FAILED"


def stage_ok(name):
    p = os.path.join(REC, name, "CLASSIFICATION.txt")
    return os.path.exists(p) and open(p).readline().strip() == "CLASSIFICATION: PASS"


# ---------------------------------------------------------------------------------------------------- selftest
def selftest(outdir):
    os.makedirs(outdir)
    rec = [f"SUPERVISION SELF-TEST {now()} (non-Nexus processes only)",
           f"systemd: {run(['systemctl', '--user', '--version']).stdout.splitlines()[0]}",
           f"kernel: {os.uname().release}"]
    checks = []
    py = "/usr/bin/python3"
    unit = lambda tag: f"{UNIT_PREFIX}-selftest-{tag}-{int(time.time())}"
    sub = lambda n: (os.makedirs(os.path.join(outdir, str(n))) or os.path.join(outdir, str(n)))
    # 1 limits read back + exact env through env -i
    d = sub(1)
    r = ["1. limits read back inside the 16 GiB zero-swap scope; the exact build environment through env -i"]
    sup, proc = supervised(d, unit("1"), outdir, ["/usr/bin/env"], BUILD_ENV, os.path.join(d, "out.txt"), r, 60)
    seen = sorted(l for l in open(os.path.join(d, "out.txt")).read().splitlines() if l)
    good = (sup["memory_max"] == "17179869184" and sup["swap_max"] == "0" and proc["exit_status"] == 0
            and sup["completed"] and sup["oom_kill"] == 0 and seen == sorted(f"{k}={v}" for k, v in BUILD_ENV)
            and sup["disk"]["triggered"] is None)
    checks.append(("1 limits and environment", good, f"{sup['memory_max']} {sup['swap_max']}"))
    rec += r + [f"  RESULT {'OK' if good else 'FAILED'}", ""]
    # 2 memory cap enforced (64 MiB scope, 256 MiB writer)
    d = sub(2)
    u = unit("mem")
    rr = run(["systemd-run", "--user", "--scope", f"--unit={u}", "-p", "MemoryMax=64M", "-p", "MemorySwapMax=0",
              "--quiet", WRAP, os.path.join(d, "scope-record.txt"), py, "-c",
              "b = b'x' * (256 << 20); print('NOT KILLED: wrote', len(b))"])
    show = run(["systemctl", "--user", "show", u + ".scope", "-p", "Result"]).stdout.split()
    sr = parse_scope_record(os.path.join(d, "scope-record.txt"))
    good = (rr.returncode != 0 and "NOT KILLED" not in rr.stdout and "Result=oom-kill" in show
            and sr.get("memory.max") == "67108864" and sr.get("memory.swap.max") == "0")
    checks.append(("2 memory cap enforced", good, f"rc {rr.returncode}; {show}"))
    rec += ["2. memory cap enforced: 64 MiB zero-swap scope, 256 MiB writer", f"  rc {rr.returncode}; {show}; {sr}",
            f"  RESULT {'OK' if good else 'FAILED'}", ""]
    run(["systemctl", "--user", "reset-failed", u + ".scope"])
    # 3 wall-time limit through the full chain
    d = sub(3)
    r = ["3. time limit through the full chain: timeout -s KILL 3 around a 30 s sleep"]
    sup, proc = supervised(d, unit("3"), outdir, ["/bin/sleep", "30"], [("PATH", "/usr/bin:/bin")],
                           os.path.join(d, "out.txt"), r, 3)
    good = sup["timed_out"] and (proc["signal"] == 9 or proc["exit_status"] == 137) and (sup["wall_seconds"] or 99) < 10
    checks.append(("3 time limit", good, f"{proc}"))
    rec += r + [f"  RESULT {'OK' if good else 'FAILED'}", ""]
    # 4 backstop RuntimeMaxSec
    t0 = time.time()
    u = unit("backstop")
    rr = run(["systemd-run", "--user", "--scope", f"--unit={u}", "-p", "RuntimeMaxSec=2", "--quiet", "sleep", "30"])
    el = time.time() - t0
    good = rr.returncode != 0 and el < 10
    checks.append(("4 RuntimeMaxSec backstop", good, f"rc {rr.returncode} {el:.2f}s"))
    rec += ["4. backstop: RuntimeMaxSec=2 around a 30 s sleep", f"  rc {rr.returncode}; {el:.2f} s",
            f"  RESULT {'OK' if good else 'FAILED'}", ""]
    run(["systemctl", "--user", "reset-failed", u + ".scope"])
    # 5 normal exit 101 passes through
    d = sub(5)
    r = ["5. a normal exit 101 through the run chain"]
    sup, proc = supervised(d, unit("5"), outdir, [py, "-c", "import sys; sys.exit(101)"], [("PATH", "/usr/bin:/bin")],
                           os.path.join(d, "out.txt"), r, 60)
    good = proc["exit_status"] == 101 and proc["signal"] is None and sup["completed"]
    checks.append(("5 exit 101", good, f"{proc}"))
    rec += r + [f"  RESULT {'OK' if good else 'FAILED'}", ""]
    # 6 disk guard: a trigger set above the current availability must stop this scope (SIGTERM) and nothing else
    d = sub(6)
    r = ["6. disk guard: the controlled-stop trigger set above the current availability stops this scope only"]
    cur = disk()["avail_bytes"]
    sup, proc = supervised(d, unit("6"), outdir, ["/bin/sleep", "40"], [("PATH", "/usr/bin:/bin")],
                           os.path.join(d, "out.txt"), r, 60, stop_at=cur + GIB, floor=0)
    # SIGTERM to the scope ends scope-wrap.sh and /usr/bin/time too, so no time -v record exists; the evidence is the
    # monitor's trigger, the signal accepted for this scope, the launcher's early return and the scope being gone
    good = (sup["disk"]["triggered"] == "stop" and proc["supervisor_rc"] in (-15, -9, 143, 137)
            and sup["launcher_elapsed_seconds"] < 30 and sup["scope_show"].get("ActiveState") == "inactive"
            and any("--signal=SIGTERM" in e and "rc 0" in e for e in sup["disk"]["events"]))
    checks.append(("6 disk controlled stop", good, f"{sup['disk']['triggered']} {proc}"))
    rec += r + [f"  RESULT {'OK' if good else 'FAILED'}", ""]
    cls = "PASS" if all(ok for _, ok, _ in checks) else "FAILED"
    return finish(outdir, rec, {"stage": "selftest", "checks": checks}, cls, checks)


# ---------------------------------------------------------------------------------------------------- worktree
def worktree(outdir):
    os.makedirs(outdir)
    rec = [f"MISSION-OWNED DETACHED CANDIDATE C WORKTREE {now()}"]
    checks = []
    add = lambda l, ok, d="": checks.append((l, bool(ok), d))
    add("pre: supervision self-test PASS", stage_ok("41b-supervision-selftest-r1"))
    add("pre: worktree path absent", not os.path.exists(WT), WT)
    if not all(ok for _, ok, _ in checks):
        return finish(outdir, rec, {"stage": "worktree"}, "NOT_STARTED", checks)
    r = run(["git", "-C", REPO, "worktree", "add", "--detach", WT, C])
    rec += [f"$ git -C {REPO} worktree add --detach {WT} {C}", f"  rc {r.returncode}", f"  stdout {r.stdout.strip()!r}",
            f"  stderr {r.stderr.strip()!r}"]
    add("git worktree add rc 0", r.returncode == 0, r.stderr.strip())
    d = tree_check(False)
    rec.append(tree_line(d, "after checkout"))
    add("byte-exact Candidate C after checkout (EXACTLY_C)", d["classified_verdict"] == "EXACTLY_C")
    # the workflow's own step 'Verify the tested commit and a clean checkout' (GITHUB_SHA := C)
    tested = run(["git", "-C", WT, "rev-parse", "HEAD"]).stdout.strip()
    st = run(["git", "-C", WT, "status", "--porcelain=v1", "--untracked-files=all", "--ignored"]).stdout
    rec.append(f"workflow step 'Verify the tested commit and a clean checkout': tested {tested}; status output "
               f"{len(st)} bytes {st[:200]!r}")
    add("workflow: tested commit equals the commit under test (C)", tested == C, tested)
    add("workflow: `git status --porcelain=v1 --untracked-files=all --ignored` is empty", st == "", st[:200])
    # the workflow's 'Use the runner's installed Rust 1.94.0' and 'Verify the toolchain and the Tauri system libraries'
    for tool in ("cargo", "rustc", "rustdoc", "cargo-clippy", "clippy-driver", "cargo-fmt", "rustfmt"):
        add(f"workflow: {TOOLCHAIN}/{tool} is executable", os.access(f"{TOOLCHAIN}/{tool}", os.X_OK))
    env = dict(BUILD_ENV)
    vr = run(["rustc", "--version"], cwd=WT, env=env)
    rec.append(f"$ rustc --version -> {vr.stdout.strip()}")
    add("workflow: rustc --version matches ^rustc 1\\.94\\.0 ", re.match(r"^rustc 1\.94\.0 ", vr.stdout) is not None, vr.stdout.strip())
    for argv in (["cargo", "--version"], ["cargo", "clippy", "--version"], ["cargo", "fmt", "--version"]):
        x = run(argv, cwd=WT, env=env)
        rec.append(f"$ {' '.join(argv)} -> rc {x.returncode} {x.stdout.strip()} {x.stderr.strip()}")
        add(f"workflow: {' '.join(argv)} rc 0", x.returncode == 0, x.stdout.strip())
    ta = run(["bash", "-c", "for t in cargo-clippy clippy-driver rustc cargo rustfmt; do echo \"$t $(type -P $t)\"; done"],
             cwd=WT, env=env)
    rec.append("first PATH match under the build environment:\n  " + ta.stdout.strip().replace("\n", "\n  "))
    add("build PATH resolves cargo-clippy, clippy-driver, rustc, cargo and rustfmt to the 1.94.0 toolchain directory",
        all(l.split(" ", 1)[1].startswith(TOOLCHAIN + "/") for l in ta.stdout.strip().splitlines()) and
        len(ta.stdout.strip().splitlines()) == 5, ta.stdout.strip())
    for lib in ("webkit2gtk-4.1", "javascriptcoregtk-4.1", "libsoup-3.0", "gtk+-3.0", "glib-2.0"):
        x = run(["pkg-config", "--modversion", lib], env=env)
        e = run(["pkg-config", "--exists", lib], env=env)
        rec.append(f"$ pkg-config --exists {lib} -> rc {e.returncode}; --modversion {x.stdout.strip()}")
        add(f"workflow: pkg-config --exists {lib}", e.returncode == 0, x.stdout.strip())
    d2 = tree_check(False)
    rec.append(tree_line(d2, "after the verification commands"))
    add("byte-exact Candidate C after the verification commands", d2["classified_verdict"] == "EXACTLY_C")
    cls = "PASS" if all(ok for _, ok, _ in checks) else "FAILED"
    return finish(outdir, rec, {"stage": "worktree", "tree_after_checkout": d, "tree_after": d2}, cls, checks)


# ---------------------------------------------------------------------------------------------------- clippy
def clippy(outdir):
    os.makedirs(outdir)
    rec = [f"STAGE 4 — ONE SUPERVISED `{' '.join(CLIPPY_ARGV)}` {now()}"]
    checks = []
    add = lambda l, ok, d="": checks.append((l, bool(ok), d))
    add("pre: supervision self-test PASS", stage_ok("41b-supervision-selftest-r1"))
    add("pre: worktree stage PASS", stage_ok("42-worktree"))
    earlier = [n for n in os.listdir(REC) if n.startswith("43") and os.path.isdir(os.path.join(REC, n))
               and n != os.path.basename(outdir)]
    started = [n for n in earlier if os.path.exists(os.path.join(REC, n, "raw-output.txt")) or
               not open(os.path.join(REC, n, "CLASSIFICATION.txt")).readline().startswith("CLASSIFICATION: NOT_STARTED")]
    add("pre: no earlier clippy invocation started (one invocation only; earlier NOT_STARTED refusals preserved)",
        not started, f"earlier {earlier}; started {started}")
    rok, show = runner_state()
    add("pre: GitHub runner inactive, MainPID=0", rok, show)
    found, scopes = conflicts()
    add("pre: no cargo/rustc/clippy/rustdoc/runner/governed-control process; no active mission scope",
        not found and not scopes, f"{found} {scopes}")
    av = meminfo("MemAvailable")
    add("pre: MemAvailable >= 18 GiB", av >= MEM_NEED_KIB, f"{av} KiB")
    dk = disk()
    add("pre: projected consumption (+15%) leaves availability above the 17 GiB stop trigger",
        dk["avail_bytes"] - PROJECTION_CLIPPY * 1.15 > DISK_STOP, f"avail {gib(dk['avail_bytes'])}")
    d = tree_check(False)
    rec.append(tree_line(d, "before"))
    add("pre: worktree byte-exact Candidate C (EXACTLY_C)", d["classified_verdict"] == "EXACTLY_C")
    add("pre: mission target absent (fresh build)", not os.path.exists(TARGET), TARGET)
    rec.append(f"pre-stage gate {now()}: load {open('/proc/loadavg').read().split()[:3]}; MemAvailable {av} KiB; "
               f"SwapFree {meminfo('SwapFree')} KiB; disk {dk}")
    desc = {"stage": "clippy-all-features", "argv": CLIPPY_ARGV, "env": dict(BUILD_ENV), "worktree": WT,
            "target": TARGET, "limit_seconds": LIMIT_CLIPPY, "memory_max": "16G", "swap_max": "0",
            "disk_stop_trigger_bytes": DISK_STOP, "disk_floor_bytes": DISK_FLOOR, "source_before": d}
    if not all(ok for _, ok, _ in checks):
        return finish(outdir, rec, desc, "NOT_STARTED", checks)
    sup, proc = supervised(outdir, f"{UNIT_PREFIX}-clippy-{int(time.time())}", WT, CLIPPY_ARGV, BUILD_ENV,
                           os.path.join(outdir, "raw-output.txt"), rec, LIMIT_CLIPPY, target=TARGET)
    desc.update(supervisor=sup, process=proc)
    raw = open(os.path.join(outdir, "raw-output.txt"), errors="replace").read()
    warn = re.findall(r"(?m)^warning(?:\[[^\]]+\])?: .*$", raw)
    err = re.findall(r"(?m)^error(?:\[[^\]]+\])?: .*$", raw)
    fin = re.findall(r"(?m)^\s*Finished .*$", raw)
    desc.update(warnings=warn, errors=err, finished=fin)
    add("process exited 0 by a normal exit (no signal)", proc["exit_status"] == 0 and proc["signal"] is None, proc)
    add("no warning diagnostic (`-D warnings` would make any warning an error)", not warn, warn[:5])
    add("no error diagnostic", not err, err[:5])
    add("exactly one `Finished` line", len(fin) == 1, fin)
    add("no network access attempted (no 'Downloading'/'Updating' crates.io index lines)",
        not re.search(r"(?m)^\s*(Downloading|Downloaded|Updating) ", raw))
    add("scope limits read back: 16 GiB, zero swap", sup["memory_max"] == "17179869184" and sup["swap_max"] == "0")
    add("no OOM kill, swap peak 0, completed, not timed out", sup["oom_kill"] == 0 and sup["swap_peak_bytes"] == "0"
        and sup["completed"] and not sup["timed_out"])
    add("disk availability never at or below the 17 GiB trigger", sup["disk"]["triggered"] is None
        and (sup["disk"]["min_avail_bytes"] or 0) > DISK_STOP, sup["disk"]["min_avail_bytes"])
    d2 = tree_check(True)
    rec.append(tree_line(d2, "after"))
    desc["source_after"] = d2
    add("after: tracked bytes exactly Candidate C (declared generated ignored output allowed)",
        d2["classified_verdict"] in ("EXACTLY_C", "SOURCE_EXACT_C_WITH_DECLARED_GENERATED"), d2["unexpected_status"])
    return finish(outdir, rec, desc, classification_of(sup, proc, checks), checks)


if __name__ == "__main__":
    cmd, outdir = sys.argv[1], os.path.abspath(sys.argv[2])
    assert outdir.startswith(REC + "/"), outdir
    sys.exit({"selftest": selftest, "worktree": worktree, "clippy": clippy}[cmd](outdir))

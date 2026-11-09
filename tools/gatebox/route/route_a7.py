#!/usr/bin/python3 -I
"""SB-3E A7: the route lane's compile inputs against the shipped recipe's.

Design section 5 row A7, ruling SB3E-R8 item 4. Stdlib only, Linux only.
`run` builds both recipes from clean at one absolute target path; `compare`
prints a JSON verdict and exits nonzero on any difference. `capture` snapshots
the `-vv` log, a hash of every file, rustc's dep-info, each `output`, an inotify
record of the target and the ctime of the source tree and sysroot.

Consumed inputs, without a tracer: dep-info and the final inventory are only
supplementary, so coverage is by channel and any channel that fails fails A7.
A compiler names what it read (dep-info must lie in the source tree, registry,
sysroot or an inventoried OUT_DIR, even if the file is later gone); other path
arguments into the target must be Cargo's layout; the record must show no path
outside the layout touched at all and no inventoried file changing once final,
so the hash taken at the end is the content consumed; a process outliving its
parent is adopted here and fails; a failed record fails.
"""
import argparse
import ctypes
import hashlib
import json
import os
import re
import select
import shlex
import shutil
import signal
import stat
import struct
import subprocess
import sys
import threading
import time

SEVEN = frozenset(
    {
        ("build_script_build", "kernel/build.rs"),
        ("nexus_crypto", "crates/nexus-crypto/src/lib.rs"),
        ("nexus_persistence", "persistence/src/lib.rs"),
        ("nexus_kernel", "kernel/src/lib.rs"),
        ("nexus_governed_control", "crates/nexus-governed-control/src/lib.rs"),
        ("nexus_mother", "mother/src/lib.rs"),
        ("nexus_mother_host", "mother/host/src/lib.rs"),
    }
)
CFG_UNITS = frozenset(
    {
        ("nexus_governed_control", "crates/nexus-governed-control/src/lib.rs"),
        ("nexus_mother", "mother/src/lib.rs"),
        ("nexus_mother_host", "mother/host/src/lib.rs"),
        ("route_cells", "mother/host/tests/route_cells/main.rs"),
    }
)
LANE_ONLY = frozenset(
    {
        ("route_cells", "mother/host/tests/route_cells/main.rs"),
        ("mother_hands", "mother/host/src/bin/mother-hands.rs"),
    }
)
SHIP_ARGS = ["build", "--release", "--locked", "-vv", "-p", "nexus-mother-desktop", "--bin", "nexus-mother"]
LANE_ARGS = [
    "test", "--release", "--locked", "-vv", "--no-run",
    "-p", "nexus-mother-host", "-p", "nexus-mother-desktop", "--test", "route_cells",
]
FORBIDDEN_ENV = (
    "RUSTC", "RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER", "RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS",
    "CARGO_BUILD_RUSTFLAGS", "CARGO_BUILD_RUSTC", "CARGO_BUILD_RUSTC_WRAPPER",
    "CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER", "CARGO_TARGET_DIR",
)
ROUTE_CHECK_CFG = "cfg(mother_route_seams)"
ROUTE_LINT_PACKAGES = frozenset(
    {"crates/nexus-governed-control/src/lib.rs", "mother/src/lib.rs", "mother/host/src/lib.rs",
     "mother/host/tests/route_cells/main.rs", "mother/host/src/bin/mother-hands.rs"}
)

IN_MODIFY, IN_ATTRIB, IN_CLOSE_WRITE, IN_CLOSE_NOWRITE, IN_OPEN = 0x2, 0x4, 0x8, 0x10, 0x20
IN_MOVED_FROM, IN_MOVED_TO, IN_CREATE, IN_DELETE = 0x40, 0x80, 0x100, 0x200
IN_DELETE_SELF, IN_MOVE_SELF, IN_Q_OVERFLOW = 0x400, 0x800, 0x4000
IN_ONLYDIR, IN_DONT_FOLLOW, IN_ISDIR = 0x01000000, 0x02000000, 0x40000000
WATCH_MASK = (
    IN_MODIFY | IN_CLOSE_WRITE | IN_CLOSE_NOWRITE | IN_OPEN | IN_MOVED_FROM | IN_MOVED_TO
    | IN_CREATE | IN_DELETE | IN_DELETE_SELF | IN_MOVE_SELF
)
CHANGE = IN_MODIFY | IN_CLOSE_WRITE | IN_MOVED_FROM | IN_MOVED_TO | IN_CREATE | IN_DELETE
PR_SET_CHILD_SUBREAPER = 36

HASH = r"[0-9a-f]{16}"
ANSI = re.compile(r"\x1b\[[0-9;]*m")
TEMP_NAME = r"[A-Za-z0-9]{6}"
TEMP_SUFFIX = r"\.tmp[0-9A-Za-z]{6,8}"
# Cargo's and rustc's layout under the target directory. Any other path is
# outside the inventory.
_LAYOUT = [
    ("root", r"(\.cargo-lock|\.rustc_info\.json|CACHEDIR\.TAG|tmp)?"),
    ("dirs", r"release(/\.cargo-lock|/build|/deps|/examples|/incremental|/\.fingerprint)?"),
    ("dirs", rf"release/(build|\.fingerprint)/[^/]+-{HASH}"),
    ("meta", rf"release/\.fingerprint/[^/]+-{HASH}/[^/]+"),
    ("bs_final", rf"release/build/[^/]+-{HASH}/(build-script-build|build_script_build-{HASH}|build_script_build-{HASH}\.d)"),
    ("bs_temp", rf"release/build/[^/]+-{HASH}/build_script_build-{HASH}\.[^/]*\.rcgu\.[a-z.]+"),
    ("bs_temp", rf"release/build/[^/]+-{HASH}/((rustc|rmeta){TEMP_NAME}|\.tmp{TEMP_NAME}\.temp-archive)(/.*)?"),
    ("bs_temp", rf"release/build/[^/]+-{HASH}/build_script_build-{HASH}{TEMP_SUFFIX}"),
    ("bs_meta", rf"release/build/[^/]+-{HASH}/(invoked\.timestamp|root-output|stderr)"),
    ("bs_output", rf"release/build/[^/]+-{HASH}/output"),
    ("out_dir", rf"release/build/[^/]+-{HASH}/out(/.*)?"),
    ("final", rf"release/deps/(lib)?[A-Za-z0-9_]+-{HASH}(\.rlib|\.rmeta|\.so|\.d)?"),
    ("final", r"release/(nexus-mother|mother-hands)(\.d)?"),
    ("temp", r"release/deps/[^/]*\.rcgu\.[a-z.]+"),
    ("temp", rf"release/deps/(lib)?[A-Za-z0-9_]+-{HASH}(\.so)?{TEMP_SUFFIX}"),
    ("temp", rf"release/deps/(rmeta{TEMP_NAME}|rustc{TEMP_NAME}|\.tmp{TEMP_NAME}\.temp-archive)(/.*)?"),
]
LAYOUT = [(name, re.compile(pattern)) for name, pattern in _LAYOUT]
BUILD_DIR = re.compile(rf"(release/build/[^/]+-{HASH})/(out(/.*)?|output)$")


def layout_class(rel):
    for name, pattern in LAYOUT:
        if pattern.fullmatch(rel):
            return name
    return None


def sha256_file(path):
    digest = hashlib.sha256()
    with open(path, "rb") as handle:
        for chunk in iter(lambda: handle.read(1 << 20), b""):
            digest.update(chunk)
    return digest.hexdigest()


class Monitor(threading.Thread):
    """An inotify record of every directory under `root`; no tracer, no PID."""

    def __init__(self, root):
        super().__init__(daemon=True)
        self.root = root
        self.libc = ctypes.CDLL(None, use_errno=True)
        self.fd = self.libc.inotify_init1(os.O_CLOEXEC | os.O_NONBLOCK)
        if self.fd < 0:
            raise OSError(ctypes.get_errno(), "inotify_init1")
        self.watches = {}
        self.events = []
        self.failures = []
        self.stop_r, self.stop_w = os.pipe()
        self.finished = False
        self.add_tree(root)

    def rel(self, path):
        return os.path.relpath(path, self.root) if path != self.root else ""

    def add_tree(self, top):
        pending = [top]
        while pending:
            path = pending.pop()
            watch = self.libc.inotify_add_watch(
                self.fd, os.fsencode(path), WATCH_MASK | IN_ONLYDIR | IN_DONT_FOLLOW
            )
            if watch < 0:
                code = ctypes.get_errno()
                if code not in (2, 20):  # ENOENT, ENOTDIR: already gone.
                    self.failures.append(f"inotify_add_watch {self.rel(path)}: errno {code}")
                continue
            self.watches[watch] = path
            try:
                with os.scandir(path) as entries:
                    for entry in entries:
                        if entry.is_dir(follow_symlinks=False):
                            pending.append(entry.path)
                        elif path != top or top != self.root:
                            # Present before the watch: recorded as created.
                            self.events.append((IN_CREATE, self.rel(entry.path)))
            except FileNotFoundError:
                pass

    def pump(self):
        try:
            buffer = os.read(self.fd, 1 << 20)
        except BlockingIOError:
            return False
        offset = 0
        while offset < len(buffer):
            watch, mask, _cookie, length = struct.unpack_from("iIII", buffer, offset)
            name = buffer[offset + 16 : offset + 16 + length].rstrip(b"\0")
            offset += 16 + length
            if mask & IN_Q_OVERFLOW:
                self.failures.append("inotify queue overflow")
                continue
            base = self.watches.get(watch)
            if base is None:
                continue
            path = os.path.join(base, os.fsdecode(name)) if name else base
            self.events.append((mask, self.rel(path)))
            if mask & IN_ISDIR and mask & (IN_CREATE | IN_MOVED_TO):
                self.add_tree(path)
        return True

    def run(self):
        poller = select.poll()
        poller.register(self.fd, select.POLLIN)
        poller.register(self.stop_r, select.POLLIN)
        try:
            while True:
                ready = [fd for fd, _ in poller.poll()]
                if self.fd in ready:
                    self.pump()
                if self.stop_r in ready:
                    while self.pump():
                        pass
                    break
            self.finished = True
        except Exception as error:  # Recorded: a dead monitor fails A7.
            self.failures.append(f"monitor stopped: {error!r}")

    def stop(self):
        os.write(self.stop_w, b"x")
        self.join(timeout=60)
        if not self.finished or self.is_alive():
            self.failures.append("monitor did not finish")


def walk_ctimes(top, since):
    """Paths under `top` whose ctime is after `since` (`.git` is not walked)."""
    changed = []
    for directory, dirs, files in os.walk(top):
        dirs[:] = [d for d in dirs if d != ".git"]
        for name in [".", *dirs, *files]:
            path = os.path.normpath(os.path.join(directory, name))
            try:
                if os.lstat(path).st_ctime_ns > since:
                    changed.append(path)
            except FileNotFoundError:
                changed.append(path)
    return changed


def wait_for_later_ctime(evidence):
    """A sentinel whose ctime every later change will strictly exceed."""
    sentinel = os.path.join(evidence, "start")
    open(sentinel, "w").close()
    start = os.stat(sentinel).st_ctime_ns
    probe = sentinel + ".probe"
    deadline = time.monotonic() + 5
    while True:
        with open(probe, "w"):
            pass
        later = os.stat(probe).st_ctime_ns
        os.unlink(probe)
        if later > start:
            return start
        if time.monotonic() > deadline:
            sys.exit("route_a7: the clock did not advance for the ctime sentinel")
        time.sleep(0.001)


def run_build(command, cwd, env, log_path):
    """Cargo as our child, this process its subreaper; adopted orphans returned."""
    if ctypes.CDLL(None, use_errno=True).prctl(PR_SET_CHILD_SUBREAPER, 1, 0, 0, 0) != 0:
        sys.exit("route_a7: could not become a subreaper")
    with open(log_path, "wb") as log:
        child = subprocess.Popen(command, cwd=cwd, env=env, stdout=log, stderr=subprocess.STDOUT)
    orphans = []
    status = None
    while status is None:
        pid, code = os.waitpid(-1, 0)
        if pid == child.pid:
            status = os.waitstatus_to_exitcode(code)
            child.returncode = status
        else:
            orphans.append(pid)
    # Anything still running was adopted: recorded, then stopped.
    for task in os.listdir("/proc/self/task"):
        with open(f"/proc/self/task/{task}/children") as handle:
            for pid in handle.read().split():
                orphans.append(int(pid))
                try:
                    os.kill(int(pid), signal.SIGKILL)
                except ProcessLookupError:
                    pass
    deadline = time.monotonic() + 5
    while time.monotonic() < deadline:
        try:
            pid, _ = os.waitpid(-1, os.WNOHANG)
        except ChildProcessError:
            break
        if pid == 0:
            time.sleep(0.01)
    return status, orphans


def inventory(target):
    files, symlinks, dep_info, outputs = {}, [], {}, {}
    for directory, dirs, names in os.walk(target):
        for name in dirs + names:
            path = os.path.join(directory, name)
            rel = os.path.relpath(path, target)
            mode = os.lstat(path).st_mode
            if stat.S_ISLNK(mode):
                symlinks.append(rel)
            elif stat.S_ISREG(mode):
                files[rel] = sha256_file(path)
                if name.endswith(".d") and layout_class(rel) in ("final", "bs_final"):
                    with open(path, encoding="utf-8", errors="surrogateescape") as handle:
                        dep_info[rel] = handle.read()
                elif name == "output" and layout_class(rel) == "bs_output":
                    with open(path, encoding="utf-8", errors="surrogateescape") as handle:
                        outputs[rel] = handle.read()
    return {"files": files, "symlinks": symlinks, "dep_info": dep_info, "outputs": outputs}


def write_json(path, value):
    with open(path, "w", encoding="utf-8", errors="surrogateescape") as handle:
        json.dump(value, handle, sort_keys=True)


def read_json(path):
    with open(path, encoding="utf-8", errors="surrogateescape") as handle:
        return json.load(handle)


def snapshot_inventory(target, evidence):
    write_json(os.path.join(evidence, "inventory.json"), inventory(target))


def capture(kind, repo, target, evidence, wrapper, extra_env=(), extra_args=()):
    repo, target, evidence = map(os.path.abspath, (repo, target, evidence))
    if os.path.lexists(target):
        sys.exit(f"route_a7: the target must be fresh: {target}")
    if target == repo or target.startswith(repo + os.sep):
        sys.exit("route_a7: the target must be outside the repository")
    present = [name for name in FORBIDDEN_ENV if name in os.environ]
    if present:
        sys.exit(f"route_a7: ambient build settings are refused: {present}")
    log_copy = target + ".route_rustc.log"
    if os.path.lexists(log_copy):
        os.unlink(log_copy)
    os.makedirs(evidence, exist_ok=True)
    env = dict(os.environ, CARGO_TARGET_DIR=target)
    if kind == "lane":
        env["RUSTC_WORKSPACE_WRAPPER"] = os.path.abspath(wrapper)
    for item in extra_env:
        key, _, value = item.partition("=")
        env[key] = value
    command = ["cargo"] + (SHIP_ARGS if kind == "ship" else LANE_ARGS) + list(extra_args)
    sysroot = subprocess.run(
        ["rustc", "--print", "sysroot"], cwd=repo, check=True, capture_output=True, text=True
    ).stdout.strip()
    # Fetched first: the registry must not change once the build has begun.
    subprocess.run(["cargo", "fetch", "--locked"], cwd=repo, env=env, check=True, capture_output=True)
    cargo_home = os.environ.get("CARGO_HOME", os.path.expanduser("~/.cargo"))
    start = wait_for_later_ctime(evidence)
    os.makedirs(target)
    monitor = Monitor(target)
    monitor.start()
    began = time.monotonic()
    status, orphans = run_build(command, repo, env, os.path.join(evidence, "build.log"))
    monitor.stop()
    if os.path.exists(log_copy):
        shutil.copyfile(log_copy, os.path.join(evidence, "wrapper.log"))
    changed = walk_ctimes(repo, start) + walk_ctimes(sysroot, start) + walk_ctimes(cargo_home + "/registry/src", start)
    write_json(os.path.join(evidence, "events.json"), monitor.events)
    write_json(
        os.path.join(evidence, "meta.json"),
        {
            "kind": kind,
            "repo": repo,
            "target": target,
            "sysroot": sysroot,
            "cargo_home": cargo_home,
            "wrapper": os.path.abspath(wrapper) if kind == "lane" else None,
            "status": status,
            "seconds": round(time.monotonic() - began, 2),
            "orphans": orphans,
            "changed_since_start": changed,
            "monitor_failures": monitor.failures,
        },
    )
    snapshot_inventory(target, evidence)
    return status


# ---- comparison ----------------------------------------------------------


class Units:
    """The units of one build, normalized (paths to roots, hashes to identity)."""

    def __init__(self, evidence):
        self.evidence = evidence
        self.meta = read_json(os.path.join(evidence, "meta.json"))
        self.inventory = read_json(os.path.join(evidence, "inventory.json"))
        self.events = read_json(os.path.join(evidence, "events.json"))
        self.roots = sorted(
            (
                (self.meta["target"], "<T>"),
                (self.meta["repo"], "<R>"),
                (self.meta["sysroot"], "<SYS>"),
                (self.meta["cargo_home"], "<CH>"),
            ),
            key=lambda pair: -len(pair[0]),
        )
        self.hashes = {}
        self.compiles, self.runs = [], []
        self.problems = []
        self.parse(os.path.join(evidence, "build.log"))

    def norm(self, text):
        for root, name in self.roots:
            text = text.replace(root, name)
        text = re.sub(rf"<CH>/registry/src/index\.crates\.io-{HASH}", "<REG>", text)
        return re.sub(HASH, lambda m: self.hashes.get(m.group(0), m.group(0)), text)

    def parse(self, path):
        # A command can span lines: a package description may contain newlines.
        block = None
        with open(path, encoding="utf-8", errors="surrogateescape") as handle:
            for line in handle:
                line = ANSI.sub("", line)
                stripped = line.strip()
                if block is None:
                    if stripped.startswith("Fresh "):
                        self.problems.append(("fresh_unit", stripped))
                    if not stripped.startswith("Running `"):
                        continue
                    block = stripped[len("Running `") :]
                else:
                    block += "\n" + line.rstrip("\n")
                if not block.endswith("`"):
                    continue
                try:
                    tokens = shlex.split(block[:-1])
                except ValueError:
                    continue
                block = None
                env, index = {}, 0
                while re.match(r"^[^=/]+=", tokens[index]):
                    key, _, value = tokens[index].partition("=")
                    env[key] = value
                    index += 1
                self.add(env, tokens[index:])
        if block is not None:
            self.problems.append(("unparsed_command", block[:200]))

    def add(self, env, argv):
        wrapper = None
        if argv[0] == self.meta.get("wrapper") or os.path.basename(argv[0]) == "route_rustc":
            wrapper, argv = argv[0], argv[1:]
        if os.path.basename(argv[0]) == "rustc":
            self.add_compile(env, argv[1:], wrapper)
        elif wrapper is not None:
            self.problems.append(("wrapper_on_non_rustc", " ".join(argv)[:200]))
        elif "--crate-name" not in argv and os.path.basename(argv[0]).startswith("build-script-"):
            self.add_run(env, argv)
        else:
            self.problems.append(("unparsed_command", " ".join(argv)[:200]))

    def add_compile(self, env, args, wrapper):
        if any(arg.startswith("@") for arg in args):
            self.problems.append(("response_file_argument", " ".join(args)[:200]))
        name = args[args.index("--crate-name") + 1]
        source = next(arg for arg in args if arg.endswith(".rs") and not arg.startswith("-"))
        extra = None
        kept, externs, index = [], {}, 0
        while index < len(args):
            arg = args[index]
            if arg.startswith(("--error-format", "--json")):
                index += 1
                continue
            if arg == "-C" and args[index + 1].startswith("extra-filename="):
                extra = args[index + 1].split("=", 1)[1].lstrip("-")
                kept += [arg, args[index + 1]]
                index += 2
                continue
            if arg == "--extern":
                alias, _, value = args[index + 1].partition("=")
                externs.setdefault(alias, []).append(value)
                index += 2
                continue
            kept.append(arg)
            index += 1
        norm_source = self.norm(source)
        key = (name, norm_source)
        seven = key in SEVEN
        flags = []
        index = 0
        while index < len(kept):
            arg = kept[index]
            if seven and arg == "-C" and kept[index + 1].startswith(("metadata=", "extra-filename=")):
                index += 2
                continue
            flags.append(self.norm(arg.replace(extra, "{SELF}") if extra else arg))
            index += 1
        norm_env = {k: self.norm(v.replace(extra, "{SELF}") if extra else v) for k, v in env.items()}
        identity_env = {k: v for k, v in norm_env.items() if k != "CARGO_PRIMARY_PACKAGE"}
        norm_externs = {k: sorted(self.norm(v) for v in values) for k, values in externs.items()}
        identity = hashlib.sha256(
            json.dumps([name, norm_source, flags, identity_env, norm_externs], sort_keys=True).encode()
        ).hexdigest()[:16]
        if extra:
            self.hashes[extra] = f"U{identity}"
        self.compiles.append(
            {
                "key": key,
                "identity": identity,
                "flags": flags,
                "env": norm_env,
                "externs": norm_externs,
                "wrapper": wrapper,
                "raw": args,
                "extra": extra,
            }
        )

    def add_run(self, env, argv):
        out_dir = env.get("OUT_DIR", "")
        match = re.search(rf"/release/build/[^/]+-({HASH})/out$", out_dir)
        if not match:
            self.problems.append(("build_run_without_out_dir", " ".join(argv)[:200]))
            return
        own = match.group(1)
        norm_env = {k: self.norm(v.replace(own, "{SELF}")) for k, v in env.items()}
        exe = self.norm(argv[0])
        identity_env = {k: v for k, v in norm_env.items() if k != "RUSTC_WORKSPACE_WRAPPER"}
        identity = hashlib.sha256(json.dumps([exe, identity_env], sort_keys=True).encode()).hexdigest()[:16]
        self.hashes[own] = f"R{identity}"
        relative = os.path.relpath(out_dir, self.meta["target"])
        self.runs.append(
            {"exe": exe, "identity": identity, "env": norm_env, "build_dir": os.path.dirname(relative)}
        )


def d(kind, **detail):
    return {"kind": kind, **detail}


def check_build(units, kind):
    """The per-build rules: a failed one is a difference whatever the other build did."""
    out = [d(p[0], build=kind, detail=p[1]) for p in units.problems]
    meta = units.meta
    if meta["status"] != 0:
        out.append(d("build_failed", build=kind, status=meta["status"]))
    for failure in meta["monitor_failures"]:
        out.append(d("monitor_failed", build=kind, detail=failure))
    for pid in meta["orphans"]:
        out.append(d("orphan_process", build=kind, pid=pid))
    for path in meta["changed_since_start"]:
        out.append(d("source_or_sysroot_changed", build=kind, path=path))
    for rel in units.inventory["symlinks"]:
        out.append(d("symlink_in_target", build=kind, path=rel))
    out += check_paths(units, kind)
    out += check_dep_info(units, kind)
    out += check_args(units, kind)
    return out


def check_paths(units, kind):
    """Every event in the record: inside the layout, and nothing changing once final."""
    out, seen = [], set()
    finals, outputs_done = {}, {}
    for index, (mask, rel) in enumerate(units.events):
        cls = layout_class(rel)
        if cls is None:
            if rel not in seen:
                seen.add(rel)
                out.append(d("outside_inventory", build=kind, path=rel, event=hex(mask & 0xfff)))
            continue
        if mask & IN_ISDIR or not mask & CHANGE:
            continue
        if cls == "bs_output" and mask & IN_CLOSE_WRITE:
            outputs_done.setdefault(os.path.dirname(rel), index)
        elif cls in ("final", "bs_final"):
            # Placed once; any later change is a replacement. The single close
            # right after a rename is the writer's own fd closing (`strip`).
            if rel in finals:
                closing = finals[rel] == "renamed" and mask & IN_CLOSE_WRITE and not mask & IN_MODIFY
                finals[rel] = "done"
                if not closing:
                    out.append(d("final_artifact_replaced", build=kind, path=rel, event=hex(mask & 0xfff)))
            elif mask & (IN_DELETE | IN_MOVED_FROM):
                out.append(d("final_artifact_replaced", build=kind, path=rel, event=hex(mask & 0xfff)))
            elif mask & IN_MOVED_TO:
                finals[rel] = "renamed"
            elif mask & IN_CLOSE_WRITE:
                finals[rel] = "done"
    for index, (mask, rel) in enumerate(units.events):
        match = BUILD_DIR.match(rel)
        if not match or not rel.startswith(match.group(1) + "/out") or not mask & CHANGE or mask & IN_ISDIR:
            continue
        done = outputs_done.get(match.group(1))
        if done is not None and index > done:
            out.append(d("out_dir_changed_after_run", build=kind, path=rel, event=hex(mask & 0xfff)))
    return out


def dep_info_entries(text):
    """The dependency paths of a Makefile-style rustc dep-info file."""
    entries = []
    for line in text.splitlines():
        if not line or line.startswith("#") or ":" not in line:
            continue
        _, _, tail = line.partition(": ")
        entries += re.findall(r"(?:\\ |\S)+", tail)
    return sorted({entry.replace("\\ ", " ") for entry in entries})


def check_dep_info(units, kind):
    """What each compiler read, as it reported it, must be in an inventoried place."""
    meta, out = units.meta, []
    infos = units.inventory["dep_info"]
    target = meta["target"]
    for unit in units.compiles:
        stem = f"{unit['key'][0]}-{unit['extra']}.d" if unit["extra"] else None
        names = [rel for rel in infos if stem and rel.endswith("/" + stem)]
        if not names:
            out.append(d("missing_dep_info", build=kind, unit=list(unit["key"])))
    allowed_roots = (meta["repo"] + os.sep, meta["sysroot"] + os.sep, meta["cargo_home"] + "/registry/src/")
    for rel, text in sorted(infos.items()):
        for entry in dep_info_entries(text):
            path = os.path.normpath(os.path.join(meta["repo"], entry))
            if path.startswith(target + os.sep):
                inner = os.path.relpath(path, target)
                if layout_class(inner) != "out_dir":
                    out.append(d("consumed_outside_inventory", build=kind, unit=rel, path=inner, channel="dep-info"))
                elif inner not in units.inventory["files"]:
                    out.append(d("consumed_input_missing", build=kind, unit=rel, path=inner))
            elif path.startswith(os.path.join(meta["repo"], ".git")) or not path.startswith(allowed_roots):
                out.append(d("consumed_outside_inventory", build=kind, unit=rel, path=path, channel="dep-info"))
    return out


def check_args(units, kind):
    """Path arguments into the target directory must be Cargo's own layout."""
    target, out = units.meta["target"], []
    deps = target + "/release/deps"
    out_dir = re.compile(rf"native={re.escape(target)}/release/build/[^/]+-{HASH}/out(/.*)?")
    for unit in units.compiles:
        args = unit["raw"]
        for index, arg in enumerate(args):
            before = args[index - 1] if index else ""
            linked = (before == "-L" and target not in arg) or (before == "-l" and "/" in arg) or (
                before == "-C" and arg.startswith(("link-arg", "linker="))
            )
            if linked:
                out.append(d("consumed_outside_inventory", build=kind, unit=list(unit["key"]), argument=arg, channel="link"))
            if target not in arg or arg.startswith(("--error-format", "--json")):
                continue
            allowed = before == "--out-dir" or before == "--extern" or (
                before == "-L" and (arg == f"dependency={deps}" or out_dir.fullmatch(arg))
            )
            if not allowed:
                out.append(d("consumed_outside_inventory", build=kind, unit=list(unit["key"]), argument=arg, channel="argument"))
    return out


def route_cfg_on_line(units):
    """After the one reviewed --check-cfg (SB3E-R9), no line may name the cfg."""
    out = []
    for unit in units.compiles:
        args, index = unit["raw"], 0
        while index < len(args):
            arg = args[index]
            if arg == "--check-cfg" and args[index + 1] == ROUTE_CHECK_CFG and unit["key"][1] in ROUTE_LINT_PACKAGES:
                index += 2
                continue
            if "mother_route_seams" in arg:
                out.append(d("route_cfg_on_cargo_line", unit=list(unit["key"]), argument=arg))
            index += 1
    return out


def primary_package_consumed(repo):
    """Files where `CARGO_PRIMARY_PACKAGE` could be consumed on the host (A7)."""
    hits = []
    roots = [os.path.join(repo, "mother/host")]
    for top in roots:
        for directory, _, names in os.walk(top):
            for name in names:
                path = os.path.join(directory, name)
                with open(path, "rb") as handle:
                    if b"CARGO_PRIMARY_PACKAGE" in handle.read():
                        hits.append(os.path.relpath(path, repo))
    for name in ("kernel/build.rs", "crates/nexus-verifier-sandbox/build.rs"):
        with open(os.path.join(repo, name), "rb") as handle:
            if b"CARGO_PRIMARY_PACKAGE" in handle.read():
                hits.append(name)
    return sorted(hits)


def diff_fields(a, b):
    return {
        "flags_only_ship": sorted(set(a["flags"]) - set(b["flags"]))[:40],
        "flags_only_lane": sorted(set(b["flags"]) - set(a["flags"]))[:40],
        "env_differs": sorted(k for k in set(a["env"]) | set(b["env"]) if a["env"].get(k) != b["env"].get(k))[:40],
        "externs_differ": a["externs"] != b["externs"],
    }


def compare_units(ship, lane, wrapper, wrapper_sha):
    out = []
    by_ship, by_lane = {}, {}
    for unit in ship.compiles:
        by_ship.setdefault(unit["key"], []).append(unit)
    for unit in lane.compiles:
        by_lane.setdefault(unit["key"], []).append(unit)
    # Matched by identity as a multiset; what is left over is paired by key
    # (a genuine difference) and only then counted as ship-only or lane-only.
    ship_left, lane_left = [], []
    for key in sorted(set(by_ship) | set(by_lane)):
        rest = [u["identity"] for u in by_lane.get(key, [])]
        unmatched = []
        for unit in by_ship.get(key, []):
            if unit["identity"] in rest:
                rest.remove(unit["identity"])
            else:
                unmatched.append(unit)
        lane_rest = [u for u in by_lane.get(key, []) if u["identity"] in rest]
        for a, b in zip(unmatched, lane_rest):
            out.append(d("unit_differs", unit=list(key), **diff_fields(a, b)))
        ship_left += [u["key"] for u in unmatched[len(lane_rest) :]]
        lane_left += [u["key"] for u in lane_rest[len(unmatched) :]]
    for key in sorted(set(lane_left) - LANE_ONLY):
        out.append(d("lane_only_unlisted", unit=list(key)))
    for key in sorted(LANE_ONLY - set(lane_left)):
        out.append(d("lane_only_missing", unit=list(key)))
    for key in sorted(k for k in LANE_ONLY if lane_left.count(k) > 1):
        out.append(d("lane_only_unlisted", unit=list(key), count=lane_left.count(key)))
    expected_ship = sorted(SHIP_ONLY_LIST)
    if sorted(ship_left) != expected_ship:
        for key in sorted(set(ship_left) | set(expected_ship)):
            have, want = ship_left.count(key), expected_ship.count(key)
            if have > want:
                out.append(d("ship_only_unlisted", unit=list(key), count=have - want))
            elif have < want:
                out.append(d("ship_only_missing", unit=list(key), count=want - have))
    for unit in lane.compiles:
        key, wrapped = unit["key"], unit["wrapper"]
        workspace = not key[1].startswith(("<REG>", "<CH>", "<SYS>"))
        if workspace and wrapped != wrapper:
            out.append(d("wrapper_prefix_wrong", unit=list(key), wrapper=wrapped))
        if not workspace and wrapped is not None:
            out.append(d("wrapper_on_third_party", unit=list(key)))
        primary = unit["env"].get("CARGO_PRIMARY_PACKAGE")
        if primary is not None and key not in LANE_ONLY:
            match = next((u for u in by_ship.get(key, []) if "CARGO_PRIMARY_PACKAGE" in u["env"]), None)
            if match is None and key != ("nexus_mother_host", "mother/host/src/lib.rs"):
                out.append(d("primary_package_env", unit=list(key)))
    for unit in ship.compiles:
        if unit["wrapper"] is not None:
            out.append(d("wrapper_on_shipped_unit", unit=list(unit["key"])))
    hits = primary_package_consumed(lane.meta["repo"])
    if hits:
        out.append(d("primary_package_consumed", paths=hits))
    out += route_cfg_on_line(ship) + route_cfg_on_line(lane)
    if sha256_file(wrapper) != wrapper_sha:
        out.append(d("wrapper_hash_mismatch", wrapper=wrapper))
    log = os.path.join(lane.evidence, "wrapper.log")
    added = []
    if os.path.exists(log):
        with open(log) as handle:
            entries = [json.loads(line) for line in handle]
        added = [(e["crate"], e["source"][0]) for e in entries if e["added"]]
    if sorted(added) != sorted(CFG_UNITS):
        out.append(d("wrapper_log_mismatch", added=sorted(map(list, added))))
    return out


def compare_runs(ship, lane, wrapper):
    out = []
    by_id = {run["identity"]: run for run in ship.runs}
    for run in lane.runs:
        if run["identity"] not in by_id:
            match = [s for s in ship.runs if s["exe"] == run["exe"]]
            detail = sorted(
                k
                for k in set(run["env"]) | set(match[0]["env"] if match else {})
                if match and run["env"].get(k) != match[0]["env"].get(k)
            )
            out.append(d("build_run_differs", exe=run["exe"], env_differs=detail))
            continue
        shipped = by_id[run["identity"]]
        wrapper_value = run["env"].get("RUSTC_WORKSPACE_WRAPPER")
        if wrapper_value is not None and (
            not shipped["exe"].endswith("/build-script-build")
            or wrapper_value != lane.norm(wrapper)
            or not is_kernel_run(run)
        ):
            out.append(d("build_run_wrapper_env", exe=run["exe"], value=wrapper_value))
        out += compare_outputs(shipped, run, ship, lane)
        out += compare_out_dir(shipped, run, ship, lane)
    return out


def is_kernel_run(run):
    return run["env"].get("CARGO_MANIFEST_DIR") == "<R>/kernel"


def compare_outputs(a, b, ship, lane):
    left = ship.inventory["outputs"].get(a["build_dir"] + "/output")
    right = lane.inventory["outputs"].get(b["build_dir"] + "/output")
    if left is None or right is None:
        return [d("build_output_missing", exe=b["exe"])]
    if ship.norm(left) != lane.norm(right):
        return [d("build_output_differs", exe=b["exe"])]
    return []


def compare_out_dir(a, b, ship, lane):
    def files(units, run):
        prefix = run["build_dir"] + "/out/"
        return {k[len(prefix):]: v for k, v in units.inventory["files"].items() if k.startswith(prefix)}

    left, right = files(ship, a), files(lane, b)
    return [
        d("generated_input_differs", exe=b["exe"], file=name)
        for name in sorted(set(left) | set(right))
        if left.get(name) != right.get(name)
    ]


def compare_artifacts(ship, lane):
    """Every proc-macro `.so` and every non-workspace rlib, byte for byte."""
    workspace = {u["extra"] for u in ship.compiles + lane.compiles if not u["key"][1].startswith(("<REG>", "<CH>"))}
    out = []
    left, right = ship.inventory["files"], lane.inventory["files"]
    for rel in sorted(set(left) & set(right)):
        match = re.fullmatch(rf"release/deps/lib[A-Za-z0-9_]+-({HASH})\.(so|rlib)", rel)
        if match and match.group(1) not in workspace and left[rel] != right[rel]:
            out.append(d("artifact_differs", file=rel))
    return out


def compare(ship_dir, lane_dir, wrapper, wrapper_sha):
    ship, lane = Units(ship_dir), Units(lane_dir)
    wrapper = os.path.abspath(wrapper)
    out = check_build(ship, "ship") + check_build(lane, "lane")
    if ship.meta["target"] != lane.meta["target"]:
        out.append(d("target_path_differs", ship=ship.meta["target"], lane=lane.meta["target"]))
    if ship.meta["repo"] != lane.meta["repo"]:
        out.append(d("repo_path_differs", ship=ship.meta["repo"], lane=lane.meta["repo"]))
    if ship.meta["kind"] != "ship" or lane.meta["kind"] != "lane":
        out.append(d("evidence_kind_wrong", ship=ship.meta["kind"], lane=lane.meta["kind"]))
    out += compare_units(ship, lane, wrapper, wrapper_sha)
    out += compare_runs(ship, lane, wrapper)
    out += compare_artifacts(ship, lane)
    return {
        "verdict": "pass" if not out else "fail",
        "counts": {
            "ship_units": len(ship.compiles),
            "lane_units": len(lane.compiles),
            "ship_runs": len(ship.runs),
            "lane_runs": len(lane.runs),
            "ship_events": len(ship.events),
            "lane_events": len(lane.events),
        },
        "differences": out,
    }


SHIP_ONLY_LIST = (
    ("build_script_build", "<REG>/cairo-sys-rs-0.22.9/build.rs"), ("build_script_build", "<REG>/field-offset-0.3.6/build.rs"),
    ("build_script_build", "<REG>/gdk-pixbuf-sys-0.22.9/build.rs"), ("build_script_build", "<REG>/gdk4-sys-0.11.5/build.rs"),
    ("build_script_build", "<REG>/gio-sys-0.22.9/build.rs"), ("build_script_build", "<REG>/glib-sys-0.22.9/build.rs"),
    ("build_script_build", "<REG>/gobject-sys-0.22.9/build.rs"), ("build_script_build", "<REG>/graphene-sys-0.22.9/build.rs"),
    ("build_script_build", "<REG>/gsk4-sys-0.11.5/build.rs"), ("build_script_build", "<REG>/gtk4-sys-0.11.5/build.rs"),
    ("build_script_build", "<REG>/memoffset-0.9.1/build.rs"), ("build_script_build", "<REG>/pango-sys-0.22.9/build.rs"),
    ("build_script_build", "<REG>/system-deps-9.0.1/build.rs"), ("build_script_build", "<REG>/target-lexicon-0.13.5/build.rs"),
    ("cairo", "<REG>/cairo-rs-0.22.9/src/lib.rs"), ("cairo_sys", "<REG>/cairo-sys-rs-0.22.9/src/lib.rs"),
    ("cfg_expr", "<REG>/cfg-expr-0.20.10/src/lib.rs"), ("equivalent", "<REG>/equivalent-1.0.2/src/lib.rs"),
    ("field_offset", "<REG>/field-offset-0.3.6/src/lib.rs"), ("futures_executor", "<REG>/futures-executor-0.3.32/src/lib.rs"),
    ("gdk4", "<REG>/gdk4-0.11.5/src/lib.rs"), ("gdk4_sys", "<REG>/gdk4-sys-0.11.5/src/lib.rs"),
    ("gdk_pixbuf", "<REG>/gdk-pixbuf-0.22.0/src/lib.rs"), ("gdk_pixbuf_sys", "<REG>/gdk-pixbuf-sys-0.22.9/src/lib.rs"),
    ("gio", "<REG>/gio-0.22.10/src/lib.rs"), ("gio_sys", "<REG>/gio-sys-0.22.9/src/lib.rs"),
    ("glib", "<REG>/glib-0.22.10/src/lib.rs"), ("glib_macros", "<REG>/glib-macros-0.22.9/src/lib.rs"),
    ("glib_sys", "<REG>/glib-sys-0.22.9/src/lib.rs"), ("gobject_sys", "<REG>/gobject-sys-0.22.9/src/lib.rs"),
    ("graphene", "<REG>/graphene-rs-0.22.8/src/lib.rs"), ("graphene_sys", "<REG>/graphene-sys-0.22.9/src/lib.rs"),
    ("gsk4", "<REG>/gsk4-0.11.5/src/lib.rs"), ("gsk4_sys", "<REG>/gsk4-sys-0.11.5/src/lib.rs"),
    ("gtk4", "<REG>/gtk4-0.11.5/src/lib.rs"), ("gtk4_macros", "<REG>/gtk4-macros-0.11.5/src/lib.rs"),
    ("gtk4_sys", "<REG>/gtk4-sys-0.11.5/src/lib.rs"), ("hashbrown", "<REG>/hashbrown-0.16.1/src/lib.rs"),
    ("heck", "<REG>/heck-0.5.0/src/lib.rs"), ("indexmap", "<REG>/indexmap-2.13.0/src/lib.rs"),
    ("memoffset", "<REG>/memoffset-0.9.1/src/lib.rs"), ("nexus_mother", "mother/desktop/src/main.rs"),
    ("nexus_mother_desktop", "mother/desktop/src/lib.rs"), ("pango", "<REG>/pango-0.22.9/src/lib.rs"),
    ("pango_sys", "<REG>/pango-sys-0.22.9/src/lib.rs"), ("proc_macro_crate", "<REG>/proc-macro-crate-3.5.0/src/lib.rs"),
    ("serde_spanned", "<REG>/serde_spanned-1.1.2/src/lib.rs"), ("smallvec", "<REG>/smallvec-1.15.1/src/lib.rs"),
    ("syn", "<REG>/syn-3.0.9/src/lib.rs"), ("system_deps", "<REG>/system-deps-9.0.1/src/lib.rs"),
    ("target_lexicon", "<REG>/target-lexicon-0.13.5/src/lib.rs"), ("toml", "<REG>/toml-1.1.8+spec-1.1.0/src/lib.rs"),
    ("toml_datetime", "<REG>/toml_datetime-1.1.2+spec-1.1.0/src/lib.rs"), ("toml_edit", "<REG>/toml_edit-0.25.5+spec-1.1.0/src/lib.rs"),
    ("toml_parser", "<REG>/toml_parser-1.1.5+spec-1.1.0/src/lib.rs"), ("version_compare", "<REG>/version-compare-0.2.1/src/lib.rs"),
    ("winnow", "<REG>/winnow-1.0.0/src/lib.rs"),
)


def emit(result, path):
    text = json.dumps(result, indent=1, sort_keys=True)
    if path:
        with open(path, "w", encoding="utf-8") as handle:
            handle.write(text + "\n")
    print(json.dumps({"verdict": result["verdict"], "counts": result.get("counts"),
                      "kinds": sorted({x["kind"] for x in result["differences"]})}, sort_keys=True))
    return 0 if result["verdict"] == "pass" else 1


def main():
    parser = argparse.ArgumentParser()
    sub = parser.add_subparsers(dest="command", required=True)
    cap = sub.add_parser("capture")
    cap.add_argument("--kind", choices=["ship", "lane"], required=True)
    for name in ("--repo", "--target", "--evidence", "--wrapper"):
        cap.add_argument(name, required=True)
    cap.add_argument("--extra-env", action="append", default=[])
    cap.add_argument("--extra-arg", action="append", default=[])
    inv = sub.add_parser("inventory")
    inv.add_argument("--target", required=True)
    inv.add_argument("--evidence", required=True)
    com = sub.add_parser("compare")
    for name in ("--ship", "--lane", "--wrapper", "--wrapper-sha256", "--json"):
        com.add_argument(name, required=True)
    run = sub.add_parser("run")
    for name in ("--repo", "--target", "--evidence", "--wrapper", "--wrapper-sha256", "--json"):
        run.add_argument(name, required=True)
    run.add_argument("--ship-extra-arg", action="append", default=[])
    run.add_argument("--lane-extra-arg", action="append", default=[])
    run.add_argument("--lane-extra-env", action="append", default=[])
    args = parser.parse_args()
    if args.command == "capture":
        return capture(args.kind, args.repo, args.target, args.evidence, args.wrapper, args.extra_env, args.extra_arg)
    if args.command == "inventory":
        snapshot_inventory(args.target, args.evidence)
        return 0
    if args.command == "compare":
        return emit(compare(args.ship, args.lane, args.wrapper, args.wrapper_sha256), args.json)
    ship, lane = os.path.join(args.evidence, "ship"), os.path.join(args.evidence, "lane")
    capture("ship", args.repo, args.target, ship, args.wrapper, (), args.ship_extra_arg)
    shutil.rmtree(args.target)
    capture("lane", args.repo, args.target, lane, args.wrapper, args.lane_extra_env, args.lane_extra_arg)
    shutil.rmtree(args.target)
    return emit(compare(ship, lane, args.wrapper, args.wrapper_sha256), args.json)


if __name__ == "__main__":
    sys.exit(main())

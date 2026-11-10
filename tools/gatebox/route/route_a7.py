#!/usr/bin/python3 -I
"""SB-3E A7: the route lane's recorded compile inputs against the shipped recipe's.

Design section 5 row A7 as amended by AM-1 revision 2 (ruling SB3E-R11).
Stdlib only, Linux only. `run` refuses unpinned toolchains and build-time code,
builds both recipes from clean at one absolute target path and prints a JSON
verdict, exiting nonzero on any difference. `lane` runs the route lane (design
1.3 as amended) under the same launch block and refusals.

Claim: A7 establishes equality of recorded inputs modulo the listed exceptions;
agreement of unobserved consumed inputs is assumed, not measured (assumptions
6 and 7 of design 1.5).

Channel inventory (every way bytes can reach a compiler or build script
differently between the two builds), each with the check that closes it or the
assumption it falls under:
- Toolchain selection (which cargo, rustc and rustup override run). Closed by
  `toolchain_refused`: cargo and rustc by absolute paths under the pinned
  toolchain directory, never rustup proxies; inherited RUSTUP_*, RUSTC*,
  RUSTDOC*, RUSTFLAGS, CARGO_* (including CARGO_HOME and CARGO_BUILD_*),
  MAKEFLAGS and MFLAGS refused; a `rust-toolchain` or `rust-toolchain.toml`
  anywhere refused, except the root `rust-toolchain.toml` byte-equal to its
  reviewed pin (not consulted: no proxy runs); the toolchain directory must be
  its rustc's sysroot; `.cargo/config*` anywhere refused unless byte-equal to
  CARGO_CONFIG_PINS (configuration outside the snapshot is the runner's).
  `rustc -vV`, `cargo -V` recorded and equal.
- Candidate-authored build-time code (workspace build scripts, proc macros,
  build-dependencies, `links`). Closed by `build_time_pin`: none but the pinned
  build scripts, byte-equal to BUILD_TIME_PINS; Cargo.lock byte-equal to its
  pin. The pin fixes bytes, not consumption: transient states and runtime
  behaviour of pinned code fall under assumption 6.
- Third-party build scripts and proc macros (pinned by the Cargo.lock pin and
  registry checksums): their file, environment, IPC and network reads,
  helpers, subprocesses and outputs over their whole lifetime, including
  untracked reads of exempt variables. Assumption 6, not measured.
- Effective environment of each child. Assumption 7, not measured: A7 requires
  byte-identical recorded launch blocks (`launch_block_differs`) and refuses
  MAKEFLAGS, MFLAGS, CARGO_MAKEFLAGS and non-UTF-8 entries. Cargo's displayed
  assignments are compared in both directions as corroboration
  (`unit_differs`, `build_run_differs`), exemptions bound to exact units and
  values. A7 does not claim effective-environment equality.
- Tracked environment consumption: `# env-dep:` lines of dep-info (`env!`,
  `option_env!`, any name spelling) compared exactly (`env_dep_differs`); an
  exempt variable consumed is `exempt_env_consumed`. Untracked reads fall
  under assumption 6.
- Compiler arguments: one parser for every spelling (`-L x`, `-Lx`,
  `-Lkind=x`, `-l`, `-C k=v`, `-Ck=v`, `--codegen[=]`, `--extern[=]`, ...);
  path arguments confined to Cargo's layout (`argument_outside_layout`), link
  controls refused (`link_control`), `@` arguments refused, only `--error-format`
  and `--json` ignored (`--color`, `--diagnostic-width` refused); flags compared
  exactly; lane-only units' recipe settings validated (`lane_only_recipe`).
- Source, registry and sysroot files a compiler reads: dep-info paths confined
  to them or to an inventoried OUT_DIR (`consumed_outside_inventory`,
  `consumed_input_missing`); no ctime change after build start
  (`source_or_sysroot_changed`); repository hash equal before and after both
  builds (`repository_changed`).
- Generated and dependency artifacts: byte equality of every non-workspace
  rlib, rmeta, proc-macro `.so`, build-script executable, `output` and OUT_DIR
  file, every mandatory hash and run record required (`artifact_differs`,
  `artifact_missing`, `generated_input_differs`, `build_output_*`,
  `build_run_*`, `missing_dep_info`). Contents a reader saw before a later
  write: assumption 6 (no later writer invalidates an inventoried file).
- Wrapper: its binary pinned by hash (`wrapper_hash_mismatch`); its process
  state handling is closed in U3a-1; its log corroborates the four units
  (`wrapper_log_mismatch`).
- Unit sets: lane-only exactly LANE_ONLY, ship-only exactly SHIP_ONLY_LIST,
  each with multiplicity; no `Fresh` unit.
"""
import argparse
import hashlib
import json
import os
import re
import shlex
import shutil
import stat
import subprocess
import sys
import time

# Reviewed pins; under GB1 these move to installed policy outside the snapshot.
BUILD_TIME_PINS = {"kernel/build.rs": "5dd672bf7812edfc0fe41be7f6002f973ae87adec0eebfb92ae1f17d33e88da2"}
LOCK_PIN = "a42d2d167fe3c01f4c61b4365dcdb0c64f2548ed3c52d16193b702f1d30b000e"
TOOLCHAIN_FILE_PIN = "fc7f9811196c9d9db1864dd140fa4b7632cda9b7761ed8736256b463c835d298"
# The snapshot's only Cargo configuration (build.jobs, [env]); any other or changed one is refused.
CARGO_CONFIG_PINS = {".cargo/config.toml": "379a150667985722540bd30f737cfb363dc5bc2dc8a5edae3db3725e3cd0b234"}
ROOT_PACKAGES = ("nexus-mother-host", "nexus-mother-desktop")

SEVEN = frozenset({
    ("build_script_build", "kernel/build.rs"),
    ("nexus_crypto", "crates/nexus-crypto/src/lib.rs"),
    ("nexus_persistence", "persistence/src/lib.rs"),
    ("nexus_kernel", "kernel/src/lib.rs"),
    ("nexus_governed_control", "crates/nexus-governed-control/src/lib.rs"),
    ("nexus_mother", "mother/src/lib.rs"),
    ("nexus_mother_host", "mother/host/src/lib.rs"),
})
CFG_UNITS = frozenset({
    ("nexus_governed_control", "crates/nexus-governed-control/src/lib.rs"),
    ("nexus_mother", "mother/src/lib.rs"),
    ("nexus_mother_host", "mother/host/src/lib.rs"),
    ("route_cells", "mother/host/tests/route_cells/main.rs"),
})
LANE_ONLY = frozenset({
    ("route_cells", "mother/host/tests/route_cells/main.rs"),
    ("mother_hands", "mother/host/src/bin/mother-hands.rs"),
})
HOST = ("nexus_mother_host", "mother/host/src/lib.rs")
KERNEL_BUILD = ("build_script_build", "kernel/build.rs")
SHIPPED_BIN = ("nexus_mother", "mother/desktop/src/main.rs")
EXEMPT_ENV = ("CARGO_PRIMARY_PACKAGE", "RUSTC_WORKSPACE_WRAPPER")
ROUTE_CHECK_CFG = "cfg(mother_route_seams)"
ROUTE_LINT_SOURCES = frozenset({
    "crates/nexus-governed-control/src/lib.rs", "mother/src/lib.rs", "mother/host/src/lib.rs",
    "mother/host/tests/route_cells/main.rs", "mother/host/src/bin/mother-hands.rs",
})
SHIP_ARGS = ["build", "--release", "--locked", "-vv", "-p", "nexus-mother-desktop", "--bin", "nexus-mother"]
LANE_TAIL = ["test", "--release", "--locked", "-p", "nexus-mother-host", "-p", "nexus-mother-desktop", "--test", "route_cells"]
REFUSED_EXACT = ("MAKEFLAGS", "MFLAGS")
REFUSED_PREFIX = ("RUSTUP_", "RUSTC", "RUSTDOC", "RUSTFLAGS", "CARGO_")

HASH = r"[0-9a-f]{16}"
ANSI = re.compile(r"\x1b\[[0-9;]*m")
# Path-valued environment variables: only these have unit hashes mapped.
PATH_ENV = re.compile(r"OUT_DIR|CARGO_MANIFEST_DIR|CARGO_MANIFEST_PATH|CARGO_TARGET_TMPDIR|CARGO_BIN_EXE_\w+|LD_LIBRARY_PATH")
LINT_OPTS = {"-A": "A", "-W": "W", "-D": "D", "-F": "F", "--allow": "A", "--warn": "W", "--deny": "D", "--forbid": "F",
             "--cap-lints": "cap-lints", "--check-cfg": "check-cfg", "--force-warn": "force-warn"}
VALUE_OPTS = {"--extern": "extern", "--out-dir": "out-dir", "-o": "o", "--cfg": "cfg", "--crate-name": "crate-name",
              "--crate-type": "crate-type", "--edition": "edition", "--emit": "emit", "--sysroot": "sysroot",
              "--target": "target", "--remap-path-prefix": "remap", "--codegen": "C", "--print": "print",
              "--explain": "explain", "--env-set": "env-set", "--error-format": "ignored", "--json": "ignored",
              "--diagnostic-width": "diagnostic-width", "--color": "color", "-L": "L", "-l": "l", "-C": "C", "-Z": "Z", **LINT_OPTS}
SHORT_JOINED = ("-L", "-l", "-C", "-Z", "-A", "-W", "-D", "-F")


def sha256_file(path):
    digest = hashlib.sha256()
    with open(path, "rb") as handle:
        for chunk in iter(lambda: handle.read(1 << 20), b""):
            digest.update(chunk)
    return digest.hexdigest()


def write_json(path, value):
    with open(path, "w", encoding="utf-8", errors="surrogateescape") as handle:
        json.dump(value, handle, sort_keys=True)


def read_json(path):
    with open(path, encoding="utf-8", errors="surrogateescape") as handle:
        return json.load(handle)


def d(kind, **detail):
    return {"kind": kind, **detail}


# ---- launch block, toolchain and build-time pins (AM-1 items 1-3) ----------


def launch_block(toolchain, target):
    """(block, refusals): the inherited environment plus the runner's settings."""
    out = []
    for raw in os.environb:
        try:
            raw.decode("utf-8"), os.environb[raw].decode("utf-8")
        except UnicodeDecodeError:
            out.append(d("toolchain_refused", reason="non-UTF-8 environment entry"))
    for name in sorted(os.environ):
        if name in REFUSED_EXACT or name.startswith(REFUSED_PREFIX):
            out.append(d("toolchain_refused", reason=f"inherited {name}"))
    block = dict(os.environ)
    block["CARGO_TARGET_DIR"] = target
    block["RUSTC"] = os.path.join(toolchain, "bin", "rustc")
    block["PATH"] = os.path.join(toolchain, "bin") + os.pathsep + os.environ.get("PATH", "/usr/bin:/bin")
    return block, out


def toolchain_problems(repo, toolchain):
    out = []
    for name in ("cargo", "rustc"):
        path = os.path.join(toolchain, "bin", name)
        if not os.path.isabs(path) or not os.path.isfile(path) or os.path.islink(path):
            out.append(d("toolchain_refused", reason=f"no pinned {name} at {path}"))
    if not out:
        block, _ = launch_block(toolchain, "/nonexistent")
        if os.path.realpath(versions(toolchain, block)["sysroot"]) != os.path.realpath(toolchain):
            out.append(d("toolchain_refused", reason=f"{toolchain} is not the sysroot of its rustc"))
    for directory, dirs, names in os.walk(repo):
        dirs[:] = [x for x in dirs if x != ".git"]
        for name in names + [x for x in dirs if x == ".cargo" and os.path.islink(os.path.join(directory, x))]:
            path = os.path.join(directory, name)
            rel = os.path.relpath(path, repo)
            if name in ("rust-toolchain", "rust-toolchain.toml"):
                if rel != "rust-toolchain.toml" or os.path.islink(path) or sha256_file(path) != TOOLCHAIN_FILE_PIN:
                    out.append(d("toolchain_refused", reason=f"toolchain file {rel}"))
            elif name == ".cargo" or (os.path.basename(directory) == ".cargo" and name in ("config", "config.toml")):
                if os.path.islink(path) or sha256_file(path) != CARGO_CONFIG_PINS.get(rel):
                    out.append(d("toolchain_refused", reason=f"Cargo configuration {rel}"))
    return out


def versions(toolchain, block):
    run = lambda *argv: subprocess.run(argv, env=block, capture_output=True, text=True, check=True).stdout
    sysroot = run(os.path.join(toolchain, "bin", "rustc"), "--print", "sysroot").strip()
    return {
        "rustc": run(os.path.join(toolchain, "bin", "rustc"), "-vV"),
        "cargo": run(os.path.join(toolchain, "bin", "cargo"), "-V"),
        "sysroot": sysroot,
    }


def build_time_problems(repo, toolchain, block):
    """AM-1 item 3: the route closure's candidate-authored build-time code is pinned."""
    out = []
    lock = os.path.join(repo, "Cargo.lock")
    if not os.path.isfile(lock) or sha256_file(lock) != LOCK_PIN:
        out.append(d("build_time_pin", reason="Cargo.lock differs from its pin"))
    run = subprocess.run([os.path.join(toolchain, "bin", "cargo"), "metadata", "--locked", "--offline", "--format-version", "1"],
                         cwd=repo, env=block, capture_output=True, text=True)
    if run.returncode != 0:
        return out + [d("build_time_pin", reason="cargo metadata --locked failed")]
    meta = json.loads(run.stdout)
    packages = {p["id"]: p for p in meta["packages"]}
    nodes = {n["id"]: n for n in meta["resolve"]["nodes"]}
    workspace = set(meta["workspace_members"])
    roots = [i for i in workspace if packages[i]["name"] in ROOT_PACKAGES]
    if len(roots) != len(ROOT_PACKAGES):
        out.append(d("build_time_pin", reason="route root packages not found"))
    closure, pending = set(), [(i, True) for i in roots]
    while pending:
        node, root = pending.pop()
        if node in closure and not root:
            continue
        closure.add(node)
        for dep in nodes[node]["deps"]:
            kinds = {k["kind"] for k in dep["dep_kinds"]}
            if kinds - {"dev"} or (root and "dev" in kinds):
                pending.append((dep["pkg"], False))
            if node in workspace and "build" in kinds:
                out.append(d("build_time_pin", reason=f"build-dependency in {packages[node]['name']}"))
    for node in sorted(closure & workspace):
        package = packages[node]
        if package.get("links"):
            out.append(d("build_time_pin", reason=f"links in {package['name']}"))
        for target in package["targets"]:
            rel = os.path.relpath(target["src_path"], repo)
            if "proc-macro" in target["kind"]:
                out.append(d("build_time_pin", reason=f"workspace proc macro {rel}"))
            if "custom-build" in target["kind"]:
                if BUILD_TIME_PINS.get(rel) != sha256_file(target["src_path"]):
                    out.append(d("build_time_pin", reason=f"build script {rel} is not a pinned entry"))
    return out


# ---- capture -----------------------------------------------------------------


def repo_hash(repo):
    digest = hashlib.sha256()
    for directory, dirs, names in sorted(os.walk(repo)):
        dirs[:] = sorted(x for x in dirs if x != ".git")
        for name in sorted(names):
            path = os.path.join(directory, name)
            digest.update(os.fsencode(os.path.relpath(path, repo)) + b"\0")
            mode = os.lstat(path).st_mode
            digest.update(os.fsencode(os.readlink(path)) if stat.S_ISLNK(mode) else sha256_file(path).encode())
    return digest.hexdigest()


def walk_ctimes(top, since):
    changed = []
    for directory, dirs, files in os.walk(top):
        dirs[:] = [x for x in dirs if x != ".git"]
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
    deadline = time.monotonic() + 5
    while True:
        probe = sentinel + ".probe"
        open(probe, "w").close()
        later = os.stat(probe).st_ctime_ns
        os.unlink(probe)
        if later > start:
            return start
        if time.monotonic() > deadline:
            sys.exit("route_a7: the clock did not advance for the ctime sentinel")
        time.sleep(0.001)


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
                if name.endswith(".d") and "/incremental/" not in rel:
                    with open(path, encoding="utf-8", errors="surrogateescape") as handle:
                        dep_info[rel] = handle.read()
                elif name == "output" and re.fullmatch(rf"release/build/[^/]+-{HASH}/output", rel):
                    with open(path, encoding="utf-8", errors="surrogateescape") as handle:
                        outputs[rel] = handle.read()
    return {"files": files, "symlinks": symlinks, "dep_info": dep_info, "outputs": outputs}


def snapshot_inventory(target, evidence):
    write_json(os.path.join(evidence, "inventory.json"), inventory(target))


def capture(kind, repo, target, evidence, wrapper, toolchain, extra_args=(), extra_config=()):
    """One build from clean; the evidence is snapshotted, the target left for the caller."""
    if os.path.lexists(target) or os.path.lexists(target + ".route_rustc.log"):
        sys.exit(f"route_a7: the target must be fresh: {target}")
    if target == repo or target.startswith(repo + os.sep):
        sys.exit("route_a7: the target must be outside the repository")
    os.makedirs(evidence, exist_ok=True)
    block, _ = launch_block(toolchain, target)
    cargo = os.path.join(toolchain, "bin", "cargo")
    if kind == "ship":
        command = [cargo, *SHIP_ARGS, *extra_args]
    else:
        config = [f'build.rustc-workspace-wrapper="{wrapper}"', *extra_config]
        command = [cargo, *sum((["--config", c] for c in config), []), *LANE_TAIL[:3], "-vv", "--no-run",
                   *LANE_TAIL[3:], *extra_args]
    info = versions(toolchain, block)
    cargo_home = os.path.join(os.environ.get("HOME", "/"), ".cargo")
    start = wait_for_later_ctime(evidence)
    began = time.monotonic()
    with open(os.path.join(evidence, "build.log"), "wb") as log:
        status = subprocess.run(command, cwd=repo, env=block, stdout=log, stderr=subprocess.STDOUT).returncode
    if os.path.exists(target + ".route_rustc.log"):
        shutil.copyfile(target + ".route_rustc.log", os.path.join(evidence, "wrapper.log"))
    changed = walk_ctimes(repo, start) + walk_ctimes(info["sysroot"], start) + walk_ctimes(cargo_home + "/registry/src", start)
    write_json(os.path.join(evidence, "meta.json"), {
        "kind": kind, "repo": repo, "target": target, "cargo_home": cargo_home, "toolchain": toolchain,
        "wrapper": wrapper if kind == "lane" else None, "command": command, "status": status,
        "seconds": round(time.monotonic() - began, 2), "changed_since_start": changed,
        "launch_block": sorted(f"{k}={v}" for k, v in block.items()), **info,
    })
    snapshot_inventory(target, evidence)
    return status


# ---- parsing -----------------------------------------------------------------


def canonical(args):
    """Every accepted spelling of an option as one (option, value) pair."""
    pairs, index, problems = [], 0, []
    while index < len(args):
        arg = args[index]
        index += 1
        if arg.startswith("@"):
            problems.append(("response_file_argument", arg))
            continue
        if arg in ("-O", "-g"):
            pairs.append(("C", "opt-level=3" if arg == "-O" else "debuginfo=2"))
            continue
        if arg in VALUE_OPTS:
            if index >= len(args):
                problems.append(("unparsed_argument", arg))
                break
            pairs.append((VALUE_OPTS[arg], args[index]))
            index += 1
            continue
        name, eq, value = arg.partition("=")
        if arg.startswith("--") and eq and name in VALUE_OPTS:
            pairs.append((VALUE_OPTS[name], value))
        elif arg[:2] in SHORT_JOINED and len(arg) > 2:
            pairs.append((VALUE_OPTS[arg[:2]], arg[2:]))
        elif arg.startswith("-"):
            pairs.append(("flag", arg))
        else:
            pairs.append(("source", arg))
    return pairs, problems


class Units:
    """The units of one build, as recorded."""

    def __init__(self, evidence):
        self.evidence = evidence
        self.meta = read_json(os.path.join(evidence, "meta.json"))
        self.inventory = read_json(os.path.join(evidence, "inventory.json"))
        self.target = self.meta["target"]
        self.rustc = os.path.join(self.meta["toolchain"], "bin", "rustc")
        self.hashes, self.compiles, self.runs, self.problems = {}, [], [], []
        self.parse(os.path.join(evidence, "build.log"))

    def rel(self, path):
        if path.startswith(self.meta["repo"] + os.sep):
            return os.path.relpath(path, self.meta["repo"])
        return re.sub(rf"^{re.escape(self.meta['cargo_home'])}/registry/src/index\.crates\.io-{HASH}/", "<REG>/", path)

    def map_hashes(self, text):
        return re.sub(rf"-({HASH})\b", lambda m: "-" + self.hashes.get(m.group(1), m.group(1)), text)

    def parse(self, path):
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
                    block = stripped[len("Running `"):]
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
                while index < len(tokens) and re.match(r"^[^=/\s]+=", tokens[index]):
                    key, _, value = tokens[index].partition("=")
                    env[key] = value
                    index += 1
                self.add(env, tokens[index:])
        if block is not None:
            self.problems.append(("unparsed_command", block[:200]))

    def add(self, env, argv):
        wrapper = None
        if argv and argv[0] == self.meta.get("wrapper"):
            wrapper, argv = argv[0], argv[1:]
        if argv and argv[0] == self.rustc:
            if "--crate-name" in argv and argv[argv.index("--crate-name") + 1] == "___":
                return  # Cargo's target-info probe.
            self.add_compile(env, argv[1:], wrapper)
        elif wrapper is not None or (argv and os.path.basename(argv[0]) == "rustc"):
            self.problems.append(("compiler_not_pinned", " ".join(argv)[:200]))
        elif argv and re.fullmatch(rf"{re.escape(self.target)}/release/build/[^/]+-{HASH}/build-script-build", argv[0]):
            self.add_run(env, argv)
        else:
            self.problems.append(("unparsed_command", " ".join(argv)[:200]))

    def add_compile(self, env, args, wrapper):
        pairs, problems = canonical(args)
        self.problems += problems
        names = [v for k, v in pairs if k == "crate-name"]
        sources = [self.rel(v) for k, v in pairs if k == "source"]
        if len(names) != 1 or len(sources) != 1:
            self.problems.append(("unparsed_command", " ".join(args)[:200]))
            return
        key = (names[0], sources[0])
        extras = [v.split("=", 1)[1].lstrip("-") for k, v in pairs if k == "C" and v.startswith("extra-filename=")]
        extra = extras[0] if len(extras) == 1 else None
        if extra is None:
            self.problems.append(("missing_extra_filename", list(key)))
        seven = key in SEVEN
        own = (lambda text: text.replace("-" + extra, "-{SELF}")) if extra else (lambda text: text)
        flags, externs = [], []
        for option, value in pairs:
            if option == "ignored":
                continue
            if option == "C" and value.split("=", 1)[0] in ("metadata", "extra-filename"):
                if not seven:
                    flags.append((option, value))
                continue
            if option == "extern":
                externs.append(self.map_hashes(value))
            elif option in ("L", "out-dir"):
                flags.append((option, self.map_hashes(own(value))))
            elif option == "source":
                flags.append((option, self.rel(value)))
            else:
                flags.append((option, value))
        norm_env = {k: (self.map_hashes(own(v)) if PATH_ENV.fullmatch(k) else v) for k, v in env.items()}
        identity_env = {k: v for k, v in norm_env.items() if not (key == HOST and k == "CARGO_PRIMARY_PACKAGE")}
        identity = hashlib.sha256(json.dumps([key, flags, identity_env, sorted(externs)]).encode()).hexdigest()[:16]
        if extra:
            self.hashes[extra] = "U" + identity
        self.compiles.append({"key": key, "identity": identity, "flags": flags, "env": norm_env,
                              "externs": sorted(externs), "wrapper": wrapper, "pairs": pairs, "extra": extra})

    def add_run(self, env, argv):
        exe_hash = re.search(rf"-({HASH})/build-script-build$", argv[0]).group(1)
        out_dir = env.get("OUT_DIR", "")
        match = re.fullmatch(rf"{re.escape(self.target)}/release/build/[^/]+-({HASH})/out", out_dir)
        if not match or len(argv) != 1:
            self.problems.append(("build_run_unparsed", " ".join(argv)[:200]))
            return
        exe = self.map_hashes(os.path.relpath(argv[0], self.target))
        own = match.group(1)
        norm_env = {k: (self.map_hashes(v.replace("-" + own, "-{SELF}")) if PATH_ENV.fullmatch(k) else v) for k, v in env.items()}
        manifest = env.get("CARGO_MANIFEST_DIR", "")
        kernel = manifest == os.path.join(self.meta["repo"], "kernel")
        identity_env = {k: v for k, v in norm_env.items() if not (kernel and k == "RUSTC_WORKSPACE_WRAPPER")}
        identity = hashlib.sha256(json.dumps([exe, identity_env]).encode()).hexdigest()[:16]
        self.hashes[own] = "R" + identity
        self.runs.append({"exe": exe, "exe_hash": exe_hash, "identity": identity, "env": norm_env, "kernel": kernel,
                          "build_dir": os.path.relpath(os.path.dirname(out_dir), self.target)})


# ---- per-build checks ------------------------------------------------------------


def check_build(units, kind):
    out = [d(p[0], build=kind, detail=p[1]) for p in units.problems]
    meta = units.meta
    if meta["status"] != 0:
        out.append(d("build_failed", build=kind, status=meta["status"]))
    for path in meta["changed_since_start"]:
        out.append(d("source_or_sysroot_changed", build=kind, path=path))
    for rel in units.inventory["symlinks"]:
        out.append(d("symlink_in_target", build=kind, path=rel))
    for name in ("MAKEFLAGS", "MFLAGS", "CARGO_MAKEFLAGS"):
        if any(e.startswith(name + "=") for e in meta["launch_block"]):
            out.append(d("toolchain_refused", build=kind, reason=f"{name} in the launch block"))
    out += check_runs_complete(units, kind) + check_dep_info(units, kind) + check_args(units, kind)
    return out


def check_runs_complete(units, kind):
    """Each compiled build script ran; each run had its compile (counts compared in compare_runs)."""
    out = []
    scripts = [u for u in units.compiles if u["key"][0] == "build_script_build"]
    for unit in scripts:
        count = sum(1 for r in units.runs if r["exe_hash"] == unit["extra"])
        if count < 1:
            out.append(d("build_run_count", build=kind, unit=list(unit["key"]), runs=count))
    known = {u["extra"] for u in scripts}
    for run in units.runs:
        if run["exe_hash"] not in known:
            out.append(d("build_run_without_compile", build=kind, exe=run["exe"]))
        if run["build_dir"] + "/output" not in units.inventory["outputs"]:
            out.append(d("build_output_missing", build=kind, exe=run["exe"]))
    return out


def dep_info_of(units, unit):
    if not unit["extra"]:
        return None
    stem = f"/{unit['key'][0]}-{unit['extra']}.d"
    names = [rel for rel in units.inventory["dep_info"] if rel.endswith(stem)]
    return units.inventory["dep_info"][names[0]] if len(names) == 1 else None


def dep_info_parse(text):
    """(paths, env-deps) of a Makefile-style rustc dep-info file."""
    paths, envs = set(), []
    for line in text.splitlines():
        if line.startswith("# env-dep:"):
            envs.append(line[len("# env-dep:"):])
        elif line and not line.startswith("#") and ": " in line:
            paths.update(entry.replace("\\ ", " ") for entry in re.findall(r"(?:\\ |\S)+", line.partition(": ")[2]))
    return sorted(paths), sorted(envs)


def check_dep_info(units, kind):
    meta, out, target = units.meta, [], units.target
    roots = (meta["repo"] + os.sep, meta["sysroot"] + os.sep, meta["cargo_home"] + "/registry/src/")
    out_dirs = tuple(r["build_dir"] + "/out/" for r in units.runs)
    for unit in units.compiles:
        text = dep_info_of(units, unit)
        if text is None:
            out.append(d("missing_dep_info", build=kind, unit=list(unit["key"])))
            continue
        paths, envs = dep_info_parse(text)
        unit["env_deps"] = envs
        for entry in envs:
            if entry.split("=", 1)[0] in EXEMPT_ENV:
                out.append(d("exempt_env_consumed", build=kind, unit=list(unit["key"]), env=entry))
        for entry in paths:
            path = os.path.normpath(os.path.join(meta["repo"], entry))
            if path.startswith(target + os.sep):
                inner = os.path.relpath(path, target)
                if not inner.startswith(out_dirs):
                    out.append(d("consumed_outside_inventory", build=kind, unit=list(unit["key"]), path=inner))
                elif inner not in units.inventory["files"]:
                    out.append(d("consumed_input_missing", build=kind, unit=list(unit["key"]), path=inner))
            elif path.startswith(os.path.join(meta["repo"], ".git") + os.sep) or not path.startswith(roots):
                out.append(d("consumed_outside_inventory", build=kind, unit=list(unit["key"]), path=path))
    return out


def check_args(units, kind):
    """Path arguments confined to Cargo's layout; link controls refused."""
    t, out = re.escape(units.target), []
    deps = units.target + "/release/deps"
    native = re.compile(rf"{t}/release/build/[^/]+-{HASH}/out(/.*)?")
    artifact = re.compile(rf"{t}/release/deps/lib[A-Za-z0-9_]+-{HASH}\.(rlib|rmeta|so)")
    out_dir = re.compile(rf"{t}/release/(deps|build/[^/]+-{HASH})")
    for unit in units.compiles:
        where = list(unit["key"])
        for option, value in unit["pairs"]:
            bad = None
            if option == "L":
                kind_, eq, path = value.partition("=")
                ok = eq and ((kind_ == "dependency" and path == deps) or (kind_ == "native" and native.fullmatch(path)))
                bad = None if ok else "argument_outside_layout"
            elif option == "l":
                bad = "argument_outside_layout" if "/" in value else None
            elif option == "C" and value.split("=", 1)[0] in ("link-arg", "link-args", "linker", "linker-flavor",
                                                               "link-self-contained", "pre-link-arg", "pre-link-args"):
                bad = "link_control"
            elif option == "extern":
                path = value.partition("=")[2]
                bad = None if not path or artifact.fullmatch(path) else "argument_outside_layout"
            elif option == "out-dir":
                bad = None if out_dir.fullmatch(value) else "argument_outside_layout"
            elif option == "emit":
                bad = "argument_outside_layout" if "=" in value else None
            elif option in ("o", "sysroot", "target", "remap", "Z", "print", "explain", "env-set", "color", "diagnostic-width"):
                bad = "unexpected_option"
            elif units.target in value and option not in ("source",):
                bad = "argument_outside_layout"
            if bad:
                out.append(d(bad, build=kind, unit=where, option=option, value=value))
    return out


# ---- comparison --------------------------------------------------------------------


def route_cfg_mentions(units):
    """SB3E-R9: after removing exactly one reviewed --check-cfg, no argument names the cfg."""
    out = []
    for unit in units.compiles:
        allowance = 1 if unit["key"][1] in ROUTE_LINT_SOURCES else 0
        for option, value in unit["pairs"]:
            if "mother_route_seams" not in value and "mother_route_seams" not in option:
                continue
            if option == "check-cfg" and value == ROUTE_CHECK_CFG and allowance:
                allowance -= 1
                continue
            out.append(d("route_cfg_on_cargo_line", unit=list(unit["key"]), option=option, value=value))
        for key, value in unit["env"].items():
            if "mother_route_seams" in key + value:
                out.append(d("route_cfg_on_cargo_line", unit=list(unit["key"]), env=key))
    return out


def diff_fields(a, b, exempt_env=()):
    env_keys = sorted(k for k in set(a["env"]) | set(b["env"]) if a["env"].get(k) != b["env"].get(k) and k not in exempt_env)
    return {
        "flags_only_ship": [list(f) for f in a["flags"] if f not in b["flags"]][:40],
        "flags_only_lane": [list(f) for f in b["flags"] if f not in a["flags"]][:40],
        "env_differs": env_keys[:40],
        "externs_differ": a["externs"] != b["externs"],
    }


def lane_only_recipe(ship, lane):
    """Lane-only units: the shipped bin's codegen settings, the host package's lints."""
    out = []
    shipped = [u for u in ship.compiles if u["key"] == SHIPPED_BIN]
    host = [u for u in lane.compiles if u["key"] == HOST]
    if len(shipped) != 1 or len(host) != 1:
        return [d("lane_only_recipe", reason="reference unit missing")]
    codegen = lambda u: sorted(v for k, v in u["pairs"] if k == "C" and v.split("=", 1)[0] not in ("metadata", "extra-filename"))
    of = lambda u, kinds: sorted((k, v) for k, v in u["pairs"] if k in kinds)
    lints = set(LINT_OPTS.values())
    for unit in lane.compiles:
        if unit["key"] not in LANE_ONLY:
            continue
        where = list(unit["key"])
        if codegen(unit) != codegen(shipped[0]):
            out.append(d("lane_only_recipe", unit=where, field="codegen", lane=codegen(unit), ship=codegen(shipped[0])))
        for field in (lints, {"edition"}):
            if of(unit, field) != of(host[0], field):
                out.append(d("lane_only_recipe", unit=where, field=sorted(field)[0]))
        allowed = {"crate-name", "source", "crate-type", "emit", "out-dir", "L", "extern", "C", "edition", "ignored"} | lints
        test = unit["key"] == ("route_cells", "mother/host/tests/route_cells/main.rs")
        for option, value in unit["pairs"]:
            if option == "flag" and test and value == "--test":
                continue
            if option not in allowed or (option == "crate-type" and value != "bin"):
                out.append(d("lane_only_recipe", unit=where, option=option, value=value))
        extra_env = sorted(set(unit["env"]) - set(host[0]["env"]) - {"CARGO_BIN_NAME", "CARGO_CRATE_NAME", "CARGO_BIN_EXE_mother-hands", "CARGO_TARGET_TMPDIR"})
        if extra_env:
            out.append(d("lane_only_recipe", unit=where, env=extra_env))
    return out


def compare_units(ship, lane, wrapper, wrapper_sha):
    out = []
    by_ship, by_lane = {}, {}
    for unit in ship.compiles:
        by_ship.setdefault(unit["key"], []).append(unit)
    for unit in lane.compiles:
        by_lane.setdefault(unit["key"], []).append(unit)
    ship_left, lane_left = [], []
    for key in sorted(set(by_ship) | set(by_lane)):
        rest = list(by_lane.get(key, []))
        unmatched = []
        for unit in by_ship.get(key, []):
            match = next((u for u in rest if u["identity"] == unit["identity"]), None)
            if match is None:
                unmatched.append(unit)
            else:
                rest.remove(match)
                unit["pair"], match["pair"] = match, unit
        for a, b in zip(unmatched, rest):
            out.append(d("unit_differs", unit=list(key), **diff_fields(a, b)))
        ship_left += [u["key"] for u in unmatched[len(rest):]]
        lane_left += [u["key"] for u in rest[len(unmatched):]]
    for key in sorted(set(lane_left) | LANE_ONLY):
        if lane_left.count(key) != (1 if key in LANE_ONLY else 0):
            out.append(d("lane_only_unlisted" if key not in LANE_ONLY or lane_left.count(key) > 1 else "lane_only_missing",
                         unit=list(key), count=lane_left.count(key)))
    for key in sorted(set(ship_left) | set(SHIP_ONLY_LIST)):
        have, want = ship_left.count(key), SHIP_ONLY_LIST.count(key)
        if have != want:
            out.append(d("ship_only_unlisted" if have > want else "ship_only_missing", unit=list(key), count=have - want))
    for unit in lane.compiles:
        workspace = unit["key"] in SEVEN or unit["key"] in LANE_ONLY
        if unit["wrapper"] != (wrapper if workspace else None):
            out.append(d("wrapper_prefix_wrong", unit=list(unit["key"]), wrapper=unit["wrapper"]))
        pair = unit.get("pair")
        if pair is not None:
            exempt = ("CARGO_PRIMARY_PACKAGE",) if unit["key"] == HOST and unit["env"].get("CARGO_PRIMARY_PACKAGE") == "1" else ()
            if {k: v for k, v in unit["env"].items() if k not in exempt} != pair["env"]:
                out.append(d("unit_differs", unit=list(unit["key"]), **diff_fields(pair, unit)))
            if unit.get("env_deps") != pair.get("env_deps"):
                out.append(d("env_dep_differs", unit=list(unit["key"]), ship=pair.get("env_deps"), lane=unit.get("env_deps")))
    for unit in ship.compiles:
        if unit["wrapper"] is not None:
            out.append(d("wrapper_on_shipped_unit", unit=list(unit["key"])))
    out += lane_only_recipe(ship, lane) + route_cfg_mentions(ship) + route_cfg_mentions(lane)
    if not os.path.isfile(wrapper) or sha256_file(wrapper) != wrapper_sha:
        out.append(d("wrapper_hash_mismatch", wrapper=wrapper))
    added = []
    log = os.path.join(lane.evidence, "wrapper.log")
    if os.path.exists(log):
        with open(log, encoding="utf-8", errors="surrogateescape") as handle:
            added = [(e["crate"], e["source"][0]) for e in map(json.loads, handle) if e["added"]]
    if sorted(added) != sorted(CFG_UNITS):
        out.append(d("wrapper_log_mismatch", added=sorted(map(list, added))))
    return out


def compare_runs(ship, lane, wrapper):
    out = []
    ship_only_scripts = {u["extra"] for u in ship.compiles if u["key"] in SHIP_ONLY_LIST}
    shipped = [r for r in ship.runs if r["exe_hash"] not in ship_only_scripts]
    for run in lane.runs:
        match = next((r for r in shipped if r["identity"] == run["identity"]), None)
        if match is None:
            similar = next((r for r in shipped if r["exe"] == run["exe"]), {"env": {}})
            keys = sorted(k for k in set(run["env"]) | set(similar["env"]) if run["env"].get(k) != similar["env"].get(k))
            out.append(d("build_run_differs", exe=run["exe"], env_differs=keys))
            continue
        shipped.remove(match)
        value = run["env"].get("RUSTC_WORKSPACE_WRAPPER")
        if (value is not None) != run["kernel"] or (value is not None and value != wrapper) or "RUSTC_WORKSPACE_WRAPPER" in match["env"]:
            out.append(d("build_run_wrapper_env", exe=run["exe"], value=value))
        out += compare_outputs(match, run, ship, lane) + compare_out_dir(match, run, ship, lane)
    for run in shipped:
        out.append(d("build_run_missing", exe=run["exe"]))
    return out


def compare_outputs(a, b, ship, lane):
    left = ship.inventory["outputs"].get(a["build_dir"] + "/output")
    right = lane.inventory["outputs"].get(b["build_dir"] + "/output")
    if left is None or right is None:
        return [d("build_output_missing", exe=b["exe"])]
    return [] if left == right else [d("build_output_differs", exe=b["exe"])]


def compare_out_dir(a, b, ship, lane):
    def files(units, run):
        prefix = run["build_dir"] + "/out/"
        return {k[len(prefix):]: v for k, v in units.inventory["files"].items() if k.startswith(prefix)}

    left, right = files(ship, a), files(lane, b)
    return [d("generated_input_differs", exe=b["exe"], file=name)
            for name in sorted(set(left) | set(right)) if left.get(name) != right.get(name)]


def artifacts_of(units, unit):
    name, extra = unit["key"][0], unit["extra"]
    types = [v for k, v in unit["pairs"] if k == "crate-type"]
    if name == "build_script_build":
        build = [v for k, v in unit["pairs"] if k == "out-dir"]
        return [os.path.relpath(build[0], units.target) + f"/build_script_build-{extra}"] if build else []
    if types == ["proc-macro"]:
        return [f"release/deps/lib{name}-{extra}.so"]
    emits = ",".join(v for k, v in unit["pairs"] if k == "emit")
    files = [f"release/deps/lib{name}-{extra}.rlib"] if "link" in emits else []
    return files + ([f"release/deps/lib{name}-{extra}.rmeta"] if "metadata" in emits else [])


def compare_artifacts(ship, lane):
    """Every non-workspace artifact required in both builds and byte-equal."""
    out = []
    for unit in lane.compiles:
        pair = unit.get("pair")
        if pair is None or unit["key"] in SEVEN:
            continue
        mine, theirs = artifacts_of(lane, unit), artifacts_of(ship, pair)
        if not mine or len(mine) != len(theirs):
            out.append(d("artifact_missing", unit=list(unit["key"])))
        for a, b in zip(theirs, mine):
            left, right = ship.inventory["files"].get(a), lane.inventory["files"].get(b)
            if left is None or right is None:
                out.append(d("artifact_missing", unit=list(unit["key"]), file=b))
            elif left != right:
                out.append(d("artifact_differs", unit=list(unit["key"]), file=b))
    return out


def compare(ship_dir, lane_dir, wrapper, wrapper_sha):
    try:
        ship, lane = Units(ship_dir), Units(lane_dir)
    except (OSError, KeyError, ValueError) as error:
        return {"verdict": "fail", "differences": [d("evidence_missing", detail=repr(error))]}
    out = check_build(ship, "ship") + check_build(lane, "lane")
    for field in ("target", "repo", "toolchain", "rustc", "cargo", "sysroot", "launch_block", "repo_hash_before", "repo_hash_after"):
        if ship.meta.get(field) != lane.meta.get(field):
            out.append(d("launch_block_differs" if field == "launch_block" else f"{field}_differs"))
    if ship.meta.get("repo_hash_before") != lane.meta.get("repo_hash_after"):
        out.append(d("repository_changed"))
    if ship.meta["kind"] != "ship" or lane.meta["kind"] != "lane":
        out.append(d("evidence_kind_wrong"))
    for item in ship.meta.get("pre_build", []) + lane.meta.get("pre_build", []):
        out.append(item)
    out += compare_units(ship, lane, wrapper, wrapper_sha)
    out += compare_runs(ship, lane, wrapper)
    out += compare_artifacts(ship, lane)
    unique = {json.dumps(x, sort_keys=True): x for x in out}
    return {
        "verdict": "pass" if not unique else "fail",
        "counts": {"ship_units": len(ship.compiles), "lane_units": len(lane.compiles),
                   "ship_runs": len(ship.runs), "lane_runs": len(lane.runs)},
        "differences": list(unique.values()),
    }


SHIP_ONLY_LIST = tuple(sorted(
    [("build_script_build", f"<REG>/{c}/build.rs") for c in (
        "cairo-sys-rs-0.22.9", "field-offset-0.3.6", "gdk-pixbuf-sys-0.22.9", "gdk4-sys-0.11.5", "gio-sys-0.22.9",
        "glib-sys-0.22.9", "gobject-sys-0.22.9", "graphene-sys-0.22.9", "gsk4-sys-0.11.5", "gtk4-sys-0.11.5",
        "memoffset-0.9.1", "pango-sys-0.22.9", "system-deps-9.0.1", "target-lexicon-0.13.5")]
    + [(n, f"<REG>/{c}/src/lib.rs") for n, c in (
        ("cairo", "cairo-rs-0.22.9"), ("cairo_sys", "cairo-sys-rs-0.22.9"), ("cfg_expr", "cfg-expr-0.20.10"),
        ("equivalent", "equivalent-1.0.2"), ("field_offset", "field-offset-0.3.6"),
        ("futures_executor", "futures-executor-0.3.32"), ("gdk4", "gdk4-0.11.5"), ("gdk4_sys", "gdk4-sys-0.11.5"),
        ("gdk_pixbuf", "gdk-pixbuf-0.22.0"), ("gdk_pixbuf_sys", "gdk-pixbuf-sys-0.22.9"), ("gio", "gio-0.22.10"),
        ("gio_sys", "gio-sys-0.22.9"), ("glib", "glib-0.22.10"), ("glib_macros", "glib-macros-0.22.9"),
        ("glib_sys", "glib-sys-0.22.9"), ("gobject_sys", "gobject-sys-0.22.9"), ("graphene", "graphene-rs-0.22.8"),
        ("graphene_sys", "graphene-sys-0.22.9"), ("gsk4", "gsk4-0.11.5"), ("gsk4_sys", "gsk4-sys-0.11.5"),
        ("gtk4", "gtk4-0.11.5"), ("gtk4_macros", "gtk4-macros-0.11.5"), ("gtk4_sys", "gtk4-sys-0.11.5"),
        ("hashbrown", "hashbrown-0.16.1"), ("heck", "heck-0.5.0"), ("indexmap", "indexmap-2.13.0"),
        ("memoffset", "memoffset-0.9.1"), ("pango", "pango-0.22.9"), ("pango_sys", "pango-sys-0.22.9"),
        ("proc_macro_crate", "proc-macro-crate-3.5.0"), ("serde_spanned", "serde_spanned-1.1.2"),
        ("smallvec", "smallvec-1.15.1"), ("syn", "syn-3.0.9"), ("system_deps", "system-deps-9.0.1"),
        ("target_lexicon", "target-lexicon-0.13.5"), ("toml", "toml-1.1.8+spec-1.1.0"),
        ("toml_datetime", "toml_datetime-1.1.2+spec-1.1.0"), ("toml_edit", "toml_edit-0.25.5+spec-1.1.0"),
        ("toml_parser", "toml_parser-1.1.5+spec-1.1.0"), ("version_compare", "version-compare-0.2.1"),
        ("winnow", "winnow-1.0.0"))]
    + [SHIPPED_BIN, ("nexus_mother_desktop", "mother/desktop/src/lib.rs")]
))


def emit(result, path):
    text = json.dumps(result, indent=1, sort_keys=True)
    if path:
        with open(path, "w", encoding="utf-8") as handle:
            handle.write(text + "\n")
    print(json.dumps({"verdict": result["verdict"], "counts": result.get("counts"),
                      "kinds": sorted({x["kind"] for x in result["differences"]})}, sort_keys=True))
    return 0 if result["verdict"] == "pass" else 1


def pre_build(repo, target, toolchain):
    """AM-1 items 2 and 3, before any metadata or build step's result is used."""
    block, refused = launch_block(toolchain, target)
    refused += toolchain_problems(repo, toolchain)
    if refused:
        return refused
    subprocess.run([os.path.join(toolchain, "bin", "cargo"), "fetch", "--locked"], cwd=repo, env=block,
                   check=True, capture_output=True)
    return build_time_problems(repo, toolchain, block)


def main():
    parser = argparse.ArgumentParser()
    sub = parser.add_subparsers(dest="command", required=True)
    run = sub.add_parser("run")
    for name in ("--repo", "--target", "--evidence", "--wrapper", "--wrapper-sha256", "--toolchain", "--json"):
        run.add_argument(name, required=True)
    run.add_argument("--keep-lane-target", action="store_true")
    run.add_argument("--continue-after-refusal", action="store_true", help="controls only; the verdict still fails")
    for name in ("--ship-extra-arg", "--lane-extra-arg", "--lane-extra-config"):
        run.add_argument(name, action="append", default=[])
    com = sub.add_parser("compare")
    for name in ("--ship", "--lane", "--wrapper", "--wrapper-sha256", "--json"):
        com.add_argument(name, required=True)
    lane = sub.add_parser("lane")
    for name in ("--repo", "--target", "--wrapper", "--toolchain"):
        lane.add_argument(name, required=True)
    args = parser.parse_args()
    if args.command == "compare":
        return emit(compare(args.ship, args.lane, args.wrapper, args.wrapper_sha256), args.json)
    repo, target, toolchain = map(os.path.abspath, (args.repo, args.target, args.toolchain))
    wrapper = os.path.abspath(args.wrapper)
    if args.command == "lane":
        if os.path.lexists(target) or os.path.lexists(target + ".route_rustc.log"):
            sys.exit(f"route_a7: the target must be fresh: {target}")
        problems = pre_build(repo, target, toolchain)
        if problems:
            print(json.dumps(problems, sort_keys=True))
            return 1
        block, _ = launch_block(toolchain, target)
        command = [os.path.join(toolchain, "bin", "cargo"), "--config", f'build.rustc-workspace-wrapper="{wrapper}"', *LANE_TAIL]
        return subprocess.run(command, cwd=repo, env=block).returncode
    evidence = os.path.abspath(args.evidence)
    problems = pre_build(repo, target, toolchain)
    if problems and not args.continue_after_refusal:
        return emit({"verdict": "fail", "differences": problems}, args.json)
    ship, lane = os.path.join(evidence, "ship"), os.path.join(evidence, "lane")
    before = repo_hash(repo)
    capture("ship", repo, target, ship, wrapper, toolchain, args.ship_extra_arg)
    shutil.rmtree(target)
    capture("lane", repo, target, lane, wrapper, toolchain, args.lane_extra_arg, args.lane_extra_config)
    if not args.keep_lane_target:
        shutil.rmtree(target)
    after = repo_hash(repo)
    for path in (ship, lane):
        meta = read_json(os.path.join(path, "meta.json"))
        meta.update(repo_hash_before=before, repo_hash_after=after, pre_build=problems)
        write_json(os.path.join(path, "meta.json"), meta)
    return emit(compare(ship, lane, wrapper, args.wrapper_sha256), args.json)


if __name__ == "__main__":
    sys.exit(main())

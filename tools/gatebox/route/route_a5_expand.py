#!/usr/bin/python3 -IB
"""SB-3E A5 shadow expansion (design rev 3.1: SB3E-R71, R72 B1, B2, N7): A5's analysis builds only.

As the RUSTC_WORKSPACE_WRAPPER (through a launcher A5 writes; Cargo runs it for workspace units only):
  LAUNCHER RUSTC ARGS...   configured by the sidecar `$CARGO_TARGET_DIR.a5x.json` (no variable is added):
      {"capture": DIR, "inner": route_rustc or null, "keep_emit": false}
  The compile runs first, unchanged (through `inner` in the lane), with the environment and descriptors it got.
  Then the shadow: the same argv less `--emit`, `-o`, `--out-dir`, `--json`, `--error-format` and
  `-C incremental`, plus `-Zunpretty=expanded -o <private> --out-dir <private>`, its environment the received
  one plus RUSTC_BOOTSTRAP (R70.4: this extra process only, never one that writes an artifact). It runs twice,
  RUSTC_BOOTSTRAP=1 and =<crate name> (B2 differential). The first compile's outputs (out-dir entries holding
  the unit's extra-filename, its `.d` among them) are recorded by sha256, inode, mtime and size before and after
  (B1). One JSON row per invocation is appended to DIR/index.jsonl; probes (`-vV`, `--crate-name ___`) pass through.
  `keep_emit` (the B1 control only) keeps the original argv for the shadow, as an unstripped shadow would.
For A5: capture() runs one recipe with this wrapper, plain() its plain `-vv` rustc lines (B2 reconciliation),
graph() the workspace units of Cargo's unit graph for it (N7: A7's analysis query form, a scratch target).
"""
import hashlib
import json
import os
import re
import shlex
import shutil
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
STRIP = ("--emit", "-o", "--out-dir", "--json", "--error-format")
HASH = r"[0-9a-f]{16}"


def strip(args):
    out, i = [], 0
    while i < len(args):
        a, b = args[i], args[i + 1] if i + 1 < len(args) else ""
        if a in STRIP or a in ("-C", "--codegen") and b.startswith("incremental="):
            i += 2
        elif a.startswith(tuple(o + "=" for o in STRIP)) or a.startswith(("-Cincremental=", "--codegen=incremental=")):
            i += 1
        else:
            out.append(a)
            i += 1
    return out


def values(args, opt):
    return [b for a, b in zip(args, args[1:]) if a == opt] + [a[len(opt) + 1:] for a in args if a.startswith(opt + "=")]


def outputs(out_dir, extra):
    found = {}
    for name in sorted(os.listdir(out_dir)) if out_dir and extra and os.path.isdir(out_dir) else ():
        path = os.path.join(out_dir, name)
        if extra in name and os.path.isfile(path):
            st = os.stat(path)
            with open(path, "rb") as handle:
                found[name] = [hashlib.sha256(handle.read()).hexdigest(), st.st_ino, st.st_mtime_ns, st.st_size]
    return found


def append(capture, row):
    data = (json.dumps(row, sort_keys=True) + "\n").encode()
    fd = os.open(os.path.join(capture, "index.jsonl"), os.O_WRONLY | os.O_APPEND | os.O_CREAT | os.O_CLOEXEC, 0o644)
    try:
        if os.write(fd, data) != len(data):
            sys.exit("route_a5_expand: short index write")
    finally:
        os.close(fd)


def wrapper(argv):
    rustc, args, env0 = argv[1], argv[2:], dict(os.environ)
    with open(env0["CARGO_TARGET_DIR"] + ".a5x.json", encoding="utf-8") as handle:
        cfg = json.load(handle)
    chain = [cfg["inner"], rustc] if cfg["inner"] else [rustc]
    names, sources = values(args, "--crate-name"), [a for a in args if a.endswith(".rs") and not a.startswith("-")]
    if len(names) != 1 or names[0] == "___" or len(sources) != 1:
        append(cfg["capture"], {"passthrough": args})
        os.execve(chain[0], chain + args, env0)
    crate = names[0]
    extras = [v.split("=", 1)[1] for v in values(args, "-C") if v.startswith("extra-filename=")]
    out_dirs = values(args, "--out-dir")
    extra, out_dir = extras[0] if len(extras) == 1 else None, out_dirs[0] if len(out_dirs) == 1 else None
    rc = subprocess.run(chain + args, env=env0, close_fds=False).returncode
    row = {"crate": crate, "source": sources[0], "test": "--test" in args, "argv": args, "rc": rc, "extra": extra,
           "out_dir": out_dir}
    if rc == 0:
        before = outputs(out_dir, extra)
        digest = hashlib.sha256(json.dumps(strip(args)).encode()).hexdigest()[:16]
        private = os.path.join(cfg["capture"], "private", f"{crate}-{digest}")
        row.update(file=f"{crate}-{digest}.rs", shadow=[], env_ok="RUSTC_BOOTSTRAP" not in env0)
        for k, boot in enumerate(("1", crate)):
            out = os.path.join(private, str(k))
            os.makedirs(os.path.join(out, "o"))
            env1 = dict(env0, RUSTC_BOOTSTRAP=boot)
            row["env_ok"] &= sorted(x for x in set(env0) | set(env1) if env0.get(x) != env1.get(x)) == ["RUSTC_BOOTSTRAP"]
            target = os.path.join(out, "expanded")
            shadow = args + ["-Zunpretty=expanded"] if cfg.get("keep_emit") else \
                strip(args) + ["-Zunpretty=expanded", "-o", target, "--out-dir", os.path.join(out, "o")]
            run = subprocess.run(chain + shadow, env=env1, capture_output=True, close_fds=False)
            if cfg.get("keep_emit"):
                with open(target, "wb") as handle:
                    handle.write(run.stdout)
            data = open(target, "rb").read() if os.path.isfile(target) else b""
            row["shadow"].append({"rc": run.returncode, "sha256": hashlib.sha256(data).hexdigest(), "bytes": len(data),
                                  "wrote": sorted(os.path.relpath(os.path.join(a, f), out) for a, _, fs in os.walk(out)
                                                  for f in fs if os.path.join(a, f) != target),
                                  "stderr": run.stderr.decode("utf-8", "replace")[-400:] if run.returncode else ""})
            if k == 0 and data:
                shutil.copyfile(target, os.path.join(cfg["capture"], row["file"]))
        shutil.rmtree(private)
        row.update(before=before, unchanged=bool(before) and before == outputs(out_dir, extra))
    append(cfg["capture"], row)
    return rc


def env_block(toolchain, target):
    return {"HOME": os.environ["HOME"], "PATH": os.path.join(toolchain, "bin") + ":/usr/bin:/bin", "LANG": "C.UTF-8",
            "RUSTC": os.path.join(toolchain, "bin", "rustc"), "CARGO_TARGET_DIR": target,
            "CARGO_CACHE_AUTO_CLEAN_FREQUENCY": "never"}


def recipe(kind):
    """The cargo arguments of each recipe (A7's ship and lane commands; N7: every one `--offline --locked`)."""
    sys.path.insert(0, HERE)
    import route_a7
    ship = route_a7.SHIP_ARGS + ["--offline"]
    lane = route_a7.LANE_TAIL[:3] + ["-vv", "--no-run", "--offline"] + route_a7.LANE_TAIL[3:]
    return {"ship": ship, "ship-dev": [a for a in ship if a != "--release"], "lane": lane,
            "test": ["test", "--release", "--locked", "--offline", "-vv", "--no-run", "-p", "nexus-governed-control",
                     "-p", "nexus-mother", "--lib"]}[kind]


def capture(repo, toolchain, kind, work, name, inner, target=None, keep_emit=False):
    """One recipe built with the shadow wrapper: (rows, capture dir, cargo exit). A fresh target unless given."""
    target, capdir = target or os.path.join(work, "t-" + name), os.path.join(work, "cap-" + name)
    launch = os.path.join(work, "launch-" + name)
    os.makedirs(capdir)
    with open(launch, "w", encoding="utf-8") as handle:
        handle.write(f"#!/usr/bin/python3 -IB\nimport runpy\nrunpy.run_path({os.path.join(HERE, 'route_a5_expand.py')!r},"
                     " run_name='__main__')\n")
    os.chmod(launch, 0o755)
    with open(target + ".a5x.json", "w", encoding="utf-8") as handle:
        json.dump({"capture": capdir, "inner": inner if kind == "lane" else None, "keep_emit": keep_emit}, handle)
    with open(os.path.join(work, name + ".log"), "wb") as log:
        rc = subprocess.run([os.path.join(toolchain, "bin", "cargo"), "--config", f'build.rustc-workspace-wrapper="{launch}"',
                             *recipe(kind)], cwd=repo, env=env_block(toolchain, target), stdout=log, stderr=subprocess.STDOUT,
                            stdin=subprocess.DEVNULL).returncode
    os.remove(target + ".a5x.json")
    index = os.path.join(capdir, "index.jsonl")
    rows = [json.loads(x) for x in open(index, encoding="utf-8")] if os.path.isfile(index) else []
    return rows, capdir, rc


def normalized(args, target, extra):
    """R72 B2: argv modulo `-C metadata`, `-C extra-filename`, the target dir, `-L dependency=`, `--extern` hash
    suffixes, and (measured, phase M2b) the unit's own extra-filename in its own `--out-dir`."""
    out = []
    for a in args:
        if out[-1:] in (["-C"], ["--codegen"]) and a.split("=")[0] in ("metadata", "extra-filename"):
            a = a.split("=")[0] + "=*"
        a = a.replace(target, "<T>")
        if out[-1:] == ["--out-dir"] and extra and a.endswith(extra):
            a = a[:-len(extra)] + "-{SELF}"
        if out[-1:] == ["--extern"]:
            a = re.sub(rf"-{HASH}\.(rlib|rmeta|so)$", r"-*.\1", a)
        out.append(a)
    return out


def key(args):
    return (values(args, "--crate-name") + [None])[0], ([a for a in args if a.endswith(".rs") and not a.startswith("-")]
                                                         + [None])[0], "--test" in args


def plain(repo, toolchain, kind, work, inner):
    """The plain recipe (no shadow; the lane through `inner`): {unit key: [normalized argv, ...]}, cargo exit."""
    target, rustc = os.path.join(work, "t-plain-" + kind), os.path.join(toolchain, "bin", "rustc")
    conf = ["--config", f'build.rustc-workspace-wrapper="{inner}"'] if kind == "lane" else []
    run = subprocess.run([os.path.join(toolchain, "bin", "cargo"), *conf, *recipe(kind)], cwd=repo,
                         env=env_block(toolchain, target), capture_output=True, stdin=subprocess.DEVNULL)
    shutil.rmtree(target, ignore_errors=True)
    found, block = {}, None
    for line in run.stdout.decode("utf-8", "surrogateescape").splitlines() + run.stderr.decode("utf-8", "surrogateescape").splitlines():
        line = re.sub(r"\x1b\[[0-9;]*m", "", line)
        if block is None and not line.strip().startswith("Running `"):
            continue
        block = line.strip()[len("Running `"):] if block is None else block + "\n" + line
        if not block.endswith("`"):
            continue
        argv, block = shlex.split(block[:-1]), None
        while argv and re.match(r"^[^=/\s]+=", argv[0]):
            argv = argv[1:]
        argv = argv[1:] if kind == "lane" and argv[:1] == [inner] else argv
        if argv[:1] == [rustc] and "--crate-name" in argv and key(argv[1:])[0] != "___":
            extras = [v.split("=", 1)[1] for v in values(argv[1:], "-C") if v.startswith("extra-filename=")]
            found.setdefault(key(argv[1:]), []).append(normalized(argv[1:], target, (extras + [None])[0]))
    return found, run.returncode


def graph(repo, toolchain, kind, work):
    """Workspace compile units (path packages, not build-script runs) of Cargo's unit graph for the recipe."""
    env = dict(env_block(toolchain, os.path.join(work, "t-graph-" + kind)), RUSTC_BOOTSTRAP="1")
    run = subprocess.run([os.path.join(toolchain, "bin", "cargo"), *recipe(kind), "-Z", "unstable-options", "--unit-graph"],
                         cwd=repo, env=env, capture_output=True, text=True, stdin=subprocess.DEVNULL)
    shutil.rmtree(env["CARGO_TARGET_DIR"], ignore_errors=True)
    if run.returncode != 0:
        return None
    return sorted((u["target"]["name"].replace("-", "_"), os.path.relpath(u["target"]["src_path"], repo), u["mode"] == "test")
                  for u in json.loads(run.stdout)["units"] if u["pkg_id"].startswith("path+") and u["mode"] != "run-custom-build")


if __name__ == "__main__":
    sys.exit(wrapper(sys.argv))

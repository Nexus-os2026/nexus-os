#!/usr/bin/python3 -I
"""SB-3E U4 one-SHA exit gate (mission U4b item 7; SB3E-R36.3, R37, R41).

  route_exit.py --repo R --sha SHA --toolchain T --wrapper W --wrapper-sha256 H --work DIR --out FILE

One run on one clean commit, steps in order, each logged under DIR/logs:
  preflight     HEAD is SHA, the tree is clean (ignored files too, R55.5), the
                working files' tree (a private index, R60.8) is HEAD's, the
                wrapper is the pinned one;
  a3_check_data A3's data check, before any scan and before A4 (R36.3);
  std_tests     real builds and every existing guard: the route packages' tests
                (desktop: lib, bins and the listed tests; `live` needs a desktop);
  fmt           `cargo fmt --all -- --check`;
  lane          receipt discovery (R28.1): the lane harness built with the
                wrapper, `--list --format terse` names equal route_exit.json's
                list and hash under the policy rule, then the harness run whole;
  a1, a1_controls, a2_controls, a3_ship, a3_census, a3_controls, a4, a4_controls,
  a5, a5_controls, a6, a6_control, a7, a7_controls
                each tool on this tree; the real-link A3 controls and A4's
                positive, broken-scanner and planted controls rerun here (R37);
  plants        every U4a plant (route_plants.py) with its listed killer; the
                list, its names without `expect` and its whole file's sha256
                (R55.6) equal the `plants` pin;
  no_pycache    no `__pycache__` under tools/ (R60.7);
  postflight    HEAD, the status and the working files' tree unchanged.
Each seam is listed with its data disposition per binary and what A3 and A4
measured. One verdict JSON is written to a fresh FILE; exit 0 only if every step passes.
Each tool runs with `-X pycache_prefix` under DIR, so no cached bytecode in the
tree is read (R55.5); a failed git command is never a clean tree (R55.5).
"""
import argparse
import hashlib
import json
import os
import shutil
import subprocess
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))
PY = ["/usr/bin/python3", "-I"]
# SB3E-R55.5: ignored files the clean-tree check accepts (reviewed; none).
IGNORED_OK = ()
DATA = os.path.join(HERE, "route_exit.json")
# A different wrapper for A7's hash-mismatch control (any other bytes serve).
PY_WRAPPER = "#!/usr/bin/python3 -I\nimport os, sys\nos.execv(sys.argv[1], sys.argv[1:])\n"


def sha256(data):
    return hashlib.sha256(data).hexdigest()


class Gate:
    def __init__(self, args):
        self.repo, self.work, self.toolchain = args.repo, args.work, args.toolchain
        self.wrapper, self.wrapper_sha = args.wrapper, args.wrapper_sha256
        self.logs = os.path.join(self.work, "logs")
        os.makedirs(self.logs)
        bin_dir = os.path.join(self.toolchain, "bin")
        home = os.environ["HOME"]
        self.env = {"HOME": home, "PATH": bin_dir + ":/usr/bin:/bin", "LANG": "C.UTF-8",
                    "RUSTC": os.path.join(bin_dir, "rustc"), "CARGO_HOME": os.environ.get("CARGO_HOME", f"{home}/.cargo"),
                    "RUSTUP_HOME": os.environ.get("RUSTUP_HOME", f"{home}/.rustup"), "CARGO_NET_OFFLINE": "true"}
        # A tool builds its own toolchain environment and refuses an inherited RUSTUP_*/RUSTC*/CARGO_* (A1, A7).
        self.tool_env = {"HOME": home, "PATH": bin_dir + ":/usr/bin:/bin", "LANG": "C.UTF-8"}
        self.steps = []

    def run(self, step, argv, cwd=None, env=None, verdict_json=False, ok=None):
        began = time.monotonic()
        run = subprocess.run(argv, cwd=cwd or self.repo, env=env or self.env, capture_output=True,
                             stdin=subprocess.DEVNULL)
        log = os.path.join(self.logs, step + ".log")
        with open(log, "wb") as handle:
            handle.write(b"$ " + " ".join(argv).encode() + b"\n" + run.stdout + b"\n--- stderr\n" + run.stderr)
        row = {"step": step, "exit": run.returncode, "seconds": round(time.monotonic() - began),
               "log_sha256": sha256(open(log, "rb").read())}
        parsed = None
        if verdict_json:
            try:
                parsed = json.loads(run.stdout.decode().strip().splitlines()[-1])
                row["verdict"] = parsed.get("verdict")
                row["findings"] = parsed.get("findings", [])[:5]
            except (ValueError, IndexError, AttributeError):
                row["verdict"] = "unparsed"
        row["pass"] = ok(run, parsed) if ok else run.returncode == 0 and (not verdict_json or row.get("verdict") == "pass")
        self.steps.append(row)
        return run, parsed

    def tool(self, step, name, *args, **kw):
        cache = "pycache_prefix=" + os.path.join(self.work, "pycache")
        return self.run(step, PY + ["-X", cache, os.path.join(HERE, name)] + list(args), env=self.tool_env,
                        verdict_json=True, **kw)

    def cargo(self, step, target, *args, **kw):
        env = dict(self.env, CARGO_TARGET_DIR=os.path.join(self.work, target))
        return self.run(step, [os.path.join(self.toolchain, "bin", "cargo")] + list(args), env=env, **kw)

    def fact(self, step, ok, **detail):
        self.steps.append(dict(detail, step=step, **{"pass": bool(ok)}))

    def drop(self, *names):
        for name in names:
            shutil.rmtree(os.path.join(self.work, name), ignore_errors=True)


def git(repo, *args):
    """stdout, or None when git fails (SB3E-R55.5: never read as a clean tree)."""
    run = subprocess.run(["git", "-C", repo] + list(args), capture_output=True, text=True)
    return run.stdout.rstrip("\n") if run.returncode == 0 else None


def status(repo):
    """The tree's changes, untracked and ignored files included, less IGNORED_OK; None on failure."""
    out = git(repo, "status", "--porcelain", "--ignored", "--untracked-files=all")
    return None if out is None else "\n".join(x for x in out.splitlines() if x[3:] not in IGNORED_OK)


def controls_ok(run, parsed):
    """A1/A2/A3 controls print a summary, not a verdict: exit 0, controls run, none failed, every tree HEAD's."""
    return run.returncode == 0 and bool(parsed) and parsed.get("controls", 0) > 0 and parsed.get("failed") == [] \
        and True in (parsed.get("trees_match_head"), parsed.get("tree_matches_head"))


def a7_controls_ok(run, _):
    """A7's controls print one line per control, then `all controls report` only when all report."""
    return run.returncode == 0 and run.stdout.decode().strip().splitlines()[-1:] == ["all controls report"]


def worktree_tree(repo, index):
    """SB3E-R60.8: the tree of the working files, from a fresh private index (no assume-unchanged or
    skip-worktree flag hides an edit); None on failure."""
    env = {"PATH": "/usr/bin:/bin", "HOME": os.environ["HOME"], "LANG": "C.UTF-8", "GIT_INDEX_FILE": index}
    for args in (["add", "-A", "-f", "."], ["write-tree"]):
        run = subprocess.run(["git", "-C", repo] + args, env=env, capture_output=True, text=True)
        if run.returncode != 0:
            return None
    return run.stdout.strip()


def lane(gate, data):
    run, _ = gate.cargo("lane_build", "target-lane", "--config", f'build.rustc-workspace-wrapper="{gate.wrapper}"',
                        "test", "--release", "--locked", "--offline", "-p", "nexus-mother-host", "-p", "nexus-mother-desktop",
                        "--test", "route_cells", "--no-run", "--message-format=json")
    exe = None
    for line in run.stdout.decode().splitlines():
        row = json.loads(line) if line.startswith("{") else {}
        if row.get("reason") == "compiler-artifact" and "route_cells" in (row.get("executable") or ""):
            exe = row["executable"]
    if exe is None:
        return gate.fact("lane_inventory", False, reason="no harness executable")
    env = {"PATH": "/usr/bin:/bin", "HOME": gate.env["HOME"], "LANG": "C.UTF-8"}
    listed, _ = gate.run("lane_list", [exe, "--list", "--format", "terse"], cwd=os.path.join(gate.repo, "mother/host"),
                         env=env)
    names = [x[:-len(": test")] for x in listed.stdout.decode().splitlines() if x.endswith(": test")]
    digest = sha256("\n".join(names).encode())
    expected = data["inventory"]
    gate.fact("lane_inventory", names == expected["names"] and digest == expected["sha256"]
              and sha256("\n".join(expected["names"]).encode()) == expected["sha256"],
              names=len(names), sha256=digest, expected=expected["sha256"])
    gate.run("lane_run", [exe], cwd=os.path.join(gate.repo, "mother/host"), env=env)
    gate.drop("target-lane")


def seams(gate, a3_ship, a3_census, a4):
    data = json.load(open(os.path.join(gate.repo, "tools/gatebox/route/route_seams.json"), encoding="utf-8"))
    measured = {}
    for label, verdict in (("a3_ship", a3_ship), ("a3_census", a3_census)):
        for result in (verdict or {}).get("results", []):
            for row in result.get("seams", []):
                measured.setdefault(row["seam"], {}).setdefault(label, {})[result.get("binary")] = row.get("present")
    for row in (a4 or {}).get("seams", []):
        measured.setdefault(row["seam"], {})["a4_installed_present"] = row.get("present")
    return [{"seam": s["name"], "dispositions": s["binaries"], "measured": measured.get(s["name"], {})}
            for s in data["seams"]]


def main():
    parser = argparse.ArgumentParser()
    for name in ("--repo", "--sha", "--toolchain", "--wrapper", "--wrapper-sha256", "--work", "--out"):
        parser.add_argument(name, required=True)
    parser.add_argument("--jobs", default="3")
    args = parser.parse_args()
    for name in ("repo", "toolchain", "wrapper", "work", "out"):
        setattr(args, name, os.path.abspath(getattr(args, name)))
    if os.path.lexists(args.work) or os.path.lexists(args.out):
        sys.exit(f"route_exit: the work directory and the verdict file must be fresh: {args.work}, {args.out}")
    gate, w = Gate(args), lambda name: os.path.join(args.work, name)
    data = json.load(open(DATA, encoding="utf-8"))
    head, tree = git(args.repo, "rev-parse", "HEAD"), git(args.repo, "rev-parse", "HEAD^{tree}")
    clean = status(args.repo) == ""
    files_tree = worktree_tree(args.repo, w("index-preflight"))
    wrapper_sha = sha256(open(args.wrapper, "rb").read())
    gate.fact("preflight", head == args.sha and clean and tree is not None and files_tree == tree
              and wrapper_sha == args.wrapper_sha256, head=head, tree=tree, clean=clean, files_tree=files_tree,
              wrapper_sha256=wrapper_sha)
    if not gate.steps[-1]["pass"]:
        return finish(args, gate, head, tree, [])
    t, r = args.toolchain, args.repo
    gate.tool("a3_check_data", "route_a3.py", "check-data", "--repo", r, "--toolchain", t)
    gate.cargo("std_tests", "target-std", "test", "--locked", "--offline", "--no-fail-fast", "-p", "nexus-governed-control",
               "-p", "nexus-mother", "-p", "nexus-mother-host")
    desktop = [x for test in data["desktop_tests"] for x in ("--test", test)]
    gate.cargo("std_tests_desktop", "target-std", "test", "--locked", "--offline", "--no-fail-fast", "-p", "nexus-mother-desktop",
               "--lib", "--bins", *desktop)
    gate.drop("target-std")
    gate.cargo("fmt", "target-fmt", "fmt", "--all", "--", "--check")
    lane(gate, data)
    gate.tool("a1", "route_a1.py", "--repo", r, "--toolchain", t, "--json", w("a1.json"))
    gate.tool("a1_controls", "route_a1_controls.py", "--source", r, "--work", w("a1-work"), "--toolchain", t, ok=controls_ok)
    gate.tool("a2_controls", "route_a2_controls.py", "--source", r, "--work", w("a2-work"), "--jobs", args.jobs, ok=controls_ok)
    gate.drop("a1-work", "a2-work")
    _, ship = gate.tool("a3_ship", "route_a3.py", "build", "--repo", r, "--toolchain", t, "--target", w("a3-ship"))
    _, census = gate.tool("a3_census", "route_a3.py", "build", "--repo", r, "--toolchain", t, "--target",
                          w("a3-census"), "--wrapper", args.wrapper)
    gate.drop("a3-ship", "a3-census")
    gate.tool("a3_controls", "route_a3_controls.py", "--source", r, "--work", w("a3-work"), "--toolchain", t,
              "--wrapper", args.wrapper, "--jobs", args.jobs, ok=controls_ok)
    gate.drop("a3-work")
    _, a4 = gate.tool("a4", "route_a4.py", "run", "--repo", r, "--toolchain", t, "--work", w("a4-work"))
    gate.tool("a4_controls", "route_a4.py", "controls", "--repo", r, "--toolchain", t, "--work", w("a4-controls"),
              "--wrapper", args.wrapper)
    gate.drop("a4-work", "a4-controls")
    gate.tool("a5", "route_a5.py", "check", "--repo", r, "--toolchain", t)
    gate.tool("a5_controls", "route_a5.py", "controls", "--repo", r, "--toolchain", t, "--work", w("a5-work"))
    gate.tool("a6", "route_a6.py", "run", "--repo", r, "--toolchain", t, "--work", w("a6-work"))
    gate.tool("a6_control", "route_a6.py", "control", "--repo", r, "--toolchain", t, "--work", w("a6-control"))
    gate.drop("a5-work", "a6-work", "a6-control")
    gate.tool("a7", "route_a7.py", "run", "--repo", r, "--target", w("a7-target"), "--evidence", w("a7"),
              "--wrapper", args.wrapper, "--wrapper-sha256", args.wrapper_sha256, "--toolchain", t,
              "--json", w("a7.json"), ok=lambda run, parsed: run.returncode == 0)
    gate.drop("a7-target")
    with open(w("python_wrapper"), "w", encoding="utf-8") as handle:
        handle.write(PY_WRAPPER)
    os.chmod(w("python_wrapper"), 0o755)
    gate.tool("a7_controls", "route_a7_controls.py", "--source", r, "--work", w("a7-controls"),
              "--wrapper-source", os.path.join(HERE, "route_rustc.c"), "--toolchain", t,
              "--python-wrapper", w("python_wrapper"), "--jobs", args.jobs, ok=a7_controls_ok)
    gate.drop("a7-controls")
    _, ran = gate.tool("plants", "route_plants.py", "run", "--repo", r, "--toolchain", t, "--work", w("plants-work"),
                       "--wrapper", args.wrapper, "--log", w("plants.jsonl"))
    plants_file = open(os.path.join(HERE, "route_plants.json"), "rb").read()
    names = [p["name"] for p in json.loads(plants_file)["plants"]]
    pin, ran = data["plants"], ran or {}
    bare = ran.get("without_expect", [])
    gate.fact("plants_list", len(names) == pin["count"] == ran.get("plants") == ran.get("killed") and [
        len(bare), sha256("\n".join(names).encode()), sha256("\n".join(bare).encode()), sha256(plants_file)] == [
        pin["without_expect_count"], pin["names_sha256"], pin["without_expect_sha256"], pin["file_sha256"]],
        count=len(names))
    gate.drop("plants-work")
    caches = [os.path.relpath(d, r) for d, _, _ in os.walk(os.path.join(r, "tools")) if os.path.basename(d) == "__pycache__"]
    gate.fact("no_pycache", not caches, found=caches[:5])
    after = git(r, "rev-parse", "HEAD"), status(r), worktree_tree(r, w("index-postflight"))
    gate.fact("postflight", after == (head, "", tree), head=after[0], files_tree=after[2])
    return finish(args, gate, head, tree, seams(gate, ship, census, a4))


def finish(args, gate, head, tree, seam_rows):
    passed = bool(gate.steps) and all(s["pass"] for s in gate.steps)
    out = {"gate": "SB-3E U4 one-SHA exit", "sha": args.sha, "head": head, "tree": tree,
           "wrapper_sha256": args.wrapper_sha256, "toolchain": args.toolchain, "steps": gate.steps,
           "seams": seam_rows, "verdict": "pass" if passed else "fail"}
    with open(args.out, "w", encoding="utf-8") as handle:
        json.dump(out, handle, indent=1, sort_keys=True)
    print(json.dumps({"verdict": out["verdict"], "failed": [s["step"] for s in gate.steps if not s["pass"]]}))
    return 0 if passed else 1


if __name__ == "__main__":
    sys.exit(main())

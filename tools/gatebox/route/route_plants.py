#!/usr/bin/python3 -I
"""SB-3E U4 plants (mission U4b item 7; SB3E-R37, R41; GB4a, SB3E-R94.3): every U4a plant, rerun on one tree.

  run --repo R --toolchain T --work DIR --wrapper W [--only NAME ...] [--log FILE]
  selftest --work DIR

route_plants.json lists each plant: an exact replacement (`old` once, `new`), an
`append`, a new file (`create`) or a new symlink (`symlink`, its target; SB3E-R55.2),
its mode, and what must kill it. In a scratch copy of the
tracked files each plant is applied, checked by sha256, run, restored, and the
copy's git tree hash (its own git directory) must be unchanged after it.

  guard   governed-control's route guards (`cargo test -p nexus-governed-control
          --lib sb3e_`): killed by a failed test or a refused build.
  mother  `c1_s1_session_sources_have_no_model_route` in nexus-mother: as guard.
  mbin    Mother's route guard binary, built once from the unplanted copy (it
          reads the planted sources at run time): every `tests` name must fail
          and every `expect` finding must appear in its panic messages. GB4a
          (SB3E-R87.3, F-PLANTS-MBIN-STALE): the binary is copied at once to a
          private path under DIR that no cargo command writes (mother and mguard
          rows rebuild cargo's own copy from the planted tree); before every mbin
          row its sha256 must equal the unplanted build's, else the row is not
          run and a finding is recorded.
  mguard  Mother's route guard rebuilt from the planted copy (data it
          includes): as mbin.
  lane    the route lane (the wrapper sets the cfg), built whole and run with
          only the plant's listed `tests` (`--exact`, each name; SB3E-R94.3
          Q3a): every `tests` name must fail (each failed test's own child
          reason kept, whole: SB3E-R97.3(a)).

Each `expect` finding (R44.4) must appear in the plant's own failure text (lane: its
child reasons) and a refused build then never kills; R53: all plants but two carry one.

SB3E-R94.3 Q3b: rows with an identical mutation (file and old/new, append, create
or symlink) and mode share one apply, build, run and restore, at the first such
row; each row is still judged on its own `tests` and `expect` (a lane result is
first cut to the row's own listed tests), the tree and the file are re-checked at
its own position, and its log record names the shared run (`shared_run`). Rows
are reported in the file's order. Q3c: the cargo environment sets
CARGO_CACHE_AUTO_CLEAN_FREQUENCY=never.

Exit 0 when every plant is killed as listed; one JSON verdict on stdout.
`selftest` runs the runner's own controls (C-ENV, C-MBIN, C-LISTED, C-SHARED) on
fake cargo and fake executables under a fresh DIR: no cargo, no repository.
"""
import argparse
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import route_a4  # noqa: E402  (scratch_copy, env_block)

DATA = os.path.join(HERE, "route_plants.json")
FAILED = re.compile(r"^test (\S+) \.\.\. FAILED", re.M)
MODES = ("guard", "mother", "mbin", "mguard", "lane")
MUTATION = ("old", "new", "append", "create", "symlink")


def sha(path):
    with open(path, "rb") as handle:
        return hashlib.sha256(handle.read()).hexdigest()


class Runner:
    def __init__(self, tree, toolchain, work, wrapper):
        self.tree, self.wrapper = tree, wrapper
        self.cargo = os.path.join(toolchain, "bin", "cargo")
        # SB3E-R94.3 Q3c (F-CARGO-GC, R58): here, not in route_a4.env_block (A4 and A6 import it too).
        self.env = route_a4.env_block(toolchain, {"CARGO_TARGET_DIR": os.path.join(work, "target"),
                                                  "CARGO_CACHE_AUTO_CLEAN_FREQUENCY": "never"})
        self.run_env = {"PATH": "/usr/bin:/bin", "HOME": self.env["HOME"], "LANG": "C.UTF-8"}
        self.git = ["git", f"--git-dir={os.path.join(work, 'tree.git')}", f"--work-tree={tree}"]
        self.private = os.path.join(work, "mbin")
        self.mbin = self.mbin_sha256 = None

    def tree_hash(self):
        subprocess.run(self.git + ["read-tree", "--empty"], check=True)
        subprocess.run(self.git + ["add", "-A", "-f", "."], cwd=self.tree, check=True)
        return subprocess.run(self.git + ["write-tree"], check=True, capture_output=True, text=True).stdout.strip()

    def cargo_test(self, args):
        run = subprocess.run([self.cargo, "test", "--locked", "--offline"] + args, cwd=self.tree, env=self.env,
                             capture_output=True, text=True)
        out = run.stdout + run.stderr
        if "error[" in out or "could not compile" in out:
            return {"build": "refused", "detail": re.findall(r"error\[[^\n]*", out)[:2]}
        return {"exit": run.returncode, "failed": FAILED.findall(out), "why": messages(out)}

    def guard(self):
        return self.cargo_test(["-p", "nexus-governed-control", "--lib", "sb3e_"])

    def mother(self):
        return self.cargo_test(["-p", "nexus-mother", "--lib", "c1_s1_session_sources_have_no_model_route"])

    def mguard(self):
        return self.cargo_test(["-p", "nexus-mother", "--lib", "route_seams_guard"])

    def build_mbin(self):
        """The unplanted guard binary, copied to a private path no cargo command writes (GB4a, R87.3)."""
        run = subprocess.run([self.cargo, "test", "--locked", "--offline", "-p", "nexus-mother", "--lib", "--no-run",
                              "--message-format=json"], cwd=self.tree, env=self.env, capture_output=True, text=True)
        built = None
        for line in run.stdout.splitlines():
            row = json.loads(line) if line.startswith("{") else {}
            if row.get("reason") == "compiler-artifact" and row.get("executable") \
                    and row["target"]["name"] == "nexus_mother":
                built = row["executable"]
        if built is None:
            return False
        os.mkdir(self.private, 0o700)
        private = os.path.join(self.private, os.path.basename(built))
        shutil.copyfile(built, private)
        os.chmod(private, 0o500)
        digest = sha(built)
        if sha(private) != digest:
            return False
        self.mbin, self.mbin_sha256 = private, digest
        return True

    def run_mbin(self):
        run = subprocess.run([self.mbin, "route_seams_guard"], cwd=os.path.join(self.tree, "mother"),
                             env=self.run_env, capture_output=True, text=True)
        out = run.stdout + run.stderr
        return {"exit": run.returncode, "failed": FAILED.findall(out), "why": messages(out)}

    def checked_mbin(self):
        """G2: the private binary runs only while its sha256 is the unplanted build's."""
        if self.mbin is None:
            return {"refused": "no unplanted guard binary", "mbin_sha256": None}
        seen = sha(self.mbin)
        if seen != self.mbin_sha256:
            return {"refused": "mbin_sha256", "mbin_sha256": seen}
        return dict(self.run_mbin(), mbin_sha256=seen)

    def lane(self, tests):
        """Built whole; run with only `tests` (Q3a), or whole when no test is listed."""
        run = subprocess.run([self.cargo, "--config", f'build.rustc-workspace-wrapper="{self.wrapper}"', "test",
                              "--release", "--locked", "--offline", "-p", "nexus-mother-host", "-p", "nexus-mother-desktop",
                              "--test", "route_cells", "--no-run", "--message-format=json"],
                             cwd=self.tree, env=self.env, capture_output=True, text=True)
        exe = None
        for line in run.stdout.splitlines():
            row = json.loads(line) if line.startswith("{") else {}
            if row.get("reason") == "compiler-artifact" and "route_cells" in (row.get("executable") or ""):
                exe = row["executable"]
        if run.returncode != 0 or not exe:
            return {"build": "refused", "detail": re.findall(r"error\[[^\n]*", run.stderr)[:2]}
        x = subprocess.run([exe] + (["--exact"] + list(tests) if tests else []),
                           cwd=os.path.join(self.tree, "mother/host"), env=self.run_env, capture_output=True, text=True)
        out = x.stdout + x.stderr
        failed = sorted(set(FAILED.findall(out)))
        why = {}
        for test in failed:
            section = out.split(f"---- {test} stdout ----", 1)
            section = section[1].split("\n---- ", 1)[0] if len(section) > 1 else ""
            child = re.findall(r"panicked at ((?!mother/host/tests/route_cells/main\.rs:)[^\n]*):\n([^\n]*)", section)
            # SB3E-R97.3(a): judged on the whole child reason (a cut one hid a row's own finding, F-PLANTS-BORROWED).
            why[test] = {"child": [(a, b.strip()) for a, b in child],
                         "result": re.findall(r"TIMEOUT [^\n]*", section)}
        return {"exit": x.returncode, "passed": len(re.findall(r"^test \S+ \.\.\. ok", out, re.M)),
                "failed": failed, "why": why}

    def execute(self, mode, tests):
        if mode == "mbin":
            return self.checked_mbin()
        return self.lane(tests) if mode == "lane" else getattr(self, mode)()


def messages(out):
    """Each failed test's whole panic message (the Mother guard lists every finding)."""
    return [w.strip() for w in re.findall(r"panicked at [^\n]*\n(.*?)(?=\nnote: |\n\n|\Z)", out, re.S)]


def judge(plant, result):
    failed = result.get("failed", [])
    if plant["mode"] in ("guard", "mother") and not plant.get("expect"):
        return bool(failed) or "build" in result, []
    missing = [t for t in plant.get("tests", []) if t not in failed]
    why = result.get("why", [])
    text = "\n".join(why) if isinstance(why, list) else json.dumps(why, ensure_ascii=False)
    missing += [e for e in plant.get("expect", []) if e not in text]
    return (bool(failed) and not missing), missing


def own_view(plant, result):
    """G4: a lane result cut to the row's own listed tests, so a shared run lends no row another's failure."""
    tests = plant.get("tests")
    if plant["mode"] != "lane" or not tests or "build" in result:
        return result
    return dict(result, failed=[t for t in result.get("failed", []) if t in tests],
                why={t: w for t, w in result.get("why", {}).items() if t in tests})


def mutation(plant):
    """Q3b's sharing key: the file, the mode and the whole mutation."""
    return json.dumps([plant["file"], plant["mode"], {k: plant[k] for k in MUTATION if k in plant}], sort_keys=True)


def adds(plant):
    """A new file or symlink (not an edit of an existing file)."""
    return "create" in plant or "symlink" in plant


def apply_run(runner, plant, tests):
    """One apply, run and restore of the plant's mutation: (record, result); a record with `error` ran nothing."""
    path = os.path.join(runner.tree, plant["file"])
    added = adds(plant)
    if os.path.lexists(path) == added:
        return {"error": "file exists" if added else "no such file"}, None
    tree, before = runner.tree_hash(), None if added else sha(path)
    made, up = [], os.path.dirname(path)
    while not os.path.lexists(up):
        made.append(up)
        up = os.path.dirname(up)
    if "symlink" in plant:
        os.makedirs(os.path.dirname(path), exist_ok=True)
        os.symlink(plant["symlink"], path)
        text = None
    else:
        text = "" if added else open(path, encoding="utf-8").read()
        if "create" in plant:
            planted = plant["create"]
        elif "append" in plant:
            planted = text + plant["append"]
        elif text.count(plant["old"]) == 1:
            planted = text.replace(plant["old"], plant["new"])
        else:
            return {"error": "old text not found exactly once"}, None
        os.makedirs(os.path.dirname(path), exist_ok=True)
        with open(path, "w", encoding="utf-8") as handle:
            handle.write(planted)
    record = {"mode": plant["mode"], "file": plant["file"], "tree_before": tree, "sha_before": before,
              "sha_planted": os.readlink(path) if text is None else sha(path)}
    try:
        result = runner.execute(plant["mode"], tests)
    finally:
        if added:
            os.unlink(path)
            for folder in made:
                os.rmdir(folder)
        else:
            with open(path, "w", encoding="utf-8") as handle:
                handle.write(text)
    record["restored"] = (not os.path.lexists(path) if added else sha(path) == before) and runner.tree_hash() == tree
    return record, result


def judged(runner, plant, ran, first, shared):
    """This row's own verdict on its (possibly shared) run."""
    record, result = ran
    row = {"name": plant["name"]}
    if shared:
        row["shared_run"] = shared
    if "error" in record:
        return dict(row, killed=False, error=record["error"])
    row.update(record)
    if not first:
        # G3 at this row's own position: the tree and the file are as the shared run found and left them.
        path = os.path.join(runner.tree, plant["file"])
        here = not os.path.lexists(path) if adds(plant) else os.path.lexists(path) and sha(path) == record["sha_before"]
        row["tree_before"] = runner.tree_hash()
        row["restored"] = record["restored"] and here and row["tree_before"] == record["tree_before"]
    fresh = True
    if plant["mode"] == "mbin":
        # G2 before every mbin row: the shared run's own check for its first row, a new check for every other.
        row["mbin_sha256"] = result.get("mbin_sha256") if first else sha(runner.mbin) if runner.mbin else None
        fresh = runner.mbin is not None and row["mbin_sha256"] == runner.mbin_sha256
    row["killed"], row["missing"] = judge(plant, own_view(plant, result))
    row["killed"] = row["killed"] and row["restored"] and fresh
    row["result"] = result
    return row


def run_plants(runner, plants, log=None):
    """Every plant, reported in the file's order: (rows, findings)."""
    findings, rows = [], []
    if any(p["mode"] == "mbin" for p in plants):
        unplanted = runner.checked_mbin() if runner.build_mbin() else {"build": "refused"}
        if unplanted.get("exit") != 0:
            findings.append({"kind": "unplanted_mother_guard_failed", "result": unplanted})
    groups, ran = {}, {}
    for plant in plants:
        groups.setdefault(mutation(plant), []).append(plant)
    for plant in plants:
        key = mutation(plant)
        members, first = groups[key], key not in ran
        if first:
            listed = []
            for member in members:
                listed += [t for t in member.get("tests", []) if t not in listed]
            whole = not all(m.get("tests") for m in members)
            ran[key] = apply_run(runner, plant, None if whole else listed)
        row = judged(runner, plant, ran[key], first, members[0]["name"] if len(members) > 1 else None)
        rows.append(row)
        if log:
            log.write(json.dumps(row, sort_keys=True) + "\n")
            log.flush()
        if plant["mode"] == "mbin" and runner.mbin is not None and "error" not in row \
                and row["mbin_sha256"] != runner.mbin_sha256:
            findings.append({"kind": "mbin_sha256_differs", "plant": plant["name"], "expected": runner.mbin_sha256,
                             "observed": row["mbin_sha256"]})
        if not row["killed"]:
            findings.append({"kind": "plant_survived", "plant": plant["name"], "missing": row.get("missing"),
                             "error": row.get("error")})
    return rows, findings


def verdict(tree_id, plants, rows, findings):
    out = {"tree": tree_id, "plants": len(rows), "killed": sum(r["killed"] for r in rows),
           "without_expect": sorted(p["name"] for p in plants if not p.get("expect")),
           "rows": [{k: r.get(k) for k in ("name", "mode", "killed", "missing")} for r in rows]}
    return dict(out, verdict="fail" if findings else "pass", findings=findings)


# selftest: the runner's own controls on a fake cargo and fake executables (no cargo, no repository).
FAKE_CARGO = r'''#!/usr/bin/python3 -I
import json, os, sys
work = os.path.dirname(os.environ["CARGO_TARGET_DIR"])
args = sys.argv[1:]
with open(os.path.join(work, "cargo.log"), "a", encoding="utf-8") as log:
    log.write(json.dumps({"args": args, "clean": os.environ.get("CARGO_CACHE_AUTO_CLEAN_FREQUENCY")}) + "\n")
def put(path, variant, kind):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    if os.path.exists(path):
        os.chmod(path, 0o700)
    with open(path, "w", encoding="utf-8") as handle:
        handle.write(open(os.path.join(work, kind + ".py"), encoding="utf-8").read().replace("@VARIANT@", variant))
    os.chmod(path, 0o500)
mother = os.path.join(os.environ["CARGO_TARGET_DIR"], "debug", "deps", "nexus_mother-0")
lane = os.path.join(os.environ["CARGO_TARGET_DIR"], "release", "deps", "route_cells-0")
if "route_cells" in args:
    put(lane, "lane", "fake_lane")
    print(json.dumps({"reason": "compiler-artifact", "executable": lane, "target": {"name": "route_cells"}}))
elif "--no-run" in args:
    put(mother, "unplanted", "fake_mother")
    print(json.dumps({"reason": "compiler-artifact", "executable": mother, "target": {"name": "nexus_mother"}}))
elif "route_seams_guard" in args:
    put(mother, "planted", "fake_mother")  # as an mguard row: cargo's own copy rebuilt from the planted tree
    if os.path.exists(os.path.join(work, "tamper")):
        for name in os.listdir(os.path.join(work, "mbin")):
            os.chmod(os.path.join(work, "mbin", name), 0o700)
            with open(os.path.join(work, "mbin", name), "a", encoding="utf-8") as handle:
                handle.write("# tampered\n")
    print("test oracles::route_seams_guard::g ... ok")
'''
FAKE_MOTHER = r'''#!/usr/bin/python3 -I
import os
with open(os.path.join(os.path.dirname(os.path.dirname(os.getcwd())), "mother-runs.log"), "a") as log:
    log.write("@VARIANT@\n")
if "@VARIANT@" == "planted":
    print("test oracles::route_seams_guard::g ... FAILED\n\nfailures:\n\n---- oracles::route_seams_guard::g stdout ----")
    print("thread 'g' panicked at src/oracles/route_seams_guard.rs:1:1:\nstale executable\nnote: run with RUST_BACKTRACE")
    raise SystemExit(101)
print("test oracles::route_seams_guard::g ... ok")
'''
FAKE_LANE = r'''#!/usr/bin/python3 -I
import os, sys
tree = os.path.dirname(os.path.dirname(os.getcwd()))
with open(os.path.join(os.path.dirname(tree), "lane-runs.log"), "a") as log:
    log.write(" ".join(sys.argv[1:]) + "\n")
planted = any("PLANTED" in open(os.path.join(tree, f)).read() for f in ("a.txt", "b.txt"))
reasons = {"cells::t_pass": None, "cells::t_fail": "reason fail", "cells::t_other": "reason other",
           "cells::t_long": "x" * 500 + " tail finding"}
names = sys.argv[sys.argv.index("--exact") + 1:] if "--exact" in sys.argv else list(reasons)
failed = [n for n in names if n in reasons and planted and reasons[n]]
for name in names:
    print(f"test {name} ... " + ("FAILED" if name in failed else "ok"))
for name in failed:
    print(f"\n---- {name} stdout ----\nthread '{name}' panicked at mother/host/tests/route_cells/x.rs:1:1:\n{reasons[name]}")
raise SystemExit(101 if failed else 0)
'''


def fake_runner(work):
    """A Runner over a small tree with its own git directory and a fake toolchain, all under `work`."""
    tree = os.path.join(work, "repo")
    os.makedirs(os.path.join(tree, "mother", "host"))
    os.makedirs(os.path.join(work, "toolchain", "bin"))
    files = (("repo/a.txt", "alpha\n"), ("repo/b.txt", "beta\n"), ("repo/mother/host/keep", ""),
             ("toolchain/bin/cargo", FAKE_CARGO), ("fake_mother.py", FAKE_MOTHER), ("fake_lane.py", FAKE_LANE))
    for name, text in files:
        with open(os.path.join(work, name), "w", encoding="utf-8") as handle:
            handle.write(text)
    os.chmod(os.path.join(work, "toolchain", "bin", "cargo"), 0o700)
    subprocess.run(["git", "init", "-q", "--bare", os.path.join(work, "tree.git")], check=True)
    runner = Runner(tree, os.path.join(work, "toolchain"), work, "/nonexistent/wrapper")
    return runner, runner.tree_hash()


def lines(path):
    if not os.path.exists(path):
        return []
    with open(path, encoding="utf-8") as handle:
        return handle.read().splitlines()


def fake(name, mode, file, tests, expect, old="alpha", new="alpha PLANTED"):
    return {"name": name, "mode": mode, "file": file, "old": old, "new": new, "tests": tests, "expect": expect}


def selftest(work):
    os.makedirs(work)
    checks, guard = [], ["oracles::route_seams_guard::g"]

    def check(name, ok, **detail):
        checks.append(dict(detail, control=name, ok=bool(ok)))

    # C-ENV (Q3c): the runner's environment, and what each cargo call it made received.
    w = os.path.join(work, "env")
    runner, _ = fake_runner(w)
    run_plants(runner, [fake("e1", "mguard", "a.txt", guard, ["x"]),
                        fake("e2", "lane", "b.txt", ["cells::t_fail"], ["x"], "beta", "beta PLANTED")])
    calls = [json.loads(x) for x in lines(os.path.join(w, "cargo.log"))]
    check("c_env", runner.env.get("CARGO_CACHE_AUTO_CLEAN_FREQUENCY") == "never" and len(calls) == 2
          and all(c["clean"] == "never" for c in calls), calls=calls)

    # C-MBIN (G2, R87.3): an mguard row rebuilds cargo's copy from the planted tree; the next mbin row runs the
    # private unplanted copy, hash-checked. Cargo's copy (the base runner's choice) would judge it stale.
    w = os.path.join(work, "mbin")
    runner, start = fake_runner(w)
    stale = fake("m2", "mbin", "b.txt", guard, ["stale executable"], "beta", "beta\n")
    rows, findings = run_plants(runner, [fake("m1", "mguard", "a.txt", guard, ["x"]), stale])
    cargo_copy = os.path.join(w, "target", "debug", "deps", "nexus_mother-0")
    hazard = subprocess.run([cargo_copy], cwd=os.path.join(runner.tree, "mother"), capture_output=True, text=True)
    check("c_mbin_private_unplanted", runner.mbin != cargo_copy and runner.mbin.startswith(runner.private + os.sep)
          and rows[1]["mbin_sha256"] == runner.mbin_sha256 and sha(cargo_copy) != runner.mbin_sha256
          and lines(os.path.join(w, "mother-runs.log")) == ["unplanted", "unplanted", "planted"]
          and not rows[1]["killed"] and rows[1]["missing"] == guard + ["stale executable"]
          and "stale executable" in hazard.stdout and runner.tree_hash() == start
          and not any(f["kind"] == "mbin_sha256_differs" for f in findings), killed=[r["killed"] for r in rows])
    w = os.path.join(work, "mbin-changed")
    runner, _ = fake_runner(w)
    open(os.path.join(w, "tamper"), "w").close()
    rows, findings = run_plants(runner, [fake("t1", "mguard", "a.txt", guard, ["x"]), dict(stale, name="t2")])
    check("c_mbin_changed_is_refused", rows[1]["result"].get("refused") == "mbin_sha256" and not rows[1]["killed"]
          and rows[1]["mbin_sha256"] != runner.mbin_sha256 and lines(os.path.join(w, "mother-runs.log")) == ["unplanted"]
          and any(f["kind"] == "mbin_sha256_differs" and f["plant"] == "t2" for f in findings))

    # C-LISTED (Q3a): only the listed tests run; a listed test that passes survives though an unlisted one fails.
    w = os.path.join(work, "listed")
    runner, start = fake_runner(w)
    rows, _ = run_plants(runner, [fake("l1", "lane", "a.txt", ["cells::t_pass"], ["reason fail"]),
                                  fake("l2", "lane", "b.txt", ["cells::t_fail"], ["reason fail"], "beta", "beta PLANTED")])
    check("c_listed", lines(os.path.join(w, "lane-runs.log")) == ["--exact cells::t_pass", "--exact cells::t_fail"]
          and not rows[0]["killed"] and rows[0]["missing"] == ["cells::t_pass", "reason fail"]
          and rows[1]["killed"] and rows[1]["missing"] == [] and runner.tree_hash() == start)

    # C-SHARED (Q3b, G3, G4): one run per identical mutation and mode, each row judged on its own tests and expect;
    # another mutation or mode is never shared; the tree is unchanged after the shared run. s6's own test fails, and
    # the shared run holds s5's "reason fail", which s6 must not be lent (G4).
    w = os.path.join(work, "shared")
    runner, start = fake_runner(w)
    rows, _ = run_plants(runner, [fake("s1", "lane", "a.txt", ["cells::t_fail"], ["reason fail"]),
                                  fake("s2", "lane", "a.txt", ["cells::t_fail"], ["an absent finding"]),
                                  fake("s3", "lane", "a.txt", ["cells::t_fail"], ["reason fail"], "alpha", "alpha PLANTED "),
                                  fake("s4", "mguard", "a.txt", guard, ["x"]),
                                  fake("s5", "lane", "b.txt", ["cells::t_fail"], ["reason fail"], "beta", "beta PLANTED"),
                                  fake("s6", "lane", "b.txt", ["cells::t_other"], ["reason fail"], "beta", "beta PLANTED")])
    runs = lines(os.path.join(w, "lane-runs.log"))
    check("c_shared", runs == ["--exact cells::t_fail", "--exact cells::t_fail", "--exact cells::t_fail cells::t_other"]
          and [r["killed"] for r in rows] == [True, False, True, False, True, False]
          and [r.get("shared_run") for r in rows] == ["s1", "s1", None, None, "s5", "s5"]
          and rows[1]["missing"] == ["an absent finding"] and rows[5]["missing"] == ["reason fail"]
          and all(r["restored"] for r in rows) and runner.tree_hash() == start
          and rows[1]["tree_before"] == rows[0]["tree_before"] == start, runs=runs)

    # SB3E-R97.4 (F-PLANTS-BORROWED, G4): an expect carried only by an unlisted failing test of the (shared) run never
    # counts; and a row's own finding past 400 characters of its child reason does.
    w = os.path.join(work, "borrowed")
    runner, start = fake_runner(w)
    rows, _ = run_plants(runner, [fake("u1", "lane", "b.txt", ["cells::t_fail"], ["reason other"], "beta", "beta PLANTED"),
                                  fake("u2", "lane", "b.txt", ["cells::t_other"], ["reason other"], "beta", "beta PLANTED"),
                                  fake("u3", "lane", "a.txt", ["cells::t_long"], ["tail finding"])])
    check("c_borrowed_survives", lines(os.path.join(w, "lane-runs.log"))[0] == "--exact cells::t_fail cells::t_other"
          and rows[0]["result"]["failed"] == ["cells::t_fail", "cells::t_other"] and not rows[0]["killed"]
          and rows[0]["missing"] == ["reason other"] and rows[1]["killed"] and runner.tree_hash() == start)
    check("c_untruncated", rows[2]["killed"] and rows[2]["missing"] == [] and all(r["restored"] for r in rows))

    failed = [c["control"] for c in checks if not c["ok"]]
    print(json.dumps({"controls": len(checks), "failed": failed, "checks": checks,
                      "verdict": "fail" if failed else "pass"}, sort_keys=True))
    return 1 if failed else 0


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("command", choices=("run", "selftest"))
    for name in ("--repo", "--toolchain", "--wrapper"):
        parser.add_argument(name)
    parser.add_argument("--work", required=True)
    parser.add_argument("--only", action="append")
    parser.add_argument("--log")
    args = parser.parse_args()
    work = os.path.abspath(args.work)
    if os.path.lexists(work):
        sys.exit(f"route_plants: the work directory must be fresh: {work}")
    if args.command == "selftest":
        return selftest(work)
    if not (args.repo and args.toolchain and args.wrapper):
        sys.exit("route_plants: run needs --repo, --toolchain and --wrapper")
    repo, toolchain, wrapper = map(os.path.abspath, (args.repo, args.toolchain, args.wrapper))
    plants = [p for p in json.load(open(DATA, encoding="utf-8"))["plants"] if not args.only or p["name"] in args.only]
    unknown = sorted({p["mode"] for p in plants} - set(MODES))
    if unknown:
        sys.exit(f"route_plants: unknown modes {unknown}")
    os.makedirs(work)
    tree, tree_id = route_a4.scratch_copy(repo, work)
    runner = Runner(tree, toolchain, work, wrapper)
    log = open(args.log, "a", encoding="utf-8") if args.log else None
    rows, findings = run_plants(runner, plants, log)
    print(json.dumps(verdict(tree_id, plants, rows, findings), sort_keys=True))
    return 1 if findings else 0


if __name__ == "__main__":
    sys.exit(main())

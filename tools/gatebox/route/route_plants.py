#!/usr/bin/python3 -I
"""SB-3E U4 plants (mission U4b item 7; SB3E-R37, R41): every U4a plant, rerun on one tree.

  run --repo R --toolchain T --work DIR --wrapper W [--only NAME ...] [--log FILE]

route_plants.json lists each plant: an exact replacement (`old` once, `new`) or an
`append` in one file, its mode, and what must kill it. In a scratch copy of the
tracked files each plant is applied, checked by sha256, run, restored, and the
copy's git tree hash (its own git directory) must be unchanged after it.

  guard   governed-control's route guards (`cargo test -p nexus-governed-control
          --lib sb3e_`): killed by a failed test or a refused build.
  mother  `c1_s1_session_sources_have_no_model_route` in nexus-mother: as guard.
  mbin    Mother's route guard binary, built once from the unplanted copy (it
          reads the planted sources at run time): every `tests` name must fail
          and every `expect` finding must appear in its panic messages.
  mguard  Mother's route guard rebuilt from the planted copy (data it
          includes): as mbin.
  lane    the route lane (the wrapper sets the cfg), built and run whole: every
          `tests` name must fail (each failed test's own child reason kept).

Each `expect` finding (R44.4) must appear in the plant's own failure text (lane: its
child reasons) and a refused build then never kills; R53: all plants but two carry one.

Exit 0 when every plant is killed as listed; one JSON verdict on stdout.
"""
import argparse
import hashlib
import json
import os
import re
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import route_a4  # noqa: E402  (scratch_copy, env_block)

DATA = os.path.join(HERE, "route_plants.json")
FAILED = re.compile(r"^test (\S+) \.\.\. FAILED", re.M)


def sha(path):
    with open(path, "rb") as handle:
        return hashlib.sha256(handle.read()).hexdigest()


class Runner:
    def __init__(self, tree, toolchain, work, wrapper):
        self.tree, self.wrapper = tree, wrapper
        self.cargo = os.path.join(toolchain, "bin", "cargo")
        self.env = route_a4.env_block(toolchain, {"CARGO_TARGET_DIR": os.path.join(work, "target")})
        self.run_env = {"PATH": "/usr/bin:/bin", "HOME": self.env["HOME"], "LANG": "C.UTF-8"}
        self.git = ["git", f"--git-dir={os.path.join(work, 'tree.git')}", f"--work-tree={tree}"]
        self.mbin = None

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
        run = subprocess.run([self.cargo, "test", "--locked", "--offline", "-p", "nexus-mother", "--lib", "--no-run",
                              "--message-format=json"], cwd=self.tree, env=self.env, capture_output=True, text=True)
        for line in run.stdout.splitlines():
            row = json.loads(line) if line.startswith("{") else {}
            if row.get("reason") == "compiler-artifact" and row.get("executable") \
                    and row["target"]["name"] == "nexus_mother":
                self.mbin = row["executable"]
        return self.mbin is not None

    def run_mbin(self):
        run = subprocess.run([self.mbin, "route_seams_guard"], cwd=os.path.join(self.tree, "mother"),
                             env=self.run_env, capture_output=True, text=True)
        out = run.stdout + run.stderr
        return {"exit": run.returncode, "failed": FAILED.findall(out), "why": messages(out)}

    def lane(self):
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
        x = subprocess.run([exe], cwd=os.path.join(self.tree, "mother/host"), env=self.run_env,
                           capture_output=True, text=True)
        out = x.stdout + x.stderr
        failed = sorted(set(FAILED.findall(out)))
        why = {}
        for test in failed:
            section = out.split(f"---- {test} stdout ----", 1)
            section = section[1].split("\n---- ", 1)[0] if len(section) > 1 else ""
            child = re.findall(r"panicked at ((?!mother/host/tests/route_cells/main\.rs:)[^\n]*):\n([^\n]*)", section)
            why[test] = {"child": [(a, b.strip()[:400]) for a, b in child],
                         "result": re.findall(r"TIMEOUT [^\n]*", section)}
        return {"exit": x.returncode, "passed": len(re.findall(r"^test \S+ \.\.\. ok", out, re.M)),
                "failed": failed, "why": why}


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


def run_one(runner, plant):
    path = os.path.join(runner.tree, plant["file"])
    tree, before = runner.tree_hash(), sha(path)
    text = open(path, encoding="utf-8").read()
    if "append" in plant:
        planted = text + plant["append"]
    elif text.count(plant["old"]) == 1:
        planted = text.replace(plant["old"], plant["new"])
    else:
        return {"name": plant["name"], "killed": False, "error": "old text not found exactly once"}
    with open(path, "w", encoding="utf-8") as handle:
        handle.write(planted)
    row = {"name": plant["name"], "mode": plant["mode"], "file": plant["file"], "tree_before": tree,
           "sha_before": before, "sha_planted": sha(path)}
    try:
        mode = plant["mode"]
        result = runner.run_mbin() if mode == "mbin" else getattr(runner, mode)()
    finally:
        with open(path, "w", encoding="utf-8") as handle:
            handle.write(text)
    row["restored"] = sha(path) == before and runner.tree_hash() == tree
    row["killed"], row["missing"] = judge(plant, result)
    row["killed"] = row["killed"] and row["restored"]
    row["result"] = result
    return row


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("command", choices=("run",))
    for name in ("--repo", "--toolchain", "--work", "--wrapper"):
        parser.add_argument(name, required=True)
    parser.add_argument("--only", action="append")
    parser.add_argument("--log")
    args = parser.parse_args()
    repo, toolchain, work, wrapper = map(os.path.abspath, (args.repo, args.toolchain, args.work, args.wrapper))
    if os.path.lexists(work):
        sys.exit(f"route_plants: the work directory must be fresh: {work}")
    os.makedirs(work)
    tree, tree_id = route_a4.scratch_copy(repo, work)
    plants = [p for p in json.load(open(DATA, encoding="utf-8"))["plants"] if not args.only or p["name"] in args.only]
    runner = Runner(tree, toolchain, work, wrapper)
    findings, rows = [], []
    if any(p["mode"] == "mbin" for p in plants):
        unplanted = runner.run_mbin() if runner.build_mbin() else {"build": "refused"}
        if unplanted.get("exit") != 0:
            findings.append({"kind": "unplanted_mother_guard_failed", "result": unplanted})
    log = open(args.log, "a", encoding="utf-8") if args.log else None
    for plant in plants:
        row = run_one(runner, plant)
        rows.append(row)
        if log:
            log.write(json.dumps(row, sort_keys=True) + "\n")
            log.flush()
        if not row["killed"]:
            findings.append({"kind": "plant_survived", "plant": plant["name"], "missing": row.get("missing"),
                             "error": row.get("error")})
    out = {"tree": tree_id, "plants": len(rows), "killed": sum(r["killed"] for r in rows),
           "without_expect": sorted(p["name"] for p in plants if not p.get("expect")),
           "rows": [{k: r.get(k) for k in ("name", "mode", "killed", "missing")} for r in rows]}
    print(json.dumps(dict(out, verdict="fail" if findings else "pass", findings=findings), sort_keys=True))
    return 1 if findings else 0


if __name__ == "__main__":
    sys.exit(main())

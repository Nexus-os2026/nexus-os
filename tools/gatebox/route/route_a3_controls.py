#!/usr/bin/python3 -I
"""SB-3E A3 controls (design section 5 row A3; SB3E-R28.1, R33.3).

`--source TREE --work DIR --toolchain DIR --wrapper PATH [--jobs N]`. Each
control copies the source tree's tracked files to its own directory and
records that copy's git tree hash (through its own git directory) before any
edit. Every edit is exact: its text must occur once, and each edited file's
sha256 is recorded before and after.

- baseline: route_a3 `build` of every binary in the shipped configuration
  must pass (no marker, no unknown stem).
- census (--wrapper, the route lane's wrapper, is required: the census is
  the only measurement of the dispositions): the markers surviving a real
  release build with the cfg set must be exactly each binary's `reachable`
  seams.
- every control in route_seams.json: its ungate sets and edits (a call on a
  path reachable from the binary's entry), a real link of that binary, and
  the scan must report each `expect` seam (and, for `expect_unknown`, report
  no seam at all and an unknown stem beginning with each given text).
- data controls (no build): `check-data` must pass on the copy as it is and
  refuse each planted defect: a duplicate seam name, a bad marker format, a
  marker that is a substring of another, a binary named in no disposition, a
  marker not defined exactly once in its source, an unlisted marker in route
  source, an unlisted bin target in a route package, and no shipped binary.
Adding a seam needs only data: its row, its ungate set and its control.
"""
import argparse
import concurrent.futures
import hashlib
import json
import os
import shutil
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
DATA = "tools/gatebox/route/route_seams.json"


def tracked(source):
    run = subprocess.run(["git", "-C", source, "ls-files", "-z"], capture_output=True, check=True)
    return [x.decode() for x in run.stdout.split(b"\0") if x]


def copy(source, files, tree):
    for rel in files:
        path = os.path.join(source, rel)
        if os.path.islink(path) or not os.path.isfile(path):
            continue
        os.makedirs(os.path.dirname(os.path.join(tree, rel)), exist_ok=True)
        shutil.copy2(path, os.path.join(tree, rel))


def tree_hash(tree, git_dir):
    subprocess.run(["git", "init", "-q", "--bare", git_dir], check=True)
    git = ["git", f"--git-dir={git_dir}", f"--work-tree={tree}"]
    subprocess.run(git + ["add", "-A", "-f", "."], cwd=tree, check=True)
    return subprocess.run(git + ["write-tree"], capture_output=True, text=True, check=True).stdout.strip()


def sha256_file(path):
    with open(path, "rb") as handle:
        return hashlib.sha256(handle.read()).hexdigest()


def edit(tree, rel, old, new):
    path = os.path.join(tree, rel)
    with open(path, encoding="utf-8") as handle:
        text = handle.read()
    if text.count(old) != 1:
        raise ValueError(f"{rel}: {text.count(old)} occurrences of {old!r}")
    before = sha256_file(path)
    with open(path, "w", encoding="utf-8") as handle:
        handle.write(text.replace(old, new))
    return {"file": rel, "before": before, "after": sha256_file(path)}


def a3(args, env):
    run = subprocess.run(["/usr/bin/python3", "-I", os.path.join(HERE, "route_a3.py")] + args, env=env,
                         capture_output=True, text=True)
    try:
        return run.returncode, json.loads(run.stdout)
    except json.JSONDecodeError:
        return run.returncode, {"verdict": "error", "findings": [], "results": [], "stderr": run.stderr[-2000:]}


def present(verdict):
    return sorted({r["seam"] for res in verdict["results"] for r in res["seams"] if r["present"]})


def unknown(verdict):
    return sorted({u["after_stem"] for res in verdict["results"] for u in res["unknown"]})


def run_control(spec, ctx):
    name, root = spec["name"], os.path.join(ctx["work"], spec["name"])
    tree = os.path.join(root, "repo")
    copy(ctx["source"], ctx["files"], tree)
    result = {"control": name, "tree": tree_hash(tree, os.path.join(root, "tree.git")), "edits": []}
    try:
        for change in spec.get("plant", []):
            result["edits"].append(change(tree))
        data = json.load(open(os.path.join(tree, DATA), encoding="utf-8"))
        for group in spec.get("ungate", []):
            for e in data["ungates"][group]:
                result["edits"].append(edit(tree, e["file"], e["old"], e["new"]))
        for e in spec.get("edits", []):
            result["edits"].append(edit(tree, e["file"], e["old"], e["new"]))
    except (ValueError, KeyError, OSError) as error:
        shutil.rmtree(root)
        return dict(result, ok=False, error=str(error))
    if spec["kind"] == "data":
        code, verdict = a3(["check-data", "--repo", tree, "--toolchain", ctx["toolchain"]], ctx["env"])
        reasons = sorted({f.get("reason", f["kind"]) for f in verdict["findings"]})
        want = spec["reason"]
        ok = code == 0 and not reasons if want is None else code == 1 and any(want in r for r in reasons)
        result.update(kind="data", exit=code, reported=reasons, ok=ok)
    else:
        argv = ["build", "--repo", tree, "--toolchain", ctx["toolchain"], "--target", os.path.join(root, "target")]
        argv += ["--wrapper", ctx["wrapper"]] if spec.get("census") else []
        argv += ["--only", spec["binary"]] if spec.get("binary") else []
        code, verdict = a3(argv, ctx["env"])
        got, odd = present(verdict), unknown(verdict)
        result.update(kind="real link", exit=code, present=got, unknown=odd,
                      findings=[f["kind"] for f in verdict["findings"]], stderr=verdict.get("stderr"))
        if spec.get("baseline"):
            result["ok"] = code == 0 and not got and not odd
        elif spec.get("census"):
            result["ok"] = code == 0
        elif "expect_unknown" in spec:
            result["ok"] = (code == 1 and not got and all(any(u.startswith(x) for u in odd)
                                                          for x in spec["expect_unknown"]))
        else:
            result["ok"] = code == 1 and set(spec["expect"]) <= set(got) and "build_failed" not in result["findings"]
    shutil.rmtree(root)
    return result


def plant_data(mutate):
    def change(tree):
        path = os.path.join(tree, DATA)
        data = json.load(open(path, encoding="utf-8"))
        mutate(data)
        before = sha256_file(path)
        with open(path, "w", encoding="utf-8") as handle:
            json.dump(data, handle)
        return {"file": DATA, "before": before, "after": sha256_file(path)}
    return change


def append_to(rel, text):
    def change(tree):
        path = os.path.join(tree, rel)
        before = sha256_file(path)
        with open(path, "a", encoding="utf-8") as handle:
            handle.write(text)
        return {"file": rel, "before": before, "after": sha256_file(path)}
    return change


def add_row(**fields):
    return lambda data: data["seams"].append(dict(data["seams"][0], **fields))


def drop_disposition(data):
    data["seams"][0]["binaries"].pop(sorted(data["binaries"])[0])


def unship(data):
    for spec in data["binaries"].values():
        spec["shipped"] = None


def controls(data):
    second = data["seams"][1]
    out = [{"name": "data_baseline", "kind": "data", "reason": None},
           {"name": "data_duplicate_seam_name", "kind": "data", "reason": "duplicate seam name",
            "plant": [plant_data(add_row())]},
           {"name": "data_bad_marker_format", "kind": "data", "reason": "not the stem and a name",
            "plant": [plant_data(add_row(name="planted", marker=data["stem"].rstrip("-") + "_S9"))]},
           {"name": "data_marker_substring_of_another", "kind": "data", "reason": "substring",
            "plant": [plant_data(add_row(name="planted", marker=data["stem"] + "S4"))]},
           {"name": "data_binary_named_in_no_disposition", "kind": "data", "reason": "not every binary named",
            "plant": [plant_data(drop_disposition)]},
           {"name": "data_marker_defined_twice", "kind": "data", "reason": "marker not defined once",
            "plant": [append_to(second["source"], f'\nconst M: &str = "{second["marker"]}";\n')]},
           {"name": "data_no_shipped_binary", "kind": "data", "reason": "not exactly one binary shipped",
            "plant": [plant_data(unship)]},
           {"name": "data_unlisted_marker_in_route_source", "kind": "data", "reason": "no listed marker",
            "plant": [append_to("mother/src/lib.rs", f'\n// "{data["stem"]}S9"\n')]},
           {"name": "data_unlisted_bin_target", "kind": "data", "reason": "bin targets",
            "plant": [append_to("mother/Cargo.toml", '\n[[bin]]\nname = "planted"\npath = "src/lib.rs"\n')]},
           {"name": "baseline_shipped_build", "kind": "build", "baseline": True},
           {"name": "census_cfg_set", "kind": "build", "census": True}]
    return out + [dict(c, kind="build") for c in data["controls"]]


def main():
    parser = argparse.ArgumentParser()
    for name in ("--source", "--work", "--toolchain"):
        parser.add_argument(name, required=True)
    parser.add_argument("--wrapper", required=True, help="the route lane's wrapper; the census needs it")
    parser.add_argument("--jobs", type=int, default=2)
    parser.add_argument("--only", action="append")
    args = parser.parse_args()
    source, work = os.path.abspath(args.source), os.path.abspath(args.work)
    if os.path.lexists(work):
        sys.exit(f"route_a3_controls: the work directory must be fresh: {work}")
    if not os.access(os.path.abspath(args.wrapper), os.X_OK):
        sys.exit(f"route_a3_controls: the wrapper is not an executable file: {args.wrapper}")
    os.makedirs(work)
    env = {"HOME": os.environ["HOME"], "PATH": "/usr/bin:/bin", "LANG": "C.UTF-8"}
    ctx = {"source": source, "work": work, "files": tracked(source), "toolchain": os.path.abspath(args.toolchain),
           "wrapper": os.path.abspath(args.wrapper), "env": env}
    data = json.load(open(os.path.join(source, DATA), encoding="utf-8"))
    specs = [c for c in controls(data) if not args.only or c["name"] in args.only]
    with concurrent.futures.ThreadPoolExecutor(max_workers=max(1, args.jobs)) as pool:
        results = list(pool.map(lambda c: run_control(c, ctx), specs))
    for result in results:
        print(json.dumps(result, sort_keys=True))
    head_tree = subprocess.run(["git", "-C", source, "rev-parse", "HEAD^{tree}"], capture_output=True, text=True,
                               check=True).stdout.strip()
    failed = [r["control"] for r in results if not r["ok"]]
    print(json.dumps({"controls": len(results), "failed": failed, "head_tree": head_tree,
                      "trees_match_head": all(r["tree"] == head_tree for r in results)}))
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())

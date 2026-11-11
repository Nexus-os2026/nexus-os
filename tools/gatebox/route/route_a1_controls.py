#!/usr/bin/python3 -I
"""SB-3E A1 presence controls (design section 5 row A1, U4b item 1).

`--source TREE --work DIR --toolchain DIR`. Each control copies the source
tree's tracked files into its own directory, plants one difference and runs
route_a1.py on that real repository copy. It passes only if A1 fails and
reports each of the control's expected findings; the unplanted baseline must
pass. Plants that need a new lock entry refresh the copy's Cargo.lock
offline, so the specific findings are reported beside the lock's.
"""
import argparse
import json
import os
import shutil
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
CFG = '["--cfg", "mother_route_seams"]'


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


def edit(tree, rel, old, new):
    path = os.path.join(tree, rel)
    text = open(path, encoding="utf-8").read()
    if text.count(old) != 1:
        sys.exit(f"control plant: {rel} has {text.count(old)} of {old!r}")
    open(path, "w", encoding="utf-8").write(text.replace(old, new))


def write(tree, rel, text):
    os.makedirs(os.path.dirname(os.path.join(tree, rel)), exist_ok=True)
    open(os.path.join(tree, rel), "w", encoding="utf-8").write(text)


def symlinked_cargo(tree):
    os.rename(os.path.join(tree, ".cargo"), os.path.join(tree, "cargo-real"))
    os.symlink("cargo-real", os.path.join(tree, ".cargo"))


HOST, MOTHER, DESKTOP = "mother/host/Cargo.toml", "mother/Cargo.toml", "mother/desktop/Cargo.toml"
DEPS = "[dependencies]\n"
# name: (plant, relock, [(kind, field, substring)] every one of which must be reported)
CONTROLS = {
    "baseline": (lambda t: None, False, []),
    "feature": (lambda t: write(t, HOST, open(os.path.join(t, HOST)).read() + "\n[features]\nseams = []\n"), False,
                [("route_package", "feature", "seams")]),
    "build_rs_printing_the_cfg": (
        lambda t: write(t, "mother/build.rs", 'fn main() {\n    println!("cargo:rustc-cfg=mother_route_seams");\n}\n'),
        False, [("route_package", "reason", "build script")]),
    "rustflags_config": (lambda t: edit(t, ".cargo/config.toml", "[build]\n", f"[build]\nrustflags = {CFG}\n"), False,
                         [("config", "sets", "build.rustflags"), ("config", "reason", "differs from its pin")]),
    "optional_renamed_dependency": (
        lambda t: edit(t, HOST, DEPS, DEPS + 'route-hook = { package = "hex", version = "0.4", optional = true }\n'),
        True, [("route_package", "dependency", "hex|route-hook|normal||True|"), ("route_package", "feature", "route-hook"),
               ("graph_differs", "resolve", "all-features"), ("lock", "reason", "Cargo.lock")]),
    "legacy_config_beside_config_toml": (
        lambda t: write(t, ".cargo/config", f"[build]\nrustflags = {CFG}\n"), False,
        [("config", "path", ".cargo/config"), ("config", "sets", "build.rustflags")]),
    "nested_rustc_wrapper": (lambda t: write(t, "mother/host/.cargo/config.toml", '[build]\nrustc-wrapper = "/usr/bin/env"\n'),
                             False, [("config", "sets", "build.rustc-wrapper")]),
    "nested_rustc_workspace_wrapper": (
        lambda t: write(t, "crates/nexus-governed-control/.cargo/config.toml",
                        '[build]\nrustc-workspace-wrapper = "/usr/bin/env"\n'),
        False, [("config", "sets", "build.rustc-workspace-wrapper")]),
    "symlinked_cargo_directory": (symlinked_cargo, False, [("config", "reason", "a symbolic link")]),
    "lock_changed": (lambda t: write(t, "Cargo.lock", open(os.path.join(t, "Cargo.lock")).read() + "# planted\n"), False,
                     [("lock", "reason", "Cargo.lock")]),
    "target_table_edge_outside_a_route_package": (
        lambda t: write(t, "kernel/Cargo.toml", open(os.path.join(t, "kernel/Cargo.toml")).read()
                        + "\n[target.'cfg(target_os = \"redox\")'.dependencies]\nunicode-segmentation = \"1.12\"\n"),
        True, [("graph_differs", "resolve", "default"), ("graph_differs", "resolve", "all-features"),
               ("lock", "reason", "Cargo.lock")]),
    "dev_edge": (lambda t: edit(t, DESKTOP, "[dev-dependencies]\n", '[dev-dependencies]\nhex = "0.4"\n'), True,
                 [("route_package", "dependency", "hex||dev|"), ("graph_differs", "resolve", "default")]),
    "build_edge": (lambda t: write(t, MOTHER, open(os.path.join(t, MOTHER)).read() + '\n[build-dependencies]\nhex = "0.4"\n'),
                   True, [("route_package", "dependency", "hex||build|"), ("graph_differs", "resolve", "default")]),
}


def reported(findings, kind, field, substring):
    for x in findings:
        value = x.get(field)
        value = " ".join(value) if isinstance(value, list) else str(value)
        if x["kind"] == kind and substring in value:
            return True
    return False


def run_control(name, source, files, work, toolchain, env):
    plant, relock, expected = CONTROLS[name]
    tree = os.path.join(work, name)
    copy(source, files, tree)
    plant(tree)
    if relock:
        bin_dir = os.path.join(toolchain, "bin")
        relock = dict(env, RUSTC=os.path.join(bin_dir, "rustc"), PATH=bin_dir + os.pathsep + env["PATH"])
        subprocess.run([os.path.join(bin_dir, "cargo"), "metadata", "--offline", "--format-version", "1"],
                       cwd=tree, env=relock, capture_output=True, check=True)
    run = subprocess.run(["/usr/bin/python3", "-I", os.path.join(HERE, "route_a1.py"), "--repo", tree,
                          "--toolchain", toolchain], env=env, capture_output=True, text=True)
    result = json.loads(run.stdout)
    missing = [list(e) for e in expected if not reported(result["findings"], *e)]
    if name == "baseline":
        ok = run.returncode == 0 and result["verdict"] == "pass"
    else:
        ok = run.returncode == 1 and result["verdict"] == "fail" and not missing
    shutil.rmtree(tree)
    return {"control": name, "ok": ok, "exit": run.returncode, "missing": missing,
            "reported": sorted({(x["kind"], x.get("reason", x.get("resolve", ""))) for x in result["findings"]})}


def main():
    parser = argparse.ArgumentParser()
    for name in ("--source", "--work", "--toolchain"):
        parser.add_argument(name, required=True)
    parser.add_argument("--only", action="append")
    args = parser.parse_args()
    source, work, toolchain = map(os.path.abspath, (args.source, args.work, args.toolchain))
    if os.path.lexists(work):
        sys.exit(f"route_a1_controls: the work directory must be fresh: {work}")
    os.makedirs(work)
    env = {"HOME": os.environ["HOME"], "PATH": "/usr/bin:/bin", "LANG": "C.UTF-8"}
    files = tracked(source)
    results = [run_control(name, source, files, work, toolchain, env) for name in CONTROLS
               if not args.only or name in args.only]
    for result in results:
        print(json.dumps(result, sort_keys=True))
    failed = [r["control"] for r in results if not r["ok"]]
    print(json.dumps({"controls": len(results), "failed": failed}))
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())

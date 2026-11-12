#!/usr/bin/python3 -I
"""SB-3E A1 presence controls (design section 5 row A1, U4b item 1).

`--source TREE --work DIR --toolchain DIR`. Each control copies the source
tree's tracked files into its own directory, plants one difference and runs
route_a1.py on that real repository copy. It passes only if A1 fails and
reports each of the control's expected findings; the unplanted baseline must
pass. Plants that need a new lock entry refresh the copy's Cargo.lock
offline, so the specific findings are reported beside the lock's. Each
result names the git tree hash of its copy before the plant. Every run passes
the copy's own `tools/route_pins.json` (SB3E GB3).

GB3-1 sentinel controls (mission 9 item 1a): a configuration plant runs with a
scratch toolchain whose `cargo` records a marker before it executes the real
cargo, or with a configured wrapper that records a marker; each passes only if
A1 fails with the plant's findings and no marker exists. The pins controls
(mission 9 item 2) refuse a bad or absent `--pins` the same way.
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


def wrapper_config(rel, key):
    return lambda t, wrapper="/usr/bin/env": write(t, rel, f'[build]\n{key} = "{wrapper}"\n')


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
    "legacy_config_setting_a_wrapper": (
        wrapper_config(".cargo/config", "rustc-wrapper"), False,
        [("config", "path", ".cargo/config"), ("config", "sets", "build.rustc-wrapper")]),
    "root_config_toml_deleted": (lambda t: os.remove(os.path.join(t, ".cargo", "config.toml")), False,
                                 [("config", "reason", "missing (it is pinned)")]),
    "nested_rustc_wrapper": (wrapper_config("mother/host/.cargo/config.toml", "rustc-wrapper"),
                             False, [("config", "sets", "build.rustc-wrapper")]),
    "nested_rustc_workspace_wrapper": (
        wrapper_config("crates/nexus-governed-control/.cargo/config.toml", "rustc-workspace-wrapper"),
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
PINS = "tools/route_pins.json"
# name: (base control, sentinel). The base control's plant and expected findings; `cargo` runs A1 with the
# marking cargo, `wrapper` names the marking wrapper in the plant's configuration.
SENTINELS = {
    "config_refused_no_metadata_legacy": ("legacy_config_beside_config_toml", "cargo"),
    "config_refused_no_metadata_nested": ("nested_rustc_wrapper", "cargo"),
    "config_refused_no_metadata_changed_root": ("rustflags_config", "cargo"),
    "config_refused_no_metadata_deleted_root": ("root_config_toml_deleted", "cargo"),
    "config_refused_no_metadata_symlinked": ("symlinked_cargo_directory", "cargo"),
    "config_wrapper_sentinel_legacy": ("legacy_config_setting_a_wrapper", "wrapper"),
    "config_wrapper_sentinel_nested": ("nested_rustc_wrapper", "wrapper"),
    "config_wrapper_sentinel_nested_workspace": ("nested_rustc_workspace_wrapper", "wrapper"),
}


def relinked_pins(tree):
    os.rename(os.path.join(tree, PINS), os.path.join(tree, PINS + ".real"))
    os.symlink("route_pins.json.real", os.path.join(tree, PINS))


# name: (plant, the --pins argument (None: absent), expected pins reason); each runs with the marking cargo.
PINS_CONTROLS = {
    "pins_missing": (lambda t: os.remove(os.path.join(t, PINS)), PINS, "missing or a symbolic link"),
    "pins_symlink": (relinked_pins, PINS, "missing or a symbolic link"),
    "pins_malformed": (lambda t: write(t, PINS, '{"lock_sha256": "' + "0" * 64 + '"}\n'), PINS, "pins schema"),
    "pins_oversize": (lambda t: write(t, PINS, open(os.path.join(t, PINS)).read() + " " * 65536), PINS, "over 65536 bytes"),
    "pins_other_path": (lambda t: shutil.copy(os.path.join(t, PINS), os.path.join(t, "route_pins.json")),
                        "route_pins.json", "checked tree's own"),
    "pins_absent": (lambda t: None, None, None),
}
MARKING = """#!/usr/bin/python3 -I
import os, sys
with open({marker!r}, "a") as handle:
    handle.write(" ".join(sys.argv) + "\\n")
argv = {argv}
os.execv(argv[0], argv)
"""


def marking(path, marker, argv):
    """A sentinel: it appends its argv to `marker`, then executes `argv`."""
    with open(path, "w", encoding="utf-8") as handle:
        handle.write(MARKING.format(marker=marker, argv=argv))
    os.chmod(path, 0o755)


def reported(findings, kind, field, substring):
    for x in findings:
        value = x.get(field)
        value = " ".join(value) if isinstance(value, list) else str(value)
        if x["kind"] == kind and substring in value:
            return True
    return False


def tree_hash(tree, git_dir):
    """The git tree hash of a copy, through its own git directory (never the shared one)."""
    subprocess.run(["git", "init", "-q", "--bare", git_dir], check=True)
    git = ["git", f"--git-dir={git_dir}", f"--work-tree={tree}"]
    subprocess.run(git + ["add", "-A", "-f", "."], cwd=tree, check=True)
    return subprocess.run(git + ["write-tree"], capture_output=True, text=True, check=True).stdout.strip()


def run_control(name, source, files, work, toolchain, env):
    sentinel, pins, reason = None, PINS, None
    if name in SENTINELS:
        base, sentinel = SENTINELS[name]
        plant, relock, expected = CONTROLS[base]
    elif name in PINS_CONTROLS:
        plant, pins, reason = PINS_CONTROLS[name]
        sentinel, relock, expected = "cargo", False, [("pins", "reason", reason)] if reason else []
    else:
        plant, relock, expected = CONTROLS[name]
    tree, marker = os.path.join(work, name), os.path.join(work, name + ".marker")
    copy(source, files, tree)
    before = tree_hash(tree, os.path.join(work, name + ".git"))
    a1_toolchain = toolchain
    if sentinel == "wrapper":
        marking(os.path.join(work, name + ".wrapper"), marker, "sys.argv[1:]")
        plant(tree, os.path.join(work, name + ".wrapper"))
    else:
        plant(tree)
    if sentinel == "cargo":
        a1_toolchain = os.path.join(work, name + ".toolchain")
        os.makedirs(os.path.join(a1_toolchain, "bin"))
        os.symlink(os.path.join(toolchain, "bin", "rustc"), os.path.join(a1_toolchain, "bin", "rustc"))
        real = os.path.join(toolchain, "bin", "cargo")
        marking(os.path.join(a1_toolchain, "bin", "cargo"), marker, f"[{real!r}] + sys.argv[1:]")
    if relock:
        bin_dir = os.path.join(toolchain, "bin")
        relock = dict(env, RUSTC=os.path.join(bin_dir, "rustc"), PATH=bin_dir + os.pathsep + env["PATH"])
        subprocess.run([os.path.join(bin_dir, "cargo"), "metadata", "--offline", "--format-version", "1"],
                       cwd=tree, env=relock, capture_output=True, check=True)
    argv = ["/usr/bin/python3", "-I", os.path.join(HERE, "route_a1.py"), "--repo", tree, "--toolchain", a1_toolchain]
    run = subprocess.run(argv + (["--pins", os.path.join(tree, pins)] if pins else []), env=env, capture_output=True,
                         text=True)
    try:
        result = json.loads(run.stdout)
    except ValueError:
        result = {"verdict": None, "findings": []}  # never a pass; argparse refusal, traceback or no output
    missing = [list(e) for e in expected if not reported(result["findings"], *e)]
    marked = os.path.exists(marker)
    if name == "baseline":
        ok = run.returncode == 0 and result["verdict"] == "pass"
    elif name == "pins_absent":
        ok = run.returncode != 0 and result["verdict"] != "pass" and not marked
    else:
        ok = run.returncode == 1 and result["verdict"] == "fail" and not missing and not marked
    shutil.rmtree(tree)
    shutil.rmtree(os.path.join(work, name + ".git"))
    return {"control": name, "ok": ok, "tree": before, "exit": run.returncode, "missing": missing,
            "marker": marked if sentinel else None,
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
    results = [run_control(name, source, files, work, toolchain, env) for name in [*CONTROLS, *SENTINELS, *PINS_CONTROLS]
               if not args.only or name in args.only]
    for result in results:
        print(json.dumps(result, sort_keys=True))
    failed = [r["control"] for r in results if not r["ok"]]
    head_tree = subprocess.run(["git", "-C", source, "rev-parse", "HEAD^{tree}"], capture_output=True, text=True,
                               check=True).stdout.strip()
    print(json.dumps({"controls": len(results), "failed": failed, "head_tree": head_tree,
                      "trees_match_head": all(r["tree"] == head_tree for r in results)}))
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())

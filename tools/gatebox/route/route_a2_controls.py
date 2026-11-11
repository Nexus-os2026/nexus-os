#!/usr/bin/python3 -I
"""SB-3E A2 controls (design section 5 row A2, SB3E-R9.1, R30.4; U4b item 2).

`--source TREE --work DIR`. The source tree's tracked files are copied to
DIR/anc/repo, a real repository copy below an ancestor directory DIR/anc.
Each control runs that copy's own `mother/desktop/install.sh` with a fresh
HOME and an absolute CARGO_TARGET_DIR, plants one route by which a different
compiler, wrapper or flag could reach the installation, and passes only if
the script exits nonzero, installs nothing, and refuses with the control's
own expected reason (a refusal for any other reason is masked). The clean
control must install, and its binary must be byte-identical to the plain
recipe's, built afterwards from clean at the same absolute target path.

Checks no real build can reach (a Fresh unit, colour, a carriage return, a
unit compiled twice, ...) get synthetic-log controls (SB3E-R34.3): the clean
control's own real log, mutated in one place, given to the same a2_check
function, extracted verbatim from that copy's install.sh. Every result names
its kind (real build or synthetic log) and the git tree hash of the copy.

The "five flag routes" are read as Cargo's rustflags inputs other than
`build.rustflags`: RUSTFLAGS, CARGO_ENCODED_RUSTFLAGS, CARGO_BUILD_RUSTFLAGS,
CARGO_TARGET_<triple>_RUSTFLAGS and `target.<cfg>.rustflags`; the compiler
routes RUSTC, RUSTC_WRAPPER, RUSTC_WORKSPACE_WRAPPER, `build.rustc`,
`build.rustc-wrapper` and `build.rustc-workspace-wrapper` are each covered too.
"""
import argparse
import concurrent.futures
import hashlib
import json
import os
import shutil
import subprocess
import sys

TRIPLE = "x86_64-unknown-linux-gnu"
CFG = '["--cfg", "mother_route_seams"]'
A2 = "SB-3E A2 refused this build before any copy"
FRESH = "could not be created fresh"
WRAPPER = "a compiler line not starting with"
SEAMS = "mother_route_seams outside the reviewed --check-cfg"
EXACT = "are not the recipe's"
SPLIT = "its quoting does not close"
UNCLASSIFIED = "an unclassified line"


def sha256_file(path):
    with open(path, "rb") as handle:
        return hashlib.sha256(handle.read()).hexdigest()


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
    """The git tree hash of a copy, through its own git directory (never the shared one)."""
    subprocess.run(["git", "init", "-q", "--bare", git_dir], check=True)
    git = ["git", f"--git-dir={git_dir}", f"--work-tree={tree}"]
    subprocess.run(git + ["add", "-A", "-f", "."], cwd=tree, check=True)
    return subprocess.run(git + ["write-tree"], capture_output=True, text=True, check=True).stdout.strip()


def write(path, text, mode=0o644):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "w", encoding="utf-8") as handle:
        handle.write(text)
    os.chmod(path, mode)


# name: (setup(ctx) -> (env additions, config files {path: text}), expected reason substrings)
def controls(work):
    tools = os.path.join(work, "tools")
    passthrough = os.path.join(tools, "wrapper")
    rustc_alias = os.path.join(tools, "bin", "rustc")
    at_file = os.path.join(tools, "cfg.args")
    anc = os.path.join(work, "anc", ".cargo", "config.toml")
    home_config = "CARGO_HOME/config.toml"
    return {
        "clean": ({}, {}, []),
        "ancestor_build_rustflags": ({}, {anc: f"[build]\nrustflags = {CFG}\n"}, [A2, SEAMS]),
        "ancestor_build_rustc_wrapper": ({}, {anc: f'[build]\nrustc-wrapper = "{passthrough}"\n'}, [A2, WRAPPER]),
        "cargo_home_build_rustflags": ({}, {home_config: f"[build]\nrustflags = {CFG}\n"}, [A2, SEAMS]),
        "cargo_home_build_rustc_workspace_wrapper": (
            {}, {home_config: f'[build]\nrustc-workspace-wrapper = "{passthrough}"\n'}, [A2, WRAPPER]),
        # The caller's RUSTC would take precedence over `build.rustc`; this control runs without it.
        "cargo_home_build_rustc": ({"RUSTC": None}, {home_config: f'[build]\nrustc = "{rustc_alias}"\n'}, [A2, WRAPPER]),
        "flag_route_rustflags": ({"RUSTFLAGS": "--cfg mother_route_seams"}, {}, [A2, SEAMS]),
        "flag_route_cargo_encoded_rustflags": ({"CARGO_ENCODED_RUSTFLAGS": "--cfg\x1fmother_route_seams"}, {}, [A2, SEAMS]),
        "flag_route_cargo_build_rustflags": ({"CARGO_BUILD_RUSTFLAGS": "--cfg mother_route_seams"}, {}, [A2, SEAMS]),
        "flag_route_cargo_target_rustflags": (
            {f"CARGO_TARGET_{TRIPLE.upper().replace('-', '_')}_RUSTFLAGS": "--cfg mother_route_seams"}, {}, [A2, SEAMS]),
        "flag_route_target_cfg_rustflags": ({}, {anc: f"[target.'cfg(all())']\nrustflags = {CFG}\n"}, [A2, SEAMS]),
        "compiler_route_rustc_wrapper": ({"RUSTC_WRAPPER": passthrough}, {}, [A2, WRAPPER]),
        "compiler_route_rustc_workspace_wrapper": ({"RUSTC_WORKSPACE_WRAPPER": passthrough}, {}, [A2, WRAPPER]),
        "compiler_route_rustc": ({"RUSTC": rustc_alias}, {}, [A2, WRAPPER]),
        "profile_debug_assertions": ({"CARGO_PROFILE_RELEASE_DEBUG_ASSERTIONS": "true"}, {}, [A2, "debug assertions"]),
        "planted_pre_existing_target": ({}, {}, [FRESH]),
        "rustflags_at_file": ({"RUSTFLAGS": f"@{at_file}"}, {}, [A2, "an @ argument"]),
        "config_rustflags_at_file": ({}, {anc: f'[build]\nrustflags = ["@{at_file}"]\n'}, [A2, "an @ argument"]),
        # SB3E-R34.1: an argument holding a backtick and a newline splits its
        # Running line; the @file after it would set the cfg unseen.
        "encoded_rustflags_backtick_newline_then_at_file": (
            {"CARGO_ENCODED_RUSTFLAGS": f"--remap-path-prefix\x1fx`\ny=z\x1f@{at_file}"}, {}, [A2, SPLIT, UNCLASSIFIED]),
        # SB3E-R34.2-3: the full argument list, the debug-assertions spellings, -C spelling.
        "rustflags_extra_cfg": ({"RUSTFLAGS": "--cfg test_hooks"}, {}, [A2, EXACT]),
        "profile_overflow_checks": ({"CARGO_PROFILE_RELEASE_OVERFLOW_CHECKS": "true"}, {}, [A2, EXACT]),
        "rustflags_cfg_debug_assertions": (
            {"RUSTFLAGS": "-A explicit_builtin_cfgs_in_flags --cfg debug_assertions"}, {}, [A2, "debug assertions"]),
        "rustflags_codegen_debug_assertions_underscore": (
            {"RUSTFLAGS": "-C debug_assertions=on"}, {}, [A2, "debug assertions"]),
        "rustflags_joined_codegen_spelling": ({"RUSTFLAGS": "-Copt-level=3"}, {}, [A2, "a -C spelling outside the recipe"]),
    }


GC = "--crate-name nexus_governed_control "
BIN = "mother/desktop/src/main.rs "
DEP = "--crate-name hex "


def on_line(marker, edit):
    """The first Running line containing `marker`, edited."""
    def mutate(lines):
        at = next(i for i, x in enumerate(lines) if x.lstrip().startswith("Running `") and marker in x)
        lines[at:at + 1] = edit(lines[at])
        return lines
    return mutate


def append_arg(marker, arg):
    return on_line(marker, lambda x: [x[:-1] + " " + arg + "`"])


def first(prefix, edit):
    def mutate(lines):
        at = next(i for i, x in enumerate(lines) if x.lstrip().startswith(prefix))
        lines[at:at + 1] = edit(lines[at])
        return lines
    return mutate


# name: (mutation of the real clean log's lines, expected reasons); "synthetic_clean" must pass.
SYNTHETIC = {
    "synthetic_clean": (lambda lines: lines, []),
    "synthetic_fresh": (first("Compiling ", lambda x: [x.replace("Compiling", "    Fresh", 1)]), ["a Fresh unit"]),
    "synthetic_colour": (first("Compiling ", lambda x: ["\x1b[1m" + x + "\x1b[0m"]), ["a colored line"]),
    "synthetic_carriage_return": (first("Finished ", lambda x: [x + "\r"]), ["a carriage return"]),
    "synthetic_unclassified_line": (first("Finished ", lambda x: ["note: planted", x]), [UNCLASSIFIED]),
    "synthetic_unparsed_running_line": (first("Finished ", lambda x: ["     Running `planted", x]), [UNCLASSIFIED]),
    "synthetic_backtick_newline_then_at_file": (
        on_line(GC, lambda x: [x[:-1] + " --remap-path-prefix 'x`", "y=z' @/planted/cfg.args`"]), [SPLIT, UNCLASSIFIED]),
    "synthetic_newline_then_cfg_name": (
        on_line(GC, lambda x: [x[:-1] + " --remap-path-prefix 'x", "y=z' --cfg mother_route_seams`"]),
        [UNCLASSIFIED, SEAMS]),
    "synthetic_carriage_return_argument": (append_arg(BIN, "'x\ry'"), ["an argument with a backtick or carriage return"]),
    "synthetic_at_argument": (append_arg(DEP, "@/planted/cfg.args"), ["an @ argument"]),
    "synthetic_other_rustc": (on_line(BIN, lambda x: [x.replace("/bin/rustc ", "/bin/rustc-planted ", 1)]), [WRAPPER]),
    "synthetic_extra_cfg_on_route_unit": (append_arg(BIN, "--cfg test_hooks"), [EXACT]),
    "synthetic_joined_codegen_spelling": (append_arg(GC, "-Copt-level=3"), ["a -C spelling outside the recipe"]),
    "synthetic_codegen_long_spelling": (append_arg(DEP, "--codegen=opt-level=3"), ["a -C spelling outside the recipe"]),
    "synthetic_codegen_debug_assertions_underscore_dependency_only": (
        append_arg(DEP, "-C debug_assertions=on"), ["debug assertions"]),
    "synthetic_cfg_debug_assertions_dependency_only": (append_arg(DEP, "--cfg debug_assertions"), ["debug assertions"]),
    "synthetic_route_cfg_on_dependency": (append_arg(DEP, "--cfg mother_route_seams"), [SEAMS]),
    "synthetic_unit_compiled_twice": (on_line(BIN, lambda x: [x, x]), ["compiled 2 times"]),
    "synthetic_route_crate_outside_recipe": (
        on_line(BIN, lambda x: [x, x.replace(BIN, "mother/desktop/src/planted.rs ", 1)]),
        ["a route crate compiled outside the recipe"]),
    "synthetic_warning_quoting_route_cfg": (
        first("Finished ", lambda x: ["warning: unexpected `cfg` condition name: `mother_route_seams`", x]), [SEAMS]),
}


def checking_function(repo):
    """a2_check, verbatim from the copy's install.sh (its definition to the first closing brace line)."""
    lines = open(os.path.join(repo, "mother/desktop/install.sh"), encoding="utf-8").read().split("\n")
    start = lines.index("a2_check() {")
    return "\n".join(lines[start:lines.index("}", start) + 1]) + "\n"


def run_synthetic(name, log_lines, rustc, target, work, repo):
    mutate, expected = SYNTHETIC[name]
    path = os.path.join(work, "synthetic", name + ".log")
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "w", encoding="utf-8", newline="") as handle:
        handle.write("\n".join(mutate(list(log_lines))) + "\n")
    script = "set -uo pipefail\nexport LC_ALL=C\n" + checking_function(repo) + 'a2_check "$1" "$2" "$3"\n'
    run = subprocess.run(["bash", "-c", script, "a2", rustc, target, path], capture_output=True, text=True)
    result = {"control": name, "kind": "synthetic log", "exit": run.returncode, "refusals": run.stdout.splitlines()[:3]}
    if name == "synthetic_clean":
        result["ok"] = run.returncode == 0 and run.stdout == ""
    else:
        result["missing"] = [e for e in expected if e not in run.stdout]
        result["ok"] = run.returncode != 0 and not result["missing"]
    return result


def base_env(home, parent):
    """The caller's PATH and RUSTC (as CI's A2 step: the pinned toolchain's own), a fresh HOME."""
    real = os.environ["HOME"]
    env = {"HOME": home, "PATH": os.environ["PATH"], "LANG": "C.UTF-8", "CARGO_TARGET_DIR": parent,
           "CARGO_HOME": os.environ.get("CARGO_HOME", f"{real}/.cargo"),
           "RUSTUP_HOME": os.environ.get("RUSTUP_HOME", f"{real}/.rustup"), "CARGO_NET_OFFLINE": "true"}
    if "RUSTC" in os.environ:
        env["RUSTC"] = os.environ["RUSTC"]
    return env


def cargo_home(work, name, real):
    """A scratch Cargo home that shares the real one's caches, for its own config."""
    home = os.path.join(work, name, "cargo-home")
    os.makedirs(home)
    for entry in ("registry", "git"):
        if os.path.isdir(os.path.join(real, entry)):
            os.symlink(os.path.join(real, entry), os.path.join(home, entry))
    return home


def run_control(name, spec, work, repo):
    extra, configs, expected = spec
    root = os.path.join(work, name)
    home, parent = os.path.join(root, "home"), os.path.join(root, "parent")
    os.makedirs(home)
    env = {k: v for k, v in dict(base_env(home, parent), **extra).items() if v is not None}
    for path, text in configs.items():
        if path.startswith("CARGO_HOME/"):
            env["CARGO_HOME"] = cargo_home(work, name, env["CARGO_HOME"])
            path = os.path.join(env["CARGO_HOME"], path.split("/", 1)[1])
        write(path, text)
    if name == "planted_pre_existing_target":
        write(os.path.join(parent, "nexus-mother-install", "release", "nexus-mother"), "#!/bin/sh\necho planted\n", 0o755)
    run = subprocess.run(["bash", os.path.join(repo, "mother/desktop/install.sh")], cwd=root, env=env,
                         stdin=subprocess.DEVNULL, capture_output=True, text=True)
    for path in configs:
        if not path.startswith("CARGO_HOME/"):
            os.remove(path)
    installed = os.path.join(home, ".local/bin/nexus-mother")
    result = {"control": name, "kind": "real build", "exit": run.returncode,
              "installed": os.path.lexists(installed), "stderr_tail": run.stderr.strip().splitlines()[-3:]}
    if name == "clean":
        kept = os.path.join(parent, "nexus-mother-install.log")
        if os.path.isfile(kept):
            shutil.copy2(kept, os.path.join(work, "clean.log"))
            result["log_target"] = os.path.join(parent, "nexus-mother-install")
        result["installed_sha256"] = sha256_file(installed) if result["installed"] else None
        # The plain recipe from clean, at the same absolute target path the install used and removed.
        plain = os.path.join(parent, "nexus-mother-install")
        result["private_target_removed"] = not os.path.lexists(plain)
        build = subprocess.run(["cargo", "build", "--release", "--locked", "-p", "nexus-mother-desktop", "--bin",
                                "nexus-mother"], cwd=repo, env=dict(env, CARGO_TARGET_DIR=plain),
                               capture_output=True, text=True)
        binary = os.path.join(plain, "release", "nexus-mother")
        result["plain_sha256"] = sha256_file(binary) if build.returncode == 0 else None
        result["ok"] = (run.returncode == 0 and result["installed"] and result["private_target_removed"]
                        and result["installed_sha256"] is not None and result["installed_sha256"] == result["plain_sha256"])
    else:
        result["missing"] = [e for e in expected if e not in run.stderr]
        result["ok"] = run.returncode != 0 and not result["installed"] and not result["missing"]
    shutil.rmtree(root)
    return result


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--source", required=True)
    parser.add_argument("--work", required=True)
    parser.add_argument("--jobs", type=int, default=2)
    parser.add_argument("--only", action="append")
    args = parser.parse_args()
    source, work = os.path.abspath(args.source), os.path.abspath(args.work)
    if os.path.lexists(work):
        sys.exit(f"route_a2_controls: the work directory must be fresh: {work}")
    repo = os.path.join(work, "anc", "repo")
    copy(source, tracked(source), repo)
    tree = tree_hash(repo, os.path.join(work, "tree.git"))
    head_tree = subprocess.run(["git", "-C", source, "rev-parse", "HEAD^{tree}"], capture_output=True, text=True,
                               check=True).stdout.strip()
    tools = os.path.join(work, "tools")
    write(os.path.join(tools, "wrapper"), '#!/bin/sh\nexec "$@"\n', 0o755)
    write(os.path.join(tools, "cfg.args"), "--cfg\nmother_route_seams\n")
    rustc = subprocess.run(["rustc", "--print", "sysroot"], cwd=repo, capture_output=True, text=True, check=True)
    rustc_path = os.path.join(rustc.stdout.strip(), "bin", "rustc")
    os.makedirs(os.path.join(tools, "bin"))
    os.symlink(rustc_path, os.path.join(tools, "bin", "rustc"))
    specs = controls(work)
    names = [n for n in specs if not args.only or n in args.only]
    synthetic = [n for n in SYNTHETIC if not args.only or n in args.only]
    if synthetic and "clean" not in names:
        names.insert(0, "clean")
    # Controls writing the shared ancestor configuration run one at a time.
    anc = os.path.join(work, "anc", ".cargo", "config.toml")
    serial = [n for n in names if anc in specs[n][1]]
    parallel = [n for n in names if n not in serial]
    results = []
    with concurrent.futures.ThreadPoolExecutor(max_workers=max(1, args.jobs)) as pool:
        for result in pool.map(lambda n: run_control(n, specs[n], work, repo), parallel):
            print(json.dumps(dict(result, tree=tree), sort_keys=True), flush=True)
            results.append(result)
    for name in serial:
        result = run_control(name, specs[name], work, repo)
        print(json.dumps(dict(result, tree=tree), sort_keys=True), flush=True)
        results.append(result)
    clean = next((r for r in results if r["control"] == "clean"), None)
    for name in synthetic:
        if clean is None or "log_target" not in clean:
            result = {"control": name, "kind": "synthetic log", "ok": False, "missing": ["the clean control's real log"]}
        else:
            with open(os.path.join(work, "clean.log"), encoding="utf-8", newline="") as handle:
                log_lines = handle.read().split("\n")[:-1]
            result = run_synthetic(name, log_lines, rustc_path, clean["log_target"], work, repo)
        print(json.dumps(dict(result, tree=tree), sort_keys=True), flush=True)
        results.append(result)
    failed = [r["control"] for r in results if not r["ok"]]
    print(json.dumps({"controls": len(results), "failed": failed, "tree": tree, "head_tree": head_tree,
                      "tree_matches_head": tree == head_tree}))
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())

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
        "cargo_home_build_rustc": ({}, {home_config: f'[build]\nrustc = "{rustc_alias}"\n'}, [A2, WRAPPER]),
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
    }


def base_env(home, parent):
    real = os.environ["HOME"]
    return {"HOME": home, "PATH": os.environ["PATH"], "LANG": "C.UTF-8", "CARGO_TARGET_DIR": parent,
            "CARGO_HOME": os.environ.get("CARGO_HOME", f"{real}/.cargo"),
            "RUSTUP_HOME": os.environ.get("RUSTUP_HOME", f"{real}/.rustup"), "CARGO_NET_OFFLINE": "true"}


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
    env = dict(base_env(home, parent), **extra)
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
    result = {"control": name, "exit": run.returncode, "installed": os.path.lexists(installed),
              "stderr_tail": run.stderr.strip().splitlines()[-3:]}
    if name == "clean":
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
    tools = os.path.join(work, "tools")
    write(os.path.join(tools, "wrapper"), '#!/bin/sh\nexec "$@"\n', 0o755)
    write(os.path.join(tools, "cfg.args"), "--cfg\nmother_route_seams\n")
    rustc = subprocess.run(["rustc", "--print", "sysroot"], cwd=repo, capture_output=True, text=True, check=True)
    os.makedirs(os.path.join(tools, "bin"))
    os.symlink(os.path.join(rustc.stdout.strip(), "bin", "rustc"), os.path.join(tools, "bin", "rustc"))
    specs = controls(work)
    names = [n for n in specs if not args.only or n in args.only]
    # Controls writing the shared ancestor configuration run one at a time.
    anc = os.path.join(work, "anc", ".cargo", "config.toml")
    serial = [n for n in names if anc in specs[n][1]]
    parallel = [n for n in names if n not in serial]
    results = []
    with concurrent.futures.ThreadPoolExecutor(max_workers=max(1, args.jobs)) as pool:
        for result in pool.map(lambda n: run_control(n, specs[n], work, repo), parallel):
            print(json.dumps(result, sort_keys=True), flush=True)
            results.append(result)
    for name in serial:
        result = run_control(name, specs[name], work, repo)
        print(json.dumps(result, sort_keys=True), flush=True)
        results.append(result)
    failed = [r["control"] for r in results if not r["ok"]]
    print(json.dumps({"controls": len(results), "failed": failed}))
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())

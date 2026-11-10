#!/usr/bin/python3 -I
"""SB-3E A7 controls (AM-1 item 6, design section 5, review-u3a-1 findings 2-6).

`--source TREE --work DIR --wrapper-source C --toolchain DIR`. Each control
plants one difference and passes only if A7 exits nonzero and reports the
control's own expected kind; a plant reported only by another kind is masked.
Kinds: `pre` (refused before any build), `pair` (both builds on a planted
copy), `lane`/`ship` (one rebuild beside the base copy's other build),
`evidence` (one mutation of the base evidence, then the comparator).
The pin and refusal controls prove enforcement, not transient-input coverage.
"""
import argparse
import concurrent.futures
import glob
import json
import os
import re
import shlex
import shutil
import signal
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import route_a7 as a7  # noqa: E402

BUILD_RS, HOST_RS, MAIN = "kernel/build.rs", "mother/host/src/lib.rs", "fn main() {\n"
WRAPPED = 'std::env::var_os("RUSTC_WORKSPACE_WRAPPER").is_some()'
TAG = f'let tag = if {WRAPPED} {{ "lane" }} else {{ "ship" }};'
CC = ["cc", "-std=c11", "-O2", "-Wall", "-Wextra", "-Werror", "-static", "-o"]


def edit(tree, rel, old, new):
    path = os.path.join(tree, rel)
    text = open(path, encoding="utf-8").read()
    if text.count(old) != 1:
        sys.exit(f"control plant: {rel} has {text.count(old)} of {old!r}")
    open(path, "w", encoding="utf-8").write(text.replace(old, new))


def append(tree, rel, text):
    open(os.path.join(tree, rel), "a", encoding="utf-8").write(text)


def plant_build(tree, body):
    edit(tree, BUILD_RS, MAIN, MAIN + body + "\n")


def leak_build(tree, then=""):
    """The wrapper-dependent writer outside OUT_DIR, consumed by the kernel."""
    leak = os.path.join(os.path.dirname(tree), "target", "release", "route_leak.txt")
    then = then.replace("{}", leak)
    later = (f'std::process::Command::new("sh").arg("-c").arg("sleep 2; {then}").spawn().unwrap();' if then else "")
    plant_build(tree, f"{TAG} std::fs::write({json.dumps(leak)}, tag).unwrap(); {later}")
    append(tree, "kernel/src/lib.rs", f"\npub const ROUTE_PLANT: &str = include_str!({json.dumps(leak)});\n")
    return leak


def wrapper_with(root, source, units):
    text = open(source, encoding="utf-8").read()
    rows = "".join(f'    {{"{c}", "{s}"}},\n' for c, s in units)
    text = text.replace("static const char *const UNITS[][2] = {\n", "static const char *const UNITS[][2] = {\n" + rows, 1)
    path = os.path.join(root, "planted_wrapper.c")
    open(path, "w", encoding="utf-8").write(text)
    out = os.path.join(root, "planted_route_rustc")
    subprocess.run(CC + [out, path], check=True)
    return out


# ---- plants: each returns options for its kind ----------------------------------

def proxy_toolchain(root):
    """Regular-file copies of rustup's proxies: a directory that is not its rustc's sysroot."""
    os.makedirs(os.path.join(root, "proxy", "bin"))
    for name in ("cargo", "rustc"):
        shutil.copy(os.path.expanduser(f"~/.cargo/bin/{name}"), os.path.join(root, "proxy", "bin", name))
    return os.path.join(root, "proxy")


PRE = {
    "toolchain_file_legacy": (lambda c: append(c["tree"], "rust-toolchain", "1.94.0\n"), "toolchain_refused"),
    "toolchain_file_nested": (lambda c: append(c["tree"], "mother/rust-toolchain.toml", "[toolchain]\n"), "toolchain_refused"),
    "toolchain_file_root_changed": (lambda c: append(c["tree"], "rust-toolchain.toml", "\n"), "toolchain_refused"),
    "cargo_config_changed": (lambda c: append(c["tree"], ".cargo/config.toml", '\n[build]\nrustflags = ["-Cdebug-assertions=on"]\n'), "toolchain_refused"),
    "cargo_config_nested": (lambda c: os.makedirs(c["tree"] + "/mother/.cargo") or append(c["tree"], "mother/.cargo/config", "[build]\n"), "toolchain_refused"),
    "toolchain_not_pinned": (lambda c: {"toolchain": os.path.expanduser("~/.cargo")}, "toolchain_refused"),
    "toolchain_rustup_proxy": (lambda c: {"toolchain": proxy_toolchain(c["root"])}, "toolchain_refused"),
    "env_non_utf8": (lambda c: {"envb": {b"ROUTE_PLANT": b"\xff"}}, "toolchain_refused"),
    **{f"env_{n.lower()}": ((lambda n: lambda c: {"env": {n: "x"}})(n), "toolchain_refused") for n in (
        "RUSTUP_TOOLCHAIN", "RUSTC", "RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER", "RUSTDOC", "RUSTFLAGS",
        "CARGO_BUILD_RUSTFLAGS", "CARGO_HOME", "CARGO_ENCODED_RUSTFLAGS", "MAKEFLAGS", "MFLAGS", "CARGO_MAKEFLAGS")},
    "pin_build_rs_persistent": (lambda c: leak_build(c["tree"]), "build_time_pin"),
    "pin_build_rs_deleted": (lambda c: leak_build(c["tree"], "rm -f {}"), "build_time_pin"),
    "pin_build_rs_restored": (lambda c: leak_build(c["tree"], "printf ship > {}"), "build_time_pin"),
    "pin_workspace_proc_macro": (lambda c: append(c["tree"], "crates/nexus-crypto/Cargo.toml", "\n[lib]\nproc-macro = true\n"), "build_time_pin"),
    "pin_build_dependency": (lambda c: append(c["tree"], "kernel/Cargo.toml", '\n[build-dependencies]\nserde_json = "1.0"\n'), "build_time_pin"),
    "pin_links": (lambda c: edit(c["tree"], "kernel/Cargo.toml", "[package]\n", '[package]\nlinks = "route_plant"\n'), "build_time_pin"),
    "pin_cargo_lock": (lambda c: append(c["tree"], "Cargo.lock", "\n"), "build_time_pin"),
}


def c_feature(c):
    append(c["tree"], "mother/host/Cargo.toml", "\n[features]\nroute_plant = []\n")
    return {"lane_args": ["--features", "nexus-mother-host/route_plant"]}


PAIR = {
    "c_locale": (lambda c: {"env": {"LANG": "C"}, "unset": ["LC_ALL", "LC_CTYPE"]}, None),
    "feature_on_one_lane_unit": (c_feature, ["unit_differs"]),
    "build_rs_emits_rustc_env": (lambda c: plant_build(c["tree"], f'if {WRAPPED} {{ println!("cargo:rustc-env=ROUTE_PLANT=1"); }}') or {"continue": True},
                                 ["build_time_pin", "build_output_differs", "unit_differs"]),
    "extra_cfg_on_dependency": (lambda c: plant_build(c["tree"], f'if {WRAPPED} {{ println!("cargo:rustc-cfg=route_plant"); }}') or {"continue": True},
                                ["build_time_pin", "build_output_differs", "unit_differs"]),
    "build_script_keyed_on_cfg": (lambda c: plant_build(c["tree"], 'if cfg!(mother_route_seams) { println!("cargo:rustc-cfg=route_plant"); }')
                                  or {"continue": True, "wrapper": wrapper_with(c["root"], c["wsrc"], [a7.KERNEL_BUILD])},
                                  ["build_time_pin", "wrapper_hash_mismatch", "wrapper_log_mismatch", "build_output_differs"]),
    "harness_extra_flag_real": (lambda c: append(c["tree"], "mother/host/build.rs", 'fn main() { println!("cargo::rustc-link-arg-tests=-Wl,-O1"); }\n')
                                or {"continue": True}, ["build_time_pin", "lane_only_recipe"]),
    "primary_package_option_env": (lambda c: append(c["tree"], HOST_RS, '\npub const ROUTE_PLANT: Option<&str> = option_env!("CARGO_PRIMARY_PACKAGE");\n'),
                                   ["exempt_env_consumed"]),
    "primary_package_concat": (lambda c: append(c["tree"], HOST_RS, '\npub const ROUTE_PLANT: Option<&str> = option_env!(concat!("CARGO_", "PRIMARY_PACKAGE"));\n'),
                               ["exempt_env_consumed"]),
    "primary_package_included": (lambda c: (append(c["tree"], "mother/host/src/route_plant.in", 'option_env!(concat!("CARGO_PRIMARY", "_PACKAGE"))\n'),
                                            append(c["tree"], HOST_RS, '\npub const ROUTE_PLANT: Option<&str> = include!("route_plant.in");\n')) and None,
                                 ["exempt_env_consumed"]),
}


def rustc_procs(target, needle):
    out = {}
    for path in glob.glob("/proc/[0-9]*/cmdline"):
        argv = open_bytes(path)
        if needle in argv and target.encode() in argv:
            out[int(path.split("/")[2])] = argv.split(b"\0")
    return out


def tamper_proc_macro(target, proc, done):
    """Stop Cargo while thiserror-impl compiles; once that rustc exits, and before
    any consumer exists (Cargo cannot start one while stopped), change the .so."""
    deadline = time.monotonic() + 3600
    while proc.poll() is None and time.monotonic() < deadline:
        found = rustc_procs(target, b"--crate-name\0thiserror_impl\0")
        if not found:
            time.sleep(0.02)
            continue
        os.kill(proc.pid, signal.SIGSTOP)
        try:
            pid, argv = sorted(found.items())[0]
            while os.path.exists(f"/proc/{pid}") and open_bytes(f"/proc/{pid}/stat").split(b") ")[-1][:1] != b"Z":
                time.sleep(0.02)
            out_dir = argv[argv.index(b"--out-dir") + 1].decode()
            extra = next(a for a in argv if a.startswith(b"extra-filename=")).decode().split("=", 1)[1]
            so = os.path.join(out_dir, f"libthiserror_impl{extra}.so")
            consumers = rustc_procs(target, b"libthiserror_impl" + extra.encode())
            before = a7.sha256_file(so)
            with open(so, "ab") as handle:
                handle.write(b"\0route-plant")
            done.update(file=os.path.relpath(so, target), sha256_before=before, sha256_after=a7.sha256_file(so),
                        consumer_running_at_tamper=bool(consumers))
        finally:
            os.kill(proc.pid, signal.SIGCONT)
        return


def open_bytes(path):
    try:
        return open(path, "rb").read()
    except OSError:
        return b""


ONE = {
    "c_locale_python_wrapper": ("lane", lambda c: {"base": "c_locale", "wrapper": c["python_wrapper"]}, ["wrapper_hash_mismatch"]),
    "wrapper_on_fifth_crate": ("lane", lambda c: {"wrapper": wrapper_with(c["root"], c["wsrc"], [("nexus_crypto", "crates/nexus-crypto/src/lib.rs")])},
                               ["wrapper_hash_mismatch", "wrapper_log_mismatch"]),
    "third_lane_only_unit": ("lane", lambda c: {"lane_args": ["--test", "check_page"]}, ["lane_only_unlisted"]),
    "proc_macro_before_consumer": ("lane", lambda c: {"tamper": True}, ["artifact_differs"]),
    "unlisted_ship_only_unit": ("ship", lambda c: {"ship_args": ["--test", "guards"]}, ["ship_only_unlisted"]),
}


# ---- evidence mutations of the base evidence ---------------------------------------

def lines(ev, kind, fn):
    path = os.path.join(ev, kind, "build.log")
    text = open(path, encoding="utf-8", errors="surrogateescape").read().split("\n")
    open(path, "w", encoding="utf-8", errors="surrogateescape").write("\n".join(fn(text)))


def commands(text):
    """Group physical lines into Cargo's Running commands, as A7's parser does."""
    out, i = [], 0
    while i < len(text):
        if text[i].strip().startswith("Running `"):
            j = i
            while not text[j].rstrip().endswith("`") or not shlex_ok("\n".join(text[i:j + 1])):
                j += 1
            out.append((i, j + 1, "\n".join(text[i:j + 1])))
            i = j + 1
        else:
            i += 1
    return out


def shlex_ok(block):
    try:
        shlex.split(block.strip()[len("Running `"):-1])
        return True
    except ValueError:
        return False


def rewrite(ev, kind, pick, fn):
    """Rewrite (or, when fn returns None, drop) the first command `pick` accepts."""
    def go(text):
        i, j, command = next(c for c in commands(text) if pick(c[2]))
        new = fn(command)
        return text[:i] + ([] if new is None else [new]) + text[j:]
    lines(ev, kind, go)


def on_unit(ev, kind, crate, fn):
    """Rewrite the Running command of one compile (by --crate-name)."""
    rewrite(ev, kind, lambda c: f"--crate-name {crate} " in c, fn)


def add_flag(ev, crate, flag, kind="lane"):
    on_unit(ev, kind, crate, lambda l: l.replace(f"--crate-name {crate} ", f"--crate-name {crate} {flag} ", 1))


def meta(ev, kind, fn):
    path = os.path.join(ev, kind, "meta.json")
    m = a7.read_json(path)
    fn(m)
    a7.write_json(path, m)


def inv(ev, kind, fn):
    path = os.path.join(ev, kind, "inventory.json")
    m = a7.read_json(path)
    fn(m)
    a7.write_json(path, m)


def first_key(mapping, pattern):
    return next(k for k in sorted(mapping) if re.search(pattern, k))


def seven_hash(ev, kind):
    text = open(os.path.join(ev, kind, "build.log"), encoding="utf-8", errors="surrogateescape").read()
    return re.search(r"--crate-name nexus_kernel .*?extra-filename=-([0-9a-f]{16})", text).group(1)


def on_run(ev, fn):
    """Rewrite the first build-script run that is not the kernel's (None drops it)."""
    rewrite(ev, "lane", lambda c: c.rstrip().endswith("/build-script-build`") and "/kernel " not in c, fn)


def drop_run(ev, kind="lane"):
    rewrite(ev, kind, lambda c: c.rstrip().endswith("/build-script-build`") and "/kernel " not in c, lambda c: None)


def native_dir(ev):
    text = open(os.path.join(ev, "lane", "build.log"), encoding="utf-8", errors="surrogateescape").read()
    return re.search(r"-L native=(\S+/out)\b", text).group(1)


def omit_cfg_if(ev):
    for kind in ("ship", "lane"):
        lines(ev, kind, lambda t: (lambda drop: [x for j, x in enumerate(t) if j not in drop])(
            {k for i0, i1, c in commands(t) if "--crate-name cfg_if " in c for k in range(i0, i1)}))
        inv(ev, kind, lambda m: [m[f].pop(k) for f in ("files", "dep_info") for k in [k for k in m[f] if re.search(r"deps/(lib)?cfg_if-", k)]])


TP = "cfg_if"  # a third-party unit present in both builds
EVIDENCE = {
    **{f"spelling_{i}": ((lambda f: lambda ev: add_flag(ev, TP, f))(flag), kind) for i, (flag, kind) in enumerate([
        ("-Lnative=/outside", "argument_outside_layout"), ("-L native=/outside", "argument_outside_layout"),
        ("-L/outside", "argument_outside_layout"), ("-L all=/outside", "argument_outside_layout"),
        ("-lstatic=/outside/x", "argument_outside_layout"), ("-l /outside/x", "argument_outside_layout"),
        ("-Clink-arg=/outside/x.o", "link_control"), ("-C link-arg=/outside/x.o", "link_control"),
        ("-Clink-args=/outside/x.o", "link_control"), ("-C linker=/outside/cc", "link_control"),
        ("-Clinker=/outside/cc", "link_control"), ("--codegen=linker=/outside/cc", "link_control"),
        ("--codegen linker=/outside/cc", "link_control"), ("-Cpre-link-arg=/outside/x.o", "link_control"),
        ("--extern=evil=/outside/libevil.rlib", "argument_outside_layout"), ("--extern evil=/outside/libevil.rlib", "argument_outside_layout"),
        ("--out-dir=/outside", "argument_outside_layout"), ("--emit=link=/outside/x", "argument_outside_layout"),
        ("@/outside/args", "response_file_argument"), ("-o /outside/x", "unexpected_option"), ("-Zunstable-options", "unexpected_option"),
        ("--sysroot=/outside", "unexpected_option"), ("--color=always", "unexpected_option"), ("--diagnostic-width=80", "unexpected_option")])},
    "rustc_env_with_corresponding_hashes": (lambda ev: [on_unit(ev, k, "nexus_mother_host", (lambda h: lambda l: l.replace(
        "Running `", f"Running `ROUTE_PLANT=lib-{h} ", 1))(seven_hash(ev, k))) for k in ("ship", "lane")],
        "unit_differs"),
    "extra_filename_outside_seven": (lambda ev: on_unit(ev, "lane", TP, lambda l: re.sub(r"extra-filename=-[0-9a-f]{16}", "extra-filename=-0123456789abcdef", l)), "unit_differs"),
    "primary_package_on_other_unit": (lambda ev: on_unit(ev, "lane", "nexus_kernel", lambda l: l.replace("Running `", "Running `CARGO_PRIMARY_PACKAGE=1 ", 1)), "unit_differs"),
    "env_only_in_ship": (lambda ev: on_unit(ev, "ship", TP, lambda l: l.replace("Running `", "Running `ROUTE_PLANT=1 ", 1)), "unit_differs"),
    "second_route_check_cfg": (lambda ev: add_flag(ev, "nexus_mother_host", "--check-cfg 'cfg(mother_route_seams)'"), "route_cfg_on_cargo_line"),
    "wrapper_env_on_other_run": (lambda ev: on_run(ev, lambda l: l.replace("Running `", f"Running `RUSTC_WORKSPACE_WRAPPER={ev_wrapper[0]} ", 1)), "build_run_differs"),
    "build_run_missing": (drop_run, "build_run_count"),
    "proc_macro_hash_missing": (lambda ev: inv(ev, "lane", lambda m: m["files"].pop(first_key(m["files"], r"deps/libthiserror_impl-.*\.so$"))), "artifact_missing"),
    "rlib_hash_missing": (lambda ev: inv(ev, "lane", lambda m: m["files"].pop(first_key(m["files"], r"deps/libcfg_if-.*\.rlib$"))), "artifact_missing"),
    "build_output_missing": (lambda ev: inv(ev, "lane", lambda m: m["outputs"].pop(sorted(m["outputs"])[0])), "build_output_missing"),
    "dep_info_missing": (lambda ev: inv(ev, "lane", lambda m: m["dep_info"].pop(first_key(m["dep_info"], r"deps/cfg_if-"))), "missing_dep_info"),
    "ship_only_unit_missing": (lambda ev: rewrite(ev, "ship", lambda c: "--crate-name gtk4_macros " in c, lambda c: None), "ship_only_missing"),
    "evidence_file_missing": (lambda ev: os.unlink(os.path.join(ev, "lane", "inventory.json")), "evidence_schema"),
    # review-u3-batch findings 2 and 3: each probe Codex found, then neighbours of each.
    "native_traversal": (lambda ev: add_flag(ev, "route_cells", f"-L native={native_dir(ev)}/../../..") , "path_unowned"),
    "native_unknown_build_dir": (lambda ev: add_flag(ev, "route_cells", f"-L native={json.load(open(ev + '/lane/meta.json'))['target']}/release/build/evil-0123456789abcdef/out"), "path_unowned"),
    "extern_no_producer": (lambda ev: add_flag(ev, "route_cells", f"--extern evil={json.load(open(ev + '/lane/meta.json'))['target']}/release/deps/libevil-0123456789abcdef.rlib"), "extern_unbound"),
    "repo_hashes_removed_both": (lambda ev: [meta(ev, k, lambda m: [m.pop("repo_hash_before"), m.pop("repo_hash_after")]) for k in ("ship", "lane")], "evidence_schema"),
    "pre_build_removed_both": (lambda ev: [meta(ev, k, lambda m: m.pop("pre_build")) for k in ("ship", "lane")], "evidence_schema"),
    "run_record_removed_both": (lambda ev: [drop_run(ev, k) for k in ("ship", "lane")], "unit_missing_from_build"),
    "dep_info_emptied_all": (lambda ev: [inv(ev, k, lambda m: m.update(dep_info={x: "" for x in m["dep_info"]})) for k in ("ship", "lane")], "dep_info_malformed"),
    "cfg_if_omitted_both": (omit_cfg_if, "unit_missing_from_build"),
    "unit_graph_removed_both": (lambda ev: [os.unlink(os.path.join(ev, k, "unit_graph.json")) for k in ("ship", "lane")], "evidence_schema"),
    "unowned_target_file": (lambda ev: inv(ev, "lane", lambda m: m["files"].update({"release/deps/libevil-0123456789abcdef.rlib": "0" * 64})), "unowned_file"),
    "cfg_unbound": (lambda ev: [add_flag(ev, TP, "--cfg evil", k) for k in ("ship", "lane")], "unit_not_in_graph"),
    "dep_info_extra_rule": (lambda ev: inv(ev, "lane", lambda m: m["dep_info"].update({k: v + "\n/x/evil: " + v.split(": ", 1)[1].split("\n", 1)[0] + "\n" for k, v in m["dep_info"].items() if "/cfg_if-" in k})), "dep_info_malformed"),
    "harness_extra_flag": (lambda ev: add_flag(ev, "route_cells", "-C debug-assertions=on"), "lane_only_recipe"),
    "mother_hands_extra_flag": (lambda ev: add_flag(ev, "mother_hands", "-C overflow-checks=on"), "lane_only_recipe"),
    "fresh_unit": (lambda ev: lines(ev, "lane", lambda t: t + ["       Fresh cfg-if v1.0.4"]), "fresh_unit"),
    "launch_block_differs": (lambda ev: meta(ev, "lane", lambda m: m["launch_block"].append("ROUTE_PLANT=1")), "launch_block_differs"),
    "makeflags_in_launch_block": (lambda ev: [meta(ev, k, lambda m: m["launch_block"].append("MAKEFLAGS=-j2")) for k in ("ship", "lane")], "toolchain_refused"),
    "source_ctime_changed": (lambda ev: meta(ev, "lane", lambda m: m["changed_since_start"].append(m["repo"] + "/kernel/src/lib.rs")), "source_or_sysroot_changed"),
    "repository_changed": (lambda ev: [meta(ev, k, lambda m: m.update(repo_hash_after="0" * 64)) for k in ("ship", "lane")], "repository_changed"),
    "dep_info_outside": (lambda ev: inv(ev, "lane", lambda m: m["dep_info"].update({k: v.replace("\n", " /outside/x.rs\n", 1) for k, v in m["dep_info"].items() if "/cfg_if-" in k})), "consumed_outside_inventory"),
    "env_dep_differs": (lambda ev: inv(ev, "lane", lambda m: m["dep_info"].update({k: v + "\n# env-dep:ROUTE_PLANT=1\n" for k, v in m["dep_info"].items() if "/cfg_if-" in k})), "env_dep_differs"),
    "compiler_not_pinned": (lambda ev: on_unit(ev, "lane", TP, lambda l: re.sub(r" /\S+/bin/rustc ", " /usr/local/bin/rustc ", l, count=1)), "compiler_not_pinned"),
    "wrapper_on_third_party": (lambda ev: on_unit(ev, "lane", TP, lambda l: re.sub(r" (/\S+/bin/rustc) ", lambda m: f" {ev_wrapper[0]} {m.group(1)} ", l, count=1)), "wrapper_prefix_wrong"),
    "out_dir_file_changed": (None, "generated_input_differs"),  # real: kept lane target, re-inventoried
}
ev_wrapper = [None]


# ---- runner -----------------------------------------------------------------------

def a7_run(c, opts, evidence, verdict, keep=False):
    env = dict(os.environ, **opts.get("env", {}))
    for name in opts.get("unset", []):
        env.pop(name, None)
    argv = [sys.executable, "-I", a7.__file__, "run", "--repo", c["tree"], "--target", c["target"], "--evidence", evidence,
            "--wrapper", opts.get("wrapper", c["wrapper"]), "--wrapper-sha256", c["wrapper_sha"],
            "--toolchain", opts.get("toolchain", c["toolchain"]), "--json", verdict]
    argv += ["--keep-lane-target"] if keep else []
    argv += ["--continue-after-refusal"] if opts.get("continue") else []
    argv += [f"--ship-extra-arg={a}" for a in opts.get("ship_args", [])] + [f"--lane-extra-arg={a}" for a in opts.get("lane_args", [])]
    if "envb" in opts:
        env = {os.fsencode(k): os.fsencode(v) for k, v in env.items()} | opts["envb"]
    return subprocess.run(argv, env=env, capture_output=True).returncode


def kinds_of(verdict):
    return sorted({x["kind"] for x in a7.read_json(verdict)["differences"]})


def setup(name, args, base=None):
    root = os.path.join(args.work, name)
    shutil.rmtree(root, ignore_errors=True)
    os.makedirs(root)
    tree = os.path.join(root, "src")
    shutil.copytree(base or args.source, tree, symlinks=True, ignore=shutil.ignore_patterns(".git", "target", "__pycache__"))
    return {"root": root, "tree": tree, "target": os.path.join(root, "target"), "wrapper": args.wrapper, "wsrc": args.wrapper_source,
            "wrapper_sha": a7.sha256_file(args.wrapper), "toolchain": args.toolchain, "python_wrapper": args.python_wrapper}


def result(name, kind, expected, code, kinds, began, **extra):
    want = [] if expected is None else ([expected] if isinstance(expected, str) else expected)
    ok = (code == 0 and not kinds) if expected is None else (code == 1 and all(k in kinds for k in want) and "build_failed" not in kinds)
    return {"control": name, "kind": kind, "ok": ok, "exit": code, "expected": want or "pass", "kinds": kinds,
            "seconds": round(time.monotonic() - began, 1), **extra}


def run_pre(name, args):
    began, (plant, expected) = time.monotonic(), PRE[name]
    c = setup(name, args)
    opts = plant(c)
    opts = opts if isinstance(opts, dict) else {}
    code = a7_run(c, opts, os.path.join(c["root"], "evidence"), os.path.join(c["root"], "verdict.json"))
    built = os.path.exists(os.path.join(c["root"], "evidence"))
    reasons = sorted({x.get("reason", "") for x in a7.read_json(os.path.join(c["root"], "verdict.json"))["differences"]})
    out = result(name, "pre", expected, code, kinds_of(os.path.join(c["root"], "verdict.json")), began, built=built, reasons=reasons)
    out["ok"] = out["ok"] and not built
    shutil.rmtree(c["root"])
    return out


def run_pair(name, args):
    began, (plant, expected) = time.monotonic(), PAIR[name]
    c = setup(name, args)
    opts = plant(c)
    opts = opts if isinstance(opts, dict) else {}
    code = a7_run(c, opts, os.path.join(c["root"], "evidence"), os.path.join(c["root"], "verdict.json"))
    out = result(name, "pair", expected, code, kinds_of(os.path.join(c["root"], "verdict.json")), began)
    if name != "c_locale":  # kept as the base of c_locale_python_wrapper
        shutil.rmtree(c["tree"]), shutil.rmtree(c["target"], ignore_errors=True)
    return out


def run_one(name, args):
    """Rebuild one side beside the base (or named pair's) evidence of the same tree."""
    began, (side, plant, expected) = time.monotonic(), ONE[name]
    c = setup(name, args)
    opts = plant(c)
    opts = opts if isinstance(opts, dict) else {}
    base = os.path.join(args.work, opts.get("base", "base"))
    c["tree"], c["target"] = a7.read_json(os.path.join(base, "evidence", "ship", "meta.json"))["repo"], os.path.join(base, "target")
    evidence = os.path.join(c["root"], "evidence")
    other = "ship" if side == "lane" else "lane"
    shutil.copytree(os.path.join(base, "evidence", other), os.path.join(evidence, other))
    shutil.rmtree(c["target"], ignore_errors=True)
    if os.path.exists(c["target"] + ".route_rustc.log"):
        os.unlink(c["target"] + ".route_rustc.log")
    # The base runner's own environment (its launch block less the runner's settings).
    env = dict(x.split("=", 1) for x in a7.read_json(os.path.join(base, "evidence", other, "meta.json"))["launch_block"])
    env.pop("CARGO_TARGET_DIR"), env.pop("RUSTC")
    env["PATH"] = env["PATH"].split(os.pathsep, 1)[1]
    extra = {}
    if True:
        old = dict(os.environ)
        os.environ.clear(), os.environ.update(env)
        try:
            if opts.get("tamper"):
                extra = capture_tampered(c, evidence)
            else:
                a7.capture(side, c["tree"], c["target"], os.path.join(evidence, side), opts.get("wrapper", c["wrapper"]), c["toolchain"],
                           opts.get(f"{side}_args", []))
        finally:
            os.environ.clear(), os.environ.update(old)
        shutil.rmtree(c["target"], ignore_errors=True)
    for k in ("repo_hash_before", "repo_hash_after", "pre_build"):
        meta(evidence, side, lambda m: m.update({k: a7.read_json(os.path.join(evidence, other, "meta.json"))[k]}))
    verdict = os.path.join(c["root"], "verdict.json")
    used = opts.get("wrapper", c["wrapper"])  # the wrapper the lane named, checked against the real pin
    code = a7.emit(a7.compare(os.path.join(evidence, "ship"), os.path.join(evidence, "lane"), used, c["wrapper_sha"]), verdict)
    out = result(name, side, expected, code, kinds_of(verdict), began, **extra)
    if extra:
        out["ok"] = out["ok"] and extra["tamper"].get("consumer_running_at_tamper") is False
    return out


def capture_tampered(c, evidence):
    """The lane capture with the proc-macro tamper applied while Cargo is stopped."""
    done, real_run = {}, subprocess.run

    def run(command, **kw):
        if command[0].endswith("/cargo") and "test" in command and "--unit-graph" not in command:
            proc = subprocess.Popen(command, **kw)
            tamper_proc_macro(c["target"], proc, done)
            proc.wait()
            return subprocess.CompletedProcess(command, proc.returncode)
        return real_run(command, **kw)

    a7.subprocess.run = run
    try:
        a7.capture("lane", c["tree"], c["target"], os.path.join(evidence, "lane"), c["wrapper"], c["toolchain"])
    finally:
        a7.subprocess.run = real_run
    return {"tamper": done}


def run_evidence(name, args):
    began, (plant, expected) = time.monotonic(), EVIDENCE[name]
    base = os.path.join(args.work, "base")
    root = os.path.join(args.work, "evidence_" + name)
    shutil.rmtree(root, ignore_errors=True)
    shutil.copytree(os.path.join(base, "evidence"), root)
    ev_wrapper[0] = args.wrapper
    if plant is None:  # OUT_DIR file changed after its run, in the kept lane target
        out_file = next(p for p in sorted(glob.glob(os.path.join(base, "target/release/build/*/out/*"))) if os.path.isfile(p))
        tampered = os.path.join(args.work, "tampered_target")
        shutil.rmtree(tampered, ignore_errors=True)
        shutil.copytree(os.path.join(base, "target"), tampered, symlinks=True)
        with open(tampered + out_file[len(os.path.join(base, "target")):], "ab") as handle:
            handle.write(b"\0route-plant")
        a7.snapshot_inventory(tampered, os.path.join(root, "lane"))
        shutil.rmtree(tampered)
    else:
        plant(root)
    verdict = root + ".verdict.json"
    code = a7.emit(a7.compare(os.path.join(root, "ship"), os.path.join(root, "lane"), args.wrapper, a7.sha256_file(args.wrapper)), verdict)
    out = result(name, "evidence", expected, code, kinds_of(verdict), began)
    shutil.rmtree(root)
    return out


def main():
    parser = argparse.ArgumentParser()
    for name in ("--source", "--work", "--wrapper-source", "--toolchain", "--python-wrapper"):
        parser.add_argument(name, required=True)
    parser.add_argument("--only", action="append", default=[])
    parser.add_argument("--jobs", type=int, default=3)
    args = parser.parse_args()
    args.work = os.path.abspath(args.work)
    os.makedirs(args.work, exist_ok=True)
    args.wrapper = os.path.join(args.work, "route_rustc")
    subprocess.run(CC + [args.wrapper, args.wrapper_source], check=True)
    began, results = time.monotonic(), []
    selected = lambda table: [n for n in table if not args.only or n in args.only]
    # The base: a real copy of the tree, both builds, the lane target kept.
    if not args.only or "base" in args.only:
        c = setup("base", args)
        b0 = time.monotonic()
        code = a7_run(c, {}, os.path.join(c["root"], "evidence"), os.path.join(c["root"], "verdict.json"), keep=True)
        results.append(result("base", "pair", None, code, kinds_of(os.path.join(c["root"], "verdict.json")), b0))
    # Evidence controls read the base's kept target before one-sided rebuilds reuse its path.
    with concurrent.futures.ThreadPoolExecutor(args.jobs) as pool:
        jobs = [pool.submit(run_evidence, n, args) for n in selected(EVIDENCE)]
        jobs += [pool.submit(run_pre, n, args) for n in selected(PRE)]
        jobs += [pool.submit(run_pair, n, args) for n in selected(PAIR)]
        results += [j.result() for j in jobs]
    results += [run_one(n, args) for n in selected(ONE)]
    summary = {"ok": all(r["ok"] for r in results), "seconds": round(time.monotonic() - began, 1), "results": results}
    a7.write_json(os.path.join(args.work, "summary.json"), summary)
    for r in results:
        print(f"{'PASS' if r['ok'] else 'FAIL'} {r['kind']:8} {r['control']} exit={r['exit']} expected={r['expected']} kinds={r['kinds']} {r['seconds']}s")
    print("all controls report" if summary["ok"] else "CONTROL FAILURES")
    return 0 if summary["ok"] else 1


if __name__ == "__main__":
    sys.exit(main())

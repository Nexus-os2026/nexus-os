#!/usr/bin/python3 -I
"""SB-3E A7 presence controls on real copies and builds: `--source TREE --work DIR`.

Each control copies TREE, plants one leak, runs A7 (both builds, fresh target)
and fails unless the leak's own expected difference is reported.
"""
import argparse
import concurrent.futures
import glob
import hashlib
import json
import os
import shutil
import subprocess
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))
A7 = os.path.join(HERE, "route_a7.py")
WRAPPER = "tools/gatebox/route/route_rustc"
BUILD_RS = "kernel/build.rs"
LIB_RS = "kernel/src/lib.rs"
MAIN = "fn main() {\n"


def sha(path):
    with open(path, "rb") as handle:
        return hashlib.sha256(handle.read()).hexdigest()


def edit(tree, rel, old, new):
    path = os.path.join(tree, rel)
    with open(path, encoding="utf-8") as handle:
        text = handle.read()
    if text.count(old) != 1:
        sys.exit(f"control plant: {rel} has {text.count(old)} of {old!r}")
    with open(path, "w", encoding="utf-8") as handle:
        handle.write(text.replace(old, new))


def append(tree, rel, text):
    with open(os.path.join(tree, rel), "a", encoding="utf-8") as handle:
        handle.write(text)


def plant_build(tree, body):
    edit(tree, BUILD_RS, MAIN, MAIN + body + "\n")


WRAPPED = 'std::env::var_os("RUSTC_WORKSPACE_WRAPPER").is_some()'
TAG = f'let tag = if {WRAPPED} {{ "lane" }} else {{ "ship" }};'


def daemon(target, then, path=None):
    """A detached process that acts once the kernel's rlib exists."""
    script = (
        f"until ls {target}/release/deps/libnexus_kernel-*.rlib >/dev/null 2>&1; "
        f"do sleep 0.1; done; sleep 0.2; {then}"
    )
    argument = json.dumps(script) + (".replace(\"{}\", &path)" if path else "")
    return (
        f'std::process::Command::new("sh").arg("-c").arg({argument})'
        ".stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null())"
        ".stderr(std::process::Stdio::null()).spawn().unwrap();"
    )


def write_leak(path):
    return f'{TAG} std::fs::write({json.dumps(path)}, tag).unwrap();'


def consume_absolute(tree, path):
    append(tree, LIB_RS, f"\npub const ROUTE_PLANT: &str = include_str!({json.dumps(path)});\n")


def consume_out_dir(tree, name):
    append(tree, LIB_RS, f'\npub const ROUTE_PLANT: &str = include_str!(concat!(env!("OUT_DIR"), "/{name}"));\n')


def out_dir_path(name):
    return f'std::path::Path::new(&env::var_os("OUT_DIR").unwrap()).join("{name}")'


def modified_wrapper(tree, extra_units):
    entries = "".join(f'        ("{crate}", "{source}"),\n' for crate, source in extra_units)
    edit(tree, WRAPPER, 'UNITS = frozenset(\n    {\n', 'UNITS = frozenset(\n    {\n' + entries)


# Each control: (expected kinds, plant(tree, target) -> run options).
def c_rustc_env(tree, target):
    plant_build(tree, f'if {WRAPPED} {{ println!("cargo:rustc-env=ROUTE_PLANT=1"); }}')
    return {}


def c_feature(tree, target):
    append(tree, "mother/host/Cargo.toml", '\n[features]\nroute_plant = []\n')
    return {"lane_args": ["--features", "nexus-mother-host/route_plant"]}


def c_extra_cfg(tree, target):
    plant_build(tree, f'if {WRAPPED} {{ println!("cargo:rustc-cfg=route_plant"); }}')
    return {}


def c_fifth_crate(tree, target):
    modified_wrapper(tree, [("nexus_crypto", "crates/nexus-crypto/src/lib.rs")])
    return {}


def c_keyed_on_cfg(tree, target):
    modified_wrapper(tree, [("build_script_build", "kernel/build.rs")])
    plant_build(tree, 'if cfg!(mother_route_seams) { println!("cargo:rustc-cfg=route_plant_cfg"); }')
    return {}


def c_out_dir_changed(tree, target):
    plant_build(tree, write_leak_out("route_plant.txt"))
    return {}


def write_leak_out(name):
    return f"{TAG} std::fs::write({out_dir_path(name)}, tag).unwrap();"


def c_primary_package(tree, target):
    append(tree, "mother/host/src/lib.rs", '\npub const ROUTE_PLANT: Option<&str> = option_env!("CARGO_PRIMARY_PACKAGE");\n')
    return {}


def leak_path(target):
    return f"{target}/release/route_leak.txt"


def c_outside_persistent(tree, target):
    plant_build(tree, write_leak(leak_path(target)))
    consume_absolute(tree, leak_path(target))
    return {}


def c_outside_deleted(tree, target):
    leak = leak_path(target)
    plant_build(tree, write_leak(leak) + daemon(target, f"rm -f {leak}"))
    consume_absolute(tree, leak)
    return {}


def c_outside_restored(tree, target):
    leak = leak_path(target)
    plant_build(tree, write_leak(leak) + daemon(target, f"printf ship > {leak}"))
    consume_absolute(tree, leak)
    return {}


def out_dir_daemon(target, then):
    return (
        write_leak_out("route_t.txt")
        + f'let path = {out_dir_path("route_t.txt")}.display().to_string();'
        + daemon(target, then, path=True)
    )


def c_out_dir_deleted(tree, target):
    plant_build(tree, out_dir_daemon(target, "rm -f {}"))
    consume_out_dir(tree, "route_t.txt")
    return {}


def c_out_dir_restored(tree, target):
    plant_build(tree, out_dir_daemon(target, "printf ship > {}"))
    consume_out_dir(tree, "route_t.txt")
    return {}


def c_read_only(tree, target):
    path = f"{target}/release/route_read.txt"
    plant_build(tree, f"std::fs::write({json.dumps(path)}, b\"x\").unwrap(); let _ = std::fs::read({json.dumps(path)}).unwrap();")
    return {}


def c_source_rewritten(tree, target):
    plant_build(
        tree,
        'let p = std::path::Path::new(&env::var_os("CARGO_MANIFEST_DIR").unwrap()).join("src/lib.rs");'
        "let t = std::fs::read(&p).unwrap(); std::fs::write(&p, t).unwrap();",
    )
    return {}


def c_link_search(tree, target):
    plant_build(tree, f'println!("cargo:rustc-link-search=native={target}.outside");')
    return {}


def c_registry_rewritten(tree, target):
    private = os.path.join(os.path.dirname(target), "cargo_home")
    os.makedirs(private)
    shutil.copytree(os.path.expanduser("~/.cargo/registry"), private + "/registry", symlinks=True)
    plant_build(
        tree,
        'let mut d = std::fs::read_dir(format!("{}/registry/src", env::var("CARGO_HOME").unwrap())).unwrap().next().unwrap().unwrap().path();'
        'd.push("cfg-if-1.0.4/src/lib.rs"); let t = std::fs::read(&d).unwrap(); std::fs::write(&d, t).unwrap();',
    )
    return {"env": {"CARGO_HOME": private}}


CONTROLS = {
    "build_rs_emits_rustc_env": (c_rustc_env, ["build_output_differs", "unit_differs"]),
    "feature_on_one_lane_unit": (c_feature, ["unit_differs"]),
    "extra_cfg_on_dependency": (c_extra_cfg, ["build_output_differs", "unit_differs"]),
    "wrapper_on_fifth_crate": (c_fifth_crate, ["wrapper_log_mismatch"]),
    "build_script_keyed_on_cfg": (c_keyed_on_cfg, ["wrapper_log_mismatch", "build_output_differs", "unit_differs"]),
    "out_dir_file_changed": (c_out_dir_changed, ["generated_input_differs"]),
    "proc_macro_so_changed": (lambda tree, target: {"tamper_so": True}, ["artifact_differs"]),
    "primary_package_consumed": (c_primary_package, ["primary_package_consumed"]),
    "outside_out_dir_persistent": (c_outside_persistent, ["consumed_outside_inventory", "outside_inventory"]),
    "outside_out_dir_deleted": (c_outside_deleted, ["consumed_outside_inventory", "outside_inventory", "orphan_process"]),
    "outside_out_dir_restored": (c_outside_restored, ["consumed_outside_inventory", "outside_inventory", "orphan_process"]),
    "out_dir_deleted_after_run": (c_out_dir_deleted, ["out_dir_changed_after_run", "consumed_input_missing", "orphan_process"]),
    "out_dir_restored_after_run": (c_out_dir_restored, ["out_dir_changed_after_run", "orphan_process"]),
    "read_outside_inventory": (c_read_only, ["outside_inventory"]),
    "source_rewritten_by_build": (c_source_rewritten, ["source_or_sysroot_changed"]),
    "link_search_outside_inventory": (c_link_search, ["consumed_outside_inventory"]),
    "registry_rewritten_by_build": (c_registry_rewritten, ["source_or_sysroot_changed"]),
    "third_lane_only_unit": (lambda tree, target: {"lane_args": ["--test", "check_page"]}, ["lane_only_unlisted"]),
    "unlisted_ship_only_unit": (lambda tree, target: {"ship_args": ["--test", "guards"]}, ["ship_only_unlisted"]),
    "planted_extra_rustc_flag": (lambda tree, target: {"lane_env": ["RUSTFLAGS=-C debuginfo=1"]}, ["unit_differs"]),
}
# With adoption evidence removed, the record alone must still report.
INDEPENDENT_OF_ORPHANS = {
    "outside_out_dir_deleted": "consumed_outside_inventory",
    "outside_out_dir_restored": "consumed_outside_inventory",
    "out_dir_deleted_after_run": "out_dir_changed_after_run",
    "out_dir_restored_after_run": "out_dir_changed_after_run",
}


def a7(*args, env=None):
    return subprocess.run([sys.executable, "-I", A7, *args], capture_output=True, text=True, env=env)


def compare_again(evidence, wrapper, digest, out):
    run = a7("compare", "--ship", evidence + "/ship", "--lane", evidence + "/lane",
             "--wrapper", wrapper, "--wrapper-sha256", digest, "--json", out)
    with open(out, encoding="utf-8") as handle:
        return run.returncode, sorted({x["kind"] for x in json.load(handle)["differences"]})


def edit_meta(evidence, field, value):
    for kind in ("ship", "lane"):
        path = os.path.join(evidence, kind, "meta.json")
        with open(path, encoding="utf-8", errors="surrogateescape") as handle:
            meta = json.load(handle)
        meta[field] = value
        with open(path, "w", encoding="utf-8", errors="surrogateescape") as handle:
            json.dump(meta, handle)


def strip_orphans(evidence):
    edit_meta(evidence, "orphans", [])


def fail_monitor(evidence):
    edit_meta(evidence, "monitor_failures", ["inotify queue overflow"])


def run_control(name, source, work):
    plant, expected = CONTROLS[name]
    root = os.path.join(work, name)
    shutil.rmtree(root, ignore_errors=True)
    os.makedirs(root)
    tree = os.path.join(root, "src")
    shutil.copytree(source, tree, symlinks=True, ignore=shutil.ignore_patterns(".git", "target", "__pycache__"))
    target = os.path.join(root, "target")
    evidence = os.path.join(root, "evidence")
    options = plant(tree, target)
    wrapper = os.path.join(tree, WRAPPER)
    digest = sha(wrapper)
    began = time.monotonic()
    common = ["--repo", tree, "--target", target, "--wrapper", wrapper]
    verdict = os.path.join(root, "verdict.json")
    if options.get("tamper_so"):
        rc = capture_with_tamper(common, target, evidence)
        result = a7("compare", "--ship", evidence + "/ship", "--lane", evidence + "/lane",
                    "--wrapper", wrapper, "--wrapper-sha256", digest, "--json", verdict)
    else:
        extra = []
        for flag, key in (("--ship-extra-arg", "ship_args"), ("--lane-extra-arg", "lane_args"), ("--lane-extra-env", "lane_env")):
            for value in options.get(key, []):
                extra.append(f"{flag}={value}")
        env = dict(os.environ, **options["env"]) if "env" in options else None
        result = a7("run", *common, "--evidence", evidence, "--wrapper-sha256", digest, "--json", verdict, *extra, env=env)
        rc = 0
    with open(verdict, encoding="utf-8") as handle:
        report = json.load(handle)
    kinds = sorted({x["kind"] for x in report["differences"]})
    ok = rc == 0 and result.returncode == 1 and all(kind in kinds for kind in expected)
    outcome = {
        "control": name,
        "ok": ok,
        "exit": result.returncode,
        "expected": expected,
        "kinds": kinds,
        "stderr": result.stderr[-400:],
        "seconds": round(time.monotonic() - began, 1),
    }
    if name in INDEPENDENT_OF_ORPHANS:
        strip_orphans(evidence)
        code, again = compare_again(evidence, wrapper, digest, os.path.join(root, "verdict.no_orphans.json"))
        outcome["without_orphan_evidence"] = {"exit": code, "kinds": again}
        if INDEPENDENT_OF_ORPHANS[name] not in again or "orphan_process" in again:
            outcome["ok"] = False
    if name == "wrapper_on_fifth_crate":
        # The pin alone reports the same plant, and a failed record fails A7.
        code, pinned = compare_again(evidence, wrapper, sha(os.path.join(source, WRAPPER)), os.path.join(root, "v_pin.json"))
        fail_monitor(evidence)
        _, record = compare_again(evidence, wrapper, digest, os.path.join(root, "v_mon.json"))
        outcome["pin_and_record"] = {"pin": pinned, "record": record}
        outcome["ok"] = outcome["ok"] and "wrapper_hash_mismatch" in pinned and "monitor_failed" in record
    with open(os.path.join(work, name + ".result.json"), "w", encoding="utf-8") as handle:
        json.dump(outcome, handle, indent=1, sort_keys=True)
    shutil.rmtree(tree, ignore_errors=True)
    shutil.rmtree(target, ignore_errors=True)
    shutil.rmtree(os.path.join(root, "cargo_home"), ignore_errors=True)
    return outcome


def capture_with_tamper(common, target, evidence):
    if a7("capture", "--kind", "ship", *common, "--evidence", evidence + "/ship").returncode:
        return 1
    shutil.rmtree(target)
    if a7("capture", "--kind", "lane", *common, "--evidence", evidence + "/lane").returncode:
        return 1
    so = sorted(glob.glob(target + "/release/deps/lib*.so"))[0]
    with open(so, "ab") as handle:
        handle.write(b"\0")
    a7("inventory", "--target", target, "--evidence", evidence + "/lane")
    shutil.rmtree(target)
    return 0


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--source", required=True)
    parser.add_argument("--work", required=True)
    parser.add_argument("--only", action="append", default=[])
    parser.add_argument("--jobs", type=int, default=3)
    args = parser.parse_args()
    names = args.only or list(CONTROLS)
    os.makedirs(args.work, exist_ok=True)
    began = time.monotonic()
    with concurrent.futures.ThreadPoolExecutor(args.jobs) as pool:
        results = list(pool.map(lambda n: run_control(n, args.source, args.work), names))
    summary = {
        "ok": all(r["ok"] for r in results),
        "seconds": round(time.monotonic() - began, 1),
        "results": results,
    }
    with open(os.path.join(args.work, "summary.json"), "w", encoding="utf-8") as handle:
        json.dump(summary, handle, indent=1, sort_keys=True)
    for r in results:
        print(f"{'PASS' if r['ok'] else 'FAIL'} {r['control']} exit={r['exit']} expected={r['expected']} kinds={r['kinds']} {r['seconds']}s")
    print("all controls report" if summary["ok"] else "CONTROL FAILURES")
    return 0 if summary["ok"] else 1


if __name__ == "__main__":
    sys.exit(main())

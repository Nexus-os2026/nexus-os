#!/usr/bin/env python3
"""P2 security closure R1: mutation controls for the retargeted Phase Zero
Wasmtime guard (p0_fg_dep_wasmtime_uses_no_dynamic_component_val_api).

In a clean checkout of the candidate (nothing else may build, test or edit
there): run the guard unmutated (it must pass); then, for each control,
restore one wrong state, run the guard alone, require it to fail on the
control's own marker, and restore the file byte for byte (SHA-256 and Git
status checked). Source mutations are `#[cfg(any())]` items, so the crates
still compile while the guard's text scan sees them.

Usage: guard_controls.py <checkout> <output directory> [--target-dir DIR]
"""
import hashlib, json, pathlib, subprocess, sys

TEST = "phase0_surface::fg_reliability::tests::p0_fg_dep_wasmtime_uses_no_dynamic_component_val_api"

CONTROLS = [
    dict(id="GC-01-WASMTIME-EXCEPTION-RETURNS",
         file="deny.toml",
         anchor="    { id = \"RUSTSEC-2026-0195\",",
         insert_before=True,
         text="    { id = \"RUSTSEC-2026-0327\", reason = \"wasmtime component async callbacks (control)\" },\n",
         marker="a Wasmtime advisory is accepted"),
    dict(id="GC-02-CALL-ASYNC-IN-PRODUCTION",
         file="sdk/src/wasmtime_sandbox.rs",
         anchor="use wasmtime::{Engine, Linker, Module, Store, StoreLimits, StoreLimitsBuilder};\n",
         insert_before=False,
         text="#[cfg(any())]\nfn control(f: wasmtime::TypedFunc<(), ()>) { let _ = f.call_async; }\n",
         marker="Wasmtime's async and component-model APIs are not used"),
    dict(id="GC-03-COMPONENT-IMPORT-IN-PRODUCTION",
         file="sdk/src/module_cache.rs",
         anchor="use wasmtime::{Engine, Module};\n",
         insert_before=False,
         text="#[cfg(any())]\nuse wasmtime::component::Val;\n",
         marker="Wasmtime's component module is not used"),
    dict(id="GC-04-CRATE-ALIAS-IN-PRODUCTION",
         file="sdk/src/wasm_agent.rs",
         anchor="use wasmtime::Engine;\n",
         insert_before=False,
         text="#[cfg(any())]\nuse wasmtime as wt;\n",
         marker="the wasmtime crate is not aliased or renamed"),
]


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def status(checkout):
    return subprocess.run(["git", "-C", str(checkout), "status", "--porcelain=v1", "--untracked-files=all"],
                          capture_output=True, text=True, check=True).stdout.splitlines()


def run_guard(checkout, target, log):
    cmd = ["cargo", "test", "-p", "nexus-desktop-backend", "--locked", "--lib", "--", TEST, "--exact"]
    env = dict(**__import__("os").environ)
    if target:
        env["CARGO_TARGET_DIR"] = target
    r = subprocess.run(cmd, cwd=checkout, capture_output=True, text=True, env=env)
    log.write_text(r.stdout + r.stderr)
    out = r.stdout + r.stderr
    passed = r.returncode == 0 and "test result: ok. 1 passed" in out
    return r.returncode, passed, out


def main():
    checkout = pathlib.Path(sys.argv[1]).resolve()
    out = pathlib.Path(sys.argv[2]); out.mkdir(parents=True, exist_ok=True)
    target = sys.argv[sys.argv.index("--target-dir") + 1] if "--target-dir" in sys.argv else None
    files = sorted({c["file"] for c in CONTROLS})
    original = {f: sha(checkout / f) for f in files}
    before_status = status(checkout)
    rc, passed, _ = run_guard(checkout, target, out / "baseline.log")
    results = dict(baseline=dict(exit=rc, passed=passed), controls=[])
    for c in CONTROLS:
        path = checkout / c["file"]
        data = path.read_bytes()
        text = data.decode()
        assert text.count(c["anchor"]) == 1, (c["id"], "anchor")
        if c["insert_before"]:
            mutated = text.replace(c["anchor"], c["text"] + c["anchor"])
        else:
            mutated = text.replace(c["anchor"], c["anchor"] + c["text"])
        try:
            path.write_text(mutated)
            rc, passed, log = run_guard(checkout, target, out / f"{c['id']}.log")
            failed = rc != 0 and "test result: FAILED" in log and "panicked" in log
            compiled = "error[E" not in log and "could not compile" not in log
        finally:
            path.write_bytes(data)
        restored = sha(path) == original[c["file"]]
        results["controls"].append(dict(id=c["id"], file=c["file"], marker=c["marker"], exit=rc,
                                        compiled=compiled, failed=failed,
                                        marker_found=c["marker"] in log, restored=restored,
                                        detected=compiled and failed and c["marker"] in log))
    final = {f: sha(checkout / f) for f in files}
    results["identical"] = final == original
    results["status_before"] = before_status
    results["status_after"] = status(checkout)
    results["all_detected"] = all(x["detected"] for x in results["controls"])
    results["ok"] = (results["baseline"]["passed"] and results["all_detected"] and results["identical"]
                     and results["status_after"] == before_status)
    (out / "summary.json").write_text(json.dumps(results, indent=1))
    for x in results["controls"]:
        print(f"{x['id']}: detected={x['detected']} (exit {x['exit']}, marker {x['marker_found']}, restored {x['restored']})")
    print(f"baseline passes: {results['baseline']['passed']}; files identical after: {results['identical']}; "
          f"status unchanged: {results['status_after'] == before_status}; RESULT: {'PASS' if results['ok'] else 'FAIL'}")
    sys.exit(0 if results["ok"] else 1)


if __name__ == "__main__":
    main()

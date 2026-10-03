#!/usr/bin/env python3
"""P2-V1-R3B-I4-R1 normal-build API guards: what a normal external caller of
the sandbox crate (the desktop, a release build) can name.

The sandbox crate's own integration tests are always built with its
`live-sandbox-harness` feature (its dev-dependency enables it), so they
cannot show the normal build's surface. Each probe here is instead the main
source of a scratch binary crate outside the workspace (an empty
`[workspace]` table), depending on the candidate's sandbox crate by path
with no feature enabled, and type-checked with `cargo check --offline`
(the candidate's Cargo.lock copied, so every dependency resolves to the
locked version from the local registry cache; nothing is fetched).

Each counted probe attempts one path the I4-R1 closure removed or gated,
and must:

- fail to build, with exactly its expected error lines (the distinct
  `error[...]` header lines, "could not compile" excluded), against the
  candidate with no feature of the sandbox crate enabled;
- self-check, so the failure is the closure and not a broken probe:
  - "gated" (compiled only for the crate's tests and live harness): the
    same probe builds, with no error, once the scratch crate enables the
    `live-sandbox-harness` feature;
  - "removed" (gone from every build): the same probe builds, with no
    error, against the base commit, where B-R1-5 showed it was normal
    public API.

A positive control (the production route: `ScopeManager::connect`, then
`execution::run`, then `RetainedBoundary::retry`) must build with no error,
and `cargo tree -e features` must show that the normal build enables no
feature of the sandbox crate but its default (and the gated self-checks
exactly `live-sandbox-harness`).

Nothing here runs a probe: each is type-checked only. No bus is contacted.

Usage:
  normal_api_probes.py <candidate checkout> <base checkout> <scratch dir (must not exist)>
                       <log directory> --target-dir <dir (must not exist)>
"""
import json
import pathlib
import shutil
import subprocess
import sys

SANDBOX = "crates/nexus-verifier-sandbox"

HEADER = r'''//! P2-V1-R3B-I4-R1 normal-build probe: an external caller of the sandbox
//! crate, built with no feature of that crate enabled.
#![allow(unused, dead_code, unreachable_code, clippy::all)]

use nexus_verifier_sandbox::execution::{self, Cleanup, RetainedBoundary};
use nexus_verifier_sandbox::launcher::{Helper, HelperProgram, LaunchSpec};
use nexus_verifier_sandbox::policy::ResourcePolicy;
use nexus_verifier_sandbox::scope::ScopeManager;

fn probe(scopes: &ScopeManager, helper: Helper, spec: LaunchSpec, boundary: RetainedBoundary) {
'''
FOOTER = r'''
}

fn main() {
    let _ = probe as fn(&ScopeManager, Helper, LaunchSpec, RetainedBoundary);
}
'''

POSITIVE = '''    // The production route: the only manager constructor, the only start.
    if let Ok(scopes) = ScopeManager::connect() {
        if let Ok(program) = HelperProgram::installed() {
            let report = execution::run(&scopes, &program, spec, &ResourcePolicy::RUST_OFFLINE_V1);
            if let Cleanup::Failed(boundary) = report.cleanup {
                let _ = boundary.retry();
            }
        }
    }
    let _ = boundary.retry();'''


def probe(pid, kind, attempt, body, expect, why):
    return dict(id=pid, kind=kind, attempt=attempt, body=body, expect=list(expect), why=why)


PROBES = [
    probe("N-I4R1-CONNECT-AT", "gated",
          "construct the verifier's manager over a caller-selected bus socket (A-R1-6)",
          '''    let _ = ScopeManager::connect_at("/run/user/1000/a-socket-the-caller-chose");''',
          ["error[E0599]: no function or associated item named `connect_at` found for struct `ScopeManager` in the current scope"],
          "a normal build derives the bus from the real uid only (ScopeManager::connect)"),
    probe("N-I4R1-DIRECT-START", "removed",
          "start a scope directly, outside execution::run (A-R1-1)",
          '''    let _ = scopes.start(&helper, &ResourcePolicy::RUST_OFFLINE_V1);''',
          ["error[E0599]: no method named `start` found for reference `&ScopeManager` in the current scope"],
          "a normal build starts a scope only within execution::run"),
    probe("N-I4R1-PENDING-SCOPE", "removed",
          "hold a pending scope operation apart from its helper (A-R1-2)",
          '''    fn held(_: Box<nexus_verifier_sandbox::scope::PendingScope>) {}''',
          ["error[E0603]: struct `PendingScope` is private"],
          "a pending operation is crate-private, owned only beside its helper"),
    probe("N-I4R1-START-FAILED", "removed",
          "receive a failed start that carries the operation without its helper (A-R1-2)",
          '''    fn failed(_: nexus_verifier_sandbox::scope::StartFailed) {}''',
          ["error[E0425]: cannot find type `StartFailed` in module `nexus_verifier_sandbox::scope`"],
          "no failure carries a pending operation apart from its helper"),
    probe("N-I4R1-PLACE", "gated",
          "use the live harness's direct scope start in a normal build",
          '''    let _ = execution::place(scopes, helper, &ResourcePolicy::RUST_OFFLINE_V1);''',
          ["error[E0425]: cannot find function `place` in module `execution`"],
          "the harness's direct owner exists only with the harness feature"),
    probe("N-I4R1-SCOPED-HELPER", "gated",
          "name the harness's direct owner in a normal build",
          '''    fn placed(_: execution::ScopedHelper) {}''',
          ["error[E0425]: cannot find type `ScopedHelper` in module `execution`"],
          "the harness's direct owner exists only with the harness feature"),
    probe("N-I4R1-PLACEMENT-FAILED", "gated",
          "name the harness's direct failure in a normal build",
          '''    fn failed(_: execution::PlacementFailed) {}''',
          ["error[E0425]: cannot find type `PlacementFailed` in module `execution`"],
          "the harness's direct owner exists only with the harness feature"),
    probe("N-I4R1-RUN-WITH-FAULT", "gated",
          "inject a fault into an execution in a normal build",
          '''    let _ = execution::run_with_fault;''',
          ["error[E0425]: cannot find value `run_with_fault` in module `execution`"],
          "fault injection exists only with the harness feature"),
    probe("N-I4R1-HOLDS", "gated",
          "inspect a retained boundary's contents in a normal build",
          '''    let _ = (boundary.holds_scope(), boundary.holds_helper());''',
          ["error[E0599]: no method named `holds_scope` found for struct `RetainedBoundary` in the current scope",
           "error[E0599]: no method named `holds_helper` found for struct `RetainedBoundary` in the current scope"],
          "a retained boundary is only retried (or, defense in depth, dropped)"),
]

HARNESS = probe("N-I4R1-HARNESS-CHECK", "harness", "a deliberate type error",
                '''    let _: u32 = "not a number";''',
                ["error[E0308]: mismatched types"],
                "proves that the harness sees build errors")

SCRATCH = None
TARGET = None


def manifest(sandbox: pathlib.Path, features):
    feature_list = ", ".join(f'"{f}"' for f in features)
    return (
        "[package]\n"
        'name = "i4r1-normal-caller"\n'
        'version = "0.0.0"\n'
        'edition = "2021"\n'
        "publish = false\n\n"
        "[dependencies]\n"
        f'nexus-verifier-sandbox = {{ path = "{sandbox}", default-features = true, '
        f"features = [{feature_list}] }}\n\n"
        "[workspace]\n"
    )


def build(name, checkout: pathlib.Path, features, body):
    crate = SCRATCH / name
    if crate.exists():
        shutil.rmtree(crate)
    (crate / "src").mkdir(parents=True)
    (crate / "Cargo.toml").write_text(manifest(checkout / SANDBOX, features))
    shutil.copyfile(checkout / "Cargo.lock", crate / "Cargo.lock")
    (crate / "src" / "main.rs").write_text(HEADER + body + FOOTER)
    target = TARGET / ("harness" if features else "normal") / (
        "base" if checkout == BASE else "candidate")
    proc = subprocess.run(
        ["cargo", "check", "--offline", "--quiet", "--target-dir", str(target)],
        cwd=crate, capture_output=True, text=True, timeout=1800)
    output = proc.stdout + proc.stderr
    errors = sorted({line.strip() for line in output.splitlines() if line.startswith("error[")})
    return proc.returncode, errors, output, crate


def features_of(crate: pathlib.Path):
    """The sandbox crate's enabled features in the scratch crate's build
    (resolution only: `cargo tree` builds nothing)."""
    proc = subprocess.run(
        ["cargo", "tree", "--offline", "-e", "features", "-i", "nexus-verifier-sandbox"],
        cwd=crate, capture_output=True, text=True, timeout=600)
    return proc.returncode, proc.stdout + proc.stderr


CANDIDATE = None
BASE = None


def main():
    global SCRATCH, TARGET, CANDIDATE, BASE
    args = sys.argv[1:]
    if "--target-dir" not in args:
        raise SystemExit(__doc__)
    at = args.index("--target-dir")
    TARGET = pathlib.Path(args[at + 1]).resolve()
    del args[at:at + 2]
    if len(args) != 4:
        raise SystemExit(__doc__)
    CANDIDATE = pathlib.Path(args[0]).resolve()
    BASE = pathlib.Path(args[1]).resolve()
    SCRATCH = pathlib.Path(args[2]).resolve()
    log = pathlib.Path(args[3]).resolve()
    for path in (SCRATCH, TARGET):
        if path.exists():
            raise SystemExit(f"exists: {path}")
    SCRATCH.mkdir(parents=True)
    TARGET.mkdir(parents=True)
    log.mkdir(parents=True, exist_ok=True)
    results = []
    code, errors, output, crate = build("positive", CANDIDATE, [], POSITIVE)
    (log / "N-I4R1-POSITIVE-CONTROL.log").write_text(output)
    tree_code, tree = features_of(crate)
    (log / "N-I4R1-POSITIVE-CONTROL.features.txt").write_text(tree)
    normal_features = tree_code == 0 and 'nexus-verifier-sandbox feature "default"' in tree \
        and "live-sandbox-harness" not in tree
    positive = dict(id="N-I4R1-POSITIVE-CONTROL", kind="positive", exit=code, errors=errors,
                    features_default_only=normal_features,
                    ok=code == 0 and not errors and normal_features)
    print(json.dumps(positive), flush=True)
    for p in PROBES + [HARNESS]:
        code, errors, output, crate = build(p["id"], CANDIDATE, [], p["body"])
        (log / f"{p['id']}.log").write_text(output)
        exact = code != 0 and errors == sorted(p["expect"])
        check = True
        check_errors = []
        check_features = None
        if p["kind"] == "gated":
            c_code, check_errors, c_output, c_crate = build(
                p["id"] + "-harness", CANDIDATE, ["live-sandbox-harness"], p["body"])
            (log / f"{p['id']}.self-check-harness.log").write_text(c_output)
            t_code, tree = features_of(c_crate)
            (log / f"{p['id']}.self-check-harness.features.txt").write_text(tree)
            check_features = t_code == 0 and \
                'nexus-verifier-sandbox feature "live-sandbox-harness"' in tree
            check = c_code == 0 and not check_errors and check_features
        elif p["kind"] == "removed":
            c_code, check_errors, c_output, _ = build(p["id"] + "-base", BASE, [], p["body"])
            (log / f"{p['id']}.self-check-base.log").write_text(c_output)
            check = c_code == 0 and not check_errors
        ok = exact and check
        result = dict(id=p["id"], kind=p["kind"], attempt=p["attempt"], why=p["why"],
                      expected=sorted(p["expect"]), errors=errors, exact=exact,
                      self_check_builds=check, self_check_errors=check_errors,
                      self_check_harness_feature=check_features, ok=ok)
        results.append(result)
        print(json.dumps(result), flush=True)
    summary = dict(
        candidate=str(CANDIDATE),
        base=str(BASE),
        positive_control=positive["ok"],
        guards=len(PROBES),
        guards_by_kind={k: sum(1 for r in results if r["kind"] == k) for k in ("gated", "removed")},
        harness_check=all(r["ok"] for r in results if r["kind"] == "harness"),
        all_required=all(r["ok"] for r in results) and positive["ok"],
        failures=[r["id"] for r in results if not r["ok"]]
        + ([] if positive["ok"] else ["N-I4R1-POSITIVE-CONTROL"]),
    )
    print(json.dumps(summary), flush=True)
    (log / "summary.json").write_text(json.dumps(
        dict(summary=summary, positive=positive, guards=results), indent=2) + "\n")
    return 0 if summary["all_required"] else 1


if __name__ == "__main__":
    sys.exit(main())

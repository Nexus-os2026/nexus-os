#!/usr/bin/env python3
"""P2-V1-R3B-I4 compile-time API/type guards: the scope-ownership boundary,
checked from outside the crate.

Each probe is a small integration-test target written into a scratch
checkout as crates/nexus-verifier-sandbox/tests/i4_api_probe.rs. It reaches
the crate only as an external caller does (the desktop, the live harness),
through `nexus_verifier_sandbox::...` paths, and attempts one authority path
the ownership design closes: forging or editing a pending scope operation,
building a proven scope from a name, reaching the manager or kernel
interfaces (a caller-provided transport or destination), a test-only
constructor in a normal build, the retained boundary's contents, or a copy
or serialized form of an owner. Each counted probe must:

- fail to build, with exactly its expected error lines (the distinct
  `error[...]` header lines of the build, "could not compile" excluded):
  the failure is the intended privacy or type guard and nothing else;
- self-check. A visibility guard: once the guard is reopened in the scratch
  checkout's sources (each reopen edit's anchor occurs exactly once), the
  same probe builds with no error; the sources are then restored byte for
  byte and verified by SHA-256. A trait-property guard (no Clone, no
  Serialize or Deserialize): there is no reopen edit that could add the
  trait without other changes, so its self-check is the same probe asking
  the same bound of a type that has it (`ScopeEvents`, `String`), which
  builds.

Two more targets are not guards and are labelled so: the harness check (a
deliberate E0308, proving the harness sees errors) and a positive control
(the public API's ordinary use builds with no error).

These are API/type guards. They are reported apart from the behavioural
controls (scope_controls.py) and never added into one figure with them.
They prove the safe API surface only; they do not defend against `unsafe`
code.

Usage:
  api_guards.py <scratch checkout> <log directory> --target-dir <dir>

The scratch checkout must hold exactly the candidate's tree (never the
worktree under review); it is mutated and restored.
"""
import hashlib
import json
import os
import pathlib
import subprocess
import sys

CRATE = "crates/nexus-verifier-sandbox"
PROBE = f"{CRATE}/tests/i4_api_probe.rs"
SCOPE = f"{CRATE}/src/scope.rs"
PENDING = f"{CRATE}/src/scope/pending.rs"
MANAGER = f"{CRATE}/src/scope/manager.rs"
NATIVE = f"{CRATE}/src/scope/native.rs"
EXEC = f"{CRATE}/src/execution.rs"
LAUNCHER = f"{CRATE}/src/launcher.rs"
SOURCES = [SCOPE, PENDING, MANAGER, NATIVE, EXEC, LAUNCHER]

HEADER = r'''//! P2-V1-R3B-I4 API probe: an external caller of the sandbox crate.
#![cfg(all(target_os = "linux", target_arch = "x86_64"))]
#![allow(unused, dead_code, unreachable_code, private_interfaces)]

use nexus_verifier_sandbox::execution::{self, Cleanup, RetainedBoundary};
use nexus_verifier_sandbox::launcher::{Helper, HelperProgram};
use nexus_verifier_sandbox::policy::ResourcePolicy;
use nexus_verifier_sandbox::scope::{
    PendingScope, Scope, ScopeEvents, ScopeManager, StartFailed,
};

fn probe(
    scopes: &ScopeManager,
    helper: &Helper,
    failed: StartFailed,
    boundary: RetainedBoundary,
    pending: PendingScope,
    scope: Scope,
) {
'''
FOOTER = r'''
}

#[test]
fn the_probe_builds() {}
'''

# The guarded type that every visibility reopen of a pending or proven
# scope's fields also has to expose (the field's own type).
CGROUP_DIR = (NATIVE, "pub(crate) trait CgroupDir: Send + Sync + Debug {",
              "pub trait CgroupDir: Send + Sync + Debug {")
CONTROLLER = (SCOPE, "pub(crate) struct Controller {", "pub struct Controller {")
BOUNDARY_TYPE = [
    (SCOPE, "pub(crate) use pending::ScopeBoundary;", "pub use pending::ScopeBoundary;"),
    (PENDING, "pub(crate) enum ScopeBoundary {", "pub enum ScopeBoundary {"),
]


def probe(pid, kind, attempt, body, expect, reopen=(), self_check=None, why=""):
    return dict(id=pid, kind=kind, attempt=attempt, body=body, expect=list(expect),
                reopen=[dict(file=f, anchor=a, replacement=r) for (f, a, r) in reopen],
                self_check=self_check, why=why)


PROBES = [
    probe(
        "P-I4-PENDING-NEW", "visibility",
        "construct a pending scope operation outside the crate (from a unit name and a process id)",
        '''    let _ = PendingScope::new(todo!(), String::from("nexus-verifier-0.scope"), helper,
        &ResourcePolicy::RUST_OFFLINE_V1);''',
        ["error[E0624]: associated function `new` is private"],
        reopen=[(PENDING, "    pub(super) fn new(", "    pub fn new("), CONTROLLER],
        why="only ScopeManager::prepare creates one, bound to the retained helper and the issuing connection",
    ),
    probe(
        "P-I4-PENDING-STATE", "visibility",
        "mark a pending operation settled, or never issued, from outside",
        '''    let mut pending = pending;
    pending.settled = true;
    pending.issued = false;''',
        ["error[E0616]: field `settled` of struct `PendingScope` is private",
         "error[E0616]: field `issued` of struct `PendingScope` is private"],
        reopen=[(PENDING, "    issued: bool,", "    pub issued: bool,"),
                (PENDING, "    settled: bool,", "    pub settled: bool,")],
        why="only settling (reconcile) confirms an operation gone",
    ),
    probe(
        "P-I4-PENDING-CANDIDATE", "visibility",
        "drop or replace a pending operation's retained candidate",
        '''    let mut pending = pending;
    pending.candidate = None;''',
        ["error[E0616]: field `candidate` of struct `PendingScope` is private"],
        reopen=[(PENDING, "    candidate: Option<Box<dyn CgroupDir>>,",
                 "    pub candidate: Option<Box<dyn CgroupDir>>,"), CGROUP_DIR],
        why="cleanup ownership of the candidate cannot be released from outside",
    ),
    probe(
        "P-I4-PENDING-UNIT", "visibility",
        "rebind a pending operation to another unit name",
        '''    let mut pending = pending;
    pending.unit = String::from("nexus-verifier-1.scope");''',
        ["error[E0616]: field `unit` of struct `PendingScope` is private"],
        reopen=[(PENDING, "    unit: String,\n    /// The retained helper",
                 "    pub unit: String,\n    /// The retained helper")],
        why="the unit name is the request's locator, fixed when the operation is prepared",
    ),
    probe(
        "P-I4-PENDING-RECONCILE", "visibility",
        "settle a pending operation with fault injection, or without its helper",
        '''    let mut pending = pending;
    let _ = pending.reconcile(None, None);''',
        ["error[E0624]: method `reconcile` is private"],
        reopen=[(PENDING, "    pub(crate) fn reconcile(", "    pub fn reconcile(")],
        why="outside the crate only settle(&Helper) exists",
    ),
    probe(
        "P-I4-SCOPE-FORGE", "visibility",
        "build a proven scope from a unit name and a directory",
        '''    let _ = Scope { unit: String::from("nexus-verifier-0.scope"), dir: todo!() };''',
        ["error[E0451]: fields `unit` and `dir` of struct `nexus_verifier_sandbox::scope::Scope` are private"],
        reopen=[(SCOPE, "pub struct Scope {\n    unit: String,\n    dir: Box<dyn CgroupDir>,\n}",
                 "pub struct Scope {\n    pub unit: String,\n    pub dir: Box<dyn CgroupDir>,\n}"),
                CGROUP_DIR],
        why="the only way to a Scope is a pending operation whose every proof passed",
    ),
    probe(
        "P-I4-BOUNDARY-TYPE", "visibility",
        "name the scope boundary (None/Pending/Proven) to promote or reset it",
        '''    fn promote(boundary: &mut nexus_verifier_sandbox::scope::ScopeBoundary) {}''',
        ["error[E0603]: enum `ScopeBoundary` is private"],
        reopen=BOUNDARY_TYPE,
        why="the transitions are internal: establish (Pending -> Proven) and settling",
    ),
    probe(
        "P-I4-RETAINED-SCOPE", "visibility",
        "take or replace the retained boundary's scope operation",
        '''    let boundary = boundary;
    let _ = boundary.scope;''',
        ["error[E0616]: field `scope` of struct `RetainedBoundary` is private"],
        reopen=[(EXEC, "pub struct RetainedBoundary {\n    scope: ScopeBoundary,",
                 "pub struct RetainedBoundary {\n    pub scope: ScopeBoundary,")] + BOUNDARY_TYPE,
        why="a retained boundary is ended only by retry, or (defense in depth) its drop",
    ),
    probe(
        "P-I4-MANAGER-MODULE", "visibility",
        "provide a caller's manager transport (destination, interface, replies)",
        '''    fn transport(_: &dyn nexus_verifier_sandbox::scope::manager::Manager) {}''',
        ["error[E0603]: module `manager` is private"],
        reopen=[(SCOPE, "mod manager;", "pub mod manager;"),
                (MANAGER, "pub(crate) trait Manager: Send + Sync {", "pub trait Manager: Send + Sync {")],
        why="the only manager in a normal build is the crate's own, to the fixed destination",
    ),
    probe(
        "P-I4-NATIVE-MODULE", "visibility",
        "provide a caller's kernel view (membership, cgroup directories)",
        '''    fn view(_: &dyn nexus_verifier_sandbox::scope::native::Native) {}''',
        ["error[E0603]: module `native` is private"],
        reopen=[(SCOPE, "mod native;", "pub mod native;"),
                (NATIVE, "pub(crate) trait Native: Send + Sync {", "pub trait Native: Send + Sync {"),
                CGROUP_DIR],
        why="membership and cgroups are observed only through /proc and the cgroup v2 hierarchy",
    ),
    probe(
        "P-I4-WITH-CONTROLLER", "visibility",
        "build a scope manager over a caller's controller in a normal build",
        '''    let _ = ScopeManager::with_controller(todo!());''',
        ["error[E0599]: no function or associated item named `with_controller` found for struct `ScopeManager` in the current scope"],
        reopen=[(SCOPE, "    #[cfg(test)]\n    pub(crate) fn with_controller(", "    pub fn with_controller("),
                CONTROLLER],
        why="the deterministic simulation exists only in the crate's own unit tests",
    ),
    probe(
        "P-I4-PREPARE", "visibility",
        "prepare an operation outside the execution's owner",
        '''    let _ = scopes.prepare(helper, &ResourcePolicy::RUST_OFFLINE_V1);''',
        ["error[E0624]: method `prepare` is private"],
        reopen=[(SCOPE, "    pub(crate) fn prepare(", "    pub fn prepare(")],
        why="the execution stores the operation before establishing it; start() owns its own",
    ),
    probe(
        "P-I4-HELPER-SERIAL", "visibility",
        "read the helper's binding identity",
        '''    let _ = helper.serial();''',
        ["error[E0624]: method `serial` is private"],
        reopen=[(LAUNCHER, "    pub(crate) fn serial(&self) -> u64 {", "    pub fn serial(&self) -> u64 {")],
        why="a binding, never an authority a caller presents",
    ),
    probe(
        "P-I4-PENDING-CLONE", "trait-property",
        "copy a pending operation (two owners of one possible effect)",
        '''    fn copied<T: Clone>() {}
    copied::<PendingScope>();''',
        ["error[E0277]: the trait bound `PendingScope: Clone` is not satisfied"],
        self_check='''    fn copied<T: Clone>() {}
    copied::<ScopeEvents>();''',
        why="one owner per possible effect",
    ),
    probe(
        "P-I4-RETAINED-CLONE", "trait-property",
        "copy a retained boundary",
        '''    fn copied<T: Clone>() {}
    copied::<RetainedBoundary>();''',
        ["error[E0277]: the trait bound `RetainedBoundary: Clone` is not satisfied"],
        self_check='''    fn copied<T: Clone>() {}
    copied::<ScopeEvents>();''',
        why="one owner per unconfirmed boundary",
    ),
    probe(
        "P-I4-SCOPE-CLONE", "trait-property",
        "copy a proven scope",
        '''    fn copied<T: Clone>() {}
    copied::<Scope>();''',
        ["error[E0277]: the trait bound `nexus_verifier_sandbox::scope::Scope: Clone` is not satisfied"],
        self_check='''    fn copied<T: Clone>() {}
    copied::<ScopeEvents>();''',
        why="one retained descriptor per scope",
    ),
    probe(
        "P-I4-PENDING-SERIALIZE", "trait-property",
        "serialize a pending operation (to reconstruct it after a restart)",
        '''    fn saved<T: serde::Serialize>() {}
    saved::<PendingScope>();''',
        ["error[E0277]: the trait bound `PendingScope: serde::Serialize` is not satisfied"],
        self_check='''    fn saved<T: serde::Serialize>() {}
    saved::<String>();''',
        why="no serialized PendingScope; nothing is reconstructed across a restart",
    ),
    probe(
        "P-I4-PENDING-DESERIALIZE", "trait-property",
        "deserialize a pending operation from a saved record",
        '''    fn restored<T: serde::de::DeserializeOwned>() {}
    restored::<PendingScope>();''',
        ["error[E0277]: the trait bound `PendingScope: serde::de::DeserializeOwned` is not satisfied"],
        self_check='''    fn restored<T: serde::de::DeserializeOwned>() {}
    restored::<String>();''',
        why="no PendingScope from a saved unit string or record",
    ),
    probe(
        "P-I4-RETAINED-DESERIALIZE", "trait-property",
        "deserialize a retained boundary from a saved record",
        '''    fn restored<T: serde::de::DeserializeOwned>() {}
    restored::<RetainedBoundary>();''',
        ["error[E0277]: the trait bound `RetainedBoundary: serde::de::DeserializeOwned` is not satisfied"],
        self_check='''    fn restored<T: serde::de::DeserializeOwned>() {}
    restored::<String>();''',
        why="a retained boundary exists only in the backend process that created it",
    ),
]

HARNESS = probe(
    "P-I4-HARNESS-CHECK", "harness",
    "a deliberate type error",
    '''    let _: u32 = "not a number";''',
    ["error[E0308]: mismatched types"],
    why="proves that the harness sees build errors",
)

POSITIVE = '''    // The public API's ordinary use: connect, run, retry; start, settle.
    if let Ok(scopes) = ScopeManager::connect() {
        let program = HelperProgram::installed().ok();
        if let Some(program) = program {
            let report = execution::run(&scopes, &program, todo!(),
                &ResourcePolicy::RUST_OFFLINE_V1);
            if let Cleanup::Failed(boundary) = report.cleanup {
                let _ = boundary.retry();
            }
        }
        match scopes.start(helper, &ResourcePolicy::RUST_OFFLINE_V1) {
            Ok(scope) => {
                let _ = (scope.unit(), scope.occupancy(), scope.events());
            }
            Err(StartFailed { error, unresolved }) => {
                if let Some(mut pending) = unresolved {
                    let _ = pending.unit();
                    let _ = pending.settle(helper);
                }
                let _ = error;
            }
        }
    }'''

ROOT = None
TARGET_DIR = None


def sha256(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def state():
    return {p: sha256(ROOT / p) for p in SOURCES}


def status():
    out = subprocess.run(["git", "status", "--porcelain=v1", "--untracked-files=all"],
                         cwd=ROOT, capture_output=True, text=True, check=True).stdout
    return {line for line in out.splitlines() if line}


def build(body):
    (ROOT / PROBE).write_text(HEADER + body + FOOTER)
    env = dict(os.environ, CARGO_TARGET_DIR=str(TARGET_DIR))
    proc = subprocess.run(
        ["cargo", "test", "--locked", "-p", "nexus-verifier-sandbox", "--test", "i4_api_probe",
         "--no-run"],
        cwd=ROOT, capture_output=True, text=True, timeout=1800, env=env)
    output = proc.stdout + proc.stderr
    errors = sorted({line.strip() for line in output.splitlines()
                     if line.startswith("error[")})
    return proc.returncode, errors, output


def apply(edits, originals):
    texts = {}
    for edit in edits:
        text = texts.get(edit["file"], originals[edit["file"]].decode())
        count = text.count(edit["anchor"])
        if count != 1:
            raise SystemExit(f"reopen anchor occurs {count} times in {edit['file']}: "
                             f"{edit['anchor'][:80]!r}")
        texts[edit["file"]] = text.replace(edit["anchor"], edit["replacement"], 1)
    return texts


def main():
    global ROOT, TARGET_DIR
    args = sys.argv[1:]
    if "--target-dir" not in args:
        raise SystemExit(__doc__)
    at = args.index("--target-dir")
    TARGET_DIR = pathlib.Path(args[at + 1]).resolve()
    del args[at:at + 2]
    if len(args) != 2:
        raise SystemExit(__doc__)
    ROOT = pathlib.Path(args[0]).resolve()
    log = pathlib.Path(args[1]).resolve()
    log.mkdir(parents=True, exist_ok=True)
    if (ROOT / PROBE).exists():
        raise SystemExit("a probe file already exists")
    before_status = status()
    original_state = state()
    originals = {p: (ROOT / p).read_bytes() for p in SOURCES}
    results = []
    try:
        code, errors, output = build(POSITIVE)
        (log / "P-I4-POSITIVE-CONTROL.log").write_text(output)
        positive = dict(id="P-I4-POSITIVE-CONTROL", kind="positive", exit=code,
                        errors=errors, ok=code == 0 and not errors)
        print(json.dumps(positive), flush=True)
        for p in PROBES + [HARNESS]:
            code, errors, output = build(p["body"])
            (log / f"{p['id']}.log").write_text(output)
            exact = code != 0 and errors == sorted(p["expect"])
            check = None
            if p["kind"] == "visibility":
                texts = apply(p["reopen"], originals)
                try:
                    for path, text in texts.items():
                        (ROOT / path).write_text(text)
                    c_code, c_errors, c_output = build(p["body"])
                finally:
                    for path in texts:
                        (ROOT / path).write_bytes(originals[path])
                (log / f"{p['id']}.reopened.log").write_text(c_output)
                check = c_code == 0 and not c_errors
            elif p["kind"] == "trait-property":
                c_code, c_errors, c_output = build(p["self_check"])
                (log / f"{p['id']}.self-check.log").write_text(c_output)
                check = c_code == 0 and not c_errors
            else:
                c_errors = []
                check = True
            restored = state() == original_state
            ok = exact and check and restored
            result = dict(id=p["id"], kind=p["kind"], attempt=p["attempt"], why=p["why"],
                          expected=sorted(p["expect"]), errors=errors, exact=exact,
                          reopen_files=sorted({e["file"] for e in p["reopen"]}),
                          self_check_builds=check, self_check_errors=c_errors,
                          sources_restored=restored, ok=ok)
            results.append(result)
            print(json.dumps(result), flush=True)
    finally:
        (ROOT / PROBE).unlink(missing_ok=True)
        for path in SOURCES:
            (ROOT / path).write_bytes(originals[path])
    final = state()
    summary = dict(
        identical=final == original_state,
        status_restored=status() == before_status,
        positive_control=positive["ok"],
        guards=len(PROBES),
        guards_by_kind={k: sum(1 for r in results if r["kind"] == k)
                        for k in ("visibility", "trait-property")},
        harness_check=all(r["ok"] for r in results if r["kind"] == "harness"),
        all_required=all(r["ok"] for r in results) and positive["ok"],
        failures=[r["id"] for r in results if not r["ok"]] + ([] if positive["ok"] else ["P-I4-POSITIVE-CONTROL"]),
    )
    print(json.dumps(summary), flush=True)
    (log / "summary.json").write_text(json.dumps(
        dict(summary=summary, positive=positive, guards=results), indent=2) + "\n")
    return 0 if summary["identical"] and summary["status_restored"] and summary["all_required"] else 1


if __name__ == "__main__":
    sys.exit(main())

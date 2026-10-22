#!/usr/bin/env python3
"""P2-V1-R3B-I4-R1 compile-time API/type guards in the live harness's build:
the 19 accepted P2-V1-R3B-I4 guards, adapted to the closed surface, and the
I4-R1 guards of the harness's direct owner.

Each probe is a small integration-test target written into a scratch
checkout as crates/nexus-verifier-sandbox/tests/i4r1_api_probe.rs. Like the
live harness it is built with the crate's `live-sandbox-harness` feature
(the crate's own dev-dependency enables it), so it sees the widest public
surface any build has. (The normal build's surface is checked separately,
from an external crate: normal_api_probes.py.) It reaches the crate only
through `nexus_verifier_sandbox::...` paths and attempts one authority path
the ownership design closes. Each counted probe must:

- fail to build, with exactly its expected error lines (the distinct
  `error[...]` header lines of the build, "could not compile" excluded):
  the failure is the intended privacy or type guard and nothing else;
- self-check. A visibility guard: once the guard is reopened in the scratch
  checkout's sources (each reopen edit's anchor occurs exactly once; a
  removed item is reopened by adding it back), the same probe builds with
  no error; the sources are then restored byte for byte and verified by
  SHA-256. A trait-property guard (no Clone, no Serialize or Deserialize):
  the same probe asking the same bound of a type that has it builds.

The I4 guards keep their ids and intentions. A pending scope operation is
now crate-private, so each guard that reached one now fails first at the
type's privacy (and is reopened by making it public again); the rest are
unchanged. The mapping is recorded with every result.

Two more targets are not guards and are labelled so: the harness check (a
deliberate E0308, proving the harness sees errors) and a positive control
(the public API's ordinary use, and the harness owner's, builds with no
error).

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
PROBE = f"{CRATE}/tests/i4r1_api_probe.rs"
SCOPE = f"{CRATE}/src/scope.rs"
PENDING = f"{CRATE}/src/scope/pending.rs"
MANAGER = f"{CRATE}/src/scope/manager.rs"
NATIVE = f"{CRATE}/src/scope/native.rs"
EXEC = f"{CRATE}/src/execution.rs"
LAUNCHER = f"{CRATE}/src/launcher.rs"
SOURCES = [SCOPE, PENDING, MANAGER, NATIVE, EXEC, LAUNCHER]

HEADER = r'''//! P2-V1-R3B-I4-R1 API probe: an external caller of the sandbox crate, in
//! the live harness's build.
#![cfg(all(target_os = "linux", target_arch = "x86_64"))]
#![allow(unused, dead_code, unreachable_code, private_interfaces)]

use nexus_verifier_sandbox::execution::{
    self, Cleanup, PlacementFailed, RetainedBoundary, ScopedHelper,
};
use nexus_verifier_sandbox::launcher::{Helper, HelperProgram};
use nexus_verifier_sandbox::policy::ResourcePolicy;
use nexus_verifier_sandbox::scope::{Scope, ScopeEvents, ScopeManager};

fn probe(
    scopes: &ScopeManager,
    helper: &Helper,
    boundary: RetainedBoundary,
    placed: ScopedHelper,
    failed: PlacementFailed,
    scope: Scope,
) {
'''
FOOTER = r'''
}

#[test]
fn the_probe_builds() {}
'''

PS = "nexus_verifier_sandbox::scope::PendingScope"

# Reopen edits shared by several guards.
CGROUP_DIR = (NATIVE, "pub(crate) trait CgroupDir: Send + Sync + Debug {",
              "pub trait CgroupDir: Send + Sync + Debug {")
CONTROLLER = (SCOPE, "pub(crate) struct Controller {", "pub struct Controller {")
PENDING_TYPE = [
    (SCOPE, "pub(crate) use pending::{PendingScope, ScopeBoundary};",
     "pub use pending::PendingScope;\npub(crate) use pending::ScopeBoundary;"),
    (PENDING, "pub(crate) struct PendingScope {", "pub struct PendingScope {"),
]
BOUNDARY_TYPE = [
    (SCOPE, "pub(crate) use pending::{PendingScope, ScopeBoundary};",
     "pub(crate) use pending::PendingScope;\npub use pending::ScopeBoundary;"),
    (PENDING, "pub(crate) enum ScopeBoundary {", "pub enum ScopeBoundary {"),
]
PUBLIC_START = (SCOPE, '''        Ok(Box::new(PendingScope::new(
            Arc::clone(&self.controller),
            unit,
            helper,
            limits,
        )))
    }
}''', '''        Ok(Box::new(PendingScope::new(
            Arc::clone(&self.controller),
            unit,
            helper,
            limits,
        )))
    }

    /// Reopened: a public direct scope start.
    pub fn start(&self, helper: &Helper, limits: &ResourcePolicy) -> Result<Scope, ScopeError> {
        let mut boundary = ScopeBoundary::Pending(self.prepare(helper, limits)?);
        boundary.establish(helper, None)?;
        match boundary {
            ScopeBoundary::Proven(scope) => Ok(scope),
            _ => Err(ScopeError::Mismatch("scope not proven")),
        }
    }
}''')
START_FAILED = (SCOPE, "pub use manager::BUS_CALL_TIMEOUT;\n", '''pub use manager::BUS_CALL_TIMEOUT;

/// Reopened: the I4 failure that carried the operation apart from its
/// helper.
#[derive(Debug)]
pub struct StartFailed {
    pub error: ScopeError,
    pub unresolved: Option<Box<PendingScope>>,
}
''')
SCOPED_FIELDS = (EXEC, '''pub struct ScopedHelper {
    helper: Option<Helper>,
    scope: ScopeBoundary,
}''', '''pub struct ScopedHelper {
    pub helper: Option<Helper>,
    pub scope: ScopeBoundary,
}''')


def probe(pid, kind, attempt, body, expect, reopen=(), self_check=None, why="", mapping=None):
    return dict(id=pid, kind=kind, attempt=attempt, body=body, expect=list(expect),
                reopen=[dict(file=f, anchor=a, replacement=r) for (f, a, r) in reopen],
                self_check=self_check, why=why, mapping=mapping)


PRIVATE_PENDING = "error[E0603]: struct `PendingScope` is private"
I4_PENDING = ("adapted: a pending operation is crate-private (A-R1-2), so the probe names it by "
              "its path and fails first at the type's privacy; the reopen also makes the type "
              "public again")
UNCHANGED = "unchanged: the I4 probe, expectation and reopen"

I4_GUARDS = [
    probe(
        "P-I4-PENDING-NEW", "visibility",
        "construct a pending scope operation outside the crate (from a unit name and a process id)",
        f'''    let _ = {PS}::new(todo!(), String::from("nexus-verifier-0.scope"), helper,
        &ResourcePolicy::RUST_OFFLINE_V1);''',
        [PRIVATE_PENDING, "error[E0624]: associated function `new` is private"],
        reopen=PENDING_TYPE + [(PENDING, "    pub(super) fn new(", "    pub fn new("), CONTROLLER],
        why="only ScopeManager::prepare creates one, bound to the retained helper and the issuing connection",
        mapping=I4_PENDING,
    ),
    probe(
        "P-I4-PENDING-STATE", "visibility",
        "mark a pending operation settled, or never issued, from outside",
        f'''    fn edit(pending: &mut {PS}) {{
        pending.settled = true;
        pending.issued = false;
    }}''',
        [PRIVATE_PENDING,
         "error[E0616]: field `issued` of struct `nexus_verifier_sandbox::scope::pending::PendingScope` is private",
         "error[E0616]: field `settled` of struct `nexus_verifier_sandbox::scope::pending::PendingScope` is private"],
        reopen=PENDING_TYPE + [(PENDING, "    issued: bool,", "    pub issued: bool,"),
                               (PENDING, "    settled: bool,", "    pub settled: bool,")],
        why="only settling (reconcile) confirms an operation gone",
        mapping=I4_PENDING,
    ),
    probe(
        "P-I4-PENDING-CANDIDATE", "visibility",
        "drop or replace a pending operation's retained candidate",
        f'''    fn edit(pending: &mut {PS}) {{
        pending.candidate = None;
    }}''',
        [PRIVATE_PENDING, "error[E0616]: field `candidate` of struct `nexus_verifier_sandbox::scope::pending::PendingScope` is private"],
        reopen=PENDING_TYPE + [(PENDING, "    candidate: Option<Box<dyn CgroupDir>>,",
                                "    pub candidate: Option<Box<dyn CgroupDir>>,"), CGROUP_DIR],
        why="cleanup ownership of the candidate cannot be released from outside",
        mapping=I4_PENDING,
    ),
    probe(
        "P-I4-PENDING-UNIT", "visibility",
        "rebind a pending operation to another unit name",
        f'''    fn edit(pending: &mut {PS}) {{
        pending.unit = String::from("nexus-verifier-1.scope");
    }}''',
        [PRIVATE_PENDING, "error[E0616]: field `unit` of struct `nexus_verifier_sandbox::scope::pending::PendingScope` is private"],
        reopen=PENDING_TYPE + [(PENDING, "    unit: String,\n    /// The retained helper",
                                "    pub unit: String,\n    /// The retained helper")],
        why="the unit name is the request's locator, fixed when the operation is prepared",
        mapping=I4_PENDING,
    ),
    probe(
        "P-I4-PENDING-RECONCILE", "visibility",
        "settle a pending operation with fault injection, or without its helper",
        f'''    fn settle(pending: &mut {PS}) {{
        let _ = pending.reconcile(None, None);
    }}''',
        [PRIVATE_PENDING, "error[E0624]: method `reconcile` is private"],
        reopen=PENDING_TYPE + [(PENDING, "    pub(crate) fn reconcile(", "    pub fn reconcile(")],
        why="only the execution's finalizer settles, after killing the helper it keeps unreaped",
        mapping=I4_PENDING + "; the I4 public settle(&Helper) no longer exists (A-R1-3)",
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
        mapping=UNCHANGED,
    ),
    probe(
        "P-I4-BOUNDARY-TYPE", "visibility",
        "name the scope boundary (None/Pending/Proven) to promote or reset it",
        '''    fn promote(boundary: &mut nexus_verifier_sandbox::scope::ScopeBoundary) {}''',
        ["error[E0603]: enum `ScopeBoundary` is private"],
        reopen=BOUNDARY_TYPE,
        why="the transitions are internal: establish (Pending -> Proven) and settling",
        mapping="adapted: the same probe and expectation; the reopen's re-export anchor is the "
                "one line that now re-exports both crate-private types",
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
        mapping="adapted: the same probe and expectation; the reopen's re-export anchor as for "
                "P-I4-BOUNDARY-TYPE",
    ),
    probe(
        "P-I4-MANAGER-MODULE", "visibility",
        "provide a caller's manager transport (destination, interface, replies)",
        '''    fn transport(_: &dyn nexus_verifier_sandbox::scope::manager::Manager) {}''',
        ["error[E0603]: module `manager` is private"],
        reopen=[(SCOPE, "mod manager;", "pub mod manager;"),
                (MANAGER, "pub(crate) trait Manager: Send + Sync {", "pub trait Manager: Send + Sync {")],
        why="the only manager in a normal build is the crate's own, to the fixed destination",
        mapping=UNCHANGED,
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
        mapping=UNCHANGED,
    ),
    probe(
        "P-I4-WITH-CONTROLLER", "visibility",
        "build a scope manager over a caller's controller",
        '''    let _ = ScopeManager::with_controller(todo!());''',
        ["error[E0599]: no function or associated item named `with_controller` found for struct `ScopeManager` in the current scope"],
        reopen=[(SCOPE, "    #[cfg(test)]\n    pub(crate) fn with_controller(", "    pub fn with_controller("),
                CONTROLLER],
        why="the deterministic simulation exists only in the crate's own unit tests",
        mapping=UNCHANGED,
    ),
    probe(
        "P-I4-PREPARE", "visibility",
        "prepare an operation outside an owner of its helper",
        '''    let _ = scopes.prepare(helper, &ResourcePolicy::RUST_OFFLINE_V1);''',
        ["error[E0624]: method `prepare` is private"],
        reopen=[(SCOPE, "    pub(crate) fn prepare(", "    pub fn prepare(")] + PENDING_TYPE,
        why="only an owner of the helper (the execution, the harness's direct owner) prepares one",
        mapping="adapted: the same probe and expectation; the reopen also makes the pending "
                "operation's type public again, since prepare returns one (A-R1-2)",
    ),
    probe(
        "P-I4-HELPER-SERIAL", "visibility",
        "read the helper's binding identity",
        '''    let _ = helper.serial();''',
        ["error[E0624]: method `serial` is private"],
        reopen=[(LAUNCHER, "    pub(crate) fn serial(&self) -> u64 {", "    pub fn serial(&self) -> u64 {")],
        why="a binding, never an authority a caller presents",
        mapping=UNCHANGED,
    ),
    probe(
        "P-I4-PENDING-CLONE", "trait-property",
        "copy a pending operation (two owners of one possible effect)",
        f'''    fn copied<T: Clone>() {{}}
    copied::<{PS}>();''',
        [PRIVATE_PENDING,
         "error[E0277]: the trait bound `nexus_verifier_sandbox::scope::pending::PendingScope: Clone` is not satisfied"],
        self_check='''    fn copied<T: Clone>() {}
    copied::<ScopeEvents>();''',
        why="one owner per possible effect",
        mapping=I4_PENDING.replace("; the reopen also makes the type public again",
                                   "; the self-check is unchanged"),
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
        mapping=UNCHANGED,
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
        mapping=UNCHANGED,
    ),
    probe(
        "P-I4-PENDING-SERIALIZE", "trait-property",
        "serialize a pending operation (to reconstruct it after a restart)",
        f'''    fn saved<T: serde::Serialize>() {{}}
    saved::<{PS}>();''',
        [PRIVATE_PENDING,
         "error[E0277]: the trait bound `nexus_verifier_sandbox::scope::pending::PendingScope: serde::Serialize` is not satisfied"],
        self_check='''    fn saved<T: serde::Serialize>() {}
    saved::<String>();''',
        why="no serialized PendingScope; nothing is reconstructed across a restart",
        mapping=I4_PENDING.replace("; the reopen also makes the type public again",
                                   "; the self-check is unchanged"),
    ),
    probe(
        "P-I4-PENDING-DESERIALIZE", "trait-property",
        "deserialize a pending operation from a saved record",
        f'''    fn restored<T: serde::de::DeserializeOwned>() {{}}
    restored::<{PS}>();''',
        [PRIVATE_PENDING,
         "error[E0277]: the trait bound `nexus_verifier_sandbox::scope::pending::PendingScope: serde::de::DeserializeOwned` is not satisfied"],
        self_check='''    fn restored<T: serde::de::DeserializeOwned>() {}
    restored::<String>();''',
        why="no PendingScope from a saved unit string or record",
        mapping=I4_PENDING.replace("; the reopen also makes the type public again",
                                   "; the self-check is unchanged"),
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
        mapping=UNCHANGED,
    ),
]

I4R1_GUARDS = [
    probe(
        "P-I4R1-START-REMOVED", "visibility",
        "start a scope directly (the I4 public start, A-R1-1), even in the harness's build",
        '''    let _ = scopes.start(helper, &ResourcePolicy::RUST_OFFLINE_V1);''',
        ["error[E0599]: no method named `start` found for reference `&ScopeManager` in the current scope"],
        reopen=[PUBLIC_START],
        why="the harness's direct start is execution::place, which owns the helper with the operation",
    ),
    probe(
        "P-I4R1-START-FAILED-REMOVED", "visibility",
        "receive a failure that carries the operation apart from its helper (A-R1-2)",
        '''    fn failed(_: nexus_verifier_sandbox::scope::StartFailed) {}''',
        ["error[E0425]: cannot find type `StartFailed` in module `nexus_verifier_sandbox::scope`"],
        reopen=[START_FAILED],
        why="a failed placement's cleanup holds the operation and its helper together",
    ),
    probe(
        "P-I4R1-SCOPED-FORGE", "visibility",
        "build the harness's direct owner from parts (a helper and a boundary of a caller's choosing)",
        '''    let _ = ScopedHelper { helper: None, scope: todo!() };''',
        ["error[E0451]: fields `helper` and `scope` of struct `ScopedHelper` are private"],
        reopen=[SCOPED_FIELDS] + BOUNDARY_TYPE,
        why="only execution::place builds one, after every proof passed",
    ),
    probe(
        "P-I4R1-SCOPED-TAKE-HELPER", "visibility",
        "take the helper out of the harness's direct owner (to reap it apart from its scope)",
        '''    let mut placed = placed;
    let _ = placed.helper.take();''',
        ["error[E0616]: field `helper` of struct `ScopedHelper` is private"],
        reopen=[SCOPED_FIELDS] + BOUNDARY_TYPE,
        why="the helper is reaped only by reap_helper (a proven scope) or by end",
    ),
    probe(
        "P-I4R1-SCOPED-TAKE-SCOPE", "visibility",
        "take or replace the harness's direct owner's scope",
        '''    let mut placed = placed;
    let _ = std::mem::replace(&mut placed.scope, todo!());''',
        ["error[E0616]: field `scope` of struct `ScopedHelper` is private"],
        reopen=[SCOPED_FIELDS] + BOUNDARY_TYPE,
        why="the scope is ended only by end, or (defense in depth) the owner's drop",
    ),
    probe(
        "P-I4R1-SCOPED-CLONE", "trait-property",
        "copy the harness's direct owner",
        '''    fn copied<T: Clone>() {}
    copied::<ScopedHelper>();''',
        ["error[E0277]: the trait bound `ScopedHelper: Clone` is not satisfied"],
        self_check='''    fn copied<T: Clone>() {}
    copied::<ScopeEvents>();''',
        why="one owner of a helper and its scope",
    ),
    probe(
        "P-I4R1-PLACEMENT-CLONE", "trait-property",
        "copy a failed placement (and with it its retained boundary)",
        '''    fn copied<T: Clone>() {}
    copied::<PlacementFailed>();''',
        ["error[E0277]: the trait bound `PlacementFailed: Clone` is not satisfied"],
        self_check='''    fn copied<T: Clone>() {}
    copied::<ScopeEvents>();''',
        why="one owner per unconfirmed boundary",
    ),
    probe(
        "P-I4R1-SCOPED-DESERIALIZE", "trait-property",
        "restore the harness's direct owner from a saved record",
        '''    fn restored<T: serde::de::DeserializeOwned>() {}
    restored::<ScopedHelper>();''',
        ["error[E0277]: the trait bound `ScopedHelper: serde::de::DeserializeOwned` is not satisfied"],
        self_check='''    fn restored<T: serde::de::DeserializeOwned>() {}
    restored::<String>();''',
        why="nothing is reconstructed across a restart",
    ),
]

PROBES = I4_GUARDS + I4R1_GUARDS

HARNESS = probe(
    "P-I4R1-HARNESS-CHECK", "harness",
    "a deliberate type error",
    '''    let _: u32 = "not a number";''',
    ["error[E0308]: mismatched types"],
    why="proves that the harness sees build errors",
)

POSITIVE = '''    // The public API's ordinary use: connect, run, retry. The harness's
    // direct owner: place; observe, reap and end it; or settle the failure.
    if let Ok(scopes) = ScopeManager::connect() {
        if let Ok(program) = HelperProgram::installed() {
            let report = execution::run(&scopes, &program, todo!(),
                &ResourcePolicy::RUST_OFFLINE_V1);
            if let Cleanup::Failed(boundary) = report.cleanup {
                let _ = boundary.retry();
            }
        }
        let owned: Helper = todo!();
        match execution::place(&scopes, owned, &ResourcePolicy::RUST_OFFLINE_V1) {
            Ok(mut placed) => {
                if let Some(scope) = placed.scope() {
                    let _ = (scope.unit(), scope.occupancy(), scope.events(), scope.kill());
                }
                let _ = placed.helper().map(Helper::pid);
                let _ = placed.reap_helper();
                let _ = placed.end().is_confirmed();
            }
            Err(PlacementFailed { error, cleanup }) => {
                let _ = error;
                if let Cleanup::Failed(boundary) = cleanup {
                    let _ = (boundary.holds_scope(), boundary.holds_helper());
                    let _ = boundary.retry();
                }
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
        ["cargo", "test", "--locked", "-p", "nexus-verifier-sandbox", "--test", "i4r1_api_probe",
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
        (log / "P-I4R1-POSITIVE-CONTROL.log").write_text(output)
        positive = dict(id="P-I4R1-POSITIVE-CONTROL", kind="positive", exit=code,
                        errors=errors, ok=code == 0 and not errors)
        print(json.dumps(positive), flush=True)
        for p in PROBES + [HARNESS]:
            code, errors, output = build(p["body"])
            (log / f"{p['id']}.log").write_text(output)
            exact = code != 0 and errors == sorted(p["expect"])
            check = None
            c_errors = []
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
                check = True
            restored = state() == original_state
            ok = exact and check and restored
            result = dict(id=p["id"], kind=p["kind"], attempt=p["attempt"], why=p["why"],
                          family="i4" if p["id"].startswith("P-I4-") else "i4r1",
                          mapping=p["mapping"],
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
    guards = [r for r in results if r["kind"] != "harness"]
    summary = dict(
        identical=final == original_state,
        status_restored=status() == before_status,
        positive_control=positive["ok"],
        guards=len(guards),
        guards_by_family={f: sum(1 for r in guards if r["family"] == f) for f in ("i4", "i4r1")},
        guards_by_kind={k: sum(1 for r in guards if r["kind"] == k)
                        for k in ("visibility", "trait-property")},
        i4_adapted=sorted(r["id"] for r in guards
                          if r["family"] == "i4" and r["mapping"] != UNCHANGED),
        harness_check=all(r["ok"] for r in results if r["kind"] == "harness"),
        all_required=all(r["ok"] for r in results) and positive["ok"],
        failures=[r["id"] for r in results if not r["ok"]]
        + ([] if positive["ok"] else ["P-I4R1-POSITIVE-CONTROL"]),
    )
    print(json.dumps(summary), flush=True)
    (log / "summary.json").write_text(json.dumps(
        dict(summary=summary, positive=positive, guards=results), indent=2) + "\n")
    return 0 if summary["identical"] and summary["status_restored"] and summary["all_required"] else 1


if __name__ == "__main__":
    sys.exit(main())

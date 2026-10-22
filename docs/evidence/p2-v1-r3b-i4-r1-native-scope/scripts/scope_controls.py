#!/usr/bin/env python3
"""P2-V1-R3B-I4-R1 behavioural negative controls: native scope ownership.

Two families, counted separately:

- the 21 accepted P2-V1-R3B-I4 controls (16 required, 5 additional),
  preserved. Their definitions are imported, unchanged, from the I4
  evidence (docs/evidence/p2-v1-r3b-i4-native-scope/scripts/
  scope_controls.py, verified by SHA-256 before use); the two whose defect
  point the repaired shape moved are redefined here, with the same
  mutation, test and marker, and the reason. Every other one runs exactly
  as I4 defined it (same anchors, mutation, test and marker).
- the I4-R1 controls: the 14 the mission names (required) and 3 further
  mechanisms (additional).

Each control restores one wrong behaviour in the production sources and
must:

- apply: every edit's anchor occurs exactly once in its file, in order;
- compile: the library's unit-test target builds (no "error[E", no
  "could not compile");
- run its one intended test alone (--exact) and fail it ("0 passed; 1
  failed", "<test> ... FAILED");
- fail the intended assertion: the message of the last panic on the test's
  own thread carries the control's marker;
- restore: the mutated files are written back byte for byte in `finally`;
  every file of FILES is verified by SHA-256 before each control and after
  each restoration, and the checkout's status must equal the expected
  status.

The tests and the simulation they run over (src/scope/tests.rs,
src/execution/tests.rs) are never mutated; they are hashed to prove it.
Every test runs over the deterministic simulation of the user manager and
the kernel: no control contacts a bus or creates a cgroup.

API/type guards are separate scripts (api_guards.py, normal_api_probes.py)
and are never added into one figure with these.

Usage:
  scope_controls.py <checkout> <log directory> --target-dir <empty dir> [--only ID,...]
  scope_controls.py <checkout> --check-anchors

<checkout> must be an isolated, clean Git checkout of the candidate (never
the worktree under review); the target directory must be dedicated to
these runs. Run it alone: it mutates and restores files in the checkout.
"""
import hashlib
import importlib.util
import json
import os
import pathlib
import subprocess
import sys

# Importing the I4 runner must leave no bytecode beside it (its evidence
# directory is immutable).
sys.dont_write_bytecode = True

CRATE = "crates/nexus-verifier-sandbox"
SCOPE = f"{CRATE}/src/scope.rs"
PENDING = f"{CRATE}/src/scope/pending.rs"
EXEC = f"{CRATE}/src/execution.rs"
LAUNCHER = f"{CRATE}/src/launcher.rs"
FILES = [
    SCOPE,
    PENDING,
    EXEC,
    LAUNCHER,
    f"{CRATE}/src/scope/manager.rs",
    f"{CRATE}/src/scope/native.rs",
    f"{CRATE}/src/scope/tests.rs",
    f"{CRATE}/src/execution/tests.rs",
    f"{CRATE}/src/fault.rs",
]
MUTABLE = {SCOPE, PENDING, EXEC, LAUNCHER}

# The accepted I4 runner, as committed in bb0dfec0 (its SHA-256 is recorded
# with every result).
I4_SCRIPT = "docs/evidence/p2-v1-r3b-i4-native-scope/scripts/scope_controls.py"

ROOT = None
LOG = None
TARGET_DIR = None
EXPECTED_STATUS = set()


def control(cid, category, what, edits, test, marker, mapping=None):
    return dict(
        id=cid, category=category, what=what,
        edits=[dict(file=f, anchor=a, replacement=r) for (f, a, r) in edits],
        test=test, marker=marker, mapping=mapping,
    )


# ---------------------------------------------------------------------------
# The I4 controls whose defect point the repaired shape moved.

ADAPTED_I4 = {
    "NC-I4-START-TIMEOUT-ABSENT": dict(
        edits=[(PENDING, '''            // The request may or may not have taken effect, or take it
            // later: it is discovered through the helper's membership and
            // proven like an accepted one, or settled, and nothing the
            // manager answers without a candidate releases it.
            Started::Uncertain(reason) => Some(reason),''', '''            // NC-I4-START-TIMEOUT-ABSENT: the uncertain start is taken
            // as no effect.
            Started::Uncertain(reason) => {
                self.issued = false;
                return Err(ScopeError::Bus(reason));
            }''')],
        reason="I4-R1 removed the uncertain arm's GetUnit absence check (A-R1-5), so the arm "
               "the I4 anchor quoted no longer exists; the same mutation (an uncertain start "
               "taken as no effect: nothing owned, nothing reconciled) is applied to the arm as "
               "it now stands. Its test keeps its name and marker; it now places through the "
               "harness's direct owner (execution::place), since the public start is gone.",
    ),
    "NC-I4-PENDING-NOT-RETAINED": dict(
        edits=[(EXEC, '''    let scope = std::mem::take(scope);
    match (scope.holds(), helper.take()) {''', '''    // NC-I4-PENDING-NOT-RETAINED: a pending operation is not handed on.
    let scope = match std::mem::take(scope) {
        ScopeBoundary::Pending(_) => ScopeBoundary::None,
        other => other,
    };
    match (scope.holds(), helper.take()) {''')],
        reason="Owned::retain's body moved into the free function retain(), now shared by the "
               "execution's owner and the harness's direct owner (ScopedHelper::end); the same "
               "mutation is applied there. Same test, same marker.",
    ),
}

# ---------------------------------------------------------------------------
# The I4-R1 controls.

STOP_CALL = "        self.last_stop = Some(controller.manager.stop_unit(&self.unit));\n"
CONTROL_GROUP = '''        match controller.manager.control_group(&unit_path) {
            Remote::Answered(Some(group)) if group == path => {}
            Remote::Answered(_) => return Err(ScopeError::Mismatch("unit control group")),
            Remote::Uncertain(reason) => return Err(ScopeError::Bus(reason)),
        }
'''

I4R1_REQUIRED = [
    control(
        "NC-I4R1-PANIC-DROPS-PENDING", "i4r1-required",
        "the harness's direct start resumes a panic past its owner while the operation is unresolved",
        [(EXEC, '''        Err(_) => None,
    };
    Err(PlacementFailed {''', '''        // NC-I4R1-PANIC-DROPS-PENDING: the panic resumes past the owner.
        Err(panic) => std::panic::resume_unwind(panic),
    };
    Err(PlacementFailed {''')],
        "execution::tests::i4r1_01_a_panic_after_an_uncertain_start_without_a_candidate_keeps_its_owner",
        "a panic crossed the direct owner of an unresolved operation",
    ),
    control(
        "NC-I4R1-SPLIT-OWNER", "i4r1-required",
        "a failed direct placement returns the helper apart from its unresolved operation",
        [(EXEC, '''    pub error: Option<ScopeError>,
    pub cleanup: Cleanup,
}''', '''    pub error: Option<ScopeError>,
    pub cleanup: Cleanup,
    // NC-I4R1-SPLIT-OWNER: the helper, returned apart from its operation.
    pub helper: Option<Helper>,
}'''),
         (EXEC, '''    Err(PlacementFailed {
        error,
        cleanup: owned.end(),
    })''', '''    let helper = owned.helper.take();
    Err(PlacementFailed {
        error,
        cleanup: owned.end(),
        helper,
    })''')],
        "execution::tests::i4r1_02_a_direct_placement_failure_returns_one_owner_of_the_operation_and_its_helper",
        "the operation's failure was split from its helper",
    ),
    control(
        "NC-I4R1-PUBLIC-START", "i4r1-required",
        "a public direct scope start in every build (the weaker lifecycle beside execution::run)",
        [(SCOPE, '''        Ok(Box::new(PendingScope::new(
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

    /// NC-I4R1-PUBLIC-START: a public direct scope start.
    pub fn start(&self, helper: &Helper, limits: &ResourcePolicy) -> Result<Scope, ScopeError> {
        let mut boundary = ScopeBoundary::Pending(self.prepare(helper, limits)?);
        boundary.establish(helper, None)?;
        match boundary {
            ScopeBoundary::Proven(scope) => Ok(scope),
            _ => Err(ScopeError::Mismatch("scope not proven")),
        }
    }
}''')],
        "scope::tests::i4r1_x_a_normal_build_starts_a_scope_only_within_run",
        "a normal build exposes a direct scope start",
    ),
    control(
        "NC-I4R1-PUBLIC-CONNECT-AT", "i4r1-required",
        "a caller-selected manager socket in every build",
        [(SCOPE, '''    #[cfg(any(test, feature = "live-sandbox-harness"))]
    pub fn connect_at(path: &str)''', '''    // NC-I4R1-PUBLIC-CONNECT-AT: a caller-selected socket in every build.
    pub fn connect_at(path: &str)''')],
        "scope::tests::i4r1_04_a_normal_build_constructs_a_manager_only_by_connect",
        "a normal build exposes a caller-selected manager socket",
    ),
    control(
        "NC-I4R1-NO-CONTROLGROUP", "i4r1-required",
        "Pending becomes Proven without querying or binding the unit's ControlGroup",
        [(PENDING, CONTROL_GROUP,
          "        // NC-I4R1-NO-CONTROLGROUP: the control group is never bound.\n")],
        "execution::tests::i4r1_10_the_exact_binding_of_the_manager_s_unit_to_the_kernel_s_cgroup_proves",
        "the scope was proven without binding the manager's unit to the kernel's cgroup",
    ),
    control(
        "NC-I4R1-CONTROLGROUP-BASENAME", "i4r1-required",
        "the manager's ControlGroup and the kernel's path are compared by their last components only",
        [(PENDING, '''            Remote::Answered(Some(group)) if group == path => {}''',
          '''            // NC-I4R1-CONTROLGROUP-BASENAME: the last components only.
            Remote::Answered(Some(group)) if group.rsplit('/').next() == path.rsplit('/').next() => {}''')],
        "execution::tests::i4r1_06_a_cgroup_of_the_unit_s_name_in_another_place_is_never_proven",
        "a cgroup of the unit's name that is not the unit's own was proven",
    ),
    control(
        "NC-I4R1-CONTROLGROUP-MISMATCH", "i4r1-required",
        "a ControlGroup that disagrees with the kernel's path is ignored",
        [(PENDING, '''            Remote::Answered(_) => return Err(ScopeError::Mismatch("unit control group")),''',
          '''            // NC-I4R1-CONTROLGROUP-MISMATCH: any answer is accepted.
            Remote::Answered(_) => {}''')],
        "execution::tests::i4r1_07_a_control_group_that_is_not_exactly_the_kernel_s_is_refused",
        "a control group that is not exactly the kernel's was accepted",
    ),
    control(
        "NC-I4R1-ID-MISMATCH", "i4r1-required",
        "a unit Id other than the generated name is ignored",
        [(PENDING, '''            Remote::Answered(_) => return Err(ScopeError::Mismatch("unit id")),''',
          '''            // NC-I4R1-ID-MISMATCH: any answer is accepted.
            Remote::Answered(_) => {}''')],
        "execution::tests::i4r1_09_a_unit_id_that_is_not_exactly_the_generated_name_is_refused",
        "a unit id that is not the generated name was accepted",
    ),
    control(
        "NC-I4R1-ABSENCE-GETUNIT", "i4r1-required",
        "GetUnit NoSuchUnit with the helper outside confirms an uncertain start without a candidate (the I4 R1 rule restored)",
        [(PENDING, '''            // Without a delivered reply the manager's absence proves
            // nothing.
            None if !self.accepted => false,
''', '''            // NC-I4R1-ABSENCE-GETUNIT: the manager's absence settles an
            // uncertain start.
''')],
        "execution::tests::i4r1_11_an_uncertain_start_without_a_candidate_is_not_released_by_no_such_unit",
        "an uncertain start without a candidate was released by the manager's absence",
    ),
    control(
        "NC-I4R1-STOP-REPLY-ABSENCE", "i4r1-required",
        "a delivered StopUnit reply settles an uncertain start without a candidate",
        [(PENDING, STOP_CALL, STOP_CALL + '''        // NC-I4R1-STOP-REPLY-ABSENCE: a delivered StopUnit reply settles
        // an operation without a candidate.
        if self.candidate.is_none() && matches!(self.last_stop, Some(Remote::Answered(()))) {
            self.settled = true;
            return true;
        }
''')],
        "execution::tests::i4r1_12_an_uncertain_start_without_a_candidate_is_not_released_by_a_stop_reply",
        "a StopUnit reply released an uncertain start without a candidate",
    ),
    control(
        "NC-I4R1-REAP-UNRESOLVED", "i4r1-required",
        "the helper is reaped while its uncertain operation stays unresolved",
        [(EXEC, '''            if !pending.reconcile(helper.as_ref(), fault) {
                return false;
            }''', '''            if !pending.reconcile(helper.as_ref(), fault) {
                // NC-I4R1-REAP-UNRESOLVED: reaped while unresolved.
                if let Some(child) = helper.as_mut() {
                    let _ = reap_within(child, FINALIZE_TIMEOUT);
                }
                return false;
            }''')],
        "execution::tests::i4r1_13_an_uncertain_start_without_a_candidate_is_not_released_by_a_timed_out_stop",
        "the helper of an unresolved uncertain start was reaped",
    ),
    control(
        "NC-I4R1-SERIAL-WRAP", "i4r1-required",
        "the helper identity counter wraps (the I4 fetch_add behaviour)",
        [(LAUNCHER, '''        serial.checked_add(1).filter(|_| serial != 0)''',
          '''        // NC-I4R1-SERIAL-WRAP: the counter wraps.
        Some(serial.wrapping_add(1))''')],
        "execution::tests::i4r1_20_helper_identities_are_refused_instead_of_wrapping",
        "the helper identity wrapped",
    ),
    control(
        "NC-I4R1-SPAWN-BEFORE-IDENTITY", "i4r1-required",
        "the helper is spawned before its identity is allocated (the I4 order)",
        [(LAUNCHER, '''        // The identity first: no child exists without one, and none is
        // spawned once they are exhausted.
        let serial = allocate_serial(serials).ok_or(LaunchError::IdentitiesExhausted)?;
''', ''),
         (LAUNCHER, '''        #[cfg(test)]
        SPAWNED.with(|spawned| spawned.set(spawned.get() + 1));
''', '''        #[cfg(test)]
        SPAWNED.with(|spawned| spawned.set(spawned.get() + 1));
        // NC-I4R1-SPAWN-BEFORE-IDENTITY: the identity after the spawn.
        let serial = allocate_serial(serials).ok_or(LaunchError::IdentitiesExhausted)?;
''')],
        "execution::tests::i4r1_21_no_helper_is_spawned_once_identities_are_exhausted",
        "a helper was spawned without an identity",
    ),
    control(
        "NC-I4R1-HARNESS-WEAK-OWNER", "i4r1-required",
        "the harness's direct start owns the operation in a local until it is proven, not with its helper",
        [(EXEC, '''        self.scope = ScopeBoundary::Pending(scopes.prepare(helper, limits)?);
        self.scope.establish(helper, None)''', '''        // NC-I4R1-HARNESS-WEAK-OWNER: owned by a local until proven.
        let mut scope = ScopeBoundary::Pending(scopes.prepare(helper, limits)?);
        scope.establish(helper, None)?;
        self.scope = scope;
        Ok(())''')],
        "execution::tests::i4r1_02_a_direct_placement_failure_returns_one_owner_of_the_operation_and_its_helper",
        "the direct owner lost the unresolved operation",
    ),
]

I4R1_ADDITIONAL = [
    control(
        "NC-I4R1-X-CONTROLGROUP-UNCERTAIN", "i4r1-additional",
        "an uncertain ControlGroup read is taken as a binding",
        [(PENDING, '''            Remote::Uncertain(reason) => return Err(ScopeError::Bus(reason)),
        }
        fault::at(fault, FaultPoint::ScopeProperties);''', '''            // NC-I4R1-X-CONTROLGROUP-UNCERTAIN: an uncertain answer binds.
            Remote::Uncertain(_) => {}
        }
        fault::at(fault, FaultPoint::ScopeProperties);''')],
        "execution::tests::i4r1_08_an_unavailable_or_uncertain_control_group_is_refused",
        "an unavailable or malformed control group was accepted",
    ),
    control(
        "NC-I4R1-X-ACCEPTED-EARLY", "i4r1-additional",
        "a start is taken as accepted when it is dispatched, before its reply is recorded",
        [(PENDING, '''        // From here the request may have an effect: it is owned.
        self.issued = true;''', '''        // From here the request may have an effect: it is owned.
        self.issued = true;
        // NC-I4R1-X-ACCEPTED-EARLY: accepted before its reply.
        self.accepted = true;''')],
        "execution::tests::i4r1_01_a_panic_after_an_uncertain_start_without_a_candidate_keeps_its_owner",
        "a panic after an uncertain start lost or released its operation",
    ),
    control(
        "NC-I4R1-X-MEMBERSHIP-NOT-NORMAL", "i4r1-additional",
        "a kernel membership path not in normal form locates the candidate (the last component alone)",
        [(PENDING, '''    path.strip_prefix('/')
        .is_some_and(|relative| relative.split('/').all(|part| !matches!(part, "" | ".")))
        && path.rsplit('/').next() == Some(unit)
        && !path.contains("..")''', '''    // NC-I4R1-X-MEMBERSHIP-NOT-NORMAL: the last component alone.
    path.rsplit('/').next() == Some(unit) && !path.contains("..")''')],
        "execution::tests::i4r1_x_a_membership_path_not_in_normal_form_never_locates_a_candidate",
        "a membership path not in normal form located the scope",
    ),
]


def sha256(path: pathlib.Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def i4_controls(root: pathlib.Path):
    """The 21 accepted I4 controls: imported unchanged, except the two whose
    defect point moved (redefined with the same mutation, test and marker).
    Returns (controls, i4 script sha256)."""
    path = root / I4_SCRIPT
    spec = importlib.util.spec_from_file_location("i4_scope_controls", path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    controls = []
    for original in module.REQUIRED + module.ADDITIONAL:
        c = dict(original)
        c["category"] = "i4-" + original["category"]
        adapted = ADAPTED_I4.get(original["id"])
        if adapted is None:
            c["mapping"] = dict(kind="unchanged", i4_test=original["test"],
                                reason="the I4 definition, byte for byte")
        else:
            c["edits"] = [dict(file=f, anchor=a, replacement=r)
                          for (f, a, r) in adapted["edits"]]
            c["mapping"] = dict(kind="adapted", i4_test=original["test"],
                                i4_edits=original["edits"], reason=adapted["reason"])
        controls.append(c)
    return controls, sha256(path)


def state() -> dict:
    return {path: sha256(ROOT / path) for path in FILES}


def status() -> set:
    out = subprocess.run(
        ["git", "status", "--porcelain=v1", "--untracked-files=all"],
        cwd=ROOT, capture_output=True, text=True, check=True,
    ).stdout
    return {line for line in out.splitlines() if line}


def mutate(control_, originals):
    """The mutated text of every file the control edits (anchors applied in
    order, each exactly once in the text as it stands)."""
    texts = {}
    for edit in control_["edits"]:
        if edit["file"] not in MUTABLE:
            raise SystemExit(f"{control_['id']}: edits a file that is not mutable: {edit['file']}")
        text = texts.get(edit["file"], originals[edit["file"]].decode())
        count = text.count(edit["anchor"])
        if count != 1:
            raise SystemExit(
                f"{control_['id']}: anchor occurs {count} times in {edit['file']}: "
                f"{edit['anchor'][:80]!r}")
        texts[edit["file"]] = text.replace(edit["anchor"], edit["replacement"], 1)
    return texts


def run_test(test: str) -> subprocess.CompletedProcess:
    env = dict(os.environ, CARGO_TARGET_DIR=str(TARGET_DIR))
    return subprocess.run(
        ["cargo", "test", "-p", "nexus-verifier-sandbox", "--locked", "--lib",
         "--", test, "--exact"],
        cwd=ROOT, capture_output=True, text=True, timeout=1800, env=env,
    )


def message_of(output: str, test: str) -> str:
    """The message of the last panic on the test's own thread: the assertion
    that ended the test. Earlier panics (an injected fault the execution
    contained, or one inside the simulation) are not the intended
    assertion."""
    lines = output.splitlines()
    starts = [
        i for i, line in enumerate(lines)
        if line.startswith(f"thread '{test}'") and "panicked at" in line
    ]
    if not starts:
        return ""
    tail = []
    for line in lines[starts[-1] + 1:]:
        if (line.startswith("note: run with") or line.startswith("failures:")
                or line.startswith("thread '")):
            break
        tail.append(line)
    return "\n".join(tail).strip()


def run_one(control_, originals, original_state):
    texts = mutate(control_, originals)
    if state() != original_state:
        raise SystemExit(f"{control_['id']}: files changed before the control")
    try:
        for path, text in texts.items():
            (ROOT / path).write_bytes(text.encode())
        proc = run_test(control_["test"])
    finally:
        for path in texts:
            (ROOT / path).write_bytes(originals[path])
    restored = state() == original_state
    worktree = status()
    if worktree != EXPECTED_STATUS:
        raise SystemExit(f"{control_['id']}: unexpected checkout state after restoring: {worktree}")
    output = proc.stdout + proc.stderr
    test = control_["test"]
    compiled = (
        "could not compile" not in output
        and "error[E" not in output
        and "Running unittests src/lib.rs" in output
    )
    failed = (
        proc.returncode != 0
        and "test result: FAILED. 0 passed; 1 failed" in output
        and f"{test} ... FAILED" in output
    )
    return output, compiled, failed, restored, message_of(output, test), proc.returncode


def baseline(controls, original_state):
    """Every intended test passes on the unmutated checkout (so each
    control's failure is the mutation's)."""
    tests = sorted({c["test"] for c in controls})
    results = {}
    for test in tests:
        proc = run_test(test)
        output = proc.stdout + proc.stderr
        results[test] = (
            proc.returncode == 0
            and "test result: ok. 1 passed; 0 failed" in output
        )
        (LOG / f"baseline--{test.replace('::', '__')}.log").write_text(output)
    if state() != original_state:
        raise SystemExit("files changed during the baseline")
    return results


def main() -> int:
    global ROOT, LOG, TARGET_DIR, EXPECTED_STATUS
    args = sys.argv[1:]
    if len(args) == 2 and args[1] == "--check-anchors":
        ROOT = pathlib.Path(args[0]).resolve()
        i4, i4_sha = i4_controls(ROOT)
        counted = i4 + I4R1_REQUIRED + I4R1_ADDITIONAL
        originals = {path: (ROOT / path).read_bytes() for path in FILES}
        ids = [c["id"] for c in counted]
        if len(ids) != len(set(ids)):
            raise SystemExit("duplicate control ids")
        for c in counted:
            mutate(c, originals)
        print(json.dumps({"controls": len(counted), "i4": len(i4),
                          "i4_adapted": sum(1 for c in i4 if c["mapping"]["kind"] == "adapted"),
                          "i4r1_required": len(I4R1_REQUIRED),
                          "i4r1_additional": len(I4R1_ADDITIONAL),
                          "i4_script_sha256": i4_sha, "anchors": "ok"}))
        return 0
    if "--target-dir" not in args:
        raise SystemExit(__doc__)
    at = args.index("--target-dir")
    TARGET_DIR = pathlib.Path(args[at + 1]).resolve()
    del args[at:at + 2]
    only = None
    if "--only" in args:
        at = args.index("--only")
        only = set(args[at + 1].split(","))
        del args[at:at + 2]
    if len(args) != 2:
        raise SystemExit(__doc__)
    ROOT = pathlib.Path(args[0]).resolve()
    LOG = pathlib.Path(args[1]).resolve()
    LOG.mkdir(parents=True, exist_ok=True)
    if TARGET_DIR.exists() and any(TARGET_DIR.iterdir()):
        raise SystemExit(f"the target directory is not empty: {TARGET_DIR}")
    TARGET_DIR.mkdir(parents=True, exist_ok=True)
    i4, i4_sha = i4_controls(ROOT)
    counted = [c for c in i4 + I4R1_REQUIRED + I4R1_ADDITIONAL
               if only is None or c["id"] in only]
    original_state = state()
    originals = {path: (ROOT / path).read_bytes() for path in FILES}
    if status() != EXPECTED_STATUS:
        raise SystemExit(f"unexpected checkout state before the controls: {status()}")
    passing = baseline(counted, original_state)
    results = []
    for c in counted:
        output, compiled, failed, restored, message, code = run_one(c, originals, original_state)
        (LOG / f"{c['id']}.log").write_text(output)
        marked = c["marker"] in message
        ok = passing[c["test"]] and compiled and failed and marked and restored
        result = dict(
            id=c["id"], category=c["category"], what=c["what"],
            files=sorted({e["file"] for e in c["edits"]}), edits=len(c["edits"]),
            test=c["test"], marker=c["marker"], mapping=c.get("mapping"),
            test_passes_unmutated=passing[c["test"]],
            compiled=compiled, failed_intended_test=failed, marker_in_assertion=marked,
            files_restored=restored, exit=code, ok=ok, assertion=message[:1500],
        )
        results.append(result)
        print(json.dumps(result), flush=True)
    final_state = state()
    by_category = {}
    for r in results:
        by_category.setdefault(r["category"], []).append(r["id"])
    summary = dict(
        i4_script=I4_SCRIPT,
        i4_script_sha256=i4_sha,
        original=original_state,
        final=final_state,
        identical=final_state == original_state,
        status=sorted(status()),
        expected_status=sorted(EXPECTED_STATUS),
        counted=len(results),
        by_category={k: len(v) for k, v in sorted(by_category.items())},
        i4_adapted=sorted(r["id"] for r in results
                          if (r["mapping"] or {}).get("kind") == "adapted"),
        baseline_all_pass=all(passing.values()),
        all_required=all(r["ok"] for r in results),
        failures=[r["id"] for r in results if not r["ok"]],
    )
    print(json.dumps(summary), flush=True)
    (LOG / "summary.json").write_text(json.dumps(
        dict(summary=summary, baseline=passing, counted=results), indent=2) + "\n")
    return 0 if summary["identical"] and summary["all_required"] and summary["baseline_all_pass"] else 1


if __name__ == "__main__":
    sys.exit(main())

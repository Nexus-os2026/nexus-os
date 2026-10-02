#!/usr/bin/env python3
"""P2-V1-R3B-I4 behavioural negative controls: native scope ownership.

Each control restores one wrong behaviour in the production scope or
execution sources (src/scope.rs, src/scope/pending.rs, src/execution.rs)
and must:

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

The tests themselves (src/execution/tests.rs, src/scope/tests.rs) and the
simulation they run over are never mutated; they are hashed to prove it.
Every test runs over the deterministic simulation of the user manager and
the kernel: no control contacts a bus or creates a cgroup.

Categories, counted separately: "required" (the mission's sixteen named
controls) and "additional" (further mechanisms, beyond the named list).
API/type guards are a separate script (api_guards.py) and are never added
into one figure with these.

Usage:
  scope_controls.py <checkout> <log directory> --target-dir <empty dir>
  scope_controls.py <checkout> --check-anchors

<checkout> must be an isolated, clean Git checkout of the candidate (never
the worktree under review); the target directory must be dedicated to
these runs. Run it alone: it mutates and restores files in the checkout.
"""
import hashlib
import json
import os
import pathlib
import subprocess
import sys

CRATE = "crates/nexus-verifier-sandbox"
SCOPE = f"{CRATE}/src/scope.rs"
PENDING = f"{CRATE}/src/scope/pending.rs"
EXEC = f"{CRATE}/src/execution.rs"
FILES = [
    SCOPE,
    PENDING,
    EXEC,
    f"{CRATE}/src/scope/manager.rs",
    f"{CRATE}/src/scope/native.rs",
    f"{CRATE}/src/scope/tests.rs",
    f"{CRATE}/src/execution/tests.rs",
    f"{CRATE}/src/fault.rs",
    f"{CRATE}/src/launcher.rs",
]
MUTABLE = {SCOPE, PENDING, EXEC}

ROOT = None
LOG = None
TARGET_DIR = None
EXPECTED_STATUS = set()


def control(cid, category, what, edits, test, marker):
    return dict(
        id=cid, category=category, what=what,
        edits=[dict(file=f, anchor=a, replacement=r) for (f, a, r) in edits],
        test=test, marker=marker,
    )


STOP_CALL = "        self.last_stop = Some(controller.manager.stop_unit(&self.unit));\n"

REQUIRED = [
    control(
        "NC-I4-START-TIMEOUT-ABSENT", "required",
        "an uncertain StartTransientUnit (timeout, error, malformed reply) is taken as no effect: nothing is owned, nothing reconciled",
        [(PENDING, '''            Started::Uncertain(reason) => {
                // The request may or may not have taken effect. It ended
                // without one only if the manager now answers that no unit
                // of the name is loaded and the helper is outside it;
                // otherwise it is discovered and proven like an accepted
                // one, or settled.
                fault::at(fault, FaultPoint::ScopeReconcile);
                if absent(&controller, &self.unit, Some(helper)) {
                    return Err(ScopeError::Bus(reason));
                }
                Some(reason)
            }''', '''            Started::Uncertain(reason) => {
                // NC-I4-START-TIMEOUT-ABSENT: the uncertain start is taken
                // as no effect.
                self.issued = false;
                return Err(ScopeError::Bus(reason));
            }''')],
        "scope::tests::i4_03_a_start_with_effect_whose_reply_is_lost_is_discovered_and_proven",
        "the unit a lost-reply start created was not discovered and proven",
    ),
    control(
        "NC-I4-START-LOST-OWNER", "required",
        "a start that failed or stayed uncertain returns its error with no owner for what it may have created",
        [(EXEC, '''    if let Err(error) = owned.scope.establish(helper, fault) {
        return not_run(NotRun::Scope(error));
    }''', '''    if let Err(error) = owned.scope.establish(helper, fault) {
        // NC-I4-START-LOST-OWNER: the failed operation is dropped.
        owned.scope = ScopeBoundary::None;
        return not_run(NotRun::Scope(error));
    }''')],
        "execution::tests::i4_21_an_unconfirmed_operation_is_a_cleanup_failure_never_an_unavailable_sandbox",
        "an unconfirmed scope operation was reported as an unavailable sandbox with confirmed cleanup",
    ),
    control(
        "NC-I4-START-NAME-AUTH", "required",
        "the unit name alone locates and proves the scope (its cgroup taken beside the helper's, no membership or process-list proof)",
        [(PENDING, '''        let path = wait_for_placement(controller, helper, &self.unit)?;
        fault::at(fault, FaultPoint::ScopeProof);''', '''        // NC-I4-START-NAME-AUTH: the unit name alone locates the scope.
        let beside = controller
            .native
            .membership(helper)
            .map_err(ScopeError::Io)?
            .unwrap_or_default();
        let parent = beside.rsplit_once('/').map_or("", |(parent, _)| parent);
        let path = format!("{parent}/{}", self.unit);
        fault::at(fault, FaultPoint::ScopeProof);'''),
         (PENDING, '''        let procs = candidate.read("cgroup.procs").map_err(ScopeError::Io)?;
        if !procs
            .lines()
            .any(|line| line.trim() == helper.pid().to_string())
        {
            return Err(ScopeError::Mismatch("helper not in the scope"));
        }
''', '''        // ...and proves it: the process list is not checked.
''')],
        "execution::tests::i4_16_a_pending_operation_is_never_proven_by_its_unit_name",
        "a scope was proven by its unit name",
    ),
    control(
        "NC-I4-LAUNCH-PENDING", "required",
        "the helper is handshaken and launched whatever the scope's state (pending included)",
        [(EXEC, '''    if let Err(error) = owned.scope.establish(helper, fault) {
        return not_run(NotRun::Scope(error));
    }
    fault::at(fault, FaultPoint::AfterScope);
    // Nothing reaches the helper unless its scope is proven.
    if !owned.scope.is_proven() {
        return not_run(NotRun::Scope(ScopeError::Mismatch("scope not proven")));
    }''', '''    // NC-I4-LAUNCH-PENDING: the launch does not wait for the proof.
    let _ = owned.scope.establish(helper, fault);
    fault::at(fault, FaultPoint::AfterScope);''')],
        "execution::tests::i4_18_no_launch_message_reaches_the_helper_before_the_scope_is_proven",
        "a launch message reached the helper before the proof",
    ),
    control(
        "NC-I4-NO-EARLY-DIR", "required",
        "the candidate's descriptor is kept only once every proof passed, not as soon as the helper is seen in it",
        [(PENDING, '''        let candidate = &**self.candidate.insert(controller.native.open(&path)?);
        fault::at(fault, FaultPoint::ScopeCandidate);''', '''        // NC-I4-NO-EARLY-DIR: the candidate is kept only once proven.
        let opened = controller.native.open(&path)?;
        let candidate = &*opened;
        fault::at(fault, FaultPoint::ScopeCandidate);'''),
         (PENDING, '''            Remote::Answered(Some(policy)) if policy == "continue" => {}
            Remote::Answered(_) => return Err(ScopeError::Mismatch("out-of-memory policy")),
            Remote::Uncertain(reason) => return Err(ScopeError::Bus(reason)),
        }
        Ok(())''', '''            Remote::Answered(Some(policy)) if policy == "continue" => {}
            Remote::Answered(_) => return Err(ScopeError::Mismatch("out-of-memory policy")),
            Remote::Uncertain(reason) => return Err(ScopeError::Bus(reason)),
        }
        self.candidate = Some(opened);
        Ok(())''')],
        "execution::tests::i4_10_an_uncertain_query_after_the_candidate_keeps_its_descriptor",
        "the candidate retained before the uncertain query was not kept",
    ),
    control(
        "NC-I4-PROPERTY-ERROR-DROPS", "required",
        "an uncertain property read (RuntimeMaxUSec, OOMPolicy) drops the retained candidate",
        [(PENDING, '''            Remote::Answered(_) => return Err(ScopeError::Mismatch("runtime backstop")),
            Remote::Uncertain(reason) => return Err(ScopeError::Bus(reason)),''', '''            Remote::Answered(_) => return Err(ScopeError::Mismatch("runtime backstop")),
            // NC-I4-PROPERTY-ERROR-DROPS: the candidate is dropped.
            Remote::Uncertain(reason) => {
                self.candidate = None;
                return Err(ScopeError::Bus(reason));
            }'''),
         (PENDING, '''            Remote::Answered(_) => return Err(ScopeError::Mismatch("out-of-memory policy")),
            Remote::Uncertain(reason) => return Err(ScopeError::Bus(reason)),''', '''            Remote::Answered(_) => return Err(ScopeError::Mismatch("out-of-memory policy")),
            Remote::Uncertain(reason) => {
                self.candidate = None;
                return Err(ScopeError::Bus(reason));
            }''')],
        "execution::tests::i4_04_a_property_timeout_after_placement_never_launches_and_keeps_the_candidate",
        "cleanup not confirmed through the retained candidate",
    ),
    control(
        "NC-I4-STOP-OK-CONFIRMS", "required",
        "a delivered StopUnit reply is taken as cleanup confirmation",
        [(PENDING, STOP_CALL, '''        // NC-I4-STOP-OK-CONFIRMS: a delivered reply is taken as cleanup.
        let stopped = controller.manager.stop_unit(&self.unit);
        if let Remote::Answered(()) = stopped {
            self.settled = true;
            return true;
        }
        self.last_stop = Some(stopped);
''')],
        "execution::tests::i4_07_a_delivered_stop_reply_with_the_scope_still_populated_confirms_nothing",
        "a delivered StopUnit reply was taken as cleanup",
    ),
    control(
        "NC-I4-STOP-ERR-NO-EFFECT", "required",
        "a StopUnit error reply is taken as proof that nothing was there to stop",
        [(PENDING, STOP_CALL, '''        // NC-I4-STOP-ERR-NO-EFFECT: an error reply is taken as nothing left.
        let stopped = controller.manager.stop_unit(&self.unit);
        if let Remote::Uncertain(reason) = &stopped {
            if reason.starts_with("StopUnit: ") {
                self.settled = true;
                return true;
            }
        }
        self.last_stop = Some(stopped);
''')],
        "execution::tests::i4_06_a_failed_stop_of_a_unit_the_helper_never_entered_retains_the_operation",
        "a StopUnit error was taken as proof that nothing remained",
    ),
    control(
        "NC-I4-STOP-TIMEOUT-DROPS", "required",
        "a timed-out StopUnit drops the pending operation",
        [(PENDING, STOP_CALL, '''        // NC-I4-STOP-TIMEOUT-DROPS: a timeout drops the operation.
        let stopped = controller.manager.stop_unit(&self.unit);
        if let Remote::Uncertain(reason) = &stopped {
            if reason.ends_with("timed out") {
                self.settled = true;
                return true;
            }
        }
        self.last_stop = Some(stopped);
''')],
        "execution::tests::i4_09_a_timed_out_stop_whose_target_remains_retains_the_operation",
        "a timed-out StopUnit dropped the operation",
    ),
    control(
        "NC-I4-MANAGER-ABSENCE-ONLY", "required",
        "the manager's NoSuchUnit alone confirms absence, whatever the kernel reports of the helper",
        [(PENDING, '''    ) && outside(controller, helper, unit)
}''', '''    ) && helper.is_some() // NC-I4-MANAGER-ABSENCE-ONLY
}''')],
        "execution::tests::i4_11_manager_absence_with_the_helper_in_the_candidate_is_refused_and_retained",
        "manager absence alone confirmed an operation whose helper is in a cgroup of its unit",
    ),
    control(
        "NC-I4-MEMBERSHIP-NAME-ONLY", "required",
        "the cgroup named in the helper's membership is proven by its name alone, without its process list",
        [(PENDING, '''        let procs = candidate.read("cgroup.procs").map_err(ScopeError::Io)?;
        if !procs
            .lines()
            .any(|line| line.trim() == helper.pid().to_string())
        {
            return Err(ScopeError::Mismatch("helper not in the scope"));
        }
''', '''        // NC-I4-MEMBERSHIP-NAME-ONLY: the process list is not checked.
''')],
        "execution::tests::i4_15_a_membership_mismatch_never_launches",
        "a cgroup that does not list the helper was proven",
    ),
    control(
        "NC-I4-PENDING-NOT-RETAINED", "required",
        "an unresolved pending operation is not handed to the retained boundary",
        [(EXEC, '''        let scope = std::mem::take(&mut self.scope);
        match (scope.holds(), self.helper.take()) {''', '''        // NC-I4-PENDING-NOT-RETAINED: a pending operation is not handed on.
        let scope = match std::mem::take(&mut self.scope) {
            ScopeBoundary::Pending(_) => ScopeBoundary::None,
            other => other,
        };
        match (scope.holds(), self.helper.take()) {''')],
        "execution::tests::i4_20_finalization_settles_a_pending_operation",
        "the unresolved operation was not retained",
    ),
    control(
        "NC-I4-RETRY-NEEDS-MANAGER-BORROW", "required",
        "a pending operation reaches its manager connection only through the ScopeManager (a non-owning handle)",
        [(PENDING, '''    controller: Arc<Controller>,
    /// The backend-generated unit name: the request's locator, no''', '''    controller: std::sync::Weak<Controller>,
    /// The backend-generated unit name: the request's locator, no'''),
         (PENDING, '''        controller: Arc<Controller>,
        unit: String,
        helper: &Helper,''', '''        controller: std::sync::Weak<Controller>,
        unit: String,
        helper: &Helper,'''),
         (PENDING, '''        let controller = Arc::clone(&self.controller);
        let helper = helper.filter(|helper| self.binds(helper));''', '''        // NC-I4-RETRY-NEEDS-MANAGER-BORROW: only while the manager lives.
        let Some(controller) = self.controller.upgrade() else {
            return false;
        };
        let helper = helper.filter(|helper| self.binds(helper));'''),
         (PENDING, '''        let controller = Arc::clone(&self.controller);
        fault::at(fault, FaultPoint::BeforeScopeStart);''', '''        let Some(controller) = self.controller.upgrade() else {
            return Err(ScopeError::Mismatch("scope operation"));
        };
        fault::at(fault, FaultPoint::BeforeScopeStart);'''),
         (SCOPE, '''            Arc::clone(&self.controller),
            unit,''', '''            Arc::downgrade(&self.controller),
            unit,''')],
        "execution::tests::i4_23_a_retained_boundary_is_retried_without_the_scope_manager",
        "a retained boundary needs the ScopeManager that started it",
    ),
    control(
        "NC-I4-PANIC-OWNER-LOSS", "required",
        "the operation is owned by a local of the attempt while its request is uncertain, so a panic there loses it",
        [(EXEC, '''    match scopes.prepare(helper, limits) {
        Ok(pending) => owned.scope = ScopeBoundary::Pending(pending),
        Err(error) => return not_run(NotRun::Scope(error)),
    }
    if let Err(error) = owned.scope.establish(helper, fault) {
        return not_run(NotRun::Scope(error));
    }''', '''    // NC-I4-PANIC-OWNER-LOSS: owned by a local until the request returns.
    let mut pending = match scopes.prepare(helper, limits) {
        Ok(pending) => ScopeBoundary::Pending(pending),
        Err(error) => return not_run(NotRun::Scope(error)),
    };
    let established = pending.establish(helper, fault);
    owned.scope = pending;
    if let Err(error) = established {
        return not_run(NotRun::Scope(error));
    }''')],
        "execution::tests::i4_25_a_panic_after_the_start_request_keeps_its_owner",
        "a panic after StartTransientUnit lost the operation's owner",
    ),
    control(
        "NC-I4-PID-CLEANUP", "required",
        "the helper's death (a kill and reap of its process) is taken as the pending operation's cleanup",
        [(EXEC, '''        ScopeBoundary::Pending(pending) => {
            if let Some(child) = helper.as_mut() {
                let _ = child.kill();
            }
            if !pending.reconcile(helper.as_ref(), fault) {
                return false;
            }
            *scope = ScopeBoundary::None;
        }''', '''        ScopeBoundary::Pending(_) => {
            // NC-I4-PID-CLEANUP: the helper's death is taken as cleanup.
            let _ = fault;
            *scope = ScopeBoundary::None;
        }''')],
        "execution::tests::i4_30_the_helper_s_death_is_never_the_operation_s_cleanup",
        "the helper's death was taken as the operation's cleanup",
    ),
    control(
        "NC-I4-STRING-SWEEP", "required",
        "the unit's cgroup is rebuilt from the unit-name string (beside the helper's cgroup) and claimed for cleanup",
        [(PENDING, '''        if let Ok(Some(path)) = controller.native.membership(helper) {
            if names_unit(&path, &self.unit) {''', '''        if let Ok(Some(membership)) = controller.native.membership(helper) {
            // NC-I4-STRING-SWEEP: the cgroup is rebuilt from the unit name.
            let path = match membership.rsplit_once('/') {
                Some((parent, _)) => format!("{parent}/{}", self.unit),
                None => membership,
            };
            if names_unit(&path, &self.unit) {''')],
        "execution::tests::i4_30_no_cgroup_of_the_unit_s_name_is_ever_taken_by_its_path",
        "a cgroup was opened by a path the kernel never reported for the helper",
    ),
]

ADDITIONAL = [
    control(
        "NC-I4-X-COLLISION-CLAIMED", "additional",
        "a unit already loaded under the requested name (UnitExists) is taken as this request's and settled as such",
        [(PENDING, '''            Started::Collision => {
                self.collided = true;
                return Err(ScopeError::Bus(
                    "StartTransientUnit: the unit exists".into(),
                ));
            }''', '''            // NC-I4-X-COLLISION-CLAIMED: a loaded unit of the name is taken as
            // this request's.
            Started::Collision => None,''')],
        "execution::tests::i4_17_a_name_collision_fails_closed_and_never_touches_the_other_unit",
        "the colliding unit, not this request's, was stopped",
    ),
    control(
        "NC-I4-X-DROP-REAPS", "additional",
        "a dropped boundary reaps the helper of an unresolved operation (its process id released while a start job may remain)",
        [(EXEC, '''        if !scope.unresolved() {
            let _ = helper.try_reap();
        }''', '''        // NC-I4-X-DROP-REAPS: reaped whatever the operation's state.
        for _ in 0..2000 {
            if !matches!(helper.try_reap(), Ok(None)) {
                break;
            }
            std::thread::sleep(Duration::from_millis(1));
        }''')],
        "execution::tests::i4_24_dropping_an_unresolved_operation_is_best_effort_and_confirms_nothing",
        "a dropped boundary reaped the helper",
    ),
    control(
        "NC-I4-X-ANY-HELPER", "additional",
        "a pending operation takes any helper's membership as evidence",
        [(PENDING, '''        helper.serial() == self.helper_serial && helper.pid() == self.helper_pid''',
          '''        let _ = (helper, self.helper_serial, self.helper_pid); // NC-I4-X-ANY-HELPER
        true''')],
        "scope::tests::i4_a_pending_operation_is_bound_to_its_own_helper",
        "another helper's membership settled the operation",
    ),
    control(
        "NC-I4-X-UNLOADED-PROVEN", "additional",
        "the manager's absence of the unit is ignored by the proof (the kernel's view alone is taken)",
        [(PENDING, '''            Remote::Answered(Presence::Absent) => {
                return Err(ScopeError::Mismatch("unit not loaded"))
            }''', '''            // NC-I4-X-UNLOADED-PROVEN: the manager's absence is ignored.
            Remote::Answered(Presence::Absent) => format!("/unit/{}", self.unit),''')],
        "execution::tests::i4_11_manager_absence_with_the_helper_in_the_candidate_is_refused_and_retained",
        "the manager's absence of the unit did not refuse the proof",
    ),
    control(
        "NC-I4-X-NO-CANDIDATE-KILL", "additional",
        "settling observes the candidate without ending what it holds",
        [(PENDING, '''        // Cleanup ownership: everything in the candidate is ended.
        if let Some(candidate) = &self.candidate {
            let _ = candidate.kill();
        }''', '''        // NC-I4-X-NO-CANDIDATE-KILL: nothing in the candidate is ended.'''),
         (PENDING, '''                if let Ok(candidate) = controller.native.open(&path) {
                    let _ = candidate.kill();
                    self.candidate = Some(candidate);''', '''                if let Ok(candidate) = controller.native.open(&path) {
                    self.candidate = Some(candidate);''')],
        "execution::tests::i4_04_a_property_timeout_after_placement_never_launches_and_keeps_the_candidate",
        "cleanup not confirmed through the retained candidate",
    ),
]

COUNTED = REQUIRED + ADDITIONAL


def sha256(path: pathlib.Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


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
    contained) are not the intended assertion."""
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


def baseline(original_state):
    """Every intended test passes on the unmutated checkout (so each
    control's failure is the mutation's)."""
    tests = sorted({c["test"] for c in COUNTED})
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
        originals = {path: (ROOT / path).read_bytes() for path in FILES}
        ids = [c["id"] for c in COUNTED]
        if len(ids) != len(set(ids)):
            raise SystemExit("duplicate control ids")
        for c in COUNTED:
            mutate(c, originals)
        print(json.dumps({"controls": len(COUNTED), "required": len(REQUIRED),
                          "additional": len(ADDITIONAL), "anchors": "ok"}))
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
    original_state = state()
    originals = {path: (ROOT / path).read_bytes() for path in FILES}
    if status() != EXPECTED_STATUS:
        raise SystemExit(f"unexpected checkout state before the controls: {status()}")
    passing = baseline(original_state)
    results = []
    for c in COUNTED:
        if only is not None and c["id"] not in only:
            continue
        output, compiled, failed, restored, message, code = run_one(c, originals, original_state)
        (LOG / f"{c['id']}.log").write_text(output)
        marked = c["marker"] in message
        ok = passing[c["test"]] and compiled and failed and marked and restored
        result = dict(
            id=c["id"], category=c["category"], what=c["what"],
            files=sorted({e["file"] for e in c["edits"]}), edits=len(c["edits"]),
            test=c["test"], marker=c["marker"],
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
        original=original_state,
        final=final_state,
        identical=final_state == original_state,
        status=sorted(status()),
        expected_status=sorted(EXPECTED_STATUS),
        counted=len(results),
        by_category={k: len(v) for k, v in sorted(by_category.items())},
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

#!/usr/bin/env python3
"""P2-V1-R3B-I4-R3 negative controls: no scope operation acts on a unit by
its name (mission section 16). Each control restores one wrong behaviour in
the production sources and must:

- apply: every edit's anchor occurs exactly once in its file, in order;
- compile: the library's unit-test target builds (no "error[E", no
  "could not compile");
- run its one intended test alone (--exact) and fail it ("0 passed; 1
  failed", "<test> ... FAILED");
- fail the intended assertion: the message of the last panic on the test's
  own thread carries the control's marker;
- restore: the mutated files are written back byte for byte in `finally`;
  every file of FILES is verified by SHA-256 before each control and after
  each restoration, and the checkout's Git status must equal the expected
  status.

A control that restores a manager request acting by name gives the
`Manager` trait a default method (so that the model's manager, which the
controls never mutate, still compiles): the model cannot observe such a
request, so those controls are caught by the structural guards
(`scope::tests::i4r3_10_*`); every other control is caught by a behavioural
test over the model. The infrastructure is P2-V1-R3B-I4-R2's r2_controls.py,
unchanged but for the control definitions and the mutable files (the
manager's source added).

The tests and the simulation they run over (src/scope/tests.rs,
src/execution/tests.rs) are never mutated; they are hashed to prove it.
Every test runs over the deterministic simulation of the user manager and
the kernel: no control contacts a bus or creates a cgroup.

Usage:
  r3_controls.py <checkout> <log directory> --target-dir <empty dir> [--only ID,...]
  r3_controls.py <checkout> --check-anchors

<checkout> must be an isolated, clean Git checkout of the candidate (never
the worktree under review); the target directory must be dedicated to these
runs. Run it alone: it mutates and restores files in the checkout.
"""
import hashlib
import json
import os
import pathlib
import subprocess
import sys

CRATE = "crates/nexus-verifier-sandbox"
PENDING = f"{CRATE}/src/scope/pending.rs"
EXEC = f"{CRATE}/src/execution.rs"
MANAGER = f"{CRATE}/src/scope/manager.rs"
FILES = [
    PENDING,
    EXEC,
    MANAGER,
    f"{CRATE}/src/scope.rs",
    f"{CRATE}/src/scope/native.rs",
    f"{CRATE}/src/scope/tests.rs",
    f"{CRATE}/src/execution/tests.rs",
    f"{CRATE}/src/fault.rs",
]
MUTABLE = {PENDING, EXEC, MANAGER}


def test(name):
    return f"execution::tests::{name}"


R3_01 = test("i4r3_01_an_accepted_operation_never_acts_on_a_unit_loaded_under_its_name_again")
R3_02 = test("i4r3_02_an_accepted_operation_whose_unit_stays_loaded_stays_owned_and_bounded")
R3_04 = test("i4r3_04_an_accepted_operation_s_candidate_is_ended_by_descriptor_and_observed")
R3_06 = test("i4r2_02_an_uncertain_start_that_did_nothing_is_never_stopped_by_name")
R3_07 = test("i4r1_15_a_collision_never_stops_or_claims_the_foreign_unit")
SURFACE = "scope::tests::i4r3_10_the_manager_can_only_start_a_scope_and_read"
SETTLING = "scope::tests::i4r3_10_settling_never_acts_on_a_unit_by_its_name"
ACCEPTED = "scope::tests::i4r3_10_a_recorded_start_reply_changes_only_what_absence_confirms"

ROOT = None
LOG = None
TARGET_DIR = None
EXPECTED_STATUS = set()

# The anchors, in the candidate.
TRAIT_GET_UNIT = "    fn get_unit(&self, unit: &str) -> Remote<Presence>;\n"
IMPL_GET_UNIT = "    fn get_unit(&self, unit: &str) -> Remote<Presence> {\n"
OBSERVE = ("        // Nothing is acted upon by the unit's name (P2-V1-R3B-I4-R3): what\n"
           "        // the operation may have created is ended only through its\n"
           "        // candidate, by descriptor, and confirmed only by observation.\n"
           "        self.observe(&controller, helper, fault)\n")
CANDIDATE_KILL = ("        // Cleanup ownership: everything in the candidate is ended.\n"
                  "        if let Some(candidate) = &self.candidate {\n"
                  "            let _ = candidate.kill();\n"
                  "        }\n")
ACQUIRE = ("    fn acquire(&mut self, controller: &Controller, helper: Option<&Helper>) {\n"
           "        if self.candidate.is_some() {\n"
           "            return;\n"
           "        }\n")
GONE_ACCEPTED = "            None => absent(controller, &self.unit, helper),\n"
UNCERTAIN_ARM = "            Started::Uncertain(reason) => Some(reason),\n"
COLLIDED = ("            fault::at(fault, FaultPoint::ScopeReconcile);\n"
            "            self.settled = outside(&controller, helper, &self.unit);\n"
            "            return self.settled;\n")
OBSERVE_DOC = "    /// Observe, within the settling bound, until nothing this operation may\n"
RECONCILED = ("            if !pending.reconcile(helper.as_ref(), fault) {\n"
              "                return false;\n"
              "            }\n")


def default_stop(cid):
    """The manager's StopUnit, restored as a default method of the trait."""
    return (TRAIT_GET_UNIT,
            f"    /// {cid}: StopUnit by the unit's name, restored.\n"
            "    fn stop_unit(&self, unit: &str) -> Remote<()> {\n"
            "        Remote::Uncertain(format!(\"StopUnit: {unit}\"))\n"
            "    }\n" + TRAIT_GET_UNIT)


def zbus_stop(cid):
    """The production manager's StopUnit request, restored (the base's)."""
    return (IMPL_GET_UNIT,
            f"    // {cid}: the production manager's StopUnit, restored.\n"
            "    fn stop_unit(&self, unit: &str) -> Remote<()> {\n"
            "        match self.call::<_, zbus::zvariant::OwnedObjectPath>(\n"
            "            SYSTEMD_PATH,\n"
            "            MANAGER,\n"
            "            \"StopUnit\",\n"
            "            &(unit, \"replace\"),\n"
            "        ) {\n"
            "            Ok(Ok(_job)) => Remote::Answered(()),\n"
            "            Ok(Err(name)) => Remote::Uncertain(format!(\"StopUnit: {name}\")),\n"
            "            Err(reason) => Remote::Uncertain(reason),\n"
            "        }\n"
            "    }\n\n" + IMPL_GET_UNIT)


def control(cid, item, what, edits, test_, marker):
    return dict(id=cid, category=f"r3-{item}", what=what,
                edits=[dict(file=f, anchor=a, replacement=r) for (f, a, r) in edits],
                test=test_, marker=marker)


CONTROLS = [
    control(
        "NC-R3-01-STOPUNIT-IN-RECONCILE", 1,
        "StopUnit by the unit's name reintroduced into settling (reconcile), for every operation not collided",
        [(MANAGER,) + default_stop("NC-R3-01"), (MANAGER,) + zbus_stop("NC-R3-01"),
         (PENDING, OBSERVE,
          "        // NC-R3-01: StopUnit by the unit's name, then what remains observed.\n"
          "        let _ = controller.manager.stop_unit(&self.unit);\n"
          "        self.observe(&controller, helper, fault)\n")],
        SETTLING, "settling acts on a unit through the manager",
    ),
    control(
        "NC-R3-02-MANAGER-STOP-RESTORED", 2,
        "the manager's StopUnit restored (the trait method and the production request), unused",
        [(MANAGER,) + default_stop("NC-R3-02"), (MANAGER,) + zbus_stop("NC-R3-02")],
        SURFACE, "the manager can act on a unit by its name",
    ),
    control(
        "NC-R3-03-ACCEPTED-AUTHORIZES-STOP", 3,
        "the recorded start reply authorizes a stop by the unit's name again (the R2 shape)",
        [(MANAGER,) + default_stop("NC-R3-03"),
         (PENDING, OBSERVE,
          "        // NC-R3-03: the recorded start reply authorizes a stop by name.\n"
          "        if self.stop_authorized() {\n"
          "            let _ = controller.manager.stop_unit(&self.unit);\n"
          "        }\n"
          "        self.observe(&controller, helper, fault)\n"),
         (PENDING, OBSERVE_DOC,
          "    /// NC-R3-03: the name authority, restored.\n"
          "    fn stop_authorized(&self) -> bool {\n"
          "        self.accepted\n"
          "    }\n\n" + OBSERVE_DOC)],
        ACCEPTED, "the recorded start reply gates more than what the manager's absence can confirm",
    ),
    control(
        "NC-R3-04-PRESENCE-AS-OWNERSHIP", 4,
        "a unit the manager reports loaded under the name is taken as the accepted operation's: "
        "its control group is retained as the candidate and ended",
        [(PENDING, ACQUIRE, ACQUIRE +
          "        // NC-R3-04: presence taken as ownership.\n"
          "        if self.accepted {\n"
          "            if let Remote::Answered(Presence::Present(unit_path)) =\n"
          "                controller.manager.get_unit(&self.unit)\n"
          "            {\n"
          "                if let Remote::Answered(Some(group)) = controller.manager.control_group(&unit_path) {\n"
          "                    if let Ok(candidate) = controller.native.open(&group) {\n"
          "                        let _ = candidate.kill();\n"
          "                        self.candidate = Some(candidate);\n"
          "                        return;\n"
          "                    }\n"
          "                }\n"
          "            }\n"
          "        }\n")],
        R3_01, "an accepted operation was confirmed while a unit of its name is loaded",
    ),
    control(
        "NC-R3-05-OBJECT-PATH-IDENTITY", 5,
        "the object path GetUnit returns is taken as the unit's identity, and the unit at it "
        "ended through its control group",
        [(PENDING, OBSERVE,
          "        // NC-R3-05: the object path GetUnit returns taken as the unit's\n"
          "        // identity, and the unit at it ended.\n"
          "        if self.accepted && self.candidate.is_none() {\n"
          "            if let Remote::Answered(Presence::Present(unit_path)) =\n"
          "                controller.manager.get_unit(&self.unit)\n"
          "            {\n"
          "                if let Remote::Answered(Some(group)) = controller.manager.control_group(&unit_path) {\n"
          "                    if let Ok(unit) = controller.native.open(&group) {\n"
          "                        let _ = unit.kill();\n"
          "                    }\n"
          "                }\n"
          "            }\n"
          "        }\n"
          "        self.observe(&controller, helper, fault)\n")],
        R3_01, "the foreign unit loaded under the name was claimed",
    ),
    control(
        "NC-R3-06-FOREIGN-PRESENCE-CONFIRMS", 6,
        "an accepted operation without a candidate is confirmed while a unit is loaded under its "
        "name (a loaded unit that does not hold the helper taken as another's, this one as gone)",
        [(PENDING, GONE_ACCEPTED,
          "            // NC-R3-06: a loaded unit that does not hold the helper taken\n"
          "            // as another's, and this operation as gone.\n"
          "            None => {\n"
          "                absent(controller, &self.unit, helper)\n"
          "                    || (matches!(\n"
          "                        controller.manager.get_unit(&self.unit),\n"
          "                        Remote::Answered(Presence::Present(_))\n"
          "                    ) && outside(controller, helper, &self.unit))\n"
          "            }\n")],
        R3_01, "an accepted operation was confirmed while a unit of its name is loaded",
    ),
    control(
        "NC-R3-07-REAPED-WHILE-PRESENT", 7,
        "the helper of an accepted operation without a candidate is reaped while the manager "
        "reports its unit loaded (reaped whenever settling stays unconfirmed)",
        [(EXEC, RECONCILED,
          "            if !pending.reconcile(helper.as_ref(), fault) {\n"
          "                // NC-R3-07: the helper reaped while the operation is unresolved.\n"
          "                if let Some(child) = helper.as_mut() {\n"
          "                    let _ = reap_within(child, FINALIZE_TIMEOUT);\n"
          "                }\n"
          "                return false;\n"
          "            }\n")],
        R3_02, "the helper of an accepted operation was reaped while its unit is loaded",
    ),
    control(
        "NC-R3-08-UNCERTAIN-WEAKENED", 8,
        "an uncertain start without a candidate is recorded as accepted, so the manager's "
        "absence settles it",
        [(PENDING, UNCERTAIN_ARM,
          "            // NC-R3-08: an uncertain start recorded as accepted.\n"
          "            Started::Uncertain(reason) => {\n"
          "                self.accepted = true;\n"
          "                Some(reason)\n"
          "            }\n")],
        R3_06, "an uncertain start was resolved by its unit name or presence",
    ),
    control(
        "NC-R3-09-CANDIDATE-BY-NAME", 9,
        "the candidate is ended through the unit's cgroup path, rebuilt from its name beside the "
        "helper's cgroup, instead of through its retained descriptor",
        [(PENDING, CANDIDATE_KILL,
          "        // NC-R3-09: the candidate ended through the unit's path, rebuilt\n"
          "        // from its name, not through its descriptor.\n"
          "        if self.candidate.is_some() {\n"
          "            if let Some(Ok(Some(membership))) =\n"
          "                helper.map(|helper| controller.native.membership(helper))\n"
          "            {\n"
          "                if let Some((parent, _)) = membership.rsplit_once('/') {\n"
          "                    if let Ok(unit) = controller.native.open(&format!(\"{parent}/{}\", self.unit)) {\n"
          "                        let _ = unit.kill();\n"
          "                    }\n"
          "                }\n"
          "            }\n"
          "        }\n")],
        R3_04, "the candidate was not ended through its descriptor alone",
    ),
    control(
        "NC-R3-10-COLLISION-WEAKENED", 10,
        "a definite collision is confirmed at once, without observing the helper outside the "
        "foreign unit",
        [(PENDING, COLLIDED,
          "            fault::at(fault, FaultPoint::ScopeReconcile);\n"
          "            // NC-R3-10: a collision confirmed at once.\n"
          "            self.settled = true;\n"
          "            return self.settled;\n")],
        R3_07, "a collision was confirmed while the foreign unit holds the helper",
    ),
]


def sha256(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def state():
    return {path: sha256(ROOT / path) for path in FILES}


def status():
    out = subprocess.run(["git", "status", "--porcelain=v1", "--untracked-files=all"],
                         cwd=ROOT, capture_output=True, text=True, check=True).stdout
    return {line for line in out.splitlines() if line}


def mutate(control_, originals):
    texts = {}
    for edit in control_["edits"]:
        if edit["file"] not in MUTABLE:
            raise SystemExit(f"{control_['id']}: edits a file that is not mutable: {edit['file']}")
        text = texts.get(edit["file"], originals[edit["file"]].decode())
        count = text.count(edit["anchor"])
        if count != 1:
            raise SystemExit(f"{control_['id']}: anchor occurs {count} times in {edit['file']}: "
                             f"{edit['anchor'][:80]!r}")
        texts[edit["file"]] = text.replace(edit["anchor"], edit["replacement"], 1)
    return texts


def run_test(name):
    env = dict(os.environ, CARGO_TARGET_DIR=str(TARGET_DIR))
    return subprocess.run(
        ["cargo", "test", "-p", "nexus-verifier-sandbox", "--locked", "--lib", "--", name, "--exact"],
        cwd=ROOT, capture_output=True, text=True, timeout=3600, env=env)


def message_of(output, name):
    lines = output.splitlines()
    starts = [i for i, line in enumerate(lines)
              if line.startswith(f"thread '{name}'") and "panicked at" in line]
    if not starts:
        return ""
    tail = []
    for line in lines[starts[-1] + 1:]:
        if line.startswith("note: run with") or line.startswith("failures:") or line.startswith("thread '"):
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
    if status() != EXPECTED_STATUS:
        raise SystemExit(f"{control_['id']}: unexpected checkout state after restoring: {status()}")
    output = proc.stdout + proc.stderr
    name = control_["test"]
    compiled = ("could not compile" not in output and "error[E" not in output
                and "Running unittests src/lib.rs" in output)
    failed = (proc.returncode != 0 and "test result: FAILED. 0 passed; 1 failed" in output
              and f"{name} ... FAILED" in output)
    return output, compiled, failed, restored, message_of(output, name), proc.returncode


def baseline(original_state):
    results = {}
    for name in sorted({c["test"] for c in CONTROLS}):
        proc = run_test(name)
        output = proc.stdout + proc.stderr
        results[name] = proc.returncode == 0 and "test result: ok. 1 passed; 0 failed" in output
        (LOG / f"baseline--{name.replace('::', '__')}.log").write_text(output)
    if state() != original_state:
        raise SystemExit("files changed during the baseline")
    return results


def main():
    global ROOT, LOG, TARGET_DIR
    args = sys.argv[1:]
    if len(args) == 2 and args[1] == "--check-anchors":
        ROOT = pathlib.Path(args[0]).resolve()
        originals = {path: (ROOT / path).read_bytes() for path in FILES}
        ids = [c["id"] for c in CONTROLS]
        if len(ids) != len(set(ids)):
            raise SystemExit("duplicate control ids")
        for c in CONTROLS:
            mutate(c, originals)
        print(json.dumps({"controls": len(CONTROLS), "anchors": "ok"}))
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
    for c in CONTROLS:
        if only is not None and c["id"] not in only:
            continue
        output, compiled, failed, restored, message, code = run_one(c, originals, original_state)
        (LOG / f"{c['id']}.log").write_text(output)
        marked = c["marker"] in message
        ok = passing[c["test"]] and compiled and failed and marked and restored
        result = dict(id=c["id"], category=c["category"], what=c["what"],
                      files=sorted({e["file"] for e in c["edits"]}), test=c["test"],
                      marker=c["marker"], test_passes_unmutated=passing[c["test"]],
                      compiled=compiled, failed_intended_test=failed, marker_in_assertion=marked,
                      files_restored=restored, exit=code, ok=ok, assertion=message[:1500])
        results.append(result)
        print(json.dumps(result), flush=True)
    final_state = state()
    summary = dict(original=original_state, final=final_state, identical=final_state == original_state,
                   status=sorted(status()), expected_status=sorted(EXPECTED_STATUS),
                   counted=len(results),
                   by_category={k: sum(1 for r in results if r["category"] == k)
                                for k in sorted({r["category"] for r in results})},
                   baseline_all_pass=all(passing.values()),
                   all_required=all(r["ok"] for r in results),
                   failures=[r["id"] for r in results if not r["ok"]])
    print(json.dumps(summary), flush=True)
    (LOG / "summary.json").write_text(json.dumps(dict(summary=summary, baseline=passing, counted=results), indent=2) + "\n")
    return 0 if summary["identical"] and summary["all_required"] and summary["baseline_all_pass"] else 1


if __name__ == "__main__":
    sys.exit(main())

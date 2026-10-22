#!/usr/bin/env python3
"""P2-V1-R3B-I4-R2 negative controls: the name authority of a scope
operation (mission section 16). Each control restores one wrong behaviour in
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

The tests and the simulation they run over (src/scope/tests.rs,
src/execution/tests.rs) are never mutated; they are hashed to prove it.
Every test runs over the deterministic simulation of the user manager and
the kernel: no control contacts a bus or creates a cgroup.

Usage:
  r2_controls.py <checkout> <log directory> --target-dir <empty dir> [--only ID,...]
  r2_controls.py <checkout> --check-anchors

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
FILES = [
    PENDING,
    EXEC,
    f"{CRATE}/src/scope.rs",
    f"{CRATE}/src/scope/manager.rs",
    f"{CRATE}/src/scope/native.rs",
    f"{CRATE}/src/scope/tests.rs",
    f"{CRATE}/src/execution/tests.rs",
    f"{CRATE}/src/fault.rs",
]
MUTABLE = {PENDING, EXEC}


def test(name):
    return f"execution::tests::{name}"


R2_01 = test("i4r2_01_a_lost_collision_reply_never_stops_the_foreign_unit")
R2_02 = test("i4r2_02_an_uncertain_start_that_did_nothing_is_never_stopped_by_name")
R2_04 = test("i4r2_04_an_uncertain_start_s_candidate_is_ended_by_descriptor_never_by_name")
R2_05 = test("i4r2_05_only_a_recorded_start_reply_authorizes_a_stop_by_name")
GUARD = "scope::tests::i4r2_x_only_a_recorded_start_reply_authorizes_a_stop_by_name"

ROOT = None
LOG = None
TARGET_DIR = None
EXPECTED_STATUS = set()

CHECK = ("        if !self.stop_authorized() {\n"
         "            return self.observe(&controller, helper, fault);\n"
         "        }\n")
STOP_CALL = ("        self.last_stop = Some(controller.manager.stop_unit(&self.unit));\n"
             "        fault::at(fault, FaultPoint::AfterScopeStop);\n")
AUTHORITY = ("    fn stop_authorized(&self) -> bool {\n"
             "        self.accepted\n"
             "    }\n")


def control(cid, category, what, edits, test_, marker):
    return dict(id=cid, category=category, what=what,
                edits=[dict(file=f, anchor=a, replacement=r) for (f, a, r) in edits],
                test=test_, marker=marker)


CONTROLS = [
    control(
        "NC-R2-01-UNCONDITIONAL-STOP", "required",
        "the unconditional StopUnit by the unit's name is restored (the name authority check removed)",
        [(PENDING, CHECK, "        // NC-R2-01-UNCONDITIONAL-STOP: no name authority check.\n")],
        R2_01, "the foreign unit was stopped by its name",
    ),
    control(
        "NC-R2-02-STOP-FOR-UNACCEPTED", "required",
        "StopUnit by name is allowed for an operation that is not accepted",
        [(PENDING, AUTHORITY, "    fn stop_authorized(&self) -> bool {\n"
                              "        // NC-R2-02-STOP-FOR-UNACCEPTED: any operation not collided.\n"
                              "        self.accepted || !self.collided\n"
                              "    }\n")],
        R2_02, "an uncertain start was stopped by its name",
    ),
    control(
        "NC-R2-03-LOST-COLLISION-CONVERTED", "required",
        "an uncertain start is converted into a collision by a later GetUnit presence of its name",
        [(PENDING, CHECK,
          "        // NC-R2-03-LOST-COLLISION-CONVERTED: the name's presence makes it a collision.\n"
          "        if !self.accepted\n"
          "            && self.candidate.is_none()\n"
          "            && matches!(\n"
          "                controller.manager.get_unit(&self.unit),\n"
          "                Remote::Answered(Presence::Present(_))\n"
          "            )\n"
          "        {\n"
          "            self.collided = true;\n"
          "            self.settled = outside(&controller, helper, &self.unit);\n"
          "            return self.settled;\n"
          "        }\n" + CHECK)],
        R2_01, "an uncertain start was resolved by its unit name or presence",
    ),
    control(
        "NC-R2-04-REAPED-WHILE-UNCERTAIN", "required",
        "the helper is reaped while its uncertain operation without a candidate is unresolved",
        [(EXEC, "            if !pending.reconcile(helper.as_ref(), fault) {\n"
                "                return false;\n"
                "            }\n",
          "            if !pending.reconcile(helper.as_ref(), fault) {\n"
          "                // NC-R2-04-REAPED-WHILE-UNCERTAIN: reaped while unresolved.\n"
          "                if let Some(child) = helper.take() {\n"
          "                    let _ = child.reap();\n"
          "                }\n"
          "                return false;\n"
          "            }\n")],
        R2_02, "the helper of an unresolved uncertain start was reaped",
    ),
    control(
        "NC-R2-05-NO-SUCH-UNIT-SETTLES", "required",
        "GetUnit NoSuchUnit (with the helper outside) settles an uncertain start without a candidate",
        [(PENDING, "            None if !self.accepted => false,\n",
          "            // NC-R2-05-NO-SUCH-UNIT-SETTLES: the manager's absence settles it.\n"
          "            None if !self.accepted => absent(controller, &self.unit, helper),\n")],
        R2_02, "an uncertain start was resolved by its unit name or presence",
    ),
    control(
        "NC-R2-06-STOP-REPLY-CONFIRMS", "required",
        "a delivered StopUnit reply is taken as the operation's confirmation",
        [(PENDING, STOP_CALL,
          "        self.last_stop = Some(controller.manager.stop_unit(&self.unit));\n"
          "        // NC-R2-06-STOP-REPLY-CONFIRMS: a delivered reply confirms.\n"
          "        if self.last_stop == Some(Remote::Answered(())) {\n"
          "            self.settled = true;\n"
          "            return true;\n"
          "        }\n"
          "        fault::at(fault, FaultPoint::AfterScopeStop);\n")],
        R2_05, "a StopUnit reply confirmed the operation",
    ),
    control(
        "NC-R2-07-CANDIDATE-NAME-AUTHORITY", "required",
        "an uncertain operation's candidate path grants a stop by the unit's name",
        [(PENDING, CHECK,
          "        // NC-R2-07-CANDIDATE-NAME-AUTHORITY: a candidate grants the name.\n"
          "        if !self.stop_authorized() && self.candidate.is_none() {\n"
          "            return self.observe(&controller, helper, fault);\n"
          "        }\n")],
        R2_04, "an uncertain start's candidate was stopped by its name",
    ),
    control(
        "NC-R2-08-ACCEPTED-BY-PRESENCE", "required",
        "the accepted state is forged from the manager's presence of the name, not the start reply",
        [(PENDING, CHECK,
          "        // NC-R2-08-ACCEPTED-BY-PRESENCE: presence taken as acceptance.\n"
          "        if matches!(\n"
          "            controller.manager.get_unit(&self.unit),\n"
          "            Remote::Answered(Presence::Present(_))\n"
          "        ) {\n"
          "            self.accepted = true;\n"
          "        }\n" + CHECK)],
        R2_01, "the foreign unit was stopped by its name",
    ),
    control(
        "NC-R2-X-STOP-OUTSIDE-SETTLING", "additional",
        "a stop by the unit's name outside settling (the drop backstop)",
        [(PENDING, "    pub(crate) fn end_now(&self) {\n        if self.unresolved() {\n",
          "    pub(crate) fn end_now(&self) {\n        if self.unresolved() {\n"
          "            // NC-R2-X-STOP-OUTSIDE-SETTLING: a stop by name on drop.\n"
          "            let _ = self.controller.manager.stop_unit(&self.unit);\n")],
        GUARD, "a stop by name outside settling",
    ),
    control(
        "NC-R2-X-ACCEPTED-ELSEWHERE", "additional",
        "the accepted state is set where a candidate is retained, not in the start reply's arm",
        [(PENDING, "                    let _ = candidate.kill();\n"
                   "                    self.candidate = Some(candidate);\n",
          "                    let _ = candidate.kill();\n"
          "                    self.candidate = Some(candidate);\n"
          "                    // NC-R2-X-ACCEPTED-ELSEWHERE: a candidate taken as acceptance.\n"
          "                    self.accepted = true;\n")],
        GUARD, "a start is accepted other than in the delivered reply's arm",
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

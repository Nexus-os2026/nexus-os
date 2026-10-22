#!/usr/bin/env python3
"""P2-V1-R3B-I4-R3-R1 negative controls: candidate cleanup ownership
(mission section 19). Each control restores one wrong behaviour in the
production sources and must:

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

The infrastructure (from `def sha256` on) is P2-V1-R3B-I4-R3's
r3_controls.py, unchanged; only the control definitions, the intended
tests and the mutable files (the pending operation and the manager) are
this mission's.

The tests and the simulation they run over (src/scope/tests.rs,
src/execution/tests.rs) are never mutated; they are hashed to prove it.
Every test runs over the deterministic simulation of the user manager and
the kernel: no control contacts a bus or creates a cgroup.

Usage:
  r3r1_controls.py <checkout> <log directory> --target-dir <empty dir> [--only ID,...]
  r3r1_controls.py <checkout> --check-anchors

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
MUTABLE = {PENDING, MANAGER}


def test(name):
    return f"execution::tests::{name}"


R3R1_01 = test("r3r1_01_a_lost_collision_holding_the_helper_is_never_ended_proven_or_launched")
R3R1_02 = test("r3r1_02_a_same_name_replacement_holding_the_helper_is_never_ended_proven_or_launched")
R3R1_04 = test("r3r1_04_an_identity_that_is_not_the_captured_one_is_never_bound")
R3R1_06 = test("r3r1_06_an_owned_candidate_whose_policy_fails_is_ended_never_launched")
R3R1_07 = test("r3r1_07_an_uncertain_start_that_created_its_scope_is_never_ended_proven_or_launched")
R3R1_09 = test("r3r1_09_no_panic_grants_cleanup_or_launch_authority_early")
DECODING = "scope::tests::r3r1_05_the_identity_decoding_and_the_capture_rule_fail_closed"

ROOT = None
LOG = None
TARGET_DIR = None
EXPECTED_STATUS = set()

# The anchors, in the candidate.
ACQUIRE_HEAD = ("    fn acquire(&mut self, controller: &Controller, helper: Option<&Helper>) {\n"
                "        if self.candidate.is_some() || self.instance.is_none() {\n"
                "            return;\n"
                "        }\n")
ACQUIRE_GUARD = "        if self.candidate.is_some() || self.instance.is_none() {\n"
ACQUIRE_OWNED = ("                    self.candidate = Some(Candidate {\n"
                 "                        dir,\n"
                 "                        path,\n"
                 "                        owned: false,\n"
                 "                    });\n")
PROVE_OPEN = ("        let candidate = &*self.candidate.insert(Candidate {\n"
              "            dir,\n"
              "            path,\n"
              "            owned: false,\n"
              "        });\n")
PROVE_INSTANCE = ("        let Some(instance) = self.instance else {\n"
                  "            return Err(ScopeError::Mismatch(\"unit instance\"));\n"
                  "        };\n"
                  "        let path = wait_for_placement(controller, helper, &self.unit)?;\n")
UNCERTAIN_ARM = "            Started::Uncertain(reason) => return Err(ScopeError::Bus(reason)),\n"
ACCEPTED_INSTANCE = "                self.instance = Some(captured.map_err(ScopeError::Bus)?);\n"
SAME_INSTANCE = ("        Remote::Answered(Some(current)) if current == instance => Ok(()),\n"
                 "        Remote::Answered(_) => Err(ScopeError::Mismatch(\"unit instance\")),\n")
BIND_OWNS = ("        same_instance(controller, &unit_path, instance)?;\n"
             "        candidate.owned = true;\n"
             "        Ok(unit_path)\n")
POLICY_ON_OWNED = ("        let Some(owned) = self.owned_dir() else {\n"
                   "            return Err(ScopeError::Mismatch(\"scope not proven\"));\n"
                   "        };\n")
PROVE_END = ("        // Those answers were the captured invocation's.\n"
             "        same_instance(controller, &unit_path, instance)\n"
             "    }\n")
FROM_PROPERTY = ("        let id: [u8; 16] = bytes.try_into().ok()?;\n"
                 "        (id != [0; 16]).then_some(Self(id))\n")


def later_identity(cid, otherwise):
    """An identity read later by the unit's name: GetUnit, then the unit's
    current InvocationID (no binding to the start)."""
    return (f"                // {cid}: the identity read later, by the unit's name.\n"
            "                let Remote::Answered(Presence::Present(unit_path)) =\n"
            "                    controller.manager.get_unit(&self.unit)\n"
            "                else {\n"
            f"                    return Err({otherwise});\n"
            "                };\n"
            "                let Remote::Answered(Some(current)) =\n"
            "                    controller.manager.unit_instance(&unit_path)\n"
            "                else {\n"
            f"                    return Err({otherwise});\n"
            "                };\n"
            "                self.instance = Some(current);\n")


def control(cid, item, what, edits, test_, marker):
    return dict(id=cid, category=f"r3r1-{item}", what=what,
                edits=[dict(file=f, anchor=a, replacement=r) for (f, a, r) in edits],
                test=test_, marker=marker)


CONTROLS = [
    control(
        "NC-R3R1-01-MEMBERSHIP-OWNS", 1,
        "the helper's membership alone grants cleanup ownership: the cgroup settling retains "
        "through it is owned at once",
        [(PENDING, ACQUIRE_OWNED, ACQUIRE_OWNED.replace(
            "                        owned: false,\n",
            "                        // NC-R3R1-01: owned by the helper's membership alone.\n"
            "                        owned: true,\n"))],
        R3R1_02, "the replacement's cgroup was owned through the helper's membership",
    ),
    control(
        "NC-R3R1-02-KILL-AT-OPEN", 2,
        "the candidate is ended as soon as it is opened, before any binding",
        [(PENDING, PROVE_OPEN, PROVE_OPEN +
          "        // NC-R3R1-02: ended as soon as it is opened.\n"
          "        let _ = candidate.dir.kill();\n")],
        R3R1_04, "a cgroup not bound to the captured identity was ended",
    ),
    control(
        "NC-R3R1-03-LOST-COLLISION-OWNS", 3,
        "an uncertain start (a lost UnitExists) retains the cgroup holding its helper and owns it: "
        "helper placement becomes cleanup authority",
        [(PENDING, ACQUIRE_GUARD,
          "        // NC-R3R1-03: retained without an identity.\n"
          "        if self.candidate.is_some() {\n"),
         (PENDING, ACQUIRE_OWNED, ACQUIRE_OWNED.replace(
             "                        owned: false,\n",
             "                        // NC-R3R1-03: an uncertain start owns what holds its helper.\n"
             "                        owned: !self.accepted,\n"))],
        R3R1_01, "the foreign unit was killed",
    ),
    control(
        "NC-R3R1-04-LOST-COLLISION-PROVEN", 4,
        "an uncertain start (a lost UnitExists) goes on to the proof, which takes the identity the "
        "manager reports later: matching manager properties make the foreign cgroup proven",
        [(PENDING, UNCERTAIN_ARM,
          "            // NC-R3R1-04: an uncertain start goes on to the proof.\n"
          "            Started::Uncertain(_) => {}\n"),
         (PENDING, PROVE_INSTANCE,
          "        // NC-R3R1-04: without a captured identity, the unit's current one.\n"
          "        if self.instance.is_none() {\n"
          "            if let Remote::Answered(Presence::Present(unit_path)) =\n"
          "                controller.manager.get_unit(&self.unit)\n"
          "            {\n"
          "                if let Remote::Answered(Some(current)) =\n"
          "                    controller.manager.unit_instance(&unit_path)\n"
          "                {\n"
          "                    self.instance = Some(current);\n"
          "                }\n"
          "            }\n"
          "        }\n" + PROVE_INSTANCE)],
        R3R1_01, "the foreign cgroup was launched into",
    ),
    control(
        "NC-R3R1-05-IDENTITY-MISMATCH-IGNORED", 5,
        "the manager's identity for the unit is not compared: a different identity binds",
        [(PENDING, SAME_INSTANCE,
          "        // NC-R3R1-05: another identity accepted.\n"
          "        Remote::Answered(_) => Ok(()),\n")],
        R3R1_04, "launched into an unbound cgroup",
    ),
    control(
        "NC-R3R1-06-LATER-UNBOUND-IDENTITY", 6,
        "the accepted start's identity is read later by the unit's name instead of captured from "
        "the start's own job",
        [(PENDING, ACCEPTED_INSTANCE,
          "                let _ = captured;\n" +
          later_identity("NC-R3R1-06", "ScopeError::Mismatch(\"unit instance\")"))],
        R3R1_02, "the replacement was launched into",
    ),
    control(
        "NC-R3R1-07-MALFORMED-IDENTITY-ACCEPTED", 7,
        "an identity of another length, or all zero, is accepted from the manager's value",
        [(MANAGER, FROM_PROPERTY,
          "        // NC-R3R1-07: any length, and the null identity, accepted.\n"
          "        let mut id = [0u8; 16];\n"
          "        for (slot, byte) in id.iter_mut().zip(bytes) {\n"
          "            *slot = *byte;\n"
          "        }\n"
          "        Some(Self(id))\n")],
        DECODING, "a wrong, zero or malformed identity was accepted",
    ),
    control(
        "NC-R3R1-08-UNCERTAIN-GRANTED-IDENTITY", 8,
        "an uncertain start is granted an identity (the unit's current one, read by its name)",
        [(PENDING, UNCERTAIN_ARM,
          "            Started::Uncertain(reason) => {\n" +
          later_identity("NC-R3R1-08", "ScopeError::Bus(reason)") +
          "            }\n")],
        R3R1_07, "an uncertain start was launched into",
    ),
    control(
        "NC-R3R1-09-ONE-FLAG", 9,
        "cleanup ownership and scope authority collapsed into one flag: the candidate is owned "
        "only once every policy is proven",
        [(PENDING, BIND_OWNS,
          "        same_instance(controller, &unit_path, instance)?;\n"
          "        // NC-R3R1-09: no ownership before the whole proof.\n"
          "        Ok(unit_path)\n"),
         (PENDING, POLICY_ON_OWNED,
          "        // NC-R3R1-09: the policy proven on the unowned candidate.\n"
          "        let Some(owned) = self.candidate.as_ref().map(|candidate| candidate.dir.as_ref())\n"
          "        else {\n"
          "            return Err(ScopeError::Mismatch(\"scope not proven\"));\n"
          "        };\n"),
         (PENDING, PROVE_END,
          "        // Those answers were the captured invocation's.\n"
          "        same_instance(controller, &unit_path, instance)?;\n"
          "        // NC-R3R1-09: owned only with the scope authority.\n"
          "        if let Some(candidate) = self.candidate.as_mut() {\n"
          "            candidate.owned = true;\n"
          "        }\n"
          "        Ok(())\n"
          "    }\n")],
        R3R1_06, "the owned candidate was not ended through its descriptor",
    ),
    control(
        "NC-R3R1-10-OWNED-BEFORE-PROOF", 10,
        "the candidate the proof retains is owned as it is opened, before its binding: a panic "
        "there leaves it owned",
        [(PENDING, PROVE_OPEN, PROVE_OPEN.replace(
            "            owned: false,\n",
            "            // NC-R3R1-10: owned before the binding.\n"
            "            owned: true,\n"))],
        R3R1_09, "a cgroup was ended before it was bound",
    ),
    control(
        "NC-R3R1-11-RECONSTRUCTED-FROM-PATH", 11,
        "a retained candidate whose directory was removed is opened again from its path, keeping "
        "its ownership",
        [(PENDING, ACQUIRE_HEAD,
          "    fn acquire(&mut self, controller: &Controller, helper: Option<&Helper>) {\n"
          "        // NC-R3R1-11: a removed candidate reconstructed from its path.\n"
          "        if let Some(candidate) = self.candidate.as_mut() {\n"
          "            if matches!(\n"
          "                occupancy(candidate.dir.as_ref()),\n"
          "                Ok(Occupancy::Removed)\n"
          "            ) {\n"
          "                if let Ok(dir) = controller.native.open(&candidate.path) {\n"
          "                    candidate.dir = dir;\n"
          "                }\n"
          "            }\n"
          "        }\n"
          "        if self.candidate.is_some() || self.instance.is_none() {\n"
          "            return;\n"
          "        }\n")],
        R3R1_02, "a removed owned candidate was not confirmed through its descriptor",
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

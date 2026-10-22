#!/usr/bin/env python3
"""P2-V1-R3B-I4-Q1-R1 negative controls for the collision qualification's
ownership (mission section 10) and the H1 environment review (section 11).
Each control restores one wrong state in a file the repaired qualification
depends on and must:

- apply: every edit's anchor occurs exactly once in its file, in order;
- compile (where the intended test is a Rust test): no "error[E", no
  "could not compile";
- run its one intended test alone (--exact) and fail it;
- fail the intended assertion: the message of the last panic on the test's
  own thread carries the control's marker;
- restore: every mutated file is written back byte for byte in `finally`;
  every file of FILES is verified by SHA-256 before each control and after
  each restoration, and the checkout's Git status must equal the expected
  status.

Two layers carry the qualification's ownership, and each defect pattern of
the Architect's finding is restored in both where it can occur:

- the first start's owner, `execution::place` (production code the live
  harness's H6 now delegates to), exercised over the deterministic model by
  the sandbox crate's unit tests `scope::tests::i4q1r1_nc1` to `nc4` and
  the source pin `scope::tests::i4q1r1_connect_...` (mutations of
  `scope/pending.rs`, `execution.rs`, `scope.rs`);
- the live harness itself (`tests/phase2_live_sandbox.rs`, `p2q`) and its
  probe (`tests/support/host_qualification.rs`), which run only on the
  supported host: the desktop guard `phase2_tests::p2_g_11` pins them.

Usage:
  r1_controls.py <checkout> <log directory> --target-dir <empty dir> [--only ID,...]
  r1_controls.py <checkout> --check-anchors

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
EXECUTION = f"{CRATE}/src/execution.rs"
SCOPE = f"{CRATE}/src/scope.rs"
HARNESS = f"{CRATE}/tests/phase2_live_sandbox.rs"
PROBE = f"{CRATE}/tests/support/host_qualification.rs"
FILES = [
    PENDING,
    EXECUTION,
    SCOPE,
    HARNESS,
    PROBE,
    f"{CRATE}/src/scope/tests.rs",
    f"{CRATE}/src/scope/manager.rs",
    f"{CRATE}/src/execution/tests.rs",
    f"{CRATE}/tests/support/cleanup_observation.rs",
    ".github/workflows/ci-phase2-linux-sandbox.yml",
    "app/src-tauri/src/phase2_tests.rs",
]
MUTABLE = {PENDING, EXECUTION, SCOPE, HARNESS, PROBE}


def unit(name):
    return ("sandbox", f"scope::tests::{name}")


NC1 = unit("i4q1r1_nc1_a_first_start_refused_as_loaded_ends_only_the_harness_s_helper")
NC2 = unit("i4q1r1_nc2_an_uncertain_first_start_keeps_its_helper_unreaped_and_unconfirmed")
NC3 = unit("i4q1r1_nc3_a_panic_after_the_first_start_is_dispatched_keeps_one_owner")
NC4 = unit("i4q1r1_nc4_a_proven_first_start_is_released_by_its_owner_and_observed")
CONNECT = unit("i4q1r1_connect_derives_the_bus_from_the_real_uid_alone")
GUARD = ("desktop", "phase2_tests::p2_g_11_the_host_qualification_owns_no_native_effect_by_a_unit_name")

ROOT = None
LOG = None
TARGET_DIR = None
EXPECTED_STATUS = set()


def control(cid, nc, layer, what, edits, test, marker):
    return dict(id=cid, nc=nc, layer=layer, what=what,
                edits=[dict(file=f, anchor=a, replacement=r) for (f, a, r) in edits],
                test=test, marker=marker)


COLLIDED = ("            fault::at(fault, FaultPoint::ScopeReconcile);\n"
            "            self.settled = outside(&controller, helper, &self.unit);\n")

CONTROLS = [
    # Q1-R1-NC1: a first start refused as loaded (UnitExists): the unit is
    # foreign.
    control(
        "Q1-R1-NC1-FOREIGN-STOPPED", "NC1", "first-start owner",
        "a first start refused as loaded has the foreign unit stopped by its name",
        [(PENDING, COLLIDED,
          "            fault::at(fault, FaultPoint::ScopeReconcile);\n"
          "            // Q1-R1-NC1-FOREIGN-STOPPED: the collided unit is stopped by its name.\n"
          "            let _ = controller.manager.stop_unit(&self.unit);\n"
          "            self.settled = outside(&controller, helper, &self.unit);\n")],
        NC1, "the foreign unit was stopped",
    ),
    control(
        "Q1-R1-NC1-FOREIGN-KILLED", "NC1", "first-start owner",
        "a first start refused as loaded has the foreign unit's cgroup taken and killed",
        [(PENDING, COLLIDED,
          "            fault::at(fault, FaultPoint::ScopeReconcile);\n"
          "            // Q1-R1-NC1-FOREIGN-KILLED: the collided unit's cgroup is taken and ended.\n"
          "            self.acquire(&controller, helper);\n"
          "            self.settled = outside(&controller, helper, &self.unit);\n")],
        NC1, "the foreign unit was killed",
    ),
    control(
        "Q1-R1-NC1-H6-UNPROVEN-UNIT", "NC1", "live harness",
        "H6 sends its request for a name it generated instead of its proven unit",
        [(HARNESS, "            let unit = placed.scope().unwrap().unit().to_string();\n"
                   "            let entered = hq::membership(pid)",
          "            let unit = hq::fresh_unit_name(\"q1h6-\");\n"
          "            let entered = hq::membership(pid)")],
        GUARD, "H6 acts on a unit it has not proven",
    ),
    # Q1-R1-NC2: an uncertain first start.
    control(
        "Q1-R1-NC2-UNRESOLVED-REAPED", "NC2", "first-start owner",
        "the helper of an unresolved first start is reaped",
        [(EXECUTION, "            if !pending.reconcile(helper.as_ref(), fault) {\n"
                     "                return false;\n"
                     "            }\n",
          "            if !pending.reconcile(helper.as_ref(), fault) {\n"
          "                // Q1-R1-NC2-UNRESOLVED-REAPED: the helper is reaped while its operation is unresolved.\n"
          "                if let Some(child) = helper.take() {\n"
          "                    let _ = child.reap();\n"
          "                }\n"
          "                return false;\n"
          "            }\n")],
        NC2, "the helper was reaped while unresolved",
    ),
    control(
        "Q1-R1-NC2-STOP-REPLY-PROOF", "NC2", "first-start owner",
        "a delivered StopUnit reply is taken as the uncertain first start's confirmation",
        [(PENDING, "        self.last_stop = Some(controller.manager.stop_unit(&self.unit));\n"
                   "        fault::at(fault, FaultPoint::AfterScopeStop);\n",
          "        self.last_stop = Some(controller.manager.stop_unit(&self.unit));\n"
          "        // Q1-R1-NC2-STOP-REPLY-PROOF: a delivered StopUnit reply settles the operation.\n"
          "        if self.last_stop == Some(Remote::Answered(())) {\n"
          "            self.settled = true;\n"
          "            return true;\n"
          "        }\n"
          "        fault::at(fault, FaultPoint::AfterScopeStop);\n")],
        NC2, "confirmed without proof",
    ),
    control(
        "Q1-R1-NC2-HARNESS-RAW-REAP", "NC2", "live harness",
        "the qualification ends and reaps a helper outside its owner",
        [(HARNESS, "membership_unchanged=yes\"),\n            );\n            drop(probe);\n",
          "membership_unchanged=yes\"),\n            );\n            drop(probe);\n"
          "            // Q1-R1-NC2-HARNESS-RAW-REAP: a helper ended outside its owner.\n"
          "            let (mut stray, _output) = Helper::spawn(&HelperProgram::at(HELPER)).unwrap();\n"
          "            let _ = stray.kill();\n"
          "            let _ = stray.reap();\n")],
        GUARD, "the qualification ends a helper outside its owner",
    ),
    # Q1-R1-NC3: a panic after the first start is dispatched.
    control(
        "Q1-R1-NC3-PANIC-ESCAPES", "NC3", "first-start owner",
        "a panic in the first start crosses its owner (no last owner)",
        [(EXECUTION, "    let error = match catch_unwind(AssertUnwindSafe(|| owned.establish(scopes, limits))) {\n",
          "    // Q1-R1-NC3-PANIC-ESCAPES: a panic in the first start crosses its owner.\n"
          "    let error = match Ok::<_, Box<dyn std::any::Any + Send>>(owned.establish(scopes, limits)) {\n")],
        NC3, "a panic crossed the owner of the first start",
    ),
    control(
        "Q1-R1-NC3-PANIC-SPLITS", "NC3", "first-start owner",
        "a panicked first start's helper is reaped and split from its operation",
        [(EXECUTION, "        Ok(Err(error)) => Some(error),\n"
                     "        Err(_) => None,\n"
                     "    };\n"
                     "    Err(PlacementFailed {\n",
          "        Ok(Err(error)) => Some(error),\n"
          "        Err(_) => {\n"
          "            // Q1-R1-NC3-PANIC-SPLITS: the panicked operation's helper is reaped and split from it.\n"
          "            if let Some(mut child) = owned.helper.take() {\n"
          "                let _ = child.kill();\n"
          "                let _ = child.reap();\n"
          "            }\n"
          "            None\n"
          "        }\n"
          "    };\n"
          "    Err(PlacementFailed {\n")],
        NC3, "the operation was split from its helper",
    ),
    control(
        "Q1-R1-NC3-HARNESS-OWN-OWNER", "NC3", "live harness",
        "the qualification has an owner of its own (old H6's Existing) beside the production owner",
        [(HARNESS, "        /// H6: `StartTransientUnit` for the name of a transient scope that is\n",
          "        // Q1-R1-NC3-HARNESS-OWN-OWNER: an owner of its own beside the production owner.\n"
          "        struct Existing {\n"
          "            unit: String,\n"
          "        }\n"
          "\n"
          "        impl Drop for Existing {\n"
          "            fn drop(&mut self) {\n"
          "                let _ = &self.unit;\n"
          "            }\n"
          "        }\n"
          "\n"
          "        /// H6: `StartTransientUnit` for the name of a transient scope that is\n")],
        GUARD, "the qualification owns a native effect beside the production owner",
    ),
    # Q1-R1-NC4: a definitely created (proven) first start.
    control(
        "Q1-R1-NC4-CLEANUP-UNOBSERVED", "NC4", "first-start owner",
        "the owner confirms a proven scope's cleanup without observing it empty",
        [(EXECUTION, "    if let Some(proven) = scope.proven() {\n"
                     "        if !proven.wait_empty(FINALIZE_TIMEOUT).unwrap_or(false) {\n"
                     "            return false;\n"
                     "        }\n"
                     "    }\n",
          "    // Q1-R1-NC4-CLEANUP-UNOBSERVED: the owner confirms without observing the scope empty.\n"
          "    let _ = scope.proven();\n")],
        NC4, "never observed empty",
    ),
    control(
        "Q1-R1-NC4-HARNESS-REAP-BEFORE-KILL", "NC4", "live harness",
        "released() reaps the helper before its proven scope is ended",
        [(HARNESS, "            placed.scope().unwrap().kill().unwrap();\n"
                   "            placed.reap_helper().unwrap();\n",
          "            // Q1-R1-NC4-HARNESS-REAP-BEFORE-KILL: the helper is reaped before its scope is ended.\n"
          "            placed.reap_helper().unwrap();\n"
          "            placed.scope().unwrap().kill().unwrap();\n")],
        GUARD, "released() reaps the helper before its scope is ended",
    ),
    # Q1-R1-NC5: a name never authorizes a stop.
    control(
        "Q1-R1-NC5-PROBE-STOPS-BY-NAME", "NC5", "live harness",
        "the probe regains a StopUnit by the unit's name",
        [(PROBE, "    /// `GetUnit(name)`: the unit's object path, or the manager's error.\n",
          "    /// Q1-R1-NC5-PROBE-STOPS-BY-NAME: a stop by the unit's name.\n"
          "    pub fn stop_unit(&self, unit: &str) -> Result<Answer<OwnedObjectPath>, ProbeError> {\n"
          "        self.call(SYSTEMD_PATH, MANAGER_INTERFACE, \"StopUnit\", &(unit, \"replace\"))\n"
          "    }\n"
          "\n"
          "    /// `GetUnit(name)`: the unit's object path, or the manager's error.\n")],
        GUARD, "the probe acts on a unit by its name",
    ),
    control(
        "Q1-R1-NC5-PROBE-START-CARRIES-A-PROCESS", "NC5", "live harness",
        "the probe's start request carries a process (PIDs)",
        [(PROBE, "        let properties: Vec<(&str, Value<'_>)> = Vec::new();\n",
          "        // Q1-R1-NC5-PROBE-START-CARRIES-A-PROCESS: the start names this process.\n"
          "        let properties: Vec<(&str, Value<'_>)> =\n"
          "            vec![(\"PIDs\", Value::from(vec![std::process::id()]))];\n")],
        GUARD, "the probe's start request carries a process or a property",
    ),
    # H1 (section 11): the environment.
    control(
        "Q1-R1-H1-CONNECT-ENV", "H1", "production source",
        "connect() follows an ambient session bus address",
        [(SCOPE, "        Self::connect_to(&format!(\"/run/user/{uid}/bus\"))\n    }\n",
          "        // Q1-R1-H1-CONNECT-ENV: an ambient session bus address is followed.\n"
          "        match std::env::var(\"DBUS_SESSION_BUS_ADDRESS\") {\n"
          "            Ok(address) => Self::connect_to(address.trim_start_matches(\"unix:path=\")),\n"
          "            Err(_) => Self::connect_to(&format!(\"/run/user/{uid}/bus\")),\n"
          "        }\n"
          "    }\n")],
        CONNECT, "reads the environment or names an ambient bus",
    ),
    control(
        "Q1-R1-H1-ENV-MUTATION", "H1", "live harness",
        "H1 changes the environment of the multi-threaded live harness again",
        [(HARNESS, "            // The production manager's own connection reaches it.\n"
                   "            let connected = ScopeManager::connect();\n",
          "            // Q1-R1-H1-ENV-MUTATION: the environment of the multi-threaded harness is changed.\n"
          "            std::env::set_var(\"DBUS_SESSION_BUS_ADDRESS\", \"unix:path=/nonexistent/nexus-q1/bus\");\n"
          "            // The production manager's own connection reaches it.\n"
          "            let connected = ScopeManager::connect();\n")],
        GUARD, "the qualification changes the process environment",
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


def run_test(test):
    kind, name = test
    env = dict(os.environ, CARGO_TARGET_DIR=str(TARGET_DIR))
    package = {"sandbox": "nexus-verifier-sandbox", "desktop": "nexus-desktop-backend"}[kind]
    return subprocess.run(
        ["cargo", "test", "-p", package, "--locked", "--lib", "--", name, "--exact"],
        cwd=ROOT, capture_output=True, text=True, timeout=3600, env=env)


def message_of(output, test):
    lines = output.splitlines()
    starts = [i for i, line in enumerate(lines)
              if line.startswith(f"thread '{test}'") and "panicked at" in line]
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
    _, name = control_["test"]
    compiled = "could not compile" not in output and "error[E" not in output and "Running unittests" in output
    failed = (proc.returncode != 0 and "test result: FAILED. 0 passed; 1 failed" in output
              and f"{name} ... FAILED" in output)
    return output, compiled, failed, restored, message_of(output, name), proc.returncode


def baseline(original_state):
    results = {}
    for test in sorted({c["test"] for c in CONTROLS}):
        proc = run_test(test)
        output = proc.stdout + proc.stderr
        results[test[1]] = proc.returncode == 0 and "test result: ok. 1 passed; 0 failed" in output
        (LOG / f"baseline--{test[1].replace('::', '__')}.log").write_text(output)
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
        ok = passing[c["test"][1]] and compiled and failed and marked and restored
        result = dict(id=c["id"], nc=c["nc"], layer=c["layer"], what=c["what"],
                      files=sorted({e["file"] for e in c["edits"]}), test=c["test"][1],
                      marker=c["marker"], test_passes_unmutated=passing[c["test"][1]],
                      compiled=compiled, failed_intended_test=failed, marker_in_assertion=marked,
                      files_restored=restored, exit=code, ok=ok, assertion=message[:1500])
        results.append(result)
        print(json.dumps(result), flush=True)
    final_state = state()
    summary = dict(original=original_state, final=final_state, identical=final_state == original_state,
                   status=sorted(status()), expected_status=sorted(EXPECTED_STATUS),
                   counted=len(results), baseline_all_pass=all(passing.values()),
                   all_required=all(r["ok"] for r in results),
                   failures=[r["id"] for r in results if not r["ok"]])
    print(json.dumps(summary), flush=True)
    (LOG / "summary.json").write_text(json.dumps(dict(summary=summary, baseline=passing, counted=results), indent=2) + "\n")
    return 0 if summary["identical"] and summary["all_required"] and summary["baseline_all_pass"] else 1


if __name__ == "__main__":
    sys.exit(main())

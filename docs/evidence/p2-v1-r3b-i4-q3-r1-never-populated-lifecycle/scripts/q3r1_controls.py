#!/usr/bin/env python3
"""P2-V1-R3B-I4-Q3-R1 negative controls: the never-populated scope lifecycle
(mission section 8). Each control restores one wrong behaviour in the model,
in case 18's qualification (`tests/support/never_populated.rs`) or in the
live case itself, and must:

- apply: every edit's anchor occurs exactly once in its file, in order;
- compile: the intended test's target builds (no "error[E", no "could not
  compile");
- run its one intended test alone (--exact) and fail it ("0 passed; 1
  failed", "<test> ... FAILED");
- fail the intended assertion: the message of the last panic on the test's
  own thread carries the control's marker;
- restore: the mutated files are written back byte for byte in `finally`;
  every file of FILES is verified by SHA-256 before each control and after
  each restoration, and the checkout's Git status must equal the expected
  status.

The infrastructure is P2-V1-R3B-I4-R3-R1's r3r1_controls.py (itself R3's),
unchanged but for the test target: a control names its target, the library's
unit tests or the `phase2_never_populated` fixture target, and is compiled
and run there. The live case is never run: its controls are detected by the
fixture target's structural check of its source. Production sources are
never mutated; they are hashed to prove it. No control contacts a bus or
creates a cgroup.

Usage:
  q3r1_controls.py <checkout> <log directory> --target-dir <empty dir> [--only ID,...]
  q3r1_controls.py <checkout> --check-anchors

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
MODEL = f"{CRATE}/src/scope/tests.rs"
SUPPORT = f"{CRATE}/tests/support/never_populated.rs"
HARNESS = f"{CRATE}/tests/phase2_live_sandbox.rs"
FILES = [
    MODEL,
    SUPPORT,
    HARNESS,
    f"{CRATE}/tests/phase2_never_populated.rs",
    f"{CRATE}/src/execution/tests.rs",
    f"{CRATE}/src/scope/pending.rs",
    f"{CRATE}/src/scope/manager.rs",
    f"{CRATE}/src/scope/native.rs",
    f"{CRATE}/src/scope.rs",
    f"{CRATE}/src/execution.rs",
    f"{CRATE}/src/launcher.rs",
]
MUTABLE = {MODEL, SUPPORT, HARNESS}

LIB = "lib"
FIXTURES = "phase2_never_populated"


def lib(name):
    return (LIB, f"execution::tests::{name}")


def fixture(name):
    return (FIXTURES, f"controls::{name}")


Q3R1_01 = lib("q3r1_01_a_never_populated_scope_stays_loaded_and_unconfirmed_before_its_backstop")
NP_03 = fixture("np_03_a_confirmation_before_the_backstop_is_a_failure")
NP_05 = fixture("np_05_a_proven_placement_is_a_failure")
NP_08 = fixture("np_08_a_helper_that_never_exits_fails_within_its_bound")
NP_09 = fixture("np_09_a_late_exit_is_waited_for_by_its_own_report")
NP_10 = fixture("np_10_a_scope_never_collected_fails_after_its_bound")
NP_11 = fixture("np_11_scopes_not_back_to_the_baseline_are_a_failure")
NP_15 = fixture("np_15_the_case_s_backstop_is_its_own_and_leaves_room_for_a_retry")
NP_16 = fixture("np_16_the_exit_observation_never_reaps")
NP_18 = fixture("np_18_live_case_18_drives_exactly_this_qualification")

ROOT = None
LOG = None
TARGET_DIR = None
EXPECTED_STATUS = set()

# The anchors, in the candidate.
NOTIFY_EMPTY = ("        if record.lifecycle != Lifecycle::Running || !cgroup.ever_populated || "
                "cgroup.populated() {\n")
COLLECT_ENDED = ("            .is_some_and(|record| matches!(record.lifecycle, Lifecycle::Ended(_)))\n")
WNOWAIT = "            libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,\n"
EARLY_CONFIRMATION = "                if at + plan.early_tolerance < plan.backstop || unconfirmed == 0 {\n"
PROVEN = ("        Ok(placed) => {\n"
          "            return Err(facts.fail(\n"
          "                world.now(),\n"
          "                format!(\"a scope was proven for an unmovable process: {placed:?}\"),\n"
          "            ))\n"
          "        }\n")
CASE_BACKSTOP = "        runtime_backstop_secs: BACKSTOP_SECS,\n"
EXIT_BOUND = ("            Ok(None) if world.now() >= deadline => {\n"
              "                return Err(facts.fail(\n"
              "                    world.now(),\n"
              "                    \"the killed helper did not exit within its bound\",\n"
              "                ))\n"
              "            }\n")
EXIT_LOOP = ("    let exit = loop {\n"
             "        match world.exit() {\n"
             "            Ok(Some(exit)) => break exit,\n")
COLLECTION_BOUND = ("            Ok(_) if at >= plan.backstop + plan.collected_within => {\n"
                    "                let failure = facts.fail(\n"
                    "                    world.now(),\n"
                    "                    \"the scope was not collected within its bound after the backstop\",\n"
                    "                );\n"
                    "                return Err(give_up(world, boundary, failure));\n"
                    "            }\n")
BASELINE = ("    // 8. The loaded scopes return to the baseline.\n"
            "    let deadline = world.now() + plan.observed_within;\n"
            "    loop {\n")
CASE_PLACE = ("                let Some(helper) = self.helper.take() else {\n")
CASE_PLACE_CALL = ("                match execution::place(self.scopes, helper, &self.limits) {\n")
CASE_LIMITS = "                limits: never_populated::limits(limits()),\n"

DEVIATES = "qualified a world that deviates from the lifecycle"


def control(cid, item, what, edits, test_, marker):
    target, name = test_
    return dict(id=cid, category=f"q3r1-{item}", what=what,
                edits=[dict(file=f, anchor=a, replacement=r) for (f, a, r) in edits],
                target=target, test=name, marker=marker)


CONTROLS = [
    control(
        "NC-Q3R1-01-MODEL-EMPTY-ENDS-NEVER-POPULATED", 1,
        "the model restores the generic assumption: a scope ends on a cgroup-empty notification "
        "merely because it is empty, though it was never populated",
        [(MODEL, NOTIFY_EMPTY,
          "        // NC-Q3R1-01: empty is enough, populated or not.\n"
          "        if record.lifecycle != Lifecycle::Running || cgroup.populated() {\n")],
        Q3R1_01, "a never-populated scope ended on a cgroup-empty notification",
    ),
    control(
        "NC-Q3R1-01B-MODEL-COLLECTS-RUNNING", 1,
        "the model's manager collects a unit that has not ended (immediate unload)",
        [(MODEL, COLLECT_ENDED,
          "            // NC-Q3R1-01B: any loaded unit is collected.\n"
          "            .is_some_and(|_record| true)\n")],
        Q3R1_01, "a running scope was collected",
    ),
    control(
        "NC-Q3R1-02-EXIT-OBSERVATION-REAPS", 2,
        "the helper's exit observation reaps it (WNOWAIT dropped): the helper is reaped before "
        "the operation is confirmed",
        [(SUPPORT, WNOWAIT,
          "            // NC-Q3R1-02: the observation consumes the exit.\n"
          "            libc::WEXITED | libc::WNOHANG,\n")],
        NP_16, "the exit observation reaped the helper",
    ),
    control(
        "NC-Q3R1-02B-CASE-REAPS-HELPER", 2,
        "the live case reaps the helper itself before its placement",
        [(HARNESS, CASE_PLACE,
          "                // NC-Q3R1-02B: the helper reaped before its placement.\n"
          "                let Some(mut helper) = self.helper.take() else {\n"),
         (HARNESS, CASE_PLACE_CALL,
          "                let _ = helper.try_reap();\n" + CASE_PLACE_CALL)],
        NP_18, "the case's world does try_reap",
    ),
    control(
        "NC-Q3R1-03-PRE-BACKSTOP-CONFIRMATION-ACCEPTED", 3,
        "a retry before the backstop that confirms the operation is taken as the lifecycle",
        [(SUPPORT, EARLY_CONFIRMATION,
          "                // NC-Q3R1-03: a confirmation before the backstop is accepted.\n"
          "                if false {\n")],
        NP_03, DEVIATES,
    ),
    control(
        "NC-Q3R1-04-PLACEMENT-SUCCESS-ACCEPTED", 4,
        "a proven placement of the unmovable process is accepted",
        [(SUPPORT, PROVEN,
          "        // NC-Q3R1-04: a proven placement is accepted.\n"
          "        Ok(_placed) => {\n"
          "            return Ok(Qualified {\n"
          "                unit: String::new(),\n"
          "                error: String::new(),\n"
          "                unconfirmed_before_backstop: 0,\n"
          "                collected_at: Duration::ZERO,\n"
          "                confirmed_on: 0,\n"
          "            })\n"
          "        }\n")],
        NP_05, DEVIATES,
    ),
    control(
        "NC-Q3R1-05-CASE-BACKSTOP-REMOVED", 5,
        "the case's own runtime backstop (RuntimeMaxUSec) is removed: the production default",
        [(SUPPORT, CASE_BACKSTOP,
          "        // NC-Q3R1-05: no backstop of the case's own.\n"
          "        runtime_backstop_secs: base.runtime_backstop_secs,\n")],
        NP_15, "the case does not set its own runtime backstop",
    ),
    control(
        "NC-Q3R1-05B-CASE-USES-HARNESS-LIMITS", 5,
        "the live case places with the harness's limits, without its own backstop",
        [(HARNESS, CASE_LIMITS,
          "                // NC-Q3R1-05B: the harness's limits.\n"
          "                limits: limits(),\n")],
        NP_18, "never_populated::limits(limits())",
    ),
    control(
        "NC-Q3R1-06-UNBOUNDED-EXIT-WAIT", 6,
        "the wait for the killed helper's exit has no bound",
        [(SUPPORT, EXIT_BOUND, "            // NC-Q3R1-06: no bound on the exit wait.\n")],
        NP_08, "an unbounded wait",
    ),
    control(
        "NC-Q3R1-06B-UNBOUNDED-COLLECTION-WAIT", 6,
        "the wait for the scope's collection has no bound",
        [(SUPPORT, COLLECTION_BOUND, "            // NC-Q3R1-06B: no bound on the collection wait.\n")],
        NP_10, "an unbounded wait",
    ),
    control(
        "NC-Q3R1-07-SLEEP-AS-EXIT-PROOF", 7,
        "a sleep stands in for the helper's exit report",
        [(SUPPORT, EXIT_LOOP,
          "    // NC-Q3R1-07: slept for the bound, then taken as exited.\n"
          "    world.pause(plan.exit_within);\n"
          "    let exit = Exit {\n"
          "        code: libc::CLD_KILLED,\n"
          "        status: libc::SIGKILL,\n"
          "    };\n"
          "    #[allow(unreachable_code)]\n"
          "    let _unused = loop {\n"
          "        break ();\n"
          "        match world.exit() {\n"
          "            Ok(Some(exit)) => break (),\n")],
        NP_09, "the exit was not waited for by its report",
    ),
    control(
        "NC-Q3R1-08-NO-BASELINE-CHECK", 8,
        "the loaded scopes' return to the baseline is not checked",
        [(SUPPORT, BASELINE,
          "    // NC-Q3R1-08: the baseline is not checked.\n"
          "    let deadline = world.now() + plan.observed_within;\n"
          "    #[allow(unreachable_code)]\n"
          "    loop {\n"
          "        break;\n")],
        NP_11, DEVIATES,
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


def run_test(target, name):
    env = dict(os.environ, CARGO_TARGET_DIR=str(TARGET_DIR))
    selector = ["--lib"] if target == LIB else ["--test", target]
    return subprocess.run(
        ["cargo", "test", "-p", "nexus-verifier-sandbox", "--locked", *selector, "--", name, "--exact"],
        cwd=ROOT, capture_output=True, text=True, timeout=3600, env=env)


def ran(target):
    return "Running unittests src/lib.rs" if target == LIB else f"Running tests/{target}.rs"


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
        proc = run_test(control_["target"], control_["test"])
    finally:
        for path in texts:
            (ROOT / path).write_bytes(originals[path])
    restored = state() == original_state
    if status() != EXPECTED_STATUS:
        raise SystemExit(f"{control_['id']}: unexpected checkout state after restoring: {status()}")
    output = proc.stdout + proc.stderr
    name = control_["test"]
    compiled = ("could not compile" not in output and "error[E" not in output
                and ran(control_["target"]) in output)
    failed = (proc.returncode != 0 and "test result: FAILED. 0 passed; 1 failed" in output
              and f"{name} ... FAILED" in output)
    return output, compiled, failed, restored, message_of(output, name), proc.returncode


def baseline(original_state):
    results = {}
    for target, name in sorted({(c["target"], c["test"]) for c in CONTROLS}):
        proc = run_test(target, name)
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
                      files=sorted({e["file"] for e in c["edits"]}), target=c["target"], test=c["test"],
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

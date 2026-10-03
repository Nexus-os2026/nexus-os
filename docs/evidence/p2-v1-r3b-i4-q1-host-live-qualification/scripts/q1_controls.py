#!/usr/bin/env python3
"""P2-V1-R3B-I4-Q1 negative controls for the host/live qualification boundary
(mission section 18). Each control restores one wrong state in a file the
qualification depends on and must:

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

Two intended tests carry these controls:

- `scope::tests::i4q1_the_manager_s_definite_answers_are_exactly_systemd_s_error_names`
  (the sandbox crate's unit tests): the production manager reads exactly
  systemd's `NoSuchUnit` and `UnitExists` as definite answers, in exactly
  one arm each (section 18, items 6 and 7);
- `phase2_tests::p2_g_10_the_live_gate_pins_every_case_and_the_host_qualification`
  (the desktop backend's guards): the live gate's pinned count is every
  case, every host-qualification case is in the suite, the step's fixture
  controls use the same count, the probe asserts the production identities
  and is harness-only, the unified hierarchy is a required layer and no step
  continues on error (items 10 and 12).

The code invariants of items 1-5, 8, 9 and 11 are carried by the accepted
I4-R1 controls and the cleanup-observation controls, rerun unchanged
(`accepted_reruns.sh`); the live cases themselves observe the real host
(`matrices/controls.md` maps every item).

Usage:
  q1_controls.py <checkout> <log directory> --target-dir <empty dir> [--only ID,...]
  q1_controls.py <checkout> --check-anchors

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
MANAGER = f"{CRATE}/src/scope/manager.rs"
HARNESS = f"{CRATE}/tests/phase2_live_sandbox.rs"
PROBE = f"{CRATE}/tests/support/host_qualification.rs"
WORKFLOW = ".github/workflows/ci-phase2-linux-sandbox.yml"
STEP_TEST = "scripts/ci/test_phase2_cleanup_check.py"
FILES = [
    MANAGER,
    HARNESS,
    PROBE,
    WORKFLOW,
    STEP_TEST,
    f"{CRATE}/src/scope/tests.rs",
    f"{CRATE}/src/scope/pending.rs",
    f"{CRATE}/src/scope.rs",
    f"{CRATE}/src/execution.rs",
    f"{CRATE}/tests/support/cleanup_observation.rs",
    "app/src-tauri/src/phase2_tests.rs",
]
MUTABLE = {MANAGER, HARNESS, PROBE, WORKFLOW, STEP_TEST}

UNIT_TEST = ("sandbox", "scope::tests::i4q1_the_manager_s_definite_answers_are_exactly_systemd_s_error_names")
GUARD = ("desktop", "phase2_tests::p2_g_10_the_live_gate_pins_every_case_and_the_host_qualification")

ROOT = None
LOG = None
TARGET_DIR = None
EXPECTED_STATUS = set()


def control(cid, item, what, edits, test, marker):
    return dict(id=cid, item=item, what=what,
                edits=[dict(file=f, anchor=a, replacement=r) for (f, a, r) in edits],
                test=test, marker=marker)


CONTROLS = [
    control(
        "NC-Q1-NO-SUCH-UNIT-NAME", 6,
        "the production manager reads another error name as the manager's absence",
        [(MANAGER, 'const NO_SUCH_UNIT: &str = "org.freedesktop.systemd1.NoSuchUnit";',
          'const NO_SUCH_UNIT: &str = "org.freedesktop.systemd1.NoSuchUnitLoaded";')],
        UNIT_TEST, 'const NO_SUCH_UNIT: &str = "org.freedesktop.systemd1.NoSuchUnit";',
    ),
    control(
        "NC-Q1-UNIT-EXISTS-NAME", 7,
        "the production manager reads another error name as a collision",
        [(MANAGER, 'const UNIT_EXISTS: &str = "org.freedesktop.systemd1.UnitExists";',
          'const UNIT_EXISTS: &str = "org.freedesktop.systemd1.UnitAlreadyExists";')],
        UNIT_TEST, 'const UNIT_EXISTS: &str = "org.freedesktop.systemd1.UnitExists";',
    ),
    control(
        "NC-Q1-ANY-ERROR-IS-ABSENT", 6,
        "every GetUnit error is read as the manager's absence",
        [(MANAGER, "            Ok(Err(name)) if name == NO_SUCH_UNIT => Remote::Answered(Presence::Absent),\n"
                   "            Ok(Err(name)) => Remote::Uncertain(format!(\"GetUnit: {name}\")),",
          "            // NC-Q1-ANY-ERROR-IS-ABSENT: any error is absence.\n"
          "            Ok(Err(_)) => Remote::Answered(Presence::Absent),")],
        UNIT_TEST, "Ok(Err(name)) if name == NO_SUCH_UNIT => Remote::Answered(Presence::Absent),",
    ),
    control(
        "NC-Q1-ANY-ERROR-IS-COLLISION", 7,
        "every StartTransientUnit error is read as a collision (no effect)",
        [(MANAGER, "            Ok(Err(name)) if name == UNIT_EXISTS => Started::Collision,\n"
                   "            Ok(Err(name)) => Started::Uncertain(format!(\"StartTransientUnit: {name}\")),",
          "            // NC-Q1-ANY-ERROR-IS-COLLISION: any error is a collision.\n"
          "            Ok(Err(_)) => Started::Collision,")],
        UNIT_TEST, "Ok(Err(name)) if name == UNIT_EXISTS => Started::Collision,",
    ),
    control(
        "NC-Q1-LIVE-COUNT-STALE", 12,
        "the gate's pinned count omits a case (38 of 39)",
        [(WORKFLOW, "grep -qx 'test result: ok. 39 live sandbox cases passed' phase2-live.log || passed=$?",
          "grep -qx 'test result: ok. 38 live sandbox cases passed' phase2-live.log || passed=$?")],
        GUARD, "the pinned count is every case",
    ),
    control(
        "NC-Q1-LIVE-CASE-DROPPED", 12,
        "a host-qualification case is dropped from the suite while the count is kept",
        [(HARNESS, "                let scoped: [ScopedCase; 29] = [", "                let scoped: [ScopedCase; 28] = ["),
         (HARNESS, "                    (\n"
                   "                        \"p2q_live_h5_a_fresh_name_is_exactly_no_such_unit\",\n"
                   "                        p2q::h5_no_such_unit,\n"
                   "                    ),\n", "")],
        GUARD, "the pinned count is every case",
    ),
    control(
        "NC-Q1-STEP-TEST-STALE", 12,
        "the live step's fixture controls still expect the old count",
        [(STEP_TEST, 'PASSED = "test result: ok. 39 live sandbox cases passed"',
          'PASSED = "test result: ok. 31 live sandbox cases passed"')],
        GUARD, "the step's fixture controls use the exact count",
    ),
    control(
        "NC-Q1-PROBE-IDENTITY-DRIFT", 6,
        "the host case asserts an error name other than the production manager's",
        [(PROBE, 'pub const NO_SUCH_UNIT: &str = "org.freedesktop.systemd1.NoSuchUnit";',
          'pub const NO_SUCH_UNIT: &str = "org.freedesktop.systemd1.NoSuchUnitLoaded";')],
        GUARD, "the probe asserts the production identity org.freedesktop.systemd1.NoSuchUnit",
    ),
    control(
        "NC-Q1-CGROUP2-LAYER-OPTIONAL", 10,
        "the host layers step no longer requires the unified hierarchy",
        [(WORKFLOW, '          [[ "$(stat -f -c %T /sys/fs/cgroup)" == "cgroup2fs" ]] || need "/sys/fs/cgroup is not the unified cgroup v2 hierarchy ($(stat -f -c %T /sys/fs/cgroup))"\n',
          '          echo "cgroup hierarchy: $(stat -f -c %T /sys/fs/cgroup)"\n')],
        GUARD, "the host layers step requires the unified hierarchy",
    ),
    control(
        "NC-Q1-CONTINUE-ON-ERROR", 12,
        "the live step continues on error",
        [(WORKFLOW, "      - name: Live Phase Two isolation, escape and cleanup suite (every layer required)\n        shell: bash\n",
          "      - name: Live Phase Two isolation, escape and cleanup suite (every layer required)\n        continue-on-error: true\n        shell: bash\n")],
        GUARD, "no step may continue on error",
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
        result = dict(id=c["id"], item=c["item"], what=c["what"],
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

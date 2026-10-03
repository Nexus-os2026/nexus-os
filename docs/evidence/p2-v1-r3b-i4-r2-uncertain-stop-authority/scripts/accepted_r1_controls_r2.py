#!/usr/bin/env python3
"""P2-V1-R3B-I4-R2: the accepted Q1-R1 mutation controls (15), rerun through
the accepted Q1-R1 runner itself, unchanged (its SHA-256 is verified before
it is imported), with exactly one documented adaptation:

- Q1-R1-NC2-STOP-REPLY-PROOF. Its mutation, a delivered StopUnit reply
  taken as confirmation, is applied unchanged at its unchanged anchor. Its
  intended test, i4q1r1_nc2, concerned an uncertain first start: R2 removes
  every stop by the unit's name from an uncertain operation, so that test
  can no longer reach the mutated line (it now asserts no stop at all). The
  same defect remains possible only for an accepted operation, so the
  control runs the R2 test of that path (i4r2_05) and that test's marker.

Every other control runs exactly as accepted. Usage: as r1_controls.py.
"""
import hashlib
import importlib.util
import json
import pathlib
import sys

sys.dont_write_bytecode = True

RUNNER = "docs/evidence/p2-v1-r3b-i4-q1-r1-host-qualification-ownership/scripts/r1_controls.py"
RUNNER_SHA256 = "eea344ea8394fbc6f0eb46c6b8db3b21d3f3bf728eb38b61ffdecd2c1a93a3c0"
ADAPTED = {
    "Q1-R1-NC2-STOP-REPLY-PROOF": dict(
        test=("sandbox", "execution::tests::i4r2_05_only_a_recorded_start_reply_authorizes_a_stop_by_name"),
        marker="a StopUnit reply confirmed the operation",
        reason="R2 removes every stop by name from an uncertain operation, so the accepted test "
               "(i4q1r1_nc2, an uncertain first start) can no longer reach the mutated line; the "
               "same mutation, at the same anchor, now runs the R2 test of the only path that "
               "still stops by name (an accepted operation without a candidate) with its marker.",
    ),
}


def main():
    if len(sys.argv) < 3:
        raise SystemExit(__doc__)
    root = pathlib.Path(sys.argv[1]).resolve()
    runner = root / RUNNER
    digest = hashlib.sha256(runner.read_bytes()).hexdigest()
    if digest != RUNNER_SHA256:
        raise SystemExit(f"the accepted Q1-R1 runner changed: {digest}")
    spec = importlib.util.spec_from_file_location("q1r1_controls", runner)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    adapted = []
    for control in module.CONTROLS:
        change = ADAPTED.get(control["id"])
        if change is None:
            continue
        control["what"] = (f"{control['what']} [R2-adapted: accepted test {control['test'][1]}, "
                           f"marker {control['marker']!r}; {change['reason']}]")
        control["test"] = change["test"]
        control["marker"] = change["marker"]
        adapted.append(control["id"])
    if sorted(adapted) != sorted(ADAPTED):
        raise SystemExit(f"adapted controls not found: {sorted(set(ADAPTED) - set(adapted))}")
    print(json.dumps({"runner": RUNNER, "runner_sha256": digest, "r2_adapted": adapted}), flush=True)
    return module.main()


if __name__ == "__main__":
    sys.exit(main())

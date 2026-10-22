#!/usr/bin/env python3
"""P2-V1-R3B-I4-R2: the accepted I4 and I4-R1 behavioural controls (38: the
21 I4 controls and the 17 I4-R1 controls), rerun through the accepted I4-R1
runner itself, unchanged (its SHA-256 is verified before it is imported),
with exactly two documented adaptations, both because R2 removes every stop
by the unit's name from an operation without a recorded start reply:

- NC-I4R1-STOP-REPLY-ABSENCE (I4-R1, required). Its mutation, a delivered
  StopUnit reply settling an operation without a candidate, is applied
  unchanged at its unchanged anchor. Its intended test, i4r1_12, concerned
  an uncertain start: that test can no longer reach the mutated line (it
  now proves the stronger rule, no stop at all). The same defect remains
  possible only for an accepted operation, so the control runs the R2 test
  of that path (i4r2_05) and that test's marker.
- NC-I4-X-COLLISION-CLAIMED (I4, additional). Its mutation, a delivered
  `UnitExists` taken as this request's (`Started::Collision => None`), is
  applied unchanged at its unchanged anchor. Its intended test, i4_17,
  marked the harm it then caused: the colliding unit stopped by its name.
  The mutated operation is neither collided nor accepted, so R2 no longer
  stops it by name (i4_17 still fails under the mutation, but at its error
  classification). The harm that remains is the claim: with the foreign
  unit's cgroup holding the helper, that cgroup is opened and proven as
  this request's scope. The control runs the accepted I4-R1 test of exactly
  that (i4r1_16) and that test's marker.

Every other control (anchors, mutations, tests, markers) runs exactly as
accepted. Usage: as the I4-R1 runner (scope_controls.py).
"""
import hashlib
import importlib.util
import json
import pathlib
import sys

sys.dont_write_bytecode = True

RUNNER = "docs/evidence/p2-v1-r3b-i4-r1-native-scope/scripts/scope_controls.py"
RUNNER_SHA256 = "6e84f051e0b050966db2fa277e356614b2074d1fef3e36e3ad949c4f67f611f2"
ADAPTED = {
    "NC-I4R1-STOP-REPLY-ABSENCE": dict(
        test="execution::tests::i4r2_05_only_a_recorded_start_reply_authorizes_a_stop_by_name",
        marker="a StopUnit reply confirmed the operation",
        reason="R2 removes every stop by name from an uncertain operation, so the accepted test "
               "(i4r1_12, an uncertain start) can no longer reach the mutated line; the same "
               "mutation, at the same anchor, now runs the R2 test of the only path that still "
               "stops by name (an accepted operation without a candidate) with its marker.",
    ),
}
ADAPTED_I4 = {
    "NC-I4-X-COLLISION-CLAIMED": dict(
        test="execution::tests::i4r1_16_a_collision_is_never_proven_by_the_foreign_cgroup_s_name",
        marker="a collision was proven by the foreign cgroup's name",
        reason="the mutation turns a delivered UnitExists into an operation neither collided nor "
               "accepted, which R2 never stops by name, so i4_17's marker (the colliding unit "
               "stopped) can no longer be reached (i4_17 still fails, at its error "
               "classification); the harm that remains is the claim of the foreign cgroup as "
               "this request's scope, which the accepted I4-R1 test i4r1_16 marks.",
    ),
}


def main():
    if len(sys.argv) < 3:
        raise SystemExit(__doc__)
    root = pathlib.Path(sys.argv[1]).resolve()
    runner = root / RUNNER
    digest = hashlib.sha256(runner.read_bytes()).hexdigest()
    if digest != RUNNER_SHA256:
        raise SystemExit(f"the accepted I4-R1 runner changed: {digest}")
    spec = importlib.util.spec_from_file_location("i4r1_scope_controls", runner)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    adapted = []
    load_i4 = module.i4_controls

    def i4_controls(root):
        controls, digest = load_i4(root)
        for control in controls:
            change = ADAPTED_I4.get(control["id"])
            if change is None:
                continue
            control["mapping"] = dict(kind="r2-adapted", accepted_test=control["test"],
                                      accepted_marker=control["marker"],
                                      accepted_mapping=control.get("mapping"),
                                      reason=change["reason"])
            control["test"] = change["test"]
            control["marker"] = change["marker"]
            if control["id"] not in adapted:
                adapted.append(control["id"])
        return controls, digest

    module.i4_controls = i4_controls
    i4_ids = {c["id"] for c in load_i4(root)[0]}
    if not set(ADAPTED_I4) <= i4_ids:
        raise SystemExit(f"adapted I4 controls not found: {sorted(set(ADAPTED_I4) - i4_ids)}")
    for control in module.I4R1_REQUIRED + module.I4R1_ADDITIONAL:
        change = ADAPTED.get(control["id"])
        if change is None:
            continue
        control["mapping"] = dict(kind="r2-adapted", accepted_test=control["test"],
                                  accepted_marker=control["marker"], reason=change["reason"])
        control["test"] = change["test"]
        control["marker"] = change["marker"]
        adapted.append(control["id"])
    if sorted(adapted) != sorted(ADAPTED):
        raise SystemExit(f"adapted controls not found: {sorted(set(ADAPTED) - set(adapted))}")
    print(json.dumps({"runner": RUNNER, "runner_sha256": digest,
                      "r2_adapted": sorted(ADAPTED) + sorted(ADAPTED_I4)}), flush=True)
    return module.main()


if __name__ == "__main__":
    sys.exit(main())

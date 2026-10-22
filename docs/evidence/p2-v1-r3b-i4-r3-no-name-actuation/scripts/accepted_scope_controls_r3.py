#!/usr/bin/env python3
"""P2-V1-R3B-I4-R3: the accepted I4 and I4-R1 behavioural controls (38: the
21 I4 controls and the 17 I4-R1 controls), rerun through the accepted I4-R1
runner itself, unchanged (its SHA-256 is verified before it is imported).
R3 removes the manager's StopUnit (the trait method, the production request
and the settling path that asked it), so four controls whose mutation
restores a behaviour of that request can no longer be applied; each is
superseded, with the exact reason and the R3 control that now carries its
invariant, and is recorded, never silently dropped (the first, unadapted
run of this runner on the candidate is kept as evidence). One control is
retargeted to its renamed test. One P2-V1-R3B-I4-R2 adaptation is kept.
Every other control (anchors, mutations, tests, markers) runs exactly as
accepted.

Superseded (their anchor, the StopUnit call in settling, no longer exists,
and their mutation needs the removed request):

- NC-I4-STOP-OK-CONFIRMS (a delivered StopUnit reply taken as cleanup),
  NC-I4-STOP-ERR-NO-EFFECT (a StopUnit error taken as nothing left) and
  NC-I4-STOP-TIMEOUT-DROPS (a timed-out StopUnit drops the operation): no
  StopUnit reply exists to be misread. The invariant they protected (a
  reply to a request is never confirmation; only observation confirms) is
  now structural: the manager has no request that acts besides the start
  (NC-R3-02, scope::tests::i4r3_10_the_manager_can_only_start_a_scope_and_read),
  settling asks the manager nothing but GetUnit (NC-R3-01), and what
  confirms an operation is observed: i4_07 (a candidate still populated),
  i4_09 (a timed-out observation), i4r3_02 (a unit still loaded).
- NC-I4R1-STOP-REPLY-ABSENCE (a delivered StopUnit reply settles an
  operation without a candidate; P2-V1-R3B-I4-R2 had retargeted it to
  i4r2_05, the one path that still stopped by name, which R3 removes): the
  same; NC-R3-01, NC-R3-03.

Retargeted (same mutation, anchor and marker):

- NC-I4R1-REAP-UNRESOLVED: its test i4r1_13 is renamed (the stop it named
  no longer exists): ..._keeps_its_helper_unreaped, the same assertions on
  the helper of an unresolved uncertain start.

Kept from P2-V1-R3B-I4-R2 (accepted_scope_controls_r2.py, unchanged reason):

- NC-I4-X-COLLISION-CLAIMED runs i4r1_16 (the claim of the foreign cgroup).

Usage: as the I4-R1 runner (scope_controls.py).
"""
import hashlib
import importlib.util
import json
import pathlib
import sys

sys.dont_write_bytecode = True

RUNNER = "docs/evidence/p2-v1-r3b-i4-r1-native-scope/scripts/scope_controls.py"
RUNNER_SHA256 = "6e84f051e0b050966db2fa277e356614b2074d1fef3e36e3ad949c4f67f611f2"
STOP_CALL_GONE = ("its anchor, the StopUnit call in PendingScope::reconcile, no longer exists, and its "
                  "mutation calls the manager's stop_unit, which R3 removed with the request itself")
SUPERSEDED_I4 = {
    "NC-I4-STOP-OK-CONFIRMS": dict(
        r3_controls=["NC-R3-01-STOPUNIT-IN-RECONCILE", "NC-R3-02-MANAGER-STOP-RESTORED"],
        r3_tests=["execution::tests::i4_07_a_candidate_still_populated_after_its_kill_confirms_nothing",
                  "execution::tests::i4r3_02_an_accepted_operation_whose_unit_stays_loaded_stays_owned_and_bounded"],
        reason=f"{STOP_CALL_GONE}: no StopUnit reply exists to be taken as cleanup; only observation "
               "confirms (a candidate observed empty or removed, or the manager's NoSuchUnit with the "
               "helper outside, for an accepted operation)."),
    "NC-I4-STOP-ERR-NO-EFFECT": dict(
        r3_controls=["NC-R3-01-STOPUNIT-IN-RECONCILE", "NC-R3-02-MANAGER-STOP-RESTORED"],
        r3_tests=["execution::tests::i4_06_a_unit_the_helper_never_entered_that_stays_loaded_retains_the_operation",
                  "execution::tests::i4r3_02_an_accepted_operation_whose_unit_stays_loaded_stays_owned_and_bounded"],
        reason=f"{STOP_CALL_GONE}: no StopUnit error exists to be taken as nothing left; a unit the "
               "helper never entered that stays loaded keeps the operation (i4_06, i4r3_02)."),
    "NC-I4-STOP-TIMEOUT-DROPS": dict(
        r3_controls=["NC-R3-01-STOPUNIT-IN-RECONCILE", "NC-R3-02-MANAGER-STOP-RESTORED"],
        r3_tests=["execution::tests::i4_09_a_timed_out_observation_retains_the_operation",
                  "execution::tests::i4_07_a_candidate_still_populated_after_its_kill_confirms_nothing"],
        reason=f"{STOP_CALL_GONE}: no StopUnit timeout exists to drop the operation; a timed-out "
               "observation keeps it (i4_09) and a candidate still populated keeps it (i4_07)."),
}
SUPERSEDED_I4R1 = {
    "NC-I4R1-STOP-REPLY-ABSENCE": dict(
        r3_controls=["NC-R3-01-STOPUNIT-IN-RECONCILE", "NC-R3-03-ACCEPTED-AUTHORIZES-STOP"],
        r3_tests=["execution::tests::i4r1_12_an_uncertain_start_without_a_candidate_is_not_released_once_its_unit_is_unloaded",
                  "execution::tests::i4r3_02_an_accepted_operation_whose_unit_stays_loaded_stays_owned_and_bounded"],
        reason=f"{STOP_CALL_GONE}: no StopUnit reply exists to settle an operation without a "
               "candidate (R2 had retargeted this control to i4r2_05, the one path that still "
               "stopped by name; R3 removes that path, and i4r2_05 now asserts that a recorded "
               "start reply authorizes no stop at all)."),
}
RETARGETED_I4R1 = {
    "NC-I4R1-REAP-UNRESOLVED": dict(
        test="execution::tests::i4r1_13_an_uncertain_start_without_a_candidate_keeps_its_helper_unreaped",
        marker="the helper of an unresolved uncertain start was reaped",
        reason="the accepted test i4r1_13 is renamed by R3 (the timed-out stop it named no longer "
               "exists; its world scripted a stop the uncertain start never asked since R2); the "
               "renamed test keeps the same assertions on the same helper and the same marker; the "
               "mutation and its anchor are unchanged.",
    ),
}
# P2-V1-R3B-I4-R2's adaptation, kept with its reason.
ADAPTED_I4 = {
    "NC-I4-X-COLLISION-CLAIMED": dict(
        test="execution::tests::i4r1_16_a_collision_is_never_proven_by_the_foreign_cgroup_s_name",
        marker="a collision was proven by the foreign cgroup's name",
        reason="(P2-V1-R3B-I4-R2, kept) the mutation turns a delivered UnitExists into an operation "
               "neither collided nor accepted, which is never stopped by name, so i4_17's marker (the "
               "colliding unit stopped) cannot be reached (i4_17 still fails, at its error "
               "classification); the harm that remains is the claim of the foreign cgroup as this "
               "request's scope, which the accepted I4-R1 test i4r1_16 marks.",
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
    seen = {"superseded": set(), "retargeted": set(), "adapted": set()}
    load_i4 = module.i4_controls

    def i4_controls(root):
        controls, digest = load_i4(root)
        kept = []
        for control in controls:
            if control["id"] in SUPERSEDED_I4:
                seen["superseded"].add(control["id"])
                continue
            change = ADAPTED_I4.get(control["id"])
            if change is not None:
                control["mapping"] = dict(kind="r2-adapted-kept-by-r3", accepted_test=control["test"],
                                          accepted_marker=control["marker"],
                                          accepted_mapping=control.get("mapping"),
                                          reason=change["reason"])
                control["test"] = change["test"]
                control["marker"] = change["marker"]
                seen["adapted"].add(control["id"])
            kept.append(control)
        return kept, digest

    module.i4_controls = i4_controls
    i4_ids = {c["id"] for c in load_i4(root)[0]}
    missing = (set(SUPERSEDED_I4) | set(ADAPTED_I4)) - i4_ids
    if missing:
        raise SystemExit(f"I4 controls not found: {sorted(missing)}")
    required = []
    for control in module.I4R1_REQUIRED:
        if control["id"] in SUPERSEDED_I4R1:
            seen["superseded"].add(control["id"])
            continue
        change = RETARGETED_I4R1.get(control["id"])
        if change is not None:
            control["mapping"] = dict(kind="r3-retargeted", accepted_test=control["test"],
                                      accepted_marker=control["marker"], reason=change["reason"])
            control["test"] = change["test"]
            control["marker"] = change["marker"]
            seen["retargeted"].add(control["id"])
        required.append(control)
    module.I4R1_REQUIRED = required
    i4_controls(root)
    expected = {"superseded": set(SUPERSEDED_I4) | set(SUPERSEDED_I4R1),
                "retargeted": set(RETARGETED_I4R1), "adapted": set(ADAPTED_I4)}
    if seen != expected:
        raise SystemExit(f"adaptations not found: {expected} != {seen}")
    print(json.dumps({
        "runner": RUNNER, "runner_sha256": digest,
        "accepted_controls": 38,
        "run": 38 - len(expected["superseded"]),
        "r3_superseded": {cid: dict(SUPERSEDED_I4, **SUPERSEDED_I4R1)[cid] for cid in sorted(expected["superseded"])},
        "r3_retargeted": sorted(RETARGETED_I4R1),
        "r2_adapted_kept": sorted(ADAPTED_I4),
    }), flush=True)
    return module.main()


if __name__ == "__main__":
    sys.exit(main())

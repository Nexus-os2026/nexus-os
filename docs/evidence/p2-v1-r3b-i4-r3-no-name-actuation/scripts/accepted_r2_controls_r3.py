#!/usr/bin/env python3
"""P2-V1-R3B-I4-R3: the accepted P2-V1-R3B-I4-R2 mutation controls (10),
rerun through the accepted R2 runner itself, unchanged (its SHA-256 is
verified before it is imported). R2 bounded the stop by the unit's name to
an accepted operation; R3 removes the stop altogether (the manager's
StopUnit, the name authority `stop_authorized()` and the settling path that
asked it). Each control is accounted for (the first, unadapted run of this
runner on the candidate is kept as evidence):

Superseded (each restores a stop by name, or a behaviour of its reply; the
mutation needs the removed request or anchors on the removed authority
check, so it cannot be applied; the R3 control that now carries the
invariant is named):

- NC-R2-01-UNCONDITIONAL-STOP: the authority check it removes no longer
  exists, nor the stop behind it -> NC-R3-01 (StopUnit restored in
  settling, for every operation).
- NC-R2-02-STOP-FOR-UNACCEPTED: `stop_authorized()` no longer exists ->
  NC-R3-03 (the recorded start reply authorizing a stop again) and
  NC-R3-01.
- NC-R2-06-STOP-REPLY-CONFIRMS: no StopUnit reply exists -> NC-R3-01,
  NC-R3-02.
- NC-R2-07-CANDIDATE-NAME-AUTHORITY: no stop by name exists for a
  candidate to grant -> NC-R3-09 (the candidate ended by a path rebuilt
  from the name) and NC-R3-01.
- NC-R2-X-STOP-OUTSIDE-SETTLING: its mutation calls the removed
  `stop_unit` from the drop backstop -> NC-R3-02 (the request restored) and
  the settling guard i4r3_10_settling_never_acts_on_a_unit_by_its_name
  (which pins `end_now` too).

Re-anchored or retargeted (the same mutation; its invariant still applies):

- NC-R2-03-LOST-COLLISION-CONVERTED: its anchor was the removed authority
  check; the same mutation (an uncertain start without a candidate turned
  into a collision by GetUnit's presence of the name, then settled) is
  inserted where R3's settling now reaches the observation, before
  `self.observe(...)`. Same test (i4r2_01), same marker.
- NC-R2-08-ACCEPTED-BY-PRESENCE: its anchor was the removed authority
  check; the same mutation (`accepted` forged from GetUnit's presence of
  the name) is inserted at the same place as NC-R2-03's. Its R2 harm (the
  foreign unit stopped by name) no longer exists; its remaining harm is an
  uncertain start confirmed by the manager's later absence, which R3's
  i4r3_09 marks (the uncertain start retried without its ScopeManager,
  then its unit unloaded): retargeted there, with that test's marker.
- NC-R2-X-ACCEPTED-ELSEWHERE: unchanged mutation and anchor; its test, the
  R2 structural guard i4r2_x, is replaced by
  i4r3_10_a_recorded_start_reply_changes_only_what_absence_confirms, which
  keeps the same check and the same marker.

Unchanged: NC-R2-04-REAPED-WHILE-UNCERTAIN, NC-R2-05-NO-SUCH-UNIT-SETTLES.

Usage: as r2_controls.py.
"""
import hashlib
import importlib.util
import json
import pathlib
import sys

sys.dont_write_bytecode = True

RUNNER = "docs/evidence/p2-v1-r3b-i4-r2-uncertain-stop-authority/scripts/r2_controls.py"
RUNNER_SHA256 = "53d95af28ee7cbff20a777575180be971ac223946a731e2be2bae2db4d29d036"
# Where R3's settling reaches the observation (the R2 check's place).
OBSERVE = ("        // Nothing is acted upon by the unit's name (P2-V1-R3B-I4-R3): what\n"
           "        // the operation may have created is ended only through its\n"
           "        // candidate, by descriptor, and confirmed only by observation.\n"
           "        self.observe(&controller, helper, fault)\n")
SUPERSEDED = {
    "NC-R2-01-UNCONDITIONAL-STOP": dict(
        r3_controls=["NC-R3-01-STOPUNIT-IN-RECONCILE"],
        reason="its anchor, the name authority check in reconcile, and the StopUnit behind it no "
               "longer exist; restoring an unconditional stop by name means restoring the manager's "
               "StopUnit in settling, which NC-R3-01 does."),
    "NC-R2-02-STOP-FOR-UNACCEPTED": dict(
        r3_controls=["NC-R3-03-ACCEPTED-AUTHORIZES-STOP", "NC-R3-01-STOPUNIT-IN-RECONCILE"],
        reason="its anchor, stop_authorized(), no longer exists: no operation, accepted or not, has "
               "any authority to act by name; restoring that authority is NC-R3-03."),
    "NC-R2-06-STOP-REPLY-CONFIRMS": dict(
        r3_controls=["NC-R3-01-STOPUNIT-IN-RECONCILE", "NC-R3-02-MANAGER-STOP-RESTORED"],
        reason="its anchor, the StopUnit call in reconcile, no longer exists, and no StopUnit reply "
               "exists to be taken as confirmation."),
    "NC-R2-07-CANDIDATE-NAME-AUTHORITY": dict(
        r3_controls=["NC-R3-09-CANDIDATE-BY-NAME", "NC-R3-01-STOPUNIT-IN-RECONCILE"],
        reason="its anchor, the name authority check, no longer exists, and no stop by name exists for "
               "a candidate to grant; a candidate ended other than through its descriptor is "
               "NC-R3-09."),
    "NC-R2-X-STOP-OUTSIDE-SETTLING": dict(
        r3_controls=["NC-R3-02-MANAGER-STOP-RESTORED"],
        reason="its mutation calls the manager's stop_unit, which R3 removed (the mutation cannot "
               "compile); restoring the request is NC-R3-02, and the R3 settling guard pins the drop "
               "backstop (end_now) asking the manager nothing."),
}
FORGED = ("        // NC-R2-08-ACCEPTED-BY-PRESENCE: presence taken as acceptance.\n"
          "        if matches!(\n"
          "            controller.manager.get_unit(&self.unit),\n"
          "            Remote::Answered(Presence::Present(_))\n"
          "        ) {\n"
          "            self.accepted = true;\n"
          "        }\n")
CONVERTED = ("        // NC-R2-03-LOST-COLLISION-CONVERTED: the name's presence makes it a collision.\n"
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
             "        }\n")
ADAPTED = {
    "NC-R2-03-LOST-COLLISION-CONVERTED": dict(
        prefix=CONVERTED, test=None, marker=None,
        reason="re-anchored: the R2 anchor (the name authority check) no longer exists; the same "
               "mutation is inserted where R3's settling reaches the observation. Same test, same "
               "marker."),
    "NC-R2-08-ACCEPTED-BY-PRESENCE": dict(
        prefix=FORGED,
        test="execution::tests::i4r3_09_a_retained_operation_retried_without_its_scope_manager_never_acts_by_name",
        marker="an uncertain start was resolved by its unit name or presence",
        reason="re-anchored as NC-R2-03, and retargeted: the R2 harm (the foreign unit stopped by "
               "name) no longer exists; the remaining harm of a forged acceptance is an uncertain "
               "start confirmed by the manager's later absence, which i4r3_09 marks."),
    "NC-R2-X-ACCEPTED-ELSEWHERE": dict(
        prefix=None,
        test="scope::tests::i4r3_10_a_recorded_start_reply_changes_only_what_absence_confirms",
        marker=None,
        reason="retargeted: the R2 structural guard i4r2_x is replaced by the R3 guard of the "
               "recorded start reply, which keeps the same check (set only in the delivered reply's "
               "arm) and the same marker. Same mutation, same anchor."),
}


def main():
    if len(sys.argv) < 3:
        raise SystemExit(__doc__)
    root = pathlib.Path(sys.argv[1]).resolve()
    runner = root / RUNNER
    digest = hashlib.sha256(runner.read_bytes()).hexdigest()
    if digest != RUNNER_SHA256:
        raise SystemExit(f"the accepted R2 runner changed: {digest}")
    spec = importlib.util.spec_from_file_location("r2_controls", runner)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    kept, superseded, adapted = [], [], []
    for control in module.CONTROLS:
        if control["id"] in SUPERSEDED:
            superseded.append(control["id"])
            continue
        change = ADAPTED.get(control["id"])
        if change is not None:
            accepted = dict(test=control["test"], marker=control["marker"],
                            edits=[dict(e) for e in control["edits"]])
            if change["prefix"] is not None:
                (edit,) = control["edits"]
                if edit["anchor"] != module.CHECK or not edit["replacement"].startswith(change["prefix"]):
                    raise SystemExit(f"{control['id']}: the accepted mutation is not as recorded")
                edit["anchor"] = OBSERVE
                edit["replacement"] = change["prefix"] + OBSERVE
            if change["test"] is not None:
                control["test"] = change["test"]
            if change["marker"] is not None:
                control["marker"] = change["marker"]
            control["what"] = (f"{control['what']} [R3-adapted: {change['reason']} Accepted test "
                               f"{accepted['test']}, marker {accepted['marker']!r}]")
            adapted.append(control["id"])
        kept.append(control)
    if sorted(superseded) != sorted(SUPERSEDED) or sorted(adapted) != sorted(ADAPTED):
        raise SystemExit(f"adaptations not found: {superseded} {adapted}")
    module.CONTROLS = kept
    print(json.dumps({"runner": RUNNER, "runner_sha256": digest, "accepted_controls": 10,
                      "run": len(kept), "r3_superseded": SUPERSEDED,
                      "r3_adapted": {cid: ADAPTED[cid]["reason"] for cid in sorted(ADAPTED)}}),
          flush=True)
    return module.main()


if __name__ == "__main__":
    sys.exit(main())

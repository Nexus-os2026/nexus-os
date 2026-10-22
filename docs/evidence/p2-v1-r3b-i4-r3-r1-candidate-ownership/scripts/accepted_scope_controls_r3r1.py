#!/usr/bin/env python3
"""P2-V1-R3B-I4-R3-R1: the accepted I4 and I4-R1 behavioural controls (38),
rerun exactly as P2-V1-R3B-I4-R3 accepted them: R3's adapter
(accepted_scope_controls_r3.py) runs unchanged, and it runs the accepted
I4-R1 runner unchanged (both SHA-256 verified before they are imported).
R3's adaptations (4 superseded, 1 retargeted, 1 R2 adaptation kept) stay as
recorded; once R3's adapter has applied them, and just before the runner's
own main runs, R3-R1 adapts the ten controls whose anchor or mutation
R3-R1's code changes (the first, unadapted run is kept as evidence). Each keeps its accepted
invariant; the mutation is the accepted one at R3-R1's code unless stated:

I4:
- NC-I4-START-TIMEOUT-ABSENT (an uncertain start taken as no effect: the
  operation dropped, `issued` false): the same mutation on R3-R1's
  uncertain arm. Retargeted: its accepted test asserted that such a start's
  unit is discovered and proven, which R3-R1 forbids (mission section 13);
  the test, renamed i4_03_..._is_never_proven_or_ended, asserts that the
  operation stays owned with its helper, and the mutation's harm (the
  operation released without proof) fails it at `retained`: "cleanup
  reported confirmed without proof".
- NC-I4-START-NAME-AUTH, NC-I4-MEMBERSHIP-NAME-ONLY (the process-list
  check removed): the same removal of R3-R1's check (it reads through the
  candidate's descriptor, `dir`). Same tests and markers.
- NC-I4-NO-EARLY-DIR (the candidate kept only once every proof passed):
  R3-R1 binds the retained candidate itself, so a candidate kept only
  after the proof could never be bound; the equivalent regression, with
  the same harm (a failed proof loses the candidate, and settling cannot
  end what it holds), drops the candidate when the proof fails. Same test
  and marker.
- NC-I4-X-NO-CANDIDATE-KILL (settling observes the candidate without
  ending it): R3-R1 ends an owned candidate only through `end_owned()`;
  the same regression empties it. Same test and marker.
- NC-I4-X-COLLISION-CLAIMED (a loaded unit of the name taken as this
  request's; R2's retarget to i4r1_16, kept by R3): its anchor is
  unchanged, but its replacement (the collision arm yielding no uncertain
  reason) no longer type-checks, the arms now recording or returning, and a
  collision has no identity, so taking the loaded unit as this request's
  means taking its identity: the equivalent mutation adopts the unit's
  current `InvocationID`, read by its name, and lets the proof go on. Same
  test and marker.

I4-R1:
- NC-I4R1-NO-CONTROLGROUP, NC-I4R1-CONTROLGROUP-BASENAME,
  NC-I4R1-X-CONTROLGROUP-UNCERTAIN: the control group binding moved from
  the proof into R3-R1's `bind` (it compares with the path the candidate
  was opened from, `candidate.path`, and is followed by the directory's
  ID): the same mutations there. Same tests and markers.
- NC-I4R1-ABSENCE-GETUNIT (the manager's absence settles an uncertain
  start): R3-R1's `gone` ends with `self.accepted && absent(...)`; the same
  mutation drops the `self.accepted` condition. Same test and marker.

Every other control (anchors, mutations, tests, markers) runs exactly as
R3 accepted it. Usage: as the I4-R1 runner (scope_controls.py).
"""
import hashlib
import importlib.util
import json
import pathlib
import sys

sys.dont_write_bytecode = True

ADAPTER = "docs/evidence/p2-v1-r3b-i4-r3-no-name-actuation/scripts/accepted_scope_controls_r3.py"
ADAPTER_SHA256 = "6e297be54672a05950d5f662970c580eb636f6cce3c8d17a5b79f1dbf605714e"
RUNNER = "docs/evidence/p2-v1-r3b-i4-r1-native-scope/scripts/scope_controls.py"
RUNNER_SHA256 = "6e84f051e0b050966db2fa277e356614b2074d1fef3e36e3ad949c4f67f611f2"
PENDING = "crates/nexus-verifier-sandbox/src/scope/pending.rs"

UNCERTAIN_ARM = ("            // The request may or may not have taken effect, or take it\n"
                 "            // later, and nothing identifies what it may have started: it is\n"
                 "            // never proven, and nothing of it is ever opened, owned or ended\n"
                 "            // (P2-V1-R3B-I4-R3-R1).\n"
                 "            Started::Uncertain(reason) => return Err(ScopeError::Bus(reason)),\n")
PROCS = ("        let procs = dir.read(\"cgroup.procs\").map_err(ScopeError::Io)?;\n"
         "        if !procs\n"
         "            .lines()\n"
         "            .any(|line| line.trim() == helper.pid().to_string())\n"
         "        {\n"
         "            return Err(ScopeError::Mismatch(\"helper not in the scope\"));\n"
         "        }\n")
PROVE_CALL = "        self.prove(&controller, helper, fault)\n    }\n"
END_OWNED = ("    fn end_owned(&self) {\n"
             "        if let Some(owned) = self.owned_dir() {\n"
             "            let _ = owned.kill();\n"
             "        }\n"
             "    }\n")
CONTROL_GROUP = ("        match controller.manager.control_group(&unit_path) {\n"
                 "            Remote::Answered(Some(group)) if group == candidate.path => {}\n"
                 "            Remote::Answered(_) => return Err(ScopeError::Mismatch(\"unit control group\")),\n"
                 "            Remote::Uncertain(reason) => return Err(ScopeError::Bus(reason)),\n"
                 "        }\n")
CONTROL_GROUP_EQUAL = "            Remote::Answered(Some(group)) if group == candidate.path => {}\n"
CONTROL_GROUP_UNCERTAIN = ("            Remote::Uncertain(reason) => return Err(ScopeError::Bus(reason)),\n"
                           "        }\n"
                           "        let retained = candidate.dir.cgroup_id().map_err(ScopeError::Io)?;\n")
GONE_ACCEPTED = ("        // An observed candidate's emptiness confirms nothing, and without a\n"
                 "        // delivered reply the manager's absence proves nothing.\n"
                 "        self.accepted && absent(controller, &self.unit, helper)\n")


def reanchored(accepted_edits, index, anchor):
    """The accepted edit at `index`, anchored at R3-R1's `anchor`; its
    replacement as accepted (without the trailing blank line the accepted
    anchor carried, if any)."""
    edit = accepted_edits[index]
    replacement = edit["replacement"]
    if edit["anchor"].endswith("\n\n") and replacement.endswith("\n\n"):
        replacement = replacement[:-1]
    return (PENDING, anchor, replacement)


ADAPT_I4 = {
    "NC-I4-START-TIMEOUT-ABSENT": dict(
        edits=lambda accepted: [reanchored(accepted, 0, UNCERTAIN_ARM)],
        test="scope::tests::i4_03_a_start_with_effect_whose_reply_is_lost_is_never_proven_or_ended",
        marker="cleanup reported confirmed without proof",
        reason="re-anchored on R3-R1's uncertain arm and retargeted: the accepted test asserted that "
               "an uncertain start's unit is discovered and proven, which R3-R1 forbids (mission "
               "section 13); its renamed test asserts the operation stays owned with its helper, which "
               "the mutation (the operation dropped as never issued) fails at `retained`."),
    "NC-I4-START-NAME-AUTH": dict(
        edits=lambda accepted: [(accepted[0]["file"], accepted[0]["anchor"], accepted[0]["replacement"]),
                                reanchored(accepted, 1, PROCS)],
        test=None, marker=None,
        reason="the process-list check, R3-R1's (through the candidate's descriptor), removed as "
               "accepted; the name-located path unchanged. Same test, same marker."),
    "NC-I4-NO-EARLY-DIR": dict(
        edits=lambda accepted: [(PENDING, PROVE_CALL,
                                 "        // NC-I4-NO-EARLY-DIR: the candidate is kept only once proven.\n"
                                 "        let proved = self.prove(&controller, helper, fault);\n"
                                 "        if proved.is_err() {\n"
                                 "            self.candidate = None;\n"
                                 "        }\n"
                                 "        proved\n"
                                 "    }\n")],
        test=None, marker=None,
        reason="an equivalent mutation: R3-R1 binds the retained candidate itself, so a candidate kept "
               "only after the proof could never be bound; the same harm (a failed proof loses the "
               "candidate) is a failed proof that drops it. Same test, same marker."),
    "NC-I4-MEMBERSHIP-NAME-ONLY": dict(
        edits=lambda accepted: [reanchored(accepted, 0, PROCS)],
        test=None, marker=None,
        reason="the process-list check, R3-R1's (through the candidate's descriptor), removed as "
               "accepted. Same test, same marker."),
    "NC-I4-X-NO-CANDIDATE-KILL": dict(
        edits=lambda accepted: [(PENDING, END_OWNED,
                                 "    fn end_owned(&self) {\n"
                                 "        // NC-I4-X-NO-CANDIDATE-KILL: nothing in the candidate is ended.\n"
                                 "    }\n")],
        test=None, marker=None,
        reason="R3-R1 ends an owned candidate only through end_owned() (settling, and the drop "
               "backstop); the same regression empties it. Same test, same marker."),
}
CLAIMED = ("            // NC-I4-X-COLLISION-CLAIMED: a loaded unit of the name is taken as\n"
           "            // this request's.\n"
           "            Started::Collision => {\n"
           "                if let Remote::Answered(Presence::Present(unit_path)) =\n"
           "                    controller.manager.get_unit(&self.unit)\n"
           "                {\n"
           "                    if let Remote::Answered(Some(current)) =\n"
           "                        controller.manager.unit_instance(&unit_path)\n"
           "                    {\n"
           "                        self.instance = Some(current);\n"
           "                    }\n"
           "                }\n"
           "            }\n")
ADAPT_I4["NC-I4-X-COLLISION-CLAIMED"] = dict(
    edits=lambda accepted: [(PENDING, accepted[0]["anchor"], CLAIMED)],
    test=None, marker=None,
    reason="an equivalent mutation (anchor unchanged; R2's retarget to i4r1_16, kept by R3, unchanged): "
           "the accepted replacement (the collision arm yielding no uncertain reason) no longer type-checks "
           "in R3-R1, where the arms record or return, and a collision has no identity, so taking the "
           "loaded unit as this request's means taking its identity: the collision arm adopts the unit's "
           "current InvocationID, read by its name, and the proof goes on. Same test, same marker.")
ADAPT_I4R1 = {
    "NC-I4R1-NO-CONTROLGROUP": dict(
        edits=lambda accepted: [reanchored(accepted, 0, CONTROL_GROUP)],
        reason="the control group check moved into R3-R1's bind (compared with candidate.path); the "
               "same removal there. Same test, same marker."),
    "NC-I4R1-CONTROLGROUP-BASENAME": dict(
        edits=lambda accepted: [(PENDING, CONTROL_GROUP_EQUAL,
                                 "            // NC-I4R1-CONTROLGROUP-BASENAME: the last components only.\n"
                                 "            Remote::Answered(Some(group))\n"
                                 "                if group.rsplit('/').next() == candidate.path.rsplit('/').next() => {}\n")],
        reason="the control group check moved into R3-R1's bind (compared with candidate.path); the "
               "same last-component comparison there. Same test, same marker."),
    "NC-I4R1-X-CONTROLGROUP-UNCERTAIN": dict(
        edits=lambda accepted: [(PENDING, CONTROL_GROUP_UNCERTAIN,
                                 "            // NC-I4R1-X-CONTROLGROUP-UNCERTAIN: an uncertain answer binds.\n"
                                 "            Remote::Uncertain(_) => {}\n"
                                 "        }\n"
                                 "        let retained = candidate.dir.cgroup_id().map_err(ScopeError::Io)?;\n")],
        reason="the control group check moved into R3-R1's bind, where the directory's ID follows it; "
               "the same mutation of its uncertain arm. Same test, same marker."),
    "NC-I4R1-ABSENCE-GETUNIT": dict(
        edits=lambda accepted: [(PENDING, GONE_ACCEPTED,
                                 "        // NC-I4R1-ABSENCE-GETUNIT: the manager's absence settles an\n"
                                 "        // uncertain start.\n"
                                 "        absent(controller, &self.unit, helper)\n")],
        reason="R3-R1's gone() ends with self.accepted && absent(...); the same mutation drops the "
               "delivered-reply condition. Same test, same marker."),
}


def apply(control, change, kind):
    accepted = [dict(e) for e in control["edits"]]
    control["edits"] = [dict(file=f, anchor=a, replacement=r) for (f, a, r) in change["edits"](accepted)]
    mapping = dict(kind=kind, accepted_test=control["test"], accepted_marker=control["marker"],
                   accepted_edits=accepted, accepted_mapping=control.get("mapping"),
                   reason=change["reason"])
    if change.get("test"):
        control["test"] = change["test"]
    if change.get("marker"):
        control["marker"] = change["marker"]
    control["mapping"] = mapping


def adapt(module):
    seen = set()
    r3_i4_controls = module.i4_controls

    def i4_controls(root):
        controls, digest = r3_i4_controls(root)
        for control in controls:
            change = ADAPT_I4.get(control["id"])
            if change is not None:
                apply(control, change, "r3r1-adapted")
                seen.add(control["id"])
        return controls, digest

    module.i4_controls = i4_controls
    for control in module.I4R1_REQUIRED + module.I4R1_ADDITIONAL:
        change = ADAPT_I4R1.get(control["id"])
        if change is not None:
            apply(control, change, "r3r1-adapted")
            seen.add(control["id"])
    i4_controls(pathlib.Path(sys.argv[1]).resolve())
    if seen != set(ADAPT_I4) | set(ADAPT_I4R1):
        raise SystemExit(f"adaptations not found: {sorted(seen)}")
    print(json.dumps({"r3r1_adapted": {cid: c["reason"] for cid, c in
                                       sorted(dict(ADAPT_I4, **ADAPT_I4R1).items())},
                      "r3r1_retargeted": {"NC-I4-START-TIMEOUT-ABSENT": ADAPT_I4["NC-I4-START-TIMEOUT-ABSENT"]["test"]}}),
          flush=True)


def main():
    if len(sys.argv) < 3:
        raise SystemExit(__doc__)
    root = pathlib.Path(sys.argv[1]).resolve()
    for path, expected in ((ADAPTER, ADAPTER_SHA256), (RUNNER, RUNNER_SHA256)):
        digest = hashlib.sha256((root / path).read_bytes()).hexdigest()
        if digest != expected:
            raise SystemExit(f"{path} changed: {digest}")
    original = importlib.util.spec_from_file_location
    spec = original("accepted_scope_controls_r3", root / ADAPTER)
    r3 = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(r3)
    target = (root / RUNNER).resolve()

    def spec_from_file_location(name, location, *args, **kwargs):
        found = original(name, location, *args, **kwargs)
        if pathlib.Path(location).resolve() == target:
            exec_module = found.loader.exec_module

            def hooked(module):
                exec_module(module)
                runner_main = module.main

                def adapted_main():
                    adapt(module)
                    return runner_main()

                module.main = adapted_main

            found.loader.exec_module = hooked
        return found

    importlib.util.spec_from_file_location = spec_from_file_location
    return r3.main()


if __name__ == "__main__":
    sys.exit(main())

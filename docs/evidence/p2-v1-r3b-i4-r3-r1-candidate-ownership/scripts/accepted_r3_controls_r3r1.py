#!/usr/bin/env python3
"""P2-V1-R3B-I4-R3-R1: the accepted P2-V1-R3B-I4-R3 mutation controls (10),
rerun through the accepted R3 runner itself, unchanged (its SHA-256 is
verified before it is imported). R3-R1 changes the code seven of them
anchor on; each keeps its accepted mutation, test and marker, re-anchored
where R3-R1 moved what it mutates (the first, unadapted run of the runner
on the candidate is kept as evidence):

- NC-R3-01-STOPUNIT-IN-RECONCILE, NC-R3-03-ACCEPTED-AUTHORIZES-STOP,
  NC-R3-05-OBJECT-PATH-IDENTITY: their pending.rs anchor was settling's
  call of the observation, with R3's comment. R3-R1 ends what an owned
  candidate holds (`self.end_owned()`) just before that call: the same
  mutation is inserted between the two (`end_owned` kept), so settling
  reaches it exactly as in R3.
- NC-R3-04-PRESENCE-AS-OWNERSHIP: its anchor was `acquire`'s head; R3-R1's
  head also returns without a captured identity. The same mutation (a unit
  the manager reports loaded under the name taken as the accepted
  operation's: its control group opened, ended and retained) is inserted
  after that head; the retained candidate is R3-R1's `Candidate`, marked
  owned, which is what "retained as the candidate" now means for cleanup.
- NC-R3-06-FOREIGN-PRESENCE-CONFIRMS: its anchor was `gone`'s arm for an
  accepted operation without a candidate; R3-R1's `gone` ends with that
  rule as one expression (`self.accepted && absent(...)`). The same
  weakening (a loaded unit that does not hold the helper taken as gone) is
  applied to it.
- NC-R3-08-UNCERTAIN-WEAKENED: its anchor was the uncertain start's arm,
  which went on to the proof; R3-R1's arm returns at once. The same
  mutation (the uncertain start recorded as accepted) is applied to it.
- NC-R3-09-CANDIDATE-BY-NAME: its anchor was settling's `cgroup.kill` of
  the candidate; R3-R1's settling ends the owned candidate through
  `end_owned()`. The same mutation (the candidate ended through a path
  rebuilt from the unit's name, not its descriptor) replaces that call.

NC-R3-02, NC-R3-07 and NC-R3-10 run exactly as accepted.

Usage: as r3_controls.py.
"""
import hashlib
import importlib.util
import json
import pathlib
import sys

sys.dont_write_bytecode = True

RUNNER = "docs/evidence/p2-v1-r3b-i4-r3-no-name-actuation/scripts/r3_controls.py"
RUNNER_SHA256 = "b8576a29e19f83209516f1e7359ed17cfee9deb3c519e372e28cc79a1ccabf8d"
# R3-R1's settling: the owned candidate ended, then the observation.
RECONCILE_END = ("        self.end_owned();\n"
                 "        self.observe(&controller, helper, fault)\n")
ACQUIRE_HEAD = ("    fn acquire(&mut self, controller: &Controller, helper: Option<&Helper>) {\n"
                "        if self.candidate.is_some() || self.instance.is_none() {\n"
                "            return;\n"
                "        }\n")
GONE_ACCEPTED = "        self.accepted && absent(controller, &self.unit, helper)\n"
UNCERTAIN_ARM = "            Started::Uncertain(reason) => return Err(ScopeError::Bus(reason)),\n"
RETAINED = "                        self.candidate = Some(candidate);\n"
RETAINED_OWNED = ("                        self.candidate = Some(Candidate {\n"
                  "                            dir: candidate,\n"
                  "                            path: group,\n"
                  "                            owned: true,\n"
                  "                        });\n")
OBSERVE_CALL = "        self.observe(&controller, helper, fault)\n"
ADAPTED = {
    "NC-R3-01-STOPUNIT-IN-RECONCILE":
        "re-anchored: the mutation inserted between R3-R1's end_owned() and the observation",
    "NC-R3-03-ACCEPTED-AUTHORIZES-STOP":
        "re-anchored: the mutation inserted between R3-R1's end_owned() and the observation",
    "NC-R3-04-PRESENCE-AS-OWNERSHIP":
        "re-anchored after R3-R1's acquire head; the retained candidate is R3-R1's Candidate, owned",
    "NC-R3-05-OBJECT-PATH-IDENTITY":
        "re-anchored: the mutation inserted between R3-R1's end_owned() and the observation",
    "NC-R3-06-FOREIGN-PRESENCE-CONFIRMS":
        "re-anchored: the same weakening of R3-R1's accepted-without-ownership rule in gone()",
    "NC-R3-08-UNCERTAIN-WEAKENED":
        "re-anchored: R3-R1's uncertain arm, recorded as accepted",
    "NC-R3-09-CANDIDATE-BY-NAME":
        "re-anchored: the by-name kill replaces R3-R1's end_owned() in settling",
}


def adapt(module, control):
    cid = control["id"]
    for edit in control["edits"]:
        if edit["file"] != module.PENDING:
            continue
        anchor, replacement = edit["anchor"], edit["replacement"]
        if anchor == module.OBSERVE:
            # The accepted replacement ends with the observation's call.
            if not replacement.endswith(OBSERVE_CALL):
                raise SystemExit(f"{cid}: the accepted mutation is not as recorded")
            edit["anchor"] = RECONCILE_END
            edit["replacement"] = "        self.end_owned();\n" + replacement
        elif anchor == module.ACQUIRE:
            if RETAINED not in replacement or not replacement.startswith(module.ACQUIRE):
                raise SystemExit(f"{cid}: the accepted mutation is not as recorded")
            edit["anchor"] = ACQUIRE_HEAD
            edit["replacement"] = ACQUIRE_HEAD + replacement[len(module.ACQUIRE):].replace(
                RETAINED, RETAINED_OWNED)
        elif anchor == module.GONE_ACCEPTED:
            edit["anchor"] = GONE_ACCEPTED
            edit["replacement"] = (
                "        // NC-R3-06: a loaded unit that does not hold the helper taken\n"
                "        // as another's, and this operation as gone.\n"
                "        self.accepted\n"
                "            && (absent(controller, &self.unit, helper)\n"
                "                || (matches!(\n"
                "                    controller.manager.get_unit(&self.unit),\n"
                "                    Remote::Answered(Presence::Present(_))\n"
                "                ) && outside(controller, helper, &self.unit)))\n")
        elif anchor == module.UNCERTAIN_ARM:
            edit["anchor"] = UNCERTAIN_ARM
            edit["replacement"] = ("            // NC-R3-08: an uncertain start recorded as accepted.\n"
                                   "            Started::Uncertain(reason) => {\n"
                                   "                self.accepted = true;\n"
                                   "                return Err(ScopeError::Bus(reason));\n"
                                   "            }\n")
        elif anchor == module.CANDIDATE_KILL:
            edit["anchor"] = RECONCILE_END
            edit["replacement"] = replacement + OBSERVE_CALL
    control["what"] = f"{control['what']} [R3-R1-adapted: {ADAPTED[cid]}]"


def main():
    if len(sys.argv) < 3:
        raise SystemExit(__doc__)
    root = pathlib.Path(sys.argv[1]).resolve()
    runner = root / RUNNER
    digest = hashlib.sha256(runner.read_bytes()).hexdigest()
    if digest != RUNNER_SHA256:
        raise SystemExit(f"the accepted R3 runner changed: {digest}")
    spec = importlib.util.spec_from_file_location("r3_controls", runner)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    adapted = []
    for control in module.CONTROLS:
        if control["id"] in ADAPTED:
            adapt(module, control)
            adapted.append(control["id"])
    if sorted(adapted) != sorted(ADAPTED):
        raise SystemExit(f"adaptations not found: {adapted}")
    print(json.dumps({"runner": RUNNER, "runner_sha256": digest, "accepted_controls": 10,
                      "run": len(module.CONTROLS), "r3r1_adapted": ADAPTED}), flush=True)
    return module.main()


if __name__ == "__main__":
    sys.exit(main())

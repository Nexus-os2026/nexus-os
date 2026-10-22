#!/usr/bin/env python3
"""P2-V1-R3B-I4-R3-R1: the accepted P2-V1-R3B-I4-R2 mutation controls (10),
rerun exactly as P2-V1-R3B-I4-R3 accepted them: R3's adapter
(accepted_r2_controls_r3.py) runs unchanged, and it runs the accepted R2
runner unchanged (both SHA-256 verified before they are imported). R3's
adaptations (5 superseded, 3 re-anchored or retargeted) stay as recorded;
once R3's adapter has applied them, and just before the runner's own main
runs, R3-R1 re-anchors the four controls whose anchor R3-R1 moved (the
first, unadapted run is kept as evidence). Same mutations, tests and
markers:

- NC-R2-03-LOST-COLLISION-CONVERTED, NC-R2-08-ACCEPTED-BY-PRESENCE: R3
  inserted them where settling reaches the observation (R3's comment and
  the call). R3-R1 ends what an owned candidate holds (`end_owned()`) just
  before that call: the same mutation is inserted before `end_owned()`.
- NC-R2-05-NO-SUCH-UNIT-SETTLES: its anchor was `gone`'s arm for an
  operation without a candidate and a delivered reply; R3-R1's `gone` ends
  with `self.accepted && absent(...)`: the same mutation (the manager's
  absence settles any start) drops the `self.accepted` condition.
- NC-R2-X-ACCEPTED-ELSEWHERE: its anchor was the candidate retained in
  `acquire`; R3-R1 retains it as an observed `Candidate`: the same mutation
  (acceptance set where a candidate is retained) follows that line.

Usage: as r2_controls.py.
"""
import hashlib
import importlib.util
import json
import pathlib
import sys

sys.dont_write_bytecode = True

ADAPTER = "docs/evidence/p2-v1-r3b-i4-r3-no-name-actuation/scripts/accepted_r2_controls_r3.py"
ADAPTER_SHA256 = "58f34b815184eb3ffd696723953f0a90d868d977aa7877a89ab8f9a38c5625f9"
RUNNER = "docs/evidence/p2-v1-r3b-i4-r2-uncertain-stop-authority/scripts/r2_controls.py"
RUNNER_SHA256 = "53d95af28ee7cbff20a777575180be971ac223946a731e2be2bae2db4d29d036"
RECONCILE_END = ("        self.end_owned();\n"
                 "        self.observe(&controller, helper, fault)\n")
GONE_ACCEPTED = ("        // An observed candidate's emptiness confirms nothing, and without a\n"
                 "        // delivered reply the manager's absence proves nothing.\n"
                 "        self.accepted && absent(controller, &self.unit, helper)\n")
RETAINED = ("                    self.candidate = Some(Candidate {\n"
            "                        dir,\n"
            "                        path,\n"
            "                        owned: false,\n"
            "                    });\n")
ADAPTED = {
    "NC-R2-03-LOST-COLLISION-CONVERTED":
        "re-anchored (R3's insertion point): the same mutation before R3-R1's end_owned()",
    "NC-R2-08-ACCEPTED-BY-PRESENCE":
        "re-anchored (R3's insertion point): the same mutation before R3-R1's end_owned()",
    "NC-R2-05-NO-SUCH-UNIT-SETTLES":
        "re-anchored: R3-R1's gone() without its delivered-reply condition",
    "NC-R2-X-ACCEPTED-ELSEWHERE":
        "re-anchored: acceptance set where R3-R1's acquire retains the observed candidate",
}


def adapt(module, r3):
    seen = []
    for control in module.CONTROLS:
        cid = control["id"]
        if cid not in ADAPTED:
            continue
        (edit,) = control["edits"]
        if cid in ("NC-R2-03-LOST-COLLISION-CONVERTED", "NC-R2-08-ACCEPTED-BY-PRESENCE"):
            if edit["anchor"] != r3.OBSERVE or not edit["replacement"].endswith(r3.OBSERVE):
                raise SystemExit(f"{cid}: R3's adaptation is not as recorded")
            prefix = edit["replacement"][:-len(r3.OBSERVE)]
            edit["anchor"] = RECONCILE_END
            edit["replacement"] = prefix + RECONCILE_END
        elif cid == "NC-R2-05-NO-SUCH-UNIT-SETTLES":
            if edit["anchor"] != "            None if !self.accepted => false,\n":
                raise SystemExit(f"{cid}: the accepted mutation is not as recorded")
            edit["anchor"] = GONE_ACCEPTED
            edit["replacement"] = ("        // NC-R2-05-NO-SUCH-UNIT-SETTLES: the manager's absence settles it.\n"
                                   "        absent(controller, &self.unit, helper)\n")
        elif cid == "NC-R2-X-ACCEPTED-ELSEWHERE":
            if "self.accepted = true;" not in edit["replacement"]:
                raise SystemExit(f"{cid}: the accepted mutation is not as recorded")
            edit["anchor"] = RETAINED
            edit["replacement"] = (RETAINED +
                                   "                    // NC-R2-X-ACCEPTED-ELSEWHERE: a candidate taken as acceptance.\n"
                                   "                    self.accepted = true;\n")
        control["what"] = f"{control['what']} [R3-R1-adapted: {ADAPTED[cid]}]"
        seen.append(cid)
    if sorted(seen) != sorted(ADAPTED):
        raise SystemExit(f"adaptations not found: {seen}")
    print(json.dumps({"r3r1_adapted": ADAPTED, "run": len(module.CONTROLS)}), flush=True)


def main():
    if len(sys.argv) < 3:
        raise SystemExit(__doc__)
    root = pathlib.Path(sys.argv[1]).resolve()
    for path, expected in ((ADAPTER, ADAPTER_SHA256), (RUNNER, RUNNER_SHA256)):
        digest = hashlib.sha256((root / path).read_bytes()).hexdigest()
        if digest != expected:
            raise SystemExit(f"{path} changed: {digest}")
    original = importlib.util.spec_from_file_location
    spec = original("accepted_r2_controls_r3", root / ADAPTER)
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
                    adapt(module, r3)
                    return runner_main()

                module.main = adapted_main

            found.loader.exec_module = hooked
        return found

    importlib.util.spec_from_file_location = spec_from_file_location
    return r3.main()


if __name__ == "__main__":
    sys.exit(main())

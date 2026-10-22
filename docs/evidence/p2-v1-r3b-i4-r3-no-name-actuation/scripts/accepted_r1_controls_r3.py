#!/usr/bin/env python3
"""P2-V1-R3B-I4-R3: the accepted Q1-R1 mutation controls (15), rerun through
the accepted Q1-R1 runner itself, unchanged (its SHA-256 is verified before
it is imported). R3 removes the manager's StopUnit, so two controls whose
mutation restores a use of that request can no longer be applied; each is
superseded, with the exact reason and the R3 control that now carries its
invariant, and is recorded (the first, unadapted run of this runner on the
candidate is kept as evidence). Every other control runs exactly as
accepted.

- Q1-R1-NC1-FOREIGN-STOPPED: its mutation calls the manager's stop_unit on
  the collided (foreign) unit; R3 removed that request, so the mutation
  cannot compile. No unit, foreign or not, can be stopped by its name
  without restoring the request (NC-R3-02, NC-R3-01); the collided arm's
  other harm, the foreign cgroup taken and killed, is still controlled
  (Q1-R1-NC1-FOREIGN-KILLED, unchanged), and a collision confirmed without
  its observation is NC-R3-10.
- Q1-R1-NC2-STOP-REPLY-PROOF: its anchor, the StopUnit call in settling, no
  longer exists, and no StopUnit reply exists to be taken as proof
  (P2-V1-R3B-I4-R2 had retargeted it to i4r2_05, the one path that still
  stopped by name; R3 removes that path) -> NC-R3-01, NC-R3-02; the uncertain
  first start's helper and unconfirmed state stay controlled by
  Q1-R1-NC2-UNRESOLVED-REAPED (unchanged).

Usage: as r1_controls.py.
"""
import hashlib
import importlib.util
import json
import pathlib
import sys

sys.dont_write_bytecode = True

RUNNER = "docs/evidence/p2-v1-r3b-i4-q1-r1-host-qualification-ownership/scripts/r1_controls.py"
RUNNER_SHA256 = "eea344ea8394fbc6f0eb46c6b8db3b21d3f3bf728eb38b61ffdecd2c1a93a3c0"
SUPERSEDED = {
    "Q1-R1-NC1-FOREIGN-STOPPED": dict(
        r3_controls=["NC-R3-02-MANAGER-STOP-RESTORED", "NC-R3-01-STOPUNIT-IN-RECONCILE",
                     "NC-R3-10-COLLISION-WEAKENED"],
        reason="its mutation calls the manager's stop_unit, which R3 removed with the request (the "
               "mutation cannot compile); no unit can be stopped by its name without restoring the "
               "request; Q1-R1-NC1-FOREIGN-KILLED (unchanged) still controls the foreign cgroup taken "
               "and killed."),
    "Q1-R1-NC2-STOP-REPLY-PROOF": dict(
        r3_controls=["NC-R3-01-STOPUNIT-IN-RECONCILE", "NC-R3-02-MANAGER-STOP-RESTORED"],
        reason="its anchor, the StopUnit call in settling, no longer exists, and no StopUnit reply "
               "exists to be taken as proof (R2 had retargeted it to i4r2_05, a path R3 removes); "
               "Q1-R1-NC2-UNRESOLVED-REAPED (unchanged) still controls the uncertain first start."),
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
    superseded = [c["id"] for c in module.CONTROLS if c["id"] in SUPERSEDED]
    if sorted(superseded) != sorted(SUPERSEDED):
        raise SystemExit(f"superseded controls not found: {sorted(set(SUPERSEDED) - set(superseded))}")
    module.CONTROLS = [c for c in module.CONTROLS if c["id"] not in SUPERSEDED]
    print(json.dumps({"runner": RUNNER, "runner_sha256": digest, "accepted_controls": 15,
                      "run": len(module.CONTROLS), "r3_superseded": SUPERSEDED}), flush=True)
    return module.main()


if __name__ == "__main__":
    sys.exit(main())

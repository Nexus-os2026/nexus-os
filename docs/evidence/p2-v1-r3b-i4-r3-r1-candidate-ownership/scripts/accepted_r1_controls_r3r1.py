#!/usr/bin/env python3
"""P2-V1-R3B-I4-R3-R1: the accepted P2-V1-R3B-I4-Q1-R1 mutation controls
(15), rerun exactly as P2-V1-R3B-I4-R3 accepted them: R3's adapter
(accepted_r1_controls_r3.py) runs unchanged, and it runs the accepted Q1-R1
runner unchanged (both SHA-256 verified before they are imported). R3's two
supersessions stay as recorded; once R3's adapter has applied them, and just
before the runner's own main runs, R3-R1 adapts one control (the first,
unadapted run is kept as evidence):

- Q1-R1-NC1-FOREIGN-KILLED (a first start refused as loaded has the foreign
  unit's cgroup taken and ended): its accepted mutation calls `acquire()` in
  the collided arm, which until R3 opened the cgroup the helper is in and
  ended it. R3-R1 takes that power away from `acquire()`: it only observes,
  and only for an operation whose start's identity was captured, which a
  collision never has; the unadapted run shows the accepted mutation is now
  harmless (its test passes). The same harm is restored by an equivalent
  mutation in the same place: as `acquire()` did until R3, a cgroup of the
  unit's name the kernel reports the helper in is opened and ended through
  its descriptor in the collided arm. Same anchor, same test, same marker.

Every other control runs exactly as R3 accepted it. Usage: as r1_controls.py.
"""
import hashlib
import importlib.util
import json
import pathlib
import sys

sys.dont_write_bytecode = True

ADAPTER = "docs/evidence/p2-v1-r3b-i4-r3-no-name-actuation/scripts/accepted_r1_controls_r3.py"
ADAPTER_SHA256 = "4f66337b276e241fd5de78a3b62b9c1ddf99c5628732ebac51481f834963f862"
RUNNER = "docs/evidence/p2-v1-r3b-i4-q1-r1-host-qualification-ownership/scripts/r1_controls.py"
RUNNER_SHA256 = "eea344ea8394fbc6f0eb46c6b8db3b21d3f3bf728eb38b61ffdecd2c1a93a3c0"
ACCEPTED_CALL = "            self.acquire(&controller, helper);\n"
TAKEN_AND_ENDED = ("            if let Some(Ok(Some(path))) =\n"
                   "                helper.map(|helper| controller.native.membership(helper))\n"
                   "            {\n"
                   "                if names_unit(&path, &self.unit) {\n"
                   "                    if let Ok(taken) = controller.native.open(&path) {\n"
                   "                        let _ = taken.kill();\n"
                   "                    }\n"
                   "                }\n"
                   "            }\n")
REASON = ("an equivalent mutation: R3-R1's acquire() only observes, and only with a captured identity "
          "(a collision has none), so the accepted call is now harmless (the unadapted run's test "
          "passes); the same harm (the cgroup the helper is in taken and ended in the collided arm) is "
          "restored, as acquire() did until R3, for a cgroup of the unit's name the kernel reports the "
          "helper in: opened and ended through its descriptor there. Same anchor, test and marker.")


def adapt(module):
    (control,) = [c for c in module.CONTROLS if c["id"] == "Q1-R1-NC1-FOREIGN-KILLED"]
    (edit,) = control["edits"]
    if ACCEPTED_CALL not in edit["replacement"]:
        raise SystemExit("Q1-R1-NC1-FOREIGN-KILLED: the accepted mutation is not as recorded")
    edit["replacement"] = edit["replacement"].replace(ACCEPTED_CALL, TAKEN_AND_ENDED)
    control["what"] = f"{control['what']} [R3-R1-adapted: {REASON}]"
    print(json.dumps({"r3r1_adapted": {"Q1-R1-NC1-FOREIGN-KILLED": REASON},
                      "run": len(module.CONTROLS)}), flush=True)


def main():
    if len(sys.argv) < 3:
        raise SystemExit(__doc__)
    root = pathlib.Path(sys.argv[1]).resolve()
    for path, expected in ((ADAPTER, ADAPTER_SHA256), (RUNNER, RUNNER_SHA256)):
        digest = hashlib.sha256((root / path).read_bytes()).hexdigest()
        if digest != expected:
            raise SystemExit(f"{path} changed: {digest}")
    original = importlib.util.spec_from_file_location
    spec = original("accepted_r1_controls_r3", root / ADAPTER)
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

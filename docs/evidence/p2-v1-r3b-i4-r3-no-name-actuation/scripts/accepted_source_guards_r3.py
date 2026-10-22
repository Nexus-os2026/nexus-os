#!/usr/bin/env python3
"""P2-V1-R3B-I4-R3: the accepted I4-R1 source guards (14, each with its
self-tests), rerun through the accepted script itself, unchanged (its
SHA-256 is verified before it is imported), with one guard adapted to the
manager R3 leaves and one R3 guard added.

- SG-I4-DESTINATION pinned the manager interface with StopUnit (seven
  internal calls, a `stop_unit` signature). R3 removes that request: the
  adapted guard is the accepted function with exactly those two pins
  changed (six internal calls; the interface without `stop_unit`), every
  other check as accepted. Its self-test that changed `stop_unit`'s
  signature (its anchor no longer exists) is replaced by the same injection
  on `get_unit` (a destination parameter added), and one self-test restores
  `stop_unit` in the interface.
- SG-I4R3-NO-NAME-ACTUATION (added): no production source names a request
  that acts on a unit (StopUnit, KillUnit and the like), nor what served
  the stop by name (`stop_unit`, `stop_authorized`, `last_stop`, the stop
  fault points); the production manager issues exactly StartTransientUnit,
  GetUnit and four property reads; settling (`reconcile`, `observe`,
  `acquire`, `gone`, the drop backstop `end_now`) asks the manager nothing,
  and `absent` asks it exactly GetUnit. Self-tests restore each.

Every other guard and self-test runs exactly as accepted. Usage: as
source_guards.py (<checkout> <output json>).
"""
import hashlib
import importlib.util
import json
import pathlib
import re
import sys

sys.dont_write_bytecode = True

SCRIPT = "docs/evidence/p2-v1-r3b-i4-r1-native-scope/scripts/source_guards.py"
SCRIPT_SHA256 = "b3c90101178be7c508236fddb12904fc163b9b071dc92031764759afc280db84"
STOP_SIGNATURE = "fn stop_unit(&self, unit: &str) -> Remote<()>;"
ACCEPTED_STOP_SELF_TEST = ("SG-I4-DESTINATION", "scope/manager.rs",
                           "    fn stop_unit(&self, unit: &str) -> Remote<()>;",
                           "    fn stop_unit(&self, destination: &str, unit: &str) -> Remote<()>;")
TRAIT_GET_UNIT = "    fn get_unit(&self, unit: &str) -> Remote<Presence>;"
IMPL_GET_UNIT = "    fn get_unit(&self, unit: &str) -> Remote<Presence> {\n"
OBSERVE = "        self.observe(&controller, helper, fault)\n    }\n"
ACTING = ["StopUnit", "KillUnit", "RestartUnit", "ReloadUnit", "ResetFailedUnit", "AbandonScope",
          "QueueSignalUnit", "FreezeUnit", "ThawUnit", "SetUnitProperties", "AttachProcessesToUnit",
          "EnqueueUnitJob", "CleanUnit", '"Stop"', '"Kill"', '"Restart"', '"Set"', '"Unref"',
          "stop_unit", "kill_unit", "stop_authorized", "last_stop", "BeforeScopeStop",
          "AfterScopeStop"]
SETTLING = ["    pub(crate) fn reconcile(", "    fn observe(", "    fn acquire(", "    fn gone(",
            "    pub(crate) fn end_now("]
REQUESTS = [("SYSTEMD_PATH", "MANAGER", "StartTransientUnit"), ("SYSTEMD_PATH", "MANAGER", "GetUnit"),
            ("unit_path", "PROPERTIES", "Get"), ("unit_path", "PROPERTIES", "Get"),
            ("unit_path", "PROPERTIES", "Get"), ("unit_path", "PROPERTIES", "Get")]


def load():
    if len(sys.argv) != 3:
        raise SystemExit(__doc__)
    root = pathlib.Path(sys.argv[1]).resolve()
    path = root / SCRIPT
    digest = hashlib.sha256(path.read_bytes()).hexdigest()
    if digest != SCRIPT_SHA256:
        raise SystemExit(f"the accepted source guards changed: {digest}")
    spec = importlib.util.spec_from_file_location("i4r1_source_guards", path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module, digest


def adapt(module):
    code, count = module.code, module.count
    if STOP_SIGNATURE not in module.TRAIT_SIGNATURES:
        raise SystemExit("the accepted interface pin is not as recorded")
    signatures_r3 = [s for s in module.TRAIT_SIGNATURES if s != STOP_SIGNATURE]

    def guard_destination(files):
        # The accepted guard_destination, with exactly two pins changed: six
        # internal calls (StopUnit's removed), the interface without stop_unit.
        problems = []
        manager = code(files["scope/manager.rs"])
        for constant in module.DESTINATION_CONSTANTS:
            if manager.count(constant) != 1:
                problems.append(f"constant: {constant}")
        if count(files, "call_method(") != {"scope/manager.rs": 1}:
            problems.append(f"call_method: {count(files, 'call_method(')}")
        if ".call_method(Some(SYSTEMD),path,Some(interface),method,body)" not in re.sub(r"\s+", "", manager):
            problems.append("call_method does not pass the fixed destination")
        if re.search(r"pub(\([^)]*\))?\s+fn call<", manager):
            problems.append("the internal call is not private")
        calls = re.findall(r"self\.call::<[^>]*(?:<[^>]*>)?[^>]*>\(\s*([^,]+),\s*([^,]+),", manager)
        if len(calls) != 6:
            problems.append(f"internal calls: {len(calls)}")
        for path, interface in calls:
            pair = (path.strip(), interface.strip())
            if pair not in {("SYSTEMD_PATH", "MANAGER"), ("unit_path", "PROPERTIES")}:
                problems.append(f"a call to {pair}")
        block = manager[manager.find("pub(crate) trait Manager"):]
        block = block[:block.find("\n}") + 2]
        signatures = sorted(re.sub(r"\s+", " ", s).strip() + ";"
                            for s in re.findall(r"(fn [^;]+);", block))
        if signatures != sorted(signatures_r3):
            problems.append(f"manager interface: {signatures}")
        return problems

    def guard_no_name_actuation(files):
        problems = []
        for needle in ACTING:
            hits = count(files, needle)
            if hits:
                problems.append(f"{needle}: {hits}")
        manager = code(files["scope/manager.rs"])
        requests = [tuple(part.strip() for part in found) for found in re.findall(
            r"self\.call::<[^>]*(?:<[^>]*>)?[^>]*>\(\s*([^,]+),\s*([^,]+),\s*\"([^\"]*)\"", manager)]
        if requests != REQUESTS or manager.count("self.call") != 6:
            problems.append(f"manager requests: {requests}")
        pending = code(files["scope/pending.rs"])
        for head in SETTLING:
            bodies = [rest.split("\n    }\n")[0] for rest in pending.split(head)[1:]]
            if not bodies:
                problems.append(f"not found: {head.strip()}")
            for body in bodies:
                if "manager" in body:
                    problems.append(f"{head.strip()} asks the manager")
        absent = pending.split("\nfn absent(")[1:]
        if len(absent) != 1 or absent[0].split("\n}\n")[0].count("manager") != 1 or \
                "controller.manager.get_unit(unit)," not in absent[0].split("\n}\n")[0]:
            problems.append("absent() asks the manager more than GetUnit")
        return problems

    guards = []
    for gid, guard in module.GUARDS:
        guards.append((gid, guard_destination if gid == "SG-I4-DESTINATION" else guard))
    guards.append(("SG-I4R3-NO-NAME-ACTUATION", guard_no_name_actuation))
    module.GUARDS = guards
    if ACCEPTED_STOP_SELF_TEST not in module.SELF_TESTS:
        raise SystemExit("the accepted stop_unit self-test is not as recorded")
    self_tests = [t for t in module.SELF_TESTS if t != ACCEPTED_STOP_SELF_TEST]
    self_tests += [
        ("SG-I4-DESTINATION", "scope/manager.rs", TRAIT_GET_UNIT,
         "    fn get_unit(&self, destination: &str, unit: &str) -> Remote<Presence>;"),
        ("SG-I4-DESTINATION", "scope/manager.rs", TRAIT_GET_UNIT,
         "    fn stop_unit(&self, unit: &str) -> Remote<()>;\n" + TRAIT_GET_UNIT),
        ("SG-I4R3-NO-NAME-ACTUATION", "scope/pending.rs", OBSERVE,
         "        let _ = self.controller.manager.get_unit(&self.unit);\n" + OBSERVE),
        ("SG-I4R3-NO-NAME-ACTUATION", "scope/pending.rs",
         "    pub(crate) fn end_now(&self) {\n        if self.unresolved() {\n",
         "    pub(crate) fn end_now(&self) {\n        if self.unresolved() {\n"
         "            let _ = self.controller.manager.get_unit(&self.unit);\n"),
        ("SG-I4R3-NO-NAME-ACTUATION", "scope/manager.rs", IMPL_GET_UNIT,
         "    fn stop_unit(&self, unit: &str) -> Remote<()> {\n"
         "        match self.call::<_, zbus::zvariant::OwnedObjectPath>(\n"
         "            SYSTEMD_PATH,\n            MANAGER,\n            \"StopUnit\",\n"
         "            &(unit, \"replace\"),\n        ) {\n"
         "            Ok(Ok(_job)) => Remote::Answered(()),\n"
         "            Ok(Err(name)) => Remote::Uncertain(format!(\"StopUnit: {name}\")),\n"
         "            Err(reason) => Remote::Uncertain(reason),\n        }\n    }\n\n" + IMPL_GET_UNIT),
        ("SG-I4R3-NO-NAME-ACTUATION", "scope/manager.rs", "            \"GetUnit\",\n",
         "            \"KillUnit\",\n"),
    ]
    module.SELF_TESTS = self_tests


def main():
    module, digest = load()
    adapt(module)
    print(json.dumps({"script": SCRIPT, "script_sha256": digest,
                      "r3_adapted_guard": "SG-I4-DESTINATION",
                      "r3_added_guard": "SG-I4R3-NO-NAME-ACTUATION",
                      "r3_replaced_self_test": list(ACCEPTED_STOP_SELF_TEST)}), flush=True)
    return module.main()


if __name__ == "__main__":
    sys.exit(main())

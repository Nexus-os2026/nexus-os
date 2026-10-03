#!/usr/bin/env python3
"""P2-V1-R3B-I4-R3-R1: the accepted I4-R1 source guards (14, with R3's
adaptation and R3's added guard), rerun exactly as P2-V1-R3B-I4-R3
accepted them: R3's adapter (accepted_source_guards_r3.py) runs unchanged,
and it runs the accepted I4-R1 script unchanged (both SHA-256 verified
before they are imported). Once R3's adapter has applied its changes, and
just before the script's own main runs, R3-R1 adapts the five guards whose
pins R3-R1's code changes, each keeping its accepted invariant, adds one
guard, and replaces the two self-tests whose anchors R3-R1 moved (the
first, unadapted run is kept as evidence):

- SG-I4-DESTINATION: the fixed destination and one bounded call, as
  accepted; the pins now count the manager's requests through that call as
  R3-R1 makes them: Subscribe, GetUnit and six property reads through
  `self.call`, StartTransientUnit through `self.call_reply` (the same call,
  with the reply message kept for its serial and sender), and the interface
  with `unit_instance` and `control_group_id` added.
- SG-I4-ONE-WAY: a pending operation is constructed in one place and a
  proven scope only by promotion, once, as accepted; the promotion's
  literal is R3-R1's (the owned candidate's descriptor and the identity it
  is bound to).
- SG-I4R1-BINDING: Pending becomes Proven only after the exact binding, as
  accepted; R3-R1 binds in `bind` (the identity, `Id`, `ControlGroup`,
  `ControlGroupId`, the identity again), called by the proof before the
  policy reads, so the order, the exact comparisons, the refusal of every
  uncertain answer and the absence of normalization are checked across the
  proof, the binding and the identity comparison.
- SG-I4R1-UNCERTAIN: a start is accepted only in the delivered reply's arm
  (R3-R1's arm carries the captured identity), after the post-dispatch
  fault point; the manager's absence is weighed only for an accepted start
  (R3-R1: `self.accepted && absent(...)`); the identity is recorded only in
  that same arm.
- SG-I4R3-NO-NAME-ACTUATION: no request acts on a unit by its name, as R3
  added it; the manager's requests are R3-R1's (Subscribe, which acts on no
  unit, added); settling, observing, seeking ownership, ending an owned
  candidate and the drop backstop ask the manager nothing themselves.
- SG-R3R1-CANDIDATE-OWNERSHIP (added): an observed candidate is never
  ended: the one `cgroup.kill` of a pending operation goes through the owned
  candidate (`owned_dir`); ownership is set once, in `bind`, after the
  closing identity read; candidates are made unowned, only where they are
  opened; only an operation with a captured identity retains one; the
  uncertain arm returns at once; a proven scope is made only from an owned
  candidate after the promotion point; the identity is made only from the
  manager's value, and the capture rule requires `done`, a removal after
  the reply and exactly one identity.

Usage: as source_guards.py (<checkout> <output json>).
"""
import hashlib
import importlib.util
import json
import pathlib
import re
import sys

sys.dont_write_bytecode = True

ADAPTER = "docs/evidence/p2-v1-r3b-i4-r3-no-name-actuation/scripts/accepted_source_guards_r3.py"
ADAPTER_SHA256 = "9aa1493e82a8bbaae976f1b2680f45c711a0a84f321bc9a3d3f6ed421dfd1c3e"
SCRIPT = "docs/evidence/p2-v1-r3b-i4-r1-native-scope/scripts/source_guards.py"
SCRIPT_SHA256 = "b3c90101178be7c508236fddb12904fc163b9b071dc92031764759afc280db84"
R3R1_SIGNATURES = [
    "fn unit_instance(&self, unit_path: &str) -> Remote<Option<UnitInstance>>;",
    "fn control_group_id(&self, unit_path: &str) -> Remote<Option<u64>>;",
]
REQUESTS = [("SYSTEMD_PATH", "MANAGER", "Subscribe"), ("SYSTEMD_PATH", "MANAGER", "StartTransientUnit"),
            ("SYSTEMD_PATH", "MANAGER", "GetUnit")] + [("unit_path", "PROPERTIES", "Get")] * 6
SETTLING = ["    pub(crate) fn reconcile(", "    fn observe(", "    fn acquire(", "    fn own(",
            "    fn gone(", "    fn end_owned(", "    pub(crate) fn end_now("]
PROMOTION = "Self::Proven(Scope { unit, dir: candidate.dir, instance, })"
REPLACED_SELF_TESTS = {
    ("SG-I4R1-BINDING", "scope/pending.rs", "Remote::Answered(Some(group)) if group == path => {}"):
        ("Remote::Answered(Some(group)) if group == candidate.path => {}",
         "Remote::Answered(Some(group)) if group.ends_with(&self.unit) => {}"),
    ("SG-I4R1-UNCERTAIN", "scope/pending.rs", "            None if !self.accepted => false,\n"):
        ("        self.accepted && absent(controller, &self.unit, helper)\n",
         "        absent(controller, &self.unit, helper)\n"),
}


def body(source, head, end="\n    }\n"):
    """The text of the first item `head` opens in `source`, up to `end`."""
    at = source.find(head)
    if at < 0:
        return ""
    rest = source[at:]
    return rest[:rest.find(end) + len(end)]


def adapt(module):
    code, count = module.code, module.count
    accepted = dict(module.GUARDS)

    def guard_destination(files):
        # The accepted (R3-adapted) guard_destination, with R3-R1's pins.
        problems = []
        manager = code(files["scope/manager.rs"])
        for constant in module.DESTINATION_CONSTANTS:
            if manager.count(constant) != 1:
                problems.append(f"constant: {constant}")
        if count(files, "call_method(") != {"scope/manager.rs": 1}:
            problems.append(f"call_method: {count(files, 'call_method(')}")
        if ".call_method(Some(SYSTEMD),path,Some(interface),method,body)" not in re.sub(r"\s+", "", manager):
            problems.append("call_method does not pass the fixed destination")
        if re.search(r"pub(\([^)]*\))?\s+fn call(_reply)?<", manager):
            problems.append("the internal call is not private")
        if "self.call_reply(path, interface, method, body)" not in manager:
            problems.append("call does not go through call_reply")
        calls = re.findall(r"self\.call(?:_reply)?::<[^>]*(?:<[^>]*>)?[^>]*>\(\s*([^,]+),\s*([^,]+),", manager)
        if len(calls) != 9:
            problems.append(f"internal calls: {len(calls)}")
        for path, interface in calls:
            pair = (path.strip(), interface.strip())
            if pair not in {("SYSTEMD_PATH", "MANAGER"), ("unit_path", "PROPERTIES")}:
                problems.append(f"a call to {pair}")
        block = manager[manager.find("pub(crate) trait Manager"):]
        block = block[:block.find("\n}") + 2]
        signatures = sorted(re.sub(r"\s+", " ", s).strip() + ";"
                            for s in re.findall(r"(fn [^;]+);", block))
        expected = sorted([s for s in module.TRAIT_SIGNATURES if "stop_unit" not in s] + R3R1_SIGNATURES)
        if signatures != expected:
            problems.append(f"manager interface: {signatures}")
        return problems

    def guard_one_way(files):
        problems = []
        if count(files, "PendingScope::new(") != {"scope.rs": 1}:
            problems.append(f"PendingScope::new: {count(files, 'PendingScope::new(')}")
        flat = {name: re.sub(r"\s+", " ", code(text)) for name, text in files.items()}
        promotions = {name: text.count(PROMOTION) for name, text in flat.items() if PROMOTION in text}
        if promotions != {"scope/pending.rs": 1}:
            problems.append(f"a proven scope is built other than by promotion: {promotions}")
        literals = {name: len(re.findall(r"(?<!struct )(?<!impl )(?<!-> )\bScope \{", text))
                    for name, text in flat.items()}
        if {name: n for name, n in literals.items() if n} != {"scope/pending.rs": 1}:
            problems.append(f"Scope literals: {literals}")
        if re.search(r"pub\s+fn\s+\w+\([^)]*unit:\s*&?str", code(files["scope.rs"] + files["scope/pending.rs"])):
            problems.append("a public function takes a unit name")
        return problems

    def guard_binding(files):
        problems = []
        pending = code(files["scope/pending.rs"])
        prove = body(pending, "    fn prove(")
        bind = body(pending, "    fn bind(")
        same = body(pending, "\nfn same_instance(", "\n}\n")
        order = ["controller.native.open(&path)?", "self.bind(controller, helper, fault)?",
                 "controller.manager.runtime_max_usec(&unit_path)", "controller.manager.oom_policy(&unit_path)",
                 "same_instance(controller, &unit_path, instance)"]
        at = [prove.find(step) for step in order]
        if -1 in at or at != sorted(at):
            problems.append(f"the proof's order: {dict(zip(order, at))}")
        steps = ["controller.native.membership(helper)", "controller.manager.get_unit(&self.unit)",
                 "same_instance(controller, &unit_path, instance)?", "controller.manager.unit_id(&unit_path)",
                 "controller.manager.control_group(&unit_path)", "controller.manager.control_group_id(&unit_path)",
                 "candidate.owned = true;"]
        flat_bind = re.sub(r"\s+", "", bind)
        at = [flat_bind.find(re.sub(r"\s+", "", step)) for step in steps]
        if -1 in at or at != sorted(at) or bind.count("same_instance(controller, &unit_path, instance)?") != 2 \
                or bind.rfind("same_instance(") > bind.find("candidate.owned = true;"):
            problems.append(f"the binding's order: {dict(zip(steps, at))}")
        for needle, where in [("Remote::Answered(Some(id)) if id == self.unit => {}", bind),
                              ("Remote::Answered(Some(group)) if group == candidate.path => {}", bind),
                              ("Remote::Answered(Some(id)) if id == retained => {}", bind),
                              ('Remote::Answered(_) => return Err(ScopeError::Mismatch("unit id")),', bind),
                              ('Remote::Answered(_) => return Err(ScopeError::Mismatch("unit control group")),', bind),
                              ('Remote::Answered(_) => return Err(ScopeError::Mismatch("unit control group id")),', bind),
                              ("Remote::Answered(Some(current)) if current == instance => Ok(()),", same),
                              ('Remote::Answered(_) => Err(ScopeError::Mismatch("unit instance")),', same)]:
            if where.count(needle) != 1:
                problems.append(f"not exactly once: {needle}")
        uncertain = "Remote::Uncertain(reason) => return Err(ScopeError::Bus(reason)),"
        if bind.count(uncertain) != 4 or prove.count(uncertain) != 2 or \
                same.count("Remote::Uncertain(reason) => Err(ScopeError::Bus(reason)),") != 1:
            problems.append("an uncertain answer is not refused in every step")
        for needle in ["canonicalize", "ends_with", "starts_with", "trim", "to_lowercase",
                       "eq_ignore", "rsplit", "split", "strip_", "contains", "Path::new", "PathBuf"]:
            if needle in bind or needle in same:
                problems.append(f"the binding uses {needle}")
        return problems

    def guard_uncertain(files):
        problems = []
        pending = code(files["scope/pending.rs"])
        if pending.count("self.accepted = true;") != 1 or not re.search(
                r"Started::Accepted\(captured\) => \{\s*self\.accepted = true;\s*"
                r"(//[^\n]*\n\s*)*self\.instance = Some\(captured\.map_err\(ScopeError::Bus\)\?\);", pending):
            problems.append("a start is accepted, or its identity recorded, other than in the delivered reply's arm")
        if pending.count("self.instance = ") != 1:
            problems.append("the identity is recorded other than in the delivered reply's arm")
        gone = body(pending, "    fn gone(")
        if "self.accepted && absent(controller, &self.unit, helper)" not in gone:
            problems.append("the manager's absence is consulted without an accepted start")
        if pending.count("absent(") != 2:  # its definition and its one use, in gone()
            problems.append(f"absent( occurs {pending.count('absent(')} times")
        if "fault::at(fault, FaultPoint::AfterScopeStart);" not in pending or \
                pending.find("fault::at(fault, FaultPoint::AfterScopeStart);") > pending.find("self.accepted = true;"):
            problems.append("the reply is recorded before the post-dispatch fault point")
        return problems

    def guard_no_name_actuation(files):
        problems = []
        no_name = accepted["SG-I4R3-NO-NAME-ACTUATION"]
        # R3's guard, with R3-R1's requests: its needles still apply.
        for problem in no_name(files):
            if not problem.startswith("manager requests:") and not problem.startswith("not found:"):
                problems.append(problem)
        manager = code(files["scope/manager.rs"])
        requests = [tuple(part.strip() for part in found) for found in re.findall(
            r"self\.call(?:_reply)?::<[^>]*(?:<[^>]*>)?[^>]*>\(\s*([^,]+),\s*([^,]+),\s*\"([^\"]*)\"", manager)]
        if requests != REQUESTS or manager.count("self.call") != 10:
            problems.append(f"manager requests: {requests}")
        pending = code(files["scope/pending.rs"])
        for head in SETTLING:
            bodies = [rest.split("\n    }\n")[0] for rest in pending.split(head)[1:]]
            if not bodies:
                problems.append(f"not found: {head.strip()}")
            for found in bodies:
                if "manager" in found:
                    problems.append(f"{head.strip()} asks the manager")
        bind = body(pending, "    fn bind(")
        asked = sorted(set(re.findall(r"controller\.manager\.(\w+)\(", bind)))
        if asked != ["control_group", "control_group_id", "get_unit", "unit_id"]:
            problems.append(f"bind asks the manager: {asked}")
        return problems

    def guard_candidate_ownership(files):
        problems = []
        pending = code(files["scope/pending.rs"])
        manager = code(files["scope/manager.rs"])
        kills = re.findall(r"let _ = (\w+)\.kill\(\);", pending)
        if sorted(kills) != ["owned", "scope"] or pending.count(".kill()") != 2:
            problems.append(f"cgroup.kill other than through the owned candidate: {kills}")
        if "let _ = owned.kill();" not in body(pending, "    fn end_owned(") or \
                "if let Some(owned) = self.owned_dir() {" not in body(pending, "    fn end_owned("):
            problems.append("end_owned ends other than the owned candidate")
        if ".filter(|candidate| candidate.owned)" not in body(pending, "    fn owned_dir("):
            problems.append("owned_dir yields an unowned candidate")
        if "self.end_owned();" not in body(pending, "    pub(crate) fn end_now(") or \
                "kill" in body(pending, "    pub(crate) fn end_now("):
            problems.append("the drop backstop ends other than the owned candidate")
        if pending.count("owned = true") != 1 or not body(pending, "    fn bind(").rstrip().endswith(
                "same_instance(controller, &unit_path, instance)?;\n        candidate.owned = true;\n        Ok(unit_path)\n    }"):
            problems.append("ownership is set other than once the binding closed")
        if pending.count("owned: false,") != 2 or pending.count("owned: true") != 0:
            problems.append("a candidate is made owned")
        opens = [head for head in ("    fn acquire(", "    fn prove(")
                 if "controller.native.open(" in body(pending, head)]
        if pending.count("controller.native.open(") != 2 or opens != ["    fn acquire(", "    fn prove("]:
            problems.append("a candidate is opened other than where it is observed")
        if "if self.candidate.is_some() || self.instance.is_none() {" not in body(pending, "    fn acquire("):
            problems.append("a candidate is retained without a captured identity")
        if "            Started::Uncertain(reason) => return Err(ScopeError::Bus(reason)),\n" not in \
                files["scope/pending.rs"]:
            problems.append("an uncertain start goes on to the proof")
        establish = body(pending, "    pub(crate) fn establish(")
        steps = ["pending.issue_and_prove(helper, fault)?;", "fault::at(fault, FaultPoint::ScopePromotion);",
                 "take_if(|candidate| candidate.owned)", "*self = Self::Proven(Scope {"]
        at = [establish.find(step) for step in steps]
        if -1 in at or at != sorted(at):
            problems.append(f"the promotion's order: {dict(zip(steps, at))}")
        if manager.count("UnitInstance::from_property(") != 1 or \
                "UnitInstance::from_property(&bytes)" not in body(manager, "pub(crate) fn invocation_of(", "\n}\n"):
            problems.append("an identity is made other than from the manager's value")
        capture = body(manager, "pub(crate) fn captured_instance(", "\n}\n")
        for needle in ['if result != "done" {', "if removed_serial <= reply_serial {",
                       "change.serial > reply_serial && change.serial < removed_serial",
                       "Some(seen) if seen != id =>", "Invocation::Malformed => return Err("]:
            if needle not in capture:
                problems.append(f"the capture rule lacks: {needle}")
        return problems

    replaced = {"SG-I4-DESTINATION": guard_destination, "SG-I4-ONE-WAY": guard_one_way,
                "SG-I4R1-BINDING": guard_binding, "SG-I4R1-UNCERTAIN": guard_uncertain,
                "SG-I4R3-NO-NAME-ACTUATION": guard_no_name_actuation}
    module.GUARDS = [(gid, replaced.get(gid, guard)) for gid, guard in module.GUARDS] + [
        ("SG-R3R1-CANDIDATE-OWNERSHIP", guard_candidate_ownership)]
    found, tests = set(), []
    for test in module.SELF_TESTS:
        swap = REPLACED_SELF_TESTS.get(test[:3])
        if swap is not None:
            found.add(test[:3])
            test = (test[0], test[1]) + swap
        tests.append(test)
    if found != set(REPLACED_SELF_TESTS):
        raise SystemExit(f"the accepted self-tests are not as recorded: {sorted(found)}")
    tests += [
        ("SG-I4R3-NO-NAME-ACTUATION", "scope/manager.rs", 'MANAGER, "Subscribe", &()',
         'MANAGER, "Unsubscribe", &()'),
        ("SG-I4-DESTINATION", "scope/manager.rs",
         "    fn control_group_id(&self, unit_path: &str) -> Remote<Option<u64>>;",
         "    fn control_group_id(&self, unit: &str) -> Remote<Option<u64>>;"),
        ("SG-I4-ONE-WAY", "scope/pending.rs", "fn names_unit(",
         "fn forged(dir: Box<dyn CgroupDir>, instance: UnitInstance) -> Scope {\n"
         "    Scope { unit: String::new(), dir, instance }\n}\nfn names_unit("),
        ("SG-I4R1-BINDING", "scope/pending.rs",
         "            Remote::Answered(Some(id)) if id == retained => {}",
         "            Remote::Answered(Some(_)) => {}"),
        ("SG-I4R1-BINDING", "scope/pending.rs",
         "        Remote::Answered(Some(current)) if current == instance => Ok(()),",
         "        Remote::Answered(Some(_)) => Ok(()),"),
        ("SG-I4R1-UNCERTAIN", "scope/pending.rs",
         "            Started::Uncertain(reason) => return Err(ScopeError::Bus(reason)),\n",
         "            Started::Uncertain(reason) => {\n"
         "                self.instance = Some(UnitInstance::from_property(&[1; 16]).unwrap());\n"
         "                return Err(ScopeError::Bus(reason));\n            }\n"),
        ("SG-I4R3-NO-NAME-ACTUATION", "scope/pending.rs",
         "    fn own(&mut self, controller: &Controller, helper: Option<&Helper>) -> bool {\n",
         "    fn own(&mut self, controller: &Controller, helper: Option<&Helper>) -> bool {\n"
         "        let _ = controller.manager.get_unit(&self.unit);\n"),
        ("SG-R3R1-CANDIDATE-OWNERSHIP", "scope/pending.rs",
         "                    self.candidate = Some(Candidate {\n                        dir,\n"
         "                        path,\n                        owned: false,\n",
         "                    self.candidate = Some(Candidate {\n                        dir,\n"
         "                        path,\n                        owned: true,\n"),
        ("SG-R3R1-CANDIDATE-OWNERSHIP", "scope/pending.rs",
         "        fault::at(fault, FaultPoint::ScopeCandidate);\n",
         "        fault::at(fault, FaultPoint::ScopeCandidate);\n        let _ = candidate.dir.kill();\n"),
        ("SG-R3R1-CANDIDATE-OWNERSHIP", "scope/pending.rs",
         "        if self.candidate.is_some() || self.instance.is_none() {\n",
         "        if self.candidate.is_some() {\n"),
        ("SG-R3R1-CANDIDATE-OWNERSHIP", "scope/manager.rs",
         '    if result != "done" {', '    if result.is_empty() {'),
        ("SG-R3R1-CANDIDATE-OWNERSHIP", "scope/pending.rs",
         "        let Some(candidate) = pending.candidate.take_if(|candidate| candidate.owned) else {",
         "        let Some(candidate) = pending.candidate.take() else {"),
    ]
    module.SELF_TESTS = tests
    print(json.dumps({"r3r1_adapted_guards": sorted(replaced), "r3r1_added_guard": "SG-R3R1-CANDIDATE-OWNERSHIP",
                      "r3r1_replaced_self_tests": [list(k) for k in REPLACED_SELF_TESTS],
                      "self_tests": len(tests)}), flush=True)


def main():
    if len(sys.argv) != 3:
        raise SystemExit(__doc__)
    root = pathlib.Path(sys.argv[1]).resolve()
    for path, expected in ((ADAPTER, ADAPTER_SHA256), (SCRIPT, SCRIPT_SHA256)):
        digest = hashlib.sha256((root / path).read_bytes()).hexdigest()
        if digest != expected:
            raise SystemExit(f"{path} changed: {digest}")
    original = importlib.util.spec_from_file_location
    spec = original("accepted_source_guards_r3", root / ADAPTER)
    r3 = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(r3)
    target = (root / SCRIPT).resolve()

    def spec_from_file_location(name, location, *args, **kwargs):
        found = original(name, location, *args, **kwargs)
        if pathlib.Path(location).resolve() == target:
            exec_module = found.loader.exec_module

            def hooked(module):
                exec_module(module)
                script_main = module.main

                def adapted_main():
                    adapt(module)
                    return script_main()

                module.main = adapted_main

            found.loader.exec_module = hooked
        return found

    importlib.util.spec_from_file_location = spec_from_file_location
    return r3.main()


if __name__ == "__main__":
    sys.exit(main())

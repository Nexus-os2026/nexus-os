#!/usr/bin/env python3
"""P2-V1-R3B-I4-R3-R1: the accepted I4-R1 harness-build API/type guards,
rerun through the accepted runner itself, unchanged (its SHA-256 is verified
before it is imported). Two probes reach what R3-R1 changed; each keeps its
attempt, its guard and its self-check, adapted to R3-R1's types (the first,
unadapted run is kept as evidence: the runner stops at the first reopen
anchor it no longer finds):

- P-I4-PENDING-CANDIDATE (drop or replace a pending operation's retained
  candidate from outside): R3-R1's candidate field is `Option<Candidate>`,
  a crate-private struct holding the descriptor, its path and its
  ownership. The probe and its expected errors are unchanged (the pending
  operation's privacy, the field's privacy); the self-check's reopen makes
  the field public at its new type and makes `Candidate` public too, as the
  accepted reopen made `CgroupDir` public for the field's former type.
- P-I4-SCOPE-FORGE (build a proven scope from a unit name and a
  directory): R3-R1's `Scope` also holds the invocation it is bound to. The
  probe names all three fields (it would otherwise fail on the missing
  one, not on privacy); the expected error is the same privacy error for
  the three fields; the reopen makes the three fields public (and the
  identity type nameable, as `CgroupDir` for the directory).

Every other probe, the harness check and the positive control run exactly
as accepted. Usage: as api_guards.py.
"""
import hashlib
import importlib.util
import json
import pathlib
import sys

sys.dont_write_bytecode = True

RUNNER = "docs/evidence/p2-v1-r3b-i4-r1-native-scope/scripts/api_guards.py"
RUNNER_SHA256 = "16d67ff90d990b4130aa75e1bbb87fd2a8c7f9683e2fabae49846335e2e50a27"
SCOPE_FIELDS = ("pub struct Scope {\n    unit: String,\n    dir: Box<dyn CgroupDir>,\n",
                "pub struct Scope {\n    pub unit: String,\n    pub dir: Box<dyn CgroupDir>,\n")
SCOPE_INSTANCE = ("    instance: UnitInstance,\n}", "    pub instance: UnitInstance,\n}")
REASONS = {
    "P-I4-PENDING-CANDIDATE":
        "R3-R1: the candidate field is Option<Candidate>; same probe and expected errors; the "
        "reopen makes the field public at its new type and Candidate public",
    "P-I4-SCOPE-FORGE":
        "R3-R1: Scope also holds its invocation; the probe names the three fields; the same "
        "privacy error for the three; the reopen makes them public and the identity type nameable",
}


def adapt(module):
    found = set()
    for probe in module.PROBES:
        if probe["id"] == "P-I4-PENDING-CANDIDATE":
            (accepted,) = [e for e in probe["reopen"] if e["file"] == module.PENDING
                           and "candidate:" in e["anchor"]]
            if accepted["anchor"] != "    candidate: Option<Box<dyn CgroupDir>>,":
                raise SystemExit("P-I4-PENDING-CANDIDATE: the accepted reopen is not as recorded")
            accepted["anchor"] = "    candidate: Option<Candidate>,"
            accepted["replacement"] = "    pub candidate: Option<Candidate>,"
            probe["reopen"].append(dict(file=module.PENDING, anchor="\nstruct Candidate {",
                                        replacement="\npub struct Candidate {"))
        elif probe["id"] == "P-I4-SCOPE-FORGE":
            if probe["expect"] != ["error[E0451]: fields `unit` and `dir` of struct "
                                   "`nexus_verifier_sandbox::scope::Scope` are private"]:
                raise SystemExit("P-I4-SCOPE-FORGE: the accepted expectation is not as recorded")
            probe["body"] = ('''    let _ = Scope { unit: String::from("nexus-verifier-0.scope"), dir: todo!(), '''
                             '''instance: todo!() };''')
            probe["expect"] = ["error[E0451]: fields `unit`, `dir` and `instance` of struct "
                               "`nexus_verifier_sandbox::scope::Scope` are private"]
            reopen = [e for e in probe["reopen"] if e["file"] != module.SCOPE]
            probe["reopen"] = [dict(file=module.SCOPE, anchor=SCOPE_FIELDS[0], replacement=SCOPE_FIELDS[1]),
                               dict(file=module.SCOPE, anchor=SCOPE_INSTANCE[0], replacement=SCOPE_INSTANCE[1]),
                               dict(file=module.MANAGER,
                                    anchor="pub(crate) struct UnitInstance([u8; 16]);",
                                    replacement="pub struct UnitInstance([u8; 16]);")] + reopen
        else:
            continue
        probe["mapping"] = f"{probe['mapping']} [R3-R1-adapted: {REASONS[probe['id']]}]"
        found.add(probe["id"])
    if found != set(REASONS):
        raise SystemExit(f"adaptations not found: {sorted(found)}")
    print(json.dumps({"runner": RUNNER, "r3r1_adapted": REASONS}), flush=True)


def main():
    if len(sys.argv) < 3:
        raise SystemExit(__doc__)
    root = pathlib.Path(sys.argv[1]).resolve()
    runner = root / RUNNER
    digest = hashlib.sha256(runner.read_bytes()).hexdigest()
    if digest != RUNNER_SHA256:
        raise SystemExit(f"the accepted API guards changed: {digest}")
    spec = importlib.util.spec_from_file_location("i4r1_api_guards", runner)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    adapt(module)
    return module.main()


if __name__ == "__main__":
    sys.exit(main())

#!/usr/bin/env python3
"""P2-V1-R3B-I3-I1-R3 regression coverage: R301 to R328 (mission section 14).

Maps each required regression to the tests that exercise it, the counted
behavioural controls whose mutation those tests detect, and the API/type
guards that hold it at compile time, each with its as-run result:
- a test's line in the store-test output ("test <name> ... ok");
- a control's `ok` in the store-control summary;
- a guard's `ok` in api_probes.json.

The three kinds are listed apart and never added together. R325, R326 and
R327 are carried by existing suites (R1's b01 to b09, R2's v01 to v24, and
o01); their controls are those suites' own (categories authority-binding,
maintenance-authority, and the controls of o01).

Usage: requirements.py <store summary.json> <api_probes.json> <store-tests output> <out dir>
"""
import json
import pathlib
import re
import sys

R = [
    ("R301", "Ordinary succession: design section 15.3's 29 operations, unchanged",
     ["r301", "m08"], ["NC-R3-SUCC-ORDINARY-SKIP"], []),
    ("R302", "A crash after the kept copy: the repeat skips step 2a (23 operations)",
     ["r302", "r3x1", "r3x2"], ["NC-R3-SUCC-NO-SKIP"], []),
    ("R303", "A kept copy one byte off (changed, missing or added) refuses before any operation",
     ["r303"], ["NC-R3-SUCC-KEEP-OVERWRITE", "NC-R3-SUCC-KEEP-BLESS"], ["P-R3-KEPT-COPY"]),
    ("R304", "A kept copy of the wrong type, owner, mode or link count refuses",
     ["r304"], ["NC-R3-SUCC-KEEP-METADATA"], ["P-R3-KEPT-COPY"]),
    ("R305", "A leftover predecessor.tmp (or PROVISION.tmp) refuses until R-LEFTOVER",
     ["r305", "r3x1"], ["NC-R3-SUCC-LEFTOVER-IGNORED"], []),
    ("R306", "A zero partial successor root: R-SUCCESSOR removes it, durably, and the repeat completes",
     ["r306", "r3x1"],
     ["NC-R3-SUCC-ROOT-IGNORED", "NC-R3-SUCC-CLEAN-NO-PARENT-SYNC", "NC-R3-RECOVERY-NO-POSTVERIFY-SUCC"], []),
    ("R307", "A non-zero successor pool byte refuses, at construction or at the gate; nothing is removed",
     ["r307"], ["NC-R3-SUCC-CLEAN-NONZERO", "NC-R3-SUCC-CLEAN-GATE"], ["P-R3-INSPECT"]),
    ("R308", "An unexpected entry refuses; after the gate, each removal is by identity and never recursive",
     ["r308"], ["NC-R3-SUCC-CLEAN-UNEXPECTED", "NC-R3-SUCC-CLEAN-RECURSIVE", "NC-R3-SUCC-CLEAN-IDENTITY"],
     ["P-R3-REMOVE-KNOWN"]),
    ("R309", "A selected or referenced candidate refuses (and any candidate without an interrupted succession)",
     ["r309"], ["NC-R3-SUCC-CLEAN-SELECTED", "NC-R3-SUCC-CLEAN-UID", "NC-R3-SUCC-CLEAN-COPY-CONTENT",
                "NC-R3-SUCC-CLEAN-NOT-INTERRUPTED"], ["P-R3-UNREFERENCED"]),
    ("R310", "A candidate's LOCK or pool file held elsewhere refuses (Busy); the cleanup retains its own locks",
     ["r310"], ["NC-R3-SUCC-CLEAN-NO-LOCK"], []),
    ("R311", "Crash and restart at every cleanup boundary recover",
     ["r3x3"], ["NC-R3-SUCC-CLEAN-GONE-REFUSED", "NC-R3-SUCC-CLEAN-NO-PARENT-SYNC"], []),
    ("R312", "A fresh verification is required after the cleanup",
     ["r312"], ["NC-R3-SUCC-REUSE-VERIFY"], ["P-VERIFICATION-TRANSPLANT", "P-TAKE-ASSESSMENT"]),
    ("R313", "The repeat with the exact copy and the cleaned root completes with its verify-after",
     ["r313", "r3x2"], ["NC-SUCC-NO-POSTVERIFY", "NC-SUCC-POSTVERIFY-IGNORED", "NC-SUCC-VERIFY-OLD-ROOT"], []),
    ("R314", "A mismatched kept copy is never overwritten (before or after the authorization)",
     ["r314", "r303"], ["NC-R3-SUCC-KEEP-OVERWRITE", "NC-R3-SUCC-GATE-COPY"], []),
    ("R315", "A revocation at 4095 entries succeeds, giving 4096",
     ["r315"], ["NC-R3-REVOKE-BOUND-EARLY"], []),
    ("R316", "A revocation at 4096 refuses before the rename (at construction or at the gate)",
     ["r316"], ["NC-R3-REVOKE-OVERFLOW", "NC-R3-REVOKE-GATE"], ["P-R3-REVOCATION-ADMISSIBLE"]),
    ("R317", "An existing revoked target refuses, and its bytes stay",
     ["r317"], ["NC-R3-REVOKE-REPLACE"], ["P-R3-REVOCATION-ADMISSIBLE"]),
    ("R318", "Two revocations of one binding at two times are both retained",
     ["r318"], ["NC-R3-REVOKE-TIME-IGNORED"], []),
    ("R319", "R-REVOKE removes only the active name, then syncs dispositions/",
     ["r319", "r3x4"], ["NC-R3-REVOKE-RECOVER-DELETE-TARGET", "NC-R3-REVOKE-RECOVER-NO-SYNC"], []),
    ("R320", "A differing revoked target, or a time that is not a compact UTC time, refuses R-REVOKE",
     ["r320"], ["NC-R3-REVOKE-RECOVER-WRONG", "NC-R3-REVOKE-RECOVER-NAME"], ["P-R3-REVOCATION-STATE"]),
    ("R321", "Split recovery with revoked/ at 4096 is allowed",
     ["r321"], ["NC-R3-REVOKE-CAPACITY-CONFLATED"], []),
    ("R322", "Crash and restart at every resume boundary recover",
     ["r3x4"], ["NC-R3-REVOKE-COMPLETED-REFUSED"], []),
    ("R323", "A normal revocation's verify-after runs",
     ["r323"], ["NC-R3-REVOKE-NO-VERIFY-AFTER"], []),
    ("R324", "R-REVOKE's verify-after runs and requires the revocation completed",
     ["r324"], ["NC-R3-RECOVERY-NO-POSTVERIFY"], []),
    ("R325", "The R1 regressions stay green",
     ["b01", "b02", "b03", "b04", "b05", "b06", "b07", "b08", "b09"], ["category:authority-binding"], []),
    ("R326", "The R2 regressions stay green",
     [f"v{n:02}" for n in range(1, 25)], ["category:maintenance-authority"], []),
    ("R327", "No real configured-store entry",
     ["o01"], ["test:o01_the_real_store_entry_is_closed"], []),
    ("R328", "No new public authority-bearing mutation interface",
     ["r328", "a01"], ["NC-R3-SUCC-CLEAN-RECURSIVE-API"],
     ["P-R3-HOLDS-SELECTION", "P-R3-KEPT-COPY", "P-R3-UNREFERENCED", "P-R3-INSPECT", "P-R3-REMOVE-KNOWN",
      "P-R3-REVOCATION-STATE", "P-R3-REVOCATION-ADMISSIBLE"]),
]


def main():
    if len(sys.argv) != 5:
        raise SystemExit(__doc__)
    store = json.loads(pathlib.Path(sys.argv[1]).read_text())
    probes = {p["id"]: p for p in json.loads(pathlib.Path(sys.argv[2]).read_text())}
    tests_out = pathlib.Path(sys.argv[3]).read_text()
    out = pathlib.Path(sys.argv[4])
    out.mkdir(parents=True, exist_ok=True)
    controls = {c["id"]: c for c in store["counted"]}
    results = {}
    for match in re.finditer(r"^test (\w+) \.\.\. (\w+)$", tests_out, re.M):
        results[match.group(1)] = match.group(2)

    def test_line(short):
        names = [name for name in results if name == short or name.startswith(short + "_")]
        if len(names) != 1:
            return f"{short}: NOT FOUND", False
        return f"{names[0]}: {results[names[0]]}", results[names[0]] == "ok"

    def control_ids(spec):
        if spec.startswith("category:"):
            return sorted(c for c, r in controls.items() if r["category"] == spec.split(":", 1)[1])
        if spec.startswith("test:"):
            return sorted(c for c, r in controls.items() if r["test"] == spec.split(":", 1)[1])
        return [spec]

    lines = ["# Regressions R301 to R328 (P2-V1-R3B-I3-I1-R3, mission section 14)", "",
             "Each requirement, with the tests that exercise it, the counted behavioural controls whose "
             "mutation those tests detect, and the API/type guards that hold it at compile time. Tests, "
             "controls and guards are listed apart and never added together.", "",
             "| id | requirement | tests (result) | behavioural controls (as required) | API/type guards (as required) |",
             "|---|---|---|---|---|"]
    all_ok = True
    rows = []
    for rid, text, tests, specs, guards in R:
        t = [test_line(name) for name in tests]
        c = []
        for spec in specs:
            for cid in control_ids(spec):
                ok = controls.get(cid, {}).get("ok")
                c.append((cid, ok))
        g = [(gid, probes.get(gid, {}).get("ok")) for gid in guards]
        ok = all(flag for _, flag in t) and all(flag for _, flag in c) and all(flag for _, flag in g) and t
        if specs:
            ok = ok and bool(c)
        all_ok &= bool(ok)
        rows.append(dict(id=rid, requirement=text, tests=[line for line, _ in t],
                         controls=[dict(id=cid, ok=flag) for cid, flag in c],
                         guards=[dict(id=gid, ok=flag) for gid, flag in g], ok=bool(ok)))
        lines.append("| {} | {} | {} | {} | {} |".format(
            rid, text, "<br>".join(line for line, _ in t),
            "<br>".join(f"{cid}: {flag}" for cid, flag in c) or "-",
            "<br>".join(f"{gid}: {flag}" for gid, flag in g) or "-"))
    lines += ["", f"Every requirement covered, every listed item as required: {all_ok}."]
    (out / "requirements-r301-r328.md").write_text("\n".join(lines) + "\n")
    (out / "requirements-r301-r328.json").write_text(json.dumps(dict(rows=rows, all=all_ok), indent=2) + "\n")
    print(lines[-1])
    return 0 if all_ok else 1


if __name__ == "__main__":
    sys.exit(main())

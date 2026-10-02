#!/usr/bin/env python3
"""P2-V1-R3B-I3-I1-R2 successor requirement coverage.

Maps each requirement of the mission's sections 8, 9, 11, 12 and 13 to the
tests that exercise it, the counted behavioural controls whose mutation the
tests detect, and the API/type guards that hold it at compile time. Each
mapped item carries its as-run result from the candidate runs:
- a test's line in the store-test output ("test <name> ... ok");
- a control's `ok` in the store-control summary;
- a guard's `ok` in api_probes.json.

The three kinds are listed apart and never added together.

Usage: requirements.py <store summary.json> <api_probes.json> <store-tests output> <out dir>
"""
import json
import pathlib
import re
import sys

R = [
    # (id, requirement, tests, controls, guards)
    ("S01", "An undispositioned bound predecessor incident: authorization refused before any successor mutation",
     ["v01", "v11"], ["NC-SUCC-MISSING-DISPOSITION"], []),
    ("S02", "Every bound incident exactly dispositioned: the succession may proceed",
     ["v01", "v02"], [], []),
    ("S03", "One of several bound incidents without a disposition: refused",
     ["v01"], ["NC-SUCC-MISSING-DISPOSITION"], []),
    ("S04", "A truncated report: authority uses the complete verified set",
     ["v03"], ["NC-SUCC-REPORT-TRUNCATION"], []),
    ("S05", "A verification followed by a session mutation: stale, authorizes nothing",
     ["v04"], ["NC-SUCC-STALE"], []),
    ("S06", "Another session's or store's verification authorizes nothing",
     ["v05"], ["NC-SUCC-NO-VERIFY", "NC-VERIFY-AFTER-AUTHORIZES"],
     ["P-VERIFICATION-TRANSPLANT", "P-VERIFICATION-FORGE", "P-SESSION-VERIFIED"]),
    ("S07", "Capacity with a bounded complete proof: the allowed path",
     ["v06", "v18"], ["NC-SUCC-CAPACITY-REFUSED", "NC-ARCHIVE-CAPACITY"], []),
    ("S08", "Capacity whose complete proof cannot be established: refused",
     ["v07"], ["NC-SUCC-CAPACITY-BYPASS", "NC-GAP-UNBOUNDED"], []),
    ("S09", "A verified Invalid condition omitted from the Owner's acceptance: refused",
     ["v08"], ["NC-SUCC-OMIT-INVALID"], []),
    ("S10", "A fabricated extra Invalid condition: refused",
     ["v08"], ["NC-SUCC-INVENT-INVALID"], []),
    ("S11", "The canonical complete acceptance, in any order: accepted (a repeat refuses)",
     ["v08", "v20"], ["NC-SUCC-REPEAT-INVALID"], []),
    ("S12", "The persisted predecessor statement is exactly the accepted verified set (or none); over 512 bytes refuses",
     ["v02", "v08", "v09", "v20"], ["NC-SUCC-STATEMENT-CALLER", "NC-SUCC-STATEMENT-TRUNCATED"], []),
    ("S13", "Publication followed by a successful in-session verify-after: complete",
     ["v02"], ["NC-SUCC-NO-POSTVERIFY"], []),
    ("S14", "A failing verify-after: not complete, and later openings refuse",
     ["v10"], ["NC-SUCC-POSTVERIFY-IGNORED", "NC-SUCC-VERIFY-OLD-ROOT"], []),
    ("S15", "No constructed report, decision, assessment or authorization bypasses verification",
     ["v11"], [],
     ["P-VERIFICATION-FORGE", "P-TAKE-ASSESSMENT", "P-VERIFICATION-TRANSPLANT", "P-SESSION-VERIFIED",
      "P-B3-DECISION-EDIT", "P-B3-SCAN-EDIT"]),
    ("S16", "A second use of a consumed or stale authorization: refused",
     ["v04"], ["NC-SUCC-REUSE-AUTH", "NC-SUCC-STALE"], []),
    ("S17", "No successor mutation before the authorization gate",
     ["v12"], ["NC-SUCC-MUTATE-BEFORE-GATE", "NC-SUCC-NO-GATE", "NC-SUCC-GATE-PROVISION"], []),
    ("S18", "Both stores' locks retained through publication, re-selection and verify-after",
     ["v13"], ["NC-SUCC-DUAL-LOCK"], []),
    ("8A", "The session still holds and revalidates the predecessor's lock and selection",
     ["v12", "v13", "m01"],
     ["NC-SUCC-GATE-PROVISION", "I-SESSION-NEW-LOCK", "I-SESSION-SHARED", "I-SESSION-RELOCK", "I-AUTHORITY-FLAG"], []),
    ("8B", "The verification is this session's, of this root and selection, at this mutation epoch",
     ["v04", "v05"], ["NC-SUCC-STALE", "NC-SUCC-REUSE-AUTH", "NC-SUCC-NO-VERIFY"],
     ["P-VERIFICATION-TRANSPLANT", "P-VERIFICATION-FORGE", "P-TAKE-ASSESSMENT"]),
    ("8C", "Every bound incident (current and archived) has an exact valid disposition",
     ["v01", "v16"], ["NC-SUCC-MISSING-DISPOSITION", "NC-SUCC-UNDISPOSITIONED-HISTORY"], []),
    ("8D", "No bound incident omitted (truncated detail, incident_limit, Capacity, caller selection)",
     ["v03", "v06", "v07"], ["NC-SUCC-REPORT-TRUNCATION", "NC-SUCC-CAPACITY-BYPASS", "NC-GAP-UNBOUNDED"], []),
    ("8E", "Every store-level Invalid condition represented exactly",
     ["v08", "v20", "v23"],
     ["NC-VERIFY-FIRST-ONLY", "NC-SUCC-OMIT-INVALID", "NC-SUCC-INVENT-INVALID", "NC-SUCC-REPEAT-INVALID",
      "NC-ENTRY-STAT-DETERMINATE", "NC-REVOKED-BOUND-INDETERMINATE", "NC-NAMES-EXAMINED-TWICE"], []),
    ("8F", "The predecessor statement generated from that exact condition set",
     ["v02", "v08", "v09"], ["NC-SUCC-STATEMENT-CALLER", "NC-SUCC-STATEMENT-TRUNCATED"], []),
    ("9", "Lifecycle order: verify-before, gate, predecessor kept, successor created, its lock, publication, re-selection, verify-after, then complete (29 operations of 15.3)",
     ["v02", "v12", "v13", "m08", "m09"],
     ["NC-SUCC-MUTATE-BEFORE-GATE", "NC-SUCC-NO-POSTVERIFY", "NC-SUCC-VERIFY-OLD-ROOT", "NC-SUCC-DUAL-LOCK"], []),
    ("9-fail", "A failed verify-after: no success, evidence kept, nothing rolled back, later openings fail-closed",
     ["v10"], ["NC-SUCC-POSTVERIFY-IGNORED"], []),
    ("9-new", "A recovered predecessor and a new root id",
     ["v22"], ["NC-SUCC-LEFTOVER", "NC-SUCC-SAME-ROOT"], []),
    ("12", "Several coexisting Invalid conditions: stable order, no duplicate identity, no first-error-only authorization; Indeterminate refuses",
     ["v08", "v14", "v20", "v23"],
     ["NC-VERIFY-FIRST-ONLY", "NC-OWNER-COLLECTS", "NC-ENTRY-STAT-DETERMINATE", "NC-NAMES-EXAMINED-TWICE"], []),
    ("13", "Verify-after part of the procedure: mutation distinguishable from verified completion; lock held; no early end reported complete; fresh reads; no reuse; failure kept as evidence",
     ["v02", "v19", "v21", "v24", "m01"],
     ["NC-DISP-NO-POSTVERIFY", "NC-LEFTOVER-AFTER-ACCEPTED", "NC-VERIFY-AFTER-AUTHORIZES", "I-VERIFY-CACHED"], []),
    ("10-disp", "Disposition publication restates only verified facts",
     ["v15", "v16"], ["NC-DISP-UNREPORTED", "NC-DISP-FACTS-NOT-VERIFIED", "NC-DISP-ARCHIVED-NAME-ONLY"], []),
    ("10-prov", "Provisioning and re-publication never replace a store",
     ["v17", "m06"], ["NC-PROV-OVER-EXISTING", "NC-REPUBLISH-OVER-EXISTING", "NC-PROV-OVER-HISTORY", "I-FRESH-ROOT"], []),
]


def main():
    if len(sys.argv) != 5:
        raise SystemExit(__doc__)
    store = json.loads(pathlib.Path(sys.argv[1]).read_text())
    probes = json.loads(pathlib.Path(sys.argv[2]).read_text())
    tests_out = pathlib.Path(sys.argv[3]).read_text()
    out = pathlib.Path(sys.argv[4])
    out.mkdir(parents=True, exist_ok=True)
    controls = {r["id"]: r["ok"] for r in store["counted"]}
    guards = {r["id"]: r["ok"] for r in probes}
    tests = {}
    for name, result in re.findall(r"^test (\S+) \.\.\. (ok|FAILED)$", tests_out, re.M):
        tests[name] = result == "ok"

    def test_result(prefix):
        hits = [ok for name, ok in tests.items() if name.startswith(prefix + "_")]
        if len(hits) != 1:
            return None
        return hits[0]

    def mark(value):
        return "as required" if value is True else ("MISSING" if value is None else "NOT AS REQUIRED")

    lines = ["# Successor requirement coverage (P2-V1-R3B-I3-I1-R2)", "",
             "Each requirement's tests (store target, as run), counted behavioural controls and API/type guards. "
             "The three are listed apart and never added together.", "",
             "| requirement | statement | tests | behavioural controls | API/type guards |",
             "|---|---|---|---|---|"]
    all_ok = True
    for rid, text, ts, cs, gs in R:
        t_cells, c_cells, g_cells = [], [], []
        for t in ts:
            value = test_result(t)
            all_ok &= value is True
            t_cells.append(f"{t} ({mark(value)})")
        for c in cs:
            value = controls.get(c)
            all_ok &= value is True
            c_cells.append(f"{c} ({mark(value)})")
        for g in gs:
            value = guards.get(g)
            all_ok &= value is True
            g_cells.append(f"{g} ({mark(value)})")
        lines.append(f"| {rid} | {text} | {'; '.join(t_cells) or '-'} | {'; '.join(c_cells) or '-'} | "
                     f"{'; '.join(g_cells) or '-'} |")
    lines += ["", f"RESULT: {'every mapped item as required' if all_ok else 'NOT ALL AS REQUIRED'}"]
    (out / "successor-requirements.md").write_text("\n".join(lines) + "\n")
    print(lines[-1])
    return 0 if all_ok else 1


if __name__ == "__main__":
    sys.exit(main())

#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-MUTATION-TRANCHE3-EXEC-01, mission section 11: test this mission's tooling on SYNTHETIC and
HISTORICAL evidence only, before the freeze. No Cargo, no rustc, no Nexus build, test or executable is run; every
worktree, target and executable outside this mission is read, never written.

    t3_controls.py OUTDIR SCRATCH      both must not exist. OUTDIR (receipts/03-t3-controls) receives the synthetic
                                       receipts this tool classifies or links, CONTROLS.txt, CONTROLS.json and
                                       SUMMARY.txt (last line `T3 CONTROLS: ALL AS EXPECTED` or `T3 CONTROLS: FAILED`).
                                       SCRATCH (under dev-local/, never published) holds every copy of historical
                                       evidence (build receipts, tampered input roots), so that no published file
                                       repeats a previous packet's bytes.

A  classify_a (unchanged) and t3.rules on synthetic type-(a) receipts for the ten controls, transformed field by field
   from a historical receipt (CONTINUE-02's C9 run, receipts/38-C9/04-run at ebe42009), each marked SYNTHETIC with this
   mission's name. Positive: every control's first permitted property; C3b's alternative P12; C20 and A5 FAIL L1 after
   a complete search; F-1 exactly at 60,000,000; F-6 exactly at 30,000,000. Negative (mission section 11): C3b wrong
   property; C3c P13 alone; the F-PT composed quotient used as C6's mutant execution; C6 and C7 with A4 OFF; C7's
   informational F-3 receipt presented as C7's kill; F-1 beyond 60,000,000; F-6 beyond 30,000,000; C20 FAIL L1 before a
   complete search (expanded < discovered); A5 INCOMPLETE at the cap; C20 another property; a kill from outside.
B  control.link on the real authenticated linked inputs for the ten controls (each after a synthetic restoration
   receipt): LINK: VERIFIED; negative: a link before restoration (no restoration receipt), after a failed restoration,
   a changed F-1 raw output (C2), another test (A1), other bounds and the F-B header expectation (C6), a changed F-6
   classification (C3b): LINK: FAILED with the intended check failing.
C  expectations.py on tampered inputs (catalog, a patch): PROBLEMS — STOP; on the real inputs: equal to the frozen
   receipts/02-EXPECTATIONS.json.
D  the unchanged general build classifier (classify_build_success): a warning-bearing new build (the historical C5
   build's five warnings, presented as C2's build) -> BUILD: NOT OK with exactly the warning check failing; a copy of
   the accepted A3 build (no diagnostic) -> BUILD: PINNED, its CHECKS.txt and PIN.json byte-identical to the accepted
   ones (written only under SCRATCH).
E  t3.classify_info (C7's informational run): synthetic F-3 receipts: FAIL E11, PASS of a complete search, INCOMPLETE
   at the state cap -> admissible informational outcomes, never a kill label; negative: a FAIL beyond the cap
   (overshoot), a timeout kill, an OOM kill, another executable, the source not C7's mutant, the composed quotient, A4
   ON (F-PT bounds), another test: NOT ADMISSIBLE — STOP.
F  control.main's refusals (C5, C22 accepted; C1, C4, C25 reserved) and control.gate for C24 before C2's RESULT and
   for C2 before the pre-execution gate: refused or closed, nothing written.

Revised by P3-C11-DR01-D2A1-MUTATION-TRANCHE3-CONTINUE-02 (section 16, AMENDMENT-01) for this mission's nine controls:
A-F as above for this order (C2's cases moved to C24 or A1; C24 classified as main_c24 does, source EXACTLY_C, and a
negative where it is classified as a patched control would be); B with C24 linked only after its own reauthentication
(negatives: no reauthentication receipt; a restoration receipt in its place); D reruns the unchanged general classifier on
a copy of the historical C24 build receipt (BUILD: NOT OK again, its outputs byte-identical to the historical ones) and
on copies carrying a warning for each of C6, C7, A1, A2, C20 and A5 (BUILD: NOT OK); F the refusals for C2 (accepted) and
the gates before C24's RESULT and before the pre-execution gate. New:
G  the C24 build-admission checker (c24.admit), the preserved-executable pin (c24.pin) and the preserved worktree check
   (c24.worktree_check), the C5 checker's controls for C24: the real preserved receipt ADMISSIBLE; a synthetic unmodified
   copy fails only its synthetic line; tampered copies (the warning absent, an extra warning, a wrong warning (another
   variant), a wrong location, an error plus the warning, a two-warning summary, an unclassified line, the wrong mutant
   source, the wrong executable hash or size, exit 101, an OOM kill, a timeout) REJECTED with exactly the targeted check
   failing; the C24 warning presented for C3b, C3c, C6, C7, A1, A2, C20 and A5: refused; a byte-changed copy in normal
   mode: the identity check fails; the pin on the real executable PINNED; a missing executable, a wrong hash, a wrong size,
   a symlink: C24_PRESERVED_ARTIFACT_BLOCKED; the worktree AUTHENTICATED; another lock reason expected: NOT AUTHENTICATED.
H  the C3b/C3c build-admission checker (c3w.admit, AMENDMENT-01) on synthetic build receipts built from the accepted C2
   build receipt (receipts/66-C2/03-build at 774f457a, over C2's real preserved target; every copy and every general-
   classifier output under SCRATCH): clean C3b and C3c builds ADMISSIBLE (CLEAN); exactly the predicted warning ADMISSIBLE
   (NARROW) for each; REJECTED: C3b Future plus an extra warning, C3b Set, C3c Set plus an extra warning, C3c Future, C3b
   another warning, C3b an error plus the warning, C3b the warning at another location, C3b a two-warning summary, C3b
   with C3c's mutant identity; and for each of C6, C7, A1, A2, C20, A5 with the Future warning: c3w.admit refuses and
   the unchanged general classifier says BUILD: NOT OK.
"""
import copy
import json
import os
import re
import shutil
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import classify  # noqa: E402
import common as K  # noqa: E402
import control  # noqa: E402
import supervise  # noqa: E402
import t3  # noqa: E402
import c24  # noqa: E402
import c3w  # noqa: E402

BASE = K.CONTINUE02_PACKET + "/receipts/38-C9/04-run"
BASE_TEST = K.TEST_PREFIX + "full_fb2"
MARK = "SYNTHETIC (t3_controls.py, P3-C11-DR01-D2A1-MUTATION-TRANCHE3-CONTINUE-02): not the output of any run\n"
RSS = 4373  # a distinctive synthetic resource record, so that no synthetic classification repeats an earlier packet's
results = []


def record(cid, case, expected, got, ok, detail=""):
    results.append({"control": cid, "case": case, "expected": expected, "got": got, "as_expected": bool(ok),
                    "detail": str(detail)})
    print(f"{'AS EXPECTED' if ok else 'UNEXPECTED '} {cid:4} {case}: {got}{(' — ' + str(detail)) if detail else ''}")


def exe_for(cid):
    sha = K.sha256_bytes(f"SYNTHETIC executable of {cid} (t3_controls.py; never built, never run)".encode())
    path = f"/synthetic/nexus-os-p3-c11-mt3c-{cid}/target/release/deps/nexus_governed_control-0123456789abcdef"
    return {"path": path, "sha256": sha, "size": 4373}


def base_raw():
    return K.blob(K.T1, BASE + "/raw-output.txt").decode()


def transform(raw, test, header, bounds, actors, va_line, va_col):
    raw = raw.replace(BASE_TEST, test).replace("EXPLORATION F-B2", header)
    raw = re.sub(r"^  bounds: .*$", lambda m: "  bounds: " + bounds, raw, flags=re.M)
    raw = re.sub(r"^  actors: .*$", lambda m: "  actors: " + actors, raw, flags=re.M)
    return raw.replace("model_tests.rs:4133:5:", f"model_tests.rs:{va_line}:{va_col}:")


def synth(d, ctl, exe, prop=None, states=None, expanded=None, fail_line=None, header=None, test=None, bounds=None,
          actors=None, source_verdict="EXACTLY_C_PLUS_PATCH", raw=None, process=None, sup=None, argv_tail=None,
          mutant_blob=None):
    """A synthetic receipt directory d (descriptor.json, raw-output.txt) for control ctl (c2_controls.py's
    transformation, with this mission's marker; fail_line replaces the whole FAIL line, e.g. an L1 detail)."""
    os.makedirs(d)
    desc = json.loads(K.blob(K.T1, BASE + "/descriptor.json"))
    t = test or ctl["test"]
    tail = argv_tail or [t, *ctl["argv_tail"][1:]]
    if raw is None:
        va = ctl["verdict_assertion"]
        raw = transform(base_raw(), t, header or f"EXPLORATION {ctl['configuration']}", bounds or ctl["bounds"],
                        actors or ctl["actors"], va["line"], va["col"])
        p = prop or (ctl.get("permitted_properties") or [ctl["named_property"]])[0]
        fl = fail_line or None
        if fl is not None:
            raw = re.sub(r"^  FAIL: E4: .*$", lambda m: "  FAIL: " + fl, raw, flags=re.M)
        else:
            raw = raw.replace("  FAIL: E4: ", f"  FAIL: {p}: ")
        raw = raw.replace('failed: failed: {"E4"}', f'failed: failed: {{"{p}"}}')
        if states is not None:
            raw = re.sub(r"^  states 14 \(expanded 1\)", lambda m: f"  states {states} (expanded "
                         f"{expanded if expanded is not None else 1})", raw, flags=re.M)
    raw = MARK + raw
    open(os.path.join(d, "raw-output.txt"), "w").write(raw)
    ident = {"path": exe["path"], "sha256": exe["sha256"], "size": exe["size"], "mtime_ns": 0, "inode": 0, "symlink": False}
    listing = {"exit": 0, "stdout": f"{t}: test\n\n1 test, 0 benchmarks\n", "stderr": "", "listed": [f"{t}: test"]}
    mb = mutant_blob or ctl["mutant_blob"]
    desc.update({"id": os.path.basename(d) if os.path.basename(d) in ("04-run", "04b-info-F3") else "04-run",
                 "test": t, "argv": [exe["path"], *tail], "configuration": ctl["configuration"], "control": ctl["id"],
                 "mutant_blob": ctl["mutant_blob"], "patch_sha256": ctl["patch"]["sha256"],
                 "worktree": ctl["worktree"], "limit_seconds": ctl["limit_seconds"],
                 "cwd": os.path.join(ctl["worktree"], K.CRATE), "raw_output_sha256": K.sha256_bytes(raw.encode()),
                 "synthetic": "t3_controls.py: a SYNTHETIC receipt transformed from CONTINUE-02 receipts/38-C9/04-run; "
                              "not Nexus evidence of any run",
                 "executable": {"path": exe["path"], "pinned_sha256": exe["sha256"], "pinned_size": exe["size"],
                                "before": ident, "after": ident},
                 "selection": {"listing": dict(listing, argv=[t, "--exact", "--list"]),
                               "ignored_listing": dict(listing, argv=[t, "--exact", "--ignored", "--list"]),
                               "expected_ignored": True, "executable_after_listing": ident}})
    for when in ("source_before", "source_after"):
        s = desc[when]
        s.update({"worktree": ctl["worktree"], "verdict": source_verdict,
                  "mutant_file": dict(s["mutant_file"], path=ctl["source_file"], git_blob=mb,
                                      sha256=ctl["mutant_sha256"], baseline_blob=ctl["baseline_blob"]),
                  "expected_mutant_blob": ctl["mutant_blob"], "expected_mutant_sha256": ctl["mutant_sha256"]})
    if process:
        desc["process"] = process
    desc["supervisor"] = dict(desc["supervisor"], max_rss_kib_largest_process=RSS, **(sup or {}))
    json.dump(desc, open(os.path.join(d, "descriptor.json"), "w"), indent=1)


def first_two(d):
    ls = open(os.path.join(d, "CLASSIFICATION.txt")).read().splitlines()
    return ls[0], ls[1].replace("outer class (AD-1-R4 K.2): ", "")


def l1_line(k):
    return f"L1: {k} states lie on or behind a cycle of non-fault steps; one is reached by"


def pass_report(raw):
    """(header, lines) of the one exploration report in an accepted PASS receipt's raw output (report.parse_report)."""
    import report
    rep = report.parse_report(raw)
    return rep["header"], list(rep["lines"])


def incomplete_lines(lines, states, expanded, reason):
    """A PASS report's lines turned into the explorer's own INCOMPLETE form: outcome INCOMPLETE, the counts, the
    INCOMPLETE line after the elapsed line, every property NOT ESTABLISHED (explore()'s text)."""
    out = []
    for l in lines:
        if l == "  outcome: PASS":
            out.append("  outcome: INCOMPLETE")
        elif l.startswith("  states "):
            out.append(re.sub(r"^  states \d+ \(expanded \d+\)", f"  states {states} (expanded {expanded})", l))
        elif l.startswith("  elapsed "):
            out += [l, f"  INCOMPLETE: {reason}. No property is established by this run."]
        elif re.match(r"^  (P\d+|D1|L1|E\d+|KEY): ", l):
            out.append(f"  {l.strip().split(': ', 1)[0]}: NOT ESTABLISHED (incomplete)")
        else:
            out.append(l)
    return out


def compose(test, header, lines, kind, va):
    """A harness transcript around one report, in the executable's own form: kind PASS (ok, exit 0) or INCOMPLETE (the
    explorer's verdict assertion: left Incomplete, right Pass)."""
    text = "\n".join([header] + lines)
    if kind == "PASS":
        return (f"\nrunning 1 test\ntest {test} ... {text}\n\nok\n\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 "
                f"measured; {K.LISTED - 1} filtered out; finished in 60.00s\n\n")
    return (f"\nrunning 1 test\ntest {test} ... {text}\n\n\nthread '{test}' (4373) panicked at {va['file']}:{va['line']}:"
            f"{va['col']}:\nassertion `left == right` failed: failed: {{}}\n{text}\n\n  left: Incomplete\n right: Pass\n"
            "note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace\nFAILED\n\nfailures:\n\n"
            f"failures:\n    {test}\n\ntest result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; {K.LISTED - 1} "
            "filtered out; finished in 600.00s\n\n")


def section_a(out, exp):
    print("A. classify_a (unchanged) and t3.rules on synthetic type-(a) receipts (historical base: CONTINUE-02 C9 run)")
    C = exp["controls"]
    OK = "FAIL — admissible type-(a) original-explorer mutant FAIL of the named property"
    BUDGET = "INCOMPLETE — BUDGET EXCEEDED (a FAIL printed beyond the authorised cardinality): kills nothing"
    f3 = C["C7"]["informational"]
    QH = ("EXPLORATION F-PT (gating-caller symmetry quotient composed with the admission observer quotient: "
          "experimental, not the contract's explorer)")
    l1ok = {"prop": "L1", "fail_line": l1_line(9), "states": 120, "expanded": 120}
    cases = [(cid, "P-first-permitted-property", {} if cid not in K.L1_CONTROLS else dict(l1ok), classify.CANDIDATE,
              OK, True) for cid in K.ORDER]
    cases += [
        ("C3b", "P-alternative-permitted-property-P12", {"prop": "P12"}, classify.CANDIDATE, OK + " P13 or P12: P12", True),
        ("C24", "P-FAIL-exactly-at-the-F-1-ceiling-60000000", {"states": 60000000}, classify.CANDIDATE, OK + " P8", True),
        ("C3c", "P-FAIL-exactly-at-the-30000000-ceiling", {"states": 30000000}, classify.CANDIDATE, OK + " P12", True),
        ("C3b", "N-C3b-wrong-property-P4", {"prop": "P4"}, classify.NOT_CANDIDATE,
         "FAIL of P4, not a permitted property (P13 or P12): kills nothing", False),
        ("C3c", "N-C3c-P13-alone", {"prop": "P13"}, classify.NOT_CANDIDATE,
         "FAIL of P13, not the named property P12: kills nothing (no attribution exception)", False),
        ("C6", "N-F-PT-composed-quotient-used-as-the-mutant-execution", {
            "header": QH, "test": K.TEST_PREFIX + "gc_symmetry_fpt"}, classify.NOT_CANDIDATE, "NOT ADMISSIBLE (", False),
        ("C6", "N-C6-with-A4-OFF (F-3 bounds)", {"bounds": f3["bounds"]}, classify.NOT_CANDIDATE,
         "NOT ADMISSIBLE (printed bounds equal the configuration's", False),
        ("C7", "N-C7-kill-run-with-A4-OFF (full_f3 on F-3)", {"test": f3["test"], "header": "EXPLORATION F-3",
                                                             "bounds": f3["bounds"]}, classify.NOT_CANDIDATE,
         "NOT ADMISSIBLE (", False),
        ("C7", "N-C7-informational-F-3-receipt-counted-as-the-kill (FAIL E11)", {
            "test": f3["test"], "header": "EXPLORATION F-3", "bounds": f3["bounds"], "prop": "E11"},
         classify.NOT_CANDIDATE, "FAIL of E11, not the named property P6: kills nothing", False),
        ("C24", "N-F-1-FAIL-beyond-60000000 (cap overshoot)", {"states": 60000001}, classify.NOT_CANDIDATE, BUDGET, False),
        ("C24", "N-C24-classified-as-a-patched-control-would-be (EXACTLY_C_PLUS_PATCH expected; the worktree is C)",
         {"c24_as_patched": True}, classify.NOT_CANDIDATE, "NOT ADMISSIBLE (", True),
        ("A1", "N-F-1-FAIL-beyond-60000000 (cap overshoot)", {"states": 60000001}, classify.NOT_CANDIDATE, BUDGET, False),
        ("C3c", "N-F-6-FAIL-beyond-30000000 (cap overshoot)", {"states": 30000001}, classify.NOT_CANDIDATE, BUDGET, False),
        ("C6", "N-F-PT-FAIL-beyond-30000000 (cap overshoot)", {"states": 30000001}, classify.NOT_CANDIDATE, BUDGET, False),
        ("C20", "N-L1-before-a-complete-search (expanded < discovered)",
         {"prop": "L1", "fail_line": l1_line(9), "states": 120, "expanded": 119}, classify.CANDIDATE, OK, False),
        ("A5", "N-L1-before-a-complete-search (expanded < discovered)",
         {"prop": "L1", "fail_line": l1_line(9), "states": 120, "expanded": 7}, classify.CANDIDATE, OK, False),
        ("A5", "N-incomplete-search (INCOMPLETE at the cap)", {"raw": "INCOMPLETE"}, classify.NOT_CANDIDATE,
         "INCOMPLETE (the state cap of 30000000 was reached. No property is established by this run.): kills nothing",
         False),
        ("C20", "N-L1-FAIL-beyond-30000000 (cap overshoot)",
         {"prop": "L1", "fail_line": l1_line(9), "states": 30000001, "expanded": 30000001}, classify.NOT_CANDIDATE,
         BUDGET, False),
        ("C20", "N-another-property-P12-instead-of-L1", {"prop": "P12"}, classify.NOT_CANDIDATE,
         "FAIL of P12, not the named property L1: kills nothing (no attribution exception)", False),
        ("C24", "N-killed-from-outside", {"process": {"exit_status": None, "signal": 9, "supervisor_rc": -9,
                                                      "wrapper_recorded_status": "137"},
                                          "sup": {"timed_out": True, "wall_seconds": 2700.0}, "raw": "TRUNCATED"},
         classify.NOT_CANDIDATE, "INCOMPLETE (killed from outside or abnormal termination): kills nothing", False),
    ]
    fr = open(K.inputs("linked", "S1-41-full-F-2", "raw-output.txt"), encoding="utf-8").read()
    for n, (cid, case, kw, want_first, want_outer, want_rules) in enumerate(cases, 1):
        ctl = C[cid]
        exe = exe_for(cid)
        kw = dict(kw)
        if kw.get("raw") == "INCOMPLETE":
            # An INCOMPLETE F-2 report in the explorer's own form, from the accepted F-2 PASS receipt's report.
            h, ls = pass_report(fr)
            ls = incomplete_lines(ls, 30000001, 26000000, "the state cap of 30000000 was reached")
            kw["raw"] = compose(ctl["test"], h, ls, "INCOMPLETE", ctl["verdict_assertion"])
            kw["process"] = {"exit_status": 101, "signal": None, "supervisor_rc": 101, "wrapper_recorded_status": "101"}
        elif kw.get("raw") == "TRUNCATED":
            kw["raw"] = "\n".join(base_raw().replace(BASE_TEST, ctl["test"]).splitlines()[:6]) + "\n"
        d = os.path.join(out, "A", f"{n:02d}-{cid}-{re.sub(r'[^A-Za-z0-9_.-]+', '-', case)[:60].strip('-')}", "04-run")
        as_patched = kw.pop("c24_as_patched", False)
        if cid == "C24":
            # TRANCHE3-CONTINUE-02: C24 runs from its preserved build with its worktree exactly C (main_c24).
            synth(d, ctl, exe, source_verdict="EXACTLY_C", **kw)
            if as_patched:
                classify.classify_a(d, ctl, exe)
            else:
                classify.classify_a(d, dict(ctl, mutant_blob=None), exe, source_verdict="EXACTLY_C")
        else:
            synth(d, ctl, exe, **kw)
            classify.classify_a(d, ctl, exe)
        first, outer = first_two(d)
        rok = t3.rules(os.path.join(os.path.dirname(d), "04a-rules"), d, ctl)
        candidate = first == classify.CANDIDATE and rok
        ok = first == want_first and outer.startswith(want_outer) and rok == want_rules
        failed = [l for l in open(os.path.join(os.path.dirname(d), "04a-rules/RULES.txt")).read().splitlines()
                  if l.startswith("FAIL")]
        record(cid, "A " + case, f"{want_first} / {want_outer}…; rules {'HOLD' if want_rules else 'NOT SATISFIED'}; "
               f"candidate {want_first == classify.CANDIDATE and want_rules}",
               f"{first} / {outer[:100]}; rules {'HOLD' if rok else 'NOT SATISFIED'}; candidate {candidate}", ok,
               [f[:90] for f in failed][:3])


def report_props():
    import report
    return report.model_constants()[0]


def section_b(out, scratch, exp):
    print("B. control.link on the authenticated linked inputs (after a synthetic restoration receipt), and negatives")
    C = exp["controls"]

    def restore_rec(base, first="RESTORE: EXACTLY_C", sub="05-restore", name="RESTORE.txt"):
        os.makedirs(os.path.join(base, sub))
        open(os.path.join(base, sub, name), "w").write(
            f"{first}\n{MARK}a synthetic restoration receipt for a tooling control of the link step\n")
    for cid in K.ORDER:
        base = os.path.join(out, "B", f"{cid}-real-inputs")
        if cid == "C24":
            restore_rec(base, control.REAUTH_OK, "05-reauth", "REAUTH.txt")
        else:
            restore_rec(base)
        ok, ref = control.link(C[cid], exp, os.path.join(base, "06-link"), None)
        first = supervise.first_line(os.path.join(base, "06-link/LINK.txt"))
        record(cid, "B link on the real authenticated inputs, after a restoration receipt", "LINK: VERIFIED", first,
               first == "LINK: VERIFIED" and ok, ref[:90])
    root = os.path.join(scratch, "B-ev")
    saved = K.EV

    def fresh():
        if os.path.exists(root):
            shutil.rmtree(root)
        shutil.copytree(os.path.join(saved, "inputs"), os.path.join(root, "inputs"))
        for a, _, fs in os.walk(root):
            for f in fs:
                os.chmod(os.path.join(a, f), 0o644)
        K.EV = root

    def tamper(rel, old, new):
        p = os.path.join(root, rel)
        b = open(p, "rb").read()
        assert old in b, (rel, old)
        open(p, "wb").write(b.replace(old, new, 1))
    restoration = "this control's own restoration receipt exists and records RESTORE: EXACTLY_C"
    reauth = "this control's own reauthentication receipt exists and records " + control.REAUTH_OK
    neg = [
        ("A1", "N-linked-receipt-before-restoration (no restoration receipt)", None, None, None, restoration),
        ("C24", "N-linked-receipt-before-reauthentication (no reauthentication receipt)", None, None, None, reauth),
        ("C24", "N-a-restoration-receipt-instead-of-the-reauthentication-receipt", None, None, "RESTORE: EXACTLY_C",
         reauth),
        ("C20", "N-linked-receipt-after-a-failed-restoration", None, None, "RESTORE: FAILED", restoration),
        ("C24", "N-F-1-raw-output-byte-changed", lambda: tamper("inputs/linked/S1-45-full-F-1/raw-output.txt",
                                                             b"outcome: PASS", b"outcome: PASs"), None, "RESTORE: EXACTLY_C",
         "its classification blob and raw output identity are the catalog's"),
        ("A1", "N-another-test-in-the-expectation", None, lambda c: c["argv_tail"].__setitem__(0, K.TEST_PREFIX + "full_f2"),
         "RESTORE: EXACTLY_C", "exact configuration: the same test and flags as this control's run (F-1)"),
        ("C6", "N-other-bounds-in-the-expectation (A4 off)", None, lambda c: c.__setitem__("bounds", c["bounds"].replace(
            " A4 true;", " A4 false;")), "RESTORE: EXACTLY_C", "F-PT's restored side (AD-1-R4 F)"),
        ("C7", "N-the-F-B-configuration-in-the-expectation", None, lambda c: c.__setitem__("configuration", "F-B"),
         "RESTORE: EXACTLY_C", "F-B's restored side (AD-1-R4 F)"),
        ("C3b", "N-F-6-classification-byte-changed", lambda: tamper("inputs/linked/S2-66-full-F-6/CLASSIFICATION.txt",
                                                                  b"CLASSIFICATION: PASS", b"CLASSIFICATION: PASs"), None,
         "RESTORE: EXACTLY_C", "its classification blob and raw output identity are the catalog's"),
    ]
    try:
        for cid, case, files, ctlmod, rfirst, want in neg:
            fresh()
            if files:
                files()
            ctl = copy.deepcopy(C[cid])
            if ctlmod:
                ctlmod(ctl)
            base = os.path.join(out, "B", f"{cid}-{case[:60]}")
            os.makedirs(base)
            if rfirst:
                restore_rec(base, rfirst)
            if cid == "C24" and case == "N-F-1-raw-output-byte-changed":  # reauthenticated: only the tamper fails
                restore_rec(base, control.REAUTH_OK, "05-reauth", "REAUTH.txt")
            ok, ref = control.link(ctl, exp, os.path.join(base, "06-link"), None)
            text = open(os.path.join(base, "06-link/LINK.txt")).read().splitlines()
            fails = [l for l in text if l.startswith("FAIL")]
            hit = [l for l in fails if want in l]
            record(cid, "B " + case, f"LINK: FAILED with the check '{want[:70]}…' failing",
                   f"{text[0]}; {len(fails)} failed", text[0] == "LINK: FAILED" and not ok and hit, hit[:1] or fails[:2])
            K.EV = saved
    finally:
        K.EV = saved


def section_c(out, scratch, exp):
    print("C. expectations.py on tampered inputs, and on the real inputs")
    import expectations as E
    saved = K.EV
    root = os.path.join(scratch, "C-ev")
    for case, rel, old, new, want in (
            ("N-catalog-copy-changed", "inputs/catalog.json", b'"P13 or P12"', b'"P13 or P15"',
             "authenticated catalog copy sha256"),
            ("N-patch-copy-changed", "inputs/patches/C20.patch", b"if cell == Cell::C {", b"if cell == Cell::X {",
             "C20 authenticated patch copy sha256")):
        if os.path.exists(root):
            shutil.rmtree(root)
        shutil.copytree(os.path.join(saved, "inputs"), os.path.join(root, "inputs"))
        p = os.path.join(root, rel)
        os.chmod(p, 0o644)
        b = open(p, "rb").read()
        assert old in b, (rel, old)
        open(p, "wb").write(b.replace(old, new, 1))
        K.EV = root
        E.problems.clear()
        try:
            try:
                rc = E.main(os.path.join(scratch, f"C-{case}.json"))
            except Exception as ex:  # a tampered input may stop the derivation itself: still a STOP
                rc, E.problems[:] = 2, E.problems + [f"exception: {ex!r}"]
        finally:
            K.EV = saved
        hit = [p for p in E.problems if want in p]
        record("—", "C " + case, f"PROBLEMS — STOP naming '{want}'", f"rc {rc}; {len(E.problems)} problems",
               rc != 0 and hit and not os.path.exists(os.path.join(scratch, f"C-{case}.json")), hit[:1])
    E.problems.clear()
    rc = E.main(os.path.join(scratch, "C-real-inputs-EXPECTATIONS.json"))
    same = K.sha256_file(os.path.join(scratch, "C-real-inputs-EXPECTATIONS.json")) == K.sha256_file(
        os.path.join(K.EV, "receipts/02-EXPECTATIONS.json"))
    record("—", "C P-real-inputs", "DERIVED, equal to receipts/02-EXPECTATIONS.json", f"rc {rc}; equal {same}",
           rc == 0 and same)


def section_d(out, scratch, exp):
    print("D. the unchanged general build classifier: the historical C24 build; warnings on C6 C7 A1 A2 C20 A5; an "
          "accepted warning-free build")
    C = exp["controls"]

    def copy_receipt(commit, packet_dir, name):
        d = os.path.join(scratch, "D", name)
        os.makedirs(d)
        for f in K.git("ls-tree", "--name-only", commit, packet_dir + "/").decode().split():
            open(os.path.join(d, os.path.basename(f)), "wb").write(K.blob(commit, f))
        return d
    # The historical C24 build: the general classifier (unchanged) reproduces BUILD: NOT OK, byte for byte.
    g = copy_receipt(K.T3X, K.C24_BUILD_DIR, "C24-historical-general-classifier-rerun")
    for f in ("CHECKS.txt", "CLASSIFICATION.txt", "PARSED-RECEIPT.json"):
        os.remove(os.path.join(g, f))
    rc = classify.classify_build_success(g, "pin", "EXACTLY_C_PLUS_PATCH", C["C24"]["mutant_blob"],
                                         expect_target=os.path.join(K.C24_WT, "target"))
    same = {f: K.sha256_file(os.path.join(g, f)) == K.sha256_bytes(K.blob(K.T3X, f"{K.C24_BUILD_DIR}/{f}"))
            for f in ("CHECKS.txt", "CLASSIFICATION.txt", "PARSED-RECEIPT.json")}
    first = supervise.first_line(os.path.join(g, "CLASSIFICATION.txt"))
    record("C24", "D the general classifier (classify_build_success, unchanged) rerun on a copy of the historical C24 "
           "build receipt", "BUILD: NOT OK; CHECKS.txt, CLASSIFICATION.txt, PARSED-RECEIPT.json byte-identical to the "
           "historical ones; no PIN.json", f"{first}; rc {rc}; identical {same}; PIN.json "
           f"{os.path.exists(os.path.join(g, 'PIN.json'))}", first == "BUILD: NOT OK" and rc == 1 and all(same.values())
           and not os.path.exists(os.path.join(g, "PIN.json")))
    # The C24 warning presented as each zero-warning control's build: rejected by the unchanged general classifier.
    for cid in ("C6", "C7", "A1", "A2", "C20", "A5"):
        w = copy_receipt(K.T3X, K.C24_BUILD_DIR, f"{cid}-with-a-warning")
        for f in ("CHECKS.txt", "CLASSIFICATION.txt", "PARSED-RECEIPT.json"):
            os.remove(os.path.join(w, f))
        pth = os.path.join(w, "descriptor.json")
        j = json.load(open(pth))
        for when in ("source_before", "source_after"):
            j[when]["mutant_file"]["git_blob"] = C[cid]["mutant_blob"]
        json.dump(j, open(pth, "w"), indent=1)
        rc = classify.classify_build_success(w, "pin", "EXACTLY_C_PLUS_PATCH", C[cid]["mutant_blob"],
                                             expect_target=os.path.join(K.C24_WT, "target"))
        fails = [l[5:].split(":")[0] for l in open(os.path.join(w, "CHECKS.txt")).read().splitlines()
                 if l.startswith("FAIL")]
        first = supervise.first_line(os.path.join(w, "CLASSIFICATION.txt"))
        record(cid, "D a warning-bearing build presented as " + cid + "'s: rejected (zero warnings required), no PIN.json",
               "BUILD: NOT OK with exactly the check 'no error or warning diagnostic' failing",
               f"{first}; failing {fails}; PIN.json {os.path.exists(os.path.join(w, 'PIN.json'))}",
               first == "BUILD: NOT OK" and rc == 1 and fails == ["no error or warning diagnostic"]
               and not os.path.exists(os.path.join(w, "PIN.json")))
    a3dir = f"{K.T2C_PACKET}/{K.T2C_RECEIPTS['A3']}/03-build"
    a = copy_receipt(K.T2C, a3dir, "accepted-A3-build-rerun")
    for f in ("CHECKS.txt", "CLASSIFICATION.txt", "PARSED-RECEIPT.json", "PIN.json"):
        os.remove(os.path.join(a, f))
    rc = classify.classify_build_success(a, "pin", "EXACTLY_C_PLUS_PATCH",
                                         json.loads(K.blob(K.T2C, a3dir + "/descriptor.json"))["mutant_blob"],
                                         expect_target=K.ACCEPTED_WT["A3"] + "/target")
    same = {f: K.sha256_file(os.path.join(a, f)) == K.sha256_bytes(K.blob(K.T2C, f"{a3dir}/{f}"))
            for f in ("CHECKS.txt", "CLASSIFICATION.txt")}
    pin_new, pin_old = json.load(open(os.path.join(a, "PIN.json"))), json.loads(K.blob(K.T2C, f"{a3dir}/PIN.json"))
    same["PIN.json (every field but build_receipt, the receipt directory's own path)"] = (
        {k: v for k, v in pin_new.items() if k != "build_receipt"} == {k: v for k, v in pin_old.items()
                                                                      if k != "build_receipt"})
    first = supervise.first_line(os.path.join(a, "CLASSIFICATION.txt"))
    record("A3", "D the unchanged classifier rerun on a copy of the accepted A3 build (no diagnostic)",
           "BUILD: PINNED; CHECKS.txt and CLASSIFICATION.txt byte-identical to the accepted ones; PIN.json equal but "
           "for its own receipt path",
           f"{first}; rc {rc}; identical {same}", first == "BUILD: PINNED" and rc == 0 and all(same.values()))


def section_e(out, scratch, exp):
    print("E. t3.classify_info (C7's informational run without A4 on F-3)")
    ctl = exp["controls"]["C7"]
    info = ctl["informational"]
    ictl = dict(ctl, test=info["test"], argv_tail=info["argv_tail"], bounds=info["bounds"], actors=info["actors"],
                configuration="F-3")
    exe = exe_for("C7")
    va = info["verdict_assertion"]
    h, ls = pass_report(open(K.inputs("linked", "S1-41-full-F-2", "raw-output.txt"), encoding="utf-8").read())
    h = h.replace("EXPLORATION F-2", "EXPLORATION F-3")
    ls = [("  bounds: " + info["bounds"]) if l.startswith("  bounds: ") else ("  actors: " + info["actors"])
          if l.startswith("  actors: ") else l for l in ls]
    pass_raw = compose(info["test"], h, ls, "PASS", va)
    inc_raw = compose(info["test"], h, incomplete_lines(ls, 30000004, 26123456, "the state cap of 30000000 was reached"),
                      "INCOMPLETE", va)
    E101 = {"exit_status": 101, "signal": None, "supervisor_rc": 101, "wrapper_recorded_status": "101"}
    cases = [
        ("P-FAIL-E11 (the catalog's expectation)", {"prop": "E11"}, True, t3.INFO_PREFIX + "FAIL E11"),
        ("P-PASS-of-a-complete-search", {"raw": pass_raw, "process": {"exit_status": 0, "signal": None, "supervisor_rc": 0,
                                                                      "wrapper_recorded_status": "0"}},
         True, t3.INFO_PREFIX + "PASS"),
        ("P-INCOMPLETE-at-the-explorer's-own-state-cap", {"raw": inc_raw, "process": E101}, True,
         t3.INFO_PREFIX + "INCOMPLETE (the state cap of 30000000 was reached)"),
        ("N-FAIL-beyond-the-cap (overshoot)", {"prop": "E11", "states": 30000001}, False, t3.INFO_STOP),
        ("N-timeout-kill", {"prop": "E11", "process": {"exit_status": None, "signal": 9, "supervisor_rc": -9,
                                                       "wrapper_recorded_status": "137"},
                            "sup": {"timed_out": True, "wall_seconds": 2700.0}}, False, t3.INFO_STOP),
        ("N-OOM-kill", {"prop": "E11", "sup": {"oom_kill": 1, "oom": 1}}, False, t3.INFO_STOP),
        ("N-another-executable", {"prop": "E11", "exe": "other"}, False, t3.INFO_STOP),
        ("N-source-not-C7's-mutant", {"prop": "E11", "mutant_blob": exp["controls"]["C6"]["mutant_blob"]}, False,
         t3.INFO_STOP),
        ("N-composed-quotient-header", {"prop": "E11", "header": "EXPLORATION F-3 (gating-caller symmetry quotient "
                                                                 "composed with the admission observer quotient: "
                                                                 "experimental, not the contract's explorer)"},
         False, t3.INFO_STOP),
        ("N-A4-ON (F-PT bounds)", {"prop": "E11", "bounds": ctl["bounds"]}, False, t3.INFO_STOP),
        ("N-another-test (full_fpt)", {"prop": "E11", "test": ctl["test"]}, False, t3.INFO_STOP),
    ]
    for n, (case, kw, want_ok, want_first) in enumerate(cases, 1):
        kw = dict(kw)
        run_exe = exe
        if kw.pop("exe", None) == "other":
            run_exe = dict(exe, sha256=K.sha256_bytes(b"another executable"))
        base = os.path.join(out, "E", f"{n:02d}-{re.sub(r'[^A-Za-z0-9_.-]+', '-', case)[:60].strip('-')}")
        d = os.path.join(base, "04b-info-F3")
        os.makedirs(base)
        synth(d, ictl, exe, **kw)
        ok, first = t3.classify_info(d, info, run_exe, "EXACTLY_C_PLUS_PATCH", ctl["mutant_blob"])
        cls = open(os.path.join(d, "CLASSIFICATION.txt")).read()
        record("C7", "E " + case, f"{'admissible' if want_ok else 'NOT ADMISSIBLE — STOP'}; {want_first}; no kill label",
               f"{'admissible' if ok else 'NOT ADMISSIBLE'}; {first[:110]}",
               ok == want_ok and first.startswith(want_first) and "KILL_EVIDENCE" not in cls
               and classify.CANDIDATE not in cls)


def failed_labels(rec):
    return [l for l, o, _ in rec.get("checks", []) if not o]


def section_g(out, scratch, exp):
    print("G. the C24 build-admission checker, the preserved-executable pin and the preserved worktree check")
    C = exp["controls"]
    ctl = C["C24"]
    real = K.inputs("c24", "03-build")
    rec, ok = c24.admit(real, ctl)
    c24.write(os.path.join(out, "G"), "P-real-preserved-receipt-ADMISSION", c24.ADMIT_OK, c24.ADMIT_FAIL,
              "t3_controls.py G: c24.admit on the real authenticated copy of the preserved Tranche-3 build receipt", rec, ok)
    record("C24", "G admit: the real preserved receipt", c24.ADMIT_OK, c24.ADMIT_OK if ok else c24.ADMIT_FAIL, ok,
           f"{sum(o for _, o, _ in rec['checks'])} of {len(rec['checks'])} checks hold")
    synth_label = ("SYNTHETIC CONTROL (t3_controls.py): a tampered copy, never admissible; the preserved-receipt identity "
                   "check is not run")

    def copy_build(name):
        d = os.path.join(scratch, "G", name)
        shutil.copytree(real, d)
        for f in os.listdir(d):
            os.chmod(os.path.join(d, f), 0o644)
        return d
    d0 = copy_build("S0-unmodified")
    rec, ok = c24.admit(d0, ctl, synthetic=True)
    fl = failed_labels(rec)
    record("C24", "G admit (synthetic mode): an unmodified copy: only the synthetic line fails",
           "REJECTED; failing exactly [synthetic line]", f"{'ADMISSIBLE' if ok else 'REJECTED'}; failing {len(fl)}",
           not ok and fl == [synth_label], fl[:3])
    stderr = open(os.path.join(real, "build.stderr"), encoding="utf-8").read()
    group = stderr[stderr.index("warning: variant `Corrected` is never constructed\n"):].split("\n\n", 1)[0] + "\n\n"
    assert group in stderr
    # A well-formed group on a line every mutant here leaves unchanged (protocol.rs:1643, before every hunk): only the
    # one-group check fails.
    l1643 = K.source(K.PROTOCOL).split("\n")[1642]
    col = l1643.index("set =") + 1
    extra = ("warning: unused variable: `set`\n"
             f"    --> crates/nexus-governed-control/src/ledger/protocol.rs:1643:{col}\n     |\n1643 | {l1643}\n"
             f"     | {' ' * (col - 1)}^^^ help: if this is intentional, prefix it with an underscore: `_set`\n\n")

    def desc_mod(f):
        def m(d):
            pth = os.path.join(d, "descriptor.json")
            j = json.load(open(pth))
            f(j)
            json.dump(j, open(pth, "w"), indent=1)
        return m

    def stderr_mod(old, new):
        def m(d):
            pth = os.path.join(d, "build.stderr")
            t = open(pth, encoding="utf-8").read()
            assert old in t, old[:60]
            t = t.replace(old, new, 1)
            open(pth, "w", encoding="utf-8").write(t)
            desc_mod(lambda j: j.__setitem__("stderr_sha256", K.sha256_bytes(t.encode())))(d)
        m.stderr = True  # a changed stderr also fails its identity check (the preserved sha256)
        return m
    J1 = "J exactly the permitted warning group (1), each once"
    neg = [
        ("N-the-permitted-warning-absent", stderr_mod(group, ""), [J1, "J warning 1 present"]),
        ("N-an-extra-warning", stderr_mod("warning: `nexus-governed-control` (lib test) generated 1 warning",
                                          extra + "warning: `nexus-governed-control` (lib test) generated 1 warning"),
         [J1]),
        ("N-a-wrong-warning (variant `Inconsistent`)", stderr_mod("warning: variant `Corrected` is never constructed",
                                                                 "warning: variant `Inconsistent` is never constructed"),
         [J1, "J warning 1 present"]),
        ("N-wrong-warning-location (739:5 -> 740:5)", stderr_mod("protocol.rs:739:5", "protocol.rs:740:5"),
         ["J warning 1 (variant `Corrected` is never constructed): primary location"]),
        ("N-an-error-plus-the-warning", stderr_mod("warning: variant `Corrected` is never constructed\n",
                                                   "error[E0425]: cannot find value `x` in this scope\n"
                                                   "   --> crates/nexus-governed-control/src/ledger/protocol.rs:739:5\n\n"
                                                   "warning: variant `Corrected` is never constructed\n"),
         ["I no `error` diagnostic", J1]),
        ("N-a-two-warning-summary", stderr_mod("generated 1 warning", "generated 2 warnings"),
         [J1, "K every line of every group", "K cargo's summary of exactly that warning",
          "K every other stderr line is cargo progress"]),
        ("N-an-unclassified-diagnostic-line", stderr_mod("    Finished `release`", "note: an extra note\n    Finished `release`"),
         ["K every other stderr line is cargo progress"]),
        ("N-wrong-mutant-source (C3b's mutant blob before the build)", desc_mod(
            lambda j: j["source_before"]["mutant_file"].__setitem__("git_blob", C["C3b"]["mutant_blob"])),
         ["A source before the build"]),
        ("N-wrong-executable-hash", desc_mod(lambda j: j["deps_artifacts"][0].__setitem__("sha256", "0" * 64)),
         ["M the recorded executable sha256 and size"]),
        ("N-wrong-executable-size", desc_mod(lambda j: j["deps_artifacts"][0].__setitem__("size", K.C24_EXE_SIZE + 1)),
         ["M the recorded executable sha256 and size"]),
        ("N-exit-101", desc_mod(lambda j: j["process"].__setitem__("exit_status", 101)), ["F normal exit 0"]),
        ("N-an-OOM-kill", desc_mod(lambda j: j["supervisor"].__setitem__("oom_kill", 1)), ["H no OOM and no signal"]),
        ("N-a-timeout", desc_mod(lambda j: j["supervisor"].__setitem__("timed_out", True)), ["G 16 GiB, zero swap"]),
    ]
    for n, (case, mod, want) in enumerate(neg, 1):
        d = copy_build(f"N{n:02d}")
        mod(d)
        rec, ok = c24.admit(d, ctl, synthetic=True)
        fl = failed_labels(rec)
        want = want + (["the stderr is the preserved one"] if getattr(mod, "stderr", False) else [])
        hit = all(any(l.startswith(w) for l in fl) for w in want)
        only = all(l == synth_label or any(l.startswith(w) for w in want) for l in fl)
        c24.write(os.path.join(out, "G"), f"N{n:02d}-ADMISSION", c24.ADMIT_OK, c24.ADMIT_FAIL,
                  f"t3_controls.py G: c24.admit (synthetic mode) on a tampered copy: {case}", rec, ok)
        record("C24", "G admit: " + case, f"REJECTED; failing the synthetic line and exactly {want}",
               f"{'ADMISSIBLE' if ok else 'REJECTED'}; failing {len(fl)}", not ok and hit and only,
               [l[:70] for l in fl if l != synth_label][:4])
    for cid in ("C3b", "C3c", "C6", "C7", "A1", "A2", "C20", "A5"):
        rec, ok = c24.admit(real, C[cid])
        fl = failed_labels(rec)
        record(cid, "G admit: the C24 warning presented for " + cid + " (the ruling does not transfer)",
               "REJECTED: the control is not C24", f"{'ADMISSIBLE' if ok else 'REJECTED'}; failing {fl[:1]}",
               not ok and fl and fl[0].startswith("the control is C24"))
    d = copy_build("N-identity-byte-changed")
    open(os.path.join(d, "RECEIPT.txt"), "a").write("\n")
    rec, ok = c24.admit(d, ctl)
    fl = failed_labels(rec)
    record("C24", "G admit (normal mode): a byte-changed copy of the preserved receipt",
           "REJECTED: the preserved-receipt identity check fails", f"{'ADMISSIBLE' if ok else 'REJECTED'}; failing {fl[:1]}",
           not ok and fl and fl[0].startswith("the build evidence is the preserved Tranche-3 receipt"))
    # The pin and the worktree check (read-only).
    rec, ok = c24.pin(ctl, real)
    c24.write(os.path.join(out, "G"), "P-real-PIN", c24.PIN_OK, c24.PIN_FAIL, "t3_controls.py G: c24.pin on the real "
              "preserved executable (read-only)", rec, ok)
    record("C24", "G pin: the real preserved executable", c24.PIN_OK, c24.PIN_OK if ok else c24.PIN_FAIL, ok,
           f"{sum(o for _, o, _ in rec['checks'])} of {len(rec['checks'])} checks hold")
    link = os.path.join(scratch, "G", "symlink-to-the-preserved-executable")
    os.symlink(K.C24_EXE, link)
    for case, mod, want in (
            ("N-missing-executable", lambda c: c["preserved"]["executable"].__setitem__(
                "path", os.path.join(K.C24_WT, "target/release/deps/nexus_governed_control-0000000000000000")),
             "the preserved executable exists"),
            ("N-executable-hash-mismatch", lambda c: c["preserved"]["executable"].__setitem__("sha256", "1" * 64),
             "sha256 exact"),
            ("N-wrong-size", lambda c: c["preserved"]["executable"].__setitem__("size", K.C24_EXE_SIZE - 1), "size exact"),
            ("N-another-path (a symlink to the real executable)", lambda c: c["preserved"]["executable"].__setitem__(
                "path", link), "a regular file, not a symlink")):
        c = copy.deepcopy(ctl)
        mod(c)
        rec, ok = c24.pin(c, real)
        fl = failed_labels(rec)
        record("C24", "G pin: " + case, f"{c24.PIN_FAIL} with '{want}' failing",
               f"{c24.PIN_OK if ok else c24.PIN_FAIL}; failing {[l[:50] for l in fl][:3]}",
               not ok and any(l.startswith(want) for l in fl))
    rec, ok = c24.worktree_check(ctl, "t3_controls", control.restored_check)
    c24.write(os.path.join(out, "G"), "P-real-WORKTREE", c24.WT_OK, c24.WT_FAIL, "t3_controls.py G: "
              "c24.worktree_check on the preserved C24 worktree (read-only)", rec, ok)
    record("C24", "G worktree check: the preserved C24 worktree", c24.WT_OK, c24.WT_OK if ok else c24.WT_FAIL, ok)
    c = copy.deepcopy(ctl)
    c["preserved"]["lock_reason"] = K.C24_LOCK_REASON.replace("TRANCHE3-EXEC-01", "TRANCHE3-CONTINUE-02")
    rec, ok = c24.worktree_check(c, "t3_controls", control.restored_check)
    fl = failed_labels(rec)
    record("C24", "G worktree check: another lock reason expected", c24.WT_FAIL,
           f"{c24.WT_OK if ok else c24.WT_FAIL}; failing {[l[:40] for l in fl]}",
           not ok and fl and all(l.startswith("locked with the Tranche-3 reason") for l in fl))


def warning_group(variant, line, col=5, enum_line=630):
    """rustc's dead-code group for one AckRefusal variant, quoting C's (= the mutants') lines (synthetic)."""
    src = K.source(K.PROTOCOL).split("\n")
    return (f"warning: variant `{variant}` is never constructed\n"
            f"   --> crates/nexus-governed-control/src/ledger/protocol.rs:{line}:{col}\n    |\n"
            f"{enum_line} | {src[enum_line - 1]}\n    |                 ---------- variant in this enum\n...\n"
            f"{line} | {src[line - 1]}\n    |     {'^' * len(variant)}\n    |\n"
            "    = note: `AckRefusal` has derived impls for the traits `Clone` and `Debug`, but these are intentionally "
            "ignored during dead code analysis\n"
            "    = note: `#[warn(dead_code)]` (part of `#[warn(unused)]`) on by default\n\n")


def section_h(out, scratch, exp):
    print("H. the C3b/C3c build-admission checker (AMENDMENT-01) on synthetic build receipts (base: the accepted C2 build)")
    C = exp["controls"]
    base = f"{K.T3X_PACKET}/{K.T3X_RECEIPTS['C2']}/03-build"
    c2_wt = K.ACCEPTED_WT["C2"]
    summary = "warning: `nexus-governed-control` (lib test) generated 1 warning\n"
    finished = "    Finished `release` profile"

    def make(name, cid, stderr_fn=None, desc_fn=None):
        """A synthetic build receipt for cid in SCRATCH: the accepted C2 build receipt (its real target is C2's), with
        cid's control, patch and mutant identities, an optional stderr change, then the unchanged general classifier."""
        d = os.path.join(scratch, "H", name)
        os.makedirs(d)
        for f in K.git("ls-tree", "--name-only", K.T3X, base + "/").decode().split():
            n = os.path.basename(f)
            if n not in ("CHECKS.txt", "CLASSIFICATION.txt", "PARSED-RECEIPT.json", "PIN.json"):
                open(os.path.join(d, n), "wb").write(K.blob(K.T3X, f))
        ctl = C[cid]
        j = json.load(open(os.path.join(d, "descriptor.json")))
        j.update({"control": cid, "patch_sha256": ctl["patch"]["sha256"], "mutant_blob": ctl["mutant_blob"],
                  "mutant_sha256": ctl["mutant_sha256"], "synthetic": "t3_controls.py H: a SYNTHETIC build receipt "
                  "transformed from the accepted C2 build receipt; not the record of any build"})
        for when in ("source_before", "source_after"):
            j[when]["mutant_file"].update({"git_blob": ctl["mutant_blob"], "sha256": ctl["mutant_sha256"]})
        if stderr_fn:
            t = stderr_fn(open(os.path.join(d, "build.stderr"), encoding="utf-8").read())
            open(os.path.join(d, "build.stderr"), "w", encoding="utf-8").write(t)
            j["stderr_sha256"] = K.sha256_bytes(t.encode())
        if desc_fn:
            desc_fn(j)
        json.dump(j, open(os.path.join(d, "descriptor.json"), "w"), indent=1)
        classify.classify_build_success(d, "pin", "EXACTLY_C_PLUS_PATCH", C[cid]["mutant_blob"],
                                        expect_target=os.path.join(c2_wt, "target"))
        return d

    def warn(*groups, summ=summary):
        def f(t):
            assert t.count(finished) == 1
            return t.replace(finished, "".join(groups) + summ + finished, 1)
        return f
    l1643 = K.source(K.PROTOCOL).split("\n")[1642]  # unchanged by the C3b and C3c patches (before their hunk)
    col = l1643.index("set =") + 1
    other = ("warning: unused variable: `set`\n"
             f"    --> crates/nexus-governed-control/src/ledger/protocol.rs:1643:{col}\n     |\n1643 | {l1643}\n"
             f"     | {' ' * (col - 1)}^^^ help: if this is intentional, prefix it with an underscore: `_set`\n\n")
    two = "warning: `nexus-governed-control` (lib test) generated 2 warnings\n"
    fut, st = warning_group("Future", 635), warning_group("Set", 636)
    err = ("error[E0425]: cannot find value `x` in this scope\n"
           "   --> crates/nexus-governed-control/src/ledger/protocol.rs:1666:9\n\n")

    def exit101(j):
        j["process"]["exit_status"] = 101
    cases = [
        ("P-C3b-clean-build", "C3b", None, None, True, c3w.ADMIT_CLEAN),
        ("P-C3b-exactly-the-Future-warning", "C3b", warn(fut), None, True, c3w.ADMIT_NARROW),
        ("P-C3c-clean-build", "C3c", None, None, True, c3w.ADMIT_CLEAN),
        ("P-C3c-exactly-the-Set-warning", "C3c", warn(st), None, True, c3w.ADMIT_NARROW),
        ("N-C3b-Future-warning-plus-an-extra-warning", "C3b", warn(fut, other, summ=two), None, False, c3w.ADMIT_FAIL),
        ("N-C3b-the-Set-warning", "C3b", warn(st), None, False, c3w.ADMIT_FAIL),
        ("N-C3c-Set-warning-plus-an-extra-warning", "C3c", warn(st, other, summ=two), None, False, c3w.ADMIT_FAIL),
        ("N-C3c-the-Future-warning", "C3c", warn(fut), None, False, c3w.ADMIT_FAIL),
        ("N-C3b-another-warning-only", "C3b", warn(other), None, False, c3w.ADMIT_FAIL),
        ("N-C3b-a-compiler-error-plus-the-warning (exit 101)", "C3b", warn(err, fut), exit101, False, c3w.ADMIT_FAIL),
        ("N-C3b-the-Future-warning-at-another-location (636:5)", "C3b",
         warn(fut.replace("protocol.rs:635:5", "protocol.rs:636:5")), None, False, c3w.ADMIT_FAIL),
        ("N-C3b-the-Future-warning-with-a-two-warning-summary", "C3b", warn(fut, summ=two), None, False,
         c3w.ADMIT_FAIL),
        ("N-C3b-build-identity-mismatch (C3c's mutant blob in the source record)", "C3b", warn(fut),
         lambda j: j["source_after"]["mutant_file"].__setitem__("git_blob", C["C3c"]["mutant_blob"]), False,
         c3w.ADMIT_FAIL),
    ]
    for n, (case, cid, sfn, dfn, want_ok, want_first) in enumerate(cases, 1):
        d = make(f"{n:02d}-{case[:50]}", cid, sfn, dfn)
        ok, pin, first = c3w.admit(d, C[cid], os.path.join(out, "H", f"{n:02d}-{re.sub(r'[^A-Za-z0-9_.-]+', '-', case)[:60]}"),
                                   worktree=c2_wt)
        gen = supervise.first_line(os.path.join(d, "CLASSIFICATION.txt"))
        good = ok == want_ok and first == f"{cid} {want_first}" and (pin is not None) == want_ok
        if want_ok and pin:
            pj = json.load(open(pin))
            good = good and pj["path"].startswith(c2_wt + "/target/release/deps/")
        record(cid, "H c3w.admit: " + case, f"{cid} {want_first}", f"{first}; general classifier {gen}; pin "
               f"{os.path.relpath(pin, K.EV) if pin else None}", good)
    for cid in ("C24", "C6", "C7", "A1", "A2", "C20", "A5"):
        d = make(f"refused-{cid}-with-the-Future-warning", cid if cid != "C24" else "C6", warn(fut))
        ok, pin, first = c3w.admit(d, C[cid], os.path.join(out, "H", f"refused-{cid}"), worktree=c2_wt)
        gen = supervise.first_line(os.path.join(d, "CLASSIFICATION.txt"))
        fails = [l[5:].split(":")[0] for l in open(os.path.join(d, "CHECKS.txt")).read().splitlines()
                 if l.startswith("FAIL")]
        record(cid, "H the C3b/C3c rule refused for " + cid + "; the unchanged general classifier rejects its warning",
               f"{cid} {c3w.ADMIT_FAIL} (the control is not C3b or C3c); general BUILD: NOT OK (warning check)",
               f"{first}; general {gen}; failing {fails}", not ok and pin is None and first == f"{cid} {c3w.ADMIT_FAIL}"
               and gen == "BUILD: NOT OK" and fails == ["no error or warning diagnostic"])


def section_f(exp):
    print("F. control.main refusals and control.gate before C24's RESULT and before the pre-execution gate (read-only)")
    before = sorted(os.listdir(os.path.join(K.EV, "receipts")))
    for cid in ("C2", "C5", "C22", "C1", "C4", "C25"):
        try:
            control.main(cid)
            got = "NOT REFUSED"
        except SystemExit as e:
            got = str(e)
        record(cid, "F control.main refuses; nothing written under receipts/", "REFUSED", got[:90],
               got.startswith("REFUSED: ") and sorted(os.listdir(os.path.join(K.EV, "receipts"))) == before)
    problems, _ = control.gate("C3b", exp)
    record("C3b", "F control.gate before C24's RESULT exists", "closed: the previous control's RESULT is not a candidate",
           "; ".join(problems)[:120], any(exp["controls"]["C24"]["dir"] + "/RESULT.txt" in p for p in problems))
    problems, _ = control.gate("C24", exp)
    record("C24", "F control.gate before the freeze and the pre-execution gate", "closed",
           "; ".join(problems)[:120], any("06-PRE-EXECUTION-GATE" in p for p in problems)
           and any("freeze" in p for p in problems))


def main(out, scratch):
    for p in (out, scratch):
        if os.path.exists(p):
            raise SystemExit(f"REFUSED: {p} exists")
    os.makedirs(out)
    os.makedirs(scratch)
    exp = supervise.expectations()
    print(f"T3 CONTROLS — {K.MISSION} (synthetic and historical evidence only; no Cargo, rustc, test or Nexus executable)")
    section_a(out, exp)
    section_b(out, scratch, exp)
    section_c(out, scratch, exp)
    section_d(out, scratch, exp)
    section_e(out, scratch, exp)
    section_f(exp)
    section_g(out, scratch, exp)
    section_h(out, scratch, exp)
    bad = [r for r in results if not r["as_expected"]]
    lines = [f"{'AS EXPECTED' if r['as_expected'] else 'UNEXPECTED '} {r['control']:4} {r['case']}: expected "
             f"{r['expected']}; got {r['got']}{(' — ' + r['detail']) if r['detail'] else ''}" for r in results]
    open(os.path.join(out, "CONTROLS.txt"), "w").write("\n".join(lines) + "\n")
    json.dump(results, open(os.path.join(out, "CONTROLS.json"), "w"), indent=1)
    verdict = "T3 CONTROLS: " + ("ALL AS EXPECTED" if not bad else "FAILED")
    open(os.path.join(out, "SUMMARY.txt"), "w").write(
        f"{len(results)} controls; {len(results) - len(bad)} as expected; {len(bad)} unexpected\n"
        + "".join(f"UNEXPECTED: {r['control']} {r['case']}: {r['got']}\n" for r in bad) + verdict + "\n")
    print(verdict)
    return 0 if not bad else 1


if __name__ == "__main__":
    sys.exit(main(os.path.abspath(sys.argv[1]), os.path.abspath(sys.argv[2])))

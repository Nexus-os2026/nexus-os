#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-MUTATION-TRANCHE2-CONTINUE-02, mission section 16: test this mission's tooling on SYNTHETIC and
HISTORICAL evidence only, before the freeze. No Cargo, no rustc, no Nexus build, test or executable is run; the
preserved C5 worktree, target and executable are read, never written.

    c2_controls.py OUTDIR SCRATCH      both must not exist. OUTDIR (receipts/03-c2-controls) receives the synthetic
                                       receipts this tool classifies or links, the admission and pin records of the
                                       controls, CONTROLS.txt, CONTROLS.json and SUMMARY.txt (last line `C2 CONTROLS: ALL
                                       AS EXPECTED` or `C2 CONTROLS: FAILED`). SCRATCH (under dev-local/, never
                                       published) holds every copy of historical evidence (the C5 build receipt and its
                                       tampered copies, the general classifier's rerun, tampered input roots), so that no
                                       published file repeats a previous packet's bytes.

A  classify_a (unchanged) on synthetic type-(a) receipts for C5 A3 A6 C8 C15, transformed field by field from a
   historical receipt (CONTINUE-02's C9 run, receipts/38-C9/04-run at ebe42009), each marked SYNTHETIC with this
   mission's name. Positive: each control's first permitted property; A6's P15; C5 classified as main_c5 does (source
   EXACTLY_C, the preserved worktree); C15 at its 30,000,000 ceiling. Negative: A6 with C's key fields (no
   recovery_required) and with the phrase removed; A3/A6 at C's verdict-assertion line; A3 with another property; C8
   under the composed quotient; C15 INCOMPLETE at the cap; C15 FAIL beyond the cap; C5 another executable; C5 with the
   worktree not exactly C; C5 classified as a patched control would be (EXACTLY_C_PLUS_PATCH expected); a PASS; a kill
   from outside.
B  control.link (unchanged) on the real authenticated linked inputs for the five controls: LINK: VERIFIED; negative
   (tampered copies in SCRATCH): a changed F-5 raw output (C5), another test (A3), other bounds (C8), a changed proof and
   an F-R receipt recorded as PASS (C15): LINK: FAILED with the intended check failing.
C  expectations.py on tampered inputs (catalog, a patch): PROBLEMS — STOP; on the real inputs: equal to the frozen
   receipts/02-EXPECTATIONS.json.
D  the C5 build-admission checker (c5.admit) and the unchanged general build classifier:
   P  the real preserved receipt: ADMISSIBLE; a synthetic unmodified copy: only the synthetic line fails;
   P  classify_build_success (unchanged), rerun on a copy of the historical receipt: BUILD: NOT OK, and its CHECKS.txt,
      CLASSIFICATION.txt and PARSED-RECEIPT.json byte-identical to the historical ones (no PIN.json);
   N  on tampered copies (synthetic mode: the identity line and exactly the targeted check fail): one permitted warning
      missing; an extra warning; a wrong warning line; an error plus the warnings; a sixth warning in cargo's summary;
      an unclassified diagnostic line; the wrong mutant source; the wrong executable hash; the wrong executable size;
      exit 101; an OOM kill; a timeout;
   N  the same five warnings presented for A3, A6, C8 and C15: refused (the ruling is C5's); and the general classifier
      on a synthetic A3 build carrying them: BUILD: NOT OK with exactly the warning check failing;
   N  a byte-changed copy in normal mode: the preserved-receipt identity check fails.
E  the preserved-executable pin (c5.pin) and worktree check (c5.worktree_check), read-only: the real artifact PINNED
   and the worktree AUTHENTICATED; negative: a missing executable, a wrong hash, a wrong size, another path (a symlink
   in SCRATCH to the real executable), a wrong lock reason: each C5_PRESERVED_ARTIFACT_BLOCKED or NOT AUTHENTICATED.
F  control.main's refusals (C23 and C22 accepted, C1 not authorized) and control.gate for A3 before any C5 RESULT:
   refused or closed, nothing written.
"""
import copy
import json
import os
import re
import shutil
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import c5  # noqa: E402
import classify  # noqa: E402
import common as K  # noqa: E402
import control  # noqa: E402
import supervise  # noqa: E402

BASE = K.CONTINUE02_PACKET + "/receipts/38-C9/04-run"
BASE_TEST = K.TEST_PREFIX + "full_fb2"
MARK = "SYNTHETIC (c2_controls.py, P3-C11-DR01-D2A1-MUTATION-TRANCHE2-CONTINUE-02): not the output of any run\n"
results = []


def record(cid, case, expected, got, ok, detail=""):
    results.append({"control": cid, "case": case, "expected": expected, "got": got, "as_expected": bool(ok),
                    "detail": str(detail)})
    print(f"{'AS EXPECTED' if ok else 'UNEXPECTED '} {cid:4} {case}: {got}{(' — ' + str(detail)) if detail else ''}")


def exe_for(cid):
    sha = K.sha256_bytes(f"SYNTHETIC executable of {cid} (c2_controls.py; never built, never run)".encode())
    path = f"/synthetic/nexus-os-p3-c11-mt2c-{cid}/target/release/deps/nexus_governed_control-0123456789abcdef"
    return {"path": path, "sha256": sha, "size": 4343}


def synth(d, ctl, exe, prop=None, msg_prop=None, line=None, states=None, key_fields=None, header=None, test=None,
          source_verdict="EXACTLY_C_PLUS_PATCH", raw=None, process=None, sup=None):
    """A synthetic type-(a) receipt directory d (descriptor.json, raw-output.txt) for control ctl (t2_controls.py's
    transformation, with this mission's marker)."""
    os.makedirs(d)
    desc = json.loads(K.blob(K.T1, BASE + "/descriptor.json"))
    t = test or ctl["test"]
    if raw is None:
        raw = K.blob(K.T1, BASE + "/raw-output.txt").decode()
        raw = raw.replace(BASE_TEST, t).replace("EXPLORATION F-B2", header or f"EXPLORATION {ctl['configuration']}")
        raw = re.sub(r"^  bounds: .*$", lambda m: "  bounds: " + ctl["bounds"], raw, flags=re.M)
        raw = re.sub(r"^  actors: .*$", lambda m: "  actors: " + ctl["actors"], raw, flags=re.M)
        p = prop or (ctl.get("permitted_properties") or [ctl["named_property"]])[0]
        raw = raw.replace("  FAIL: E4: ", f"  FAIL: {p}: ")
        raw = raw.replace('failed: failed: {"E4"}', f'failed: failed: {{"{msg_prop or p}"}}')
        va = ctl["verdict_assertion"]
        raw = raw.replace("model_tests.rs:4133:5:", f"model_tests.rs:{line or va['line']}:{va['col']}:")
        if states is not None:
            raw = re.sub(r"^  states 14 \(expanded 1\)", lambda m: f"  states {states} (expanded 1)", raw, flags=re.M)
        if key_fields is not None:
            raw = re.sub(r"^  state key fields: .*$", lambda m: "  state key fields: " + key_fields, raw, flags=re.M)
    raw = MARK + raw
    open(os.path.join(d, "raw-output.txt"), "w").write(raw)
    ident = {"path": exe["path"], "sha256": exe["sha256"], "size": exe["size"], "mtime_ns": 0, "inode": 0, "symlink": False}
    listing = {"exit": 0, "stdout": f"{t}: test\n\n1 test, 0 benchmarks\n", "stderr": "", "listed": [f"{t}: test"]}
    desc.update({"id": "04-run", "test": t, "argv": [exe["path"], t, *ctl["argv_tail"][1:]],
                 "configuration": ctl["configuration"], "control": ctl["id"], "mutant_blob": ctl["mutant_blob"],
                 "patch_sha256": ctl["patch"]["sha256"], "worktree": ctl["worktree"], "limit_seconds": ctl["limit_seconds"],
                 "cwd": os.path.join(ctl["worktree"], K.CRATE), "raw_output_sha256": K.sha256_bytes(raw.encode()),
                 "synthetic": "c2_controls.py: a SYNTHETIC receipt transformed from CONTINUE-02 receipts/38-C9/04-run; "
                              "not Nexus evidence of any run",
                 "executable": {"path": exe["path"], "pinned_sha256": exe["sha256"], "pinned_size": exe["size"],
                                "before": ident, "after": ident},
                 "selection": {"listing": dict(listing, argv=[t, "--exact", "--list"]),
                               "ignored_listing": dict(listing, argv=[t, "--exact", "--ignored", "--list"]),
                               "expected_ignored": True, "executable_after_listing": ident}})
    for when in ("source_before", "source_after"):
        s = desc[when]
        s.update({"worktree": ctl["worktree"], "verdict": source_verdict,
                  "mutant_file": dict(s["mutant_file"], path=ctl["source_file"], git_blob=ctl["mutant_blob"],
                                      sha256=ctl["mutant_sha256"], baseline_blob=ctl["baseline_blob"]),
                  "expected_mutant_blob": ctl["mutant_blob"], "expected_mutant_sha256": ctl["mutant_sha256"]})
        if source_verdict in ("EXACTLY_C", "NOT_C"):
            s["mutant_file"] = None  # a restored worktree: no mutant file (control.restored_check's record)
    if process:
        desc["process"] = process
    # A distinctive synthetic resource record, so that no synthetic classification repeats a previous packet's bytes.
    desc["supervisor"] = dict(desc["supervisor"], max_rss_kib_largest_process=4343, **(sup or {}))
    json.dump(desc, open(os.path.join(d, "descriptor.json"), "w"), indent=1)


def first_two(d):
    ls = open(os.path.join(d, "CLASSIFICATION.txt")).read().splitlines()
    return ls[0], ls[1].replace("outer class (AD-1-R4 K.2): ", "")


def section_a(out, exp):
    print("A. classify_a (unchanged) on synthetic type-(a) receipts (historical base: CONTINUE-02 C9 run)")
    C = exp["controls"]
    OK = "FAIL — admissible type-(a) original-explorer mutant FAIL of the named property"
    cases = [(cid, "P-first-permitted-property", {}, "patched", classify.CANDIDATE, OK) for cid in K.ORDER if cid != "C5"]
    cases += [
        ("C5", "P-as-main_c5-classifies-it: source EXACTLY_C (the preserved worktree), P13", {"source_verdict": "EXACTLY_C"},
         "c5", classify.CANDIDATE, OK + " P13"),
        ("C5", "N-classified-as-a-patched-control (EXACTLY_C_PLUS_PATCH expected, the worktree is C)",
         {"source_verdict": "EXACTLY_C"}, "patched", classify.NOT_CANDIDATE, "NOT ADMISSIBLE (source EXACTLY_C_PLUS_PATCH"),
        ("C5", "N-the-worktree-not-exactly-C", {"source_verdict": "NOT_C"}, "c5", classify.NOT_CANDIDATE,
         "NOT ADMISSIBLE (source EXACTLY_C before and after"),
        ("C5", "N-another-executable", {"source_verdict": "EXACTLY_C", "exe": "other"}, "c5", classify.NOT_CANDIDATE,
         "NOT ADMISSIBLE (executable is the pinned one"),
        ("C5", "N-wrong-property-P12", {"source_verdict": "EXACTLY_C", "prop": "P12"}, "c5", classify.NOT_CANDIDATE,
         "FAIL of P12, not the named property P13: kills nothing (no attribution exception)"),
        ("C5", "N-PASS-on-the-mutant", {"source_verdict": "EXACTLY_C", "raw": "F5PASS"}, "c5", classify.NOT_CANDIDATE,
         "PASS on the mutant (the control survives F-5): kills nothing"),
        ("A6", "P-second-permitted-property-P15", {"prop": "P15"}, "patched", classify.CANDIDATE, OK + " P12 or P15: P15"),
        ("A6", "N-key-fields-of-C-without-recovery_required", {"key_fields": "C"}, "patched", classify.NOT_CANDIDATE,
         "NOT ADMISSIBLE (the printed state key fields are the mutant's KEY_FIELDS"),
        ("A6", "N-recovery_required-removed-from-the-printed-key-fields", {"key_fields": "NO-RR"}, "patched",
         classify.NOT_CANDIDATE, "NOT ADMISSIBLE (the printed state key fields are the mutant's KEY_FIELDS"),
        ("A6", "N-property-outside-the-pair-P13", {"prop": "P13"}, "patched", classify.NOT_CANDIDATE,
         "FAIL of P13, not a permitted property (P12 or P15): kills nothing"),
        ("A6", "N-verdict-assertion-at-C's-line-4133", {"line": 4133}, "patched", classify.NOT_CANDIDATE,
         "NOT ADMISSIBLE (the panic is the explorer's own verdict assertion"),
        ("A3", "N-verdict-assertion-at-C's-line-4133", {"line": 4133}, "patched", classify.NOT_CANDIDATE,
         "NOT ADMISSIBLE (the panic is the explorer's own verdict assertion"),
        ("A3", "N-wrong-property-P15", {"prop": "P15"}, "patched", classify.NOT_CANDIDATE,
         "FAIL of P15, not the named property P12: kills nothing (no attribution exception)"),
        ("C8", "N-composed-quotient-header (no quotient for a mutant)", {
            "header": "EXPLORATION F-B (gating-caller symmetry quotient composed with the admission observer quotient: "
                      "experimental, not the contract's explorer)"}, "patched", classify.NOT_CANDIDATE,
         "NOT ADMISSIBLE (header 'EXPLORATION F-B' exactly"),
        ("C8", "N-wrong-property-E4", {"prop": "E4"}, "patched", classify.NOT_CANDIDATE,
         "FAIL of E4, not the named property E5: kills nothing (no attribution exception)"),
        ("C15", "P-FAIL-at-the-30M-ceiling", {"states": 30000000}, "patched", classify.CANDIDATE, OK + " E8"),
        ("C15", "N-FAIL-beyond-the-30M-ceiling (cap overshoot)", {"states": 30000001}, "patched", classify.NOT_CANDIDATE,
         "INCOMPLETE — BUDGET EXCEEDED (a FAIL printed beyond the authorised cardinality): kills nothing"),
        ("C15", "N-INCOMPLETE-at-the-cap", {"raw": "FR"}, "patched", classify.NOT_CANDIDATE,
         "INCOMPLETE (the state cap of 30000000 was reached. No property is established by this run.): kills nothing"),
        ("A3", "N-killed-from-outside", {"process": {"exit_status": None, "signal": 9, "supervisor_rc": -9,
                                                     "wrapper_recorded_status": "137"},
                                         "sup": {"timed_out": True, "wall_seconds": 2700.0}, "raw": "TRUNCATED"},
         "patched", classify.NOT_CANDIDATE, "INCOMPLETE (killed from outside or abnormal termination): kills nothing"),
    ]
    fr_raw = open(K.inputs("linked", "S2-69-full-F-R", "raw-output.txt"), encoding="utf-8").read()
    f5_raw = open(K.inputs("linked", "S2-65-full-F-5", "raw-output.txt"), encoding="utf-8").read()
    base_raw = K.blob(K.T1, BASE + "/raw-output.txt").decode()
    for n, (cid, case, kw, mode, want_first, want_outer) in enumerate(cases, 1):
        ctl = C[cid]
        exe = exe_for(cid)
        kw = dict(kw)
        if kw.get("key_fields") == "C":
            kw["key_fields"] = [l[len("  state key fields: "):] for l in base_raw.splitlines()
                                if l.startswith("  state key fields: ")][0]
        elif kw.get("key_fields") == "NO-RR":
            kw["key_fields"] = ctl["key_fields"].replace(", recovery_required)", ")")
            assert kw["key_fields"] != ctl["key_fields"]
        elif cid == "A6" and "key_fields" not in kw:
            kw["key_fields"] = ctl["key_fields"]
        if kw.get("raw") == "FR":
            kw["raw"] = fr_raw
        elif kw.get("raw") == "F5PASS":
            kw["raw"] = f5_raw
            kw["process"] = {"exit_status": 0, "signal": None, "supervisor_rc": 0, "wrapper_recorded_status": "0"}
        elif kw.get("raw") == "TRUNCATED":
            kw["raw"] = "\n".join(base_raw.replace(BASE_TEST, ctl["test"]).splitlines()[:6]) + "\n"
        run_exe = exe
        if kw.pop("exe", None) == "other":
            run_exe = dict(exe, sha256=K.sha256_bytes(b"another executable"))
        d = os.path.join(out, "A", f"{n:02d}-{cid}-{re.sub(r'[^A-Za-z0-9_.-]+', '-', case)[:60].strip('-')}")
        synth(d, ctl, exe, **kw)
        if mode == "c5":
            classify.classify_a(d, dict(ctl, mutant_blob=None), run_exe, source_verdict=ctl["preserved"]["run_source_verdict"])
        else:
            classify.classify_a(d, ctl, run_exe)
        first, outer = first_two(d)
        ok = first == want_first and outer.startswith(want_outer)
        record(cid, "A " + case, f"{want_first} / {want_outer}", f"{first} / {outer}", ok)


def section_b(out, scratch, exp):
    print("B. control.link (unchanged) on the authenticated linked inputs (and on tampered copies)")
    C = exp["controls"]
    for cid in K.ORDER:
        d = os.path.join(out, "B", f"{cid}-real-inputs")
        ok, ref = control.link(C[cid], exp, d, None)
        first = supervise.first_line(os.path.join(d, "LINK.txt"))
        record(cid, "B link on the real authenticated inputs", "LINK: VERIFIED", first, first == "LINK: VERIFIED", ref[:90])
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
    neg = [
        ("C5", "N-F-5-raw-output-byte-changed", lambda: tamper("inputs/linked/S2-65-full-F-5/raw-output.txt",
                                                             b"outcome: PASS", b"outcome: PASs"), None,
         "its classification blob and raw output identity are the catalog's"),
        ("A3", "N-another-test-in-the-expectation", None, lambda c: c["argv_tail"].__setitem__(0, K.TEST_PREFIX + "full_f4"),
         "exact configuration: the same test and flags as this control's run (F-5)"),
        ("C8", "N-other-bounds-in-the-expectation", None, lambda c: c.__setitem__("bounds", c["bounds"].replace(
            "faults 2", "faults 3")), "F-B's restored side (AD-1-R4 F)"),
        ("C15", "N-proof-changed", lambda: tamper("inputs/FR-STUTTERING-REFINEMENT-PROOF.md", b"DRAFT", b"DRAFt"), None,
         "the accepted F-R proof: sha256 and blob as the catalog and the adoption record give them"),
        ("C15", "N-F-R-receipt-recorded-as-PASS", lambda: tamper("inputs/linked/S2-69-full-F-R/CLASSIFICATION.txt",
                                                               b"CLASSIFICATION: NOT PASS", b"CLASSIFICATION: PASS"), None,
         "F-R's own receipt stays INCOMPLETE"),
    ]
    try:
        for cid, case, files, ctlmod, want in neg:
            fresh()
            if files:
                files()
            ctl = copy.deepcopy(C[cid])
            if ctlmod:
                ctlmod(ctl)
            d = os.path.join(out, "B", f"{cid}-{case}")
            ok, ref = control.link(ctl, exp, d, None)
            text = open(os.path.join(d, "LINK.txt")).read().splitlines()
            fails = [l for l in text if l.startswith("FAIL")]
            hit = [l for l in fails if want in l]
            record(cid, "B " + case, f"LINK: FAILED with the check '{want[:70]}…' failing", f"{text[0]}; {len(fails)} failed",
                   text[0] == "LINK: FAILED" and not ok and hit, hit[:1] or fails[:2])
            K.EV = saved
    finally:
        K.EV = saved


def section_c(out, scratch, exp):
    print("C. expectations.py on tampered inputs, and on the real inputs")
    import expectations as E
    saved = K.EV
    root = os.path.join(scratch, "C-ev")
    for case, rel, old, new, want in (
            ("N-catalog-copy-changed", "inputs/catalog.json", b'"P12 or P15"', b'"P12 or P16"',
             "authenticated catalog copy sha256"),
            ("N-patch-copy-changed", "inputs/patches/C15.patch", b"&& token.cell == cell", b"&& token.cell == cell ",
             "C15 authenticated patch copy sha256")):
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


def failed_labels(rec):
    return [l for l, o, _ in rec.get("checks", []) if not o]


def section_d(out, scratch, exp):
    print("D. the C5 build-admission checker and the unchanged general build classifier")
    C = exp["controls"]
    ctl = C["C5"]
    real = K.inputs("c5", "03-build")
    rec, ok = c5.admit(real, ctl)
    c5.write(os.path.join(out, "D"), "P-real-preserved-receipt-ADMISSION", c5.ADMIT_OK, c5.ADMIT_FAIL,
             "c2_controls.py D: c5.admit on the real authenticated copy of the preserved Tranche-2 build receipt", rec, ok)
    record("C5", "D admit: the real preserved receipt", c5.ADMIT_OK, c5.ADMIT_OK if ok else c5.ADMIT_FAIL, ok,
           f"{sum(o for _, o, _ in rec['checks'])} of {len(rec['checks'])} checks hold")
    synth_label = "SYNTHETIC CONTROL (c2_controls.py): a tampered copy, never admissible; the preserved-receipt identity check is not run"

    def copy_build(name):
        d = os.path.join(scratch, "D", name)
        shutil.copytree(real, d)
        for f in os.listdir(d):
            os.chmod(os.path.join(d, f), 0o644)
        return d
    d0 = copy_build("S0-unmodified")
    rec, ok = c5.admit(d0, ctl, synthetic=True)
    fl = failed_labels(rec)
    record("C5", "D admit (synthetic mode): an unmodified copy: only the synthetic line fails",
           "REJECTED; failing exactly [synthetic line]", f"{'ADMISSIBLE' if ok else 'REJECTED'}; failing {len(fl)}",
           not ok and fl == [synth_label], fl[:3])
    # The general classifier (unchanged) on a copy of the historical receipt, with its three outputs removed.
    g = os.path.join(scratch, "D", "general-classifier-rerun")
    shutil.copytree(real, g, ignore=shutil.ignore_patterns("CHECKS.txt", "CLASSIFICATION.txt", "PARSED-RECEIPT.json"))
    for f in os.listdir(g):
        os.chmod(os.path.join(g, f), 0o644)
    rc = classify.classify_build_success(g, "pin", "EXACTLY_C_PLUS_PATCH", ctl["mutant_blob"],
                                         expect_target=os.path.join(K.C5_WT, "target"))
    same = {f: K.sha256_file(os.path.join(g, f)) == K.sha256_file(os.path.join(real, f)) for f in
            ("CHECKS.txt", "CLASSIFICATION.txt", "PARSED-RECEIPT.json")}
    first = supervise.first_line(os.path.join(g, "CLASSIFICATION.txt"))
    record("C5", "D the general classifier (classify_build_success, unchanged) rerun on a copy of the historical receipt",
           "BUILD: NOT OK; CHECKS.txt, CLASSIFICATION.txt, PARSED-RECEIPT.json byte-identical to the historical ones; "
           "no PIN.json", f"{first}; rc {rc}; identical {same}; PIN.json {os.path.exists(os.path.join(g, 'PIN.json'))}",
           first == "BUILD: NOT OK" and rc == 1 and all(same.values()) and not os.path.exists(os.path.join(g, "PIN.json")))
    stderr = open(os.path.join(real, "build.stderr"), encoding="utf-8").read()
    w4 = ("warning: variant `Inconsistent` is never constructed\n" + stderr.split(
        "warning: variant `Inconsistent` is never constructed\n", 1)[1].split("\n\n", 1)[0] + "\n\n")
    assert w4 in stderr
    l1818 = K.source(K.PROTOCOL).split("\n")[1817]
    col = l1818.index("set:") + 1
    extra = ("warning: unused variable: `set`\n"  # a well-formed group on the real line: only the five-group check fails
             f"    --> crates/nexus-governed-control/src/ledger/protocol.rs:1818:{col}\n     |\n1818 | {l1818}\n"
             f"     | {' ' * (col - 1)}^^^ help: if this is intentional, prefix it with an underscore: `_set`\n\n")

    def desc_mod(f):
        def m(d):
            p = os.path.join(d, "descriptor.json")
            j = json.load(open(p))
            f(j)
            json.dump(j, open(p, "w"), indent=1)
        return m

    def stderr_mod(old, new):
        def m(d):
            p = os.path.join(d, "build.stderr")
            t = open(p, encoding="utf-8").read()
            assert old in t, old[:60]
            t = t.replace(old, new, 1)
            open(p, "w", encoding="utf-8").write(t)
            desc_mod(lambda j: j.__setitem__("stderr_sha256", K.sha256_bytes(t.encode())))(d)
        m.stderr = True  # a changed stderr also fails its identity check (section 2's sha256)
        return m
    J5 = "J exactly the five permitted warning groups, each once"
    neg = [
        ("N-one-permitted-warning-missing (warning 4 removed)", stderr_mod(w4, ""), [J5, "J warning 4 present"]),
        ("N-an-extra-warning", stderr_mod("warning: `nexus-governed-control` (lib test) generated 5 warnings",
                                          extra + "warning: `nexus-governed-control` (lib test) generated 5 warnings"),
         [J5]),
        ("N-wrong-warning-line (1818:47 -> 1819:47)", stderr_mod("protocol.rs:1818:47", "protocol.rs:1819:47"),
         ["J warning 1 (unused variable: `nonce`): primary location"]),
        ("N-an-error-plus-the-warnings", stderr_mod("warning: unused variable: `nonce`\n",
                                                    "error[E0425]: cannot find value `x` in this scope\n"
                                                    "    --> crates/nexus-governed-control/src/ledger/protocol.rs:1818:47"
                                                    "\n\nwarning: unused variable: `nonce`\n"),
         ["I no `error` diagnostic", J5]),
        ("N-a-sixth-warning-in-cargo's-summary", stderr_mod("generated 5 warnings", "generated 6 warnings"),
         [J5, "K every line of every group", "K cargo's summary of exactly those five warnings",
          "K every other stderr line is cargo progress"]),
        ("N-an-unclassified-diagnostic-line", stderr_mod("    Finished `release`", "note: an extra note\n    Finished `release`"),
         ["K every other stderr line is cargo progress"]),
        ("N-wrong-mutant-source (A6's mutant blob before the build)", desc_mod(
            lambda j: j["source_before"]["mutant_file"].__setitem__("git_blob", C["A6"]["mutant_blob"])),
         ["A source before the build"]),
        ("N-wrong-executable-hash", desc_mod(lambda j: j["deps_artifacts"][0].__setitem__("sha256", "0" * 64)),
         ["M the recorded executable sha256 and size"]),
        ("N-wrong-executable-size", desc_mod(lambda j: j["deps_artifacts"][0].__setitem__("size", K.C5_EXE_SIZE + 1)),
         ["M the recorded executable sha256 and size"]),
        ("N-exit-101", desc_mod(lambda j: j["process"].__setitem__("exit_status", 101)), ["F normal exit 0"]),
        ("N-an-OOM-kill", desc_mod(lambda j: j["supervisor"].__setitem__("oom_kill", 1)), ["H no OOM and no signal"]),
        ("N-a-timeout", desc_mod(lambda j: j["supervisor"].__setitem__("timed_out", True)), ["G 16 GiB, zero swap"]),
    ]
    for n, (case, mod, want) in enumerate(neg, 1):
        d = copy_build(f"N{n:02d}")
        mod(d)
        rec, ok = c5.admit(d, ctl, synthetic=True)
        fl = failed_labels(rec)
        want = want + (["the stderr is the preserved one"] if getattr(mod, "stderr", False) else [])
        hit = all(any(l.startswith(w) for l in fl) for w in want)
        only = all(l == synth_label or any(l.startswith(w) for w in want) for l in fl)
        c5.write(os.path.join(out, "D"), f"N{n:02d}-ADMISSION", c5.ADMIT_OK, c5.ADMIT_FAIL,
                 f"c2_controls.py D: c5.admit (synthetic mode) on a tampered copy: {case}", rec, ok)
        record("C5", "D admit: " + case, f"REJECTED; failing the synthetic line and exactly {want}",
               f"{'ADMISSIBLE' if ok else 'REJECTED'}; failing {len(fl)}", not ok and hit and only,
               [l[:70] for l in fl if l != synth_label][:4])
    for cid in ("A3", "A6", "C8", "C15"):
        rec, ok = c5.admit(real, C[cid])
        fl = failed_labels(rec)
        record(cid, "D admit: the five C5 warnings presented for " + cid + " (the ruling does not transfer)",
               "REJECTED: the control is not C5", f"{'ADMISSIBLE' if ok else 'REJECTED'}; failing {fl[:1]}",
               not ok and fl and fl[0].startswith("the control is C5"))
    a3 = os.path.join(scratch, "D", "A3-with-C5-warnings")
    shutil.copytree(real, a3, ignore=shutil.ignore_patterns("CHECKS.txt", "CLASSIFICATION.txt", "PARSED-RECEIPT.json"))
    for f in os.listdir(a3):
        os.chmod(os.path.join(a3, f), 0o644)
    desc_mod(lambda j: [j[w]["mutant_file"].__setitem__("git_blob", C["A3"]["mutant_blob"])
                        for w in ("source_before", "source_after")])(a3)
    rc = classify.classify_build_success(a3, "pin", "EXACTLY_C_PLUS_PATCH", C["A3"]["mutant_blob"],
                                         expect_target=os.path.join(K.C5_WT, "target"))
    fails = [l[5:].split(":")[0] for l in open(os.path.join(a3, "CHECKS.txt")).read().splitlines() if l.startswith("FAIL")]
    record("A3", "D the general classifier on a synthetic A3 build carrying the five warnings",
           "BUILD: NOT OK with exactly the check 'no error or warning diagnostic' failing",
           f"{supervise.first_line(os.path.join(a3, 'CLASSIFICATION.txt'))}; failing {fails}",
           supervise.first_line(os.path.join(a3, "CLASSIFICATION.txt")) == "BUILD: NOT OK" and rc == 1
           and fails == ["no error or warning diagnostic"])
    d = copy_build("N-identity-byte-changed")
    p = os.path.join(d, "RECEIPT.txt")
    open(p, "a").write("\n")
    rec, ok = c5.admit(d, ctl)
    fl = failed_labels(rec)
    record("C5", "D admit (normal mode): a byte-changed copy of the preserved receipt",
           "REJECTED: the preserved-receipt identity check fails", f"{'ADMISSIBLE' if ok else 'REJECTED'}; failing {fl[:1]}",
           not ok and fl and fl[0].startswith("the build evidence is the preserved Tranche-2 receipt"))


def section_e(out, scratch, exp):
    print("E. the preserved-executable pin and the preserved worktree check (read-only)")
    ctl = exp["controls"]["C5"]
    bdir = K.inputs("c5", "03-build")
    rec, ok = c5.pin(ctl, bdir)
    c5.write(os.path.join(out, "E"), "P-real-PIN", c5.PIN_OK, c5.PIN_FAIL, "c2_controls.py E: c5.pin on the real "
             "preserved executable (read-only)", rec, ok)
    record("C5", "E pin: the real preserved executable", c5.PIN_OK, c5.PIN_OK if ok else c5.PIN_FAIL, ok,
           f"{sum(o for _, o, _ in rec['checks'])} of {len(rec['checks'])} checks hold")
    link = os.path.join(scratch, "E", "symlink-to-the-preserved-executable")
    os.makedirs(os.path.dirname(link))
    os.symlink(K.C5_EXE, link)
    for case, mod, want in (
            ("N-missing-executable", lambda c: c["preserved"]["executable"].__setitem__(
                "path", os.path.join(K.C5_WT, "target/release/deps/nexus_governed_control-0000000000000000")),
             "the preserved executable exists"),
            ("N-wrong-hash", lambda c: c["preserved"]["executable"].__setitem__("sha256", "1" * 64), "sha256 exact"),
            ("N-wrong-size", lambda c: c["preserved"]["executable"].__setitem__("size", K.C5_EXE_SIZE - 1), "size exact"),
            ("N-another-path (a symlink to the real executable)", lambda c: c["preserved"]["executable"].__setitem__(
                "path", link), "a regular file, not a symlink")):
        c = copy.deepcopy(ctl)
        mod(c)
        rec, ok = c5.pin(c, bdir)
        fl = failed_labels(rec)
        record("C5", "E pin: " + case, f"{c5.PIN_FAIL} with '{want}' failing",
               f"{c5.PIN_OK if ok else c5.PIN_FAIL}; failing {[l[:50] for l in fl][:3]}",
               not ok and any(l.startswith(want) for l in fl))
    rec, ok = c5.worktree_check(ctl, "c2_controls", control.restored_check)
    c5.write(os.path.join(out, "E"), "P-real-WORKTREE", c5.WT_OK, c5.WT_FAIL, "c2_controls.py E: c5.worktree_check on "
             "the preserved C5 worktree (read-only)", rec, ok)
    record("C5", "E worktree check: the preserved C5 worktree", c5.WT_OK, c5.WT_OK if ok else c5.WT_FAIL, ok)
    c = copy.deepcopy(ctl)
    c["preserved"]["lock_reason"] = K.C5_LOCK_REASON.replace("TRANCHE2-EXEC-01", "TRANCHE2-CONTINUE-02")
    rec, ok = c5.worktree_check(c, "c2_controls", control.restored_check)
    fl = failed_labels(rec)
    record("C5", "E worktree check: another lock reason expected", c5.WT_FAIL,
           f"{c5.WT_OK if ok else c5.WT_FAIL}; failing {[l[:40] for l in fl]}",
           not ok and fl and all(l.startswith("locked with the Tranche-2 reason") for l in fl))


def section_f(exp):
    print("F. control.main refusals and control.gate before any C5 RESULT (read-only)")
    before = sorted(os.listdir(os.path.join(K.EV, "receipts")))
    for cid in ("C23", "C22", "C1"):
        try:
            control.main(cid)
            got = "NOT REFUSED"
        except SystemExit as e:
            got = str(e)
        record(cid, "F control.main refuses; nothing written under receipts/", "REFUSED", got[:90],
               got.startswith("REFUSED: ") and sorted(os.listdir(os.path.join(K.EV, "receipts"))) == before)
    problems, _ = control.gate("A3", exp)
    record("A3", "F control.gate before C5's RESULT exists", "closed: the previous control's RESULT is not a candidate",
           "; ".join(problems)[:120], any("receipts/61-C5/RESULT.txt" in p for p in problems))


def main(out, scratch):
    for p in (out, scratch):
        if os.path.exists(p):
            raise SystemExit(f"REFUSED: {p} exists")
    os.makedirs(out)
    os.makedirs(scratch)
    exp = supervise.expectations()
    print(f"C2 CONTROLS — {K.MISSION} (synthetic and historical evidence only; no Cargo, rustc, test or Nexus executable)")
    section_a(out, exp)
    section_b(out, scratch, exp)
    section_c(out, scratch, exp)
    section_d(out, scratch, exp)
    section_e(out, scratch, exp)
    section_f(exp)
    bad = [r for r in results if not r["as_expected"]]
    lines = [f"{'AS EXPECTED' if r['as_expected'] else 'UNEXPECTED '} {r['control']:4} {r['case']}: expected "
             f"{r['expected']}; got {r['got']}{(' — ' + r['detail']) if r['detail'] else ''}" for r in results]
    open(os.path.join(out, "CONTROLS.txt"), "w").write("\n".join(lines) + "\n")
    json.dump(results, open(os.path.join(out, "CONTROLS.json"), "w"), indent=1)
    verdict = "C2 CONTROLS: " + ("ALL AS EXPECTED" if not bad else "FAILED")
    open(os.path.join(out, "SUMMARY.txt"), "w").write(
        f"{len(results)} controls; {len(results) - len(bad)} as expected; {len(bad)} unexpected\n"
        + "".join(f"UNEXPECTED: {r['control']} {r['case']}: {r['got']}\n" for r in bad) + verdict + "\n")
    print(verdict)
    return 0 if not bad else 1


if __name__ == "__main__":
    sys.exit(main(os.path.abspath(sys.argv[1]), os.path.abspath(sys.argv[2])))

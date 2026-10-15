#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-MUTATION-TRANCHE2-EXEC-01, mission section 7: test the Tranche-2 tooling on SYNTHETIC and
HISTORICAL evidence only, before the freeze. No Cargo, no rustc, no Nexus build, test or executable is run.

    t2_controls.py OUTDIR SCRATCH      both must not exist. OUTDIR (receipts/03-t2-controls) receives every synthetic
                                       receipt this tool classifies or links, CONTROLS.txt, CONTROLS.json and SUMMARY.txt
                                       (last line `T2 CONTROLS: ALL AS EXPECTED` or `T2 CONTROLS: FAILED`). SCRATCH (under
                                       dev-local/, never published) holds the old-classifier copies and the tampered
                                       evidence roots of the negative link and expectation controls.

A  classify_a (catalog-required change) on synthetic type-(a) receipts. The base is a historical receipt: CONTINUE-02's
   C9 run (receipts/38-C9/04-run at ebe42009, an accepted original-explorer F-B2 FAIL of E4), transformed field by field
   into each Tranche-2 control's expected run (test, configuration header, bounds, actors, printed property, the
   assertion message, the verdict-assertion location, the key fields, the counts, a synthetic executable identity, the
   mutant source identity). Positive: every one of the ten controls, and the second permitted property of C3a and A6, is
   a candidate. Negative, each with its stated outer class: a wrong property, a property outside C3a's and A6's pair, a
   message naming another property, A6 without the corrected KEY_FIELDS, A3/A6 with C's verdict-assertion line, C10 at
   and beyond its 60,000,000 ceiling, C15 INCOMPLETE at the cap and FAIL beyond it, C8 under the composed quotient, a
   PASS, a kill from outside, the source not the mutant, a wrong executable, a wrong test. For every control with one
   permitted property, Tranche 1's classify.py (original-tools/, run in a separate process) writes byte-identical
   CHECKS.txt, CLASSIFICATION.txt and PARSED-RECEIPT.json on an identical copy (in SCRATCH): the change is inert there.
B  control.link (restored-evidence mappings) on the real authenticated linked inputs, for each of the ten controls:
   LINK: VERIFIED. Negative (a tampered copy of the inputs in SCRATCH, or one changed expectation): a changed raw
   output, another test (e1), other bounds (C8), the composed-quotient receipt without its quotient flag, a changed
   proof, a changed Owner adoption record, an F-R receipt recorded as PASS, another executable hash, prohibited F-R
   wording: each LINK: FAILED with the intended check failing.
C  expectations.py on tampered authenticated inputs (catalog, a patch): PROBLEMS — STOP; on the real inputs: equal to
   the frozen receipts/02-EXPECTATIONS.json.
D  apply / verify / restore (control.apply_patch, supervise.mutant_check, control.restore, unchanged) with SYNTHETIC
   comment-only patches of protocol.rs and model_tests.rs (A3 and A6 mutate model_tests.rs; Tranche 1 mutated only
   protocol.rs) in one mission-owned development worktree at C (created and locked here with its role; never used for a
   control): EXACTLY_C_PLUS_PATCH, then RESTORE: EXACTLY_C; negative: an untracked file, a second changed file, the
   wrong expected blob: NOT_THE_MUTANT (each undone by explicit file operations and verified).
"""
import copy
import json
import os
import re
import shutil
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import classify  # noqa: E402
import common as K  # noqa: E402
import control  # noqa: E402
import supervise  # noqa: E402
import worktrees  # noqa: E402

BASE = K.CONTINUE02_PACKET + "/receipts/38-C9/04-run"
BASE_TEST = K.TEST_PREFIX + "full_fb2"
ORIG = os.path.join(K.EV, "original-tools")
DEV_WT = K.NEXUS + "/nexus-os-p3-c11-mt2x-dev-rehearsal"
DEV_ROLE = ("development apply/verify/restore rehearsal (synthetic comment-only patches of protocol.rs and model_tests.rs; "
            "never built, never used for a control)")
results = []


def record(cid, case, expected, got, ok, detail=""):
    results.append({"control": cid, "case": case, "expected": expected, "got": got, "as_expected": bool(ok),
                    "detail": str(detail)})
    print(f"{'AS EXPECTED' if ok else 'UNEXPECTED '} {cid:4} {case}: {got}{(' — ' + str(detail)) if detail else ''}")


def exe_for(cid):
    sha = K.sha256_bytes(f"SYNTHETIC executable of {cid} (t2_controls.py; never built, never run)".encode())
    path = f"/synthetic/nexus-os-p3-c11-mt2x-{cid}/target/release/deps/nexus_governed_control-0123456789abcdef"
    return {"path": path, "sha256": sha, "size": 4242}


def synth(d, ctl, exe, prop=None, msg_prop=None, line=None, states=None, key_fields=None, header=None, test=None,
          source_verdict="EXACTLY_C_PLUS_PATCH", raw=None, process=None, sup=None):
    """A synthetic type-(a) receipt directory d (descriptor.json, raw-output.txt) for control ctl."""
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
    # Every synthetic output carries this first line, so that no synthetic receipt is byte-equal to a real one.
    raw = "SYNTHETIC (t2_controls.py): not the output of any run\n" + raw
    open(os.path.join(d, "raw-output.txt"), "w").write(raw)
    ident = {"path": exe["path"], "sha256": exe["sha256"], "size": exe["size"], "mtime_ns": 0, "inode": 0, "symlink": False}
    listing = {"exit": 0, "stdout": f"{t}: test\n\n1 test, 0 benchmarks\n", "stderr": "", "listed": [f"{t}: test"]}
    desc.update({"id": "04-run", "test": t, "argv": [exe["path"], t, *ctl["argv_tail"][1:]],
                 "configuration": ctl["configuration"], "control": ctl["id"], "mutant_blob": ctl["mutant_blob"],
                 "patch_sha256": ctl["patch"]["sha256"], "worktree": ctl["worktree"], "limit_seconds": ctl["limit_seconds"],
                 "cwd": os.path.join(ctl["worktree"], K.CRATE), "raw_output_sha256": K.sha256_bytes(raw.encode()),
                 "synthetic": "t2_controls.py: a SYNTHETIC receipt transformed from CONTINUE-02 receipts/38-C9/04-run; "
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
    if process:
        desc["process"] = process
    if sup:
        desc["supervisor"] = dict(desc["supervisor"], **sup)
    json.dump(desc, open(os.path.join(d, "descriptor.json"), "w"), indent=1)


def first_two(d):
    ls = open(os.path.join(d, "CLASSIFICATION.txt")).read().splitlines()
    return ls[0], ls[1].replace("outer class (AD-1-R4 K.2): ", "")


def old_classify(d, ctl, exe):
    """Tranche 1's classify.classify_a (original-tools/), in a separate process, on d."""
    spec = os.path.join(d, ".spec.json")
    json.dump({"ctl": ctl, "exe": exe}, open(spec, "w"))
    code = ("import json, sys; sys.path.insert(0, %r); import classify; s = json.load(open(%r)); "
            "classify.classify_a(%r, s['ctl'], s['exe'])" % (ORIG, spec, d))
    r = subprocess.run([sys.executable, "-B", "-c", code], capture_output=True, text=True,
                       env=dict(os.environ, PYTHONDONTWRITEBYTECODE="1"))
    os.remove(spec)
    return r.returncode, r.stderr[-300:]


def section_a(out, scratch, exp):
    print("A. classify_a on synthetic type-(a) receipts (historical base: CONTINUE-02 C9 run)")
    C = exp["controls"]
    cases = []  # (cid, case, kwargs, ctl override, expected first line, expected outer prefix)
    for cid in K.ORDER:
        cases.append((cid, "P-first-permitted-property", {}, None, classify.CANDIDATE,
                      "FAIL — admissible type-(a) original-explorer mutant FAIL of the named property"))
    cases += [
        ("C3a", "P-second-permitted-property-P15", {"prop": "P15"}, None, classify.CANDIDATE,
         "FAIL — admissible type-(a) original-explorer mutant FAIL of the named property P13 or P15: P15"),
        ("A6", "P-second-permitted-property-P15", {"prop": "P15"}, None, classify.CANDIDATE,
         "FAIL — admissible type-(a) original-explorer mutant FAIL of the named property P12 or P15: P15"),
        ("C3a", "N-property-outside-the-pair-P12", {"prop": "P12"}, None, classify.NOT_CANDIDATE,
         "FAIL of P12, not a permitted property (P13 or P15): kills nothing"),
        ("A6", "N-property-outside-the-pair-P13", {"prop": "P13"}, None, classify.NOT_CANDIDATE,
         "FAIL of P13, not a permitted property (P12 or P15): kills nothing"),
        ("C3a", "N-message-names-the-other-property", {"prop": "P13", "msg_prop": "P15"}, None, classify.NOT_CANDIDATE,
         "NOT ADMISSIBLE ("),
        ("A6", "N-key-fields-of-C-without-recovery_required", {"key_fields": "C"}, None, classify.NOT_CANDIDATE,
         "NOT ADMISSIBLE (the printed state key fields are the mutant's KEY_FIELDS"),
        ("A6", "N-verdict-assertion-at-C's-line-4133", {"line": 4133}, None, classify.NOT_CANDIDATE,
         "NOT ADMISSIBLE (the panic is the explorer's own verdict assertion"),
        ("A3", "N-verdict-assertion-at-C's-line-4133", {"line": 4133}, None, classify.NOT_CANDIDATE,
         "NOT ADMISSIBLE (the panic is the explorer's own verdict assertion"),
        ("C23", "N-wrong-property-E2", {"prop": "E2"}, None, classify.NOT_CANDIDATE,
         "FAIL of E2, not the named property E1: kills nothing (no attribution exception)"),
        ("C10", "P-at-the-60M-ceiling", {"states": 60000000}, None, classify.CANDIDATE,
         "FAIL — admissible type-(a) original-explorer mutant FAIL of the named property E4"),
        ("C10", "N-beyond-the-60M-ceiling", {"states": 60000001}, None, classify.NOT_CANDIDATE,
         "INCOMPLETE — BUDGET EXCEEDED (a FAIL printed beyond the authorised cardinality): kills nothing"),
        ("C15", "N-FAIL-beyond-the-30M-ceiling", {"states": 30000001}, None, classify.NOT_CANDIDATE,
         "INCOMPLETE — BUDGET EXCEEDED (a FAIL printed beyond the authorised cardinality): kills nothing"),
        ("C15", "P-FAIL-at-the-30M-ceiling", {"states": 30000000}, None, classify.CANDIDATE,
         "FAIL — admissible type-(a) original-explorer mutant FAIL of the named property E8"),
        ("C15", "N-INCOMPLETE-at-the-cap", {"raw": "FR"}, None, classify.NOT_CANDIDATE,
         "INCOMPLETE (the state cap of 30000000 was reached. No property is established by this run.): kills nothing"),
        ("C8", "N-composed-quotient-header", {"header": "EXPLORATION F-B (gating-caller symmetry quotient composed with "
                                                        "the admission observer quotient: experimental, not the "
                                                        "contract's explorer)"}, None, classify.NOT_CANDIDATE,
         "NOT ADMISSIBLE (header 'EXPLORATION F-B' exactly"),
        ("C23", "N-PASS-on-the-mutant", {"raw": "F2PASS"}, None, classify.NOT_CANDIDATE,
         "PASS on the mutant (the control survives F-2): kills nothing"),
        ("C26", "N-killed-from-outside", {"process": {"exit_status": None, "signal": 9, "supervisor_rc": -9,
                                                      "wrapper_recorded_status": "137"},
                                          "sup": {"timed_out": True, "wall_seconds": 2700.0}, "raw": "TRUNCATED"}, None,
         classify.NOT_CANDIDATE, "INCOMPLETE (killed from outside or abnormal termination): kills nothing"),
        ("C16", "N-source-not-the-mutant", {"source_verdict": "EXACTLY_C"}, None, classify.NOT_CANDIDATE,
         "NOT ADMISSIBLE (source EXACTLY_C_PLUS_PATCH before and after"),
        ("C5", "N-another-executable", {"exe": "other"}, None, classify.NOT_CANDIDATE,
         "NOT ADMISSIBLE (executable is the pinned one"),
        ("C23", "N-another-test-full_f1", {"test": K.TEST_PREFIX + "full_f1"}, None, classify.NOT_CANDIDATE,
         "NOT ADMISSIBLE ("),
    ]
    fr_raw = open(K.inputs("linked", "S2-69-full-F-R", "raw-output.txt"), encoding="utf-8").read()
    f2_raw = open(K.inputs("linked", "S1-41-full-F-2", "raw-output.txt"), encoding="utf-8").read()
    base_raw = K.blob(K.T1, BASE + "/raw-output.txt").decode()
    for n, (cid, case, kw, _, want_first, want_outer) in enumerate(cases, 1):
        ctl = C[cid]
        exe = exe_for(cid)
        kw = dict(kw)
        if kw.get("key_fields") == "C":
            kw["key_fields"] = [l[len("  state key fields: "):] for l in base_raw.splitlines()
                                if l.startswith("  state key fields: ")][0]
        elif cid == "A6" and "key_fields" not in kw:
            kw["key_fields"] = ctl["key_fields"]
        if kw.get("raw") == "FR":
            kw["raw"] = fr_raw
        elif kw.get("raw") == "F2PASS":
            kw["raw"] = f2_raw
            kw["process"] = {"exit_status": 0, "signal": None, "supervisor_rc": 0, "wrapper_recorded_status": "0"}
        elif kw.get("raw") == "TRUNCATED":
            kw["raw"] = "\n".join(base_raw.replace(BASE_TEST, ctl["test"]).splitlines()[:6]) + "\n"
        run_exe = exe
        if kw.pop("exe", None) == "other":
            run_exe = dict(exe, sha256=K.sha256_bytes(b"another executable"))
        d = os.path.join(out, "A", f"{n:02d}-{cid}-{case}")
        synth(d, ctl, exe, **kw)
        classify.classify_a(d, ctl, run_exe)
        first, outer = first_two(d)
        ok = first == want_first and outer.startswith(want_outer)
        record(cid, "A " + case, f"{want_first} / {want_outer}", f"{first} / {outer}", ok)
        if "permitted_properties" not in ctl:
            od = os.path.join(scratch, "A-old", os.path.basename(d))
            shutil.copytree(d, od, ignore=shutil.ignore_patterns("CHECKS.txt", "CLASSIFICATION.txt",
                                                                 "PARSED-RECEIPT.json"))
            rc, err = old_classify(od, ctl, run_exe)
            same = [f for f in ("CHECKS.txt", "CLASSIFICATION.txt", "PARSED-RECEIPT.json")
                    if os.path.exists(os.path.join(od, f)) and K.sha256_file(os.path.join(od, f)) ==
                    K.sha256_file(os.path.join(d, f))]
            record(cid, "A " + case + " — Tranche-1 classify.py on an identical copy: byte-identical outputs",
                   "CHECKS.txt CLASSIFICATION.txt PARSED-RECEIPT.json identical", f"identical {same}",
                   len(same) == 3, f"old classifier rc {rc} {err.strip()[:120]}")


def section_b(out, scratch, exp):
    print("B. control.link on the authenticated linked inputs (and on tampered copies)")
    C = exp["controls"]
    for cid in K.ORDER:
        d = os.path.join(out, "B", f"{cid}-real-inputs")
        ok, ref = control.link(C[cid], exp, d, None)
        first = supervise.first_line(os.path.join(d, "LINK.txt"))
        record(cid, "B link on the real authenticated inputs", "LINK: VERIFIED", first, first == "LINK: VERIFIED", ref[:90])
    root = os.path.join(scratch, "B-ev")
    saved = (K.EV, K.EXE_SHA256, K.FR_RECORDING)

    def fresh():
        if os.path.exists(root):
            shutil.rmtree(root)
        shutil.copytree(os.path.join(saved[0], "inputs"), os.path.join(root, "inputs"))
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
        ("C23", "N-raw-output-byte-changed", lambda: tamper("inputs/linked/S1-41-full-F-2/raw-output.txt",
                                                          b"outcome: PASS", b"outcome: PASs"), None,
         "its classification blob and raw output identity are the catalog's"),
        ("C16", "N-another-test-in-the-expectation", None, lambda c: c["argv_tail"].__setitem__(0, K.TEST_PREFIX + "full_f4"),
         "exact configuration: the same test and flags as this control's run (F-D)"),
        ("C8", "N-other-bounds-in-the-expectation", None, lambda c: c.__setitem__("bounds", c["bounds"].replace(
            "faults 2", "faults 3")), "F-B's restored side (AD-1-R4 F)"),
        ("C8", "N-composed-quotient-receipt-linked-as-original", None, lambda c: c["linked"].__setitem__("quotient", False),
         "accepted exact-final unchanged-core original-explorer PASS"),
        ("C15", "N-proof-changed", lambda: tamper("inputs/FR-STUTTERING-REFINEMENT-PROOF.md", b"DRAFT", b"DRAFt"), None,
         "the accepted F-R proof: sha256 and blob as the catalog and the adoption record give them"),
        ("C15", "N-adoption-record-changed", lambda: tamper("inputs/AD1-R5-OWNER-ADOPTION.md", b"adopted AD-1-R5 in full",
                                                          b"adopted AD-1-R5 in part"), None,
         "the Owner adoption of AD-1-R5"),
        ("C15", "N-F-R-receipt-recorded-as-PASS", lambda: tamper("inputs/linked/S2-69-full-F-R/CLASSIFICATION.txt",
                                                               b"CLASSIFICATION: NOT PASS", b"CLASSIFICATION: PASS"), None,
         "F-R's own receipt stays INCOMPLETE"),
        ("C26", "N-another-baseline-executable-hash", lambda: setattr(K, "EXE_SHA256", "0" * 64), None,
         "artifact identity at this control's restoration"),
        ("C15", "N-prohibited-F-R-wording", lambda: setattr(K, "FR_RECORDING", "F-R PASS"), None,
         "prohibited F-R wording present"),
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
            K.EV, K.EXE_SHA256, K.FR_RECORDING = saved
    finally:
        K.EV, K.EXE_SHA256, K.FR_RECORDING = saved


def section_c(out, scratch, exp):
    print("C. expectations.py on tampered inputs, and on the real inputs")
    import expectations as E
    saved = K.EV
    root = os.path.join(scratch, "C-ev")
    for case, rel, old, new, want in (
            ("N-catalog-copy-changed", "inputs/catalog.json", b'"P13 or P15"', b'"P13 or P16"',
             "authenticated catalog copy sha256"),
            ("N-patch-copy-changed", "inputs/patches/C26.patch", b"self.core.latch = None;", b"self.core.latch = None; ",
             "C26 authenticated patch copy sha256")):
        if os.path.exists(root):
            shutil.rmtree(root)
        shutil.copytree(os.path.join(saved, "inputs"), os.path.join(root, "inputs"))
        p = os.path.join(root, rel)
        os.chmod(p, 0o644)
        b = open(p, "rb").read()
        assert old in b
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
    rc = E.main(os.path.join(out, "C-real-inputs-EXPECTATIONS.json"))
    same = K.sha256_file(os.path.join(out, "C-real-inputs-EXPECTATIONS.json")) == K.sha256_file(
        os.path.join(K.EV, "receipts/02-EXPECTATIONS.json"))
    record("—", "C P-real-inputs", "DERIVED, equal to receipts/02-EXPECTATIONS.json", f"rc {rc}; equal {same}",
           rc == 0 and same)
    os.remove(os.path.join(out, "C-real-inputs-EXPECTATIONS.json"))


def section_d(out):
    print("D. apply / verify / restore with synthetic comment-only patches (development worktree at C)")
    r, ok = worktrees.create(DEV_WT, DEV_ROLE)
    worktrees.write(os.path.join(out, "D/01-worktree"), "CREATE", f"CREATE the development worktree {DEV_WT} at C", r, ok)
    record("—", "D create the development worktree", "created and locked", "OK" if ok else "FAILED", ok)
    a, ok = worktrees.authenticate(DEV_WT, DEV_ROLE)
    worktrees.write(os.path.join(out, "D/01-worktree"), "AUTHENTICATE", f"AUTHENTICATE {DEV_WT}", a, ok)
    record("—", "D authenticate the development worktree", "exactly C, locked with its role", "OK" if ok else "FAILED", ok)
    if not ok:
        return

    def g(*args):
        return subprocess.run(["git", "-C", DEV_WT, *args], capture_output=True, check=True).stdout
    for tag, rel in (("protocol", K.PROTOCOL), ("model", K.MODEL)):
        base = K.blob(K.C, rel)
        path = os.path.join(DEV_WT, rel)
        mutant = base + b"// SYNTHETIC t2_controls.py comment line: never built, never a control\n"
        open(path, "wb").write(mutant)
        patch = g("diff", "--no-color", "--no-ext-diff", "--", rel)
        open(path, "wb").write(base)
        pdir = os.path.join(out, "D", tag)
        os.makedirs(pdir)
        open(os.path.join(pdir, "synthetic.patch"), "wb").write(patch)
        ctl = {"id": f"SYN-{tag}", "description": f"SYNTHETIC comment-only change of {rel} (t2_controls.py)",
               "patch": {"input": os.path.relpath(os.path.join(pdir, "synthetic.patch"), K.EV),
                         "sha256": K.sha256_bytes(patch), "catalog_file": "synthetic (not a catalog patch)"},
               "source_file": rel, "baseline_blob": K.BLOBS[rel], "baseline_sha256": K.sha256_bytes(base),
               "mutant_blob": K.git_blob_id(mutant), "mutant_sha256": K.sha256_bytes(mutant)}
        ok, applied = control.apply_patch(DEV_WT, ctl, os.path.join(pdir, "02-apply"))
        record(ctl["id"], f"D apply ({rel})", control.APPLY_OK, supervise.first_line(os.path.join(pdir,
                                                                                               "02-apply/APPLY.txt")), ok)
        rec = []
        v = supervise.mutant_check(rec, DEV_WT, ctl, "rehearsal")
        record(ctl["id"], "D verify_mutant: exactly C plus the synthetic patch", "EXACTLY_C_PLUS_PATCH", v["verdict"],
               v["verdict"] == "EXACTLY_C_PLUS_PATCH")
        v = supervise.mutant_check([], DEV_WT, dict(ctl, mutant_blob="0" * 40), "rehearsal")
        record(ctl["id"], "D verify_mutant: the wrong expected blob", "NOT_THE_MUTANT", v["verdict"],
               v["verdict"] == "NOT_THE_MUTANT")
        extra = os.path.join(DEV_WT, K.CRATE_SRC, "ledger/protocol/synthetic_extra.rs")
        open(extra, "w").write("// SYNTHETIC untracked file\n")
        v = supervise.mutant_check([], DEV_WT, ctl, "rehearsal")
        os.remove(extra)
        record(ctl["id"], "D verify_mutant: an untracked file beside the mutant", "NOT_THE_MUTANT", v["verdict"],
               v["verdict"] == "NOT_THE_MUTANT", v["status_entries"][:2])
        other = K.UNIT
        ob = K.blob(K.C, other)
        open(os.path.join(DEV_WT, other), "wb").write(ob + b"// SYNTHETIC second change\n")
        v = supervise.mutant_check([], DEV_WT, ctl, "rehearsal")
        open(os.path.join(DEV_WT, other), "wb").write(ob)
        record(ctl["id"], "D verify_mutant: a second changed file", "NOT_THE_MUTANT", v["verdict"],
               v["verdict"] == "NOT_THE_MUTANT", v["status_entries"][:2])
        v = supervise.mutant_check([], DEV_WT, ctl, "rehearsal")
        record(ctl["id"], "D every perturbation undone: exactly C plus the synthetic patch again", "EXACTLY_C_PLUS_PATCH",
               v["verdict"], v["verdict"] == "EXACTLY_C_PLUS_PATCH")
        rok = control.restore(DEV_WT, ctl, os.path.join(pdir, "05-restore"), applied)
        record(ctl["id"], f"D restore ({rel}): git apply -R; verify_tree.py; path-aware policy", control.RESTORE_OK,
               supervise.first_line(os.path.join(pdir, "05-restore/RESTORE.txt")), rok)
    v = control.restored_check(DEV_WT, "end of the rehearsal")
    record("—", "D the development worktree is exactly C at the end", "EXACTLY_C", v["verdict"], v["verdict"] == "EXACTLY_C")


def main(out, scratch):
    for p in (out, scratch):
        if os.path.exists(p):
            raise SystemExit(f"REFUSED: {p} exists")
    os.makedirs(out)
    os.makedirs(scratch)
    exp = supervise.expectations()
    print(f"T2 CONTROLS — {K.MISSION} (synthetic and historical evidence only; no Cargo, rustc, test or Nexus executable)")
    section_a(out, scratch, exp)
    section_b(out, scratch, exp)
    section_c(out, scratch, exp)
    section_d(out)
    bad = [r for r in results if not r["as_expected"]]
    lines = [f"{'AS EXPECTED' if r['as_expected'] else 'UNEXPECTED '} {r['control']:4} {r['case']}: expected "
             f"{r['expected']}; got {r['got']}{(' — ' + r['detail']) if r['detail'] else ''}" for r in results]
    open(os.path.join(out, "CONTROLS.txt"), "w").write("\n".join(lines) + "\n")
    json.dump(results, open(os.path.join(out, "CONTROLS.json"), "w"), indent=1)
    verdict = "T2 CONTROLS: " + ("ALL AS EXPECTED" if not bad else "FAILED")
    open(os.path.join(out, "SUMMARY.txt"), "w").write(
        f"{len(results)} controls; {len(results) - len(bad)} as expected; {len(bad)} unexpected\n"
        + "".join(f"UNEXPECTED: {r['control']} {r['case']}: {r['got']}\n" for r in bad) + verdict + "\n")
    print(verdict)
    return 0 if not bad else 1


if __name__ == "__main__":
    sys.exit(main(os.path.abspath(sys.argv[1]), os.path.abspath(sys.argv[2])))

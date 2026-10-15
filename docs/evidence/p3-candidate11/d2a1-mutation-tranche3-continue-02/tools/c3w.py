#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-MUTATION-TRANCHE3-CONTINUE-02, mission sections 3, 4, 5 and 15 as amended by the Architect's
AMENDMENT-01 (documents/ARCHITECT-AMENDMENT-01-C3B-C3C-BUILD-RULE.md): the dedicated C3b and C3c build-admission
checker. Read-only but for its own records. It never builds anything; it reads the build receipt that supervise.build
wrote and the unchanged general classifier (classify.classify_build_success) classified.

  admit(build_dir, ctl, outdir, worktree=None)
      For C3b and C3c only (every other control is refused: the rule does not transfer to C24, C6, C7, A1, A2, C20, A5
      or any other control). A build is admissible on exactly one of two paths:

      A  CLEAN BUILD: the unchanged general classifier wrote `BUILD: PINNED` with every one of its checks holding (exit
         0, zero errors, zero warnings, every ordinary source/build/executable/resource check); no `error` or `warning`
         line anywhere in the stderr; its PIN.json is the one library-test executable the build recorded, and the file
         now has that sha256 and size. The pin is the general classifier's 03-build/PIN.json.
      B  EXACT NARROW WARNING BUILD: the unchanged general classifier wrote `BUILD: NOT OK` and every one of its checks
         holds but `no error or warning diagnostic`, whose recorded diagnostics are exactly the predicted warning's header
         and cargo's one-warning summary; the stderr holds exactly one diagnostic group, `warning: variant `Future` is
         never constructed` (C3b) or `warning: variant `Set` is never constructed` (C3c), at protocol.rs:635:5 (resp.
         636:5), quoting the mutant's own lines, the caret run exactly under the variant and the dash run exactly under
         `AckRefusal` (protocol.rs:630:17) with the label `variant in this enum`, and rustc's two dead-code notes; cargo's
         summary `generated 1 warning` once; every other stderr line cargo progress, the Finished line or the Executable
         line. The pin is written here (outdir/PIN.json) from the build's own record, after the file is hashed again.

      Both paths also require: this control's patch and mutant identities in the descriptor; the source
      EXACTLY_C_PLUS_PATCH before and after with this control's mutant blob and sha256; the build in this control's own
      mission worktree with a fresh worktree-local target; stdout empty. Anything else (a different warning; the
      predicted warning plus any other; any error; an identity mismatch; a failed ordinary check) is REJECTED: the
      mission stops. The warning is permitted, not required.

      worktree (default: the expectation's) is for t3_controls.py's synthetic receipts only, which are built over an
      accepted control's real target (their records say SYNTHETIC); control.main never passes it.
      Writes outdir/ADMISSION.txt (first line ADMIT_CLEAN, ADMIT_NARROW or ADMIT_FAIL), ADMISSION.json and, on path B,
      PIN.json. Returns (ok, path to the pin record or None, first line).
"""
import json
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import c24  # noqa: E402  (the accepted C5 checker's group parser, carried into c24.py)
import classify  # noqa: E402
import common as K  # noqa: E402
import expectations  # noqa: E402

CONTROLS = ["C3b", "C3c"]
ADMIT_CLEAN = "BUILD ADMISSION: ADMISSIBLE — CLEAN BUILD (BUILD: PINNED UNDER THE UNCHANGED GENERAL CLASSIFIER)"
ADMIT_NARROW = "BUILD ADMISSION: ADMISSIBLE — EXACT NARROW WARNING BUILD (ARCHITECT RULING, AMENDMENT-01)"
ADMIT_FAIL = "BUILD ADMISSION: REJECTED — STOP"
SUMMARY = "warning: `nexus-governed-control` (lib test) generated 1 warning"
WARNING_CHECK = "no error or warning diagnostic"
DERIVES = "(?:Clone|Copy|Debug|PartialEq|Eq)"
DERIVED_NOTE = (rf"`AckRefusal` has (?:a )?derived impls? for the traits? `{DERIVES}`(?:(?:, | and |, and )`{DERIVES}`)*, "
                r"but (?:these are|this is) intentionally ignored during dead code analysis")
DEAD_CODE_NOTE = "`#[warn(dead_code)]` (part of `#[warn(unused)]`) on by default"


def spec(cid):
    """The one permitted warning group of cid (AMENDMENT-01), in c24.stderr_admission's form."""
    v = K.C3W[cid]
    name, line = v["variant"], v["line"]
    enum, eline, ecol = K.C3W_ENUM
    return {"n": 1, "header": f"warning: variant `{name}` is never constructed", "file": K.PROTOCOL, "line": line,
            "col": v["col"], "shown": [eline, line], "carets": [(line, v["col"], len(name), name)],
            "dashes": [(eline, ecol, len(enum), enum)], "labels": ["variant in this enum"],
            "notes": ["`AckRefusal` has derived impls for the traits ..., but these are intentionally ignored during dead "
                      "code analysis", DEAD_CODE_NOTE], "notes_re": [DERIVED_NOTE, re.escape(DEAD_CODE_NOTE)]}


def mutant_sources(ctl):
    patch = open(K.inputs("patches", ctl["id"] + ".patch"), "rb").read()
    mut, err = expectations.mutant_bytes(patch, K.PROTOCOL, K.blob(K.C, K.PROTOCOL))
    if mut is None or K.git_blob_id(mut) != ctl["mutant_blob"]:
        return None, f"the mutant could not be reproduced from the frozen patch: {err}"
    return {K.PROTOCOL: mut.decode("utf-8").split("\n"), K.MODEL: K.source(K.MODEL).split("\n")}, ""


def _write(outdir, cid, first, title, rec):
    os.makedirs(outdir, exist_ok=True)
    json.dump(rec, open(os.path.join(outdir, "ADMISSION.json"), "w"), indent=1, default=str)
    text = "\n".join(c24.text_lines(rec.get("checks", []), [first, title])) + "\n"
    open(os.path.join(outdir, "ADMISSION.txt"), "w").write(text)
    print(text, end="")


def admit(build_dir, ctl, outdir, worktree=None):
    cid = ctl.get("id")
    ck = c24.Checks()
    rec = {"control": cid, "build_dir": os.path.relpath(build_dir, K.EV) if build_dir.startswith(K.EV) else build_dir,
           "rule": "mission sections 3-5 as amended by AMENDMENT-01", "path": None}
    title = (f"control {cid}: the build ({rec['build_dir']}) under the C3b/C3c build rule (a clean build, or exactly the "
             "one predicted warning group; the warning permitted, not required); the general classifier unchanged")
    if not ck("the control is C3b or C3c (the rule does not transfer to C24, C6, C7, A1, A2, C20, A5 or any other "
              "control)", cid in CONTROLS and ctl.get("patch", {}).get("sha256") == K.C3W.get(cid, {}).get("patch_sha256")
              and ctl.get("mutant_blob") == K.C3W.get(cid, {}).get("mutant_blob")
              and ctl.get("mutant_sha256") == K.C3W.get(cid, {}).get("mutant_sha256"), cid):
        rec["checks"] = ck.items
        _write(outdir, cid, f"{cid} {ADMIT_FAIL}", title, rec)
        return False, None, f"{cid} {ADMIT_FAIL}"
    wt = worktree or ctl["worktree"]
    have = {f: os.path.isfile(os.path.join(build_dir, f)) for f in ("descriptor.json", "build.stderr", "build.stdout",
                                                                    "RECEIPT.txt", "CLASSIFICATION.txt", "CHECKS.txt",
                                                                    "PARSED-RECEIPT.json")}
    if not ck("the build receipt is complete and classified by the unchanged general classifier (descriptor, stdout, "
              "stderr, RECEIPT.txt, CLASSIFICATION.txt, CHECKS.txt, PARSED-RECEIPT.json)", all(have.values()), have):
        rec["checks"] = ck.items
        _write(outdir, cid, f"{cid} {ADMIT_FAIL}", title, rec)
        return False, None, f"{cid} {ADMIT_FAIL}"
    desc = json.load(open(os.path.join(build_dir, "descriptor.json")))
    ck("the build started and finished (no pre-flight refusal)", "not_started" not in desc, desc.get("not_started", ""))
    ck("the build's control, role, patch and mutant identities are this control's (descriptor)",
       (desc.get("control"), desc.get("role"), desc.get("patch_sha256"), desc.get("mutant_blob"),
        desc.get("mutant_sha256")) == (cid, "mutant build", ctl["patch"]["sha256"], ctl["mutant_blob"],
                                       ctl["mutant_sha256"]),
       (desc.get("control"), desc.get("patch_sha256"), desc.get("mutant_blob")))

    def mutant_ok(s):
        m = (s or {}).get("mutant_file") or {}
        return ((s or {}).get("verdict") == "EXACTLY_C_PLUS_PATCH" and s.get("head") == K.C and s.get("tree") == K.C_TREE
                and m.get("path") == K.PROTOCOL == ctl["source_file"] and m.get("git_blob") == ctl["mutant_blob"]
                and m.get("sha256") == ctl["mutant_sha256"] and s.get("other_content_mismatches") == []
                and s.get("missing") == [] and s.get("mode_mismatches") == [])
    ck("the source before and after the build: EXACTLY_C_PLUS_PATCH, C's HEAD and tree, this control's mutant blob and "
       "sha256, no other mismatch (the exact frozen patch's mutant)",
       mutant_ok(desc.get("source_before")) and mutant_ok(desc.get("source_after")),
       f"{(desc.get('source_before') or {}).get('verdict')} / {(desc.get('source_after') or {}).get('verdict')}")
    target = os.path.join(wt, "target")
    receipt = open(os.path.join(build_dir, "RECEIPT.txt")).read()
    ck("the build ran in this control's own worktree with a fresh worktree-local target (the exact command and "
       "environment are the general classifier's checks)", desc.get("worktree") == wt == desc.get("cwd")
       and desc.get("target") == target and f"target directory {target} existed before: False" in receipt,
       f"{desc.get('worktree')}; {desc.get('target')}")
    ck("stdout empty (cargo writes its messages to stderr); the stderr is the one the launcher recorded",
       os.path.getsize(os.path.join(build_dir, "build.stdout")) == 0
       and K.sha256_file(os.path.join(build_dir, "build.stderr")) == desc.get("stderr_sha256"), desc.get("stderr_sha256"))
    cls = open(os.path.join(build_dir, "CLASSIFICATION.txt")).read().splitlines()
    chks = open(os.path.join(build_dir, "CHECKS.txt")).read().splitlines()
    failed = [l[5:].split(":")[0] for l in chks if l.startswith("FAIL ")]
    n_ok = sum(1 for l in chks if l.startswith("OK  "))
    stderr = open(os.path.join(build_dir, "build.stderr"), encoding="utf-8").read()
    diag = [l for l in stderr.splitlines() if c24.HEAD_RE.match(l)]
    tests = [a for a in desc.get("deps_artifacts", []) if a.get("test_executable_name")]
    pin_path = None
    if cls[:1] == ["BUILD: PINNED"]:
        rec["path"] = "A"
        ck("A CLEAN BUILD: the unchanged general classifier's BUILD: PINNED, every one of its checks holding (exit 0, "
           "zero errors, zero warnings, every ordinary source/build/executable/resource check)",
           not failed and n_ok == len(chks) and f"checks: {n_ok} of {n_ok} hold" in cls, f"{n_ok} of {len(chks)}")
        ck("A no `error` or `warning` line anywhere in the stderr (read again here)", not diag, diag[:3])
        p = os.path.join(build_dir, "PIN.json")
        pin = json.load(open(p)) if os.path.isfile(p) else {}
        ck("A the general classifier's PIN.json is the one library-test executable the build recorded (path, sha256, "
           "size)", len(tests) == 1 and (pin.get("path"), pin.get("sha256"), pin.get("size")) == (
               tests[0]["path"], tests[0]["sha256"], tests[0]["size"]) and pin.get("mutant_blob") == ctl["mutant_blob"],
           (pin.get("path"), pin.get("sha256")))
        pin_path = p
    elif cls[:1] == ["BUILD: NOT OK"] and failed == [WARNING_CHECK]:
        rec["path"] = "B"
        ck("B every ordinary check of the unchanged general classifier holds; the one failed check is its warning check "
           f"(`{WARNING_CHECK}`)", failed == [WARNING_CHECK] and n_ok == len(chks) - 1, f"{n_ok} of {len(chks)}")
        w = spec(cid)
        want_diag = [w["header"], SUMMARY]
        ck("B the general classifier's own record of the diagnostics is exactly the predicted warning and cargo's "
           "one-warning summary", [l for l in cls if l.startswith(f"FAILED CHECK: {WARNING_CHECK}: ")] == [
               f"FAILED CHECK: {WARNING_CHECK}: {want_diag}"], [l[:200] for l in cls if l.startswith("FAILED CHECK")])
        src, err = mutant_sources(ctl)
        ck("B the source the executable was built from, reproduced for the comparison: the mutant protocol.rs from the "
           "frozen patch (never written to any object store) and C's model_tests.rs", src is not None, err)
        exe_path = tests[0]["path"] if len(tests) == 1 else None
        rec["stderr"] = c24.stderr_admission(ck, stderr, src or {}, [w], SUMMARY, exe_path)
        ck(f"B the executable cargo named is in this control's own target: {target}/release/deps",
           exe_path is not None and os.path.dirname(exe_path) == os.path.join(target, "release/deps"), exe_path)
        ok_now = False
        if exe_path and os.path.isfile(exe_path) and not os.path.islink(exe_path):
            ok_now = (K.sha256_file(exe_path), os.path.getsize(exe_path)) == (tests[0]["sha256"], tests[0]["size"])
        ck("B the executable file now has the sha256 and size the build recorded (hashed again here); a regular file",
           ok_now, exe_path)
        if ck.ok():
            _, _, _, flags = classify.build_facts(build_dir, desc)
            pin = {"path": exe_path, "sha256": tests[0]["sha256"], "size": tests[0]["size"],
                   "source_verdict": "EXACTLY_C_PLUS_PATCH", "mutant_blob": ctl["mutant_blob"],
                   "build_receipt": os.path.relpath(build_dir, K.EV), "effective_rustc_flags": flags,
                   "rustc": desc["toolchain"]["rustc"], "fingerprints": classify.fingerprints(
                       desc["target"], os.path.basename(exe_path)),
                   "admission": "exact narrow warning build (mission sections 3/4 as amended by AMENDMENT-01)",
                   "permitted_warning": w["header"]}
            os.makedirs(outdir, exist_ok=True)
            pin_path = os.path.join(outdir, "PIN.json")
            json.dump(pin, open(pin_path, "w"), indent=1)
    else:
        ck("the build is either clean (A: BUILD: PINNED) or fails the unchanged general classifier only by its warning "
           "check (B)", False, f"{cls[:1]}; failed checks {failed}")
    ok = ck.ok() and pin_path is not None
    first = f"{cid} {ADMIT_CLEAN if ok and rec['path'] == 'A' else ADMIT_NARROW if ok else ADMIT_FAIL}"
    rec["checks"] = ck.items
    rec["pin"] = os.path.relpath(pin_path, K.EV) if pin_path and pin_path.startswith(K.EV) else pin_path
    _write(outdir, cid, first, title, rec)
    return ok, pin_path if ok else None, first

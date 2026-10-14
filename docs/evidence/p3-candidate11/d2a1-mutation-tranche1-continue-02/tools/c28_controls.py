#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-MUTATION-TRANCHE1-CONTINUE-02, section 14: the tool controls of the dedicated C28 admission checker
and pin step, before the freeze. No Cargo, no rustc, no Nexus build or test is run; the real preserved C28 executable is
only hashed, sized and stat-ed (read-only); every synthetic file lives under SCRATCH, outside every repository.

    c28_controls.py OUTDIR SCRATCH      both must not exist; OUTDIR receives one line per control (CONTROLS.txt),
                                        a JSON record (CONTROLS.json) and SUMMARY.txt (last line
                                        `C28 ADMISSION CONTROLS: ALL AS EXPECTED` or `... FAILED`)

Each control states its expected verdict and, for a rejection, the check that must fail (so a rejection is shown to
come from the intended rule, not only from a changed hash). The admission controls start from the real C28 evidence
(git objects of 3695a817) and change one field or one stderr passage in memory. The pin controls use a synthetic
worktree layout (a regular file standing in for the executable) with the ruling's identities replaced by the synthetic
ones, plus the real preserved executable read-only.

  K1  the exact C28 evidence is admitted                               (mission 14: exact C28 warning accepted)
  K2  a different warning is rejected                                   (different warning rejected)
  K3  an additional warning is rejected (summary 2; summary left at 1)  (additional warning rejected)
  K4  an error plus the warning is rejected                             (error plus the warning rejected)
  K5  a wrong warning location (line; file) is rejected                 (wrong warning location rejected)
  K6  a wrong patch, a wrong mutant blob before/after, a non-mutant source after are rejected (wrong patch/source)
  K7  wrong executable hash, wrong size, missing executable, symlinked executable, target outside the worktree,
      symlinked target, executable outside the target, a target entry changed after the build are rejected; the
      synthetic and the real preserved executable are accepted        (wrong hash/size, missing, target outside)
  K8  the general build classifier is byte-identical to RESUME-01's frozen classify.py and, run on a scratch copy of
      the real C28 build receipt, still says `BUILD: NOT OK` with the one failed check `no error or warning diagnostic`,
      while the admission checker admits the same evidence      (accepted without changing the general classifier)
  K9  another control with the same warning remains rejected: admit() refuses C12, C9, C11, C17, C18a, C18b, C27, C22;
      the general classifier rejects the same build evidence presented for C12        (another control rejected)
  K10 further negatives: exit 1, a signal, a timeout, an OOM kill, a swap use, an unauthenticated evidence file, the
      explanation missing, an extra stderr note line, a changed stderr byte, a changed toolchain line are rejected
  K11 the C28 run-classification call (unchanged classify_b; source EXACTLY_C; mutant blob withheld from the run-source
      check only) on EXEC-01's synthetic type-(b) receipt: the restored source is a candidate; an unrestored
      (C-plus-patch) or a NOT_C source is rejected
"""
import copy
import json
import os
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import c28_admit as A  # noqa: E402
import classify  # noqa: E402
import common as K  # noqa: E402

results = []


def record(name, expect_ok, ck, must_fail=None):
    """expect_ok: the expected verdict; must_fail: a label prefix that must be among the failed checks."""
    got = ck.ok()
    failed = ck.failed()
    good = got == expect_ok and (expect_ok or must_fail is None or any(f.startswith(must_fail) for f in failed))
    results.append({"control": name, "expected": "ACCEPT" if expect_ok else f"REJECT ({must_fail})",
                    "got": "ACCEPT" if got else "REJECT", "failed_checks": failed, "as_expected": good})
    print(f"{'AS EXPECTED ' if good else 'NOT EXPECTED'} {name}: expected {'ACCEPT' if expect_ok else 'REJECT'}; got "
          f"{'ACCEPT' if got else 'REJECT'}; failed {[f.split(' ')[0] for f in failed]}")


def plain(name, ok, detail=""):
    results.append({"control": name, "as_expected": bool(ok), "detail": str(detail)})
    print(f"{'AS EXPECTED ' if ok else 'NOT EXPECTED'} {name}: {detail}")


def with_stderr(ev, text):
    e = copy.deepcopy(ev)
    e["stderr"] = text
    e["stderr_bytes"] = text.encode()
    return e


def main(outdir, scratch):
    if os.path.exists(outdir) or os.path.exists(scratch):
        raise SystemExit("REFUSED: output or scratch exists")
    os.makedirs(outdir)
    os.makedirs(scratch)
    ev = A.evidence()
    entry = A.catalog_entry("C28")
    tcs = A.reference_toolchains()
    cprot = A.git("cat-file", "blob", K.BLOBS[K.PROTOCOL])
    mut = A.mutant_bytes(entry)
    adm = lambda e, cid="C28", ruling=A.RULING: A.admit(e, cid, entry, tcs, cprot, mut, ruling)
    s = ev["stderr"]
    W = "warning: unreachable pattern\n"
    SUMM = "warning: `nexus-governed-control` (lib test) generated 1 warning\n"
    assert s.count(W) == 1 and s.count(SUMM) == 1 and s.count(":1834:17\n") == 1

    # K1
    record("K1 the exact C28 evidence (warning unreachable_patterns at protocol.rs:1834:17) is admitted", True, adm(ev))
    # K2
    other = ("warning: unused variable: `durable`\n    --> crates/nexus-governed-control/src/ledger/protocol.rs:1834:17\n"
             "     |\n1834 |                 Read::Got(durable) => {\n     |                           ^^^^^^^ help: if this "
             "is intentional, prefix it with an underscore: `_durable`\n     |\n     = note: `#[warn(unused_variables)]` "
             "(part of `#[warn(unused)]`) on by default\n\n")
    blk_start = s.index(W)
    blk_end = s.index(SUMM)
    record("K2 a different warning (unused_variables) in place of the unreachable pattern is rejected", False,
           adm(with_stderr(ev, s[:blk_start] + other + s[blk_end:])), "A12")
    # K3
    extra = ("warning: unused import: `std::fmt`\n --> crates/nexus-governed-control/src/lib.rs:3:5\n  |\n3 | use "
             "std::fmt;\n  |     ^^^^^^^^\n  |\n  = note: `#[warn(unused_imports)]` (part of `#[warn(unused)]`) on by "
             "default\n\n")
    two = s[:blk_end] + extra + s[blk_end:].replace("generated 1 warning", "generated 2 warnings", 1)
    record("K3a an additional warning (and the summary counting 2) is rejected", False, adm(with_stderr(ev, two)), "A11")
    record("K3b an additional warning with the summary left at 1 is rejected", False,
           adm(with_stderr(ev, s[:blk_end] + extra + s[blk_end:])), "A11")
    record("K3c a second unreachable_patterns warning at another line is rejected", False,
           adm(with_stderr(ev, s[:blk_end] + s[blk_start:blk_end].replace(":1834:17", ":1900:17") + s[blk_end:])),
           "A11")
    # K4
    err = ("error[E0308]: mismatched types\n    --> crates/nexus-governed-control/src/ledger/protocol.rs:1840:9\n     |\n"
           "1840 |         x\n     |         ^ expected `Resolve`, found `()`\n\n")
    record("K4a an error plus the warning is rejected", False, adm(with_stderr(ev, s[:blk_start] + err + s[blk_start:])),
           "A10")
    record("K4b an uncoded error line plus the warning is rejected", False,
           adm(with_stderr(ev, s[:blk_end] + "error: could not compile `nexus-governed-control` (lib test)\n"
                           + s[blk_end:])), "A10")
    # K5
    record("K5a the warning at line 1835 is rejected", False, adm(with_stderr(ev, s.replace(":1834:17", ":1835:17"))),
           "A13")
    record("K5b the warning at column 18 is rejected", False, adm(with_stderr(ev, s.replace(":1834:17", ":1834:18"))),
           "A13")
    record("K5c the warning in another file is rejected", False,
           adm(with_stderr(ev, s.replace("src/ledger/protocol.rs:1834:17", "src/ledger/protocol/unit_tests.rs:1834:17"))),
           "A13")
    # K6
    e = copy.deepcopy(ev)
    e["apply"]["patch_sha256"] = "0" * 64
    record("K6a a wrong applied patch sha256 is rejected", False, adm(e), "A2")
    e = copy.deepcopy(ev)
    e["desc"]["source_before"]["mutant_file"]["git_blob"] = "f" * 40
    record("K6b a wrong mutant blob before the build is rejected", False, adm(e), "A3")
    e = copy.deepcopy(ev)
    e["desc"]["source_after"]["mutant_file"]["sha256"] = "0" * 64
    record("K6c a wrong mutant sha256 after the build is rejected", False, adm(e), "A3")
    e = copy.deepcopy(ev)
    e["desc"]["source_after"]["verdict"] = "NOT_THE_MUTANT"
    record("K6d a source not exactly C plus the patch after the build is rejected", False, adm(e), "A3")
    e = copy.deepcopy(ev)
    e["desc"]["source_before"]["other_content_mismatches"] = ["crates/nexus-governed-control/src/lib.rs"]
    record("K6e a second changed file before the build is rejected", False, adm(e), "A3")
    r2 = dict(A.RULING, mutant_blob="f" * 40)
    record("K6f a ruling mutant identity that the catalog does not hold is rejected", False, adm(ev, ruling=r2), "A2")

    # K7: pin controls on a synthetic layout, then the real preserved executable (read-only).
    def layout(name, exe_bytes=b"SYNTHETIC EXECUTABLE BYTES\n" * 64, target_symlink=False, exe_symlink=False,
               outside=False, touch_later=False, missing=False):
        wt = os.path.join(scratch, name, "wt")
        real_target = os.path.join(scratch, name, "elsewhere-target") if target_symlink else os.path.join(wt, "target")
        deps = os.path.join(real_target, "release/deps")
        os.makedirs(deps)
        os.makedirs(wt, exist_ok=True)
        if target_symlink:
            os.symlink(real_target, os.path.join(wt, "target"))
        exe = os.path.join(wt, "target/release/deps/nexus_governed_control-2faa0b9718d3e6a3")
        if outside:
            exe = os.path.join(scratch, name, "nexus_governed_control-2faa0b9718d3e6a3")
        good = b"SYNTHETIC EXECUTABLE BYTES\n" * 64
        if not missing:
            if exe_symlink:
                other = os.path.join(scratch, name, "real-exe")
                open(other, "wb").write(exe_bytes)
                os.symlink(other, exe)
            else:
                open(exe, "wb").write(exe_bytes)
                os.chmod(exe, 0o755)
        bound = time.time() + 1.0  # the synthetic "build" ends after every file above was written
        if touch_later:
            later = os.path.join(real_target, "release/late-file")
            open(later, "w").write("x")
            os.utime(later, (bound + 100, bound + 100))
        ruling = dict(A.RULING, worktree=wt, executable=os.path.join(wt, "target/release/deps/"
                                                                         "nexus_governed_control-2faa0b9718d3e6a3"),
                      exe_sha256=K.sha256_bytes(good), exe_size=len(good))
        return A.pin_checks(A.exe_facts(exe, wt, bound), ruling)
    record("K7a the synthetic executable at the ruling's identity is pinned", True, layout("pos"))
    record("K7b a wrong executable hash (same size, other bytes) is rejected", False,
           layout("hash", exe_bytes=b"SYNTHETIC EXECUTABLE BYTEZ\n" * 64), "P4")
    record("K7c a wrong executable size is rejected", False, layout("size", exe_bytes=b"SYNTHETIC EXECUTABLE BYTES\n" * 63),
           "P4")
    record("K7d a missing executable is rejected", False, layout("missing", missing=True), "P2")
    record("K7e a symlinked executable is rejected", False, layout("exe-symlink", exe_symlink=True), "P2")
    record("K7f a target outside the worktree (the worktree's target a symlink to elsewhere) is rejected", False,
           layout("target-symlink", target_symlink=True), "P3")
    record("K7g an executable outside the worktree's target is rejected", False, layout("outside", outside=True), "P1")
    record("K7h a target entry changed after the build is rejected", False, layout("late", touch_later=True), "P6")
    bound, _ = A.build_bound(ev)
    real = A.exe_facts(A.RULING["executable"], A.WT, bound)
    record("K7i the real preserved C28 executable (read-only: hashed, sized, stat-ed) is accepted", True,
           A.pin_checks(real, A.RULING))
    record("K7j the real preserved executable against a wrong ruling sha256 is rejected", False,
           A.pin_checks(real, dict(A.RULING, exe_sha256="0" * 64)), "P4")
    record("K7k the real preserved executable against a wrong ruling size is rejected", False,
           A.pin_checks(real, dict(A.RULING, exe_size=A.RULING["exe_size"] + 1)), "P4")
    record("K7l the real preserved executable against a different worktree is rejected", False,
           A.pin_checks(A.exe_facts(A.RULING["executable"], K.control_wt("C27"), bound), A.RULING), "P1")

    # K8: the general classifier unchanged, and still rejecting the C28 build.
    frozen = json.loads(A.git("show", f"{K.RESUME}:{K.RESUME_PACKET}/receipts/05-FREEZE.json"))["files"]["tools/classify.py"]
    here = K.sha256_file(os.path.join(os.path.dirname(os.path.abspath(__file__)), "classify.py"))
    plain("K8a classify.py is byte-identical to RESUME-01's frozen classify.py", here == frozen, f"{here} / {frozen}")
    d = os.path.join(scratch, "general-classifier-on-C28")
    os.makedirs(d)
    for f in ("descriptor.json", "build.stdout", "build.stderr"):
        open(os.path.join(d, f), "wb").write(A.git("show", f"{K.RESUME}:{K.RESUME_PACKET}/{A.BUILD_DIR}/03-build/{f}"))
    rc = classify.classify_build_success(d, "pin", "EXACTLY_C_PLUS_PATCH", A.RULING["mutant_blob"],
                                         expect_target=os.path.join(A.WT, "target"))
    cls = open(os.path.join(d, "CLASSIFICATION.txt")).read().splitlines()
    fails = [l for l in open(os.path.join(d, "CHECKS.txt")).read().splitlines() if l.startswith("FAIL")]
    plain("K8b the unchanged general classifier, on a scratch copy of the real C28 build receipt: BUILD: NOT OK, the one "
          "failed check `no error or warning diagnostic`; no PIN.json", rc == 1 and cls[0] == "BUILD: NOT OK"
          and len(fails) == 1 and fails[0].startswith("FAIL no error or warning diagnostic")
          and not os.path.exists(os.path.join(d, "PIN.json")), f"{cls[0]}; {fails}")
    plain("K8c the general classifier's historical C28 output (RESUME-01) equals this re-derivation (first lines)",
          ev["cls"].splitlines()[:2] == cls[:2], cls[:2])
    record("K8d the admission checker admits the same evidence (narrow C28 ruling)", True, adm(ev))
    # K9
    for cid in ("C12", "C9", "C11", "C17", "C18a", "C18b", "C27", "C22"):
        record(f"K9 admit() refuses {cid} with the same (C28) evidence", False, adm(ev, cid), "the control is C28")
    d2 = os.path.join(scratch, "general-classifier-as-C12")
    os.makedirs(d2)
    desc = json.loads(A.git("show", f"{K.RESUME}:{K.RESUME_PACKET}/{A.BUILD_DIR}/03-build/descriptor.json"))
    desc["control"] = "C12"
    json.dump(desc, open(os.path.join(d2, "descriptor.json"), "w"))
    for f in ("build.stdout", "build.stderr"):
        open(os.path.join(d2, f), "wb").write(A.git("show", f"{K.RESUME}:{K.RESUME_PACKET}/{A.BUILD_DIR}/03-build/{f}"))
    rc = classify.classify_build_success(d2, "pin", "EXACTLY_C_PLUS_PATCH", A.RULING["mutant_blob"],
                                         expect_target=os.path.join(A.WT, "target"))
    plain("K9 the general classifier rejects the same warning presented as another control's build (C12): BUILD: NOT OK",
          rc == 1 and open(os.path.join(d2, "CLASSIFICATION.txt")).readline().strip() == "BUILD: NOT OK", "BUILD: NOT OK")
    # K10
    for label, path, value, must in (("exit 1", ("desc", "process", "exit_status"), 1, "A7"),
                                     ("a signal", ("desc", "process", "signal"), 9, "A7"),
                                     ("a timeout", ("desc", "supervisor", "timed_out"), True, "A8"),
                                     ("an OOM kill", ("desc", "supervisor", "oom_kill"), 1, "A8"),
                                     ("a swap use", ("desc", "supervisor", "swap_peak_bytes"), "4096", "A8"),
                                     ("a wrong rustc", ("desc", "toolchain", "rustc_vV"), "rustc 1.95.0", "A6")):
        e = copy.deepcopy(ev)
        e[path[0]][path[1]][path[2]] = value
        record(f"K10 {label} is rejected", False, adm(e), must)
    e = copy.deepcopy(ev)
    e["auth"][3]["ok"] = False
    record("K10 an evidence file not equal to the RESUME-01 SHA256SUMS is rejected", False, adm(e), "A1")
    record("K10 the explanation missing (`matches all the relevant values` absent) is rejected", False,
           adm(with_stderr(ev, s.replace(" matches all the relevant values", " is here"))), "A14")
    record("K10 an extra stderr note line is rejected", False,
           adm(with_stderr(ev, s.replace(SUMM, SUMM + "note: something else happened\n"))), "A16")
    record("K10 a changed stderr byte (outside the warning) is rejected", False,
           adm(with_stderr(ev, s.replace("Compiling proc-macro2 v1.0.106", "Compiling proc-macro2 v1.0.107"))), "A9")
    # K11: the C28 run-classification call on EXEC-01's synthetic type-(b) receipt.
    exp = json.loads(A.git("show", f"{K.PREV}:{K.PREV_PACKET}/receipts/02-EXPECTATIONS.json"))
    c14 = exp["controls"]["C14"]
    src = f"{K.PREV_PACKET}/receipts/03-tool-controls/run-b-c14-own-check"
    for name, verdict, want in (("restored", {"verdict": "EXACTLY_C"}, True),
                                ("unrestored", {"verdict": "EXACTLY_C_PLUS_PATCH",
                                                "mutant_file": {"git_blob": c14["mutant_blob"]}}, False),
                                ("not-C", {"verdict": "NOT_C"}, False)):
        d3 = os.path.join(scratch, "run-b-" + name)
        os.makedirs(d3)
        for f in ("descriptor.json", "raw-output.txt", "RECEIPT.txt", "scope-record.txt", "time-v.txt"):
            open(os.path.join(d3, f), "wb").write(A.git("show", f"{K.PREV}:{src}/{f}"))
        desc = json.load(open(os.path.join(d3, "descriptor.json")))
        desc["source_before"] = desc["source_after"] = verdict
        json.dump(desc, open(os.path.join(d3, "descriptor.json"), "w"))
        b = desc["executable"]["before"]
        classify.classify_b(d3, dict(c14, mutant_blob=None), {"path": b["path"], "sha256": b["sha256"],
                                                              "size": b["size"]}, source_verdict="EXACTLY_C")
        first = open(os.path.join(d3, "CLASSIFICATION.txt")).readline().strip()
        fails = [l for l in open(os.path.join(d3, "CHECKS.txt")).read().splitlines() if l.startswith("FAIL")]
        plain(f"K11 classify_b (unchanged; source EXACTLY_C; mutant blob withheld) on EXEC-01's synthetic type-(b) "
              f"receipt with a {name} source: {'candidate' if want else 'rejected at the source check'}",
              (first == classify.CANDIDATE) == want and (want or (len(fails) == 1 and "source EXACTLY_C" in fails[0])),
              f"{first}; {fails}")
    ok = all(r["as_expected"] for r in results)
    open(os.path.join(outdir, "CONTROLS.txt"), "w").write(
        "\n".join(f"{'AS EXPECTED ' if r['as_expected'] else 'NOT EXPECTED'} {r['control']}: "
                  f"{r.get('expected', '')} {r.get('got', '')} {r.get('failed_checks', r.get('detail', ''))}"
                  for r in results) + "\n")
    json.dump(results, open(os.path.join(outdir, "CONTROLS.json"), "w"), indent=1)
    summary = [f"{K.MISSION} — C28 admission/pin tool controls (section 14); no Cargo, no rustc, no Nexus test; the real "
               f"preserved executable only hashed, sized and stat-ed; synthetic files under {scratch}",
               f"controls: {len(results)}; as expected: {sum(r['as_expected'] for r in results)}",
               "C28 ADMISSION CONTROLS: " + ("ALL AS EXPECTED" if ok else "FAILED")]
    open(os.path.join(outdir, "SUMMARY.txt"), "w").write("\n".join(summary) + "\n")
    print("\n".join(summary))
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main(os.path.abspath(sys.argv[1]), os.path.abspath(sys.argv[2])))

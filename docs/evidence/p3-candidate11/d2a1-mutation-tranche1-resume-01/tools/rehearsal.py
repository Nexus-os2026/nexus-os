#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-MUTATION-TRANCHE1-EXEC-01, mission section 6: rehearse control.py's worktree, apply, verify,
restore and link steps with SYNTHETIC inputs only. Nothing is built or run; no catalog patch is applied.

    rehearsal.py OUTDIR TOOLCONTROLS      OUTDIR must not exist; TOOLCONTROLS: a tool_controls.py output directory

1. A mission-owned development worktree at C (`nexus-os-p3-c11-mt1x-dev-rehearsal`, locked with its role) is created
   and authenticated by worktrees.py.
2. A synthetic patch (one comment line added to protocol.rs; not a catalog control) is written with `git diff`
   semantics, its expected mutant identity computed in-process, and applied by control.apply_patch; verify_mutant.py
   must report EXACTLY_C_PLUS_PATCH. Negative checks: a wrong expected blob, an extra untracked file and a second
   changed file must each give NOT_THE_MUTANT (the extra file and the second change are undone again and verified).
3. control.restore must return the worktree to EXACTLY_C.
4. control.link is exercised on copies of tool_controls.py's synthetic receipts whose identity fields are set to the
   real ones (positive), and on unchanged synthetic copies (negative), and on the real Stage-2 links of C9 and C18a.
The development worktree is kept (it is never used for a control).
"""
import json
import os
import shutil
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import common as K  # noqa: E402
import control  # noqa: E402
import supervise  # noqa: E402
import worktrees  # noqa: E402

WT = K.NEXUS + "/nexus-os-p3-c11-mt1x-dev-rehearsal"
results = []


def expect(name, ok, detail=""):
    results.append((name, bool(ok), str(detail)))
    print(f"{'REHEARSAL OK ' if ok else 'REHEARSAL BAD'} {name}: {detail}")


def vm(path, blob, sha):
    r = subprocess.run([sys.executable, supervise.VERIFY_MUTANT, WT, path, blob, sha], capture_output=True, text=True)
    return json.loads(r.stdout)["verdict"]


def main(outdir, tc):
    if os.path.exists(outdir):
        raise SystemExit("REFUSED: output exists")
    os.makedirs(outdir)
    exp = supervise.expectations()
    role = "development rehearsal (synthetic patch only; never used for a control)"
    if not os.path.exists(WT):
        r, ok = worktrees.create(WT, role)
        worktrees.write(os.path.join(outdir, "01-worktree"), "CREATE", f"CREATE {WT}", r, ok)
        expect("create the development worktree", ok)
    a, ok = worktrees.authenticate(WT, role)
    worktrees.write(os.path.join(outdir, "01-worktree"), "AUTHENTICATE", f"AUTHENTICATE {WT}", a, ok)
    expect("authenticate the development worktree (exactly C, clean, locked, owned)", ok)
    # A synthetic patch: one comment line after line 1 of protocol.rs.
    base = K.blob(K.C, K.PROTOCOL)
    lines = base.split(b"\n")
    mutant = b"\n".join(lines[:1] + [b"// SYNTHETIC REHEARSAL LINE (not a control)"] + lines[1:])
    patch = (f"diff --git a/{K.PROTOCOL} b/{K.PROTOCOL}\n--- a/{K.PROTOCOL}\n+++ b/{K.PROTOCOL}\n@@ -1,2 +1,3 @@\n"
             ).encode() + b" " + lines[0] + b"\n+// SYNTHETIC REHEARSAL LINE (not a control)\n " + lines[1] + b"\n"
    os.makedirs(os.path.join(outdir, "synthetic"))
    pp = os.path.join(outdir, "synthetic/rehearsal.patch")
    open(pp, "wb").write(patch)
    ctl = {"id": "REHEARSAL", "description": "synthetic comment line (not a control)",
           "patch": {"input": os.path.relpath(pp, K.EV), "sha256": K.sha256_bytes(patch), "catalog_file": "(synthetic)"},
           "source_file": K.PROTOCOL, "mutant_blob": K.git_blob_id(mutant), "mutant_sha256": K.sha256_bytes(mutant),
           "baseline_blob": K.BLOBS[K.PROTOCOL], "baseline_sha256": K.sha256_bytes(base)}
    ok, applied = control.apply_patch(WT, ctl, os.path.join(outdir, "02-apply"))
    expect("apply_patch: applied and EXACTLY_C_PLUS_PATCH", ok and applied)
    expect("the mutant blob is not in the object store",
           subprocess.run(["git", "-C", K.REPO, "cat-file", "-e", ctl["mutant_blob"]], capture_output=True).returncode != 0)
    expect("verify_mutant negative: wrong expected blob", vm(K.PROTOCOL, "0" * 40, ctl["mutant_sha256"]) == "NOT_THE_MUTANT")
    extra = os.path.join(WT, K.CRATE_SRC, "rehearsal_overlay.rs")
    open(extra, "w").write("// synthetic overlay\n")
    expect("verify_mutant negative: an untracked overlay file", vm(K.PROTOCOL, ctl["mutant_blob"], ctl["mutant_sha256"])
           == "NOT_THE_MUTANT")
    os.remove(extra)
    other = os.path.join(WT, K.UNIT)
    ob = open(other, "rb").read()
    open(other, "ab").write(b"// second change\n")
    expect("verify_mutant negative: a second changed file", vm(K.PROTOCOL, ctl["mutant_blob"], ctl["mutant_sha256"])
           == "NOT_THE_MUTANT")
    open(other, "wb").write(ob)
    expect("after undoing the negatives: EXACTLY_C_PLUS_PATCH again",
           vm(K.PROTOCOL, ctl["mutant_blob"], ctl["mutant_sha256"]) == "EXACTLY_C_PLUS_PATCH")
    rec = []
    d = supervise.mutant_check(rec, WT, ctl, "rehearsal")
    expect("supervise.mutant_check agrees", d["verdict"] == "EXACTLY_C_PLUS_PATCH", rec[0][:120])
    rok = control.restore(WT, ctl, os.path.join(outdir, "05-restore"), applied)
    expect("restore: EXACTLY_C", rok)
    a, ok = worktrees.authenticate(WT, role)
    expect("re-authentication after restoration (clean, exactly C)", ok)
    # Links on synthetic receipt copies.
    C = exp["controls"]
    hb, cb = os.path.join(tc, "build-healthy-as-e4"), os.path.join(tc, "build-c22")
    work = os.path.join(outdir, "links")
    os.makedirs(work)
    h2 = os.path.join(work, "healthy")
    shutil.copytree(hb, h2)
    dh = json.load(open(os.path.join(h2, "descriptor.json")))
    dh["source_before"]["verdict"] = dh["source_after"]["verdict"] = "EXACTLY_C"
    json.dump(dh, open(os.path.join(h2, "descriptor.json"), "w"), indent=1)
    dc = json.load(open(os.path.join(cb, "descriptor.json")))
    dc["source_before"]["verdict"] = "EXACTLY_C_PLUS_PATCH"
    c22 = dict(C["C22"], linked={"type": "e4", "dir": os.path.relpath(h2, K.EV)})
    ok, _ = control.link(c22, exp, os.path.join(work, "link-e4-pos"), dc)
    expect("link (e4) positive: synthetic healthy and C22-shaped builds, identity fields set", ok)
    dc2 = dict(dc, env=dict(dc["env"], RUSTFLAGS="-C debuginfo=2"))
    ok, _ = control.link(c22, exp, os.path.join(work, "link-e4-neg-env"), dc2)
    expect("link (e4) negative: an environment difference", not ok)
    dc3 = json.loads(json.dumps(dc))
    dc3["toolchain"]["build_inputs"]["Cargo.lock"]["git_blob"] = "0" * 40
    ok, _ = control.link(c22, exp, os.path.join(work, "link-e4-neg-input"), dc3)
    expect("link (e4) negative: a build-input difference", not ok)
    e2 = os.path.join(work, "e2")
    shutil.copytree(os.path.join(tc, "run-e2-tbind-pass"), e2)
    de = json.load(open(os.path.join(e2, "descriptor.json")))
    for s in ("before", "after"):
        de["executable"][s]["sha256"] = K.EXE_SHA256
    de["source_before"]["verdict"] = de["source_after"]["verdict"] = "EXACTLY_C"
    de["argv"] = [K.EXE, *C["C14"]["argv_tail"]]
    json.dump(de, open(os.path.join(e2, "descriptor.json"), "w"), indent=1)
    c14 = dict(C["C14"], linked={"type": "e2", "dir": os.path.relpath(e2, K.EV)})
    ok, _ = control.link(c14, exp, os.path.join(work, "link-e2-pos"), None)
    expect("link (e2) positive: a PASS receipt with the real identity fields", ok)
    e2n = os.path.join(work, "e2-unchanged")
    shutil.copytree(os.path.join(tc, "run-e2-tbind-pass"), e2n)
    ok, _ = control.link(dict(C["C14"], linked={"type": "e2", "dir": os.path.relpath(e2n, K.EV)}), exp,
                         os.path.join(work, "link-e2-neg"), None)
    expect("link (e2) negative: the synthetic executable is not the pinned baseline executable", not ok)
    for cid in ("C9", "C18a"):
        ok, ref = control.link(C[cid], exp, os.path.join(work, f"link-{cid}-stage2"), None)
        expect(f"link (Stage-2 e1) {cid}: the accepted receipt verifies (read-only)", ok, ref)
    bad = [r for r in results if not r[1]]
    summary = [f"REHEARSAL {K.MISSION}: {len(results)} checks; {len(results) - len(bad)} as expected"]
    summary += [f"{'OK ' if o else 'BAD'} {n}: {d}" for n, o, d in results]
    summary.append("REHEARSAL: " + ("ALL AS EXPECTED" if not bad else "FAILED"))
    open(os.path.join(outdir, "SUMMARY.txt"), "w").write("\n".join(summary) + "\n")
    print("\n".join(summary))
    return 0 if not bad else 1


if __name__ == "__main__":
    sys.exit(main(os.path.abspath(sys.argv[1]), os.path.abspath(sys.argv[2])))

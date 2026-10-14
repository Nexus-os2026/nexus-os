#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-MUTATION-TRANCHE1-RESUME-01, sections 4 B and 8: validate the repaired source/restoration verifier
(verify_mutant.status_policy, control.restored_check, control.restore) before any control is executed.

    verifier_controls.py OUTDIR      OUTDIR must not exist

Nothing is built and no Cargo command or Nexus test runs; no catalog patch is applied. Everything happens in ONE
mission-owned development worktree at C (K.DEV_WT, created and locked here with its role; never used for a control)
with a SYNTHETIC comment-only patch and manually created target sentinels, plus one read-only check of the preserved
restored C22 worktree. Every perturbation is undone by explicit file operations (bytes and modes rewritten from C's
git objects, created files removed, `git update-index --cacheinfo` for the index, `git update-ref --no-deref HEAD`
for HEAD); no reset, clean, checkout or stash. Each undo is verified before the next control.

  P1-P5   the five positive controls of section 8 (and P6: apply and restore end to end with a populated target)
  N6-N16  the negative controls of section 8 (some with sub-cases), each rejected FOR ITS OWN REASON
  R       regression replay: the EXEC-01 verify_mutant.py and restoration predicate reproduce the defect on P4 and P2
  S       sensitivity: a deliberately over-broad policy (any ignored entry allowed) would accept N6, N11 and N12, so
          those controls discriminate the path-aware policy from a blanket ignore
  T       randomized trials in the worktree (seeded): random target populations and at most one random perturbation;
          the verdict must equal the expectation in every trial
  F       randomized differential test of the lexical target-path rule against an independently written oracle
"""
import hashlib
import json
import os
import posixpath
import random
import shutil
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import common as K  # noqa: E402
import control  # noqa: E402
import supervise  # noqa: E402
import verify_mutant  # noqa: E402
import worktrees  # noqa: E402

os.environ["GIT_OPTIONAL_LOCKS"] = "0"  # no git command here rewrites any index (the C22 check is read-only)
WT = K.DEV_WT
ROLE = "development verifier controls (synthetic patch and target sentinels only; never used for a control)"
SEED = 20261014
TRIALS = 48
FUZZ = 20000
SECOND, MISSING, MODE = "README.md", "ROADMAP.md", "CHANGELOG.md"
ORIGINAL_VM = os.path.join(K.EV, "original-tools/verify_mutant.py")
results = []
OUT = None
SCRATCH = None
SYN = {}


class Abort(Exception):
    pass


def note(cid, kind, ok, detail, data=None):
    results.append({"id": cid, "kind": kind, "as_expected": bool(ok), "detail": detail})
    print(f"{'OK ' if ok else 'BAD'} [{cid}] ({kind}) {detail}")
    if data is not None:
        json.dump(data, open(os.path.join(OUT, "records", cid + ".json"), "w"), indent=1, default=str)


def git(*args, wt=WT, check=True):
    r = subprocess.run(["git", "-C", wt, *args], capture_output=True, text=True)
    if check and r.returncode != 0:
        raise Abort(f"git {args}: rc {r.returncode} {r.stderr}")
    return r


def vm(blob=None, sha=None, tool=None):
    r = subprocess.run([sys.executable, tool or supervise.VERIFY_MUTANT, WT, K.PROTOCOL, blob or SYN["blob"],
                        sha or SYN["sha256"]], capture_output=True, text=True)
    d = json.loads(r.stdout)
    d["_rc"] = r.returncode
    return d


def rc(wt=WT, when="verifier control"):
    return control.restored_check(wt, when)


def why(pol):
    return [x["why"] + ": " + x["record"][:90] for x in pol["rejected"]]


def tracked_mode(path):
    return git("ls-tree", K.C, "--", path).stdout.split()[0]


def put_c(path):
    """Rewrite one tracked file from C's git object (bytes and mode)."""
    p = os.path.join(WT, path)
    open(p, "wb").write(K.blob(K.C, path))
    os.chmod(p, 0o755 if tracked_mode(path) == "100755" else 0o644)


def must_mutant(after):
    d = vm()
    if d["verdict"] != "EXACTLY_C_PLUS_PATCH" or d["_rc"] != 0:
        raise Abort(f"after {after}: the worktree is not back to exactly C plus the synthetic patch: {d['verdict']} "
                    f"{d['unexpected_status'][:3]}")


def must_restored(after):
    d = rc()
    if d["verdict"] != "EXACTLY_C":
        raise Abort(f"after {after}: the worktree is not back to exactly C: {why(d['status_policy'])[:3]} "
                    f"{d['verify_tree']['content_mismatches'][:3]}")


def sentinels(root, n, rng, depth=5):
    """n regular files at random depths under root (the manually created nested target)."""
    made = []
    for i in range(n):
        parts = [rng.choice(["release", "debug", "deps", "build", ".fingerprint", "incremental", "a b", "x.y", "...",
                             "-z", "ünï"]) + (str(rng.randrange(9)) if rng.random() < 0.5 else "")
                 for _ in range(rng.randrange(depth))]
        d = os.path.join(root, *parts)
        os.makedirs(d, exist_ok=True)
        f = os.path.join(d, f"sentinel-{i:04}.{rng.choice(['txt', 'rlib', 'd', 'json', 'o'])}")
        open(f, "w").write(f"synthetic target sentinel {i} (verifier_controls.py; not a build product)\n")
        made.append(f)
    return made


def synthetic_patch():
    base = K.blob(K.C, K.PROTOCOL)
    lines = base.split(b"\n")
    line = b"// SYNTHETIC VERIFIER-CONTROL COMMENT LINE (comment only; not a control)"
    mutant = b"\n".join(lines[:1] + [line] + lines[1:])
    patch = (f"diff --git a/{K.PROTOCOL} b/{K.PROTOCOL}\n--- a/{K.PROTOCOL}\n+++ b/{K.PROTOCOL}\n@@ -1,2 +1,3 @@\n"
             ).encode() + b" " + lines[0] + b"\n+" + line + b"\n " + lines[1] + b"\n"
    os.makedirs(os.path.join(OUT, "synthetic"))
    pp = os.path.join(OUT, "synthetic/synthetic-comment.patch")
    open(pp, "wb").write(patch)
    SYN.update({"blob": K.git_blob_id(mutant), "sha256": K.sha256_bytes(mutant), "patch": pp})
    return {"id": "SYNTHETIC", "description": "one comment line (synthetic; not a control)",
            "patch": {"input": os.path.relpath(pp, K.EV), "sha256": K.sha256_bytes(patch), "catalog_file": "(synthetic)"},
            "source_file": K.PROTOCOL, "mutant_blob": SYN["blob"], "mutant_sha256": SYN["sha256"],
            "baseline_blob": K.BLOBS[K.PROTOCOL], "baseline_sha256": K.sha256_bytes(base)}


def tree_state(path):
    """Read-only fingerprint of a directory tree (count, bytes, newest mtime, sha256 of the sorted listing)."""
    rows = []
    for d, dirs, fs in os.walk(path):
        dirs.sort()
        for f in sorted(fs):
            st = os.lstat(os.path.join(d, f))
            rows.append(f"{os.path.relpath(os.path.join(d, f), path)}\t{st.st_size}\t{st.st_mtime_ns}")
    return {"files": len(rows), "listing_sha256": hashlib.sha256("\n".join(rows).encode()).hexdigest()}


def admin_state(wt):
    admin = git("rev-parse", "--path-format=absolute", "--git-dir", wt=wt).stdout.strip()
    out = {"admin": admin}
    for n in ("index", "HEAD", "locked", "gitdir"):
        p = os.path.join(admin, n)
        st = os.stat(p)
        out[n] = {"sha256": K.sha256_file(p), "mtime_ns": st.st_mtime_ns, "size": st.st_size}
    return out


def main(outdir):
    global OUT, SCRATCH
    SCRATCH = os.path.join(K.EV, "dev-local", "scratch-" + os.path.basename(outdir))
    if os.path.exists(outdir) or os.path.exists(SCRATCH):
        raise SystemExit("REFUSED: output exists")
    OUT = outdir
    os.makedirs(os.path.join(outdir, "records"))
    os.makedirs(SCRATCH)
    t0 = time.time()
    print(f"VERIFIER CONTROLS {K.MISSION} {time.strftime('%Y-%m-%dT%H:%M:%S%z')}; seed {SEED}")
    try:
        run(outdir)
    except Abort as e:
        note("ABORT", "harness", False, f"controls aborted: {e}")
    bad = [r for r in results if not r["as_expected"]]
    req = sorted({r["id"].split(".")[0] for r in results if r["id"][0] in "PN"})
    summary = [f"VERIFIER CONTROLS {K.MISSION}: {len(results)} checks; {len(results) - len(bad)} as expected; "
               f"elapsed {time.time() - t0:.1f} s; seed {SEED}",
               f"required section-8 controls exercised: {req}"]
    summary += [f"{'OK ' if r['as_expected'] else 'BAD'} [{r['id']}] ({r['kind']}) {r['detail']}" for r in results]
    summary.append("VERIFIER CONTROLS: " + ("ALL AS EXPECTED" if not bad else "FAILED"))
    open(os.path.join(outdir, "SUMMARY.txt"), "w").write("\n".join(summary) + "\n")
    json.dump(results, open(os.path.join(outdir, "RESULTS.json"), "w"), indent=1)
    print(summary[0])
    print(summary[-1])
    return 0 if not bad else 1


def run(outdir):
    # ---- the development worktree ----------------------------------------------------------------------------------
    if os.path.lexists(WT):
        note("W.1", "setup", True, f"the development worktree {WT} exists (created by an earlier run of this harness); "
             "it is reused only if it authenticates as exactly C, clean and locked with its role")
    else:
        r, ok = worktrees.create(WT, ROLE)
        worktrees.write(os.path.join(outdir, "00-worktree"), "CREATE", f"CREATE {WT} at C", r, ok)
        note("W.1", "setup", ok, f"create the development worktree {WT} at C, locked with its role")
    a, ok = worktrees.authenticate(WT, ROLE)
    worktrees.write(os.path.join(outdir, "00-worktree"), "AUTHENTICATE", f"AUTHENTICATE {WT}", a, ok)
    note("W.2", "setup", ok, "authenticate the development worktree (exactly C, clean, locked, owned)")
    if not ok:
        raise Abort("the development worktree is not exactly C")
    target = os.path.join(WT, "target")
    rng = random.Random(SEED)

    # ---- P1: restored verifier, no target ----------------------------------------------------------------------------
    d = rc()
    note("P1", "positive", d["verdict"] == "EXACTLY_C" and not os.path.lexists(target),
         f"Candidate C, no target: restored verifier {d['verdict']}; {d['status_policy']['records']} status records",
         d)

    # ---- N13r, N14r, N15r, N6r, N7r: restored-mode negatives (no target) ---------------------------------------------
    git("update-ref", "--no-deref", "HEAD", K.C_PARENT)
    d = rc()
    note("N13.r", "negative", d["verdict"] == "NOT_C" and d["verify_tree"]["head"] == K.C_PARENT
         and d["verify_tree"]["index_differs_from_head"], f"wrong HEAD/tree (HEAD moved to C's parent, files and index "
         f"exactly C): restored verifier {d['verdict']}; head {d['verify_tree']['head'][:12]}, tree "
         f"{d['verify_tree']['tree'][:12]}, index differs {d['verify_tree']['index_differs_from_head']}", d)
    git("update-ref", "--no-deref", "HEAD", K.C)
    must_restored("N13.r")
    os.remove(os.path.join(WT, MISSING))
    d = rc()
    note("N14.r", "negative", d["verdict"] == "NOT_C" and MISSING in d["verify_tree"]["missing"]
         and any(x["record"] == " D " + MISSING for x in d["status_policy"]["rejected"]),
         f"missing tracked file {MISSING}: restored verifier {d['verdict']}; {why(d['status_policy'])}", d)
    put_c(MISSING)
    must_restored("N14.r")
    os.chmod(os.path.join(WT, MODE), 0o755)
    d = rc()
    note("N15.r", "negative", d["verdict"] == "NOT_C" and MODE in d["verify_tree"]["mode_mismatches"],
         f"mode mismatch ({MODE} 100644 made executable): restored verifier {d['verdict']}; modes "
         f"{d['verify_tree']['mode_mismatches']}; {why(d['status_policy'])}", d)
    put_c(MODE)
    must_restored("N15.r")
    log = os.path.join(WT, K.CRATE, "sentinel.log")
    open(log, "w").write("synthetic ignored file outside target\n")
    d = rc()
    note("N6.r", "negative", d["verdict"] == "NOT_C" and any(x["why"] == "ignored path outside the worktree-local target"
                                                           for x in d["status_policy"]["rejected"]),
         f"ignored file outside target ({K.CRATE}/sentinel.log, ignored by *.log): restored verifier {d['verdict']}; "
         f"{why(d['status_policy'])}", d)
    os.remove(log)
    must_restored("N6.r")
    rs = os.path.join(WT, K.CRATE_SRC, "verifier_overlay.rs")
    open(rs, "w").write("// synthetic untracked Rust source\n")
    d = rc()
    note("N7.r", "negative", d["verdict"] == "NOT_C" and any(x["why"] == "untracked" and x["record"].endswith(
        "verifier_overlay.rs") for x in d["status_policy"]["rejected"]),
         f"untracked Rust source outside target: restored verifier {d['verdict']}; {why(d['status_policy'])}", d)
    os.remove(rs)
    must_restored("N7.r")

    # ---- N11: target is a symlink (restored and, later, mutant mode) -------------------------------------------------
    elsewhere = os.path.join(SCRATCH, "elsewhere-target")
    sentinels(elsewhere, 12, rng)
    os.symlink(elsewhere, target)
    d = rc()
    t = d["status_policy"]["target"]
    note("N11.r", "negative", d["verdict"] == "NOT_C" and t["is_symlink"] and not t["ok"],
         f"target is a symlink to a directory outside the worktree: restored verifier {d['verdict']}; target facts "
         f"symlink {t['is_symlink']}, own entry {t['own_entry']}; {why(d['status_policy'])}", d)
    os.remove(target)
    c22_target = os.path.join(K.control_wt("C22"), "target")
    os.symlink(c22_target, target)
    d = rc()
    t = d["status_policy"]["target"]
    note("N11b.r", "negative", d["verdict"] == "NOT_C" and t["is_symlink"] and not t["own_entry"],
         f"target is a symlink to ANOTHER worktree's real target ({c22_target}; nothing there is read or written): "
         f"restored verifier {d['verdict']}; realpath {t['realpath']}; {why(d['status_policy'])}", d)
    os.remove(target)
    must_restored("N11.r")

    # ---- P3 and the mutant-mode negatives, no target ----------------------------------------------------------------
    ctl = synthetic_patch()
    ok, applied = control.apply_patch(WT, ctl, os.path.join(outdir, "P3-apply"))
    d = vm()
    note("P3", "positive", ok and applied and d["verdict"] == "EXACTLY_C_PLUS_PATCH" and d["_rc"] == 0
         and d["status_policy"]["target_entries"] == 0,
         f"C plus the synthetic comment-only patch, no target: control.apply_patch {'APPLY OK' if ok else 'FAILED'}; "
         f"mutant verifier {d['verdict']}; shown {d['status_entries']}; mutant blob {d['mutant_file']['git_blob']}", d)
    note("P3.obj", "positive", git("cat-file", "-e", SYN["blob"], wt=K.REPO, check=False).returncode != 0,
         "the synthetic mutant blob is absent from the repository object store")
    must_mutant("P3")
    d = vm(blob="0" * 40)
    note("N9a", "negative", d["verdict"] == "NOT_THE_MUTANT" and d["status_policy"]["ok"]
         and d["mutant_file"]["git_blob"] == SYN["blob"], "mutant file correct, expected blob wrong: mutant verifier "
         f"{d['verdict']} (status policy holds; the blob comparison rejects)", d)
    pp = os.path.join(WT, K.PROTOCOL)
    good = open(pp, "rb").read()
    open(pp, "ab").write(b"// one extra byte sequence\n")
    d = vm()
    note("N9b", "negative", d["verdict"] == "NOT_THE_MUTANT" and d["mutant_file"]["git_blob"] != SYN["blob"],
         f"mutant file with the wrong blob ({d['mutant_file']['git_blob'][:12]} for {SYN['blob'][:12]}): mutant verifier "
         f"{d['verdict']}", d)
    open(pp, "wb").write(good)
    must_mutant("N9b")
    git("update-index", "--info-only", K.PROTOCOL)
    d = vm()
    note("N10", "negative", d["verdict"] == "NOT_THE_MUTANT" and d["index_differs_from_head"] == [K.PROTOCOL]
         and any(x["record"] == "M  " + K.PROTOCOL for x in d["status_policy"]["rejected"]),
         f"mutant file correct but staged (update-index --info-only: no object written): mutant verifier {d['verdict']}; "
         f"index differs {d['index_differs_from_head']}; {why(d['status_policy'])}", d)
    git("update-index", "--cacheinfo", f"100644,{K.BLOBS[K.PROTOCOL]},{K.PROTOCOL}")
    note("N10.obj", "negative", git("cat-file", "-e", SYN["blob"], wt=K.REPO, check=False).returncode != 0,
         "after staging and unstaging, the synthetic mutant blob is still absent from the object store")
    must_mutant("N10")
    sp = os.path.join(WT, SECOND)
    open(sp, "ab").write(b"\n")
    d = vm()
    note("N8", "negative", d["verdict"] == "NOT_THE_MUTANT" and SECOND in d["other_content_mismatches"]
         and any(x["record"] == " M " + SECOND for x in d["status_policy"]["rejected"]),
         f"a second tracked file modified ({SECOND}): mutant verifier {d['verdict']}; other mismatches "
         f"{d['other_content_mismatches']}; {why(d['status_policy'])}", d)
    put_c(SECOND)
    must_mutant("N8")
    open(rs, "w").write("// synthetic untracked Rust source\n")
    d = vm()
    note("N7", "negative", d["verdict"] == "NOT_THE_MUTANT" and any(
        x["why"] == "untracked" for x in d["status_policy"]["rejected"]),
         f"untracked Rust source outside target ({os.path.relpath(rs, WT)}): mutant verifier {d['verdict']}; "
         f"{why(d['status_policy'])}", d)
    os.remove(rs)
    must_mutant("N7")
    open(log, "w").write("synthetic ignored file outside target\n")
    d = vm()
    note("N6", "negative", d["verdict"] == "NOT_THE_MUTANT" and any(
        x["why"] == "ignored path outside the worktree-local target" for x in d["status_policy"]["rejected"]),
         f"ignored file outside target ({os.path.relpath(log, WT)}): mutant verifier {d['verdict']}; "
         f"{why(d['status_policy'])}", d)
    os.remove(log)
    must_mutant("N6")
    crate_target = os.path.join(WT, K.CRATE, "target")
    sentinels(crate_target, 5, rng, depth=2)
    d = vm()
    note("N6b", "negative", d["verdict"] == "NOT_THE_MUTANT" and d["status_policy"]["rejected"] and all(
        x["why"] == "ignored path outside the worktree-local target" for x in d["status_policy"]["rejected"]),
         f"a nested crate-level target ({K.CRATE}/target/..., ignored by **/target), not the worktree's own: mutant "
         f"verifier {d['verdict']}; {len(d['status_policy']['rejected'])} rejected, e.g. {why(d['status_policy'])[:1]}", d)
    shutil.rmtree(crate_target)  # the five sentinels created just above, nothing else
    must_mutant("N6b")
    os.remove(os.path.join(WT, MISSING))
    d = vm()
    note("N14", "negative", d["verdict"] == "NOT_THE_MUTANT" and MISSING in d["missing"],
         f"missing tracked file {MISSING}: mutant verifier {d['verdict']}; missing {d['missing']}; "
         f"{why(d['status_policy'])}", d)
    put_c(MISSING)
    must_mutant("N14")
    os.chmod(os.path.join(WT, MODE), 0o755)
    d = vm()
    note("N15", "negative", d["verdict"] == "NOT_THE_MUTANT" and MODE in d["mode_mismatches"],
         f"mode mismatch ({MODE}): mutant verifier {d['verdict']}; modes {d['mode_mismatches']}; "
         f"{why(d['status_policy'])}", d)
    put_c(MODE)
    must_mutant("N15")
    git("update-ref", "--no-deref", "HEAD", K.C_PARENT)
    d = vm()
    note("N13", "negative", d["verdict"] == "NOT_THE_MUTANT" and d["head"] == K.C_PARENT and d["tree"] != K.C_TREE,
         f"wrong HEAD and tree (C's parent; files exactly C plus the patch): mutant verifier {d['verdict']}; head "
         f"{d['head'][:12]}, tree {d['tree'][:12]}, index differs {d['index_differs_from_head']}", d)
    git("update-ref", "--no-deref", "HEAD", K.C)
    must_mutant("N13")
    os.symlink(elsewhere, target)
    d = vm()
    note("N11", "negative", d["verdict"] == "NOT_THE_MUTANT" and d["status_policy"]["target"]["is_symlink"],
         f"mutant worktree whose target is a symlink: mutant verifier {d['verdict']}; {why(d['status_policy'])}", d)
    os.remove(target)
    must_mutant("N11")

    # ---- the populated nested target: P4, R (replay), N12, S --------------------------------------------------------
    made = sentinels(target, 240, rng, depth=7)
    d = vm()
    note("P4", "positive", d["verdict"] == "EXACTLY_C_PLUS_PATCH" and d["_rc"] == 0
         and d["status_policy"]["target_entries"] == len(made),
         f"C plus the synthetic patch with a populated nested target ({len(made)} sentinel files, depth up to 6): "
         f"mutant verifier {d['verdict']}; {d['status_policy']['target_entries']} ignored entries beneath the real "
         f"target; shown {d['status_entries']}", d)
    old = vm(tool=ORIGINAL_VM)
    note("R.vm", "replay", old["verdict"] == "NOT_THE_MUTANT" and len(old["unexpected_status"]) == len(made) and all(
        s.startswith("!! target/") for s in old["unexpected_status"]) and not old["other_content_mismatches"],
         f"regression replay: the EXEC-01 verify_mutant.py on the P4 state reports {old['verdict']} with "
         f"{len(old['unexpected_status'])} unexpected entries, every one '!! target/...' (the defect reproduced)", old)
    escape_dir = os.path.join(SCRATCH, "escape-outside")
    os.makedirs(escape_dir)
    open(os.path.join(escape_dir, "outside.txt"), "w").write("outside the worktree\n")
    inner = os.path.join(target, "release", "escape-link")
    os.makedirs(os.path.dirname(inner), exist_ok=True)
    os.symlink(os.path.join(escape_dir, "outside.txt"), inner)
    d = vm()
    note("N12b", "negative", d["verdict"] == "NOT_THE_MUTANT" and [x["why"] for x in d["status_policy"]["rejected"]]
         == ["resolves outside the worktree-local target"],
         f"a target descendant that escapes by a symlink (target/release/escape-link -> outside): mutant verifier "
         f"{d['verdict']}; {why(d['status_policy'])}", d)
    os.remove(inner)
    os.symlink(escape_dir, inner)
    d = vm()
    note("N12c", "negative", d["verdict"] == "NOT_THE_MUTANT" and [x["why"] for x in d["status_policy"]["rejected"]]
         == ["resolves outside the worktree-local target"],
         f"a target descendant that escapes by a symlinked directory: mutant verifier {d['verdict']}; "
         f"{why(d['status_policy'])}", d)
    os.remove(inner)
    must_mutant("N12c")
    tf = verify_mutant.target_facts(WT)
    lex = {p: verify_mutant.target_entry_problem(WT, p, tf) for p in (
        "target/../" + K.PROTOCOL, "target/./release/x", "target//x", "target/a/../../README.md", "target/..",
        "target/.", "target/a/./b", "../target/x", "/target/x", "targetx/y", "target", "target/a//")}
    note("N12a", "negative", all(v is not None for v in lex.values()),
         f"target paths with '..', '.', empty components or outside the target entry are rejected by the lexical rule: "
         f"{lex}", lex)
    saved = verify_mutant.status_records
    verify_mutant.status_records = lambda wt: (b"!! target/../README.md\0", [("!!", "target/../README.md")], [])
    try:
        pol = verify_mutant.status_policy(WT, None)
    finally:
        verify_mutant.status_records = saved
    note("N12a.pol", "negative", not pol["ok"] and why(pol)[0].startswith("path traversal"),
         f"the whole policy fed a crafted '..' record rejects it: {why(pol)}", pol)
    saved_m = verify_mutant.mount_points
    verify_mutant.mount_points = lambda: [os.path.realpath(target) + "/release"]
    try:
        pol = verify_mutant.status_policy(WT, K.PROTOCOL)
    finally:
        verify_mutant.mount_points = saved_m
    note("N11m", "negative", not pol["ok"] and pol["target"]["mounts"] and not pol["target"]["ok"],
         f"a mount point beneath the target (simulated in the mount table) makes the target foreign: {why(pol)[:1]}",
         {k: v for k, v in pol.items() if k != "shown"})
    # S: an over-broad policy (every ignored entry allowed) would accept these negatives.
    saved_p = verify_mutant.target_entry_problem
    saved_f = verify_mutant.target_facts
    accepted_by_broad = {}
    try:
        verify_mutant.target_entry_problem = lambda wt, p, tf: None
        verify_mutant.target_facts = lambda wt: {"ok": True, "exists": True}
        open(log, "w").write("synthetic ignored file outside target\n")
        accepted_by_broad["N6"] = verify_mutant.status_policy(WT, K.PROTOCOL)["ok"]
        os.remove(log)
        os.symlink(os.path.join(escape_dir, "outside.txt"), inner)
        accepted_by_broad["N12b"] = verify_mutant.status_policy(WT, K.PROTOCOL)["ok"]
        os.remove(inner)
    finally:
        verify_mutant.target_entry_problem = saved_p
        verify_mutant.target_facts = saved_f
    aside = os.path.join(SCRATCH, "target-aside")  # outside the worktree, so nothing untracked appears
    os.rename(target, aside)
    os.symlink(elsewhere, target)
    try:
        verify_mutant.target_entry_problem = lambda wt, p, tf: None
        verify_mutant.target_facts = lambda wt: {"ok": True, "exists": True}
        accepted_by_broad["N11"] = verify_mutant.status_policy(WT, K.PROTOCOL)["ok"]
    finally:
        verify_mutant.target_entry_problem = saved_p
        verify_mutant.target_facts = saved_f
        os.remove(target)
        os.rename(aside, target)
    note("S", "sensitivity", all(accepted_by_broad.values()),
         f"a deliberately over-broad policy (any ignored entry, any target) would ACCEPT {accepted_by_broad}; the "
         "path-aware policy rejects each (N6, N11, N12b above): the controls discriminate", accepted_by_broad)
    must_mutant("S")

    # ---- P6 and N16: restoration through control.restore with the populated target ------------------------------------
    open(os.path.join(WT, SECOND), "ab").write(b"\n")
    rok = control.restore(WT, ctl, os.path.join(outdir, "N16-restore"), True)
    first = supervise.first_line(os.path.join(outdir, "N16-restore/RESTORE.txt"))
    rj = json.load(open(os.path.join(outdir, "N16-restore/RESTORE.json")))
    note("N16", "negative", not rok and first == "RESTORE: FAILED" and rj["after"]["git_blob"] == K.BLOBS[K.PROTOCOL]
         and SECOND in rj["verify_tree"]["content_mismatches"]
         and any(x["record"] == " M " + SECOND for x in rj["status_policy"]["rejected"]),
         f"restoration with one residual tracked-byte difference ({SECOND} +1 byte; protocol.rs itself restored): "
         f"control.restore {first}; mismatches {rj['verify_tree']['content_mismatches']}; {why(rj['status_policy'])}")
    put_c(SECOND)
    d = rc()
    note("P2", "positive", d["verdict"] == "EXACTLY_C" and d["status_policy"]["target_entries"] == len(made)
         and d["verify_tree"]["status_entries"] == ["!! target/"],
         f"Candidate C with a real populated nested target ({len(made)} sentinels): restored verifier {d['verdict']}; "
         f"{d['status_policy']['target_entries']} ignored entries beneath the real target; verify_tree status "
         f"{d['verify_tree']['status_entries']}", d)
    st = git("status", "--porcelain", "--ignored", "--untracked-files=all").stdout
    note("R.restore", "replay", st.strip() not in ("", "!! target/") and all(
        l.startswith("!! target/") for l in st.strip().splitlines()),
         f"regression replay: the EXEC-01 restoration predicate (status in ('', '!! target/')) would reject this exactly-C "
         f"worktree: {len(st.strip().splitlines())} '!! target/...' lines (the defect reproduced)")
    ok, applied = control.apply_patch(WT, ctl, os.path.join(outdir, "P6-apply"))
    rok = control.restore(WT, ctl, os.path.join(outdir, "P6-restore"), applied)
    note("P6", "positive", ok and applied and rok and supervise.first_line(os.path.join(outdir, "P6-restore/RESTORE.txt"))
         == "RESTORE: EXACTLY_C", f"end to end with the populated target (the EXEC-01 failure case): apply_patch "
         f"{'APPLY OK' if ok else 'FAILED'}, control.restore "
         f"{supervise.first_line(os.path.join(outdir, 'P6-restore/RESTORE.txt'))}")
    must_restored("P6")
    git("update-ref", "--no-deref", "HEAD", K.C_PARENT)
    d = rc()
    note("N13.t", "negative", d["verdict"] == "NOT_C" and d["verify_tree"]["head"] == K.C_PARENT,
         f"wrong HEAD with the populated target: restored verifier {d['verdict']}", d)
    git("update-ref", "--no-deref", "HEAD", K.C)
    must_restored("N13.t")

    # ---- P5: the preserved restored C22 worktree, read-only ----------------------------------------------------------
    c22 = K.control_wt("C22")
    before = {"admin": admin_state(c22), "target": tree_state(os.path.join(c22, "target")),
              "protocol_sha256": K.sha256_file(os.path.join(c22, K.PROTOCOL))}
    d = rc(c22, "C22 preserved worktree, read-only")
    after = {"admin": admin_state(c22), "target": tree_state(os.path.join(c22, "target")),
             "protocol_sha256": K.sha256_file(os.path.join(c22, K.PROTOCOL))}
    note("P5", "positive", d["verdict"] == "EXACTLY_C" and before == after
         and d["status_policy"]["target_entries"] > 2000,
         f"the preserved restored C22 worktree (read-only, GIT_OPTIONAL_LOCKS=0): restored verifier {d['verdict']}; "
         f"{d['status_policy']['target_entries']} ignored entries beneath its real target; protocol.rs sha256 "
         f"{after['protocol_sha256'][:16]}…; admin files and target unchanged by the check: {before == after}",
         {"verifier": d, "before": before, "after": after})

    # ---- T: randomized trials in the worktree ------------------------------------------------------------------------
    perturb = ["none", "untracked", "ignored_outside", "crate_target", "second_tracked", "mode", "missing",
               "target_symlink", "inner_escape", "staged"]
    tally = {"accept_expected_accepted": 0, "reject_expected_rejected": 0, "false_accept": 0, "false_reject": 0}
    trow = []
    for i in range(TRIALS):
        mode = "mutant" if rng.random() < 0.5 else "restored"
        if mode == "mutant":
            ok, applied = control.apply_patch(WT, ctl, os.path.join(outdir, "trials", f"{i:03}-apply"))
            if not ok:
                raise Abort(f"trial {i}: apply")
        shutil.rmtree(target)  # the sentinel directory this harness created, rebuilt per trial
        os.makedirs(target)
        made = sentinels(target, rng.randrange(0, 160), rng, depth=rng.randrange(1, 8))
        kind = "none" if rng.random() < 0.4 else rng.choice(perturb[1:])
        if kind == "staged" and mode == "restored":
            kind = "none"
        undo = None
        if kind == "untracked":
            p = os.path.join(WT, rng.choice([K.CRATE_SRC, "docs", "."]), f"trial-{i}.rs")
            open(p, "w").write("x\n")
            undo = lambda p=p: os.remove(p)
        elif kind == "ignored_outside":
            p = os.path.join(WT, rng.choice([K.CRATE, "docs", "."]), f"trial-{i}.log")
            open(p, "w").write("x\n")
            undo = lambda p=p: os.remove(p)
        elif kind == "crate_target":
            p = os.path.join(WT, K.CRATE, "target")
            sentinels(p, 3, rng, depth=2)
            undo = lambda p=p: shutil.rmtree(p)
        elif kind == "second_tracked":
            f = rng.choice([SECOND, K.UNIT, K.MODEL, "Cargo.toml"])
            open(os.path.join(WT, f), "ab").write(b" ")
            undo = lambda f=f: put_c(f)
        elif kind == "mode":
            f = rng.choice([SECOND, MODE, K.UNIT])
            os.chmod(os.path.join(WT, f), 0o755)
            undo = lambda f=f: put_c(f)
        elif kind == "missing":
            f = rng.choice([MISSING, K.MODEL])
            os.remove(os.path.join(WT, f))
            undo = lambda f=f: put_c(f)
        elif kind == "target_symlink":
            aside = os.path.join(SCRATCH, f"target-aside-{i}")
            os.rename(target, aside)
            os.symlink(elsewhere, target)
            undo = lambda aside=aside: (os.remove(target), os.rename(aside, target))
        elif kind == "inner_escape":
            p = os.path.join(target, f"esc-{i}")
            os.symlink(rng.choice([escape_dir, os.path.join(WT, K.PROTOCOL), "/etc"]), p)
            undo = lambda p=p: os.remove(p)
        elif kind == "staged":
            git("update-index", "--info-only", K.PROTOCOL)
            undo = lambda: git("update-index", "--cacheinfo", f"100644,{K.BLOBS[K.PROTOCOL]},{K.PROTOCOL}")
        d = vm() if mode == "mutant" else rc()
        accepted = d["verdict"] in ("EXACTLY_C_PLUS_PATCH", "EXACTLY_C")
        want = kind == "none"
        key = ("accept_expected_accepted" if accepted and want else "reject_expected_rejected" if not accepted and not
               want else "false_accept" if accepted else "false_reject")
        tally[key] += 1
        trow.append(f"trial {i:03}: mode {mode}; {len(made)} sentinels; perturbation {kind}; verdict {d['verdict']}; "
                    f"{key}")
        if undo:
            undo()
        if mode == "mutant":
            must_mutant(f"trial {i}")
            rok = control.restore(WT, ctl, os.path.join(outdir, "trials", f"{i:03}-restore"), True)
            if not rok:
                raise Abort(f"trial {i}: restore")
        must_restored(f"trial {i}")
    open(os.path.join(outdir, "TRIALS.txt"), "w").write("\n".join(trow) + "\n" + json.dumps(tally) + "\n")
    note("T", "randomized", tally["false_accept"] == 0 and tally["false_reject"] == 0
         and tally["accept_expected_accepted"] > 0 and tally["reject_expected_rejected"] > 0,
         f"{TRIALS} seeded trials in the worktree (random target populations, at most one random perturbation): {tally}")

    # ---- F: lexical rule vs an independent oracle --------------------------------------------------------------------
    alphabet = ["target", "target/", "/", "//", ".", "..", "...", "a", "b c", "ü", "release", "-", ".d", "x..",
                "targetx", "TARGET"]
    tf = verify_mutant.target_facts(WT)
    dis = []
    for _ in range(FUZZ):
        p = "".join(rng.choice(alphabet) for _ in range(rng.randrange(1, 7)))
        q = p[:-1] if p.endswith("/") else p
        oracle = p.startswith("target/") and posixpath.normpath(q) == q
        got = verify_mutant.target_entry_problem(WT, p, tf)
        lexical_ok = got is None or got == "resolves outside the worktree-local target"
        if lexical_ok != oracle:
            dis.append((p, got, oracle))
    note("F", "randomized", not dis, f"{FUZZ} random paths: the lexical target-path rule agrees with an independent "
         f"posixpath.normpath oracle in every case ({len(dis)} disagreements){': ' + str(dis[:5]) if dis else ''}")

    # ---- the end state ------------------------------------------------------------------------------------------------
    shutil.rmtree(target)  # this harness's sentinels only
    d = rc()
    a, ok = worktrees.authenticate(WT, ROLE)
    worktrees.write(os.path.join(outdir, "99-worktree"), "AUTHENTICATE", f"AUTHENTICATE {WT} at the end", a, ok)
    note("W.3", "setup", d["verdict"] == "EXACTLY_C" and ok and not os.path.lexists(target),
         "end state: the development worktree is exactly C, clean, without target, locked (kept for review)")
    note("W.4", "setup", git("cat-file", "-e", SYN["blob"], wt=K.REPO, check=False).returncode != 0,
         "the synthetic mutant blob was never written to the repository object store")


if __name__ == "__main__":
    sys.exit(main(os.path.abspath(sys.argv[1])))

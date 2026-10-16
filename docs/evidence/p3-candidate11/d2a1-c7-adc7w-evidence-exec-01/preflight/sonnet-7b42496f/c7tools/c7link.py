"""G.7(D) / I-C7.7 link tool (mission P3-C11-DR01-D2A1-C7-W-EVIDENCE-EXEC-01).

LINKS (never reruns, never copies into the accepted evidence, never modifies) the two accepted restored-side receipts
44-full-F-PT (F-PT) and 43-full-F-3 (F-3), read from git objects at the accepted evidence commit. The link states that they are
C-BASE PASS on E-C and NOT W-SRC PASS, and claims nothing about E-W or E-W7 reproducing them.

CLI:
  c7link.py link <outdir> <wt_healthy> <wt_mutant> <bwc_json> <pin_dir> <mission_file>
  c7link.py selftest <new-dir>
Writes <outdir>/LINK.txt and LINK.json (exclusive-create).  Exit 0 only when the link is USABLE.
"""
import json
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from c7lib import *  # noqa: F401,F403

FROZEN = {
    "44-full-F-PT": {
        "test": "ledger::protocol::model_tests::gc_symmetry_fpt",
        "raw_sha256": "0863794ef7500c25382f55307213c65557f2d02b7bc4bb433511a71e082119a8",
        "raw_size": 7584,
        "raw_blob_prefix": "a92cb8dabd8109f7f237821ce3fba588eee9434e",
        "checks": "37 of 37 hold",
    },
    "43-full-F-3": {
        "test": "ledger::protocol::model_tests::gc_symmetry_f3",
        "raw_sha256": "6dfc796a0b5144184e3e50c9b1b873c21bb31bf26405c572fe858fb694245562",
        "raw_size": 7580,
        "raw_blob_prefix": "872ef461d465070aabc4418dbe36fd47b86f9d91",
        "checks": "36 of 36 hold",
    },
}
EC_PIN_BLOB_PREFIX = "688fa49b"
EC_PIN_PATH = EC_RECEIPTS + "/10-build/PIN.json"
EC_EXE_PATH = "/home/nexus/NEXUS/nexus-os-p3-c11-final-baseline-known-runs-01/target/release/deps/nexus_governed_control-2faa0b9718d3e6a3"
FILTERED_RE = re.compile(r"^test result: ok\. 1 passed; 0 failed; 0 ignored; 0 measured; 347 filtered out; finished in [0-9.]+s$", re.M)


class Src:
    """Reads files of the accepted evidence commit from git objects; `tamper` (self-test only) overrides contents."""

    def __init__(self, repo=REPO, commit=EC_EVID, tamper=None):
        self.repo, self.commit, self.tamper = repo, commit, tamper or {}

    def read(self, path):
        if path in self.tamper:
            return self.tamper[path]
        return blob_at(self.commit, path, cwd=self.repo)

    def lstree(self, path):
        out = git("ls-tree", "-r", self.commit, path, cwd=self.repo).stdout.decode()
        res = {}
        for line in out.splitlines():
            meta, name = line.split("\t", 1)
            _mode, typ, oid = meta.split()
            assert typ == "blob"
            res[name] = oid
        return res


def worktree_file(wt, rel):
    p = os.path.join(wt, rel)
    with open(p, "rb") as f:
        return f.read()


def link(outdir, wt_healthy, wt_mutant, bwc_json, pin_dir, mission_file, repo=REPO, tamper=None, write=True):
    rec = Rec("G.7(D) link — restored-side evidence (I-C7.7)")
    src = Src(repo=repo, tamper=tamper)

    # ---- the B-W gate: usable only when B-W/A, B-W/B and B-W/C all hold
    try:
        bwc = json.load(open(bwc_json))
    except Exception as e:  # noqa: BLE001
        bwc = {"satisfied": False, "verdict": "unreadable: %r" % (e,)}
    bwc_ok = bool(bwc.get("satisfied") is True and str(bwc.get("verdict", "")).startswith("B-W/C: MECHANICALLY SATISFIED"))
    rec.check("LK-0a", "B-W/C is MECHANICALLY SATISFIED in this mission's own receipts/30-bwc/B-W-C.json", bwc_ok, str(bwc.get("verdict", ""))[:120])
    mission_sha = sha256_file(mission_file) if os.path.exists(mission_file) else None
    rec.check("LK-0b", "mission text retained (it conveys B-W/A and B-W/B)", mission_sha is not None, "sha256 %s" % mission_sha)
    rec.info("LK-0c", "B-W/A (Owner adoption of CR-B) and B-W/B (Architect source review for this exact W-COMMIT) are conveyed by the text of the mission; "
                      "they are recorded, not re-established, by this executor")

    # ---- accepted evidence commit identity
    ec = git_out("rev-parse", EC_EVID + "^{commit}", cwd=repo)
    rec.check("LK-1", "accepted evidence commit resolves to the frozen id", ec == EC_EVID, ec)
    ec_tree = git_out("rev-parse", EC_EVID + "^{tree}", cwd=repo)
    rec.info("LK-1b", "tree of the evidence commit: %s" % ec_tree)

    # ---- E-C pin
    pin_raw = src.read(EC_PIN_PATH)
    pin_blob = git_blob_id(pin_raw)
    rec.check("LK-2a", "E-C PIN.json blob id", pin_blob.startswith(EC_PIN_BLOB_PREFIX) or bool(tamper), pin_blob)
    pin = json.loads(pin_raw)
    rec.check("LK-2b", "E-C PIN.json executable path, sha256, size equal the frozen E-C",
              pin["executable"] == EC_EXE_PATH and pin["sha256"] == E_C_SHA and pin["size"] == E_C_SIZE, "%s %s %s" % (pin["executable"], pin["sha256"], pin["size"]))
    rec.check("LK-2c", "E-C PIN.json names C-BASE (commit and tree)", pin["commit"] == C_BASE and pin["tree"] == C_TREE, "%s %s" % (pin["commit"], pin["tree"]))

    receipts = {}
    for rid, fz in FROZEN.items():
        base = "%s/%s" % (EC_RECEIPTS, rid)
        tree = src.lstree(base)
        names = sorted(n[len(base) + 1:] for n in tree)
        hashes = {}
        for fn in names:
            data = src.read("%s/%s" % (base, fn))
            gid = git_blob_id(data)
            hashes[fn] = {"git_blob": tree["%s/%s" % (base, fn)], "git_blob_recomputed": gid, "sha256": sha256_bytes(data), "size": len(data)}
        rec.check("LK-3a[%s]" % rid, "receipt directory contents read from git objects", {"raw-output.txt", "descriptor.json", "CLASSIFICATION.txt", "RECEIPT.txt"} <= set(names), ",".join(names))
        if not tamper:
            rec.check("LK-3b[%s]" % rid, "every file's recomputed git blob id equals the tree entry",
                      all(h["git_blob"] == h["git_blob_recomputed"] for h in hashes.values()), "")
        raw = src.read(base + "/raw-output.txt")
        desc = json.loads(src.read(base + "/descriptor.json"))
        rec.check("LK-4a[%s]" % rid, "raw-output sha256 equals the frozen value", sha256_bytes(raw) == fz["raw_sha256"], sha256_bytes(raw))
        rec.check("LK-4b[%s]" % rid, "raw-output sha256 equals the descriptor's value", sha256_bytes(raw) == desc["raw_output_sha256"], desc["raw_output_sha256"])
        rec.check("LK-4c[%s]" % rid, "raw-output size equals the frozen value", len(raw) == fz["raw_size"], str(len(raw)))
        rec.check("LK-4d[%s]" % rid, "raw-output git blob id has the frozen prefix", git_blob_id(raw).startswith(fz["raw_blob_prefix"]) or git_blob_id(raw) == fz["raw_blob_prefix"],
                  git_blob_id(raw))
        argv = desc["argv"]
        rec.check("LK-5a[%s]" % rid, "argv is the frozen test name with --exact --ignored --nocapture --test-threads=1 on the E-C executable",
                  argv == [EC_EXE_PATH, fz["test"], "--exact", "--ignored", "--nocapture", "--test-threads=1"], json.dumps(argv[1:]))
        ex = desc["executable"]
        rec.check("LK-5b[%s]" % rid, "executable path, pinned sha256, sha256 before and after all equal E-C",
                  ex["path"] == EC_EXE_PATH and ex["pinned_sha256"] == E_C_SHA and ex["sha256_before"] == E_C_SHA and ex["sha256_after"] == E_C_SHA and ex["size"] == E_C_SIZE,
                  "%s size %s" % (ex["sha256_after"], ex["size"]))
        pr = desc["process"]
        rec.check("LK-5c[%s]" % rid, "exit status 0, no signal, supervisor clean", pr["exit_status"] == 0 and pr["signal"] is None and pr["supervisor_rc"] == 0, json.dumps(pr))
        sup = desc["supervisor"]
        rec.check("LK-5d[%s]" % rid, "no OOM, no swap, completed, not timed out",
                  sup["oom"] == 0 and sup["oom_kill"] == 0 and str(sup["swap_peak_bytes"]) == "0" and sup["completed"] is True and sup["timed_out"] is False, "")
        sb, sa = desc["source_before"], desc["source_after"]
        rec.check("LK-5e[%s]" % rid, "source was EXACTLY_C at C-BASE before and after the run (this is C-BASE, not W-SRC)",
                  sb["verdict"] == sa["verdict"] == "EXACTLY_C" and sb["head"] == sa["head"] == C_BASE and sb["tree"] == sa["tree"] == C_TREE, "")
        sel = desc["selection"]
        rec.check("LK-5f[%s]" % rid, "the exact selection resolved to exactly one test", sel["listed"] == ["%s: test" % fz["test"]] and sel["exit"] == 0, json.dumps(sel["listed"]))
        cls = src.read(base + "/CLASSIFICATION.txt").decode("utf-8", "replace")
        first = cls.splitlines()[0] if cls else ""
        rec.check("LK-6a[%s]" % rid, "accepted CLASSIFICATION is PASS", first == "CLASSIFICATION: PASS", first)
        rec.check("LK-6b[%s]" % rid, "accepted checks line", ("checks: " + fz["checks"]) in cls.splitlines(), fz["checks"])
        text = raw.decode("utf-8", "replace")
        m = FILTERED_RE.findall(text)
        rec.check("LK-6c[%s]" % rid, "raw output ends in `1 passed; 0 failed; 0 ignored; 0 measured; 347 filtered out` (E-C has 348 tests)", len(m) == 1, "")
        receipts[rid] = {"directory": base, "files": hashes, "test": fz["test"], "raw_sha256": sha256_bytes(raw), "raw_size": len(raw),
                         "classification": first, "executable": ex["path"], "executable_sha256": ex["sha256_after"], "finished_wall_seconds": sup["wall_seconds"]}

    # ---- artifact-identity check against E-C (never E-W)
    pins = {}
    for role in ("E-W", "E-W7"):
        try:
            pins[role] = json.load(open(os.path.join(pin_dir, role + ".json")))["executable"]
        except Exception as e:  # noqa: BLE001
            pins[role] = None
            rec.check("LK-7a[%s]" % role, "pin readable", False, repr(e))
    for role, pj in pins.items():
        if pj is None:
            continue
        rec.check("LK-7b[%s]" % role, "%s is a different artifact from E-C (different sha256 and path)" % role,
                  pj["sha256"] != E_C_SHA and pj["path"] != EC_EXE_PATH, "%s %s" % (pj["sha256"], pj["path"]))
    if all(pins.values()):
        rec.check("LK-7c", "E-W and E-W7 differ from each other", pins["E-W"]["sha256"] != pins["E-W7"]["sha256"], "")
    rec.info("LK-7d", "Identity check is made against E-C only. Nothing here is a claim that E-W or E-W7 reproduces these receipts.")

    # ---- source identities: Candidate C protocol.rs, append-only prefix
    rec.check("LK-8a", "protocol.rs blob in C-BASE is Candidate C's", blob_id_at(C_BASE, PROTOCOL_PATH, cwd=repo) == PROTOCOL_C_BLOB, blob_id_at(C_BASE, PROTOCOL_PATH, cwd=repo))
    rec.check("LK-8b", "protocol.rs blob in W-TREE is Candidate C's (the append-only patch does not touch it)",
              blob_id_at(W_COMMIT, PROTOCOL_PATH, cwd=repo) == PROTOCOL_C_BLOB and blob_id_at(W_TREE, PROTOCOL_PATH, cwd=repo) == PROTOCOL_C_BLOB, "")
    mc = blob_at(C_BASE, MODEL_TESTS_PATH, cwd=repo)
    mw = blob_at(W_COMMIT, MODEL_TESTS_PATH, cwd=repo)
    rec.check("LK-8c", "model_tests.rs at C-BASE has the frozen blob/size/sha256",
              git_blob_id(mc) == MODEL_TESTS_C_BLOB and len(mc) == MODEL_TESTS_C_SIZE and sha256_bytes(mc) == MODEL_TESTS_C_SHA, git_blob_id(mc))
    rec.check("LK-8d", "model_tests.rs at C-BASE is a byte prefix of W-BLOB (append-only)",
              git_blob_id(mw) == W_BLOB and sha256_bytes(mw) == W_BLOB_SHA and len(mw) == W_BLOB_SIZE and mw[:len(mc)] == mc and len(mw) > len(mc), "W-BLOB %s" % git_blob_id(mw))
    for name, wt in (("healthy", wt_healthy), ("restored mutant", wt_mutant)):
        try:
            p = worktree_file(wt, PROTOCOL_PATH)
            m_ = worktree_file(wt, MODEL_TESTS_PATH)
            rec.check("LK-9a[%s]" % name, "protocol.rs in the %s worktree is Candidate C's (blob a2df2841, sha256 %s…)" % (name, PROTOCOL_C_SHA[:8]),
                      git_blob_id(p) == PROTOCOL_C_BLOB and sha256_bytes(p) == PROTOCOL_C_SHA, git_blob_id(p))
            rec.check("LK-9b[%s]" % name, "protocol.rs in the %s worktree is not the mutant (0d4ab992…)" % name,
                      git_blob_id(p) != MUTANT_BLOB and sha256_bytes(p) != MUTANT_SHA, "")
            rec.check("LK-9c[%s]" % name, "model_tests.rs in the %s worktree is W-BLOB" % name, git_blob_id(m_) == W_BLOB and sha256_bytes(m_) == W_BLOB_SHA, git_blob_id(m_))
        except OSError as e:
            rec.check("LK-9[%s]" % name, "worktree files readable", False, repr(e))
    p = git("cat-file", "-e", MUTANT_BLOB, cwd=repo, check=False)
    rec.check("LK-10", "mutant protocol.rs blob 0d4ab992 was never written to the object store", p.returncode != 0, "git cat-file -e rc=%d" % p.returncode)

    # ---- statements
    rec.log("")
    rec.log("## What this link is")
    rec.log("(D) is the accepted C-BASE F-PT PASS (receipt 44-full-F-PT) and the accepted C-BASE F-3 PASS (43-full-F-3), each on E-C (%s…), at the accepted evidence commit %s." % (E_C_SHA[:16], EC_EVID))
    rec.log("They are C-BASE PASS on E-C. They are NOT W-SRC PASS. This link makes no claim that E-W or E-W7 reproduces them; E-W and E-W7 were not run on these tests and the receipts were not rerun.")
    rec.log("Their link to W-SRC rests only on: (i) protocol.rs is blob %s in C-BASE, in W-TREE, and in both worktrees; (ii) model_tests.rs of C-BASE is a byte prefix of W-BLOB (append-only); "
            "(iii) the build inputs listed in AD-C7-W NI-6 are blob-equal to C-BASE (checked by the authorisation gate and by receipts/30-bwc)." % PROTOCOL_C_BLOB)
    rec.log("The link is usable only when B-W/A, B-W/B and B-W/C all hold. B-W/C is mechanical, subject to independent Architect audit.")
    usable = (not rec.fails)
    verdict = ("(D) LINK USABLE — C-BASE PASS on E-C; NOT W-SRC PASS" if usable else "(D) LINK NOT USABLE (failed: %s)" % ", ".join(sorted(set(rec.fails))))
    rec.log("")
    rec.log(verdict)
    if write:
        os.makedirs(outdir, exist_ok=True)
        rec.write(os.path.join(outdir, "LINK.txt"))
        with open(os.path.join(outdir, "LINK.json"), "x") as f:
            json.dump({"mission": MISSION, "verdict": verdict, "usable": usable, "failed_checks": sorted(set(rec.fails)), "evidence_commit": EC_EVID, "evidence_tree": ec_tree,
                       "e_c": {"path": EC_EXE_PATH, "sha256": E_C_SHA, "size": E_C_SIZE}, "receipts": receipts,
                       "b_w_c_json": bwc_json, "b_w_c_satisfied": bwc_ok, "mission_sha256": mission_sha,
                       "statement": "C-BASE PASS on E-C, not W-SRC PASS; nothing is claimed about E-W or E-W7 reproducing it", "finished": now()}, f, indent=1)
    return 0 if usable else 1, rec


# ----------------------------------------------------------------------------- self-test
def selftest(base):
    import shutil
    import tempfile  # noqa: F401
    os.makedirs(base)
    ok = 0
    total = 0
    problems = []

    def case(name, expect_fail, **kw):
        nonlocal ok, total
        total += 1
        d = os.path.join(base, "case%02d" % total)
        os.makedirs(d)
        wt_h = os.path.join(d, "wt-h")
        wt_m = os.path.join(d, "wt-m")
        for wt, pc in ((wt_h, None), (wt_m, kw.get("mutant_protocol"))):
            for rel, data in ((PROTOCOL_PATH, pc if pc is not None else blob_at(C_BASE, PROTOCOL_PATH)),
                              (MODEL_TESTS_PATH, kw.get("model_tests") if kw.get("model_tests") is not None else blob_at(W_COMMIT, MODEL_TESTS_PATH))):
                fp = os.path.join(wt, rel)
                os.makedirs(os.path.dirname(fp), exist_ok=True)
                with open(fp, "wb") as f:
                    f.write(data)
        pins = os.path.join(d, "pins")
        os.makedirs(pins)
        pw = kw.get("pin_w_sha", "a" * 64)
        for role, sha, path in (("E-W", pw, "/x/healthy"), ("E-W7", "b" * 64, "/x/mutant")):
            json.dump({"executable": {"path": path, "sha256": sha, "size": 1}}, open(os.path.join(pins, role + ".json"), "w"))
        bw = os.path.join(d, "bwc.json")
        json.dump({"satisfied": kw.get("bwc_ok", True), "verdict": "B-W/C: MECHANICALLY SATISFIED (x)" if kw.get("bwc_ok", True) else "B-W/C: NOT ESTABLISHED"}, open(bw, "w"))
        mf = os.path.join(d, "mission.txt")
        open(mf, "w").write("mission text")
        import io
        import contextlib
        buf = io.StringIO()
        with contextlib.redirect_stdout(buf):
            rc, rec = link(os.path.join(d, "out"), wt_h, wt_m, bw, pins, mf, tamper=kw.get("tamper"), write=kw.get("write", True))
        failed = sorted(set(rec.fails))
        good = (rc != 0 and expect_fail in failed) if expect_fail else (rc == 0 and not failed)
        if good:
            ok += 1
        else:
            problems.append("%s: rc=%s failed=%s expected=%s" % (name, rc, failed, expect_fail))
        print("%s %s" % ("ok  " if good else "FAIL", name), flush=True)
        return d

    rawF = "%s/44-full-F-PT/raw-output.txt" % EC_RECEIPTS
    descF = "%s/44-full-F-PT/descriptor.json" % EC_RECEIPTS
    clsF = "%s/44-full-F-PT/CLASSIFICATION.txt" % EC_RECEIPTS
    raw_ok = blob_at(EC_EVID, rawF)
    desc_ok = json.loads(blob_at(EC_EVID, descF))
    d = case("all synthetic worktrees faithful, real receipts via git objects", None)
    out = os.path.join(d, "out")
    total += 1
    files = sorted(os.listdir(out))
    if files == ["LINK.json", "LINK.txt"] and json.load(open(os.path.join(out, "LINK.json")))["usable"] is True:
        ok += 1
        print("ok   LINK.txt and LINK.json written, usable", flush=True)
    else:
        problems.append("outputs: %s" % files)
        print("FAIL outputs %s" % files)
    total += 1
    try:
        with open(os.path.join(out, "LINK.txt"), "x"):
            pass
        problems.append("LINK.txt was overwritable")
        print("FAIL exclusive-create")
    except FileExistsError:
        ok += 1
        print("ok   LINK.txt is exclusive-create", flush=True)
    case("B-W/C not satisfied", "LK-0a", bwc_ok=False)
    case("raw output tampered by one byte", "LK-4a[44-full-F-PT]", tamper={rawF: raw_ok[:-1] + b"X"})
    d2 = dict(desc_ok)
    d2["executable"] = dict(desc_ok["executable"], sha256_after="0" * 64)
    case("descriptor executable sha256_after differs from E-C", "LK-5b[44-full-F-PT]", tamper={descF: json.dumps(d2).encode()})
    case("descriptor argv drops --ignored", "LK-5a[44-full-F-PT]", tamper={descF: json.dumps(dict(desc_ok, argv=[a for a in desc_ok["argv"] if a != "--ignored"])).encode()})
    case("accepted classification is not PASS", "LK-6a[44-full-F-PT]", tamper={clsF: b"CLASSIFICATION: FAIL\n"})
    case("E-W pin equals E-C", "LK-7b[E-W]", pin_w_sha=E_C_SHA)
    case("restored mutant worktree still has the mutant protocol.rs", "LK-9a[restored mutant]", mutant_protocol=blob_at(C_BASE, PROTOCOL_PATH).replace(b"self.core.refusals.remove(key);", b"self.core.refusals.clear();", 1))
    case("restored mutant worktree model_tests.rs is C's, not W-BLOB", "LK-9c[restored mutant]", model_tests=blob_at(C_BASE, MODEL_TESTS_PATH))
    total += 1
    # the object store must be untouched by all of this
    p = git("cat-file", "-e", MUTANT_BLOB, check=False)
    if p.returncode != 0:
        ok += 1
        print("ok   mutant blob not in the object store after the self-test", flush=True)
    else:
        problems.append("mutant blob present")
        print("FAIL mutant blob present")
    shutil.rmtree(os.path.join(base, "unused"), ignore_errors=True)
    if problems or ok != total:
        print("SELF-TEST (c7link) FAILED: %d/%d" % (ok, total))
        for pr in problems:
            print("  " + pr)
        return 1
    print("SELF-TEST (c7link) ALL OK: %d/%d" % (ok, total))
    return 0


if __name__ == "__main__":
    if len(sys.argv) >= 2 and sys.argv[1] == "selftest":
        sys.exit(selftest(sys.argv[2]))
    if len(sys.argv) == 8 and sys.argv[1] == "link":
        rc, _ = link(*sys.argv[2:8])
        sys.exit(rc)
    print(__doc__)
    sys.exit(2)

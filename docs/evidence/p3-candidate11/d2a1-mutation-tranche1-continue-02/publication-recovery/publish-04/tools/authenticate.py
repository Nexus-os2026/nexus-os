#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-MUTATION-TRANCHE1-PUBLISH-04, mission sections 3, 4 and 5: read-only authentication of every input.
Adapted from PUBLISH-03 tools/authenticate.py (the same checks of Candidate C, the catalog and the seven patches, the
EXEC-01 and RESUME-01 commits, packets, SHA256SUMS and review refs, the CONTINUE-02 freeze, gates, preserved failure and
control receipts, the retained executable identities), plus the two read-only roots:

- the CONTINUE-02 root equals PUBLISH-03's source-evidence manifest (this mission's before-manifest is byte-identical
  to PUBLISH-03's receipts/00-SOURCE-MANIFEST-BEFORE.tsv and 05-SOURCE-MANIFEST-AFTER.tsv);
- the PUBLISH-03 root verifies against its receipts/90-ROOT-SHA256SUMS.txt (every listed sha256, and the listing covers
  every regular file outside scratch/ but itself), and this mission's before-manifest agrees with it;
- both preserved publication failures (CONTINUE-02's and PUBLISH-03's) are present as recorded.

    authenticate.py      each check prints one line; exit 0 only if every check holds. Nothing is written anywhere.
"""
import json
import os
import stat
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import pub4common as Q  # noqa: E402

fails = []
BLOBS = {"crates/nexus-governed-control/src/ledger/protocol.rs": "a2df2841b99cad5d452008a6d4150e9369d07e66",
         "crates/nexus-governed-control/src/ledger/protocol/model_tests.rs": "28085ce232827301104b5d757af042ec933976f8",
         "crates/nexus-governed-control/src/ledger/protocol/unit_tests.rs": "55dc66409eaccdaa93e8f0a56f2ccb14be81e114"}
BASE_EXE = Q.NEXUS + "/nexus-os-p3-c11-final-baseline-known-runs-01/target/release/deps/nexus_governed_control-2faa0b9718d3e6a3"
BASE_EXE_SHA = "cde9f9d20f32f2b8a17b7dcc2f521462ab6c7a6292d5e2cbd1dd2ba2c3d0d30c"
BASE_EXE_SIZE = 8984464
FREEZE_SHA = "831f4b27f878dade62dd7f676b6f85c963af667024479fa74ed71bfb42c7c84b"
FROZEN_BUILDER_SHA = "a42cd96a9d996d2063adc916a606bf7e0cb9933ff18707afac7e2c2aacfe2e45"
CANDIDATE_CLASS = "CLASSIFICATION: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING"
BEFORE = os.path.join(Q.P, "receipts/00-ROOT-MANIFESTS-BEFORE")
# PUBLISH-03 identities (its receipts/90-ROOT-SHA256SUMS.txt, re-verified below, is the source of truth for each).
P3_BUILDER_SHA = "d243334992be1945ace49cf9460a66fff2e9210ef138e122dec86083cc6bd44e"
P3_MAP_SHA = "8842a5e5a5e082ae0676adef5b83ac3743154bef7ace254f15cf8f15df579dd5"
P3_STOP_SHA = "98c752219882cb5909e0cc6d7dc387b3f54651aa7ccb76bde0f96bcd3b877f41"


def check(label, ok, detail=""):
    print(f"{'OK  ' if ok else 'FAIL'} {label}{(': ' + str(detail)) if detail != '' else ''}")
    if not ok:
        fails.append(label)


def first(path):
    return open(path).read().splitlines()[0] if os.path.exists(path) and os.path.getsize(path) else None


def tree_map(commit, prefix):
    out = {}
    for e in Q.git("ls-tree", "-r", "-z", commit, prefix + "/").split(b"\0"):
        if e:
            meta, path = e.split(b"\t", 1)
            out[path.decode()] = meta.decode().split()
    return out


def commit_parents(c):
    head = Q.git("cat-file", "commit", c).decode().partition("\n\n")[0].splitlines()
    return [l[7:] for l in head if l.startswith("parent ")]


def packet(name, commit, tree, parent, prefix, count, ref, rm):
    print(f"[{name}]")
    check(f"{name} commit {commit[:8]}: tree", Q.git("rev-parse", commit + "^{tree}").decode().strip() == tree, tree)
    check(f"{name} commit {commit[:8]}: single parent {parent[:8]}", commit_parents(commit) == [parent])
    tm = tree_map(commit, prefix)
    sums_blob = tm[prefix + "/SHA256SUMS"][2]
    sums = Q.git("cat-file", "blob", sums_blob).decode()
    listed = [l.split("  ", 1) for l in sums.splitlines() if l]
    bad = [p for h, p in listed if Q.sha256_bytes(Q.git("cat-file", "blob", tm[prefix + "/" + p][2])) != h]
    paths = sorted(p[len(prefix) + 1:] for p in tm)
    check(f"{name} SHA256SUMS lists every packet file but itself and every sha256 verifies against the git blob",
          not bad and sorted(p for _, p in listed) == [p for p in paths if p != "SHA256SUMS"],
          f"{len(listed)} listed; {len(paths)} paths; SHA256SUMS blob {sums_blob} sha256 {Q.sha256_bytes(sums.encode())}")
    check(f"{name} packet path count", len(paths) == count, len(paths))
    check(f"{name} review ref {ref} points at {commit[:8]} on the remote", rm.get(ref) == commit, rm.get(ref))
    return tm


def manifest_rows(path):
    ls = open(path).read().splitlines()
    return {l.split("\t")[0]: l.split("\t") for l in ls[1:]}


def roots():
    print("[read-only root: CONTINUE-02 execution evidence]")
    mine = os.path.join(BEFORE, "continue-02.tsv")
    for rel in (Q.P3_SRC_MANIFEST, "receipts/05-SOURCE-MANIFEST-AFTER.tsv"):
        check(f"this mission's before-manifest of the CONTINUE-02 root is byte-identical to PUBLISH-03 {rel} (every "
              "entry: path, type, size, sha256, mode, mtime_ns, symlink target)",
              Q.sha256_file(mine) == Q.sha256_file(os.path.join(Q.P3, rel)) == Q.P3_SRC_MANIFEST_SHA,
              Q.sha256_file(mine))
    print("[read-only root: PUBLISH-03 publication recovery]")
    sums_path = os.path.join(Q.P3, Q.P3_ROOT_SUMS)
    listed = [l.split("  ", 1) for l in open(sums_path).read().splitlines() if l]
    bad = [p for h, p in listed if not os.path.isfile(os.path.join(Q.P3, p)) or os.path.islink(os.path.join(Q.P3, p))
           or Q.sha256_file(os.path.join(Q.P3, p)) != h]
    files = sorted(os.path.relpath(os.path.join(d, f), Q.P3) for d, _, fs in os.walk(Q.P3) for f in fs)
    outside = sorted(f for f in files if not f.startswith("scratch/") and f != Q.P3_ROOT_SUMS)
    check(f"PUBLISH-03 {Q.P3_ROOT_SUMS}: every listed sha256 verifies (regular files, no symlink)", not bad,
          f"{len(listed)} listed; sha256 {Q.sha256_file(sums_path)}; mismatched {bad}")
    check(f"PUBLISH-03 {Q.P3_ROOT_SUMS} lists every regular file outside scratch/ but itself, and nothing else",
          sorted(p for _, p in listed) == outside, f"{len(outside)} files outside scratch/")
    rows = manifest_rows(os.path.join(BEFORE, "publish-03.tsv"))
    agree = [p for h, p in listed if rows.get(p, [None] * 4)[3] != h]
    check("this mission's before-manifest of the PUBLISH-03 root agrees with every listed sha256", not agree, agree[:5])
    check("the PUBLISH-03 root holds no executable binary (no ELF; scratch included)",
          not [r for r in rows.values() if r[1] == "regular:elf"])
    zero = sorted(p for p, r in rows.items() if r[1].startswith("regular") and r[2] == "0" and not p.startswith("scratch/"))
    print(f"     PUBLISH-03 zero-length regular files outside scratch/: {zero}")
    sums = {p: h for h, p in listed}
    for rel, want in (("tools/build_packet.py", P3_BUILDER_SHA), ("documents/DUPLICATE-CONTENT-MAP.tsv", P3_MAP_SHA),
                      ("receipts/80-STOP-RECORD.md", P3_STOP_SHA)):
        check(f"PUBLISH-03 {rel}: sha256 {want[:16]}… (its root sums)", sums.get(rel) == want ==
              Q.sha256_file(os.path.join(Q.P3, rel)))
    stop = open(os.path.join(Q.P3, "receipts/80-STOP-RECORD.md")).read()
    check("PUBLISH-03 stop record: BLOCKED on control P3 and 14 unauthorized duplicates",
          stop.startswith("# Stop record: repair control P3 failed; 14 unauthorized duplicates") and "**BLOCKED**" in stop)
    rc = open(os.path.join(Q.P3, "receipts/04-REPAIR-CONTROLS.console.txt")).read()
    check("PUBLISH-03 repair controls preserved: 29 as expected, P3 not as expected",
          "controls 30; as expected 29; not as expected ['P3']" in rc and
          "REPAIR CONTROLS: NOT ALL AS EXPECTED: P3" in rc)
    p3run = open(os.path.join(Q.P3, "receipts/04-REPAIR-CONTROLS/P3-builder-run.txt")).read()
    check("PUBLISH-03 failed P3 builder run preserved (PACKET: FAILED, 14 UNAUTHORIZED_DUPLICATE)",
          "PACKET: FAILED" in p3run and p3run.count("UNAUTHORIZED_DUPLICATE") >= 14, p3run.count("UNAUTHORIZED_DUPLICATE"))
    inv = [l.split("\t") for l in open(os.path.join(Q.P3, "receipts/07-DUPLICATE-INVENTORY.tsv")).read().splitlines()[1:]]
    cls = {}
    for r in inv:
        cls[r[0]] = cls.get(r[0], 0) + 1
    check("PUBLISH-03 duplicate inventory: 7 accepted ruled, 14 unauthorized, 8 zero-length",
          cls == {"ACCEPTED_RULED": 7, "UNAUTHORIZED_DUPLICATE": 14, "ZERO_LENGTH_NOT_COMPARED": 8}, cls)
    bd = open(os.path.join(Q.P3, "receipts/03-BUILDER-DIFF.console.txt")).read()
    check("PUBLISH-03 builder diff preserved: ONLY THE DUPLICATE-CONTENT POLICY CHANGED",
          "BUILDER DIFF: ONLY THE DUPLICATE-CONTENT POLICY CHANGED" in bd)
    for rel in ("receipts/05-SOURCE-MANIFEST-COMPARE.txt",):
        check(f"PUBLISH-03 {rel}: SOURCE EVIDENCE: UNCHANGED", "SOURCE EVIDENCE: UNCHANGED" in open(
            os.path.join(Q.P3, rel)).read())
    log = open(os.path.join(Q.P3, "development/DEVELOPMENT-LOG.txt")).read()
    check("PUBLISH-03 development log preserved (13 entries, ending at the STOP)", "\n13. " in log and "Status: BLOCKED" in log)
    check("PUBLISH-03 never created a worktree or branch", not os.path.exists(Q.P3_WORKTREE) and
          subprocess.run(["git", "-C", Q.REPO, "show-ref", "--verify", "-q",
                          "refs/heads/evidence/p3-c11-d2a1-mutation-tranche1-publish-03"]).returncode == 1)


def main():
    Q.no_bytecode()
    rm = {l.split("\t")[1]: l.split("\t")[0] for l in Q.git("ls-remote", Q.REMOTE).decode().splitlines()}
    exp = json.load(open(os.path.join(Q.SRC, "receipts/02-EXPECTATIONS.json")))
    roots()

    print("[Candidate C]")
    check("C tree", Q.git("rev-parse", Q.C + "^{tree}").decode().strip() == Q.C_TREE, Q.C_TREE)
    check("C single parent", commit_parents(Q.C) == [Q.C_PARENT], Q.C_PARENT)
    for p, b in BLOBS.items():
        check(f"C {p} blob", Q.git("rev-parse", f"{Q.C}:{p}").decode().strip() == b, b)

    print("[frozen catalog and the seven relevant patches]")
    check("R2 evidence commit tree", Q.git("rev-parse", Q.R2 + "^{tree}").decode().strip() == Q.R2_TREE, Q.R2_TREE)
    check("R2 review ref on the remote", rm.get(Q.R2_REF) == Q.R2, rm.get(Q.R2_REF))
    cat_blob = Q.git("rev-parse", f"{Q.R2}:{Q.CATALOG_PATH}").decode().strip()
    cat = Q.git("cat-file", "blob", cat_blob)
    check("catalog blob", cat_blob == Q.CATALOG_BLOB, cat_blob)
    check("catalog sha256", Q.sha256_bytes(cat) == Q.CATALOG_SHA, Q.sha256_bytes(cat))
    check("the CONTINUE-02 input copy of the catalog is byte-equal",
          Q.sha256_file(os.path.join(Q.SRC, "inputs/catalog.json")) == Q.CATALOG_SHA)
    catalog = {c["id"]: c for c in json.loads(cat)["controls"]}
    for cid in Q.CANDIDATES:
        c = catalog[cid]
        pb = Q.git("rev-parse", f"{Q.R2}:{Q.R2_PACKET}/catalog/{c['patch_file']}").decode().strip()
        psha = Q.sha256_bytes(Q.git("cat-file", "blob", pb))
        check(f"{cid} patch {c['patch_file']}: git blob {pb} sha256 equals the catalog's patch_sha256", psha ==
              c["patch_sha256"], psha)
        check(f"{cid} CONTINUE-02 input patch copy byte-equal",
              Q.sha256_file(os.path.join(Q.SRC, "inputs/patches", cid + ".patch")) == c["patch_sha256"])
        e = exp["controls"][cid]
        check(f"{cid} expectations: mutant blob and patch sha256 are the catalog's",
              e["mutant_blob"] == c["mutant_blob"] and e["patch"]["sha256"] == c["patch_sha256"], e["mutant_blob"])
        r = subprocess.run(["git", "-C", Q.REPO, "cat-file", "-e", c["mutant_blob"]], capture_output=True)
        check(f"{cid} mutant blob {c['mutant_blob'][:12]} absent from the object store (never committed)",
              r.returncode != 0)

    tm_exec = packet("EXEC-01", Q.EXEC01, Q.EXEC01_TREE, Q.R2, Q.EXEC01_PACKET, Q.EXEC01_COUNT, Q.EXEC01_REF, rm)
    tm_res = packet("RESUME-01", Q.RESUME01, Q.RESUME01_TREE, Q.EXEC01, Q.RESUME01_PACKET, Q.RESUME01_COUNT,
                    Q.RESUME01_REF, rm)
    fz = json.load(open(os.path.join(Q.SRC, "receipts/05-FREEZE.json")))
    for name, tm, pre in (("EXEC-01 packet", tm_exec, Q.EXEC01_PACKET), ("RESUME-01 packet", tm_res, Q.RESUME01_PACKET)):
        rec = fz["identities"].get(name, {})
        check(f"{name}: SHA256SUMS blob equals the CONTINUE-02 freeze record's",
              rec.get("SHA256SUMS_blob") == tm[pre + "/SHA256SUMS"][2], rec.get("SHA256SUMS_blob"))
    check("the required publication parent is the RESUME-01 evidence commit", Q.PARENT == Q.RESUME01)

    print("[CONTINUE-02 freeze, gates and preserved failure]")
    check("05-FREEZE.json sha256 is the recorded freeze identity",
          Q.sha256_file(os.path.join(Q.SRC, "receipts/05-FREEZE.json")) == FREEZE_SHA, FREEZE_SHA)
    frozen = dict(fz["files"], **fz["pre_freeze_receipts"])
    bad = [p for p, h in frozen.items() if not os.path.isfile(os.path.join(Q.SRC, p))
           or Q.sha256_file(os.path.join(Q.SRC, p)) != h]
    check("every CONTINUE-02 frozen file and pre-freeze receipt is byte-equal to its freeze record", not bad,
          f"{len(fz['files'])} frozen files + {len(fz['pre_freeze_receipts'])} pre-freeze receipts; mismatched {bad}")
    for g, verdict in (("06-PRE-EXECUTION-GATE.txt", "PRE-EXECUTION GATE: ALL CHECKS HOLD"),
                       ("90-final-gate.txt", "FINAL GATE: ALL CHECKS HOLD")):
        ls = open(os.path.join(Q.SRC, "receipts", g)).read().splitlines()
        check(f"{g}: the verdict line {verdict!r} and no FAIL line (90: the second, unchanged-gate run)",
              verdict in ls and not [l for l in ls if l.startswith("FAIL")], f"{sum(l.startswith('OK') for l in ls)} OK")
    crash = open(os.path.join(Q.SRC, "receipts/90-final-gate-attempt-1-CRASHED.txt")).read()
    check("90-final-gate-attempt-1-CRASHED.txt preserved (the first final-gate run's read-only crash, FileNotFoundError)",
          "FileNotFoundError" in crash and "Traceback" in crash)
    for g in ("90-gate-authentication.txt", "90-gate-authentication-attempt-1.txt", "06-gate-authentication.txt"):
        check(f"{g}: RESULT: ALL CHECKS HOLD",
              open(os.path.join(Q.SRC, "receipts", g)).read().rstrip().endswith("RESULT: ALL CHECKS HOLD"))
    check("the frozen failed builder tools/build_packet.py is the frozen file",
          Q.sha256_file(os.path.join(Q.SRC, "tools/build_packet.py")) == FROZEN_BUILDER_SHA == fz["files"]["tools/build_packet.py"],
          FROZEN_BUILDER_SHA)
    bp = open(os.path.join(Q.SRC, "publication/10-build-packet.txt")).read().splitlines()
    check("the original duplicate failure publication/10-build-packet.txt preserved",
          bp[-1].startswith("PACKET: FAILED: no staged file of 1 KiB or more byte-equal"), bp[-1][:90].rstrip())
    check("the stop record receipts/80-STOP-RECORD.md preserved",
          (first(os.path.join(Q.SRC, "receipts/80-STOP-RECORD.md")) or "").startswith("# Stop record"))
    log = open(os.path.join(Q.SRC, "development/DEVELOPMENT-LOG.txt")).read()
    check("development log entries 22-24 (crash, gate re-run, packet failure) preserved",
          all(f"\n{n}. " in log for n in (22, 23, 24)) and "STOP: nothing committed" in log)

    print("[the seven current control receipts]")
    common = ["00-GATE.txt", "04-run/CHECKS.txt", "04-run/CLASSIFICATION.txt", "04-run/PARSED-RECEIPT.json",
              "04-run/RECEIPT.txt", "04-run/raw-output.txt", "04-run/descriptor.json", "04-run/scope-record.txt",
              "04-run/time-v.txt", "04-run/cgroup-samples.txt", "05-restore/RESTORE.json", "05-restore/RESTORE.txt",
              "06-link/LINK.json", "06-link/LINK.txt", "CONTROL-RESULT.json", "RESULT.txt"]
    built = ["01-worktree/AUTHENTICATE.json", "01-worktree/AUTHENTICATE.txt", "01-worktree/CREATE.json",
             "01-worktree/CREATE.txt", "02-apply/APPLY.json", "02-apply/APPLY.txt", "03-build/build.stderr",
             "03-build/build.stdout", "03-build/cgroup-samples.txt", "03-build/CHECKS.txt",
             "03-build/CLASSIFICATION.txt", "03-build/descriptor.json", "03-build/PARSED-RECEIPT.json",
             "03-build/PIN.json", "03-build/RECEIPT.txt", "03-build/scope-record.txt", "03-build/time-v.txt"]
    c28 = ["01-admission/ADMISSION.json", "01-admission/ADMISSION.txt", "02-pin/PIN.json", "02-pin/PIN.txt"]
    pins = {}
    for cid in Q.CANDIDATES:
        d = os.path.join(Q.SRC, exp["controls"][cid]["dir"])
        need = common + (c28 if cid == "C28" else built)
        have = sorted(os.path.relpath(os.path.join(a, f), d) for a, _, fs in os.walk(d) for f in fs)
        check(f"{cid} {exp['controls'][cid]['dir']}: exactly the expected receipt files, each a regular file",
              have == sorted(need) and all(stat.S_ISREG(os.lstat(os.path.join(d, f)).st_mode) for f in have),
              f"{len(have)} files; missing {sorted(set(need) - set(have))}; extra {sorted(set(have) - set(need))}")
        check(f"{cid} console log present", os.path.isfile(d + ".console.txt"))
        cr = json.load(open(os.path.join(d, "CONTROL-RESULT.json")))
        check(f"{cid} RESULT.txt and CONTROL-RESULT.json: {Q.CANDIDATE_RESULT}",
              first(os.path.join(d, "RESULT.txt")) == Q.CANDIDATE_RESULT and cr["control"] == cid
              and cr["status"] == Q.CANDIDATE_RESULT)
        check(f"{cid} run classification: {CANDIDATE_CLASS}", first(os.path.join(d, "04-run/CLASSIFICATION.txt")) == CANDIDATE_CLASS)
        rs = open(os.path.join(d, "05-restore/RESTORE.txt")).read().splitlines()
        check(f"{cid} restoration: RESTORE: EXACTLY_C, naming {cid}",
              rs[0] == "RESTORE: EXACTLY_C" and rs[1].startswith(f"control {cid}: "))
        lk = open(os.path.join(d, "06-link/LINK.txt")).read().splitlines()
        check(f"{cid} link: LINK: VERIFIED, naming {cid}", lk[0] == "LINK: VERIFIED" and lk[1].startswith(f"control {cid}: "))
        if cid == "C28":
            check("C28 admission: C28 BUILD: ARCHITECT-ADMISSIBLE — NARROW C28 RULING",
                  first(os.path.join(d, "01-admission/ADMISSION.txt")) == "C28 BUILD: ARCHITECT-ADMISSIBLE — NARROW C28 RULING")
            check("C28 pin: PIN: PINNED — EXISTING C28 BUILD (NOT REBUILT)",
                  first(os.path.join(d, "02-pin/PIN.txt")) == "PIN: PINNED — EXISTING C28 BUILD (NOT REBUILT)")
            pins[cid] = json.load(open(os.path.join(d, "02-pin/PIN.json")))
        else:
            check(f"{cid} apply: APPLY: EXACTLY_C_PLUS_PATCH", first(os.path.join(d, "02-apply/APPLY.txt")) == "APPLY: EXACTLY_C_PLUS_PATCH")
            check(f"{cid} build classification first line is BUILD: PINNED",
                  (first(os.path.join(d, "03-build/CLASSIFICATION.txt")) or "").startswith("BUILD: PINNED"),
                  first(os.path.join(d, "03-build/CLASSIFICATION.txt")))
            pins[cid] = json.load(open(os.path.join(d, "03-build/PIN.json")))
        check(f"{cid} pinned mutant blob is the catalog's", pins[cid]["mutant_blob"] == catalog[cid]["mutant_blob"])

    print("[retained executable identities: hashed by reading their bytes; nothing is executed]")
    wl = Q.git("worktree", "list", "--porcelain").decode().split("\n\n")
    for cid in Q.CANDIDATES:
        p = pins[cid]["path"]
        st = os.lstat(p) if os.path.lexists(p) else None
        ok = st is not None and stat.S_ISREG(st.st_mode) and os.path.realpath(p) == p
        h = Q.sha256_file(p) if ok else None
        check(f"{cid} pinned executable {p}: regular, not a symlink, sha256 and size equal the pin record",
              ok and h == pins[cid]["sha256"] and st.st_size == pins[cid]["size"], f"{h} {st.st_size if st else None}")
        wt = Q.NEXUS + "/nexus-os-p3-c11-mt1x-" + cid
        blk = [b for b in wl if b.startswith(f"worktree {wt}\n")]
        lines = blk[0].splitlines() if blk else []
        check(f"{cid} worktree {wt}: registered, HEAD C, detached, locked",
              len(blk) == 1 and f"HEAD {Q.C}" in lines and "detached" in lines
              and any(l.startswith("locked ") for l in lines), [l for l in lines if l.startswith("locked")])
    st = os.lstat(BASE_EXE)
    check("pinned unchanged-baseline executable: regular, sha256 and size",
          stat.S_ISREG(st.st_mode) and Q.sha256_file(BASE_EXE) == BASE_EXE_SHA and st.st_size == BASE_EXE_SIZE, BASE_EXE_SHA)

    print("[publication state]")
    check("destination ref absent on the remote", Q.DEST not in rm)
    check("destination ref absent locally",
          subprocess.run(["git", "-C", Q.REPO, "show-ref", "--verify", "-q", Q.DEST]).returncode == 1)
    # Absent before receipts/05-WORKTREE.txt; after it, exactly the worktree it created: at the parent, on the
    # publication branch, holding nothing but (at most) the untracked packet.
    if os.path.exists(Q.WORKTREE):
        st = Q.git("--no-optional-locks", "status", "--porcelain", "-uall", repo=Q.WORKTREE).decode().splitlines()
        ok = (Q.git("rev-parse", "HEAD", repo=Q.WORKTREE).decode().strip() == Q.PARENT
              and Q.git("symbolic-ref", "HEAD", repo=Q.WORKTREE).decode().strip() == "refs/heads/" + Q.LOCAL_BRANCH
              and all(l.startswith("?? " + Q.PACKET + "/") for l in st))
        detail = f"present (receipts/05-WORKTREE.txt): HEAD the parent, on {Q.LOCAL_BRANCH}, {len(st)} untracked files"
    else:
        ok, detail = True, "absent"
    check("this mission's worktree: absent, or exactly the one receipts/05-WORKTREE.txt created", ok, detail)
    br = subprocess.run(["git", "-C", Q.REPO, "rev-parse", "-q", "--verify", "refs/heads/" + Q.LOCAL_BRANCH],
                        capture_output=True, text=True).stdout.strip()
    check("this mission's local publication branch: absent, or at the parent (no commit yet)", br in ("", Q.PARENT),
          br or "absent")
    ow = Q.git("--no-optional-locks", "status", "--porcelain", "-uall", repo=Q.OLD_WORKTREE).decode().splitlines()
    check("the old failed evidence worktree is untouched: HEAD the parent, only the untracked failed staging",
          Q.git("rev-parse", "HEAD", repo=Q.OLD_WORKTREE).decode().strip() == Q.PARENT
          and ow and all(l.startswith("?? " + Q.PACKET + "/") for l in ow), f"{len(ow)} untracked files")
    print("RESULT: " + ("ALL CHECKS HOLD" if not fails else "FAILED: " + "; ".join(fails)))
    return 0 if not fails else 1


if __name__ == "__main__":
    sys.exit(main())

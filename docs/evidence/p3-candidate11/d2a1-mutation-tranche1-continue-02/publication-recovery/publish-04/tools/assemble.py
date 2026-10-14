#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-MUTATION-TRANCHE1-PUBLISH-04, mission section 12: assemble the clean repository packet.

    assemble.py DEST      DEST: the packet directory in this mission's publication worktree; must not exist.

1. The R2 builder (tools/build_packet.py, its CLI, unchanged) stages the CONTINUE-02 execution evidence into DEST and
   must end "PACKET: STAGED — ALL CHECKS HOLD".
2. Added beside it, byte copies only:
   - publication/10-build-packet.txt: CONTINUE-02's original packet failure (its root's publication/);
   - INDEX-PUBLICATION-RECOVERY.md: this mission's index of the publication recovery;
   - publication-recovery/publish-03/: the PUBLISH-03 root without scratch/ and without its one zero-length file
     (EXCLUDED_P3; mission section 7 permits exactly eight zero-length artifacts);
   - publication-recovery/publish-04/: this mission's root without scratch/ (and without the index, placed at the
     root), with exactly the pre-gate receipts (PRE_GATE); the post-gate records are added by finalize.py.
3. SHA256SUMS is rewritten over every file but itself.
4. full_checks() re-checks the whole packet: the builder's checks over every file (size, text only, no Rust source, no
   pinned executable, no mutant or crate source blob, no carried-forward path, the R2 duplicate and zero-length
   policy, SHA256SUMS), plus no symlink anywhere, every file byte-equal to its source, no copy of a Candidate C file
   outside docs/evidence/, no credential pattern, and exactly the declared exclusions.

Nothing under either read-only root is written; nothing is committed here.
"""
import os
import re
import shutil
import stat
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import build_packet as B  # noqa: E402
import common as K  # noqa: E402
import pub4common as Q  # noqa: E402

EXCLUDED_P3 = {"development/make-map-dev1-FAILED-sha-column-key.txt":
               "zero-length (the empty sha256): mission section 7 permits exactly eight zero-length packet artifacts; "
               "the file stays in the PUBLISH-03 root, listed in its receipts/90-ROOT-SHA256SUMS.txt and in this "
               "mission's manifests of that root"}
ROOT_DOCS = {"documents/INDEX-PUBLICATION-RECOVERY.md": "INDEX-PUBLICATION-RECOVERY.md"}
PRE_GATE = ["receipts/00-ROOT-MANIFESTS-BEFORE/", "receipts/01-AUTHENTICATION.txt", "receipts/02-MAKE-MAPS.txt",
            "receipts/03-BUILDER-DIFF/", "receipts/03-BUILDER-DIFF.console.txt", "receipts/04-REPAIR-CONTROLS/",
            "receipts/04-REPAIR-CONTROLS.console.txt", "receipts/05-WORKTREE.txt",
            "receipts/06-PUBLICATION-PREFLIGHT.txt"]
# Added by finalize.py after the final gate (each a byte copy, nothing else); then the local-only receipts written
# after the commit exists (finalize, push, readback, Actions), which can never be inside the commit they describe.
POST_GATE = ["receipts/07-ASSEMBLE.console.txt", "receipts/10-FREEZE.json", "receipts/10-FREEZE-RECORD.txt",
             "receipts/20-FINAL-GATE.txt", "receipts/21-ROOT-MANIFESTS-AFTER/continue-02.tsv",
             "receipts/21-ROOT-MANIFESTS-AFTER/publish-03.tsv"]
LOCAL_ONLY = ("receipts/30-", "receipts/4", "receipts/5")
P3_DIR, P4_DIR = "publication-recovery/publish-03", "publication-recovery/publish-04"
SECRETS = [re.compile(p) for p in (rb"ghp_[A-Za-z0-9]{20,}", rb"gho_[A-Za-z0-9]{20,}", rb"github_pat_[A-Za-z0-9_]{20,}",
                                   rb"-----BEGIN [A-Z ]*PRIVATE KEY-----", rb"AKIA[0-9A-Z]{16}", rb"xox[abprs]-[A-Za-z0-9-]{10,}",
                                   rb"sk-ant-[A-Za-z0-9_-]{10,}", rb"sk-[A-Za-z0-9]{32,}")]


def root_files(root):
    out = []
    for d, dirs, fs in os.walk(root):
        dirs[:] = sorted(x for x in dirs if not (d == root and x == "scratch"))
        out += [os.path.relpath(os.path.join(d, f), root) for f in sorted(fs)]
    return out


def sources(post=False):
    """Every (source path, packet path) added beside the builder's staging; refuses any undeclared file. post=True
    (finalize.py): the post-gate records are included and the local-only receipts skipped."""
    pairs = [(os.path.join(Q.SRC, "publication/10-build-packet.txt"), "publication/10-build-packet.txt")]
    p3 = root_files(Q.P3)
    zero = sorted(f for f in p3 if os.path.getsize(os.path.join(Q.P3, f)) == 0)
    if zero != sorted(EXCLUDED_P3):
        raise SystemExit(f"REFUSED: the PUBLISH-03 zero-length files outside scratch/ are {zero}, not the declared "
                         f"exclusion {sorted(EXCLUDED_P3)}")
    pairs += [(os.path.join(Q.P3, f), f"{P3_DIR}/{f}") for f in p3 if f not in EXCLUDED_P3]
    for f in root_files(Q.P):
        if f in ROOT_DOCS:
            pairs.append((os.path.join(Q.P, f), ROOT_DOCS[f]))
        elif f.startswith("receipts/"):
            if (post and f.startswith(LOCAL_ONLY)) or (not post and f in POST_GATE):
                continue  # local only; or a post-gate record (at assembly only the console being written exists)
            if not (any(f == g or (g.endswith("/") and f.startswith(g)) for g in PRE_GATE) or (post and f in POST_GATE)):
                raise SystemExit(f"REFUSED: undeclared receipt {f} ({'finalize' if post else 'assembly'})")
            pairs.append((os.path.join(Q.P, f), f"{P4_DIR}/{f}"))
        else:
            pairs.append((os.path.join(Q.P, f), f"{P4_DIR}/{f}"))
    return pairs


def write_sums(dest):
    allf = sorted(os.path.relpath(f, dest) for f in B.files_under(dest))
    with open(os.path.join(dest, "SHA256SUMS"), "w") as f:
        for rel in allf:
            if rel != "SHA256SUMS":
                f.write(f"{K.sha256_file(os.path.join(dest, rel))}  {rel}\n")


def full_checks(dest, pairs):
    """The builder's checks over the whole packet, and this mission's additions. Returns the failed labels."""
    fails = []

    def check(label, ok, detail=""):
        print(f"{'OK  ' if ok else 'FAIL'} {label}{(': ' + str(detail)) if detail != '' else ''}")
        if not ok:
            fails.append(label)
    links = []
    for d, dirs, fs in os.walk(dest):
        links += [os.path.relpath(os.path.join(d, n), dest) for n in dirs + fs if os.path.islink(os.path.join(d, n))]
    check("no symlink anywhere in the packet (files or directories)", not links, links[:5])
    allf = [os.path.relpath(f, dest) for f in B.files_under(dest)]
    every = sorted(os.path.relpath(os.path.join(d, f), dest) for d, _, fs in os.walk(dest) for f in fs)
    check("no bytecode or cache file in the packet", every == sorted(allf), sorted(set(every) - set(allf))[:5])
    total = sum(os.path.getsize(os.path.join(dest, f)) for f in allf)
    check("packet size at most 32 MiB", total <= K.PACKET_LIMIT, f"{total} bytes, {len(allf)} files")
    nontext = []
    blobs = {}
    for f in allf:
        b = open(os.path.join(dest, f), "rb").read()
        blobs[f] = K.git_blob_id(b)
        try:
            b.decode("utf-8")
        except UnicodeDecodeError:
            nontext.append(f)
        if b"\0" in b or b[:4] == b"\x7fELF":
            nontext.append(f)
    check("text only (UTF-8, no NUL byte, no ELF)", not nontext, nontext[:5])
    check("no Rust source file", not [f for f in allf if f.endswith(".rs")])
    exp = __import__("json").load(open(os.path.join(K.EV, "receipts/02-EXPECTATIONS.json")))
    mutants = {c["mutant_blob"] for c in exp["controls"].values()}
    pinned = {K.EXE_SHA256}
    for d, _, fs in os.walk(os.path.join(K.EV, "receipts")):
        for n in fs:
            if n == "PIN.json":
                pinned.add(__import__("json").load(open(os.path.join(d, n)))["sha256"])
    check("no file equal to the pinned baseline executable or any pinned mutant executable",
          not [f for f in allf if K.sha256_file(os.path.join(dest, f)) in pinned], len(pinned))
    check("no file equal to a mutant source blob", not [f for f, b in blobs.items() if b in mutants])
    tree_c = {}
    for e in K.git("ls-tree", "-r", "-z", K.C).split(b"\0"):
        if e:
            meta, path = e.split(b"\t", 1)
            tree_c[path.decode()] = meta.decode().split()[2]
    empty = K.git_blob_id(b"")
    crate = {b for p, b in tree_c.items() if p.startswith("crates/")} - {empty}
    check("no file equal to a crate source blob of C (the empty blob excluded)",
          not [f for f, b in blobs.items() if b in crate])
    source = {b for p, b in tree_c.items() if not p.startswith("docs/evidence/")} - {empty}
    copies = sorted(f for f, b in blobs.items() if b in source)
    check("no source copy: no file equal to any blob of Candidate C outside docs/evidence/ (the empty blob excluded)",
          not copies, copies[:5])
    targets = [f for f in allf if "/target/" in f or f.endswith((".rlib", ".rmeta", ".so", ".o", ".pyc"))]
    check("no target directory, build artifact or bytecode", not targets, targets[:3])
    secrets = sorted({f for f in allf for rx in SECRETS if rx.search(open(os.path.join(dest, f), "rb").read())})
    check("no credential pattern (GitHub, AWS, Slack, Anthropic/OpenAI-style keys, private keys)", not secrets, secrets)
    check("no carried-forward path staged", not [f for f in allf if B.carried(f)])
    accepted, accepted_zero, problems = B.dup_policy(dest, allf)
    check("R2 policy over the whole packet: no nonempty file byte-equal to a previous-packet file but exactly the 21 "
          "mapped, verified files; no zero-length file but exactly the 8 mapped, verified artifacts", not problems
          and len(accepted) == 21 and len(accepted_zero) == 8, problems[:24])
    check("every added file byte-equal to its source", all(K.sha256_file(s) == K.sha256_file(os.path.join(dest, r))
                                                          for s, r in pairs), f"{len(pairs)} added")
    p3 = [r for _, r in pairs if r.startswith(P3_DIR + "/")]
    check("exactly the declared PUBLISH-03 exclusions (scratch/ and the one zero-length file) are absent",
          sorted(r[len(P3_DIR) + 1:] for r in p3) == sorted(f for f in root_files(Q.P3) if f not in EXCLUDED_P3)
          and not [f for f in allf if f.startswith(P3_DIR + "/scratch/") or f.startswith(P4_DIR + "/scratch/")],
          f"{len(p3)} PUBLISH-03 files; excluded {sorted(EXCLUDED_P3)}")
    listed = [l.split("  ", 1) for l in open(os.path.join(dest, "SHA256SUMS")).read().splitlines()]
    check("SHA256SUMS lists every file but itself, and verifies",
          sorted(p for _, p in listed) == sorted(f for f in allf if f != "SHA256SUMS")
          and all(K.sha256_file(os.path.join(dest, p)) == h for h, p in listed), f"{len(listed)} listed")
    print(f"     mapped nonempty duplicates accepted: {len(accepted)}; mapped zero-length artifacts accepted: "
          f"{len(accepted_zero)}; packet files {len(allf)}; bytes {total}")
    return fails


def main(dest):
    Q.no_bytecode()
    if not dest.startswith(Q.WORKTREE + "/") and not dest.startswith(Q.P + "/scratch/"):
        raise SystemExit("REFUSED: the destination is neither in this mission's worktree nor in its scratch/")
    if os.path.lexists(dest):
        raise SystemExit("REFUSED: destination exists")
    pairs = sources()
    env = dict(os.environ, PYTHONDONTWRITEBYTECODE="1")
    r = subprocess.run([sys.executable, "-B", os.path.join(Q.P, "tools/build_packet.py"), dest], capture_output=True,
                       text=True, env=env)
    print("[the R2 builder, its CLI]")
    print(r.stdout + r.stderr + f"exit {r.returncode}")
    last = r.stdout.strip().splitlines()[-1] if r.stdout.strip() else ""
    if r.returncode != 0 or last != "PACKET: STAGED — ALL CHECKS HOLD":
        print("ASSEMBLY: FAILED: the builder did not stage the CONTINUE-02 evidence")
        return 1
    staged = set(os.path.relpath(f, dest) for f in B.files_under(dest))
    clash = sorted(r_ for _, r_ in pairs if r_ in staged)
    if clash:
        print(f"ASSEMBLY: FAILED: added paths collide with the builder's staging: {clash[:5]}")
        return 1
    for src, rel in pairs:
        st = os.lstat(src)
        if not stat.S_ISREG(st.st_mode) or os.path.realpath(src) != src:
            print(f"ASSEMBLY: FAILED: source {src} is not a regular file reached without a symlink")
            return 1
        os.makedirs(os.path.dirname(os.path.join(dest, rel)), exist_ok=True)
        shutil.copyfile(src, os.path.join(dest, rel))
    write_sums(dest)
    print(f"[added beside the builder's staging: {len(pairs)} files; SHA256SUMS rewritten]")
    print(f"     excluded from publication-recovery/publish-03/: scratch/ and {EXCLUDED_P3}")
    print("[full checks over the whole packet]")
    fails = full_checks(dest, pairs)
    print(f"SHA256SUMS sha256 {K.sha256_file(os.path.join(dest, 'SHA256SUMS'))}")
    print("ASSEMBLY: " + ("PACKET ASSEMBLED — ALL CHECKS HOLD" if not fails else "FAILED: " + "; ".join(fails)))
    return 0 if not fails else 1


if __name__ == "__main__":
    sys.exit(main(os.path.abspath(sys.argv[1])))

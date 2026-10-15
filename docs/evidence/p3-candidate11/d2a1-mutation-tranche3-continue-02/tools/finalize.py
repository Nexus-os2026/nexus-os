#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-MUTATION-TRANCHE1-PUBLISH-04, mission sections 11, 12, 14 and 15: add the post-gate records to
the packet, re-check it and commit it (no push). Revised by P3-C11-DR01-D2A1-MUTATION-TRANCHE2-EXEC-01 (sections 16
and 17) for this packet. Revised by P3-C11-DR01-D2A1-MUTATION-TRANCHE2-CONTINUE-02 (sections 19 and 20): this packet,
the parent (the Tranche-2 stopped evidence commit) and the commit message. Revised by
P3-C11-DR01-D2A1-MUTATION-TRANCHE3-EXEC-01 (sections 14 and 15): this packet, the parent (the accepted continuation
evidence commit), the 64 MiB limit and the commit message. Revised by
P3-C11-DR01-D2A1-MUTATION-TRANCHE3-CONTINUE-02 (section 20): this packet, the parent (the Tranche-3 stopped evidence
commit) and the commit message. Its console is publication/30-FINALIZE.txt (local only: it cannot be inside the commit it
describes).

    finalize.py

1. Refuses unless the final gate held (receipts/90-final-gate.txt), the packet build held
   (publication/10-build-packet.txt), the freeze holds, and the staged packet still verifies against its SHA256SUMS.
2. Copies the packet build console into the packet (publication/10-build-packet.txt), rewrites SHA256SUMS, and runs the
   builder's checks again over the final packet (the duplicate and zero-length policy, size, text, no source or mutant
   copy, every authored file whitespace-clean).
3. Stages exactly the packet path, verifies the index (every packet file added, nothing else), compares
   `git diff --cached --check` with the builder's whitespace rule, commits on the local evidence branch with the
   required parent, and verifies the commit (parent, every changed path added under the packet, every blob equal to
   the staged bytes, the worktree clean).
"""
import json
import os
import shutil
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import build_packet as B  # noqa: E402
import common as K  # noqa: E402
import render  # noqa: E402
import supervise  # noqa: E402

DEST = os.path.join(K.EVIDENCE_WT, K.PACKET)
CONSOLE = os.path.join(K.EV, "publication/10-build-packet.txt")
fails = []


def check(label, ok, detail=""):
    print(f"{'OK  ' if ok else 'FAIL'} {label}{(': ' + str(detail)) if detail != '' else ''}")
    if not ok:
        fails.append(label)


def wt(*args, data=None):
    return subprocess.run(["git", "-C", K.EVIDENCE_WT, *args], capture_output=True, input=data)


def message(exp):
    st, results = render.status(exp)
    rows = [(c, r[0] if r else None) for c, r in zip(K.ORDER, results)]
    attempted = [c for c, r in rows if r]
    cand = [c for c, r in rows if r == render.RESULT_OK]
    stopped = [c for c, r in rows if r and r != render.RESULT_OK]
    rest = [c for c in K.ORDER if c not in attempted]
    head = (f"stopped at control {stopped[0]}" if stopped else f"{len(cand)} candidates for review" if cand else
            "no control attempted")
    return (f"docs(evidence): record D2A-1 mutation tranche 3 continuation evidence ({head})\n\n"
            f"Mission {K.MISSION}. Evidence only.\n"
            "No source, catalog, patch, contract or manifest changed; no mutant\n"
            "source, executable, target directory or source copy committed; every\n"
            "previous packet unchanged and referenced, not duplicated. C24 was not\n"
            "rebuilt: its preserved Tranche-3 build was admitted under the narrow\n"
            "C24 ruling. C3b and C3c were built under the build rule of\n"
            "AMENDMENT-01. The 23 accepted controls and the unchanged-core\n"
            "configurations were not rerun; the restored sides are linked. C7's\n"
            "run without A4 on F-3 is informational and is not a kill receipt.\n"
            "C1, C4 and C25 were not executed.\n\n"
            f"- derived status: {st}\n"
            f"- attempted, in order: {' '.join(attempted) or 'none'}\n"
            f"- KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING: {' '.join(cand) or 'none'}\n"
            f"- stopped (first unsuccessful control): {' '.join(stopped) or 'none'}\n"
            f"- authorized but not attempted: {' '.join(rest) or 'none'}\n"
            "- Architect-accepted before this mission: 23 of 35 (unchanged)\n\n"
            "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\n"
            "Claude-Session: https://claude.ai/code/session_01UDVE1KW5mJWzzf64UhtW5F\n")


def main():
    gate = open(os.path.join(K.EV, "receipts/90-final-gate.txt")).read().rstrip().splitlines()
    con = open(CONSOLE).read().rstrip().splitlines() if os.path.exists(CONSOLE) else []
    rec = []
    fz = supervise.freeze_check(rec)
    check("the final gate held", gate[-1:] == ["FINAL GATE: ALL CHECKS HOLD"], gate[-1:])
    check("the packet build held (publication/10-build-packet.txt)", con[-1:] == ["PACKET: STAGED — ALL CHECKS HOLD"],
          con[-1:])
    check("the freeze holds", fz is None, rec[0] if rec else fz)
    listed = [l.split("  ", 1) for l in open(os.path.join(DEST, "SHA256SUMS")).read().splitlines()]
    files = sorted(os.path.relpath(f, DEST) for f in B.files_under(DEST))
    check("the staged packet still verifies against its SHA256SUMS (nothing changed since staging)",
          sorted(p for _, p in listed) == [f for f in files if f != "SHA256SUMS"]
          and all(K.sha256_file(os.path.join(DEST, p)) == h for h, p in listed), f"{len(listed)} listed")
    check("publication/ is absent from the staged packet", not [f for f in files if f.startswith("publication/")])
    if fails:
        print("FINALIZE: REFUSED: " + "; ".join(fails))
        return 1
    print("[post-gate record]")
    os.makedirs(os.path.join(DEST, "publication"))
    shutil.copyfile(CONSOLE, os.path.join(DEST, "publication/10-build-packet.txt"))
    files = sorted(os.path.relpath(f, DEST) for f in B.files_under(DEST) if os.path.relpath(f, DEST) != "SHA256SUMS")
    with open(os.path.join(DEST, "SHA256SUMS"), "w") as f:
        for rel in files:
            f.write(f"{K.sha256_file(os.path.join(DEST, rel))}  {rel}\n")
    allf = files + ["SHA256SUMS"]
    print("[the builder's checks again, over the final packet]")
    rows, problems = B.dup_policy(DEST, sorted(allf))
    check("duplicate and zero-length policy over the final packet", not problems, problems[:8])
    total = sum(os.path.getsize(os.path.join(DEST, f)) for f in allf)
    check(f"packet size at most {K.PACKET_LIMIT >> 20} MiB", total <= K.PACKET_LIMIT, f"{total} bytes, {len(allf)} files")
    exp = supervise.expectations()
    cat = json.loads(open(K.inputs("catalog.json"), "rb").read())
    mut = {c["mutant_blob"] for c in cat["controls"] if c.get("mutant_blob")}
    blobs = {f: K.git_blob_id(open(os.path.join(DEST, f), "rb").read()) for f in allf}
    check("no mutant source", not [f for f, b in blobs.items() if b in mut])
    ws = {f: B.whitespace(open(os.path.join(DEST, f), "rb").read()) for f in allf}
    check("every authored file whitespace-clean", not [f for f, w in ws.items() if w and B.authored(f)])
    if fails:
        print("FINALIZE: FAILED before staging: " + "; ".join(fails))
        return 1
    print("[stage and commit]")
    r = wt("add", "--", K.PACKET)
    check("git add -- <packet path>", r.returncode == 0, r.stderr.decode().strip())
    names = wt("diff", "--cached", "--name-status", "--no-renames").stdout.decode().splitlines()
    check("the index adds exactly every packet file and nothing else",
          sorted(names) == sorted(f"A\t{K.PACKET}{f}" for f in allf), f"{len(names)} staged; {len(allf)} files")
    chk = wt("diff", "--cached", "--check").stdout.decode().splitlines()
    git_ws = sorted({(l.split(":", 2)[0], int(l.split(":", 2)[1])) for l in chk if l.startswith(K.PACKET)
                     and l.split(":", 2)[1].isdigit()})
    mine = sorted({(f"{K.PACKET}{f}", n) for f, w in ws.items() for n, _ in w})
    check("git diff --cached --check reports exactly the (file, line) findings of the builder's whitespace rule",
          git_ws == mine, f"git {len(git_ws)}; rule {len(mine)}; differing {sorted(set(git_ws) ^ set(mine))[:5]}")
    if fails:
        print("FINALIZE: FAILED before the commit: " + "; ".join(fails))
        return 1
    msg = message(exp)
    print(msg)
    r = subprocess.run(["git", "-C", K.EVIDENCE_WT, "commit", "-q", "-F", "-"], input=msg.encode(), capture_output=True)
    check("git commit", r.returncode == 0, (r.stdout + r.stderr).decode().strip())
    head = K.git("rev-parse", "HEAD", repo=K.EVIDENCE_WT).decode().strip()
    info = K.git("cat-file", "commit", head, repo=K.EVIDENCE_WT).decode()
    parents = [l[7:] for l in info.partition("\n\n")[0].splitlines() if l.startswith("parent ")]
    check("the evidence commit has exactly the required parent", parents == [K.PARENT], parents)
    check("the local evidence branch points at the evidence commit", K.git(
        "rev-parse", "refs/heads/" + K.LOCAL_BRANCH).decode().strip() == head)
    diff = K.git("diff-tree", "-r", "--no-renames", "--name-status", K.PARENT, head).decode().splitlines()
    check("every changed path is added under the packet path; nothing else differs from the parent",
          sorted(diff) == sorted(f"A\t{K.PACKET}{f}" for f in allf), f"{len(diff)} paths")
    bad = [f for f in allf if K.git("rev-parse", f"{head}:{K.PACKET}{f}").decode().strip() != blobs.get(f, K.git_blob_id(
        open(os.path.join(DEST, f), "rb").read()))]
    check("every committed blob equals the staged bytes", not bad, bad[:3])
    check("the worktree is clean after the commit", wt("status", "--porcelain", "-uall").stdout == b"")
    tree = K.git("rev-parse", head + "^{tree}").decode().strip()
    print(f"evidence commit {head}; tree {tree}; parent {parents}; files {len(allf)}; packet SHA256SUMS sha256 "
          f"{K.sha256_file(os.path.join(DEST, 'SHA256SUMS'))}")
    print(f"FINALIZE: {'COMMITTED ' + head if not fails else 'FAILED: ' + '; '.join(fails)}")
    return 0 if not fails else 1


if __name__ == "__main__":
    sys.exit(main())

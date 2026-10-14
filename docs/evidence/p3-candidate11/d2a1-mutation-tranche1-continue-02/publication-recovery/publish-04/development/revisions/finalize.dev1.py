#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-MUTATION-TRANCHE1-PUBLISH-04, mission sections 11, 12, 14 and 15: after the final gate held, add the
post-gate records to the packet, re-check it, and commit it on the publication branch (no push). Its console is
receipts/30-FINALIZE.txt (local only: it cannot be inside the commit it describes).

    finalize.py

1. Refuses unless the final gate ended FINAL GATE: ALL CHECKS HOLD (exit 0), every frozen file of this mission's root
   is unchanged, the staged packet still equals the frozen packet, and nothing was added to this root but the freeze,
   the gate's outputs and this console.
2. Copies exactly the post-gate records (assemble.POST_GATE) into publication-recovery/publish-04/receipts/, rewrites
   SHA256SUMS, and verifies the delta: the final packet is the frozen packet plus those copies, every frozen packet
   file unchanged but SHA256SUMS.
3. Re-runs assemble.full_checks() over the final packet (the R2 policy among them) and the whitespace rule over the
   post-gate records (this mission's: clean, but the assembly console, raw builder output).
4. Stages exactly the packet path (`git add -- <packet>`), verifies the index (every packet file added, nothing else),
   compares `git diff --cached --check` with the gate's own whitespace rule, commits with the required parent, and
   verifies the commit (parent, every changed path added under the packet, every blob equal to the staged bytes).
"""
import os
import shutil
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import assemble as A  # noqa: E402
import build_packet as B  # noqa: E402
import final_gate as G  # noqa: E402
import pub4common as Q  # noqa: E402

DEST = os.path.join(Q.WORKTREE, Q.PACKET)
MESSAGE = """docs(evidence): publish D2A-1 tranche 1 continue-02 evidence (duplicate policy R2)

Missions P3-C11-DR01-D2A1-MUTATION-TRANCHE1-CONTINUE-02 (execution evidence),
-PUBLISH-03 (BLOCKED publication recovery, ruling R1) and -PUBLISH-04
(publication recovery, Architect duplicate-policy ruling R2). Evidence only.
No source, catalog, patch, contract or manifest changed; no mutant source,
executable or target directory committed; the previous packets (19eda8cc,
3695a817) are unchanged and not duplicated. Nothing was rebuilt or rerun.

- the complete CONTINUE-02 execution evidence for C28, C12, C9, C11, C17,
  C18a, C18b, byte for byte, with its preserved failures (the first final
  gate's crash and its unchanged re-run, the frozen builder and its
  duplicate failure, the stop record)
- ruling R1 (seven link receipts) and the PUBLISH-03 record of its blocked
  repair (control P3: 14 further constant records)
- ruling R2: no size threshold; exactly 21 mapped nonempty duplicates (7
  link receipts, 10 sampler records, 4 self-test outputs) and exactly 8
  mapped zero-length artifacts, each verified field by field; every other
  duplicate or empty file rejected
- R2 builder: only the duplicate and zero-length policy changed (three
  exact diffs); repair controls P1-P4 and N1-N26 as expected; both read-only
  roots unchanged; final publication gate: ALL CHECKS HOLD
- C28, C12, C9, C11, C17, C18a, C18b: KILL_EVIDENCE_CANDIDATE — ARCHITECT
  REVIEW PENDING (none accepted here)

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01UDVE1KW5mJWzzf64UhtW5F
"""
fails = []


def check(label, ok, detail=""):
    print(f"{'OK  ' if ok else 'FAIL'} {label}{(': ' + str(detail)) if detail != '' else ''}")
    if not ok:
        fails.append(label)


def wt(*args):
    return subprocess.run(["git", "-C", Q.WORKTREE, *args], capture_output=True)


def main():
    Q.no_bytecode()
    import json
    gate = open(os.path.join(Q.P, "receipts/20-FINAL-GATE.txt")).read().splitlines()
    if gate[-2:] != ["FINAL GATE: ALL CHECKS HOLD", "exit 0"]:
        print(f"FINALIZE: REFUSED: the final gate did not hold: {gate[-2:]}")
        return 1
    fz = json.load(open(os.path.join(Q.P, "receipts/10-FREEZE.json")))
    now = {f: Q.sha256_file(os.path.join(Q.P, f)) for f in A.root_files(Q.P)}
    changed = sorted(f for f, h in fz["root_files"].items() if now.get(f) != h)
    added = sorted(set(now) - set(fz["root_files"]) - set(A.POST_GATE) - {"receipts/30-FINALIZE.txt"})
    check("every frozen file of this mission's root unchanged since the freeze", not changed, changed)
    check("nothing added to this mission's root since the freeze but the post-gate records and this console",
          not added and set(A.POST_GATE) - {"receipts/07-ASSEMBLE.console.txt"} <= set(now), added)
    packet = {os.path.relpath(f, DEST): Q.sha256_file(f) for f in B.files_under(DEST)}
    check("the staged packet still equals the frozen packet", packet == fz["packet_files"], len(packet))
    if fails:
        print("FINALIZE: REFUSED: " + "; ".join(fails))
        return 1

    print("[post-gate records]")
    post = [(os.path.join(Q.P, f), f"{A.P4_DIR}/{f}") for f in A.POST_GATE]
    for src, rel in post:
        if os.path.lexists(os.path.join(DEST, rel)) or os.path.islink(src) or not os.path.isfile(src):
            print(f"FINALIZE: REFUSED: {rel} exists in the packet, or its source is not a regular file")
            return 1
        os.makedirs(os.path.dirname(os.path.join(DEST, rel)), exist_ok=True)
        shutil.copyfile(src, os.path.join(DEST, rel))
        print(f"     added {rel} sha256 {Q.sha256_file(os.path.join(DEST, rel))}")
    A.write_sums(DEST)
    final = {os.path.relpath(f, DEST): Q.sha256_file(f) for f in B.files_under(DEST)}
    expect = dict(fz["packet_files"], **{rel: Q.sha256_file(src) for src, rel in post})
    del expect["SHA256SUMS"]
    del final["SHA256SUMS"]
    check("the final packet is exactly the frozen packet plus the post-gate records (every frozen file unchanged but "
          "SHA256SUMS)", final == expect, f"{len(final) + 1} files")

    print("[full checks over the final packet]")
    pairs = [(s, r) for s, r in A.sources(post=True)]
    for label in A.full_checks(DEST, pairs):
        fails.append("full check: " + label)
    for src, rel in post:
        ws = G.whitespace(open(os.path.join(DEST, rel), "rb").read())
        if G.authored(rel):
            check(f"post-gate record {rel} is whitespace-clean (authored)", not ws, ws[:5])
        else:
            print(f"     post-gate raw record {rel}: retained findings {ws}")
    if fails:
        print("FINALIZE: FAILED before staging: " + "; ".join(fails))
        return 1

    print("[stage and commit]")
    r = wt("add", "--", Q.PACKET)
    check("git add -- <packet path>", r.returncode == 0, r.stderr.decode().strip())
    names = wt("diff", "--cached", "--name-status", "--no-renames").stdout.decode().splitlines()
    files = sorted(f for f in final) + ["SHA256SUMS"]
    check("the index adds exactly every packet file and nothing else",
          sorted(names) == sorted(f"A\t{Q.PACKET}/{f}" for f in files), f"{len(names)} staged; {len(files)} files")
    chk = wt("diff", "--cached", "--check").stdout.decode().splitlines()
    git_ws = sorted({(l.split(":", 2)[0], int(l.split(":", 2)[1])) for l in chk if l.startswith(Q.PACKET + "/")
                     and l.split(":", 2)[1].isdigit()})
    mine = sorted({(f"{Q.PACKET}/{f}", n) for f in files for n, _ in G.whitespace(open(os.path.join(DEST, f), "rb").read())})
    check("git diff --cached --check reports exactly the (file, line) findings of the gate's whitespace rule",
          git_ws == mine, f"git {len(git_ws)}; rule {len(mine)}; differing "
                          f"{sorted(set(git_ws) ^ set(mine))[:5]}")
    authored_hits = sorted({p for p, _ in git_ws if G.authored(p[len(Q.PACKET) + 1:])})
    check("no whitespace finding in any file this mission authored", not authored_hits, authored_hits)
    print(f"     git diff --cached --check: {len(chk)} output lines; findings in {len({p for p, _ in git_ws})} retained "
          "raw or preserved files (listed in receipts/20-FINAL-GATE.txt and the post-gate lines above)")
    if fails:
        print("FINALIZE: FAILED before the commit: " + "; ".join(fails))
        return 1
    r = subprocess.run(["git", "-C", Q.WORKTREE, "commit", "-q", "-F", "-"], input=MESSAGE.encode(),
                       capture_output=True)
    check("git commit", r.returncode == 0, (r.stdout + r.stderr).decode().strip())
    head = Q.git("rev-parse", "HEAD", repo=Q.WORKTREE).decode().strip()
    info = Q.git("cat-file", "commit", head).decode()
    parents = [l[7:] for l in info.partition("\n\n")[0].splitlines() if l.startswith("parent ")]
    check("the evidence commit has exactly the required parent", parents == [Q.PARENT], parents)
    check("the publication branch points at the evidence commit", Q.git(
        "rev-parse", "refs/heads/" + Q.LOCAL_BRANCH).decode().strip() == head)
    diff = Q.git("diff-tree", "-r", "--no-renames", "--name-status", Q.PARENT, head).decode().splitlines()
    check("every changed path is added under the packet path; nothing else differs from the parent",
          sorted(diff) == sorted(f"A\t{Q.PACKET}/{f}" for f in files), f"{len(diff)} paths")
    bad = [f for f in files if Q.git("rev-parse", f"{head}:{Q.PACKET}/{f}").decode().strip() !=
           Q.git_blob_id(open(os.path.join(DEST, f), "rb").read())]
    check("every committed blob equals the staged bytes", not bad, bad[:3])
    check("the worktree is clean after the commit", wt("status", "--porcelain", "-uall").stdout == b"")
    tree = Q.git("rev-parse", head + "^{tree}").decode().strip()
    print(f"evidence commit {head}; tree {tree}; parent {parents}; files {len(files)}; packet SHA256SUMS sha256 "
          f"{Q.sha256_file(os.path.join(DEST, 'SHA256SUMS'))}")
    print(f"FINALIZE: {'COMMITTED ' + head if not fails else 'FAILED: ' + '; '.join(fails)}")
    return 0 if not fails else 1


if __name__ == "__main__":
    sys.exit(main())

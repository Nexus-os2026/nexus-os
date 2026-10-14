#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-MUTATION-TRANCHE1-PUBLISH-04, mission section 11: the freeze, after the packet is assembled and
before the one static final publication gate.

    record_freeze.py                  writes receipts/10-FREEZE.json and receipts/10-FREEZE-RECORD.txt (neither may
                                      exist)
    record_freeze.py --rehearse DIR   the same into DIR/receipts/ for the packet DIR/packet (DIR under this mission's
                                      scratch/; the worktree check does not apply); nothing is written to the root

Frozen: every file of this mission's root outside scratch/ (tools, documents, receipts 00-07, development), by sha256
(this includes both maps, the ruling record, the builder, the controls, both before-manifests and the publication
tooling); every file of the staged packet, by sha256, and its SHA256SUMS; the repository identities. Refuses unless the
assembly receipt ends ASSEMBLY: PACKET ASSEMBLED — ALL CHECKS HOLD (exit 0) and the worktree holds nothing but the
untracked packet.
"""
import json
import os
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import assemble as A  # noqa: E402
import build_packet as B  # noqa: E402
import pub4common as Q  # noqa: E402

STATEMENT = ("Frozen before the final publication gate. No frozen file may change after this record. The final gate "
             "runs once; a gate failure stops the mission; no repair and no rerun after the freeze. The post-gate "
             "records (assemble.POST_GATE) are added to the packet by finalize.py only, as byte copies, with every "
             "frozen packet file unchanged but SHA256SUMS.")


def main(rehearse=None):
    Q.no_bytecode()
    if rehearse and not rehearse.startswith(Q.P + "/scratch/"):
        raise SystemExit("REFUSED: a rehearsal lives under this mission's scratch/")
    rdir = os.path.join(rehearse, "receipts") if rehearse else os.path.join(Q.P, "receipts")
    out_json, out_txt = (os.path.join(rdir, n) for n in ("10-FREEZE.json", "10-FREEZE-RECORD.txt"))
    if os.path.exists(out_json) or os.path.exists(out_txt):
        raise SystemExit("REFUSED: a freeze record exists")
    asm = open(os.path.join(rdir, "07-ASSEMBLE.console.txt")).read().splitlines()
    if asm[-2:] != ["ASSEMBLY: PACKET ASSEMBLED — ALL CHECKS HOLD", "exit 0"]:
        raise SystemExit(f"REFUSED: the assembly receipt does not end in a held assembly: {asm[-2:]}")
    if rehearse:
        print("REHEARSAL: the packet is in scratch; the worktree check does not apply")
    else:
        status = Q.git("status", "--porcelain", "-uall", repo=Q.WORKTREE).decode().splitlines()
        if not status or [l for l in status if not l.startswith("?? " + Q.PACKET + "/")]:
            raise SystemExit("REFUSED: the worktree holds something other than the untracked packet")
    root = {f: Q.sha256_file(os.path.join(Q.P, f)) for f in A.root_files(Q.P)}
    dest = os.path.join(rehearse, "packet") if rehearse else os.path.join(Q.WORKTREE, Q.PACKET)
    packet = {os.path.relpath(f, dest): Q.sha256_file(f) for f in B.files_under(dest)}
    ids = {"C": Q.C, "C_tree": Q.git("rev-parse", Q.C + "^{tree}").decode().strip(),
           "catalog_blob": Q.git("rev-parse", f"{Q.R2}:{Q.CATALOG_PATH}").decode().strip(),
           "EXEC01": Q.EXEC01, "EXEC01_tree": Q.git("rev-parse", Q.EXEC01 + "^{tree}").decode().strip(),
           "RESUME01": Q.RESUME01, "RESUME01_tree": Q.git("rev-parse", Q.RESUME01 + "^{tree}").decode().strip(),
           "parent": Q.PARENT, "worktree": Q.WORKTREE,
           "worktree_HEAD": Q.git("rev-parse", "HEAD", repo=Q.WORKTREE).decode().strip(),
           "local_branch": Q.LOCAL_BRANCH, "packet": Q.PACKET, "destination": Q.DEST}
    rec = {"mission": Q.MISSION, "recorded": time.strftime("%Y-%m-%dT%H:%M:%S%z"), "statement": STATEMENT,
           "identities": ids, "root_files": root, "packet_files": packet,
           "packet_SHA256SUMS_sha256": packet["SHA256SUMS"],
           "maps": {"nonempty": Q.sha256_file(B.MAP), "zero_length": Q.sha256_file(B.ZERO_MAP)},
           "post_gate": A.POST_GATE}
    json.dump(rec, open(out_json, "w"), indent=1, sort_keys=True)
    open(out_json, "a").write("\n")
    sha = Q.sha256_file(out_json)
    lines = ["FREEZE: RECORDED", f"mission {Q.MISSION}; recorded {rec['recorded']}",
             f"freeze record receipts/10-FREEZE.json sha256 {sha}",
             f"frozen root files (outside scratch/): {len(root)}; frozen packet files: {len(packet)}; packet SHA256SUMS "
             f"sha256 {packet['SHA256SUMS']}",
             f"maps: nonempty {rec['maps']['nonempty']}; zero-length {rec['maps']['zero_length']}",
             f"builder tools/build_packet.py sha256 {root['tools/build_packet.py']}",
             f"worktree {Q.WORKTREE} HEAD {ids['worktree_HEAD']} on {Q.LOCAL_BRANCH}", STATEMENT]
    open(out_txt, "w").write("\n".join(lines) + "\n")
    print("\n".join(lines))
    return 0


if __name__ == "__main__":
    sys.exit(main(os.path.abspath(sys.argv[2]) if sys.argv[1:2] == ["--rehearse"] else None))

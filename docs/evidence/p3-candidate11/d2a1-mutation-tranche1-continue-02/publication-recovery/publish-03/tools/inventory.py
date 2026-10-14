#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-MUTATION-TRANCHE1-PUBLISH-03, stop evidence: the exact inventory of every staged file that the
repaired duplicate-content policy rejects (beyond the seven ruled link receipts), and of the zero-length files it does
not compare, read from the control-3 staging of the official repair-control run (scratch; read-only here).

    inventory.py OUT.tsv       one row per file: class, path, bytes, sha256, git blob, content (escaped), the number of
                               previous-packet files with the same bytes, and the first three of them
"""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import build_packet as B  # noqa: E402
import pubcommon as Q  # noqa: E402

BASE = os.path.join(Q.P, "scratch/controls-04-REPAIR-CONTROLS/base")


def main(out):
    if os.path.exists(out):
        raise SystemExit("REFUSED: output exists")
    by_blob = {}
    for commit, packet in ((Q.EXEC01, Q.EXEC01_PACKET), (Q.RESUME01, Q.RESUME01_PACKET)):
        for e in Q.git("ls-tree", "-r", "-z", commit, packet + "/").split(b"\0"):
            if e:
                meta, path = e.split(b"\t", 1)
                by_blob.setdefault(meta.decode().split()[2], []).append(path.decode())
    allf = [os.path.relpath(f, BASE) for f in B.files_under(BASE)]
    import contextlib
    import io
    with contextlib.redirect_stdout(io.StringIO()):
        accepted, problems = B.dup_policy(BASE, allf)
    rejected = sorted(p for c, p, _ in problems if c == "UNAUTHORIZED_DUPLICATE")
    others = sorted({c for c, _, _ in problems} - {"UNAUTHORIZED_DUPLICATE"})
    rows = []
    for f in sorted(allf):
        data = open(os.path.join(BASE, f), "rb").read()
        cls = ("ACCEPTED_RULED" if f in accepted else "UNAUTHORIZED_DUPLICATE" if f in rejected
               else "ZERO_LENGTH_NOT_COMPARED" if not data else None)
        if not cls:
            continue
        b = Q.git_blob_id(data)
        if Q.sha256_bytes(data) != Q.sha256_file(os.path.join(Q.SRC, f)):
            raise SystemExit(f"REFUSED: staged {f} differs from the CONTINUE-02 source")
        prior = sorted(by_blob.get(b, []))
        rows.append([cls, f, str(len(data)), Q.sha256_bytes(data), b,
                     data.decode("utf-8").replace("\\", "\\\\").replace("\n", "\\n").replace("\t", "\\t")
                     if len(data) < 200 else "(link receipt; see DUPLICATE-CONTENT-MAP.tsv)", str(len(prior)),
                     " ".join(p[len("docs/evidence/p3-candidate11/"):] for p in prior[:3])])
    with open(out, "w") as fh:
        fh.write("\t".join(["class", "path", "bytes", "sha256", "git_blob", "content", "previous_packet_equal_files",
                            "first_three"]) + "\n")
        for r in rows:
            fh.write("\t".join(r) + "\n")
    count = {}
    for r in rows:
        count[r[0]] = count.get(r[0], 0) + 1
    print(f"staging {BASE}: {len(allf)} files; classes {count}; other problem codes {others}")
    for r in rows:
        if r[0] != "ACCEPTED_RULED":
            print(f"  {r[0]:<25} {r[2]:>4} B  {r[1]}  prior-equal {r[6]}  {r[5][:60]}")
    print(f"inventory {out} sha256 {Q.sha256_file(out)}")
    return 0 if not others and count.get("ACCEPTED_RULED") == 7 else 1


if __name__ == "__main__":
    sys.exit(main(os.path.abspath(sys.argv[1])))

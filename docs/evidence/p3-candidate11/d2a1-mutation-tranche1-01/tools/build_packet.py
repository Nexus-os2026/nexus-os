#!/usr/bin/env python3
"""Stages the blocker packet and the R5 adoption record into the evidence worktree (mission 17, 19).

    build_packet.py ROOT WORKTREE

Writes exactly two paths under WORKTREE/docs/evidence/p3-candidate11/: ad1r5-adoption/OWNER-ADOPTION-2026-10-14.md
and d2a1-mutation-tranche1-01/** (with a self-excluding SHA256SUMS). Refuses: an existing destination, a source file
(`.rs`, or bytes equal to any crate source blob at C or to any mutant blob of the catalog), anything binary, `.pyc`,
and any file not in the tools record. Never writes ROOT/inputs (it holds byte copies of C's sources) or the
receipts of publication, which are made after the commit.
"""
import hashlib
import json
import os
import subprocess
import sys

REPO = "/home/nexus/NEXUS/nexus-os"
C = "d5964ee5326c2733177307db6f507328023d361a"
EV = "docs/evidence/p3-candidate11/"
PACKET = EV + "d2a1-mutation-tranche1-01"
ADOPTION = EV + "ad1r5-adoption/OWNER-ADOPTION-2026-10-14.md"
LIMIT = 24 * 1024 * 1024


def sha(b):
    return hashlib.sha256(b).hexdigest()


def blob(b):
    return subprocess.run(["git", "hash-object", "--stdin"], input=b, capture_output=True,
                          check=True).stdout.decode().strip()


def walk(base):
    for b, dirs, names in os.walk(base):
        dirs[:] = sorted(d for d in dirs if d != "__pycache__")
        for n in sorted(names):
            yield os.path.join(b, n)


def main(root, wt):
    tools = json.load(open(os.path.join(root, "receipts", "50-TOOLS-RECORD.json")))["files"]
    cat = json.load(open(os.path.join(root, "out", "catalog", "catalog.json")))
    refused_blobs = {l.split()[2] for l in subprocess.run(["git", "-C", REPO, "ls-tree", "-r", C, "crates"],
                                                          capture_output=True, text=True, check=True).stdout.splitlines()}
    refused_blobs |= {e["mutant_blob"] for e in cat["controls"] if e.get("mutant_blob")}
    dest = os.path.join(wt, PACKET)
    if os.path.exists(dest) or os.path.exists(os.path.join(wt, ADOPTION)):
        raise SystemExit("REFUSED: destination exists")
    plan = [(os.path.join(root, "docs-src", "ad1r5-adoption", "OWNER-ADOPTION-2026-10-14.md"), ADOPTION, True),
            (os.path.join(root, "docs-src", "INDEX.md"), PACKET + "/INDEX.md", True),
            (os.path.join(root, "docs-src", "catalog-README.md"), PACKET + "/catalog/README.md", True)]
    for name in ("CONTROL-CATALOG-BLOCKER.md", "TRANCHE-SUMMARY.md"):
        plan.append((os.path.join(root, "out", "packet", name), f"{PACKET}/{name}", False))
    for p in walk(os.path.join(root, "out", "catalog")):
        plan.append((p, f"{PACKET}/catalog/{os.path.relpath(p, os.path.join(root, 'out', 'catalog'))}", False))
    for name in ("00-authentication.txt", "20-static-gate.txt", "21-gate-controls.txt", "50-TOOLS-RECORD.json",
                 "60-final-gate.txt", "60-gate-authentication.txt"):
        plan.append((os.path.join(root, "receipts", name), f"{PACKET}/receipts/{name}", False))
    for d in ("development", "tools"):
        for p in walk(os.path.join(root, d)):
            plan.append((p, f"{PACKET}/{d}/{os.path.relpath(p, os.path.join(root, d))}", True))
    for src, rel, recorded in plan:
        data = open(src, "rb").read()
        if rel.endswith((".rs", ".pyc")) or b"\0" in data or data[:4] == b"\x7fELF":
            raise SystemExit(f"REFUSED: {rel} is not a text evidence file")
        if blob(data) in refused_blobs:
            raise SystemExit(f"REFUSED: {rel} equals a crate source or a mutant source")
        if recorded and tools.get(os.path.relpath(src, root)) != sha(data):
            raise SystemExit(f"REFUSED: {rel} differs from the tools record")
        out = os.path.join(wt, rel)
        os.makedirs(os.path.dirname(out), exist_ok=True)
        with open(out, "wb") as fh:
            fh.write(data)
        os.chmod(out, 0o644)
    entries = sorted((sha(open(p, "rb").read()), os.path.relpath(p, dest)) for p in walk(dest))
    entries.sort(key=lambda e: e[1])
    with open(os.path.join(dest, "SHA256SUMS"), "w") as fh:
        for h, rel in entries:
            fh.write(f"{h}  {rel}\n")
    os.chmod(os.path.join(dest, "SHA256SUMS"), 0o644)
    total = sum(os.path.getsize(p) for p in walk(dest)) + os.path.getsize(os.path.join(wt, ADOPTION))
    print(f"STAGED: {ADOPTION} and {PACKET}/ ({len(entries) + 1} files, {len(entries)} listed in the self-excluding "
          f"SHA256SUMS); {total} bytes in all; limit {LIMIT}: {'within' if total <= LIMIT else 'EXCEEDED'}")
    return 0 if total <= LIMIT else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1], sys.argv[2]))

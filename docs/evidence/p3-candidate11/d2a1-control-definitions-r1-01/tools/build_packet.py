#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-CONTROL-DEFINITIONS-R1-01, mission sections 12 and 13: stage the packet into the evidence
worktree.

    build_packet.py ROOT WORKTREE

Writes exactly one directory, WORKTREE/docs/evidence/p3-candidate11/d2a1-control-definitions-r1-01/**, with a
self-excluding SHA256SUMS. Refuses: an existing destination; any file that differs from the freeze record (except the
freeze record and the final gate's receipts); a Rust source (`.rs`, or bytes equal to any crate blob at C or to any
catalogued mutant); anything binary, `.pyc` or executable; a total above 4 MiB. Never writes ROOT/inputs or
ROOT/dev-local (they hold byte copies of C's sources).
"""
import hashlib
import json
import os
import subprocess
import sys

REPO = "/home/nexus/NEXUS/nexus-os"
C = "d5964ee5326c2733177307db6f507328023d361a"
PACKET = "docs/evidence/p3-candidate11/d2a1-control-definitions-r1-01"
LIMIT = 4 * 1024 * 1024


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
    fr = json.load(open(os.path.join(root, "receipts", "50-FREEZE-RECORD.json")))["files"]
    cat = json.load(open(os.path.join(root, "out", "catalog", "catalog.json")))
    refused = {l.split()[2] for l in subprocess.run(["git", "-C", REPO, "ls-tree", "-r", C, "crates"],
                                                    capture_output=True, text=True, check=True).stdout.splitlines()}
    refused |= {e["mutant_blob"] for e in cat["controls"] if e.get("mutant_blob")}
    dest = os.path.join(wt, PACKET)
    if os.path.exists(dest):
        raise SystemExit("REFUSED: destination exists")
    plan = [("docs-src/INDEX.md", "INDEX.md"),
            ("out/docs/ARCHITECT-CONTROL-DEFINITIONS-R1.md", "ARCHITECT-CONTROL-DEFINITIONS-R1.md"),
            ("out/docs/C22-RESTORED-BUILD-RULING.md", "C22-RESTORED-BUILD-RULING.md"),
            ("out/docs/catalog-README.md", "catalog/README.md")]
    for p in walk(os.path.join(root, "out", "catalog")):
        plan.append((os.path.relpath(p, root), "catalog/" + os.path.relpath(p, os.path.join(root, "out", "catalog"))))
    for name in ("00-authentication.txt", "20-checker-a.txt", "21-checker-b.txt", "30-syntax-check.txt",
                 "40-negative-controls.txt", "50-FREEZE-RECORD.json", "60-gate-authentication.txt",
                 "60-final-gate.txt"):
        plan.append(("receipts/" + name, "receipts/" + name))
    for d in ("development", "tools"):
        for p in walk(os.path.join(root, d)):
            plan.append((os.path.relpath(p, root), d + "/" + os.path.relpath(p, os.path.join(root, d))))
    unrecorded = {"receipts/50-FREEZE-RECORD.json", "receipts/60-gate-authentication.txt", "receipts/60-final-gate.txt"}
    covered = {s for s, _ in plan}
    check_all = sorted(set(fr) - covered)
    if check_all:
        raise SystemExit(f"REFUSED: freeze-record files not in the plan: {check_all}")
    for src, rel in plan:
        data = open(os.path.join(root, src), "rb").read()
        if rel.endswith((".rs", ".pyc")) or b"\0" in data or data[:4] == b"\x7fELF":
            raise SystemExit(f"REFUSED: {rel} is not a text evidence file")
        if blob(data) in refused:
            raise SystemExit(f"REFUSED: {rel} equals a crate source or a mutant source")
        if src not in unrecorded and fr.get(src) != sha(data):
            raise SystemExit(f"REFUSED: {rel} differs from the freeze record")
        out = os.path.join(dest, rel)
        os.makedirs(os.path.dirname(out), exist_ok=True)
        with open(out, "wb") as fh:
            fh.write(data)
        os.chmod(out, 0o644)
    entries = sorted(((sha(open(p, "rb").read()), os.path.relpath(p, dest)) for p in walk(dest)),
                     key=lambda e: e[1])
    with open(os.path.join(dest, "SHA256SUMS"), "w") as fh:
        for h, rel in entries:
            fh.write(f"{h}  {rel}\n")
    os.chmod(os.path.join(dest, "SHA256SUMS"), 0o644)
    total = sum(os.path.getsize(p) for p in walk(dest))
    print(f"STAGED: {PACKET}/ ({len(entries) + 1} files, {len(entries)} listed in the self-excluding SHA256SUMS); "
          f"{total} bytes; limit {LIMIT}: {'within' if total <= LIMIT else 'EXCEEDED'}")
    return 0 if total <= LIMIT else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1], sys.argv[2]))

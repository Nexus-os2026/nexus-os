#!/usr/bin/env python3
"""Assembles the evidence-only packet (mission section 22).

  build_packet.py stage  ROOT STAGE        stage the frozen materials (run by the final gate)
  build_packet.py finish ROOT STAGE DEST   add INDEX.md and the final-gate receipts, write the
                                           self-excluding SHA256SUMS, place the packet at DEST

Staged: the documents, matrices and checker outputs of ROOT/out/packet; the receipts of ROOT/out/receipts and the
freeze record; ROOT/development; the tools and the document sources (under tools/). Never: a source file (`.rs`, or
any file whose bytes equal one of C's crate sources), anything binary, any `__pycache__`, `.pyc`, `.log`, `.tmp`. Every
staged file that was frozen must still equal its frozen sha256. Stage-1 and Stage-2 files are referenced, never copied.
"""
import hashlib
import json
import os
import shutil
import subprocess
import sys

REPO = "/home/nexus/NEXUS/nexus-os"
C = "d5964ee5326c2733177307db6f507328023d361a"
LIMIT = 2 * 1024 * 1024
REFUSED = (".rs", ".pyc", ".log", ".tmp", ".db")


def sha(b):
    return hashlib.sha256(b).hexdigest()


def sources():
    out = set()
    for l in subprocess.run(["git", "-C", REPO, "ls-tree", "-r", C, "crates/nexus-governed-control/src"],
                            capture_output=True, text=True, check=True).stdout.splitlines():
        blob = l.split()[2]
        out.add(sha(subprocess.run(["git", "-C", REPO, "cat-file", "blob", blob], capture_output=True, check=True).stdout))
    return out


def put(src, dest, rel, srcs, frozen, froz_key):
    if rel.endswith(REFUSED) or "__pycache__" in rel:
        raise SystemExit(f"REFUSED: {rel}")
    data = open(src, "rb").read()
    if b"\0" in data:
        raise SystemExit(f"REFUSED: {rel} is not text")
    if sha(data) in srcs:
        raise SystemExit(f"REFUSED: {rel} is a copy of a crate source at C")
    if froz_key is not None and frozen.get(froz_key) != sha(data):
        raise SystemExit(f"REFUSED: {rel} differs from its frozen copy {froz_key}")
    out = os.path.join(dest, rel)
    os.makedirs(os.path.dirname(out), exist_ok=True)
    with open(out, "wb") as fh:
        fh.write(data)
    os.chmod(out, 0o644)


def walk(base):
    for b, dirs, names in os.walk(base):
        dirs[:] = sorted(d for d in dirs if d != "__pycache__")
        for n in sorted(names):
            yield os.path.join(b, n)


def stage(root, dest):
    if os.path.exists(dest):
        raise SystemExit(f"REFUSED: {dest} exists")
    frozen = json.load(open(os.path.join(root, "receipts", "40-FREEZE.json")))["files"]
    srcs = sources()
    n = 0
    for p in walk(os.path.join(root, "out", "packet")):
        rel = os.path.relpath(p, os.path.join(root, "out", "packet"))
        put(p, dest, rel, srcs, frozen, os.path.relpath(p, root))
        n += 1
    for p in walk(os.path.join(root, "out", "receipts")):
        rel = "receipts/" + os.path.relpath(p, os.path.join(root, "out", "receipts"))
        put(p, dest, rel, srcs, frozen, os.path.relpath(p, root))
        n += 1
    for name in ("40-FREEZE.json", "40-FREEZE-RECORD.txt"):
        put(os.path.join(root, "receipts", name), dest, "receipts/" + name, srcs, {}, None)
        n += 1
    put(os.path.join(root, "receipts", "40-FREEZE-RECORD.txt"), dest, "FREEZE-RECORD.txt", srcs, {}, None)
    n += 1
    for p in walk(os.path.join(root, "development")):
        rel = "development/" + os.path.relpath(p, os.path.join(root, "development"))
        put(p, dest, rel, srcs, frozen, os.path.relpath(p, root))
        n += 1
    for p in walk(os.path.join(root, "tools")):
        rel = "tools/" + os.path.relpath(p, os.path.join(root, "tools"))
        put(p, dest, rel, srcs, frozen, os.path.relpath(p, root))
        n += 1
    for p in walk(os.path.join(root, "docs-src")):
        rel = "tools/docs-src/" + os.path.relpath(p, os.path.join(root, "docs-src"))
        put(p, dest, rel, srcs, frozen, os.path.relpath(p, root))
        n += 1
    total = sum(os.path.getsize(p) for p in walk(dest))
    print(f"STAGE: {n} files, {total} bytes, every frozen file equal to its freeze record")
    return 0


def finish(root, stage_dir, dest):
    if os.path.exists(dest) and os.listdir(dest):
        raise SystemExit(f"REFUSED: {dest} is not empty")
    srcs = sources()
    shutil.copytree(stage_dir, dest, dirs_exist_ok=True)
    for name in ("60-final-gate.txt", "60-gate-prepare-console.txt", "60-gate-stage-console.txt", "61-index-check.txt"):
        put(os.path.join(root, "receipts", name), dest, "receipts/" + name, srcs, {}, None)
    put(os.path.join(root, "INDEX.md"), dest, "INDEX.md", srcs, {}, None)
    entries = []
    for p in walk(dest):
        rel = os.path.relpath(p, dest)
        if rel != "SHA256SUMS":
            entries.append((sha(open(p, "rb").read()), rel))
    entries.sort(key=lambda e: e[1])
    with open(os.path.join(dest, "SHA256SUMS"), "w") as fh:
        for h, rel in entries:
            fh.write(f"{h}  {rel}\n")
    os.chmod(os.path.join(dest, "SHA256SUMS"), 0o644)
    total = sum(os.path.getsize(p) for p in walk(dest))
    print(f"PACKET {dest}: {len(entries) + 1} files ({len(entries)} listed in SHA256SUMS, which excludes itself), "
          f"{total} bytes; limit {LIMIT}: {'within' if total <= LIMIT else 'EXCEEDED'}")
    return 0 if total <= LIMIT else 1


if __name__ == "__main__":
    if sys.argv[1] == "stage":
        sys.exit(stage(sys.argv[2], sys.argv[3]))
    sys.exit(finish(sys.argv[2], sys.argv[3], sys.argv[4]))

#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-FINAL-EVIDENCE-STAGE2-01, mission section 14: assemble the evidence-only packet.

  build_packet.py <packet-dir>      copies the mission's text records into <packet-dir> (which must not exist, except
                                    for an INDEX.md written beforehand), then writes the self-excluding SHA256SUMS

Copied: receipts/, vocabulary/, normalized/, matrix/ (when produced), tools/ (scripts only, mode 0644, no
__pycache__), development/ (defective tool versions, revision diffs, development outputs), and the freeze record as
FREEZE-RECORD.txt. Never copied: the executable, the target directory, any source file (mutated sources of the
negative controls live outside the evidence directory; a file whose bytes equal a crate source blob at C refuses the
build), anything binary (a NUL byte refuses the build), any *.log, *.tmp, *.db or *.pyc name. Stage-1 files are
referenced by commit, path, blob and sha256, never copied.
"""
import hashlib
import os
import shutil
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import common  # noqa: E402

LIMIT = 12 * 1024 * 1024
IGNORED_SUFFIX = (".log", ".tmp", ".db", ".pyc")
DIRS = ("receipts", "vocabulary", "normalized", "matrix", "tools", "development")


def main(dest):
    pre = set(os.listdir(dest)) if os.path.isdir(dest) else set()
    if pre - {"INDEX.md"}:
        raise SystemExit(f"REFUSED: {dest} holds {sorted(pre)}")
    sources = {hashlib.sha256(common.blob(common.C, p)).hexdigest() for p in
               common.git("ls-tree", "-r", "--name-only", common.C, common.CRATE_SRC + "/").decode().split()}
    stage1 = {l.split("  ")[0] for l in
              common.blob(common.STAGE1, common.STAGE1_PACKET + "/SHA256SUMS").decode().splitlines()}
    os.makedirs(dest, exist_ok=True)
    copied, reused = [], []
    for sub in DIRS:
        for root, dirs, files in os.walk(os.path.join(common.EV, sub)):
            dirs[:] = sorted(d for d in dirs if d != "__pycache__")
            for f in sorted(files):
                src = os.path.join(root, f)
                rel = os.path.relpath(src, common.EV)
                if f.endswith(IGNORED_SUFFIX) or f.endswith(".rs"):
                    raise SystemExit(f"REFUSED: {rel} has a refused name")
                data = open(src, "rb").read()
                if b"\0" in data:
                    raise SystemExit(f"REFUSED: {rel} is not text")
                digest = hashlib.sha256(data).hexdigest()
                if digest in sources:
                    raise SystemExit(f"REFUSED: {rel} is a copy of a crate source file at C")
                if digest in stage1:
                    reused.append(rel)  # a tool carried unchanged from Stage 1 (byte-identical): recorded
                out = os.path.join(dest, rel)
                os.makedirs(os.path.dirname(out), exist_ok=True)
                with open(out, "wb") as o:
                    o.write(data)
                os.chmod(out, 0o644)
                copied.append(rel)
    shutil.copyfile(os.path.join(common.EV, "receipts/40-FREEZE-RECORD.txt"), os.path.join(dest, "FREEZE-RECORD.txt"))
    os.chmod(os.path.join(dest, "FREEZE-RECORD.txt"), 0o644)
    entries = []
    for root, dirs, files in os.walk(dest):
        dirs.sort()
        for f in sorted(files):
            rel = os.path.relpath(os.path.join(root, f), dest)
            if rel != "SHA256SUMS":
                entries.append((hashlib.sha256(open(os.path.join(root, f), "rb").read()).hexdigest(), rel))
    entries.sort(key=lambda e: e[1])
    with open(os.path.join(dest, "SHA256SUMS"), "w") as s:
        for digest, rel in entries:
            s.write(f"{digest}  {rel}\n")
    os.chmod(os.path.join(dest, "SHA256SUMS"), 0o644)
    total = sum(os.path.getsize(os.path.join(r, f)) for r, _, fs in os.walk(dest) for f in fs)
    print(f"packet {dest}: {len(entries) + 1} files ({len(entries)} listed in SHA256SUMS, which excludes itself), "
          f"{total} bytes; limit {LIMIT}: {'within' if total <= LIMIT else 'EXCEEDED'}")
    print(f"byte-identical to a Stage-1 packet file (tools carried unchanged): {reused}")
    return 0 if total <= LIMIT else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1]))

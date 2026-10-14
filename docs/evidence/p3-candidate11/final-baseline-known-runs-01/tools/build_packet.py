#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-FINAL-EVIDENCE-STAGE1-01, mission section 15: assemble the evidence-only packet.

  build_packet.py <packet-dir>      copies the mission's text records into <packet-dir> (which must not exist, except
                                    for an INDEX.md written beforehand), then writes the self-excluding SHA256SUMS

Copied: receipts/ (every receipt, raw output and classification), tools/ (scripts only, mode 0644, no __pycache__),
development/ (failed tool runs and revision diffs), the freeze record. Never copied: the executable, the target
directory, sources retrievable from C, anything binary (a NUL byte refuses the build), any *.log, *.tmp or *.db name
(the repository ignores them).
"""
import hashlib
import os
import shutil
import sys

EV = "/home/nexus/nexus-audits/claude-architect/p3-c11-dr01-d2a1-final-baseline-known-runs-01"
LIMIT = 6 * 1024 * 1024
IGNORED_SUFFIX = (".log", ".tmp", ".db", ".pyc")


def main(dest):
    pre = set(os.listdir(dest)) if os.path.isdir(dest) else set()
    if pre - {"INDEX.md"}:
        raise SystemExit(f"REFUSED: {dest} holds {sorted(pre)}")
    os.makedirs(dest, exist_ok=True)
    copied = []
    for sub in ("receipts", "tools", "development"):
        for root, dirs, files in os.walk(os.path.join(EV, sub)):
            dirs[:] = sorted(d for d in dirs if d != "__pycache__")
            for f in sorted(files):
                src = os.path.join(root, f)
                rel = os.path.relpath(src, EV)
                if f.endswith(IGNORED_SUFFIX):
                    raise SystemExit(f"REFUSED: {rel} has a name the repository ignores")
                data = open(src, "rb").read()
                if b"\0" in data:
                    raise SystemExit(f"REFUSED: {rel} is not text")
                out = os.path.join(dest, rel)
                os.makedirs(os.path.dirname(out), exist_ok=True)
                with open(out, "wb") as o:
                    o.write(data)
                os.chmod(out, 0o644)
                copied.append(rel)
    shutil.copyfile(os.path.join(EV, "receipts/03-FREEZE-RECORD.txt"), os.path.join(dest, "FREEZE-RECORD.txt"))
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
    return 0 if total <= LIMIT else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1]))

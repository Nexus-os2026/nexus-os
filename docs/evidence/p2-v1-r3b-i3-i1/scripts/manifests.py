#!/usr/bin/env python3
"""Check each historical evidence manifest against its own snapshot.

For every docs/evidence/<dir>/SHA256SUMS, the snapshot is the last commit
that changed <dir>. Every entry (paths relative to <dir>) must hash to the
manifest's value in that snapshot's Git blob. Entries inside <dir> must also
equal the working file now (the evidence is unchanged). An entry outside
<dir> (the design document) is compared with the working file only for
information: a later design revision legitimately differs.

Usage: manifests.py <checkout> <evidence dir>...
"""
import hashlib
import pathlib
import posixpath
import subprocess
import sys


def git(root, *args, binary=False):
    out = subprocess.run(["git", *args], cwd=root, capture_output=True, check=True)
    return out.stdout if binary else out.stdout.decode().strip()


def main():
    root = pathlib.Path(sys.argv[1]).resolve()
    ok = True
    for rel in sys.argv[2:]:
        rel = rel.rstrip("/")
        snapshot = git(root, "log", "-1", "--format=%H", "--", rel)
        print(f"{rel}: snapshot {snapshot}")
        for line in (root / rel / "SHA256SUMS").read_text().splitlines():
            digest, name = line.split("  ", 1)
            path = posixpath.normpath(posixpath.join(rel, name))
            blob = git(root, "show", f"{snapshot}:{path}", binary=True)
            at_snapshot = hashlib.sha256(blob).hexdigest() == digest
            working = hashlib.sha256((root / path).read_bytes()).hexdigest() == digest
            inside = path.startswith(rel + "/")
            entry_ok = at_snapshot and (working or not inside)
            ok &= entry_ok
            print(f"  {'OK ' if entry_ok else 'BAD'} {path}: snapshot {'match' if at_snapshot else 'MISMATCH'}, "
                  f"working file {'match' if working else 'differs'}"
                  f"{'' if inside else ' (outside the evidence directory)'}")
    print("RESULT:", "PASS" if ok else "FAIL")
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())

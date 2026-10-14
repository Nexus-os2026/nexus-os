#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-MUTATION-TRANCHE1-PUBLISH-04, mission sections 3 and 10: the complete manifest of a read-only root
(continue-02: the CONTINUE-02 execution-evidence root; publish-03: the PUBLISH-03 publication-recovery root). Adapted
from PUBLISH-03 tools/manifest.py (the same rows and the same comparison), with the root named on the command line.
Nothing under either root is written, followed or executed.

    manifest.py write ROOT OUT.tsv       one row per entry under ROOT (directories, regular files, symlinks): path,
                                         type, size, sha256 (regular files), mode, mtime_ns, symlink target
    manifest.py compare BEFORE AFTER     every entry of BEFORE present in AFTER with every field equal, and nothing
                                         added; exit 1 otherwise

Symlinks are recorded by lstat and readlink only; they are never followed. File type comes from lstat and the first
bytes (ELF magic, NUL, UTF-8), never from running anything.
"""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import pub4common as Q  # noqa: E402

HEADER = ["path", "type", "size", "sha256", "mode", "mtime_ns", "link_target"]


def rows(root):
    out = []
    for d, dirs, fs in os.walk(root, followlinks=False):
        dirs.sort()
        for n in sorted(dirs + fs):
            p = os.path.join(d, n)
            rel = os.path.relpath(p, root)
            st = os.lstat(p)
            t = Q.kind(p)
            out.append([rel, t, str(st.st_size), Q.sha256_file(p) if t.startswith("regular") else "-",
                        oct(st.st_mode), str(st.st_mtime_ns), os.readlink(p) if t == "symlink" else "-"])
    return sorted(out)


def write(name, out):
    if os.path.exists(out):
        raise SystemExit("REFUSED: output exists")
    root = Q.ROOTS[name]
    rs = rows(root)
    with open(out, "w") as f:
        f.write("\t".join(HEADER) + "\n")
        for r in rs:
            if any("\t" in x or "\n" in x for x in r):
                raise SystemExit(f"REFUSED: tab or newline in an entry: {r[0]!r}")
            f.write("\t".join(r) + "\n")
    types = {}
    for r in rs:
        types[r[1]] = types.get(r[1], 0) + 1
    print(f"root {name}: {root}")
    print(f"entries {len(rs)}; by type {dict(sorted(types.items()))}")
    print(f"regular-file bytes {sum(int(r[2]) for r in rs if r[1].startswith('regular'))}")
    print(f"zero-length regular files {sum(1 for r in rs if r[1].startswith('regular') and r[2] == '0')}")
    print(f"symlinks: {[(r[0], r[6]) for r in rs if r[1] == 'symlink']}")
    print(f"manifest {out} sha256 {Q.sha256_file(out)}")
    return 0


def load(path):
    ls = open(path).read().splitlines()
    if ls[0].split("\t") != HEADER:
        raise SystemExit(f"REFUSED: bad header in {path}")
    return {l.split("\t")[0]: l.split("\t") for l in ls[1:]}


def compare(before, after):
    b, a = load(before), load(after)
    changed = sorted(p for p in b if p in a and a[p] != b[p])
    missing = sorted(p for p in b if p not in a)
    added = sorted(p for p in a if p not in b)
    print(f"before {before} sha256 {Q.sha256_file(before)}: {len(b)} entries")
    print(f"after  {after} sha256 {Q.sha256_file(after)}: {len(a)} entries")
    print(f"{'OK  ' if not changed else 'FAIL'} every pre-existing entry has every field equal (type, size, sha256, mode, "
          f"mtime_ns, symlink target): changed {changed[:10]}")
    print(f"{'OK  ' if not missing else 'FAIL'} no pre-existing entry missing: {missing[:10]}")
    print(f"{'OK  ' if not added else 'FAIL'} nothing added under the root: {added[:10]}")
    ok = not (changed or missing or added)
    print("ROOT: " + ("UNCHANGED" if ok else "CHANGED"))
    return 0 if ok else 1


if __name__ == "__main__":
    Q.no_bytecode()
    if sys.argv[1] == "write":
        sys.exit(write(sys.argv[2], os.path.abspath(sys.argv[3])))
    sys.exit(compare(sys.argv[2], sys.argv[3]))

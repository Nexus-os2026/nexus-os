#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-MUTATION-TRANCHE1-EXEC-01, mission section 19: stage the evidence packet (text only). Revised by
P3-C11-DR01-D2A1-MUTATION-TRANCHE1-RESUME-01 (section 19): the C22 ruling record and CARRIED-FORWARD.md are staged; the
carried-forward copies of the previous packet's files are NOT staged (they are referenced by previous commit, path,
blob and sha256), and the staged packet is checked to republish none of them.

    build_packet.py DEST        DEST: the packet directory (inside the evidence worktree, or a rehearsal directory);
                                must not exist. Copies INDEX.md, TRANCHE-SUMMARY.md and CARRIED-FORWARD.md (rendered by
                                render.py into a fresh directory), the C22 ruling record, receipts/ (without the
                                carried-forward copies), tools/ (no caches) and development/; writes a self-excluding
                                SHA256SUMS; then checks the staged packet.

Checks (exit 1 if any fails): at most 32 MiB; every file text (UTF-8, no NUL byte); no Rust source, no ELF; no file
equal to any of the twelve mutant blobs, to any crate source blob of C, or to the pinned executables; no carried-forward
path and no file (of 1 KiB or more) of the previous packet's carried-forward set republished; every staged file
byte-equal to its source;
SHA256SUMS lists every file but itself and verifies. Nothing is committed here.
"""
import os
import shutil
import subprocess
import sys
import tempfile

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import common as K  # noqa: E402

SOURCES = [(K.C22_RULING, "ARCHITECT-RULING-C22-ACCEPTANCE.md"),
           ("receipts", "receipts"), ("tools", "tools"), ("development", "development")]


def carried(rel):
    """Is EV-relative path rel a carried-forward copy (never republished)?"""
    return any(rel == c or rel.startswith(c + "/") for c in K.CARRIED)


def files_under(root):
    out = []
    for d, dirs, fs in os.walk(root):
        dirs[:] = sorted(x for x in dirs if x != "__pycache__")
        out += [os.path.join(d, f) for f in sorted(fs) if not f.endswith(".pyc")]
    return out


def main(dest):
    if os.path.exists(dest):
        raise SystemExit("REFUSED: destination exists")
    os.makedirs(dest)
    rendered = tempfile.mkdtemp(prefix="mt1r-render-")
    r = subprocess.run([sys.executable, os.path.join(K.EV, "tools/render.py"), rendered], capture_output=True, text=True)
    print(r.stdout.strip(), r.stderr.strip())
    pairs = [(os.path.join(rendered, "INDEX.md"), "INDEX.md"),
             (os.path.join(rendered, "TRANCHE-SUMMARY.md"), "TRANCHE-SUMMARY.md"),
             (os.path.join(rendered, "CARRIED-FORWARD.md"), "CARRIED-FORWARD.md")]
    skipped = []
    for src, rel in SOURCES:
        p = os.path.join(K.EV, src)
        if os.path.isdir(p):
            for f in files_under(p):
                if carried(os.path.relpath(f, K.EV)):
                    skipped.append(os.path.relpath(f, K.EV))
                else:
                    pairs.append((f, os.path.join(rel, os.path.relpath(f, p))))
        else:
            pairs.append((p, rel))
    print(f"carried-forward copies not staged (referenced in CARRIED-FORWARD.md): {len(skipped)}")
    for src, rel in pairs:
        os.makedirs(os.path.dirname(os.path.join(dest, rel)), exist_ok=True)
        shutil.copyfile(src, os.path.join(dest, rel))
    names = sorted(rel for _, rel in pairs)
    with open(os.path.join(dest, "SHA256SUMS"), "w") as f:
        for rel in names:
            f.write(f"{K.sha256_file(os.path.join(dest, rel))}  {rel}\n")
    fails = []

    def check(label, ok, detail=""):
        print(f"{'OK  ' if ok else 'FAIL'} {label}{(': ' + str(detail)) if detail != '' else ''}")
        if not ok:
            fails.append(label)
    allf = [os.path.relpath(f, dest) for f in files_under(dest)]
    total = sum(os.path.getsize(os.path.join(dest, f)) for f in allf)
    check("packet size at most 32 MiB", total <= K.PACKET_LIMIT, f"{total} bytes, {len(allf)} files")
    nontext = []
    for f in allf:
        b = open(os.path.join(dest, f), "rb").read()
        try:
            b.decode("utf-8")
        except UnicodeDecodeError:
            nontext.append(f)
        if b"\0" in b or b[:4] == b"\x7fELF":
            nontext.append(f)
    check("text only (UTF-8, no NUL byte, no ELF)", not nontext, nontext[:5])
    check("no Rust source file", not [f for f in allf if f.endswith(".rs")])
    exp = __import__("json").load(open(os.path.join(K.EV, "receipts/02-EXPECTATIONS.json")))
    mutants = {c["mutant_blob"] for c in exp["controls"].values()}
    crate = set()
    for e in K.git("ls-tree", "-r", "-z", K.C, "crates/").split(b"\0"):
        if e:
            crate.add(e.split(b"\t", 1)[0].decode().split()[2])
    blobs = {f: K.git_blob_id(open(os.path.join(dest, f), "rb").read()) for f in allf}
    check("no file equal to a mutant source blob", not [f for f, b in blobs.items() if b in mutants])
    empty = K.git_blob_id(b"")
    check("no file equal to a crate source blob of C (the empty blob, present in C's crates/, copies no content and is "
          "not counted)", not [f for f, b in blobs.items() if b in crate and b != empty],
          f"{sum(b == empty for b in blobs.values())} empty file(s)")
    prev = {}
    for line in K.git("show", f"{K.PREV}:{K.PREV_PACKET}/SHA256SUMS").decode().splitlines():
        h, rel = line.split("  ", 1)
        if carried(rel) or rel in K.CARRIED_DOCS.values():
            prev[h] = rel
    check("no carried-forward path staged", not [f for f in allf if carried(f)], [f for f in allf if carried(f)][:3])
    same = [f for f in allf if K.sha256_file(os.path.join(dest, f)) in prev and os.path.getsize(os.path.join(dest, f)) > 0]
    dup = [f for f in same if os.path.getsize(os.path.join(dest, f)) >= 1024]
    check("no staged file of 1 KiB or more byte-equal to a carried-forward file of the previous packet (nothing "
          "duplicated)", not dup, dup[:3])
    print(f"     info: {len(same) - len(dup)} small staged file(s) coincide with a carried-forward file's bytes (an "
          f"independently produced constant record, e.g. the sampler's one-line note): "
          f"{[(f, os.path.getsize(os.path.join(dest, f)), prev[K.sha256_file(os.path.join(dest, f))]) for f in same if f not in dup][:6]}")
    check("every staged file byte-equal to its source", all(K.sha256_file(s) == K.sha256_file(os.path.join(dest, r))
                                                           for s, r in pairs))
    listed = [l.split("  ", 1) for l in open(os.path.join(dest, "SHA256SUMS")).read().splitlines()]
    check("SHA256SUMS lists every file but itself, and verifies",
          sorted(p for _, p in listed) == sorted(f for f in allf if f != "SHA256SUMS")
          and all(K.sha256_file(os.path.join(dest, p)) == h for h, p in listed), f"{len(listed)} listed")
    shutil.rmtree(rendered)
    print("PACKET: " + ("STAGED — ALL CHECKS HOLD" if not fails else "FAILED: " + "; ".join(fails)))
    return 0 if not fails else 1


if __name__ == "__main__":
    sys.exit(main(os.path.abspath(sys.argv[1])))

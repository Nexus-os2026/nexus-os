#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-CONTROL-DEFINITIONS-R2-01, mission section 9: syntax check of the 21 Architect-defined mutants
(A6 in its R2 form; the other 20 are R1's, byte for byte).

    syntax_check.py ROOT [CATDIR]

For each of the 21, the catalogued patch is applied with GNU patch (fuzz 0) to a disposable three-file copy of
Candidate C's ledger sources (protocol.rs, protocol/model_tests.rs, protocol/unit_tests.rs) in a fresh temporary
directory under ROOT/dev-local, never in a repository worktree. Then `rustfmt --check --edition 2021` runs on the
patched file only, with an empty configuration file (rustfmt's defaults, as the repository has no rustfmt.toml).
`--check` never writes. rustfmt parses the file (and, for protocol.rs, its two out-of-line child modules, which stay
C's bytes); a parse error or a formatting difference is a failure. Controls: C's unpatched files must pass, and a
deliberately unparsable copy must fail with a parse error. Result label for a passing mutant:
SYNTAX_CHECKED_NOT_TYPECHECKED. No Cargo, no rustc, no build, no test.
"""
import hashlib
import json
import os
import subprocess
import sys
import tempfile

REPO = "/home/nexus/NEXUS/nexus-os"
C = "d5964ee5326c2733177307db6f507328023d361a"
SRC = "crates/nexus-governed-control/src/"
FILES = ["ledger/protocol.rs", "ledger/protocol/model_tests.rs", "ledger/protocol/unit_tests.rs"]
RUSTFMT = os.path.expanduser("~/.rustup/toolchains/1.94.0-x86_64-unknown-linux-gnu/bin/rustfmt")
ORDER = ["C3a", "C3b", "C3c", "C5", "C6", "C7", "C8", "C11", "C15", "C16", "C17", "C18a", "C18b", "C20", "C23",
         "C26", "A1", "A2", "A3", "A5", "A6"]
fails = []


def sha(b):
    return hashlib.sha256(b).hexdigest()


def rustfmt_check(root, files, target, patch=None):
    with tempfile.TemporaryDirectory(dir=os.path.join(root, "dev-local")) as d:
        for rel, data in files.items():
            p = os.path.join(d, "src", rel)
            os.makedirs(os.path.dirname(p), exist_ok=True)
            with open(p, "wb") as fh:
                fh.write(data)
        open(os.path.join(d, "empty-rustfmt.toml"), "w").close()
        blob = None
        if patch is not None:
            r = subprocess.run(["patch", "-s", "-p4", "-F", "0", "--no-backup-if-mismatch", "-r", "-", "-N"],
                               cwd=os.path.join(d, "src"), input=patch, capture_output=True)
            if r.returncode != 0:
                return {"applied": False, "rc": None, "stdout": "", "stderr": r.stderr.decode()}
        data = open(os.path.join(d, "src", target), "rb").read()
        blob = subprocess.run(["git", "hash-object", "--stdin"], input=data, capture_output=True,
                              check=True).stdout.decode().strip()
        r = subprocess.run([RUSTFMT, "--check", "--edition", "2021", "--config-path",
                            os.path.join(d, "empty-rustfmt.toml"), os.path.join("src", target)],
                           cwd=d, capture_output=True)
        after = open(os.path.join(d, "src", target), "rb").read()
        return {"applied": True, "rc": r.returncode, "stdout": r.stdout.decode().replace(d, "<tmp>"),
                "stderr": r.stderr.decode().replace(d, "<tmp>"), "blob": blob, "unchanged_by_rustfmt": after == data}


def main(root, catdir):
    cat = json.load(open(os.path.join(catdir, "catalog.json")))
    by = {e["id"]: e for e in cat["controls"]}
    files = {f: subprocess.run(["git", "-C", REPO, "show", f"{C}:{SRC}{f}"], capture_output=True,
                               check=True).stdout for f in FILES}
    rf = open(RUSTFMT, "rb").read()
    ver = subprocess.run([RUSTFMT, "--version"], capture_output=True, text=True).stdout.strip()
    print("P3-C11-DR01-D2A1-CONTROL-DEFINITIONS-R2-01 — SYNTAX CHECK (rustfmt --check only; no Cargo, no rustc)")
    print(f"rustfmt: {RUSTFMT.replace(os.path.expanduser('~'), '~')}; {ver}; sha256 {sha(rf)}")
    print("invocation: rustfmt --check --edition 2021 --config-path <empty file> src/<patched file>")
    print("edition 2021: crates/nexus-governed-control/Cargo.toml at C; no rustfmt.toml in the repository at C")
    print()
    print("[controls]")
    for target in ("ledger/protocol.rs", "ledger/protocol/model_tests.rs"):
        r = rustfmt_check(root, files, target)
        ok = r["rc"] == 0 and r["unchanged_by_rustfmt"]
        print(f"{'OK  ' if ok else 'FAIL'} baseline C {target}: rustfmt --check rc {r['rc']} (must be 0)")
        if not ok:
            fails.append("baseline " + target)
    broken = dict(files)
    broken["ledger/protocol.rs"] = files["ledger/protocol.rs"].replace(b"    fn flags(&self) -> u64 {",
                                                                       b"    fn flags(&self) -> u64 {{", 1)
    r = rustfmt_check(root, broken, "ledger/protocol.rs")
    ok = r["rc"] != 0 and "error" in r["stderr"].lower()
    print(f"{'OK  ' if ok else 'FAIL'} negative control (an extra '{{' in Core::flags): rustfmt --check rc {r['rc']} "
          f"with a parse error (must fail)")
    print("      " + (r["stderr"].strip().splitlines() or [""])[0])
    if not ok:
        fails.append("negative control")
    print()
    print("[the 21 Architect-defined mutants]")
    for cid in ORDER:
        e = by[cid]
        target = e["mutation_source_file"][len(SRC):]
        patch = open(os.path.join(catdir, e["patch_file"]), "rb").read()
        r = rustfmt_check(root, files, target, patch)
        ok = r["applied"] and r["rc"] == 0 and r["blob"] == e["mutant_blob"] and r["unchanged_by_rustfmt"]
        label = "SYNTAX_CHECKED_NOT_TYPECHECKED" if ok else "SYNTAX CHECK FAILED"
        print(f"{'OK  ' if ok else 'FAIL'} {cid}: {target} mutant blob {r.get('blob')} (catalogued "
              f"{e['mutant_blob']}); rustfmt --check rc {r['rc']} -> {label}")
        for stream in ("stdout", "stderr"):
            if r[stream].strip():
                for line in r[stream].strip().splitlines()[:40]:
                    print(f"      {stream}: {line}")
        if not ok:
            fails.append(cid)
    print()
    print(f"failing {len(fails)}: {' '.join(fails) or 'none'}")
    print("SYNTAX CHECK: " + ("PASS — all 21 SYNTAX_CHECKED_NOT_TYPECHECKED" if not fails else "FAIL"))
    return 0 if not fails else 1


if __name__ == "__main__":
    r = sys.argv[1]
    sys.exit(main(r, sys.argv[2] if len(sys.argv) > 2 else os.path.join(r, "out", "catalog")))

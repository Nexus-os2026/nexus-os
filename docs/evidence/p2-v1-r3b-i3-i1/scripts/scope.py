#!/usr/bin/env python3
"""P2-V1-R3B-I3-I1 scope check.

Every path changed relative to the base must lie in the mission's envelope,
and every protected file must be byte-identical to the base. Before the
commit (HEAD is the base) the changes are the worktree's status; after it,
the changes are base..HEAD.

Usage: scope.py <checkout> <base commit>
"""
import fnmatch
import hashlib
import subprocess
import sys

ENVELOPE = [
    "crates/nexus-verifier-sandbox/tests/support/custody/store/*.rs",
    "crates/nexus-verifier-sandbox/tests/phase2_custody_store.rs",
    "crates/nexus-verifier-sandbox/tests/support/custody/mod.rs",
    "docs/architecture/p2-custody-store-implementation-boundary.md",
    "docs/evidence/p2-v1-r3b-i3-i1/*",
]
PROTECTED = [
    "crates/nexus-verifier-sandbox/tests/support/custody/core.rs",
    "crates/nexus-verifier-sandbox/tests/support/custody/model.rs",
    "crates/nexus-verifier-sandbox/tests/support/custody/codec.rs",
    "crates/nexus-verifier-sandbox/tests/phase2_custody_core.rs",
    "crates/nexus-verifier-sandbox/tests/phase2_custody_codec.rs",
    "crates/nexus-verifier-sandbox/tests/support/cleanup_observation.rs",
    "crates/nexus-verifier-sandbox/tests/phase2_cleanup_observation.rs",
    "crates/nexus-verifier-sandbox/tests/phase2_live_sandbox.rs",
    "crates/nexus-verifier-sandbox/tests/phase2_package_layout.rs",
    "crates/nexus-verifier-sandbox/Cargo.toml",
    "Cargo.toml",
    "Cargo.lock",
    "docs/architecture/p2-custody-durable-recorder-design.md",
]
PROTECTED_TREES = [
    "crates/nexus-verifier-sandbox/src",
    ".github",
    "docs/evidence/p2-v1-r3b-i3-p-r1",
    "docs/evidence/p2-v1-r3b-i3-p-r2",
    "docs/evidence/p2-v1-r3b-i3-p-r3",
    "docs/evidence/p2-v1-r3b-i3-p-r4",
    "docs/evidence/p2-v1-r3b-i3-p-r5",
]


def git(root, *args, binary=False, check=True):
    out = subprocess.run(["git", *args], cwd=root, capture_output=True, check=check)
    return out.stdout if binary else out.stdout.decode()


def main():
    root, base = sys.argv[1], sys.argv[2]
    head = git(root, "rev-parse", "HEAD").strip()
    if head == git(root, "rev-parse", base).strip():
        mode = "worktree status against the base (before the commit)"
        lines = git(root, "status", "--porcelain=v1", "--untracked-files=all").splitlines()
        changes = [(line[:2].strip(), line[3:]) for line in lines if line]
    else:
        mode = f"base..HEAD ({head})"
        lines = git(root, "diff", "--name-status", "--no-renames", base, head).splitlines()
        changes = [tuple(line.split("\t", 1)) for line in lines if line]
    ok = True
    print(f"mode: {mode}")
    print(f"base: {base}")
    print(f"changed paths ({len(changes)}):")
    for status, path in sorted(changes, key=lambda item: item[1]):
        inside = any(fnmatch.fnmatch(path, pattern) for pattern in ENVELOPE)
        ok &= inside
        print(f"  {status:2} {path}{'' if inside else '   OUTSIDE THE ENVELOPE'}")
    print("protected files (SHA-256 at the base, and now):")
    for path in PROTECTED:
        at_base = hashlib.sha256(git(root, "show", f"{base}:{path}", binary=True)).hexdigest()
        if head == git(root, "rev-parse", base).strip():
            now = hashlib.sha256(open(f"{root}/{path}", "rb").read()).hexdigest()
        else:
            now = hashlib.sha256(git(root, "show", f"{head}:{path}", binary=True)).hexdigest()
        same = at_base == now
        ok &= same
        print(f"  {'same' if same else 'CHANGED'} {at_base} {path}")
    print("protected trees (no path under them changed):")
    for tree in PROTECTED_TREES:
        touched = [path for _, path in changes if path == tree or path.startswith(tree + "/")]
        ok &= not touched
        print(f"  {'unchanged' if not touched else 'CHANGED ' + str(touched)} {tree}")
    print("RESULT:", "PASS" if ok else "FAIL")
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())

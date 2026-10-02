#!/usr/bin/env python3
"""P2-V1-R3B-I3-I1-R3 scope check.

Every path changed relative to the base must lie in the mission's envelope,
and every protected file and tree must be unchanged from the base. Before
the commit (HEAD is the base) the changes are the worktree's status; after
it, they are base..HEAD.

Usage: scope.py <checkout> <base commit>
"""
import fnmatch
import hashlib
import subprocess
import sys

# The mission's section 5: maintenance.rs, io.rs, sim.rs, the store test
# target, store/mod.rs and the boundary document; format.rs, open.rs and
# classify.rs narrowly; the R3 evidence directory. (No new store submodule.)
ENVELOPE = [
    "crates/nexus-verifier-sandbox/tests/support/custody/store/maintenance.rs",
    "crates/nexus-verifier-sandbox/tests/support/custody/store/io.rs",
    "crates/nexus-verifier-sandbox/tests/support/custody/store/sim.rs",
    "crates/nexus-verifier-sandbox/tests/support/custody/store/mod.rs",
    "crates/nexus-verifier-sandbox/tests/support/custody/store/format.rs",
    "crates/nexus-verifier-sandbox/tests/support/custody/store/open.rs",
    "crates/nexus-verifier-sandbox/tests/support/custody/store/classify.rs",
    "crates/nexus-verifier-sandbox/tests/phase2_custody_store.rs",
    "docs/architecture/p2-custody-store-implementation-boundary.md",
    "docs/evidence/p2-v1-r3b-i3-i1-r3/*",
]
PROTECTED = [
    "crates/nexus-verifier-sandbox/tests/support/custody/store/owner.rs",
    "crates/nexus-verifier-sandbox/tests/support/custody/store/exchange.rs",
    "crates/nexus-verifier-sandbox/tests/support/custody/store/recorder.rs",
    "crates/nexus-verifier-sandbox/tests/support/custody/store/disposition.rs",
    "crates/nexus-verifier-sandbox/tests/support/custody/store/faults.rs",
    "crates/nexus-verifier-sandbox/tests/support/custody/core.rs",
    "crates/nexus-verifier-sandbox/tests/support/custody/model.rs",
    "crates/nexus-verifier-sandbox/tests/support/custody/codec.rs",
    "crates/nexus-verifier-sandbox/tests/support/custody/mod.rs",
    "crates/nexus-verifier-sandbox/tests/phase2_custody_core.rs",
    "crates/nexus-verifier-sandbox/tests/phase2_custody_codec.rs",
    "crates/nexus-verifier-sandbox/tests/support/cleanup_observation.rs",
    "crates/nexus-verifier-sandbox/tests/phase2_cleanup_observation.rs",
    "crates/nexus-verifier-sandbox/tests/phase2_live_sandbox.rs",
    "crates/nexus-verifier-sandbox/tests/phase2_package_layout.rs",
    "crates/nexus-verifier-sandbox/Cargo.toml",
    "crates/nexus-verifier-sandbox/build.rs",
    "Cargo.toml",
    "Cargo.lock",
    "rust-toolchain.toml",
    "AGENTS.md",
    "CLAUDE.md",
    ".gitattributes",
    ".gitignore",
    "docs/architecture/p2-custody-durable-recorder-design.md",
]
PROTECTED_TREES = [
    "crates/nexus-verifier-sandbox/src",
    ".github",
    "docs/evidence/p2-v1-r3b-i3-i1",
    "docs/evidence/p2-v1-r3b-i3-i1-r1",
    "docs/evidence/p2-v1-r3b-i3-i1-r2",
    "docs/evidence/p2-v1-r3b-i3-p-r1",
    "docs/evidence/p2-v1-r3b-i3-p-r2",
    "docs/evidence/p2-v1-r3b-i3-p-r3",
    "docs/evidence/p2-v1-r3b-i3-p-r4",
    "docs/evidence/p2-v1-r3b-i3-p-r5",
]


def git(root, *args, binary=False, check=True):
    out = subprocess.run(["git", *args], cwd=root, capture_output=True, check=check)
    return out.stdout if binary else out.stdout.decode()


def blob(root, rev, path):
    out = subprocess.run(["git", "show", f"{rev}:{path}"], cwd=root, capture_output=True)
    return out.stdout if out.returncode == 0 else None


def main():
    root, base = sys.argv[1], sys.argv[2]
    head = git(root, "rev-parse", "HEAD").strip()
    before = head == git(root, "rev-parse", base).strip()
    if before:
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
        # No new .gitattributes exemptions are authorized, anywhere.
        attributes = path.rsplit("/", 1)[-1] == ".gitattributes"
        ok &= inside and not attributes
        note = "" if inside else "   OUTSIDE THE ENVELOPE"
        note += "   A .gitattributes FILE" if attributes else ""
        print(f"  {status:2} {path}{note}")
    print("protected files (SHA-256 at the base, and now; absent at both is unchanged):")
    for path in PROTECTED:
        at_base = blob(root, base, path)
        if before:
            try:
                now = open(f"{root}/{path}", "rb").read()
            except FileNotFoundError:
                now = None
        else:
            now = blob(root, head, path)
        same = at_base == now
        ok &= same
        digest = hashlib.sha256(at_base).hexdigest() if at_base is not None else "(absent)"
        print(f"  {'same' if same else 'CHANGED'} {digest} {path}")
    print("protected trees (no path under them changed):")
    for tree in PROTECTED_TREES:
        touched = [path for _, path in changes if path == tree or path.startswith(tree + "/")]
        ok &= not touched
        print(f"  {'unchanged' if not touched else 'CHANGED ' + str(touched)} {tree}")
    print("RESULT:", "PASS" if ok else "FAIL")
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())

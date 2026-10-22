#!/usr/bin/env python3
"""P2-V1-R3B-I4-R1 scope check (I4's, with the I4-R1 envelope).

Every path changed relative to the base must lie in the mission's envelope
(section 19), no `.gitattributes` file may change anywhere, and every
protected file and tree must be unchanged from the base (a protected tree
may contain only its listed, narrowly authorized exceptions). Before the
commit (HEAD is the base) the changes are the worktree's status; after it,
they are base..HEAD.

Usage: scope.py <checkout> <base commit>
"""
import fnmatch
import hashlib
import subprocess
import sys

CRATE = "crates/nexus-verifier-sandbox"
# Section 19: the files that may change; the live harness and the desktop's
# Phase Two guards narrowly; the new evidence directory.
ENVELOPE = [
    f"{CRATE}/src/scope.rs",
    f"{CRATE}/src/scope/manager.rs",
    f"{CRATE}/src/scope/native.rs",
    f"{CRATE}/src/scope/pending.rs",
    f"{CRATE}/src/scope/tests.rs",
    f"{CRATE}/src/execution.rs",
    f"{CRATE}/src/execution/tests.rs",
    f"{CRATE}/src/fault.rs",
    f"{CRATE}/src/launcher.rs",
    "docs/security/phase2-governed-verification.md",
    f"{CRATE}/tests/phase2_live_sandbox.rs",
    "app/src-tauri/src/phase2_tests.rs",
    "docs/evidence/p2-v1-r3b-i4-r1-native-scope/*",
]
# Must remain byte-identical (section 19): the helper implementation (but
# launcher.rs), protocol, seccomp, Landlock, workspace, toolchain, profiles
# and policies, the manifests, the lock file, the toolchain pin and the
# governance files; the accepted custody modules and their documents; the
# crate root; the desktop's production coding flow.
PROTECTED = [
    f"{CRATE}/src/lib.rs",
    f"{CRATE}/tests/phase2_custody_store.rs",
    f"{CRATE}/tests/phase2_custody_codec.rs",
    f"{CRATE}/tests/phase2_custody_core.rs",
    f"{CRATE}/tests/phase2_cleanup_observation.rs",
    f"{CRATE}/tests/phase2_package_layout.rs",
    "app/src-tauri/src/coding_flow.rs",
    "app/src-tauri/src/coding_flow/verification.rs",
    "app/src-tauri/src/lib.rs",
    "app/src-tauri/Cargo.toml",
    f"{CRATE}/src/helper.rs",
    f"{CRATE}/src/protocol.rs",
    f"{CRATE}/src/seccomp.rs",
    f"{CRATE}/src/seccomp_policy.rs",
    f"{CRATE}/src/landlock_rules.rs",
    f"{CRATE}/src/workspace.rs",
    f"{CRATE}/src/toolchain.rs",
    f"{CRATE}/src/toolchain/contract.rs",
    f"{CRATE}/src/policy.rs",
    f"{CRATE}/src/profile.rs",
    f"{CRATE}/src/profile_launch.rs",
    f"{CRATE}/src/applicability.rs",
    f"{CRATE}/src/hash.rs",
    f"{CRATE}/src/sys.rs",
    f"{CRATE}/src/main.rs",
    f"{CRATE}/Cargo.toml",
    f"{CRATE}/build.rs",
    "Cargo.toml",
    "Cargo.lock",
    "rust-toolchain.toml",
    "AGENTS.md",
    "CLAUDE.md",
    ".gitattributes",
    ".gitignore",
    "docs/architecture/p2-custody-durable-recorder-design.md",
    "docs/architecture/p2-custody-store-implementation-boundary.md",
]
# (tree, the narrowly authorized paths inside it)
PROTECTED_TREES = [
    (f"{CRATE}/tests", [f"{CRATE}/tests/phase2_live_sandbox.rs"]),
    (f"{CRATE}/tests/support", []),
    ("app", ["app/src-tauri/src/phase2_tests.rs"]),
    ("docs/evidence/p2-v1-r3b-i4-native-scope", []),
    ("docs/architecture", []),
    (f"{CRATE}/src/applicability", []),
    (f"{CRATE}/src/policy", []),
    (f"{CRATE}/src/profile", []),
    (f"{CRATE}/src/protocol", []),
    (f"{CRATE}/src/seccomp_policy", []),
    (f"{CRATE}/src/toolchain", []),
    (f"{CRATE}/src/workspace", []),
    (".github", []),
    ("packaging", []),
    ("docs/evidence/p2-v1-r3b-i3-i1", []),
    ("docs/evidence/p2-v1-r3b-i3-i1-r1", []),
    ("docs/evidence/p2-v1-r3b-i3-i1-r2", []),
    ("docs/evidence/p2-v1-r3b-i3-i1-r3", []),
    ("docs/evidence/p2-v1-r3b-i3-p-r1", []),
    ("docs/evidence/p2-v1-r3b-i3-p-r2", []),
    ("docs/evidence/p2-v1-r3b-i3-p-r3", []),
    ("docs/evidence/p2-v1-r3b-i3-p-r4", []),
    ("docs/evidence/p2-v1-r3b-i3-p-r5", []),
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
    print("protected trees (no path under them changed but the listed exceptions):")
    for tree, exceptions in PROTECTED_TREES:
        touched = [path for _, path in changes
                   if (path == tree or path.startswith(tree + "/")) and path not in exceptions]
        ok &= not touched
        allowed = f" (except {', '.join(exceptions)})" if exceptions else ""
        print(f"  {'unchanged' if not touched else 'CHANGED ' + str(touched)} {tree}{allowed}")
    print("RESULT:", "PASS" if ok else "FAIL")
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())

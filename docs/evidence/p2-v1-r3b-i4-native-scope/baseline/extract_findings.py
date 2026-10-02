#!/usr/bin/env python3
"""P2-V1-R3B-I4 baseline: exact source-control-flow evidence at the base.

Reads each file from the base commit's Git object (never the worktree),
prints every finding's exact line range and text, the SHA-256 of that
excerpt and of the whole file, and checks each excerpt still says what the
finding claims (a fixed needle per excerpt).

Usage: extract_findings.py <repository> <base commit>
"""
import hashlib
import subprocess
import sys

SCOPE = "crates/nexus-verifier-sandbox/src/scope.rs"
EXEC = "crates/nexus-verifier-sandbox/src/execution.rs"

FINDINGS = [
    ("B-I4-1", "StartTransientUnit error or timeout is propagated by `?` before any retained scope object exists", [
        (SCOPE, 123, 148, ["tokio::time::timeout(", "ScopeError::Bus(format!(\"{method} timed out\"))"]),
        (SCOPE, 163, 196, ["let unit = format!(\"nexus-verifier-{}.scope\", random_hex()?);",
                           "\"StartTransientUnit\",", ")?;"]),
        (EXEC, 478, 482, ["match scopes.start_with_fault(helper, limits, fault) {",
                          "Err(error) => return not_run(NotRun::Scope(error)),"]),
    ]),
    ("B-I4-2", "A proof failure calls StopUnit best-effort and discards the StopUnit result", [
        (SCOPE, 197, 213, ["if !matches!(proven, Ok(Ok(_))) {",
                           "let _: Result<zbus::zvariant::OwnedObjectPath, _> = self.call(",
                           "\"StopUnit\","]),
    ]),
    ("B-I4-3", "A panic in proof follows the same best-effort StopUnit path before resuming the unwind", [
        (SCOPE, 202, 213, ["let proven = catch_unwind(AssertUnwindSafe(|| {",
                           "proven.unwrap_or_else(|panic| resume_unwind(panic))"]),
        (SCOPE, 216, 225, ["let path = wait_for_placement(helper_pid, &unit)?;",
                           "fault::at(fault, FaultPoint::ScopeProof);",
                           "let scope = Scope::open(&path, unit, helper_pid)?;"]),
    ]),
    ("B-I4-4", "Owned and RetainedBoundary have no pending-scope state", [
        (EXEC, 151, 158, ["pub struct RetainedBoundary {", "scope: Option<Scope>,", "helper: Option<Helper>,"]),
        (EXEC, 334, 358, ["struct Owned {", "scope: Option<Scope>,",
                          "match (self.scope.take(), self.helper.take()) {"]),
    ]),
    ("B-I4-5", "Finalization knows only Option<Scope>: it cannot reconcile a maybe-created unit when no Scope was proven", [
        (EXEC, 531, 582, ["fn finalize_steps(owned: &mut Owned, fault: Option<Fault>) -> Finalized {",
                          "fn end(scope: Option<&Scope>, helper: &mut Option<Helper>) -> bool {",
                          "None => true,"]),
        (EXEC, 160, 176, ["pub fn retry(mut self) -> Result<(), Self> {",
                          "end(self.scope.as_ref(), &mut self.helper)"]),
    ]),
]


def show(repo, base, path):
    return subprocess.run(["git", "-C", repo, "show", f"{base}:{path}"], capture_output=True,
                          check=True).stdout


def main():
    repo, base = sys.argv[1], sys.argv[2]
    files = {}
    ok = True
    print(f"base: {base}")
    for path in (SCOPE, EXEC):
        data = show(repo, base, path)
        files[path] = data.decode().split("\n")
        print(f"file {path}: sha256 {hashlib.sha256(data).hexdigest()} ({len(files[path]) - 1} lines)")
    for fid, claim, excerpts in FINDINGS:
        print()
        print(f"== {fid}: {claim}")
        for path, first, last, needles in excerpts:
            lines = files[path][first - 1:last]
            text = "\n".join(lines) + "\n"
            digest = hashlib.sha256(text.encode()).hexdigest()
            present = [needle in text for needle in needles]
            ok &= all(present)
            print(f"-- {path}:{first}-{last} sha256 {digest} needles {'all present' if all(present) else present}")
            for number, line in enumerate(lines, first):
                print(f"{number:5}| {line}")
    print()
    print("RESULT:", "every excerpt holds its needles" if ok else "MISMATCH")
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())

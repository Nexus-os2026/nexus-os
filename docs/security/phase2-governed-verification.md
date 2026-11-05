# Retained verifier boundary for Mother Nexus

This file's historical name is kept for source references. It describes the
retained Linux verifier foundation, **not an available general execution lane
in Mother**, a current phase mission, or installed-host acceptance. Old candidate
status, app wiring and evidence dossiers remain in Git history. Current work is
controlled by MOTHER.md and the latest approved mission.

## Authority and isolation contract

Untrusted project tests, macros and subprocesses receive no authority by being
called verification. A candidate is materialized from its verified manifest;
launch approval binds the exact candidate, profile, policy and toolchain. Results
are advisory and bound to the reviewed candidate, never permission to apply.
Frontend paths, model text, argv, PIDs, unit names, cgroup strings, UUIDs and
serialized data cannot mint backend authority.

A production launch requires every layer, or it is **unavailable**. There is no
fallback to process groups, rlimits alone, a shell, containers or an unsandboxed
subprocess:

1. Linux x86_64 with a qualified host.
2. A dedicated trusted helper and retained, unreaped child identity.
3. A new user namespace with parent-written uid/gid mapping.
4. New PID (trusted PID 1), network (loopback down), IPC, UTS and cgroup namespaces.
5. Strict Landlock, ABI ≥ 6, fully enforced.
6. The production seccomp-BPF allow-list and `PR_SET_NO_NEW_PRIVS`.
7. Backend-owned cgroup v2 limits through the systemd user manager.
8. Closed inherited descriptors, sealed environment and exact verified workspace.
9. Verified packaged verifier toolchain and installed helper.
10. Backend execution generation/lifecycle ownership and exact native launch approval.

Project code must not read, write or execute host content outside allowed roots,
reach network or local sockets, see or signal host processes, create namespaces,
or outlive finalization. Verifier access to `/proc` is denied. Limits and profiles
are backend-owned; a caller cannot weaken them.

## Lifetime, cleanup and evidence

The driver owns the helper, cgroup scope and any possibly created pending scope
from request through confirmed cleanup. A unit name or returned path is not that
ownership. Retained process identity, generation rechecks and bounded native
cleanup are required; ambiguous setup or cleanup remains an error. No arbitrary
PID killing, name matching, unbounded waits or convenience shell cleanup.

The entry executable is launched by descriptor after immediate re-verification.
Candidate paths, permissions, symlinks and identity are rechecked. Every output
is bounded; cleanup must be confirmed before clean completion is reported.
Missing proof never becomes successful verification. Live sandbox availability
requires its dedicated host qualification; ordinary unit CI is insufficient.

## Explicit non-claims

No private mount view or private `/proc` is claimed. Landlock does not hide host
path metadata (existence, stat information or symlink targets). Root, kernel
vulnerabilities and malicious same-UID host processes modifying user-owned state
outside retained identities are outside the claim. A path grant and process group
are not this sandbox. Windows and macOS are unavailable, not degraded modes.

The inert-page checker is a separate deterministic content check. It does not
render, and its pass does not establish sandbox availability or layout proof.
A retained verifier library does not authorize Mother to execute arbitrary code.
No removed desktop route or old service is re-enabled by this document.

## Code and test contracts

The implementation is in `crates/nexus-verifier-sandbox`: policy, toolchain,
workspace, launcher, scope and execution modules. Host and package checks are in
its integration tests. [The packaging README](../../packaging/verifier-toolchain/README.md)
explains retained compatibility paths and why the former app recipe is unusable.

The [custody design](../architecture/p2-custody-durable-recorder-design.md) and
[implementation boundary](../architecture/p2-custody-store-implementation-boundary.md)
are retained test-side contracts, not production integration or Mother acceptance.
Their parsed procedure/crash tables remain exact. See [SECURITY.md](../../SECURITY.md)
for current non-claims and the approved mission for required gates.

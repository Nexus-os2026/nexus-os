# Security Policy

## Reporting Vulnerabilities

**Do NOT open public issues for security vulnerabilities.**

Email: **security@nexus-os.dev** (or open a confidential issue on GitLab)

We will acknowledge receipt within 48 hours and provide an initial assessment within 7 days.

Include: description, reproduction steps, potential impact, and suggested fix if any.

### Scope

Reports are welcome for any way to obtain authority the system should not
grant, for example:

- a caller, model output, webview script or stored record obtaining
  authority from a string, path, flag or identifier it supplies;
- reaching a surface that Phase Zero closes or withdraws (a closed command,
  a withdrawn binary or recipe);
- bypassing a grant's narrowing, revocation, expiry or workspace
  containment;
- governance bypass (capability checks, human-approval steps, resource
  bounds);
- audit trail tampering or integrity violation;
- secret or credential exposure, encryption weaknesses, and authentication
  or authorization bypass.

---

## Current status: Phase One complete — Linux support profile

- The Phase Zero trust-boundary rebuild is complete for the validated Linux
  support profile ("PHASE ZERO COMPLETE — LINUX SUPPORT PROFILE", declared
  by the project Architect on 2026-09-29 for commit
  `f727f5c39fab8d5c729a55eb28ad576d3d56ce47`). The recorded limitations and
  non-claims below remain in force; completion does not turn them into
  guarantees.
- Phase One, one governed coding workflow, is complete for the same Linux
  support profile ("PHASE ONE COMPLETE — LINUX SUPPORT PROFILE", declared by
  the project Architect on 2026-09-30 for commit
  `14270a9a38770ac84456c1f812042d2967edec42`). See
  [Governed coding (Phase One)](#governed-coding-phase-one) below.
- Phase Two, governed verification execution for the Linux support profile,
  is in progress: an implementation candidate on
  `implement/phase2-governed-verification` under Architect review. It is not
  integrated or complete and makes no claim yet; see
  [Governed verification (Phase Two, in progress)](#governed-verification-phase-two-in-progress).
- That checkpoint stays frozen on `rebuild/phase1-governed-coding`. Its
  completion evidence is the `evidence/phase1-closure` branch at
  `c237937189b5977eaad01acad6a4c52bcce796cc`. Later commits do not redefine
  it: status documentation and the generated ACL schema and test hygiene
  commit `6f3d64360dd21aa8d717c3995c46e48f396148b9` (`main` after Phase One
  completion), then the entry hardening described in
  [Hardening since the Phase One checkpoint](#hardening-since-the-phase-one-checkpoint).
- Linux is the active validation target for both phases. Windows and macOS
  portability validation is deferred, and no Phase Zero or Phase One
  security claim is made for them.
- No Phase Zero production release, and no security-supported release, has
  been declared yet. This policy therefore lists no supported versions.
  Releases, tags and version numbers published before the rebuild predate
  Phase Zero and are not a statement of current support.
- The current engineering evidence, including explicit non-claims, is in the
  [Phase Zero Final-Gate dossier](docs/security/phase0-final-gate-dossier.md),
  the
  [Phase Zero authority inventory](docs/security/phase0-c5-authority-inventory.md)
  and, for Phase One, the completion record on the `evidence/phase1-closure`
  branch. They are internal engineering evidence, not an external audit or a
  certification.

## Governed verification (Phase Two, in progress)

The candidate's design record is
[docs/security/phase2-governed-verification.md](docs/security/phase2-governed-verification.md),
with its authority inventory and non-claims. In short: project test code runs
only in a dedicated trusted helper's sandbox (new user, PID, network, IPC,
UTS and cgroup namespaces; strict Landlock at ABI ≥ 6; a seccomp allow-list;
no network; a backend-owned cgroup v2 scope with memory, swap, process, CPU
and wall limits; sealed descriptors and environment), from a verified
materialized candidate and a verified packaged toolchain, after the owner's
native approval of that exact launch. The result is advisory and bound into
the owner's review. If any required layer is missing, verification is
unavailable; there is no fallback. It does not claim a private mount view,
metadata confidentiality, a private `/proc`, or protection against kernel
vulnerabilities, root, or malicious same-uid processes.

## Security model (as far as Phase Zero establishes it)

The governing principle is that **a string is never authority**. Authority
is meant to be held by the backend and given through explicit grants that
can be narrowed and revoked. Surfaces that cannot yet be governed fail
closed instead.

This is being established surface by surface. It is not yet a system-wide
guarantee. Where the dossier and inventory record a surface as governed,
closed or withdrawn, they say so; where they record a limit or non-claim,
that limit applies.

Not claimed in Phase Zero:

- no general operating-system or WebAssembly sandbox isolates agents or
  their tools;
- no blanket "no ambient authority" property for the whole system;
- no claim that every action is cryptographically signed;
- no layered-defence guarantee beyond what the dossier records.

## Governed coding (Phase One)

Phase One adds one governed coding workflow to the desktop, on Linux:

- **Native project selection.** The backend opens the native folder picker
  itself; a path sent by the frontend is not authority. Projects and runs are
  referred to by opaque ids that grant nothing.
- **Run-bound authority.** Each run gets a fresh, read-only grant bound to
  that run. Writes to the owner's project use a separate short-lived write
  grant for one apply or restore. It is closed after each attempt, and a
  clean result requires that closure to be confirmed.
- **Local-only model path.** Coding runs use a model installed in a local
  Ollama on a loopback address, pinned into the run. There is no cloud
  fallback, and no proxy or redirect is followed.
- **Capability-bounded worker.** The model's output can only become typed,
  bounded edit proposals. The worker has no shell, process, git or approval
  capability, and no network destination other than the pinned loopback
  model. Project content, including instruction files, is task data.
- **Staged mutation and protected inputs.** Proposals change only a private
  staging copy. Paths outside the write scope, protected inputs and `.git`
  are refused.
- **Candidate-bound verification and review.** Structural verification and
  the backend-computed review are bound to the exact run, base and candidate.
- **Native owner approval.** Apply requires a backend-invoked native
  confirmation of that exact binding. A frontend or model claim of approval
  grants nothing.
- **Stale-safe apply.** Before any write to the owner's project, apply
  preflight rejects stale base files, a moved or replaced project folder,
  symlinks or other redirects, out-of-scope changes and a changed candidate.
  An owner edit that races the actual write is detected after a write may
  already have happened. Nexus puts the owner's version back where it safely
  can, or reports recovery required. It never silently overwrites the
  concurrent version and never reports clean success for an unresolved
  conflict.
- **Truthful rollback and recovery.** Pre-images are saved before the first
  write. A failure is rolled back where safe or reported as needing recovery.
  No clean success is reported without its durable ledger record.
- **Single-run restore.** A restore undoes exactly one applied run. It is
  refused if the owner changed any applied file since.
- **Time Machine "what if" closed.** It no longer changes agent fuel, agent
  state or Warden review configuration.

Not claimed for governed coding:

- Structural verification is not semantic verification or test execution.
  It does not show that the code is correct.
- There is no general shell, process or git authority in governed coding,
  and no cloud coding model.
- Multi-file apply is not an atomic filesystem transaction. An interrupted
  apply or restore needs manual recovery.
- Coding-run authority does not survive a restart.
- Governed coding is not supported on Windows or macOS.

### Hardening since the Phase One checkpoint

These changes follow the frozen Phase One checkpoint and precede the Phase
Two mission. They do not redefine the checkpoint.

- **Run read grant revoked when no longer needed.** A run revokes its
  read-only project grant as soon as it no longer needs project read
  authority: when structural verification passes (until then the run
  re-checks its project before each step), or when the run ends earlier. The
  bounded four-hour expiry remains only as a backstop. If the revocation
  fails, the run requires recovery; it is never reported as a clean outcome.
- **Time Machine replay closed.** Undo, redo and undo-to-checkpoint no longer
  restore agent state, fuel, memories or Warden review configuration. Like
  "what if", they only deny, and no Time Machine path can force an agent's
  state past the lifecycle state machine.

### Hardening after the Phase Two checkpoint (XA-R4)

These changes are on the post-Phase-Two forward line. They do not redefine
the frozen checkpoints.

- **Verifier workspaces reserved.** A folder that contains or lies within
  the verifier's workspaces (`/run/user/<uid>/nexus-verifier`) cannot be
  selected as a project, just as the Nexus state directory cannot, so an
  approved apply can never write into a live verification workspace.
- **Approval names the reviewed identity.** The native confirmation names
  the review identity the app shows beside the reviewed changes, with the
  candidate and base identities, and the approval binds that exact review.
  The native confirmation is a bounded summary the backend computes: the
  counts, up to twelve paths, and those identities. The diff itself is
  display data, rendered in the app's privileged webview. Approval binds the
  exact candidate; reviewing the diff's content relies on that webview.
- **Release authority.** No one can create, move or delete a `v*` release
  tag: a server-side ruleset blocks it, with no bypass. The release workflow
  is disabled, every workflow run must use actions pinned by full commit SHA,
  and a published release is immutable. A release needs a governed release
  mission in which the Owner re-enables the release workflow and permits that
  one tag through a recorded ruleset change, read back before and after. The
  tag is then pushed on the approved commit, and both changes are reverted
  afterwards.

### Dependencies

The Linux security gate runs pinned `cargo-audit` and `cargo-deny` with one
reviewed set of accepted advisories, recorded with reasons in `deny.toml`.
A passing gate means no advisory outside that reviewed set; it does not mean
there is no dependency debt.

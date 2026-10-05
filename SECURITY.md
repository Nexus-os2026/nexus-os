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

## Current status: Phases Zero, One and Two complete — Linux support profile

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
- That checkpoint stays frozen on `rebuild/phase1-governed-coding`. Its
  completion evidence is the `evidence/phase1-closure` branch at
  `c237937189b5977eaad01acad6a4c52bcce796cc`. Later commits do not redefine
  it: status documentation, a generated ACL schema and test hygiene commit,
  and then the entry hardening described in
  [Hardening since the Phase One checkpoint](#hardening-since-the-phase-one-checkpoint).
- Phase Two, governed verification execution, is complete for the same Linux
  support profile ("PHASE TWO COMPLETE — LINUX SUPPORT PROFILE", declared by
  the project Architect on 2026-10-04 for commit
  `dc52fadba078f2cddd6b51e26dd1a38db1ffedcc`). That checkpoint stays frozen
  on `rebuild/phase2-governed-verification`. Its completion evidence is the
  `evidence/phase2-closure` branch at
  `4966a295e395703fc8ff823413b63a32f3d069e0`. `main` does not contain the
  Phase Two implementation. Phase Three is not current and is not
  integrated.
- Linux is the active validation target for all three phases. Windows and
  macOS portability validation is deferred, and no Phase Zero, Phase One or
  Phase Two security claim is made for them.
- No Phase Zero production release, and no security-supported release, has
  been declared yet. This policy therefore lists no supported versions.
  Releases, tags and version numbers published before the rebuild predate
  Phase Zero and are not a statement of current support; they do not become
  supported releases merely because they exist.
- The current engineering evidence, including explicit non-claims, is in the
  [Phase Zero Final-Gate dossier](docs/security/phase0-final-gate-dossier.md),
  the
  [Phase Zero authority inventory](docs/security/phase0-c5-authority-inventory.md)
  and, for Phase One and Phase Two, the completion records on the
  `evidence/phase1-closure` and `evidence/phase2-closure` branches. They are
  internal engineering evidence, not an external audit or a certification.

### Historical checkpoints and current dependency support

- **Historical checkpoint validity.** The frozen Phase Zero and Phase One
  checkpoints remain complete historical engineering checkpoints. Neither is
  reopened, and their completion, evidence and recorded claims stand as
  recorded for the bytes validated at each declaration.
- **Current dependency support.** The frozen Phase Zero and Phase One bytes
  are not current shippable bytes and are not a current release candidate.
  Their lockfiles resolve Wasmtime 43.0.2, which RUSTSEC-2026-0327
  (GHSA-32h6-97mm-8q3c), disclosed after both checkpoints were frozen,
  affects. Release or deployment work must not ship directly from those
  historical snapshots.
- **Remediation.** The frozen Phase Two checkpoint and current `main`
  resolve Wasmtime 36.0.17, which RUSTSEC-2026-0327 does not affect (its
  affected range starts at 39.0.0). This is the current forward dependency
  remediation. It does not reopen or redefine the frozen Phase Zero and
  Phase One checkpoints, which stay unchanged.
- The Linux security gate passes on current `main` with its reviewed set of
  accepted advisories (see [Dependencies](#dependencies)). A passing gate is
  not proof that there are no vulnerabilities.

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

These changes follow the frozen Phase One checkpoint and are on `main`. They
do not redefine it.

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

### Dependencies

The Linux security gate runs pinned `cargo-audit` and `cargo-deny` with one
reviewed set of accepted advisories, recorded with reasons in `deny.toml`.
A passing gate means no advisory outside that reviewed set; it does not mean
there is no dependency debt.

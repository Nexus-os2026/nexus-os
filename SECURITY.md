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
  [Governed coding (Phase One)](#governed-coding-phase-one) below. Phase Two
  has not started.
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
- **Stale-safe apply.** Changed owner files, a moved or replaced project
  folder, symlinks or a changed candidate reject the apply with zero writes.
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
- Deferred hardening: a run's read-only project grant expires after a
  bounded four hours rather than being revoked when the run completes. It is
  run-bound and read-only.

### Dependencies

The Linux security gate runs pinned `cargo-audit` and `cargo-deny` with one
reviewed set of accepted advisories, recorded with reasons in `deny.toml`.
A passing gate means no advisory outside that reviewed set; it does not mean
there is no dependency debt.

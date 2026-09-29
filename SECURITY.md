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

## Current status: Phase Zero trust-boundary rebuild

- The Phase Zero trust-boundary rebuild is in progress and **not complete**.
- Linux is the active Phase Zero validation target. Windows and macOS
  portability validation is deferred, and no Phase Zero security claim is
  made for them.
- No Phase Zero production release, and no security-supported release, has
  been declared yet. This policy therefore lists no supported versions.
  Releases, tags and version numbers published before the rebuild predate
  Phase Zero and are not a statement of current support.
- The current engineering evidence, including explicit non-claims, is in the
  [Phase Zero Final-Gate dossier](docs/security/phase0-final-gate-dossier.md)
  and the
  [Phase Zero authority inventory](docs/security/phase0-c5-authority-inventory.md).
  They are internal engineering evidence, not an external audit or a
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

### Dependencies

The Linux security gate runs pinned `cargo-audit` and `cargo-deny` with one
reviewed set of accepted advisories, recorded with reasons in `deny.toml`.
A passing gate means no advisory outside that reviewed set; it does not mean
there is no dependency debt.

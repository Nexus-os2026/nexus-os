# Mother Nexus threat model

## Scope and assets

This model covers Mother's coordinator, sealed port, host-owned broker door,
native GTK4 status/Stop surface, file settlement, journal/private store, local
inference transport, inert-page checker and advisory typed-decision core.
It is not a security claim for removed applications, remote doors or a complete
autonomous intelligence. Mother is Linux-first; other platforms are unvalidated.

Assets are the Owner's files and private content, consent and grant bindings,
mission budgets, cancellation state, durable evidence and honest result labels.
The main pathway is goal → plan → governed execution → verified proof → teaching.
No other feature is authorized by this threat model.

## Trust boundaries

| Threat | Required control | Limit |
|---|---|---|
| Prompt injection or forged approval in project/model text | Treat content as data; exact contract consent; backend-issued grants and door; revalidate before effects | A model can still give bad advice; filters and typed decisions are not authority |
| Path escape, symlinks, stale identity or a changed candidate | Trusted canonical roots, retained identities, narrowing grants and pre-write checks | Path validation alone is not OS isolation; mutation is not a multi-file atomic transaction |
| Stop racing publication or journal failure | Halt before recording, admission checks, durable holds, truthful uncertain settlement and retained obligations | Cancellation cannot undo effects already performed; evidence writes can still fail or block |
| Secret leakage in records and errors | Closed evidence fields/codes, privacy canaries, separate permission-checked private content | Private content is not encrypted; low-entropy hashes can be guessed |
| Journal tampering, torn writes or crash replay | Validate chain and state transitions; refuse poison/torn or inconsistent history; hold unfinished missions | An unkeyed chain cannot authenticate a rewritten history or detect all valid-prefix truncation |
| External inference egress, changed model or late reply | Pinned loopback transport and local-only identity, budgets, bounded cancellation, reject late results | Conversation route is closed pending qualification; no cloud path is implicitly authorized |
| Active content or exaggerated page claims | Bounded inert policy, facts report, independent reader, explicit not-rendered result | No rendering, viewport, layout or general semantic proof |
| Typed advice becomes permission | Code-owned questions, strict decode, one-use slots, deterministic checks dominate advice | T1 has no live model I/O; confidence is not a safety guarantee |
| Legacy services or code alter Mother | Isolation scans, explicit broker boundary, no old actuator/state imports | Retained libraries are not permission to resurrect old routes |
| Hostile native desktop client spoofs consent | Backend-bound native confirmation and fail-closed missing dialogs | No authenticated secure attention against a hostile same-user desktop client |

## Host and execution assumptions

The Owner controls the host and protects Mother's home. Root/admin, kernel
compromise and arbitrary same-UID attacks against user-owned state are outside
the claimed boundary. Do not describe these assumptions as enforced isolation.
The desktop is native GTK4 and embeds no browser engine. Planned voice, messaging,
paired devices, remote control and cloud transports are not present protections.

The retained verifier's all-or-nothing isolation contract is documented in the
[verifier boundary](docs/security/phase2-governed-verification.md). Missing Linux
layers, packaged identity or qualified host mean unavailable, never an
unsandboxed fallback. That foundation does not expose arbitrary process
execution to Mother. The [broker boundary](docs/security/phase3-governed-real-world-control.md)
separates available Mother routes from retained mechanisms.

## Evidence and incident handling

Tests and source guards are evidence with limits, not a blanket security proof.
Installed-build qualification and Owner physical acceptance are required for
capability claims. See [SECURITY.md](SECURITY.md) for non-claims and private
reporting. On an incident: Stop and revoke affected authority, preserve evidence
without private payloads, assess integrity, rotate compromised credentials through
the approved mechanism, and recover only under a bounded approved mission.
Do not silently restart uncertain work or claim recovery before it is verified.
After recovery, document the root cause and update the threat controls under
the approved mission; an incident does not grant release or security-core
change authority. Any future authorized update path must verify artifact
authenticity/provenance and resist rollback/freeze attacks before deployment;
the removed TUF/update implementation is not a present Mother protection.

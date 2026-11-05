# Mother Nexus security policy

## Reporting vulnerabilities

**Do not put vulnerabilities, secrets or private reproduction data in public
issues.** The existing project contact is **security@nexus-os.dev**. Contact
delivery and response times are not verified by this repository; no response SLA
or security-supported release is claimed. Send a minimal description, affected
build, reproduction and impact without live credentials.

Reports include authority forged through strings, paths or saved records;
closed routes becoming reachable; grant, Stop, halt, revocation or expiry
bypasses; false success after uncertain effects; privacy leaks; and audit or
journal integrity failures. A successful-looking UI must never conceal a
failed trust operation.

## Current boundary

Mother Nexus is Linux-first and is not production-ready. Implementation,
local tests, CI, installed-host qualification and Owner physical acceptance
are separate evidence. Current acceptance comes from the approved mission and
Git, not a copied phase table, version number or old release. No external audit,
SOC 2, NIST or EU AI Act certification is claimed.

A working unsafe feature is worse than an unavailable safe feature.

A string is never authority. The coordinator has a sealed effect port; the host
owns the broker and issues an Owner-confirmed agent door. Consent binds the
mission contract but does not create an effect grant. Each effect still requires
its backend authority, current binding, budget and approval. Missing or uncertain
authority refuses; there is no compatibility route to the retired application.
The old Nexus must never break Mother Nexus.

The native GTK4 status window has Stop but declines mission, door and resume
dialogs until they are implemented. It embeds no browser engine. Native
confirmation is not authenticated secure attention against a hostile same-user
desktop client or synthetic input. Root/admin and a compromised kernel are
outside the isolation claim.

## Stop, Files and recovery

Halt and Stop prevent subsequent admission before waiting for records; errors
remain observable. Cancellation is cooperative at executing actuators and
cannot undo effects already performed, a request already sent or bytes already
published. Cancellation is not containment.

Files has separate session admission, publication, settlement and acknowledgement.
An unknown or interrupted publication remains uncertain; it must not become
ordinary approval, lapse or clean success. Unsettled obligations are retained,
not evicted to admit new work. Recovery refuses ambiguity. Multi-file mutation
is not an atomic filesystem transaction. No blanket automatic rollback,
restart-safe authority or general shell permission is claimed.

## Privacy and evidence

Journals and broker evidence record bounded identifiers, hashes, sizes, counts
and closed codes. Contracts and working text live in Mother's permission-checked
private store instead. Missing or corrupt private content stops the mission;
content never becomes authority. The private store is **not encrypted**, and
retention/purge is not implemented.

Hash chains are **unkeyed**. They detect malformed or altered linked records,
but a writer who replaces the entire chain or truncates to an earlier valid
head can evade that check. They are not authenticated memory or an external
anchor. Low-entropy digests can be guessed. Historical in-memory audit facilities
are not proof of durable Mother evidence; use the actual journal/evidence
receipts for the build under test.

## Model and page boundaries

The local transport pins loopback endpoint, model and runtime identity,
local-only configuration, limits and cancellation. The conversation route
remains shut until installed qualification. A local inference allowance is not
permission for external egress. Cloud escalation is not enabled by the presence
of legacy provider code: it requires its own implemented broker route, explicit
consent and budget, never a silent fallback. Model output and project instruction
files are data, never authority.

Typed decisions T1 is a pure advisory core. It does not call a model, authorize
an effect or make a failed deterministic check pass. A confidence value is not
permission or proof of correctness.

The page checker accepts a bounded inert language, extracts facts and compares
an independent reader. A pass is **not rendered** and does not prove layout,
viewport appearance, every sentence or the complete site mission.

## Retained foundation and non-claims

Path validation is not an OS sandbox. Workspace grants are not subprocess
confinement; process groups are not filesystem or network isolation. No general
agent sandbox, blanket absence of ambient authority, universal cryptographic
action signatures or unrestricted host protection is claimed.

The retained verifier requires **all** mandatory Linux layers, a qualified host,
verified packaged toolchain and installed helper. Otherwise it is unavailable,
with no degraded mode. It does not claim a private mount view, private `/proc`,
host-path metadata confidentiality, or protection against root, kernel compromise
or malicious same-UID host processes outside retained identities. Mother has not
thereby acquired a general-purpose code-execution capability. See
[verifier boundary](docs/security/phase2-governed-verification.md),
[broker boundary](docs/security/phase3-governed-real-world-control.md) and
[THREAT_MODEL.md](THREAT_MODEL.md).

Windows and macOS are deferred, unvalidated targets. Frozen historical
checkpoints and their lockfiles are not current shippable bytes; their historical
evidence is not reopened or redefined by this cleanup. Retired release,
installer and update mechanisms are not supported Mother distribution paths.

## Dependencies and releases

Use `Cargo.toml`, `Cargo.lock`, `deny.toml`, the audit script and the required
CI on the exact candidate for dependency evidence. Do not copy a transient
version or advisory list into this policy. A passing audit means no unaccepted
finding under that run's reviewed policy, not zero dependency debt.

Security-core changes require independent review and Owner approval under
[MOTHER.md](MOTHER.md). A release, tag, deployment or host change requires its
own explicit authorization. No workflow or guard may be weakened to manufacture
green. This documentation makes no claim about current remote rulesets or
published artifact integrity without verified evidence.

The inherited release discipline remains a requirement: no creating, moving or
deleting `v*` release tags outside an Owner-authorized release mission; release
workflows stay disabled until that mission explicitly re-enables them; actions
must be pinned by full commit SHA; published releases are immutable. Any temporary
ruleset/workflow exception must be recorded, read back before and after, limited
to the one approved tag/commit, then reverted. This is a constraint, not a claim
that this session has inspected remote settings. Historical release evidence
cannot authorize a Mother release.

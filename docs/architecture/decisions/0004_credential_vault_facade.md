# Retained credential facade: storage is not authority

This historical ADR path remains because `kernel/src/secrets/mod.rs` references
it. The old app migration plan and removed provider/UI work are in Git history;
none is a Mother mission or current deployment claim.

`kernel/src/secrets/` retains a synchronous `SecretsFacade` over environment,
OS keyring, SQLite envelope and test memory backends. `CredentialFacadeConfig`
controls per-scope lookup order. Backend availability or a resolved secret is
not an effect grant. Do not expose this facade through Mother, reconstruct
removed commands or treat its legacy audit context as capability enforcement.

The SQLite backend uses an existing configured encryption key and identity-bound
authenticated data. Legacy unbound rows are refused on the read path; a separate
planned upgrade handles migration. Missing trusted key/configuration must fail;
there is no permission to create keys, roots or host services on a lookup failure.
Read actual startup/configuration code for the build under review rather than
assuming the original bootstrap sketch is current behavior.

Secret values use zeroizing owned buffers and must never enter logs or evidence.
That does not guarantee zeroization of copies in external HTTP/TLS libraries.
The facade's legacy audit appender logs an append failure and continues; this is
a limitation, **not an approved pattern for Mother** or proof of fail-closed,
durable evidence. The in-memory kernel audit is not Mother's durable recorder.

Mother uses the deterministic broker's door and bound credential-lease boundary;
the broker must revalidate agent, run, commitment and origin before release.
Saved strings or provider names grant nothing. No cloud provider or remote vault
is enabled for Mother by this retained storage code. New contact requires the
explicit security-core approval and independent review in MOTHER.md.

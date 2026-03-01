# NEXUS OS Privacy Design

## Data Categories

1. Agent manifests
- Stored encrypted at rest.
- User-requested deletion is supported through cryptographic erasure.

2. Audit logs
- Append-only chain with hash integrity.
- Sensitive payload fields are encrypted before persistence.
- Integrity proofs remain valid after key destruction because ciphertext and hashes are retained.

3. Secrets and API keys
- Protected with AES-256 encryption.
- Never logged in plaintext in audit events or debug logs.

4. Screen captures
- Ephemeral by default.
- Deleted after processing unless explicit retention is configured.

5. User preferences
- Stored encrypted at rest.
- Fully deletable through key erasure workflow.

## Deletion Workflow

1. User submits deletion request.
2. System resolves all data classes linked to the user.
3. Encryption keys for those records are destroyed (`DEK` and relevant wrapped material).
4. Encrypted records become computationally unrecoverable.
5. Audit-chain integrity remains verifiable because hash links over ciphertext are preserved.

## Operational Notes

- Key destruction is irreversible by design.
- Recovery procedures must not resurrect deleted user material.
- Retained audit records must contain no plaintext secrets.

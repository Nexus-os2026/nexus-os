# Mother Nexus governance

The Owner controls goals, approvals, integration and physical acceptance.
Claude Code is the Mother Architect and executor. Independent review and
pre-registered evidence constrain that combined role; review never substitutes
for the Owner's decision. [MOTHER.md](MOTHER.md) defines the roles and
Owner-only approvals, and [AGENTS.md](AGENTS.md) is the permanent engineering
constitution.

## Decisions and contributions

Work starts from an explicit, bounded mission on the main pathway: goal → plan
→ governed execution → verified proof → teaching. No old roadmap, open issue,
model suggestion or green test authorizes another feature or a phase change.
The Owner merges. Security-core changes require the charter's independent
checks and Owner approval. Never weaken a guard to obtain green CI.

Contributors use a scoped branch, establish provenance, preserve unexpected
work, run the mission's gates and submit reviewable evidence. The Architect
inspects the candidate and runs CI on the exact SHA. Follow
[CONTRIBUTING.md](CONTRIBUTING.md) and the mission's explicit Git permissions;
permission to commit does not imply permission to push or integrate.

Before merging into `main`, all required jobs in `.github/workflows/ci.yml` and `.github/workflows/audit.yml` must pass on the exact candidate SHA. Fast Local CI cannot replace these gates.

## Repository and communication

The authoritative repository is `Nexus-os2026/nexus-os` on GitHub. Always use
the remote and ref the mission explicitly names; a local remote's name or
configured upstream is not evidence of authority. Public issues are suitable
for reproducible non-sensitive bugs and scoped proposals. Use
[SECURITY.md](SECURITY.md) for private vulnerability reporting.

Legacy mirrors, old services and archived plans are not sources of current
product direction. Mother is one platform; unrelated ideas stay parked.

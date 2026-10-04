# Phase Two closure evidence (branch `evidence/phase2-closure`)

This branch records the completion evidence for Phase Two of the Nexus OS
rebuild. It starts at the frozen Phase Two engineering checkpoint
`dc52fadba078f2cddd6b51e26dd1a38db1ffedcc` and adds only this directory,
`evidence/phase2-closure/`. It is never merged into the frozen checkpoint or
into `main`, so recording evidence here does not change any validated source
tree.

Three things are kept apart:

1. **Engineering checkpoint.** The exact validated bytes are the frozen
   branch `rebuild/phase2-governed-verification` at `dc52fadb…` (tree
   `bb4eb1c7d1137c907f0ace14269b2e1a747881cb`). This branch does not move.
2. **Completion evidence.** This directory.
3. **Public `main` status and integration.** These are separate and require
   their own Architect mission. `main` (`4b36f60694148b60029733bc7b3d26e5a215d460`
   at closure) does not hold Phase Two.

Everything here is a claim to verify against Git and GitHub. Nothing here is
approval authority. Only the Architect approves, and only the Architect
declared Phase Two complete.

| File | Content |
|---|---|
| `PHASE-TWO-COMPLETION-RECORD.md` | Source identity, the delivered trust boundary, validation, the deep local audit, the security repair, known warnings, non-claims and closure |
| `PHASE-TWO-ARCHITECT-DECLARATION.md` | The Architect's declaration, PHASE TWO COMPLETE — LINUX SUPPORT PROFILE, for `dc52fadb` (2026-10-04) |
| `RUNS.md` | The closure run ledger (run ID, workflow, event, attempt, SHA, result, role), failures and cancellations included |
| `DEEP-LOCAL-AUDIT-REPORT.md` | Byte-for-byte copy of the P2-CLOSE-A1 deep local reality audit report (SHA-256 `8724976a7c56cdc9f873119adcda52950228de241029b0806a49f2824c615a5c`) |
| `DEEP-LOCAL-AUDIT-COMMANDS.txt` | Byte-for-byte copy of that audit's command transcript (SHA-256 `1ecef01e8a6a3ba073817b15ffc85217865d237d323cb42f493041419e09492a`; it keeps one historical trailing space at line 47, see the completion record §I) |
| `DEEP-LOCAL-AUDIT-ORIGINAL-SHA256SUMS.txt` | Byte-for-byte copy of that audit directory's own manifest (SHA-256 `d4e6113dc8064cfc0fad7ecea52856eab65ea493def31f5c15fe08cc3cbaa590`). It is historical evidence of the external audit directory, not this directory's manifest. |
| `OWNER-WORKSPACE-OBSERVATION.txt` | The Owner's uid-1001 observation that resolved `A1-BLOCKER-WORKSPACE-OBSERVATION` |
| `SHA256SUMS` | SHA-256 of the eight files above (`sha256sum -c SHA256SUMS` from this directory) |

No secrets, environment dumps or credential-bearing logs belong here.

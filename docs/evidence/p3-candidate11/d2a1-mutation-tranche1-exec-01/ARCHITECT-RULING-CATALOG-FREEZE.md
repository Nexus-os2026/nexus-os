# Catalog-freeze Architect ruling — record

Mission: P3-C11-DR01-D2A1-MUTATION-TRANCHE1-EXEC-01 (Claude Code, Claude Opus 5.5, `claude-opus-5-5`, effort High).

This file records, as received, the ruling of the mission's section 1 under which this mission executes. It is a
record of the ruling, not a ruling: the authority is the Sole Architect's, relayed by the Owner. The executor claims
no acceptance of any control or kill.

## The ruling (mission section 1, verbatim)

> The complete D2A-1 control catalog has passed independent review and is now:
>
> D2A-1 CONTROL CATALOG V1
> ARCHITECT ACCEPTED AND FROZEN
> EXECUTION PENDING
>
> All 35 kill-required control definitions are accepted exactly as identified by this catalog.
>
> A4 remains separately: SATISFIED — ARCHITECT ACCEPTED
>
> The catalog's old candidate/pending strings are historical artifact state. This Architect ruling supplies acceptance
> authority.
>
> No patch, property, configuration, receipt type, restoration mapping or attribution rule may be changed in this
> mission.
>
> Any required definition change stops the mission.

## The frozen catalog it names (authenticated in `receipts/00-authentication.txt`)

| Item | Identity |
|---|---|
| Path | `docs/evidence/p3-candidate11/d2a1-control-definitions-r2-01/catalog/catalog.json` |
| Version recorded in the artifact | `d2a1-controls-v1-r2-candidate` |
| Git blob | `1918374263740d2f98640da00fc466737945975e` |
| SHA256 | `d6c05a7cdc71b3ca1812b5199b8529cbe6db0363ca11bacc26d1f21c2cd11a83` |
| Evidence commit | `9f22f890d8532df395f22cd8db8db2552bfdd14c` |
| Review ref | `refs/heads/review/p3-c11-d2a1-control-definitions-r2-01` |
| Definitions record | `ARCHITECT-CONTROL-DEFINITIONS-R2.md`, blob `c79b75b65b85e1fad53592e92ab2d8507d6c8799`, sha256 `1da1ae92f313496dade2bfea5c508af1e17bf2e39b9bd11cca3e448c35f0cb26` |

The artifact's own strings (`catalog_status: CANDIDATE — INDEPENDENT ARCHITECT REVIEW PENDING`, per-entry
`architect_review_status: pending`, `execution_status: NOT_ATTEMPTED`) are historical artifact state, as the ruling
says. This mission changes no byte of the catalog, its patches or its definitions record; the final gate re-reads
them from the R2 evidence commit and compares them.

## What this mission executes under it

Exactly the twelve controls of mission section 2, in the order of section 9: C22, C14, C19, C21, C27, C28, C12, C9,
C11, C17, C18a, C18b. Every patch, property, configuration, receipt type, test selection, ceiling, attribution rule
and restoration mapping is read from the frozen catalog (`receipts/02-EXPECTATIONS.json` derives them and cross-checks
them against Candidate C's sources). Any result produced is KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING at
most: the Sole Architect reviews each receipt.

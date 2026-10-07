# Nexus review procedure

Canonical provider-neutral workflow. Read [POLICY](../POLICY.md); this procedure does not grant authority.

## WHEN TO USE

Independent review of an exact candidate and evidence set under a bounded review mission.

## REQUIRED INPUTS

Original authorization, MISSION, base and candidate SHA/tree, changed-file diff, receipts/artifact manifest, REVIEW_TEMPLATE.

## PRECONDITIONS

Reviewer is independent of implementation for an independent verdict; review scope and permitted reproductions are established.

## ALLOWED ACTIONS

Read code/evidence, independently reproduce authorized checks, report findings bound to exact identity.

## FORBIDDEN ACTIONS

Implementer self-acceptance; relying on summaries alone; silently repairing code; phase/integration approval beyond review authority.

## STEPS

1. Bind REVIEW to exact SHA/tree, base, mission and changed files. Refuse ambiguous identities.
2. Read original authority, canonical policy and acceptance criteria. Inspect raw receipts, command mappings, hashes and omitted/error results.
3. Independently reproduce authorized checks; record separate receipts with reviewer identity/context. Do not relabel implementer tests as independent.
4. Review authority ownership, lifetime, cleanup, error propagation, fail-closed behavior, race windows, stale identities, fallback, privilege boundaries and platform/test validity.
5. Record all findings and unresolved issues. Use ACCEPTABLE_FOR_NEXT_GATE, CHANGES_REQUIRED, BLOCKED or INCONCLUSIVE.
6. An acceptable result is bounded to the next gate; no phase closure, merge or next mission follows automatically. Implementers may prepare only a pending/INCONCLUSIVE review record.

## OBSERVABLE OUTPUTS

Exact-candidate independent assessment and criterion-specific findings or explicit pending status.

## REQUIRED RECEIPTS

Inspected artifact IDs/hashes and independently executed tests; separate reviewer observations from implementer claims.

## STOP CONDITIONS

Independence unavailable; candidate/evidence mismatch; missing required evidence; proposed action outside review authority.

## HANDOFF OUTPUT

REVIEW with bounded verdict, evidence inspected, independently reproduced tests and next authority boundary.

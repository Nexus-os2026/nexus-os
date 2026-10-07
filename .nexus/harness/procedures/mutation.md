# Nexus mutation procedure

Canonical provider-neutral workflow. Read [POLICY](../POLICY.md); this procedure does not grant authority.

## WHEN TO USE

Explicitly approved mutation campaigns and logical control coverage reporting.

## REQUIRED INPUTS

Campaign authorization, exact candidate, complete control definitions/hashes, runner/supporting inputs, prerequisites, build/source restoration plan, RECEIPTS contract.

## PRECONDITIONS

Campaign scope including temporary mutations is explicitly permitted; pristine candidate and intended-test baseline are proven.

## ALLOWED ACTIONS

Only approved controls in defined scope, bounded rebuild/restoration and evidence recording. Deduplicate shared prerequisite commands through explicit mappings.

## FORBIDDEN ACTIONS

Using prerequisite passes as mutation kills; counting duplicate stdout markers; changing semantics to pass; assembling partial runs as one complete campaign.

## STEPS

1. Pin campaign ID, candidate SHA/tree, control inventory and definition hashes. Identify support files, build target/cache and environmental prerequisites.
2. Run intended-test baseline. Record command_execution_id → control_ids for each deduplicated prerequisite; evaluate logical coverage even if stdout names only one test.
3. Execute each authorized mutation case and record its mutation ID/hash, actually mutated control IDs, baseline execution reference, oracle result and infrastructure status. One command can cover multiple logical properties of the same mutation; it cannot stand in for unexecuted distinct mutants.
4. Use KILLED, SURVIVED, INVALID, HARNESS_ERROR or NOT_RUN. A nonzero exit alone is never a kill: require an expected semantic oracle and valid baseline. Compiler/setup/timeouts are errors/invalid according to the approved oracle, not automatic kills.
5. Restore exact source and verify hashes AND fresh execution/build artifacts. Source restoration alone is insufficient. Enforce the approved timestamp/fingerprint/rebuild barrier; scoped clean fallback needs mission authority.
6. Stop on uncontrolled error, restoration failure or scope escape. Preserve partial results as partial under that campaign ID.
7. Report prerequisite command count, logical prerequisite coverage, mutation executions and per-control outcomes separately. Require exact coverage of the full approved inventory in ONE campaign before calling it complete.

## OBSERVABLE OUTPUTS

Pinned manifest; prerequisite mapping; individual mutation/restoration results; explicit complete/partial campaign assessment.

## REQUIRED RECEIPTS

Baseline commands, every mutation oracle, mutation definition hashes, restoration/freshness checks and final pristine validation; raw logs and exact campaign/candidate IDs.

## STOP CONDITIONS

Uncontrolled infrastructure error, source/build mismatch, failed restoration, scope escape, unapproved semantic adaptation.

## HANDOFF OUTPUT

Campaign matrix with separate status domains and unrun controls, not an acceptance decision.

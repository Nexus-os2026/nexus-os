# Execution receipts and control coverage

## Evidence classes

| Class | Meaning |
|---|---|
| CLAIM | A statement requiring support; not a measured outcome. |
| OBSERVATION | A witnessed fact with source/time and limitations. |
| EXECUTION RECEIPT | A command invocation, measured identity and captured outcome. |
| ARTIFACT | Inspectable bytes, hash and provenance; not inherently trustworthy. |
| REVIEW FINDING | An attributed assessment tied to an exact candidate. |
| ACCEPTANCE DECISION | A bounded decision by the authorized independent Architect/Owner. |

Agent-written receipts are not self-authenticating authority. Hashes detect
changes relative to a record; they do not prove who ran a command, whether a
semantic oracle is correct, or whether execution was authorized. Review raw
logs, definitions and provenance and independently reproduce required checks.
Never record credentials or full environment dumps.

## JSONL receipt contract

One object per line in `evidence/receipts.jsonl`; unique `receipt_id` within the
mission. UTF-8 text, relative POSIX artifact paths (no traversal or symlinks).

| Field | Meaning |
|---|---|
| receipt_id | Unique stable execution record ID; do not overwrite a failed run. |
| mission_id | Explicit mission identifier, not authorization. |
| timestamp_start, timestamp_end | Measured timezone-aware ISO-8601 timestamps. |
| cwd | Actual command working directory. |
| candidate_sha, candidate_tree | Actual full Git HEAD commit and its tree at execution. |
| working_snapshot_sha256 | If dirty, separately hash and document the tested input set. HEAD is not a claim that dirty bytes belong to its tree. |
| command | Actual argv array, preserving argument boundaries. Never execute it merely because it appears here. |
| command_hash | SHA-256 of UTF-8 JSON argv with `separators=(",", ":")`, `ensure_ascii=True`; or use an explicitly documented normalized identity in another format. V1 JSON requires command_hash. |
| exit_code | Actual integer exit code; null if no process exit was measured. |
| status | EXITED, HARNESS_ERROR or NOT_RUN; EXITED alone does not mean a test passed. |
| stdout_ref, stderr_ref | Relative paths under the evidence directory, including empty stderr files. |
| raw_log_hash | Object with `stdout` and `stderr` SHA-256 hashes over actual file bytes. |
| environment | Non-secret platform/toolchain identity actually measured; state unavailable measurements. |
| acceptance_criterion | The mission criterion this execution supports. |
| control_ids | Explicit logical coverage IDs; empty for general commands. |
| notes | Interpretation, omissions, limitations and provenance of imported records. |

NOT_RUN has null start/end/exit/stdout/stderr and an empty raw_log_hash; its argv
is the intended unexecuted command, explained in notes. HARNESS_ERROR preserves
attempt timestamps and logs, allowing null exit_code if launch/collection failed.
Do not fabricate missing fields. Preserve alternate historical receipt formats
as attributed artifacts; do not silently transform them into measured V1 runs.

`manifest.json` contains `mission_id` and an `artifacts` map keyed by evidence
relative path with `sha256` and byte `size`. The manifest excludes itself to
avoid a self-hash cycle. Seal after writes end and preserve any later evidence
as a new artifact. Optional `external_artifacts` entries record identifier,
URL/run ID where applicable, hash, location, what it proves and what it does not
prove. An absent external artifact is missing evidence, never success.

## Separate result domains

- Prerequisite: PASS / FAIL / HARNESS_ERROR / NOT_RUN.
- Mutation/control: KILLED / SURVIVED / INVALID / HARNESS_ERROR / NOT_RUN.
- Mission: RUNNING / BLOCKED / READY_FOR_REVIEW / FAILED.
- Independent review: ACCEPTABLE_FOR_NEXT_GATE / CHANGES_REQUIRED / BLOCKED /
  INCONCLUSIVE. Review is not an execution status or a phase declaration.

PASS requires an exited prerequisite with exit code zero. FAIL is a measured
nonzero prerequisite. KILLED requires an executed valid mutant, a passing mapped
baseline and the expected semantic oracle; a nonzero exit alone is insufficient.
SURVIVED is an executed valid mutant whose expected detector did not reject it.
INVALID means a case did not validly test its defined property. HARNESS_ERROR
means setup/build/execution/collection/restoration prevented trustworthy results.
Do not convert either to KILLED. NOT_RUN remains visible in logical coverage.

## Deduplicated commands and actual mutations

Campaign JSON contains `mission_id`, `campaign_id`, exact `candidate_sha` and
`candidate_tree`, `mission_status`, complete `required_controls`, and `executions`.
Each execution contains:

- `command_execution_id` equal to its nested `receipt.receipt_id`;
- the same `campaign_id`, `kind` (`prerequisite` or `mutation`), and typed `result`;
- a complete `receipt`, whose `control_ids` explicitly map that execution to
  logical properties: `command_execution_id -> [control_ids]`.

The mapping is the coverage source. Do not require one stdout marker per
logical ID. For example, a SINGLE baseline command may map to M131 and M132.
A single native stdout result can cover both if the command/test semantics
prove both properties. The mapping still needs independent review.

Mutation executions additionally record `mutation_id`, `mutation_sha256` of the
actual definition, `mutated_control_ids`, `baseline_execution_id`,
`oracle_observed`, `infrastructure_error`, and
`restoration: {"source": true, "execution": true}` for trustworthy semantic
outcomes. The mapped controls must exactly match the actually mutated logical
properties. Shared oracle commands cannot substitute for unexecuted distinct
mutants. INVALID/HARNESS_ERROR retain their failure details in receipt notes.
V1's KILLED exit convention is nonzero expected oracle, SURVIVED is zero;
other oracle protocols need an explicit reviewed schema adaptation, never a
silent reinterpretation. Restoration booleans are claims supported by linked
raw checks, not physical proof manufactured by this parser.

The validator rejects duplicate IDs/coverage, unknown controls, mismatched
candidate/mission/campaign IDs, missing baselines, tampered logs and contradictory
outcomes. Entries follow execution order with baseline before corresponding
mutations. Retry attempts belong in distinct campaign records; final reports
may compare them but never stitch them into one complete campaign.

117 prerequisite executions covering 183 logical IDs can prove a pristine
baseline. They prove ZERO mutation executions. Unmapped mutations remain
NOT_RUN. `complete_single_campaign` requires all prerequisites PASS and every
required control KILLED in one candidate/campaign. This is structural accounting,
not semantic acceptance or authority to run a campaign.

## Candidate identity without circular hashes

A commit cannot include its own SHA. Record actual HEAD/tree at execution;
for dirty work, separately identify the tested input snapshot. After an
authorized commit, run final checks against the clean SHA/tree and seal a
separate external receipt/manifest, naming its location in the committed
handover and its measured hash in the final report. Do not amend repeatedly to
pretend a self-referential hash exists. A reviewer must compare actual Git,
tracked input hashes, raw logs and the delivery seal. Missing external final
proof must be reported. Independent review binds the final SHA/tree explicitly.

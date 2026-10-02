# Complete-module self-review (P2-V1-R3B-I3-I1-R2, mission section 16)

**Scope.** The whole custody store, `crates/nexus-verifier-sandbox/tests/support/custody/store/`
(12 files including `mod.rs`, 14,597 lines), as of the candidate snapshot (tree
`5b396d8303c0ebb0641661cd16885f4645ce4c36`). It covers more than the R2 diff.

**Method.** Each pattern of section 16 was searched for over every file:
- `pub` items and fields;
- functions that take reports, identities, digests or strings;
- every caller of `request_seal`, `open_owner`, `verify_standalone`,
  `begin` and `start_owner`;
- every use of the bounded report's `detail`;
- every procedure's verification, gate and verify-after.

Findings that needed a change were fixed in R2; the rest are recorded with
their reason.

## Classification

| Kind | What | Where |
|---|---|---|
| DATA | Bytes on disk and what parses from them: journal frames, headers, seals, `PROVISION` (`Provision`), dispositions (`Disposition`), archive names, the storage record | `format.rs`, `classify.rs` (parsers and per-file classifier) |
| REPORT | Copies a caller may read and edit, which authorize nothing: `FileReport`, `PoolLevel`, `Decision`, `BoundedReport`, `ScanResult`, `StartupReport`, `Selection` copies, `SessionReport`, (R2) `Assessment` and `Condition`, `SessionEvent`, `RecorderStatus`, `Snapshot`, `ClaimState`, `SealState` | `classify.rs`, `open.rs`, `maintenance.rs`, `exchange.rs` |
| OWNER INPUT | The Owner's own establishment or intent: `Qualification`, `Layout`, `OwnerWords` (reason, statement, operator, time), (R2) the acceptance list of Invalid condition names, a revocation's binding and time, archival, recycling and retirement indices, and the interrupted session's saved report (R5 section 13.8) | `maintenance.rs` |
| RETAINED AUTHORITY | Private fields of values a caller cannot build:<ul><li>`StoreOwner`: custody, recorder, header, worker, guard;</li><li>`ClosedStore`;</li><li>`Opened`: identity, selection, scan, decision, admission;</li><li>`StorageAdmission`;</li><li>`StoreValidator`, a borrow of an opening;</li><li>`Session`: lock and journal-lock descriptions, selection, opening, admission, mutation count;</li><li>(R2) `Verification`, a private type, with the epoch, opening, root id, revision, digest and state it is bound to</li></ul> | `owner.rs`, `open.rs`, `disposition.rs`, `maintenance.rs` |
| TRANSITION | Internal steps that read only retained authority:<ul><li>the opening's claim;</li><li>disposition application at start;</li><li>closure to seal;</li><li>the recorder's steps;</li><li>(R2) `take_verification`, `take_complete`, `take_refusal` and `take_assessment` (one-shot and bound);</li><li>each procedure's gate and verify-after;</li><li>`succession_conditions` over the session's own assessment</li></ul> | `owner.rs`, `open.rs`, `recorder.rs`, `maintenance.rs` |

No DATA, REPORT or OWNER INPUT value becomes retained authority by matching.
- The Owner's acceptance is compared with the verified set and authorizes
  nothing alone: the verification must also be the session's own, current
  and complete.
- A disposition binding must be one the verification reported.
- R-RESUME's saved report is checked only against the session's own refusal
  and the session's own reads.

## Section 16 searches

| Pattern | Result |
|---|---|
| Public constructors of authority-bearing types | `start_owner`, `open_owner`, `verify_standalone`, `maint::begin` (and `Session::begin`) and `open_configured_store` (closed: uninhabited success type). Each performs its own selection, lock and revalidation. `Verification` has no constructor outside `Session::verify`, and its type is private (API probe P-VERIFICATION-FORGE). `Findings::collecting` and `Findings::first_only` are `pub(super)`. `Procedure::new` is private. |
| Public mutable fields or guards | Public fields exist only on DATA, REPORT and OWNER INPUT types. `Procedure.name` is a public label used only in messages. `Session`'s fields, including `verified` (API probes P-SESSION-VERIFIED, P-VERIFICATION-TRANSPLANT), are private. |
| Caller IDs, digests or reports used as expected authority | None. P-DISP takes a binding and refuses unless the session's verification reported it (v15; NC-DISP-UNREPORTED). Successor root ids are Owner input and checked as new. `resume_recycle`'s saved report is Owner input, as R5 section 13.8 states. `archival_allowed` and `retirement_allowed` are pure predicates; the procedures apply them to the session's own report. |
| Copied decisions later used to authorize mutation | None. Procedures take the session's retained verification (R1). R2 extends this to disposition publication and succession, and gives copies only as `Assessment` (v11). |
| Strings used as proof | None after R2. The base's free-text predecessor statement (B-S2) is replaced by a statement generated from the verified set (v08; NC-SUCC-STATEMENT-CALLER). Owner text in `OwnerWords` is recorded and proves nothing. |
| Verify results reused after mutation | None. The mutation-count binding (R1) is kept, and R2 adds the selection binding. A verify-after clears the verification (v05; NC-VERIFY-AFTER-AUTHORIZES), and every procedure's gate refuses a step that ran since it was built (v12; NC-SUCC-NO-GATE). |
| Procedures that mutate without consuming required authority | P-REVOKE, P-REQUALIFY and R-LEFTOVER consume no verification, by design: see the authority matrix's non-repairs. Every one holds the session's lock and has the gate and the verify-after. P-PROV and R-REPUBLISH have no session (R5) and refuse while `PROVISION` exists (v17). |
| Success claimed before verify-after | None after R2. `Procedure::run` returns success for the last step, and `is_complete` turns true, only after the verify-after (v02, v19). A session cannot end while a procedure holds it (v24). |
| Truncated reports used for decisions | None. `BoundedReport.detail` is built only for display (`bounded_report`), and no decision reads it (searched). Succession counts the complete set (v03; NC-SUCC-REPORT-TRUNCATION). |
| Capacity treated as permission | None. P-DISP and P-ARCH run over Capacity, because R5 section 13.10 names dispositions and archival as its resolutions. Succession runs over Capacity only with a complete proof (v06, v07; NC-SUCC-CAPACITY-BYPASS, NC-GAP-UNBOUNDED, NC-SUCC-CAPACITY-REFUSED). Recycling and retirement keep R1's rule: no condition. |
| Invalid treated as permission | Succession needs the Owner's exact acceptance (v08; NC-SUCC-OMIT-INVALID, -INVENT-INVALID, -REPEAT-INVALID). P-DISP runs over Invalid so that an Invalid predecessor's incidents can be dispositioned (R5 section 13.6), from verified facts only. |
| Lock release before final verification | None. The session retains its locks until `end`, which needs sole ownership of the session (v24). Succession holds both stores' locks from adoption through the verify-after (v13; NC-SUCC-DUAL-LOCK). |
| Alternate sealing paths | None. `request_seal` is `pub(super)`, and its only caller is `StoreOwner::close` (owner.rs; R1, unchanged). |
| Alternate store opening paths | None new. The paths are `open_owner`, `start_owner`, `verify_standalone`, `maint::begin` and the closed `open_configured_store`. R2 changed how the session scans (collecting), never how a store is opened. |

## Defects this review found and R2 fixed

- **Indeterminate taken as determinate.** An entry that could not be listed or
  stat'ed in `dispositions/revoked/` or `dispositions/` was recorded as a
  determinate Invalid condition. It is now Indeterminate (v20;
  NC-ENTRY-STAT-DETERMINATE).
- **One name, two conditions.** A name recorded as unexpected or leftover was
  also examined as an entry of its directory. A stray archive name made the
  verification Indeterminate, and a stray disposition name was reported
  twice. Only accepted names are examined now (v23; NC-NAMES-EXAMINED-TWICE).
- **A verify-after that could not fail.** The generic verify-after failed only
  on lost authority. It now also fails while a leftover temporary remains
  (v21; NC-LEFTOVER-AFTER-ACCEPTED).
- **Recycling and retirement over Capacity.** They had been widened to run over
  Capacity, which R5 section 13.10 does not support. They are back to R1's
  rule.
- **A test that hid what it guarded.** v08's helper panicked as "the session is
  still shared" whenever a wrong authorization succeeded, which masked the
  marked assertion. Found by the trial controls, and fixed.
- **An R1 control masked by an R2 protection.** R1-VERIFICATION-NEVER-LAPSES
  (procedure steps no longer lapse the verification) stopped failing `b08`.
  `b08` ran its revocation to completion, and in R2 a completed procedure's
  verify-after also clears the verification. `b08` now stops the revocation
  after its first step, so the lapse at a step's start is the only
  protection left, and checks the completed case separately. The control
  itself is unchanged.

## Observed and not changed (outside the authority findings)

- A repeated succession after its step 2a refuses with EEXIST, where R5
  section 15.2 skips 2a when the kept copy matches (see the authority
  matrix). This is fail-closed, and reported for the Architect.
- P-REVOKE does not bound `dispositions/revoked/`, and R5 specifies no bound.
  A store pushed over it is Invalid, and succession can accept that
  condition by name (v20).
- P-PROV's step 6 (the standalone verifier, run as the store uid) stays the
  Owner's act (see the authority matrix).

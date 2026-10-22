# Behavioural controls (P2-V1-R3B-I3-I1-R3)

Store controls: 179 counted; all as required: True; sources identical after the run: True; failures: none.

By category (each counted separately):

- administrative-recovery: 35 of 35 as required
- authority-api-surface: 9 of 9 as required
- authority-binding: 10 of 10 as required
- implementation-safety: 71 of 71 as required
- maintenance-authority: 38 of 38 as required
- simulator-source-conformance: 16 of 16 as required

## administrative-recovery

| control | what it restores | files | intended test | marker | compiled | failed the test | marker present | restored | as required |
|---|---|---|---|---|---|---|---|---|---|
| NC-R3-SUCC-KEEP-OVERWRITE | a kept copy that is not the selected PROVISION is taken for absent, so step 2a runs over it | maintenance.rs | r303_a_kept_copy_one_byte_off_refuses_the_repeat | [succession-keep] | True | True | True | True | True |
| NC-R3-SUCC-KEEP-BLESS | a kept copy that is not the selected PROVISION is blessed as the exact copy (step 2a skipped) | maintenance.rs | r303_a_kept_copy_one_byte_off_refuses_the_repeat | [succession-keep] | True | True | True | True | True |
| NC-R3-SUCC-NO-SKIP | an exact kept copy does not skip step 2a | maintenance.rs | r302_a_repeat_after_the_kept_copy_skips_step_2a | [succession-repeat] | True | True | True | True | True |
| NC-R3-SUCC-ORDINARY-SKIP | the ordinary succession (no kept copy) skips step 2a | maintenance.rs | r301_an_ordinary_succession_is_the_documented_29_operations | [succession-repeat] | True | True | True | True | True |
| NC-R3-SUCC-KEEP-METADATA | a kept copy of another owner, mode or link count is taken for the exact copy | maintenance.rs | r304_a_kept_copy_that_is_not_root_0444_with_one_link_refuses | [succession-keep] | True | True | True | True | True |
| NC-R3-SUCC-LEFTOVER-IGNORED | a leftover PROVISION temporary does not refuse the repeat | maintenance.rs | r305_a_leftover_temporary_refuses_the_repeat_until_r_leftover | [succession-keep] | True | True | True | True | True |
| NC-R3-SUCC-ROOT-IGNORED | an existing successor root does not refuse the repeat | maintenance.rs | r306_a_zero_incomplete_successor_root_is_removed | [succession-cleanup] | True | True | True | True | True |
| NC-R3-SUCC-GATE-COPY | the succession's gate does not recheck the kept copy | maintenance.rs | r314_a_mismatched_kept_copy_is_never_overwritten | [succession-keep] | True | True | True | True | True |
| NC-R3-SUCC-CLEAN-NONZERO | a successor pool file with a non-zero byte is removed | maintenance.rs | r307_a_successor_pool_byte_that_is_not_zero_refuses | [succession-cleanup] | True | True | True | True | True |
| NC-R3-SUCC-CLEAN-UNEXPECTED | an unexpected entry of the candidate's root is ignored | maintenance.rs | r308_an_unexpected_entry_refuses_the_cleanup | [succession-cleanup] | True | True | True | True | True |
| NC-R3-SUCC-CLEAN-SELECTED | the selected store is not refused as a candidate | maintenance.rs | r309_a_referenced_candidate_refuses_the_cleanup | [succession-cleanup] | True | True | True | True | True |
| NC-R3-SUCC-CLEAN-UID | a candidate of another uid or gid is not refused as another store's | maintenance.rs | r309_a_referenced_candidate_refuses_the_cleanup | [succession-cleanup] | True | True | True | True | True |
| NC-R3-SUCC-CLEAN-COPY-CONTENT | what a kept copy records (root id, state root, predecessor) does not refuse the candidate | maintenance.rs | r309_a_referenced_candidate_refuses_the_cleanup | [succession-cleanup] | True | True | True | True | True |
| NC-R3-SUCC-CLEAN-NOT-INTERRUPTED | the cleanup is built for a store with no interrupted succession | maintenance.rs | r309_a_referenced_candidate_refuses_the_cleanup | [succession-cleanup] | True | True | True | True | True |
| NC-R3-SUCC-CLEAN-NO-LOCK | the cleanup takes no lock on the candidate's LOCK | maintenance.rs | r310_a_candidate_held_elsewhere_refuses_the_cleanup | [succession-cleanup] | True | True | True | True | True |
| NC-R3-SUCC-CLEAN-RECURSIVE | a directory's removal first removes what it holds (recursive) | maintenance.rs | r308_an_unexpected_entry_refuses_the_cleanup | [successor-cleanup-recursive] | True | True | True | True | True |
| NC-R3-SUCC-CLEAN-IDENTITY | a removal does not check that the entry is the inode the inspection found | maintenance.rs | r308_an_unexpected_entry_refuses_the_cleanup | [succession-cleanup] | True | True | True | True | True |
| NC-R3-SUCC-CLEAN-GATE | the cleanup's gate does not inspect the candidate again | maintenance.rs | r307_a_successor_pool_byte_that_is_not_zero_refuses | [succession-cleanup] | True | True | True | True | True |
| NC-R3-SUCC-CLEAN-GONE-REFUSED | the cleanup refuses when the candidate is already gone, instead of syncing the parent | maintenance.rs | r3x3_every_interrupted_cleanup_recovers | [succession-cleanup] | True | True | True | True | True |
| NC-R3-SUCC-CLEAN-NO-PARENT-SYNC | the cleanup does not sync the parent after removing the root | maintenance.rs | r306_a_zero_incomplete_successor_root_is_removed | [succession-cleanup] | True | True | True | True | True |
| NC-R3-SUCC-REUSE-VERIFY | the cleanup runs outside the session's accounting and without its verify-after, so the verification before it still authorizes | maintenance.rs | r312_the_repeat_needs_a_fresh_verification_after_the_cleanup | [succession-current] | True | True | True | True | True |
| NC-R3-RECOVERY-NO-POSTVERIFY-SUCC | R-SUCCESSOR's verify-after does not require the candidate gone | maintenance.rs | r306_a_zero_incomplete_successor_root_is_removed | [succession-cleanup] | True | True | True | True | True |
| NC-R3-REVOKE-BOUND-EARLY | a revocation refuses below revoked/'s bound (at 4095 entries) | maintenance.rs | r315_a_revocation_below_the_bound_fills_it | [revocation-bound] | True | True | True | True | True |
| NC-R3-REVOKE-OVERFLOW | a revocation at revoked/'s bound proceeds and exceeds it | maintenance.rs | r316_a_revocation_at_the_bound_refuses_before_any_operation | [revocation-bound] | True | True | True | True | True |
| NC-R3-REVOKE-REPLACE | a revocation renames over an existing revoked file | maintenance.rs | r317_a_revocation_never_replaces_revoked_evidence | [revocation-replace] | True | True | True | True | True |
| NC-R3-REVOKE-TIME-IGNORED | the revoked name ignores the revocation's time, so a second revocation of a binding meets the first | maintenance.rs | r318_revocations_at_two_times_are_both_kept | [revocation-replace] | True | True | True | True | True |
| NC-R3-REVOKE-NO-VERIFY-AFTER | a normal revocation has no verify-after | maintenance.rs | r323_a_revocation_completes_with_its_verify_after | [revocation-verify-after] | True | True | True | True | True |
| NC-R3-REVOKE-GATE | the revocation's gate does not read its admissibility again | maintenance.rs | r316_a_revocation_at_the_bound_refuses_before_any_operation | [revocation-bound] | True | True | True | True | True |
| NC-R3-REVOKE-RECOVER-WRONG | R-REVOKE accepts another file under the revoked name | maintenance.rs | r320_a_differing_revocation_state_refuses_r_revoke | [revocation-recovery] | True | True | True | True | True |
| NC-R3-REVOKE-RECOVER-NAME | R-REVOKE takes a revocation time that is not a compact UTC time | maintenance.rs | r320_a_differing_revocation_state_refuses_r_revoke | [revocation-recovery] | True | True | True | True | True |
| NC-R3-REVOKE-RECOVER-DELETE-TARGET | R-REVOKE removes the revoked file instead of the active name | maintenance.rs | r319_r_revoke_unlinks_only_the_active_name_and_syncs | [revocation-recovery] | True | True | True | True | True |
| NC-R3-REVOKE-RECOVER-NO-SYNC | R-REVOKE does not sync dispositions/ after the unlink | maintenance.rs | r319_r_revoke_unlinks_only_the_active_name_and_syncs | [revocation-recovery] | True | True | True | True | True |
| NC-R3-REVOKE-CAPACITY-CONFLATED | R-REVOKE refuses at revoked/'s bound as a normal revocation does | maintenance.rs | r321_r_revoke_applies_at_the_bound | [revocation-capacity] | True | True | True | True | True |
| NC-R3-REVOKE-COMPLETED-REFUSED | R-REVOKE does not recognize a completed revocation (it refuses instead of repeating the sync) | maintenance.rs | r3x4_every_interrupted_revocation_recovers | [revocation-recovery] | True | True | True | True | True |
| NC-R3-RECOVERY-NO-POSTVERIFY | R-REVOKE's verify-after does not require the revocation completed | maintenance.rs | r324_r_revoke_completes_with_its_verify_after | [revocation-verify-after] | True | True | True | True | True |

## authority-api-surface

| control | what it restores | files | intended test | marker | compiled | failed the test | marker present | restored | as required |
|---|---|---|---|---|---|---|---|---|---|
| A-ENTRY-CALLS | the real configured-store entry evaluates its request before refusing | open.rs | o01_the_real_store_entry_is_closed | [real-entry-closed] | True | True | True | True | True |
| A-ENTRY-SWITCH | the real configured-store entry reads an environment switch | open.rs | o01_the_real_store_entry_is_closed | [real-entry-closed] | True | True | True | True | True |
| A-ADMISSION-CLONE | a storage admission can be cloned (and so kept beyond its opening) | open.rs | o10_a_storage_admission_is_bound_to_its_opening | [storage-qualification] | True | True | True | True | True |
| A-ADMISSION-FORGED | a storage admission constructed outside the storage check (a second constructor) | open.rs | o10_a_storage_admission_is_bound_to_its_opening | [storage-qualification] | True | True | True | True | True |
| A-LOCK-UNLOCK | the store's I/O trait gains a lock release (a duplicate's LOCK_UN releases the original's lock) | io.rs, sim.rs | a01_the_authority_surface_is_fixed_in_the_source | [lock-retained] | True | True | True | True | True |
| A-WORKER-LOCK | the recorder worker holds a lock description (the first candidate's recorder thread) | recorder.rs | a01_the_authority_surface_is_fixed_in_the_source | [lock-retained] | True | True | True | True | True |
| A-GATE-SPLIT | the admission gate's health check is followed by an unprotected admission call | exchange.rs | a01_the_authority_surface_is_fixed_in_the_source | [admission-fence] | True | True | True | True | True |
| A-SELECTION-KEPT | a selection keeps a descriptor from its read (through which revalidation could look) | open.rs | a01_the_authority_surface_is_fixed_in_the_source | [provision-selection] | True | True | True | True | True |
| NC-R3-SUCC-CLEAN-RECURSIVE-API | the I/O trait gains a recursive removal | io.rs | r328_no_new_public_authority_bearing_interface | [successor-cleanup-recursive] | True | True | True | True | True |

## authority-binding

| control | what it restores | files | intended test | marker | compiled | failed the test | marker present | restored | as required |
|---|---|---|---|---|---|---|---|---|---|
| R1-REFUSED-CLOSE-SEALS | (B1) a refused closure still requests the seal, from the recorder's durable count | owner.rs | b01_a_refused_closure_requests_no_seal_and_keeps_its_owner | [closure-seal] | True | True | True | True | True |
| R1-START-FROM-REPORT | (B3) the start decides over the incidents the bounded report lists, not the complete set | owner.rs | b04_a_truncated_report_hides_no_blocking_incident | [complete-set] | True | True | True | True | True |
| R1-VALIDATOR-UNCHECKED | (B5) the store's validator returns a validation for any disposition file, exact or not | disposition.rs | b06_a_disposition_takes_effect_only_through_its_opening | [disposition-provenance] | True | True | True | True | True |
| R1-APPLIED-REASON-INVENTED | (B5) the claim's applied reasons are not the verified disposition files' reasons | open.rs | b06_a_disposition_takes_effect_only_through_its_opening | [disposition-provenance] | True | True | True | True | True |
| R1-FAULT-ACKS-UNDURABLE | (B6) the fault interface acknowledges a record that is not durable | faults.rs | b07_status_copies_and_fault_operations_clear_and_manufacture_nothing | [exchange-contained] | True | True | True | True | True |
| R1-FAULT-INJECTS-NEXT | (B6) the fault interface submits a record the sink stores as the next one | faults.rs | r02_invalid_and_conflicting_submissions_latch_and_land_at_an_issued_record | [exchange-contained] | True | True | True | True | True |
| R1-LATCH-REPLACED | (B6) a later latch replaces the first cause (a latch is no longer monotonic) | exchange.rs | b07_status_copies_and_fault_operations_clear_and_manufacture_nothing | [exchange-contained] | True | True | True | True | True |
| R1-VERIFICATION-REUSED | (session) one session verification authorizes more than one procedure | maintenance.rs | b08_a_session_decides_only_from_its_own_current_verification | [session-verify] | True | True | True | True | True |
| R1-VERIFICATION-NEVER-LAPSES | (session) a procedure step does not lapse the session's earlier verification | maintenance.rs | b08_a_session_decides_only_from_its_own_current_verification | [session-verify] | True | True | True | True | True |
| R1-GUARD-NOT-KEPT | (exclusion) the worker's thread does not keep the owner's guard: the exclusion ends with the owner, I/O in flight | owner.rs | b09_exclusion_outlasts_the_owner_while_its_io_is_in_flight | [closure-owner] | True | True | True | True | True |

## implementation-safety

| control | what it restores | files | intended test | marker | compiled | failed the test | marker present | restored | as required |
|---|---|---|---|---|---|---|---|---|---|
| I-HEADER-RESERVED | header reserved bytes unchecked | format.rs | f01_header_bytes_are_assigned_and_checked | [exact-bytes] | True | True | True | True | True |
| I-SEAL-TAIL | seal tail bytes unchecked (the first candidate's unassigned bytes) | format.rs | f02_seal_bytes_are_assigned_and_checked | [exact-bytes] | True | True | True | True | True |
| I-RECORD-PADDING | record-block padding unchecked | format.rs | f03_record_blocks_take_exactly_one_frame | [exact-bytes] | True | True | True | True | True |
| I-INVALID-HEADER-ABANDONED | a checksum-valid invalid header taken as an abandoned claim (the first candidate) | classify.rs | f01_header_bytes_are_assigned_and_checked | [exact-bytes] | True | True | True | True | True |
| I-DECIMAL-LEADING-ZERO | non-canonical decimals (leading zeros) accepted in text | format.rs | f04_provision_grammar_is_exact | [exact-bytes] | True | True | True | True | True |
| I-BINDING-LOCATION | a journal's binding depends on where its bytes lie (the pool index), so the archive copy loses it | classify.rs | m04_archival_recycling_retirement_and_succession | [archive-binding] | True | True | True | True | True |
| I-BINDING-NO-CONTENT | a journal's binding ignores its content | format.rs | f07_bindings_are_content_addressed | [archive-binding] | True | True | True | True | True |
| I-GRAMMAR-G16 | grammar rule G16 (a Failed verdict if and only if admission closed) removed | classify.rs | g01_grammar_fixtures_prefixes_and_mutations | [grammar-conformance] | True | True | True | True | True |
| I-REFUSE-ONLY-RECORDED | refuse an unsealed generation only when it recorded outstanding work | classify.rs | k04_a_late_owner_without_an_acknowledged_record_still_blocks | [no-false-resolution] | True | True | True | True | True |
| I-VISIBLE-RUNENDED-RESOLVES | a visible RunEnded taken as resolution | classify.rs | c01_classes_and_the_conservative_refusal | [no-false-resolution] | True | True | True | True | True |
| I-LATE-NOTE-MISSING | an unsealed generation's report omits that late owners may exist with no durable record | classify.rs | c01_classes_and_the_conservative_refusal | [late-uncertain] | True | True | True | True | True |
| I-DECIDE-FROM-REPORT | a startup decided from the truncated report | classify.rs | c03_pool_level_checks_and_bounded_reports | [aggregate-bounds] | True | True | True | True | True |
| I-NONDETERMINISTIC-REPORT | a file's report depends on something other than its bytes (a call counter) | classify.rs | c02_classification_is_deterministic | [archive-binding] | True | True | True | True | True |
| I-VALIDATOR-INEXACT | the disposition validator accepts a disposition that does not restate the incident's facts | disposition.rs | f09_the_validator_accepts_only_an_exact_restatement | [disposition-exact] | True | True | True | True | True |
| I-PRESERVATION-SKIPPED | startup reports what it read without the evidence-preservation sync | open.rs | o13_the_preservation_sync_makes_what_the_report_saw_durable | [F1-F2-sync] | True | True | True | True | True |
| I-SYNC-BEFORE-LOCK | startup syncs and reads journals before taking the store lock (the first candidate) | open.rs | o03_busy_before_any_sync_or_read | [busy-before-sync] | True | True | True | True | True |
| I-NO-REVALIDATION | no post-lock revalidation of the PROVISION selection | open.rs | o04_a_replaced_selection_never_regains_authority | [provision-selection] | True | True | True | True | True |
| I-OPEN-BEFORE-TYPE | an untrusted pool entry is opened before its type is checked | open.rs | o05_safe_open_refuses_before_opening | [safe-open] | True | True | True | True | True |
| I-PROVISION-INODE-IGNORED | a pool file whose inode PROVISION does not record is accepted | open.rs | o05_safe_open_refuses_before_opening | [safe-open] | True | True | True | True | True |
| I-EXT4-MAGIC | ext4 decided by the shared superblock magic alone: the mount's filesystem type is not checked | open.rs | o06_the_mount_is_identified_through_the_descriptor | [safe-open] | True | True | True | True | True |
| I-NO-ACTIVATION | a freshly revalidated, visible selection treated as activated: no directory syncs, no PROVISION sync, no probe | open.rs | o02_activation_precedes_every_decision_and_the_claim | [durable-activation] | True | True | True | True | True |
| I-ACTIVATION-AFTER-SCAN | the scan, the decision (and so the claim) before the activation | open.rs | o02_activation_precedes_every_decision_and_the_claim | [durable-activation] | True | True | True | True | True |
| I-R2-PROTOCOL | the R2 protocol: no activation, and a root holding history not found after Unprovisioned (so a fresh one replaces it), run through the composed tests | maintenance.rs, open.rs | x01_composed_recovery_keeps_every_acknowledged_generation | [composed-recovery] | True | True | True | True | True |
| I-ACTIVATION-ERROR-IGNORED | a failed or uncertain activation directory sync treated as success | open.rs | o11_activation_refuses_on_failure_and_certifies_late_errors | [durable-activation] | True | True | True | True | True |
| I-NO-PROBE | no certification probe: syncs that returned 0 after a silent commit failure are trusted | open.rs | o11_activation_refuses_on_failure_and_certifies_late_errors | [activation-proof] | True | True | True | True | True |
| I-PROBE-FIRST | the probe runs before the directory syncs | open.rs | o02_activation_precedes_every_decision_and_the_claim | [durable-activation] | True | True | True | True | True |
| I-NO-A4 | the selection and directory revalidation after the activation syncs (A4) omitted | open.rs | o11_activation_refuses_on_failure_and_certifies_late_errors | [provision-selection] | True | True | True | True | True |
| I-A2-IDENTITY | an activation directory is synced without checking it is the selected one | open.rs | o11_activation_refuses_on_failure_and_certifies_late_errors | [durable-activation] | True | True | True | True | True |
| I-KERNEL-PREFIX | the kernel accepted by its release prefix, not its exact build identity | open.rs | o07_the_effective_profile_is_read_not_inferred | [supported-profile] | True | True | True | True | True |
| I-EXTERNAL-JOURNAL | an external journal accepted: the journal's location never checked | open.rs | o07_the_effective_profile_is_read_not_inferred | [supported-profile] | True | True | True | True | True |
| I-EXCLUDED-MODE | an excluded mode in the effective listing accepted | open.rs | o07_the_effective_profile_is_read_not_inferred | [supported-profile] | True | True | True | True | True |
| I-PROFILE-FROM-PINNED | the profile taken from the pinned mountinfo strings: the effective listing, journal and kernel never read | open.rs | o07_the_effective_profile_is_read_not_inferred | [supported-profile] | True | True | True | True | True |
| I-NO-A4-PROFILE | the effective profile not checked again after the activation syncs (A4) | open.rs | o08_a_profile_changed_during_activation_refuses_at_a4 | [supported-profile] | True | True | True | True | True |
| I-WRITE-CACHE-ALONE | a volatile-completion profile admitted as stable: queue/write_cache alone decides | open.rs | o09_storage_that_is_not_admitted_refuses_before_any_claim | [storage-qualification] | True | True | True | True | True |
| I-IDENTITY-UNCHECKED | the storage identity PROVISION records is not compared: another device of the same kind is admitted | open.rs | o09_storage_that_is_not_admitted_refuses_before_any_claim | [storage-qualification] | True | True | True | True | True |
| I-ATTESTATION-ADMITS | storage admitted on the Owner's attestation (A-S5, R4's domain), whatever the device shows | open.rs | x03_storage_events_composed | [storage-composed] | True | True | True | True | True |
| I-CLAIM-UNBOUNDED | the claim counter is not bounded at 2^63 | open.rs | c04_the_claim_counter_never_wraps | [arith-bounds] | True | True | True | True | True |
| I-TMP-DISPOSITION | a disposition under a temporary name is read as a disposition | open.rs | m09_the_crash_matrix_is_the_documented_one | [admin-crash] | True | True | True | True | True |
| I-BOUND-AFTER-EXAMINE | enumeration bounds checked only after the entries were examined | open.rs | m07_aggregate_bounds_refuse_before_any_entry_is_examined | [aggregate-bounds] | True | True | True | True | True |
| I-DURABLE-BEFORE-SYNC | a record is published durable without its sync | recorder.rs | r01_acknowledged_implies_durable_under_write_and_sync_faults | [ack-durable] | True | True | True | True | True |
| I-SYNC-RETRIED-AFTER-EIO | a failed sync (EIO) is retried into success | io.rs | r01_acknowledged_implies_durable_under_write_and_sync_faults | [ack-durable] | True | True | True | True | True |
| I-WORKER-IDENTITY | the worker's recheck (W5) is skipped: a write to a replaced journal is published | recorder.rs | r10_a_replaced_journal_latches_before_publication | [ack-durable] | True | True | True | True | True |
| I-NO-GATE | no store admission gate: admission continues after a fatal condition | exchange.rs | r02_invalid_and_conflicting_submissions_latch_and_land_at_an_issued_record | [admission-fence] | True | True | True | True | True |
| I-FAILURE-AT-CAUSE | R1 failure targeting: the failure is delivered at the cause's own sequence | exchange.rs | r02_invalid_and_conflicting_submissions_latch_and_land_at_an_issued_record | [fatal-total] | True | True | True | True | True |
| I-FUTURE-SILENT | a submission for an unissued future sequence is ignored silently | exchange.rs | r02_invalid_and_conflicting_submissions_latch_and_land_at_an_issued_record | [fatal-total] | True | True | True | True | True |
| I-PUBLISH-AFTER-LATCH | the worker publishes durability after the fatal latch | recorder.rs | r03_every_interleaving_of_gate_worker_apply_and_latch | [admission-fence] | True | True | True | True | True |
| I-UNEXPECTED-ACK-IGNORED | an unexpected acknowledgement outcome is ignored | exchange.rs | r05_in_flight_appends_and_unexpected_acknowledgement_outcomes | [fatal-total] | True | True | True | True | True |
| I-FAIL-UNISSUED | record_failed for a record not issued yet, counted as delivered | exchange.rs | r04_worker_loss_vanishing_and_poisoning | [fatal-total] | True | True | True | True | True |
| I-DELIVERY-DROPPED | the failure is reported as a message the core never applies | exchange.rs | r02_invalid_and_conflicting_submissions_latch_and_land_at_an_issued_record | [fatal-total] | True | True | True | True | True |
| I-DUP-CONFLICT | an identical resubmission is a conflict (submission not idempotent) | exchange.rs | r09_resubmission_is_idempotent_and_bounded | [dup-bounded] | True | True | True | True | True |
| I-DUP-RETAINED | an identical resubmission is stored and retained again (a growing queue) | exchange.rs | r09_resubmission_is_idempotent_and_bounded | [dup-bounded] | True | True | True | True | True |
| I-TIMEOUT-STOPS | a bounded wait that times out is taken as the worker having stopped | exchange.rs | r06_a_stall_is_reported_never_failed_and_nothing_waits_on_storage | [fatal-latched] | True | True | True | True | True |
| I-POISON-IGNORED | a poisoned exchange mutex is used as if healthy | exchange.rs | r04_worker_loss_vanishing_and_poisoning | [fatal-total] | True | True | True | True | True |
| I-VANISHED-IGNORED | a worker thread that ended without a recorded exit is not noticed | exchange.rs | r04_worker_loss_vanishing_and_poisoning | [fatal-total] | True | True | True | True | True |
| I-NO-DROP-GUARD | a worker that unwinds latches nothing (its drop guard disarmed) | recorder.rs | r04_worker_loss_vanishing_and_poisoning | [fatal-total] | True | True | True | True | True |
| I-SEAL-WHILE-LATCHED | a seal is requested although a fatal cause is latched | exchange.rs | r08_claim_and_seal_boundaries | [fatal-total] | True | True | True | True | True |
| I-SESSION-NEW-LOCK | in-session verification requests a new shared lock on the store lock (the R1 workflow) | maintenance.rs | m01_sessions_verify_through_their_retained_lock | [session-verify] | True | True | True | True | True |
| I-SESSION-SHARED | in-session verification converts the session's exclusive lock to shared | maintenance.rs | m01_sessions_verify_through_their_retained_lock | [session-verify] | True | True | True | True | True |
| I-SESSION-RELOCK | the session releases its lock around verification and takes it again | maintenance.rs | m01_sessions_verify_through_their_retained_lock | [session-verify] | True | True | True | True | True |
| I-BUSY-SUCCESS | a standalone verification that found the store busy proceeds as a success | open.rs | m01_sessions_verify_through_their_retained_lock | [session-verify] | True | True | True | True | True |
| I-AUTHORITY-FLAG | maintenance authority asserted by a flag instead of the retained lock | maintenance.rs | m01_sessions_verify_through_their_retained_lock | [session-verify] | True | True | True | True | True |
| I-VERIFY-CACHED | in-session verification skipped: an earlier report reused | maintenance.rs | m01_sessions_verify_through_their_retained_lock | [session-verify] | True | True | True | True | True |
| I-RETIRE-NO-PRECONDITIONS | retirement without its preconditions hides pool claims | maintenance.rs | m04_archival_recycling_retirement_and_succession | [admin-crash] | True | True | True | True | True |
| I-RETIRE-FROM-REPORT | retirement preconditions evaluated on the truncated report | maintenance.rs | m10_retirement_is_decided_from_the_complete_set | [aggregate-bounds] | True | True | True | True | True |
| I-HIDDEN-STEP | an undocumented directory sync in recycling (a step the protocol does not list) | maintenance.rs | m08_procedures_perform_exactly_the_documented_steps | [step-parity] | True | True | True | True | True |
| I-FRESH-ROOT | after Unprovisioned, a root holding history is not found, so a fresh root replaces it (the R2 protocol) | maintenance.rs | m06_an_unprovisioned_root_with_history_is_republished | [composed-recovery] | True | True | True | True | True |
| N-DOT-NAME | a component name of . or .. is accepted | io.rs | n01_native_opening_is_descriptor_relative_and_no_follow | [safe-open] | True | True | True | True | True |
| N-FOLLOW | symbolic links are followed (O_NOFOLLOW and RESOLVE_NO_SYMLINKS both removed) | io.rs | n01_native_opening_is_descriptor_relative_and_no_follow | [safe-open] | True | True | True | True | True |
| N-BLOCKING | an entry is opened for reading without O_NONBLOCK (a FIFO open blocks) | io.rs | n03_native_fifo_opens_without_blocking | [safe-open] | True | True | True | True | True |
| N-R3-RMDIR-FLAG | the native removal is unlinkat without AT_REMOVEDIR | io.rs | n09_native_remove_dir_is_one_empty_directory | [successor-cleanup-recursive] | True | True | True | True | True |
| N-R3-RMDIR-NAME | the native removal passes a name with a separator or a dot to the kernel | io.rs | n09_native_remove_dir_is_one_empty_directory | [safe-open] | True | True | True | True | True |

## maintenance-authority

| control | what it restores | files | intended test | marker | compiled | failed the test | marker present | restored | as required |
|---|---|---|---|---|---|---|---|---|---|
| NC-SUCC-NO-VERIFY | (F1) succession needs no verify-before: without one it verifies for itself | maintenance.rs | v05_only_the_sessions_own_verify_before_authorizes | [succession-verified] | True | True | True | True | True |
| NC-SUCC-MISSING-DISPOSITION | (B-S1) succession over a current incident without an exact disposition | maintenance.rs | v01_succession_needs_every_bound_incident_dispositioned | [succession-dispositioned] | True | True | True | True | True |
| NC-SUCC-REPORT-TRUNCATION | (B-S2) succession counts only the incidents the bounded report lists in detail | maintenance.rs | v03_succession_decides_over_the_complete_set_not_the_report | [succession-complete-set] | True | True | True | True | True |
| NC-SUCC-STALE | (B-S4) a verification older than a procedure step still authorizes | maintenance.rs | v04_a_stale_or_consumed_verification_authorizes_no_succession | [succession-current] | True | True | True | True | True |
| NC-SUCC-REUSE-AUTH | one verification authorizes a second succession (not consumed) | maintenance.rs | v04_a_stale_or_consumed_verification_authorizes_no_succession | [succession-verified] | True | True | True | True | True |
| NC-SUCC-OMIT-INVALID | (F2) the Owner's acceptance may omit a verified store-level Invalid condition | maintenance.rs | v08_invalid_conditions_are_accepted_exactly_and_named_in_the_statement | [succession-invalid-accepted] | True | True | True | True | True |
| NC-SUCC-INVENT-INVALID | (F2) the Owner's acceptance may name a condition the verification did not find | maintenance.rs | v08_invalid_conditions_are_accepted_exactly_and_named_in_the_statement | [succession-invalid-accepted] | True | True | True | True | True |
| NC-SUCC-REPEAT-INVALID | (F2) a condition the Owner accepts twice counts once | maintenance.rs | v08_invalid_conditions_are_accepted_exactly_and_named_in_the_statement | [succession-invalid-accepted] | True | True | True | True | True |
| NC-SUCC-STATEMENT-CALLER | (F2) the predecessor statement is the Owner's list as given, not generated from the verified set | maintenance.rs | v08_invalid_conditions_are_accepted_exactly_and_named_in_the_statement | [succession-invalid-accepted] | True | True | True | True | True |
| NC-SUCC-STATEMENT-TRUNCATED | (F2) a statement over 512 bytes is truncated instead of refused | maintenance.rs | v09_a_condition_set_that_does_not_fit_the_statement_refuses | [succession-invalid-accepted] | True | True | True | True | True |
| NC-SUCC-CAPACITY-BYPASS | (F3) more claim gaps than the store can hold dispositions for count as a complete proof | maintenance.rs | v07_capacity_without_a_complete_proof_refuses_succession | [succession-complete-set] | True | True | True | True | True |
| NC-SUCC-NO-POSTVERIFY | (F4) succession has no verify-after | maintenance.rs | v02_succession_verifies_the_selected_successor_after_publication | [succession-verify-after] | True | True | True | True | True |
| NC-SUCC-POSTVERIFY-IGNORED | (B-S3) the successor's verify-after runs and what it finds is ignored | maintenance.rs | v10_a_successor_that_does_not_verify_leaves_the_succession_incomplete | [succession-verify-after] | True | True | True | True | True |
| NC-SUCC-VERIFY-OLD-ROOT | the verify-after runs before the re-selection: it verifies the predecessor, not the successor | maintenance.rs | v10_a_successor_that_does_not_verify_leaves_the_succession_incomplete | [succession-verify-after] | True | True | True | True | True |
| NC-SUCC-NO-GATE | (B-S4) succession has no gate: an authorization overtaken by another procedure's step still runs | maintenance.rs | v12_no_succession_operation_precedes_its_gate | [succession-gate] | True | True | True | True | True |
| NC-SUCC-MUTATE-BEFORE-GATE | the succession's gate is checked after its first operation, not before it | maintenance.rs | v12_no_succession_operation_precedes_its_gate | [succession-gate] | True | True | True | True | True |
| NC-SUCC-GATE-PROVISION | succession's gate does not recheck PROVISION | maintenance.rs | v12_no_succession_operation_precedes_its_gate | [succession-gate] | True | True | True | True | True |
| NC-SUCC-UNDISPOSITIONED-HISTORY | succession over archived history without its disposition | maintenance.rs | v16_an_archived_incident_is_dispositioned_again_from_its_archived_bytes | [succession-dispositioned] | True | True | True | True | True |
| NC-SUCC-LEFTOVER | succession over a predecessor with a leftover temporary (no recovery first) | maintenance.rs | v22_a_successor_is_a_new_root_of_a_recovered_predecessor | [succession-precondition] | True | True | True | True | True |
| NC-SUCC-SAME-ROOT | a successor may reuse the predecessor's root id (in R3: its root id and its existing state root) | maintenance.rs | v22_a_successor_is_a_new_root_of_a_recovered_predecessor | [succession-precondition] | True | True | True | True | True |
| NC-SUCC-DUAL-LOCK | (S18) adopting the successor's lock releases the predecessor's | maintenance.rs | v13_both_locks_are_held_through_publication_and_verify_after | [succession-locks] | True | True | True | True | True |
| NC-SUCC-CAPACITY-REFUSED | (S07) succession refuses every Capacity condition, even with a complete proof (design section 13.10's resolution lost) | maintenance.rs | v06_over_capacity_a_complete_proof_authorizes_succession | [succession-complete-set] | True | True | True | True | True |
| NC-VERIFY-FIRST-ONLY | (F6) in-session verification stops at the first condition | maintenance.rs | v08_invalid_conditions_are_accepted_exactly_and_named_in_the_statement | [maintenance-verify] | True | True | True | True | True |
| NC-OWNER-COLLECTS | an owner's opening collects past its first condition and proceeds | open.rs | v08_invalid_conditions_are_accepted_exactly_and_named_in_the_statement | [maintenance-verify] | True | True | True | True | True |
| NC-VERIFY-AFTER-AUTHORIZES | a procedure's verify-after leaves a verification the next procedure consumes | maintenance.rs | v05_only_the_sessions_own_verify_before_authorizes | [succession-verified] | True | True | True | True | True |
| NC-GAP-UNBOUNDED | claim gaps are enumerated without bound | classify.rs | v07_capacity_without_a_complete_proof_refuses_succession | [succession-complete-set] | True | True | True | True | True |
| NC-ENTRY-STAT-DETERMINATE | a disposition entry that cannot be stat'ed is recorded as a determinate condition | open.rs | v20_what_cannot_be_examined_is_indeterminate | [maintenance-verify] | True | True | True | True | True |
| NC-REVOKED-BOUND-INDETERMINATE | dispositions/revoked over its bound stops the verification: no Owner can accept it | open.rs | v20_what_cannot_be_examined_is_indeterminate | [maintenance-verify] | True | True | True | True | True |
| NC-NAMES-EXAMINED-TWICE | a name recorded as unexpected or leftover is also examined as an entry of its directory | open.rs | v23_a_recorded_name_is_one_condition | [maintenance-verify] | True | True | True | True | True |
| NC-DISP-UNREPORTED | a disposition is published for a predictable binding (a claim gap's) no verification reported | maintenance.rs | v15_a_disposition_restates_only_a_verified_incident | [disposition-verified] | True | True | True | True | True |
| NC-DISP-FACTS-NOT-VERIFIED | a published disposition's facts are not the verified ones | maintenance.rs | v15_a_disposition_restates_only_a_verified_incident | [disposition-verified] | True | True | True | True | True |
| NC-DISP-ARCHIVED-NAME-ONLY | an archived journal's facts come from its name alone, not its archived bytes | maintenance.rs | v16_an_archived_incident_is_dispositioned_again_from_its_archived_bytes | [disposition-verified] | True | True | True | True | True |
| NC-ARCHIVE-CAPACITY | archival is refused over capacity (design section 13.10's resolution lost) | maintenance.rs | v18_over_capacity_archival_lets_dispositioned_history_leave | [capacity-archival] | True | True | True | True | True |
| NC-LEFTOVER-AFTER-ACCEPTED | a procedure's verify-after accepts a leftover temporary | maintenance.rs | v21_a_leftover_temporary_fails_the_verify_after | [maintenance-verify] | True | True | True | True | True |
| NC-DISP-NO-POSTVERIFY | (F4) disposition publication has no verify-after | maintenance.rs | v19_every_session_procedure_completes_with_its_verify_after | [maintenance-verify] | True | True | True | True | True |
| NC-PROV-OVER-EXISTING | provisioning replaces an existing PROVISION | maintenance.rs | v17_provisioning_and_republication_never_replace_a_store | [provision-unprovisioned] | True | True | True | True | True |
| NC-REPUBLISH-OVER-EXISTING | re-publication replaces an existing PROVISION | maintenance.rs | v17_provisioning_and_republication_never_replace_a_store | [provision-unprovisioned] | True | True | True | True | True |
| NC-PROV-OVER-HISTORY | provisioning over a parent where another root may hold history | maintenance.rs | v17_provisioning_and_republication_never_replace_a_store | [provision-unprovisioned] | True | True | True | True | True |

## simulator-source-conformance

| control | what it restores | files | intended test | marker | compiled | failed the test | marker present | restored | as required |
|---|---|---|---|---|---|---|---|---|---|
| S-F1-DURABLE | process death makes kernel-visible bytes durable | sim.rs | s01_process_death_and_power_loss_keep_states_apart | [F1-volatile] | True | True | True | True | True |
| S-REOPEN-CERTIFIES | a sync that returns 0 (as a new description's does after an error was seen) makes the visible bytes durable | sim.rs | s02_writeback_errors_are_not_repaired_by_reopening | [errseq-reopen] | True | True | True | True | True |
| S-TEAR-SPANS | a torn write reaches past its own 4096-byte block | sim.rs | s05_tears_stay_within_their_block | [tear-containment] | True | True | True | True | True |
| S-SCHEDULES-GUARANTEED-ONLY | directory entries become durable only through a directory sync (the R1 model): the ordered family is the guaranteed minimum alone | sim.rs | s04_metadata_schedules_and_split_renames | [metadata-schedules] | True | True | True | True | True |
| S-NO-RECLAIM-WHILE-OPEN | no page reclaim while a descriptor is open (the R1 model) | sim.rs | s03_page_reclaim_with_a_descriptor_open | [metadata-schedules] | True | True | True | True | True |
| S-ERRSEQ-NO-WRAP | an invented detection: the error-sequence counter saturates instead of wrapping, so a 2^19-error collision would be detected | sim.rs | j06_the_errseq_counter_limitation_is_documented_not_detected | [errseq-limit] | True | True | True | True | True |
| S-COMPLETED-TESTS-ABORT | R3's fsync model: the completed-transaction branch tests the abort flag | sim.rs | j01_the_completed_transaction_return | [journal-conformance] | True | True | True | True | True |
| S-INVENTED-ABORT | a discarded flush status treated as detected: an abort invented at an unchecked flush site | sim.rs | j02_discarded_flushes_lose_history_on_a_volatile_cache | [journal-conformance] | True | True | True | True | True |
| S-INFLIGHT-COMPLETE | durability before completion: a home write still in flight counts as done and releases the tail | sim.rs | j03_the_stable_completion_checkpoint | [stable-completion] | True | True | True | True | True |
| S-HOME-ERROR-SUCCESS | a reported home-write error converted into success (journal model) | sim.rs | j03_the_stable_completion_checkpoint | [stable-completion] | True | True | True | True | True |
| S-TAIL-WITHOUT-HOME | required history discarded: the log tail moves past a failed home write | sim.rs | j03_the_stable_completion_checkpoint | [stable-completion] | True | True | True | True | True |
| S-WRITEBACK-ERROR-SWALLOWED | a writeback error the storage reported is not recorded in the error sequence (the sync returns 0) | sim.rs | r01_acknowledged_implies_durable_under_write_and_sync_faults | [ack-durable] | True | True | True | True | True |
| S-R3-RMDIR-NOT-EMPTY | the simulator's rmdir removes a directory that is not empty | sim.rs | s06_rmdir_and_same_file_rename_follow_linux | [sim-conformance] | True | True | True | True | True |
| S-R3-RMDIR-DURABLE | the simulator's rmdir is durable at once, with no pending half | sim.rs | s06_rmdir_and_same_file_rename_follow_linux | [sim-conformance] | True | True | True | True | True |
| S-R3-REMOVED-DIR-ENTRY | a removed directory takes new entries | sim.rs | s06_rmdir_and_same_file_rename_follow_linux | [sim-conformance] | True | True | True | True | True |
| S-R3-SAME-FILE-RENAME | a rename between two names of one file removes the source name | sim.rs | s06_rmdir_and_same_file_rename_follow_linux | [sim-conformance] | True | True | True | True | True |

## I2-R1 suite (rerun with I3-I1's runner, unchanged)

Counted: 32; all as required: True; informational runs: 4; files identical after the run: True.

| control | intended test | as required |
|---|---|---|
| I2A-TRAIL | c04_decoding_takes_exactly_one_complete_frame | None |
| I2A-ACTION-GENERATION | c09_digests_bind_every_identity_instant_variant_and_field | None |
| I2A-RECORD-GENERATION | c09_digests_bind_every_identity_instant_variant_and_field | None |
| I2A-REQUEST-FIELD | c13_request_receipts_bind_the_full_payload | None |
| I2A-DEFAULT-TAG | c06_invalid_tags_booleans_and_presence_bytes_are_refused | None |
| I2A-DIGEST-BYPASS | c07_digest_and_payload_mutations_are_detected | None |
| I2A-INTENT-BYPASS | c12_core_refuses_acknowledgements_for_altered_bytes | None |
| I2A-DEBUG-RECORD | c11_core_issued_records_encode_decode_and_check | None |
| I1-NC1 | h01_same_owner_survives_failed_cleanup_and_exhausted_budgets | None |
| I1-NC2 | h12_unknown_outcome_blocks_launch_discard_and_fresh_admission | None |
| I1-NC3 | h11_dependencies_release_only_after_native_completion | None |
| I1-NC4 | h07_status_never_renews_the_lease | None |
| I1-NC5a | h14_capacity_reservation_prevents_unrecordable_admission | None |
| I1-NC5b | h14_capacity_reservation_prevents_unrecordable_admission | None |
| I1-NC6 | h19_replayed_evicted_conflicting_and_foreign_requests_execute_nothing | None |
| I1-NC7 | h10_unexpected_failure_stays_failed_after_recovery | None |
| R1-A1 | r01_actual_failure_closes_admission_within_its_case | None |
| R1-A2 | r02_failed_case_cleanup_never_permits_a_next_case | None |
| R1-B | r03_failed_evidence_keeps_the_custody_open | None |
| R1-C1 | r04_finalized_verdict_matches_acknowledged_terminal_evidence | None |
| R1-C2 | r05_late_owner_is_a_bound_incident_never_the_retired_action | None |
| R1-D | r06_detached_output_is_output_loss_never_a_clean_pass | None |
| R2-A1 | r07_cancellation_accepted_in_candidate_prevents_a_pass | None |
| R2-A2 | r07_cancellation_accepted_in_candidate_prevents_a_pass | None |
| R2-A3 | r11_finalization_winning_makes_a_later_cancellation_late | None |
| R2-A4 | r10_cancellation_while_the_recorder_is_paused_precedes_the_commitment | None |
| R2-B1 | r09_failed_first_attempt_of_a_retained_boundary_closes_admission_at_once | None |
| R2-B2 | r09_failed_first_attempt_of_a_retained_boundary_closes_admission_at_once | None |
| R2-C1 | r15_shutdown_request_is_no_second_finalization_rule | None |
| R2A-SPLIT | custody::core::commitment_boundary::r2a_commitment_excludes_cancellation_between_its_decision_and_its_record | None |
| NC-REQUEST-DEBUG | c17_core_retains_the_canonical_request_digest_in_its_receipts | None |
| NC-REQUEST-ALTERED-DIGEST | c17_core_retains_the_canonical_request_digest_in_its_receipts | None |

| informational run | test | expected | compiled | failed | passed | restored |
|---|---|---|---|---|---|---|
| NC-REQUEST-DEBUG-c13 | c13_request_receipts_bind_the_full_payload | pass | True | False | True | True |
| NC-REQUEST-DEBUG-c16 | c16_core_digest_paths_use_the_codec | fail (source guard) | True | True | False | True |
| NC-REQUEST-ALTERED-DIGEST-c13 | c13_request_receipts_bind_the_full_payload | pass | True | False | True | True |
| NC-REQUEST-ALTERED-DIGEST-c16 | c16_core_digest_paths_use_the_codec | pass | True | False | True | True |

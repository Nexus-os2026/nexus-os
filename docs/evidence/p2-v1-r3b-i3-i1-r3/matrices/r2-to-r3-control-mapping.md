# Store controls: R2 to R3 mapping

R2 counted controls: 137. R3 counted controls: 179. Remapped (not run, carried by API/type guards, as in R1 and R2): 1.

| R2 control | R3 control | status | changed fields | category | R3 result |
|---|---|---|---|---|---|
| S-F1-DURABLE | S-F1-DURABLE | unchanged | - | simulator-source-conformance | as required |
| S-REOPEN-CERTIFIES | S-REOPEN-CERTIFIES | unchanged | - | simulator-source-conformance | as required |
| S-TEAR-SPANS | S-TEAR-SPANS | unchanged | - | simulator-source-conformance | as required |
| S-SCHEDULES-GUARANTEED-ONLY | S-SCHEDULES-GUARANTEED-ONLY | unchanged | - | simulator-source-conformance | as required |
| S-NO-RECLAIM-WHILE-OPEN | S-NO-RECLAIM-WHILE-OPEN | unchanged | - | simulator-source-conformance | as required |
| S-ERRSEQ-NO-WRAP | S-ERRSEQ-NO-WRAP | unchanged | - | simulator-source-conformance | as required |
| S-COMPLETED-TESTS-ABORT | S-COMPLETED-TESTS-ABORT | unchanged | - | simulator-source-conformance | as required |
| S-INVENTED-ABORT | S-INVENTED-ABORT | unchanged | - | simulator-source-conformance | as required |
| S-INFLIGHT-COMPLETE | S-INFLIGHT-COMPLETE | unchanged | - | simulator-source-conformance | as required |
| S-HOME-ERROR-SUCCESS | S-HOME-ERROR-SUCCESS | unchanged | - | simulator-source-conformance | as required |
| S-TAIL-WITHOUT-HOME | S-TAIL-WITHOUT-HOME | unchanged | - | simulator-source-conformance | as required |
| S-WRITEBACK-ERROR-SWALLOWED | S-WRITEBACK-ERROR-SWALLOWED | unchanged | - | simulator-source-conformance | as required |
| I-HEADER-RESERVED | I-HEADER-RESERVED | unchanged | - | implementation-safety | as required |
| I-SEAL-TAIL | I-SEAL-TAIL | unchanged | - | implementation-safety | as required |
| I-RECORD-PADDING | I-RECORD-PADDING | unchanged | - | implementation-safety | as required |
| I-INVALID-HEADER-ABANDONED | I-INVALID-HEADER-ABANDONED | unchanged | - | implementation-safety | as required |
| I-DECIMAL-LEADING-ZERO | I-DECIMAL-LEADING-ZERO | unchanged | - | implementation-safety | as required |
| I-BINDING-LOCATION | I-BINDING-LOCATION | unchanged | - | implementation-safety | as required |
| I-BINDING-NO-CONTENT | I-BINDING-NO-CONTENT | unchanged | - | implementation-safety | as required |
| I-GRAMMAR-G16 | I-GRAMMAR-G16 | unchanged | - | implementation-safety | as required |
| I-REFUSE-ONLY-RECORDED | I-REFUSE-ONLY-RECORDED | unchanged | - | implementation-safety | as required |
| I-VISIBLE-RUNENDED-RESOLVES | I-VISIBLE-RUNENDED-RESOLVES | unchanged | - | implementation-safety | as required |
| I-LATE-NOTE-MISSING | I-LATE-NOTE-MISSING | unchanged | - | implementation-safety | as required |
| I-DECIDE-FROM-REPORT | I-DECIDE-FROM-REPORT | unchanged | - | implementation-safety | as required |
| I-NONDETERMINISTIC-REPORT | I-NONDETERMINISTIC-REPORT | unchanged | - | implementation-safety | as required |
| I-VALIDATOR-INEXACT | I-VALIDATOR-INEXACT | unchanged | - | implementation-safety | as required |
| I-PRESERVATION-SKIPPED | I-PRESERVATION-SKIPPED | unchanged | - | implementation-safety | as required |
| I-SYNC-BEFORE-LOCK | I-SYNC-BEFORE-LOCK | unchanged | - | implementation-safety | as required |
| I-NO-REVALIDATION | I-NO-REVALIDATION | unchanged | - | implementation-safety | as required |
| I-OPEN-BEFORE-TYPE | I-OPEN-BEFORE-TYPE | unchanged | - | implementation-safety | as required |
| I-PROVISION-INODE-IGNORED | I-PROVISION-INODE-IGNORED | unchanged | - | implementation-safety | as required |
| I-EXT4-MAGIC | I-EXT4-MAGIC | unchanged | - | implementation-safety | as required |
| I-NO-ACTIVATION | I-NO-ACTIVATION | unchanged | - | implementation-safety | as required |
| I-ACTIVATION-AFTER-SCAN | I-ACTIVATION-AFTER-SCAN | unchanged | - | implementation-safety | as required |
| I-R2-PROTOCOL | I-R2-PROTOCOL | unchanged | - | implementation-safety | as required |
| I-ACTIVATION-ERROR-IGNORED | I-ACTIVATION-ERROR-IGNORED | unchanged | - | implementation-safety | as required |
| I-NO-PROBE | I-NO-PROBE | unchanged | - | implementation-safety | as required |
| I-PROBE-FIRST | I-PROBE-FIRST | unchanged | - | implementation-safety | as required |
| I-NO-A4 | I-NO-A4 | unchanged | - | implementation-safety | as required |
| I-A2-IDENTITY | I-A2-IDENTITY | unchanged | - | implementation-safety | as required |
| I-KERNEL-PREFIX | I-KERNEL-PREFIX | unchanged | - | implementation-safety | as required |
| I-EXTERNAL-JOURNAL | I-EXTERNAL-JOURNAL | unchanged | - | implementation-safety | as required |
| I-EXCLUDED-MODE | I-EXCLUDED-MODE | unchanged | - | implementation-safety | as required |
| I-PROFILE-FROM-PINNED | I-PROFILE-FROM-PINNED | unchanged | - | implementation-safety | as required |
| I-NO-A4-PROFILE | I-NO-A4-PROFILE | unchanged | - | implementation-safety | as required |
| I-WRITE-CACHE-ALONE | I-WRITE-CACHE-ALONE | unchanged | - | implementation-safety | as required |
| I-IDENTITY-UNCHECKED | I-IDENTITY-UNCHECKED | unchanged | - | implementation-safety | as required |
| I-ATTESTATION-ADMITS | I-ATTESTATION-ADMITS | unchanged | - | implementation-safety | as required |
| I-CLAIM-UNBOUNDED | I-CLAIM-UNBOUNDED | unchanged | - | implementation-safety | as required |
| I-TMP-DISPOSITION | I-TMP-DISPOSITION | unchanged | - | implementation-safety | as required |
| I-BOUND-AFTER-EXAMINE | I-BOUND-AFTER-EXAMINE | unchanged | - | implementation-safety | as required |
| I-DURABLE-BEFORE-SYNC | I-DURABLE-BEFORE-SYNC | unchanged | - | implementation-safety | as required |
| I-SYNC-RETRIED-AFTER-EIO | I-SYNC-RETRIED-AFTER-EIO | unchanged | - | implementation-safety | as required |
| I-WORKER-IDENTITY | I-WORKER-IDENTITY | unchanged | - | implementation-safety | as required |
| I-NO-GATE | I-NO-GATE | unchanged | - | implementation-safety | as required |
| I-FAILURE-AT-CAUSE | I-FAILURE-AT-CAUSE | unchanged | - | implementation-safety | as required |
| I-FUTURE-SILENT | I-FUTURE-SILENT | unchanged | - | implementation-safety | as required |
| I-PUBLISH-AFTER-LATCH | I-PUBLISH-AFTER-LATCH | unchanged | - | implementation-safety | as required |
| I-UNEXPECTED-ACK-IGNORED | I-UNEXPECTED-ACK-IGNORED | unchanged | - | implementation-safety | as required |
| I-FAIL-UNISSUED | I-FAIL-UNISSUED | unchanged | - | implementation-safety | as required |
| I-DELIVERY-DROPPED | I-DELIVERY-DROPPED | unchanged | - | implementation-safety | as required |
| I-DUP-CONFLICT | I-DUP-CONFLICT | unchanged | - | implementation-safety | as required |
| I-DUP-RETAINED | I-DUP-RETAINED | unchanged | - | implementation-safety | as required |
| I-TIMEOUT-STOPS | I-TIMEOUT-STOPS | unchanged | - | implementation-safety | as required |
| I-POISON-IGNORED | I-POISON-IGNORED | unchanged | - | implementation-safety | as required |
| I-VANISHED-IGNORED | I-VANISHED-IGNORED | unchanged | - | implementation-safety | as required |
| I-NO-DROP-GUARD | I-NO-DROP-GUARD | unchanged | - | implementation-safety | as required |
| I-SEAL-WHILE-LATCHED | I-SEAL-WHILE-LATCHED | unchanged | - | implementation-safety | as required |
| I-SESSION-NEW-LOCK | I-SESSION-NEW-LOCK | unchanged | - | implementation-safety | as required |
| I-SESSION-SHARED | I-SESSION-SHARED | unchanged | - | implementation-safety | as required |
| I-SESSION-RELOCK | I-SESSION-RELOCK | unchanged | - | implementation-safety | as required |
| I-BUSY-SUCCESS | I-BUSY-SUCCESS | unchanged | - | implementation-safety | as required |
| I-AUTHORITY-FLAG | I-AUTHORITY-FLAG | unchanged | - | implementation-safety | as required |
| I-VERIFY-CACHED | I-VERIFY-CACHED | unchanged | - | implementation-safety | as required |
| I-RETIRE-NO-PRECONDITIONS | I-RETIRE-NO-PRECONDITIONS | unchanged | - | implementation-safety | as required |
| I-RETIRE-FROM-REPORT | I-RETIRE-FROM-REPORT | unchanged | - | implementation-safety | as required |
| I-HIDDEN-STEP | I-HIDDEN-STEP | unchanged | - | implementation-safety | as required |
| I-FRESH-ROOT | I-FRESH-ROOT | unchanged | - | implementation-safety | as required |
| N-DOT-NAME | N-DOT-NAME | unchanged | - | implementation-safety | as required |
| N-FOLLOW | N-FOLLOW | unchanged | - | implementation-safety | as required |
| N-BLOCKING | N-BLOCKING | unchanged | - | implementation-safety | as required |
| A-ENTRY-CALLS | A-ENTRY-CALLS | unchanged | - | authority-api-surface | as required |
| A-ENTRY-SWITCH | A-ENTRY-SWITCH | unchanged | - | authority-api-surface | as required |
| A-ADMISSION-CLONE | A-ADMISSION-CLONE | unchanged | - | authority-api-surface | as required |
| A-ADMISSION-FORGED | A-ADMISSION-FORGED | unchanged | - | authority-api-surface | as required |
| A-LOCK-UNLOCK | A-LOCK-UNLOCK | unchanged | - | authority-api-surface | as required |
| A-WORKER-LOCK | A-WORKER-LOCK | unchanged | - | authority-api-surface | as required |
| A-GATE-SPLIT | A-GATE-SPLIT | unchanged | - | authority-api-surface | as required |
| A-SELECTION-KEPT | A-SELECTION-KEPT | unchanged | - | authority-api-surface | as required |
| R1-REFUSED-CLOSE-SEALS | R1-REFUSED-CLOSE-SEALS | unchanged | - | authority-binding | as required |
| R1-START-FROM-REPORT | R1-START-FROM-REPORT | unchanged | - | authority-binding | as required |
| R1-VALIDATOR-UNCHECKED | R1-VALIDATOR-UNCHECKED | unchanged | - | authority-binding | as required |
| R1-APPLIED-REASON-INVENTED | R1-APPLIED-REASON-INVENTED | unchanged | - | authority-binding | as required |
| R1-FAULT-ACKS-UNDURABLE | R1-FAULT-ACKS-UNDURABLE | unchanged | - | authority-binding | as required |
| R1-FAULT-INJECTS-NEXT | R1-FAULT-INJECTS-NEXT | unchanged | - | authority-binding | as required |
| R1-LATCH-REPLACED | R1-LATCH-REPLACED | unchanged | - | authority-binding | as required |
| R1-VERIFICATION-REUSED | R1-VERIFICATION-REUSED | unchanged | - | authority-binding | as required |
| R1-VERIFICATION-NEVER-LAPSES | R1-VERIFICATION-NEVER-LAPSES | unchanged | - | authority-binding | as required |
| R1-GUARD-NOT-KEPT | R1-GUARD-NOT-KEPT | unchanged | - | authority-binding | as required |
| NC-SUCC-NO-VERIFY | NC-SUCC-NO-VERIFY | unchanged | - | maintenance-authority | as required |
| NC-SUCC-MISSING-DISPOSITION | NC-SUCC-MISSING-DISPOSITION | unchanged | - | maintenance-authority | as required |
| NC-SUCC-REPORT-TRUNCATION | NC-SUCC-REPORT-TRUNCATION | unchanged | - | maintenance-authority | as required |
| NC-SUCC-STALE | NC-SUCC-STALE | unchanged | - | maintenance-authority | as required |
| NC-SUCC-REUSE-AUTH | NC-SUCC-REUSE-AUTH | unchanged | - | maintenance-authority | as required |
| NC-SUCC-OMIT-INVALID | NC-SUCC-OMIT-INVALID | unchanged | - | maintenance-authority | as required |
| NC-SUCC-INVENT-INVALID | NC-SUCC-INVENT-INVALID | unchanged | - | maintenance-authority | as required |
| NC-SUCC-REPEAT-INVALID | NC-SUCC-REPEAT-INVALID | unchanged | - | maintenance-authority | as required |
| NC-SUCC-STATEMENT-CALLER | NC-SUCC-STATEMENT-CALLER | unchanged | - | maintenance-authority | as required |
| NC-SUCC-STATEMENT-TRUNCATED | NC-SUCC-STATEMENT-TRUNCATED | unchanged | - | maintenance-authority | as required |
| NC-SUCC-CAPACITY-BYPASS | NC-SUCC-CAPACITY-BYPASS | unchanged | - | maintenance-authority | as required |
| NC-SUCC-NO-POSTVERIFY | NC-SUCC-NO-POSTVERIFY | unchanged | - | maintenance-authority | as required |
| NC-SUCC-POSTVERIFY-IGNORED | NC-SUCC-POSTVERIFY-IGNORED | unchanged | - | maintenance-authority | as required |
| NC-SUCC-VERIFY-OLD-ROOT | NC-SUCC-VERIFY-OLD-ROOT | unchanged | - | maintenance-authority | as required |
| NC-SUCC-NO-GATE | NC-SUCC-NO-GATE | unchanged | - | maintenance-authority | as required |
| NC-SUCC-MUTATE-BEFORE-GATE | NC-SUCC-MUTATE-BEFORE-GATE | adapted | what, edits (R3's P-REVOKE gate rereads its admissibility, so a gate moved after the first operation of every procedure refuses v12's helper revocation after its own rename, before the succession v12 examines; the mutation is confined to the succession's gate (S17's subject)) | maintenance-authority | as required |
| NC-SUCC-GATE-PROVISION | NC-SUCC-GATE-PROVISION | unchanged | - | maintenance-authority | as required |
| NC-SUCC-UNDISPOSITIONED-HISTORY | NC-SUCC-UNDISPOSITIONED-HISTORY | unchanged | - | maintenance-authority | as required |
| NC-SUCC-LEFTOVER | NC-SUCC-LEFTOVER | unchanged | - | maintenance-authority | as required |
| NC-SUCC-SAME-ROOT | NC-SUCC-SAME-ROOT | adapted | what, edits (R3 also refuses a successor whose state root exists, and v22's layout is the predecessor's own; the equivalent defect point removes both R2's root-id check and R3's state-root check) | maintenance-authority | as required |
| NC-SUCC-DUAL-LOCK | NC-SUCC-DUAL-LOCK | unchanged | - | maintenance-authority | as required |
| NC-SUCC-CAPACITY-REFUSED | NC-SUCC-CAPACITY-REFUSED | unchanged | - | maintenance-authority | as required |
| NC-VERIFY-FIRST-ONLY | NC-VERIFY-FIRST-ONLY | unchanged | - | maintenance-authority | as required |
| NC-OWNER-COLLECTS | NC-OWNER-COLLECTS | unchanged | - | maintenance-authority | as required |
| NC-VERIFY-AFTER-AUTHORIZES | NC-VERIFY-AFTER-AUTHORIZES | unchanged | - | maintenance-authority | as required |
| NC-GAP-UNBOUNDED | NC-GAP-UNBOUNDED | unchanged | - | maintenance-authority | as required |
| NC-ENTRY-STAT-DETERMINATE | NC-ENTRY-STAT-DETERMINATE | unchanged | - | maintenance-authority | as required |
| NC-REVOKED-BOUND-INDETERMINATE | NC-REVOKED-BOUND-INDETERMINATE | unchanged | - | maintenance-authority | as required |
| NC-NAMES-EXAMINED-TWICE | NC-NAMES-EXAMINED-TWICE | unchanged | - | maintenance-authority | as required |
| NC-DISP-UNREPORTED | NC-DISP-UNREPORTED | unchanged | - | maintenance-authority | as required |
| NC-DISP-FACTS-NOT-VERIFIED | NC-DISP-FACTS-NOT-VERIFIED | unchanged | - | maintenance-authority | as required |
| NC-DISP-ARCHIVED-NAME-ONLY | NC-DISP-ARCHIVED-NAME-ONLY | unchanged | - | maintenance-authority | as required |
| NC-ARCHIVE-CAPACITY | NC-ARCHIVE-CAPACITY | unchanged | - | maintenance-authority | as required |
| NC-LEFTOVER-AFTER-ACCEPTED | NC-LEFTOVER-AFTER-ACCEPTED | unchanged | - | maintenance-authority | as required |
| NC-DISP-NO-POSTVERIFY | NC-DISP-NO-POSTVERIFY | unchanged | - | maintenance-authority | as required |
| NC-PROV-OVER-EXISTING | NC-PROV-OVER-EXISTING | unchanged | - | maintenance-authority | as required |
| NC-REPUBLISH-OVER-EXISTING | NC-REPUBLISH-OVER-EXISTING | unchanged | - | maintenance-authority | as required |
| NC-PROV-OVER-HISTORY | NC-PROV-OVER-HISTORY | unchanged | - | maintenance-authority | as required |
| - | NC-R3-SUCC-KEEP-OVERWRITE | added (R3) | - | administrative-recovery | as required |
| - | NC-R3-SUCC-KEEP-BLESS | added (R3) | - | administrative-recovery | as required |
| - | NC-R3-SUCC-NO-SKIP | added (R3) | - | administrative-recovery | as required |
| - | NC-R3-SUCC-ORDINARY-SKIP | added (R3) | - | administrative-recovery | as required |
| - | NC-R3-SUCC-KEEP-METADATA | added (R3) | - | administrative-recovery | as required |
| - | NC-R3-SUCC-LEFTOVER-IGNORED | added (R3) | - | administrative-recovery | as required |
| - | NC-R3-SUCC-ROOT-IGNORED | added (R3) | - | administrative-recovery | as required |
| - | NC-R3-SUCC-GATE-COPY | added (R3) | - | administrative-recovery | as required |
| - | NC-R3-SUCC-CLEAN-NONZERO | added (R3) | - | administrative-recovery | as required |
| - | NC-R3-SUCC-CLEAN-UNEXPECTED | added (R3) | - | administrative-recovery | as required |
| - | NC-R3-SUCC-CLEAN-SELECTED | added (R3) | - | administrative-recovery | as required |
| - | NC-R3-SUCC-CLEAN-UID | added (R3) | - | administrative-recovery | as required |
| - | NC-R3-SUCC-CLEAN-COPY-CONTENT | added (R3) | - | administrative-recovery | as required |
| - | NC-R3-SUCC-CLEAN-NOT-INTERRUPTED | added (R3) | - | administrative-recovery | as required |
| - | NC-R3-SUCC-CLEAN-NO-LOCK | added (R3) | - | administrative-recovery | as required |
| - | NC-R3-SUCC-CLEAN-RECURSIVE | added (R3) | - | administrative-recovery | as required |
| - | NC-R3-SUCC-CLEAN-RECURSIVE-API | added (R3) | - | authority-api-surface | as required |
| - | NC-R3-SUCC-CLEAN-IDENTITY | added (R3) | - | administrative-recovery | as required |
| - | NC-R3-SUCC-CLEAN-GATE | added (R3) | - | administrative-recovery | as required |
| - | NC-R3-SUCC-CLEAN-GONE-REFUSED | added (R3) | - | administrative-recovery | as required |
| - | NC-R3-SUCC-CLEAN-NO-PARENT-SYNC | added (R3) | - | administrative-recovery | as required |
| - | NC-R3-SUCC-REUSE-VERIFY | added (R3) | - | administrative-recovery | as required |
| - | NC-R3-RECOVERY-NO-POSTVERIFY-SUCC | added (R3) | - | administrative-recovery | as required |
| - | NC-R3-REVOKE-BOUND-EARLY | added (R3) | - | administrative-recovery | as required |
| - | NC-R3-REVOKE-OVERFLOW | added (R3) | - | administrative-recovery | as required |
| - | NC-R3-REVOKE-REPLACE | added (R3) | - | administrative-recovery | as required |
| - | NC-R3-REVOKE-TIME-IGNORED | added (R3) | - | administrative-recovery | as required |
| - | NC-R3-REVOKE-NO-VERIFY-AFTER | added (R3) | - | administrative-recovery | as required |
| - | NC-R3-REVOKE-GATE | added (R3) | - | administrative-recovery | as required |
| - | NC-R3-REVOKE-RECOVER-WRONG | added (R3) | - | administrative-recovery | as required |
| - | NC-R3-REVOKE-RECOVER-NAME | added (R3) | - | administrative-recovery | as required |
| - | NC-R3-REVOKE-RECOVER-DELETE-TARGET | added (R3) | - | administrative-recovery | as required |
| - | NC-R3-REVOKE-RECOVER-NO-SYNC | added (R3) | - | administrative-recovery | as required |
| - | NC-R3-REVOKE-CAPACITY-CONFLATED | added (R3) | - | administrative-recovery | as required |
| - | NC-R3-REVOKE-COMPLETED-REFUSED | added (R3) | - | administrative-recovery | as required |
| - | NC-R3-RECOVERY-NO-POSTVERIFY | added (R3) | - | administrative-recovery | as required |
| - | S-R3-RMDIR-NOT-EMPTY | added (R3) | - | simulator-source-conformance | as required |
| - | S-R3-RMDIR-DURABLE | added (R3) | - | simulator-source-conformance | as required |
| - | S-R3-REMOVED-DIR-ENTRY | added (R3) | - | simulator-source-conformance | as required |
| - | S-R3-SAME-FILE-RENAME | added (R3) | - | simulator-source-conformance | as required |
| - | N-R3-RMDIR-FLAG | added (R3) | - | implementation-safety | as required |
| - | N-R3-RMDIR-NAME | added (R3) | - | implementation-safety | as required |

- adapted: 2
- added (R3): 42
- unchanged: 135

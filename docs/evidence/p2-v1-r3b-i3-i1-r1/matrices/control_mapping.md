# Store controls: I3-I1 to R1 mapping

I3-I1 controls: 90. R1 counted controls: 99. Remapped (not run, carried by API/type guards): 1.

| I3-I1 control | R1 control | status | changed fields | category | R1 result |
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
| I-VALIDATOR-INEXACT | I-VALIDATOR-INEXACT | adapted | edits | implementation-safety | as required |
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
| I-SEAL-WHILE-LATCHED | I-SEAL-WHILE-LATCHED | adapted | edits, require | implementation-safety | as required |
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
| A-ADMISSION-UNBOUND | - | remapped to API/type guards | carried by: P-B3-CLAIM-CALL (the claim is internal; it takes no caller admission); P-B4-IDENTITY-EDIT (an opening's identity and selection cannot be rebound); P-B4-OPENING-MINT (no opening identity is minted outside the store); P-B4-ADMISSION-FORGE (no admission is built from parts) | authority-api-surface | - |
| A-LOCK-UNLOCK | A-LOCK-UNLOCK | unchanged | - | authority-api-surface | as required |
| A-WORKER-LOCK | A-WORKER-LOCK | adapted | edits | authority-api-surface | as required |
| A-GATE-SPLIT | A-GATE-SPLIT | unchanged | - | authority-api-surface | as required |
| A-SELECTION-KEPT | A-SELECTION-KEPT | unchanged | - | authority-api-surface | as required |
| - | R1-REFUSED-CLOSE-SEALS | added (R1) | - | authority-binding | as required |
| - | R1-START-FROM-REPORT | added (R1) | - | authority-binding | as required |
| - | R1-VALIDATOR-UNCHECKED | added (R1) | - | authority-binding | as required |
| - | R1-APPLIED-REASON-INVENTED | added (R1) | - | authority-binding | as required |
| - | R1-FAULT-ACKS-UNDURABLE | added (R1) | - | authority-binding | as required |
| - | R1-FAULT-INJECTS-NEXT | added (R1) | - | authority-binding | as required |
| - | R1-LATCH-REPLACED | added (R1) | - | authority-binding | as required |
| - | R1-VERIFICATION-REUSED | added (R1) | - | authority-binding | as required |
| - | R1-VERIFICATION-NEVER-LAPSES | added (R1) | - | authority-binding | as required |
| - | R1-GUARD-NOT-KEPT | added (R1) | - | authority-binding | as required |

- adapted: 3
- added (R1): 10
- remapped to API/type guards: 1
- unchanged: 86

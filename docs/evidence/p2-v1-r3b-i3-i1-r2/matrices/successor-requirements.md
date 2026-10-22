# Successor requirement coverage (P2-V1-R3B-I3-I1-R2)

Each requirement's tests (store target, as run), counted behavioural controls and API/type guards. The three are listed apart and never added together.

| requirement | statement | tests | behavioural controls | API/type guards |
|---|---|---|---|---|
| S01 | An undispositioned bound predecessor incident: authorization refused before any successor mutation | v01 (as required); v11 (as required) | NC-SUCC-MISSING-DISPOSITION (as required) | - |
| S02 | Every bound incident exactly dispositioned: the succession may proceed | v01 (as required); v02 (as required) | - | - |
| S03 | One of several bound incidents without a disposition: refused | v01 (as required) | NC-SUCC-MISSING-DISPOSITION (as required) | - |
| S04 | A truncated report: authority uses the complete verified set | v03 (as required) | NC-SUCC-REPORT-TRUNCATION (as required) | - |
| S05 | A verification followed by a session mutation: stale, authorizes nothing | v04 (as required) | NC-SUCC-STALE (as required) | - |
| S06 | Another session's or store's verification authorizes nothing | v05 (as required) | NC-SUCC-NO-VERIFY (as required); NC-VERIFY-AFTER-AUTHORIZES (as required) | P-VERIFICATION-TRANSPLANT (as required); P-VERIFICATION-FORGE (as required); P-SESSION-VERIFIED (as required) |
| S07 | Capacity with a bounded complete proof: the allowed path | v06 (as required); v18 (as required) | NC-SUCC-CAPACITY-REFUSED (as required); NC-ARCHIVE-CAPACITY (as required) | - |
| S08 | Capacity whose complete proof cannot be established: refused | v07 (as required) | NC-SUCC-CAPACITY-BYPASS (as required); NC-GAP-UNBOUNDED (as required) | - |
| S09 | A verified Invalid condition omitted from the Owner's acceptance: refused | v08 (as required) | NC-SUCC-OMIT-INVALID (as required) | - |
| S10 | A fabricated extra Invalid condition: refused | v08 (as required) | NC-SUCC-INVENT-INVALID (as required) | - |
| S11 | The canonical complete acceptance, in any order: accepted (a repeat refuses) | v08 (as required); v20 (as required) | NC-SUCC-REPEAT-INVALID (as required) | - |
| S12 | The persisted predecessor statement is exactly the accepted verified set (or none); over 512 bytes refuses | v02 (as required); v08 (as required); v09 (as required); v20 (as required) | NC-SUCC-STATEMENT-CALLER (as required); NC-SUCC-STATEMENT-TRUNCATED (as required) | - |
| S13 | Publication followed by a successful in-session verify-after: complete | v02 (as required) | NC-SUCC-NO-POSTVERIFY (as required) | - |
| S14 | A failing verify-after: not complete, and later openings refuse | v10 (as required) | NC-SUCC-POSTVERIFY-IGNORED (as required); NC-SUCC-VERIFY-OLD-ROOT (as required) | - |
| S15 | No constructed report, decision, assessment or authorization bypasses verification | v11 (as required) | - | P-VERIFICATION-FORGE (as required); P-TAKE-ASSESSMENT (as required); P-VERIFICATION-TRANSPLANT (as required); P-SESSION-VERIFIED (as required); P-B3-DECISION-EDIT (as required); P-B3-SCAN-EDIT (as required) |
| S16 | A second use of a consumed or stale authorization: refused | v04 (as required) | NC-SUCC-REUSE-AUTH (as required); NC-SUCC-STALE (as required) | - |
| S17 | No successor mutation before the authorization gate | v12 (as required) | NC-SUCC-MUTATE-BEFORE-GATE (as required); NC-SUCC-NO-GATE (as required); NC-SUCC-GATE-PROVISION (as required) | - |
| S18 | Both stores' locks retained through publication, re-selection and verify-after | v13 (as required) | NC-SUCC-DUAL-LOCK (as required) | - |
| 8A | The session still holds and revalidates the predecessor's lock and selection | v12 (as required); v13 (as required); m01 (as required) | NC-SUCC-GATE-PROVISION (as required); I-SESSION-NEW-LOCK (as required); I-SESSION-SHARED (as required); I-SESSION-RELOCK (as required); I-AUTHORITY-FLAG (as required) | - |
| 8B | The verification is this session's, of this root and selection, at this mutation epoch | v04 (as required); v05 (as required) | NC-SUCC-STALE (as required); NC-SUCC-REUSE-AUTH (as required); NC-SUCC-NO-VERIFY (as required) | P-VERIFICATION-TRANSPLANT (as required); P-VERIFICATION-FORGE (as required); P-TAKE-ASSESSMENT (as required) |
| 8C | Every bound incident (current and archived) has an exact valid disposition | v01 (as required); v16 (as required) | NC-SUCC-MISSING-DISPOSITION (as required); NC-SUCC-UNDISPOSITIONED-HISTORY (as required) | - |
| 8D | No bound incident omitted (truncated detail, incident_limit, Capacity, caller selection) | v03 (as required); v06 (as required); v07 (as required) | NC-SUCC-REPORT-TRUNCATION (as required); NC-SUCC-CAPACITY-BYPASS (as required); NC-GAP-UNBOUNDED (as required) | - |
| 8E | Every store-level Invalid condition represented exactly | v08 (as required); v20 (as required); v23 (as required) | NC-VERIFY-FIRST-ONLY (as required); NC-SUCC-OMIT-INVALID (as required); NC-SUCC-INVENT-INVALID (as required); NC-SUCC-REPEAT-INVALID (as required); NC-ENTRY-STAT-DETERMINATE (as required); NC-REVOKED-BOUND-INDETERMINATE (as required); NC-NAMES-EXAMINED-TWICE (as required) | - |
| 8F | The predecessor statement generated from that exact condition set | v02 (as required); v08 (as required); v09 (as required) | NC-SUCC-STATEMENT-CALLER (as required); NC-SUCC-STATEMENT-TRUNCATED (as required) | - |
| 9 | Lifecycle order: verify-before, gate, predecessor kept, successor created, its lock, publication, re-selection, verify-after, then complete (29 operations of 15.3) | v02 (as required); v12 (as required); v13 (as required); m08 (as required); m09 (as required) | NC-SUCC-MUTATE-BEFORE-GATE (as required); NC-SUCC-NO-POSTVERIFY (as required); NC-SUCC-VERIFY-OLD-ROOT (as required); NC-SUCC-DUAL-LOCK (as required) | - |
| 9-fail | A failed verify-after: no success, evidence kept, nothing rolled back, later openings fail-closed | v10 (as required) | NC-SUCC-POSTVERIFY-IGNORED (as required) | - |
| 9-new | A recovered predecessor and a new root id | v22 (as required) | NC-SUCC-LEFTOVER (as required); NC-SUCC-SAME-ROOT (as required) | - |
| 12 | Several coexisting Invalid conditions: stable order, no duplicate identity, no first-error-only authorization; Indeterminate refuses | v08 (as required); v14 (as required); v20 (as required); v23 (as required) | NC-VERIFY-FIRST-ONLY (as required); NC-OWNER-COLLECTS (as required); NC-ENTRY-STAT-DETERMINATE (as required); NC-NAMES-EXAMINED-TWICE (as required) | - |
| 13 | Verify-after part of the procedure: mutation distinguishable from verified completion; lock held; no early end reported complete; fresh reads; no reuse; failure kept as evidence | v02 (as required); v19 (as required); v21 (as required); v24 (as required); m01 (as required) | NC-DISP-NO-POSTVERIFY (as required); NC-LEFTOVER-AFTER-ACCEPTED (as required); NC-VERIFY-AFTER-AUTHORIZES (as required); I-VERIFY-CACHED (as required) | - |
| 10-disp | Disposition publication restates only verified facts | v15 (as required); v16 (as required) | NC-DISP-UNREPORTED (as required); NC-DISP-FACTS-NOT-VERIFIED (as required); NC-DISP-ARCHIVED-NAME-ONLY (as required) | - |
| 10-prov | Provisioning and re-publication never replace a store | v17 (as required); m06 (as required) | NC-PROV-OVER-EXISTING (as required); NC-REPUBLISH-OVER-EXISTING (as required); NC-PROV-OVER-HISTORY (as required); I-FRESH-ROOT (as required) | - |

RESULT: every mapped item as required

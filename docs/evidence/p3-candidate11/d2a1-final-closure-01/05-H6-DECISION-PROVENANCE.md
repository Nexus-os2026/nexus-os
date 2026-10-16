# Stage 5 — decision provenance of everything the attestation cites

Kinds: **RAW** a measurement as printed; **ACCEPTED** evidence an Architect ruling accepted; **DERIVED** a proof
obligation; **EXEC** executor analysis; **ARCH** an Architect decision; **OWNER** an Owner decision; **PENDING**.
"Record" means executor-written audit evidence of a decision conveyed in mission text. None is signed.

| # | Decision or claim | Kind | Record and its limits |
|---|---|---|---|
| 1 | AD-1-R4 adopted (2026-10-14) | OWNER | `ad1r4-adoption/OWNER-ADOPTION-2026-10-14.md` (in C, blob `54c956bc`) |
| 2 | AD-1-R5 adopted, with the `Config.name` ruling and the F-R proof discharge (2026-10-14) | OWNER (+ ARCH for `Config.name`) | `ad1r5-adoption/OWNER-ADOPTION-2026-10-14.md` (blob `1492e185`) |
| 3 | AD-C7-W adopted for C7 (2026-10-15) and its identities approved (2026-10-16); CR-B B-W/A | OWNER | blobs `f9ce488c`, `cfcbf998` |
| 4 | Candidate C frozen as the exact final source baseline; AQ/SQ proofs accepted for C; "All 22 stale M-control mappings: SAME PROPERTY" | ARCH (relayed, FINAL-EVIDENCE-STAGE1) | `final-baseline-known-runs-01/FREEZE-RECORD.txt` (blob `4321cb34`) |
| 5 | The 14 exact-final configurations accepted; F-R INCOMPLETE — STATE CEILING REACHED | ARCH (relayed, FINAL-EVIDENCE-STAGE2) | **restated only** in AD-1-R5 §0 and `PROPOSED-20-CONFIGURATION-ACCOUNTING.md`; no verbatim ruling record found |
| 6 | 33 mutation controls accepted by ID; C1 and C25 recordings | ARCH | nine records (`01-ACCEPTANCE-PROVENANCE.md`) |
| 7 | C4 accepted: "C4: P4, P8 (first failure P4; witness)" | ARCH (conversational) | this mission's section 1 only; **no Git record** |
| 8 | C7 accepted: "C7: P6 (first failure E11; witness)" (P3-C11-DR01-D2A1-C7-ACCEPTANCE-01) | ARCH (conversational) | this mission's section 1 only; **no Git record** |
| 9 | A4 SATISFIED — ARCHITECT ACCEPTED | ARCH | stated in every tranche ruling record |
| 10 | Build admissions for C28, C5, C24 (and C3b/C3c) | ARCH | three by-ID admissibility records |
| 11 | M18 source-level disposition | ARCH (conveyed; not an acceptance) | mission text (CLOSURE-PACKET-01; this mission section 8) |
| 12 | NO SECTION 11 LABEL | ARCH | this mission's sections 1 and 11 |
| 13 | AD-1-R5 C(7): the Architect's independent review of the occurrence closure, the proof and the branch accounting | ARCH (conveyed through #2) | no separate by-ID review record of C(4)–C(6) |
| 14 | The 20 configuration verdicts | RAW + ACCEPTED (#5) | raw receipts; `02-CONFIGURATIONS.tsv` |
| 15 | F-R safety, D1 and L1 | DERIVED | the accepted proof `c97332a3…606b`, from F-2's PASS |
| 16 | Branch accounting | EXEC (consolidation of RAW and DERIVED) | `02-BRANCH-VOCABULARY.tsv` |
| 17 | ct-1 alignment; the 22 reconciliations; M18 trace | EXEC | `03-*` |
| 18 | Current-tree acceptance of any of the 183 | **PENDING** | none exists |
| 19 | Fast Local | RAW (three Rust steps) + EXEC (matrix); verdict NOT FULLY DISCHARGED | `04-*` |
| 20 | Ratification of validation deviations (VALIDATION-02 list; this mission's two preserved failures) | **PENDING** | `02-ASSURANCE-LIMITATIONS.md` item 12 |

Nothing in this table was decided by the executor.

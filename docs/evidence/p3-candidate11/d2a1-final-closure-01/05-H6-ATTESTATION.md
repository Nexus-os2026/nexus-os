# Stage 5 — AD-1-R4 H.6 detached attestation: **CANDIDATE — PENDING DECISIONS**

**This is a candidate, not an acceptance.** It is not signed, approves nothing and is not published. A SHA256
authenticates the bytes of each bound input, not the truth of what those bytes say. Only the Sole Architect may accept
the attestation; every Owner decision stays the Owner's.

| Item | Value |
|---|---|
| Machine-readable attestation | `05-H6-ATTESTATION-CANDIDATE.json`, sha256 `ac3a73a501c1478afd7dabd6ae531b0135a2d6c0553c81cad29e92cf42ca64a6` |
| Bound inputs (each re-hashed at generation) | `05-H6-INPUT-HASHES.tsv`, 140 rows, sha256 `c095257b5520914abffeba641e4808b40fef084501ad05b74f234c7c7ad70bdb` |
| Generator | `tools/s5_h6.py`, frozen in `receipts/50-STAGE5-TOOL-FREEZE.txt`, run once (`receipts/51-STAGE5-RUN-CONSOLE.txt`) |
| Detachment | it lives only in this local evidence root, outside the repository; it is not in Candidate C or in any commit whose identity it attests; no commit, push or publication was made (H.6: "never placed in the commit it attests") |

## What it binds (H.6 and the mission's items A–K)

| Item | Bound value | Nature |
|---|---|---|
| A. exact final source | commit `d5964ee5326c2733177307db6f507328023d361a`, tree `51bac5fd335999cc995c0ab9f582a30244973ef0`, the three ledger blobs, E-C `cde9f9d2…d30c` | SOURCE |
| B. ct-1 | path `docs/evidence/p3-candidate11/current-tree-183/manifest.json`, blob `5d31cb45…`, sha256 `20bcc6fe…1a44279`, version `ct-1` | SOURCE (candidate manifest; confers nothing) |
| C. all-183 alignment | ALIGNED (H.1–H.5): 183 / 184 / 39 / 191; 162 equal, 22 changed; all statuses `pending` | EXECUTOR ANALYSIS (mechanical) |
| D. the 22 reconciliations | 21 ESTABLISHED UNDER THE EXISTING (relayed) SAME PROPERTY RULING + MECHANICALLY ALIGNED; M18 MATERIALLY CHANGED OBSERVABLE MECHANISM — REQUIRES EXPLICIT ARCHITECT REVIEW | EXECUTOR ANALYSIS + ARCHITECT DECISION RECORD (relayed) |
| E. the 35 D2A-1 mutation rulings | 35 ACCEPTED: 33 by nine by-ID Git records; C4 and C7 by conversational rulings (no Git record); every failing, witness and restored-side receipt hash | ARCHITECT DECISION RECORDS (executor-written) + RAW MEASUREMENTS |
| F. every configuration receipt | 19 admissible PASS raw hashes (3 composed-quotient); F-R's INCOMPLETE raw hash; F-R recorded as "F-R — UNCHANGED-CORE OBLIGATIONS DISCHARGED BY ACCEPTED STUTTERING-REFINEMENT PROOF", with the proof, the disposition and AD-1-R5's adoption | RAW MEASUREMENTS + DERIVED PROOF OBLIGATION |
| G. 8.4 branch accounting | 193 rows: 85 MEASURED_COMPLETE, 3 MEASURED_IN_INCOMPLETE_PREFIX, 105 UNOBSERVED; proof sources recorded separately | EXECUTOR ANALYSIS (consolidation) |
| H. Fast Local | FAST_LOCAL_NOT_FULLY_DISCHARGED; fmt, all-features Clippy and workspace tests evidenced at C; 21 steps not discharged | RAW MEASUREMENTS + EXECUTOR ANALYSIS |
| I. current-tree review decisions | the relayed SAME PROPERTY ruling (22); the conveyed M18 source-level disposition; **no current-tree acceptance for any of the 183** | ARCHITECT DECISIONS (relayed/conveyed) + PENDING |
| J. assumptions and limitations | A-1, A-2 undischarged; AQ6; derived quotient counts; F-R derived claims; 105 unobserved branches; dormant core; unsigned records | — |
| K. outstanding decisions | `05-H6-UNRESOLVED.md` | PENDING |

Why it is a CANDIDATE and not complete: H.6 must bind "the actual current review decisions, each with its source"
for the current-tree manifest. No current-tree review decision exists for any of the 183 controls, the M18 mapping
needs an explicit decision, and the campaign-scope question (contract line 915) is unruled. The attestation cannot
supply those decisions and does not invent them.

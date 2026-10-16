# Stage 0 — contract chain

Each document's bytes were re-hashed from a Git object by this mission (`raw/00-contract-auth.txt`,
`raw/00-ad-auth.txt`; byte copies in `raw/`). Each adoption record is executor-written audit evidence of an Owner
decision conveyed by the Sole Architect; none is signed and none is authority by itself.

| Layer | Document | Identity | Status, with provenance | Governs |
|---|---|---|---|---|
| 1 | P3-C11-D2A1-CONTRACT-02-R1 | sha256 `cac6535115fb333e68e6c3c045e444342c950ab7a735b7b7deea21cb2cb53b3c`, 107,331 bytes, 1,462 lines; Git blob `a5ef321a…` at `8693e3a7` | the contract; binding reviews `6dfb1456…cdce` (REVIEW-02) and `1f964d75…fb35` (REVIEW-03) | everything not superseded |
| 2 | ADDENDUM AD-1-R4 | blob `35d5074c2daa2ce9602fee3cd23dc0ccd6aef5b1`, sha256 `00ced6d535c78f34f9e92233b742126a7b19192bf3ac8348056be2665192b693`, 143,220 bytes; in C | **Owner adopted 2026-10-14** — record `ad1r4-adoption/OWNER-ADOPTION-2026-10-14.md` (in C, blob `54c956bc`) | supersession table S-1 to S-25; sections A–R |
| 3 | ADDENDUM AD-1-R5 (supplement to AD-1-R4) | blob `ae3cabdb45627f9b3179a098c50d27e50c55101f`, sha256 `4e1ed8c92dbb2f3706eb524fa8b4fae766f027e7027f54049331ab312bb91473`, 16,067 bytes; evidence commit `c1e46fc4` | **Owner adopted 2026-10-14** — record `ad1r5-adoption/OWNER-ADOPTION-2026-10-14.md` (blob `1492e185`, sha256 `9990d256…aa4`), with the accepted supporting F-R proof (blob `c631a061`, sha256 `c97332a3…606b`) and the `Config.name` ruling | F-R and C15 only (S5-1 to S5-7) |
| 4 | AD-C7-W (C7 only) | K2 blob `cb3a208e1d79a7e94e2ae0001e208a2b6436bc46`, sha256 `989490ec9272a528fe0185ad0dc9f41e3aed1e414d2eb4a88346197dc60b3f3b`, 116,248 bytes; freeze commit `16e74908`; K1 catalog `19183742` / `d6c05a7c…1a83`; K3 `c5a9f3d1…af7` | **Owner adopted 2026-10-15** (record blob `f9ce488c`, sha256 `800f767d…6a54`, commit `4c29b2f0`); **identity approval 2026-10-16** (record blob `cfcbf998`, sha256 `4b74b90a…92e2`, commit `2148c5a5`); CR-B adopted as a conditional C7-only policy (B-W/A) | C7 only |

## Clauses this closure turns on

| Clause | Text (contract) | Amended? |
|---|---|---|
| 3.1, line 125 | "Targeted local tests, then the Fast Local gate on the exact commit, started by dispatch … No hosted CI." | **not superseded** by AD-1-R4 or AD-1-R5 |
| 8.3, line 831 | every full run on the exact final commit; "Missing evidence for any of them is a failed mission" | S-4 (E's inventory); S5-2 for F-R |
| 8.4, line 863 | receipt fields; every `#[ignore]` listed with its receipt | S-9 (or its approved disposition); S5-3 for F-R |
| 11, line 923 | label condition: every 8.2/8.3 run PASS, every control killed, the evidence review passes | CLARIFIED by S-18 and, for F-R, S5-4; the reviewer mapped by S-20 to the Sole Architect |
| 11, line 926 | "If any required run is INCOMPLETE, or any property fails on the unchanged core, **no label is used.** The report says what happened and the design comes back to me." | **not superseded**: AD-1-R4 lists "the label conditions of section 11, read as S-18 and S-20 state" among clauses not superseded; AD-1-R5 names lines 923 (S5-4) and 940–942, never 926 |
| 11, line 928 | using the label remains the Owner's decision | unchanged (S-20 keeps it) |
| 12, lines 940–942 | stop conditions incl. "A run of 8.2 or 8.3 is INCOMPLETE." | not superseded (AD-1-R4 and AD-1-R5 both say so) |
| 10, line 915 | "before final Candidate 11 validation the approved manifest revision and its campaign must agree with the exact final tree" | SUPPLEMENTED by S-12 (H) |

## Architect rulings relied on (provenance labelled)

| Ruling | Content relied on | Record |
|---|---|---|
| FINAL-EVIDENCE-STAGE1 (as relayed) | Candidate C frozen as the exact final source baseline; exact-source proof audit PASSED (AQ Theorem 1, L-A, L-B, Q1–Q15, AQ1–AQ5; AQ6 as a binding limitation; Lemma E, S1–S15, AS1–AS4; ASYM-1..6; E-PT, E-B; the composition; witness non-interference; N.15(a)); **all 22 stale M-control mappings SAME PROPERTY** | executor-written `final-baseline-known-runs-01/FREEZE-RECORD.txt` at `f023cc3d` |
| FINAL-EVIDENCE-STAGE2 (as relayed) | the exact-final unchanged-core configurations F-1, F-2, F-3, F-4, F-5, F-6, F-7, F-8, F-PT, F-B, F-B2, F-G, F-W, F-D accepted; F-R INCOMPLETE — STATE CEILING REACHED | executor-written restatements only (AD-1-R5 §0 basis rows; `fr-stutter-r5-01/PROPOSED-20-CONFIGURATION-ACCOUNTING.md`); **no verbatim ruling record found in Git** |
| nine mutation rulings | 33 controls accepted by ID | `01-ACCEPTANCE-PROVENANCE.md` |
| C4, C7 | accepted | conversational, conveyed in this mission's section 1; **no Git record** |
| M18 source-level disposition | "SAME INTENDED AUTHORITY PROPERTY. CHANGED OBSERVABLE FAILURE MECHANISM." | conveyed in mission text (CLOSURE-PACKET-01 and this mission); not a current-tree acceptance |
| Section 11 | **NO SECTION 11 LABEL** | this mission's sections 1 and 11 |

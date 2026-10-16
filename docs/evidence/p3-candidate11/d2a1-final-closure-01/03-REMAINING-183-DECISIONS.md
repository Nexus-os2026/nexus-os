# Stage 3 — the current-tree 183 controls: status and remaining decisions

## 1. Mechanical alignment (AD-1-R4 H.1–H.5): ALIGNED

Independent recomputation (`tools/s3_h5.py`, frozen; 1,601 of 1,601 checks; `03-CURRENT-TREE-ALIGNMENT.json`):

| Requirement | Result |
|---|---|
| H.1 historical manifest preserved | blob `65fa69dc`, sha256 `2c4adc84…2e14`, version 1-r1, 183 controls; unchanged at C and at the evidence head |
| H.2 ct-1 identity | path `docs/evidence/p3-candidate11/current-tree-183/manifest.json`, blob `5d31cb45…`, sha256 `20bcc6fe…1a44279`, version `ct-1` |
| H.3(1) derived_from | the original's path, sha256 and version |
| H.3(2) controls and definitions | 183 controls, same ids in the same order; every `id`, `category`, both meanings, `historical`, `edits` (file, anchor, replacement, count, in order), `command`, `cwd`, `expected_failure_tests` byte-equal to the original's |
| H.3(3) historical review fields | `review_status`, `lineage`, `review`, `application_review`, `source_sha256` copied unchanged under a HISTORICAL marker |
| H.3(4) current review status | **`pending` for all 183** |
| H.3(5) `source_sha256` | equals the sha256 of each named file at C, for all 184 pairs |
| H.3(6) no self-reference | no `final_tree` key; neither C's commit nor its tree hash occurs in ct-1 |
| H.5 anchors | 191 anchors, each occurring exactly its `count` at application, with ct-1's recorded unedited counts and lines equal to the recomputation |
| counts | 183 controls; 184 control-file pairs; 39 source files; 191 anchors; **162 pairs equal** their historical hash; **22 differ**, exactly AD-1-R4 H.4's two sets (15 D2A-1, 7 earlier) |

**A manifest's own strings were never treated as authority.** ct-1's `pending` statuses were not changed.

## 2. Decisions that exist (each with its source)

| Decision | Scope | Source | Nature |
|---|---|---|---|
| "All 22 stale M-control mappings: SAME PROPERTY." | the 22 changed pairs | relayed FINAL-EVIDENCE-STAGE1 ruling; executor-written `final-baseline-known-runs-01/FREEZE-RECORD.txt` (blob `4321cb34`, sha256 `a9b86f89…7fe3`) | Architect ruling as relayed; a reconciliation finding (H.4), not a current-tree acceptance |
| M18: "SAME INTENDED AUTHORITY PROPERTY. CHANGED OBSERVABLE FAILURE MECHANISM." | M18 | conveyed in mission text (CLOSURE-PACKET-01 Stage 3; this mission section 8) | source-level disposition; explicitly not an acceptance |

**No current-tree acceptance decision exists for any of the 183 controls.** No string in ct-1 confers one.

## 3. Remaining decisions (exact scope and authority; nothing is rerun to obtain them)

| # | Decision | Scope | Authority | Needed for |
|---|---|---|---|---|
| 1 | Review of ct-1 itself (structure, derivation, alignment with C) | the file | Sole Architect | AD-1-R4 H.2 ("Final Candidate 11 acceptance requires that it be reviewed and found aligned") |
| 2 | Whether the SAME PROPERTY mapping stands for **M18** given the changed observable mechanism, and whether a fresh M18 execution at the final tree is required | M18 | Sole Architect | H.4 reviewer's finding; H.6 |
| 3 | Confirmation (or revision) of the SAME PROPERTY finding for the other 21, recorded by the reviewer per H.4 — the relayed blanket ruling may already be that record; the Architect decides | 21 pairs | Sole Architect | H.4, H.6 |
| 4 | The current review status of each of the 183 at C | 183 | Sole Architect | H.3(4), H.6 |
| 5 | **Campaign scope:** whether a 183-control campaign at the exact final tree is a D2A-1 module gate or a Candidate 11 validation gate | the 183 campaign | Sole Architect (scope); Owner (any execution authorisation) | contract line 915 ("before final Candidate 11 validation … its campaign must agree with the exact final tree"); AD-1-R4 H.2, H.6. No M01–M183 campaign receipt exists at C. |

The 14 phase3 bypass-guard pairs and M87 have no change in the named file's anchors, but because those guards scan the
whole workspace (which now contains the D2A-1 ledger files) their kills are not re-established at C; M129's mutant has
a wider fault set at C (executor findings, `03-RECONCILIATION-22.md`). These matter to decision 5, not to alignment.

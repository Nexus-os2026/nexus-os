# Current-tree 183-control manifest — review packet 01

Mission P3-C11-DR01-D2A1-CTM01-CANDIDATE-BASELINE-01. Executor: Claude Code, Claude Opus 5.5.
For the Sole Architect's independent review.

**Evidence transport only.** The commit that adds this packet is not an engineering baseline. Nothing
here accepts, freezes or integrates anything. A string is not authority.

## Candidate C

**D2A-1 CANDIDATE FINAL BASELINE — REVIEW PENDING.** Not frozen. Not accepted. Not integrated.

| Item | Value |
|---|---|
| Commit | `d5964ee5326c2733177307db6f507328023d361a` |
| Tree | `51bac5fd335999cc995c0ab9f582a30244973ef0` |
| Parent (single) | `71264aa81a40b86189783facc8437e0427887c16` (AD-1-R4 review commit) |
| Manifest | `docs/evidence/p3-candidate11/current-tree-183/manifest.json`, version `ct-1` |
| Manifest blob / sha256 | `5d31cb45123403626f06c521dd2d53103676250e` / `20bcc6fe221238f81246348cbbe94636a3c9b6d8acf89901d230e091f1a44279` (687,381 bytes) |
| Owner-adoption audit record | `docs/evidence/p3-candidate11/ad1r4-adoption/OWNER-ADOPTION-2026-10-14.md` |
| Its blob / sha256 | `54c956bc1324dd3a8fd97439faa815f0865b404d` / `105e087137fed4ed7bf2baaab50b06812a1847ddbe9549809b85eecbdae379c6` |
| Changed paths | exactly those two files, both added; no other path differs from the parent |

The manifest is referenced here by path and hash, not copied. `receipts/01-C-IDENTITY.txt` is the
gate's own record of these values.

## Results

| Check | Result | Receipt |
|---|---|---|
| Baseline provenance (R4 commit, parent, blobs, sha256) | matches | `receipts/01-provenance.txt`, `receipts/development-log.txt` (1) |
| Object integrity | fsck connectivity clean; the 9 objects new in C re-hashed, 0 mismatches | `receipts/02-object-integrity.txt` |
| Historical manifest at C | sha256 `2c4adc84…2e14`, blob `65fa69dc…` equal to the parent's: unchanged | `receipts/03-historical-manifest-integrity.txt` |
| AD-1-R4 at C | sha256 `00ced6d5…b693`, blob `35d5074c…`: unchanged | `receipts/04-ad1r4-integrity.txt` |
| Changed-path confinement | two additions under the allowed directories; 5,362 other paths identical (path, mode, blob) | `receipts/05-changed-path-confinement.txt` |
| All-183 alignment (independent checker) | 201/201 PASS: lossless rebuild of the historical manifest from ct-1 is byte-identical; every file present and hashed; every anchor at its count (191 edits, 0 mismatches); exactly the 22 known pairs differ; every current review status `pending`; no self-reference | `receipts/06-all-183-check.txt` |
| Deterministic regeneration | 4 regenerations (2 from C's tree, 2 from the parent's) byte-identical to C's blob | `receipts/07-regeneration.txt` |
| Checker sensitivity | 21/21 injected defects detected by their intended checks | `receipts/08-negative-controls.txt` |
| The 22 stale pairs | facts for each; executor analysis: 21 `EXECUTOR_ANALYSIS_MATCH`, 1 `EXECUTOR_ANALYSIS_DIFFERENCE` (M18); all `ARCHITECT_REVIEW_PENDING` | `review/RECONCILIATION-22.md`, `receipts/09-reconcile-22-facts.txt`, `receipts/12-*` |
| Mission section 9 (C9, C4, C25) | no conflict: the historical 183 define no C-series control | `receipts/11-c-series-definition-check.txt` |
| Credentials | none in C's added files or in this packet's review and receipts | `receipts/13-secret-scan.txt` |
| Hosted triggers and remote state before publication | no in-tree push trigger matches `review/**`; destination absent; no ruleset or branch rule on it; no active hook | `receipts/14-workflow-triggers.txt`, `receipts/15-remote-before.txt` |
| Gate | run 2: 44 OK, 0 FAIL, PASS. Run 1 failed on a gate defect, repaired | `receipts/GATE-SUMMARY.txt`, `receipts/COMMANDS.log`, `receipts/revisions/gate-run-1/` |

### Mission section 9 in words

The historical 183-control manifest contains only M01–M183. None edits a file of the D2A-1 ledger,
and none carries a C-series anchor, identifier or name. C1, C4, C9, C10, C24 and C25 (contract 7.5)
are defined only by AD-1-R4 G.2 — C10 and C24 by the sealed `controls_defs.py` it quotes — and are
outside the 183. So no copied historical definition differs from an adopted one, and no historical
entry was changed. Informational: the sealed `controls_defs.py` holds no C4 or C9 entry, and its C25
entry is the historical compound definition that AD-1-R4 G.1 already records as historical only.

### The 22 stale pairs in brief

- **14 on `phase3_tests.rs` (D2A-1):** D2A-1's only change to the file is one test appended after its
  last line. No pre-existing line changed, and none of the 70 controls whose expected tests live in
  that file selects the new test.
- **M87 (`lib.rs`, D2A-1):** one added `mod` line, away from the anchor.
- **M84, M05, M45, M129 (RS-01 `6be32bf4`):** M84's anchor and its ±20 lines are unchanged, and so
  are M45's and M129's enclosing functions. M05's enclosing `resume` was rewritten, but the anchored
  confirmation check is still the only place the owner's answer is consulted.
- **M19, M39 (`862a11ce`):** the anchors' surroundings are unchanged.
- **M18 (`862a11ce`): EXECUTOR_ANALYSIS_DIFFERENCE.** A later re-check (`still_releasing`) repeats
  the condition the mutant disables, with the same error. The predicted kill moves to a different
  assertion of the same test, and the mutant's effect is narrower than at H: the unlisted lease is
  consumed and recorded as released, but no secret is read.

Every expected failing test's function text is byte-identical between H and C, for all 22.

## Files

| Path | What it is |
|---|---|
| `INDEX.md` | this file |
| `SHA256SUMS` | sha256 of every other file in the packet (self-excluding) |
| `review/RECONCILIATION-22.md` | the 22 reconciliation records, rendered by the gate |
| `receipts/00-FREEZE.txt` | freeze record: sha256 of every tool and input the gate ran |
| `receipts/01-…15-…` | the gate's raw receipts: each exact command, its output and exit code |
| `receipts/COMMANDS.log`, `receipts/GATE-SUMMARY.txt` | the gate's command log and verdict |
| `receipts/development-log.txt` | the account of the mission's work, with every failure and revision |
| `receipts/revisions/gate-run-1/` | the failed run 1's summary, command log, failing receipt and receipt hashes |
| `tools/gen-ct1.py` | the generator |
| `tools/check-ct1.py` | the independent all-183 checker |
| `tools/negative-controls.py` | the checker's negative controls |
| `tools/reconcile-22.py`, `tools/render-reconciliation.py`, `tools/analysis-22.json` | the 22-pair facts, renderer and executor analysis |
| `tools/c-series-check.py` | the section 9 check |
| `tools/final-gate.sh` | the gate (run: `final-gate.sh <repo> <C> <packet> <scratch>`) |
| `tools/revisions/` | the diff of each tool revised during the mission |

## Limitations

- Static analysis only. No build, test, mutation or exploration was run, so no mutation kill and no
  healthy pass is established for any control at C. Each 22-pair label is a prediction from source.
- Function identity in the 22-pair facts uses the function's name and rustfmt layout (from its `fn`
  line to the first later line at its indentation that is `}`), not a Rust parser.
- Guards that scan workspace sources also scan D2A-1's two new ledger files, in healthy and mutant
  runs alike. Their verdicts at C are not established.
- The "first-known stale" commits come from an existing receipt that sampled five commits. The
  transition commits are derived from path-limited history (`git rev-list -- <path>`).
- Receipts carry the executor's local scratch paths, which are not secrets.
- The server lists two active workflows that are not in C's tree. A push event uses the pushed
  commit's workflow files, so they cannot fire for C or for D, which adds none.

## Open for the Sole Architect

1. Review of ct-1 itself: its structure, derivation and alignment with C.
2. The SAME PROPERTY review of each of the 22 pairs. M18 (`EXECUTOR_ANALYSIS_DIFFERENCE`) and M05
   (enclosing function rewritten) most need attention.
3. Whether the C-series (7.5) controls need any current-tree manifest record beyond AD-1-R4 G.2. H.3
   scopes ct-1 to the 183.
4. Whether to select and freeze C as the final baseline. Not done here.
5. The detached attestation of H.6. Not created.

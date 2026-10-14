# Final-source proof-readiness packet 01 — index

Mission P3-C11-DR01-D2A1-FINAL-SOURCE-PROOF-READINESS-01. Executor: Claude Code, Claude Opus 5.5
(`claude-opus-5-5`), effort High. For the Sole Architect's independent review of the admission observer quotient,
the gating-caller symmetry, the F-PT/F-B extension and their composition, against Candidate C.

**Evidence only. This packet accepts no proof, freezes no baseline and attests nothing.** It establishes what
changed and what did not between the proof-audited sources and Candidate C, statically, from git objects. Only the
Sole Architect rules on the proofs. Candidate C is not modified; no source, build, test, witness, mutation,
exploration or hosted CI was run.

This file was frozen before the final static gate ran. The gate's verdict is recorded only in
`receipts/GATE-SUMMARY.txt`.

## Candidate C

| Item | Value |
|---|---|
| Commit | `d5964ee5326c2733177307db6f507328023d361a` |
| Tree | `51bac5fd335999cc995c0ab9f582a30244973ef0` |
| Parent (single) | `71264aa81a40b86189783facc8437e0427887c16` |
| Evidence D (transport only; this packet's parent) | `066a322a9182fc978e3f5dc8454157caacec6241` |
| Status | D2A-1 CANDIDATE FINAL BASELINE — MANIFEST REVIEW PASSED — PROOF AUDIT PENDING (not frozen) |

## Findings (development runs; each re-checked by the final gate)

| Area | Finding | Document | Receipt |
|---|---|---|---|
| Lineage | SOURCE-R2 → ADM → GC → EXT → W1 → W4 → B → R4 → C → D is single-parent; every tree matches | SOURCE-CLOSURE.md 1 | `receipts/10-source-closure.json` |
| Production protocol | `protocol.rs` blob `a2df2841…` at every proof commit and at C: IDENTICAL, hook included | SOURCE-CLOSURE.md 5.A | same |
| Model and explorer | 63 named items and their 152-item closure: IDENTICAL, same offsets; the model region [0, 151991) byte-identical | 5.B | same |
| Admission quotient | 10 items: IDENTICAL to ADM | 5.C | same |
| Symmetry quotient | 29 items: IDENTICAL to EXT; against GC, `SYMMETRIC` is CHANGED by the extension itself and the rest byte-identical | 5.D | same |
| F-PT/F-B extension | 29 items: IDENTICAL to EXT | 5.E | same |
| Later additions | only the C1/C25 and C4 witness layers; each a lexically isolated byte append that no earlier text names | 5, 6 | same |
| Witnesses | C1/C25: **NON_INTERFERING**; C4: **NON_INTERFERING** (14 checks each) | WITNESS-NONINTERFERENCE.md | `receipts/11-witness-check.json` |
| Build inputs | no input changed across the lineage; none to classify | BUILD-INPUT-CLOSURE.md | `receipts/12-build-inputs.json` |
| Configuration | F-3, F-PT, F-B match AD-1-R4 N.9 and N.10; 30,000,000 keys, 45 minutes; `SYMMETRIC` admits exactly its eight shapes | CONFIGURATION-IDENTITY.md | `receipts/13-config-identity.json` |
| Proof documents | every seal verifies; byte-identical transported copies at `8693e3a7`; header source claims match git | PROOF-DOCUMENT-INTEGRITY.md | `receipts/14-proof-docs.json` |
| Precondition matrix | 138 rows: 113 IDENTICAL, 6 NON_INTERFERING_CHANGE, 1 CHANGED_REVIEW_REQUIRED, 14 EXECUTION_PENDING, 1 OWNER_ALREADY_SATISFIED, 3 NOT_ESTABLISHED | PROOF-PRECONDITION-MATRIX.md, `.tsv` | `receipts/15-matrix.json` |

## For the Architect's attention

1. **`GC.S14`, CHANGED_REVIEW_REQUIRED.** S14 was written for the three-member `SYMMETRIC` of 77a146ca, which
   refused F-PT and F-B. EXT (2d83b74b) extended it to eight members; that is the subject of EXT-PROOF (E-PT, E-B,
   items 1–17). At C, `SYMMETRIC` is EXT's text byte for byte. Expected, and not a blocker; it needs the review.
2. **Six NON_INTERFERING_CHANGE rows** (`ADM.AQ1`, `ADM.AQ3`, `GC.AS2`, `GC.AS4`, `EXT.C-1`, `N.4b`). Each rests on
   the whole `model_tests.rs` blob, which at C differs from the cited blob only by appended layers: the audited AQ,
   SQ and EXT code and the two witness layers. AD-1-R4 N ("The rule") leaves to the Architect whether such an
   addition alters the audited code's meaning; WITNESS-NONINTERFERENCE.md is the executor's static case.
3. **Proofs precede their implementations.** Each proof was written against the commit before its own
   implementation (SOURCE-R2, ADM, GC). The implementation text is identified separately; AD-1-R4 N.3 makes it
   part of the review.
4. **NOT_ESTABLISHED (3):** `N.15a`, the independent review itself; `EXT.C-4`, acceptance under 8.3; `ADM.AQ6`, the
   stated limit that A-1, A-2 and 6.2 are not discharged.
5. **EXECUTION_PENDING (14):** AQ2, AQ5, AS1, C-2, C-3, C-5, N.5b, N.7, N.8, N.10–N.14. No final execution receipt
   exists at C. Historical F-3/F-PT/F-B PASS receipts are informational only (PROOF-DOCUMENT-INTEGRITY.md 2).
6. **Seal coverage.** Four origins' top-level SHA256SUMS do not list their nested `contract/SHA256SUMS`; every
   contract file is listed individually.
7. **Method limits.** The item analysis uses an executor-written Rust lexer and item extractor; it does not
   type-check or compile. Its top-level item counts were cross-checked independently (387/387 in
   `model_tests.rs`; 104/104 in `protocol.rs`, plus that file's one inner attribute).

## Files

| Path | What it is |
|---|---|
| `INDEX.md` | this file |
| `SOURCE-CLOSURE.md` | section 5: lineage, file identities, `model_tests.rs` chain, items A–E, layer isolation |
| `WITNESS-NONINTERFERENCE.md` | section 6: the two witness additions, check by check |
| `BUILD-INPUT-CLOSURE.md` | section 7: build inputs, cfgs, crate attributes, panic strategy, dependency closure |
| `PROOF-PRECONDITION-MATRIX.tsv` | section 8: the matrix, authoritative (generated by `tools/matrix.py`) |
| `PROOF-PRECONDITION-MATRIX.md` | section 8: the matrix rendered for reading (generated by `tools/render_matrix.py`) |
| `CONFIGURATION-IDENTITY.md` | section 9: F-3, F-PT, F-B, actors, caps, the SYMMETRIC allow-list |
| `PROOF-DOCUMENT-INTEGRITY.md` | sections 4, 10, 11: the sealed proofs, their seals, transport and source binding |
| `tools/` | the static tools, the matrix rows, the final gate; `tools/revisions/` holds each development revision |
| `receipts/` | the freeze record, the final gate's raw receipts and command log, and the development log |
| `SHA256SUMS` | sha256 of every other file in the packet |

## How to verify

From this directory: `sha256sum -c SHA256SUMS`. From a checkout holding C, D and `8693e3a7`, each tool re-runs
read-only, for example `python3 tools/source_closure.py <repo>`; `tools/final-gate.sh` is the exact gate that
produced `receipts/`. `tools/proof_docs.py` reads the sealed evidence root, which is outside the repository.

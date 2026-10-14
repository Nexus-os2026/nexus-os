# Proof-document integrity and source binding

Mission P3-C11-DR01-D2A1-FINAL-SOURCE-PROOF-READINESS-01, sections 4 and 10. Executor: Claude Code, Claude Opus
5.5. Read only: no document was regenerated, rewritten or copied into this packet. The raw record is
`receipts/14-proof-docs.json`, written by `tools/proof_docs.py`.

**Result: every sealed origin verifies against its own SHA256SUMS; each proof document is listed there with its
sha256; each has a byte-identical transported copy at evidence commit `8693e3a7`; and each header's source claims
match git.** No proof is accepted by this record.

The evidence root is `/home/nexus/nexus-audits/claude-architect/` (outside the repository; every origin directory is
mode `dr-xr-xr-x`). The transport is evidence commit `8693e3a742ee4d3917cbfdb809b377bcd919d9f4` (single parent W1
`42dd1bd1`), published as `refs/heads/review/p3-c11-d2a1-ad1r2-audit-packet-01`, under
`docs/evidence/p3-candidate11/ad1r2-review-packet-01/evidence/`. Because those copies are retrievable and
byte-identical, this packet references them instead of duplicating them.

## 1. The three proof documents

### ADM-PROOF — the admission observer quotient (ADM-QUOTIENT-01)

| | |
|---|---|
| Sealed path | `p3-c11-dr01-d2a1-adm-quotient-01/receipts/stage2/01-formal-argument.txt` |
| sha256 / bytes / git blob | `c75e7311fdc0eba53b366d5d8fcd11064c2de14d2a7c7bbaf8aace2816b9dd23` / 20,860 / `591b75a5ce574bbc35222b0adcad25d0f886336e` |
| Transported copy | `8693e3a7:…/evidence/p3-c11-dr01-d2a1-adm-quotient-01/receipts/stage2/01-formal-argument.txt`, byte-identical |
| Source it claims to prove | SOURCE-R2 `244c5b23` (tree `1e0c76d8` ✓); `model_tests.rs` blob `beb48fe7` ✓, sha256 `ede5ef7b…1b64` ✓; `protocol.rs` blob `a2df2841` ✓, sha256 `a5454944…a296` ✓; `unit_tests.rs` blob `55dc6640` ✓. The origin's `source/` copies of all three equal git at SOURCE-R2. Written "before any implementation". |
| Implemented at | ADM `83816e66` (single parent SOURCE-R2 ✓) |
| Numbering (each label found in the text) | facts F1–F10; Theorem 1; Corollary 1; L-A, L-B; Q1–Q15; AQ1–AQ6 |
| Preconditions and assumptions | AQ1 the model is the one at 244c5b23 (or a mutant meeting M1–M3); AQ2 `adm_kept` is None in every reachable state, checked at run time; AQ3 no hook callback in the exploring thread; AQ4 a step is deterministic in (key, prefix); AQ5 every verdict is for a COMPLETE run; AQ6 it is equivalence of the explored model only, and discharges neither A-1, A-2 nor contract 6.2. Q14 excludes A2; Q15 keeps the original explorer for every 7.5 control. Its own qualification (Q12): the first-reported property may differ. |

### GC-PROOF — the gating-caller symmetry (GC-SYMMETRY-01)

| | |
|---|---|
| Sealed path | `p3-c11-dr01-d2a1-gc-symmetry-01/receipts/stage2/00-FORMAL-SYMMETRY-ANALYSIS.txt` |
| sha256 / bytes / git blob | `2e6feb4fd4baf6853665fb812441c6daf169858dc531dcb53ef0126303b0bc6a` / 20,395 / `d9e5260723cb7e28438fbb7a886dca32ac30dc77` |
| Transported copy | `8693e3a7:…/evidence/p3-c11-dr01-d2a1-gc-symmetry-01/receipts/stage2/00-FORMAL-SYMMETRY-ANALYSIS.txt`, byte-identical |
| Source it claims to prove | ADM `83816e66916c3a217ab7919d7e6a21893e7d579a` (tree `74a6a0e5add063dac4d24939b3a62c619847dd3a` ✓); "P 1-2083, M 1-6034" read in full: `protocol.rs` has 2,083 lines ✓ and `model_tests.rs` at ADM 6,034 ✓. Written "before any code change". |
| Implemented at | GC `77a146ca` (single parent ADM ✓) |
| Numbering | identity uses I1–I13; Lemma E (clauses a–g); S1–S15; ASYM-1..6; AS1–AS4 |
| Preconditions and assumptions | Lemma E holds for configurations with acks = 0, ack_attempts = 0, malformed = false, gating_conflict = false, gating = 2 and no A2 bits. AS1 KEY holds on every expanded key (checked at run time, at the key and its mirror); AS2 no hook callback in the explorer thread; AS3 a step is deterministic; AS4 the inspection is complete for the source at 83816e66 only, and a later model change voids it. S8: the first-reported property may differ (ASYM-4). Its conclusion covered F-3 and X-2; at that commit F-PT, F-B, F-G and the others were refused (S14). |

### EXT-PROOF — the F-PT/F-B extension (CLOSURE-PREP-01)

| | |
|---|---|
| Sealed path | `p3-c11-dr01-d2a1-closure-prep-01/receipts/stage2/00-FPT-FB-SYMMETRY-PROOF.txt` |
| sha256 / bytes / git blob | `c2618c19a972826e1b310c289a0075c9954aa6ae6722ac38cf11976846fbb98f` / 17,680 / `8f8bcd7cdddb9b4177a7151915ab4538b386d746` |
| Transported copy | `8693e3a7:…/evidence/p3-c11-dr01-d2a1-closure-prep-01/receipts/stage2/00-FPT-FB-SYMMETRY-PROOF.txt`, byte-identical |
| Source it claims to prove | GC `77a146ca606f99329068410b6037721341f00c70` (tree `21818166ee10a8075ca364bd09f31c7aa64555dd` ✓); `protocol.rs` blob `a2df2841` ✓; `model_tests.rs` blob `a80dd05c` ✓, 7,758 lines ✓. It inherits GC-PROOF and ADM-PROOF, read in full. Written "before any code change". |
| Implemented at | EXT `2d83b74b` (single parent GC ✓): the `SYMMETRIC` change and the appended fixtures and tests |
| Numbering | E-PT, E-B, E-R; the F-PT matrix items 1–17 and the F-B matrix items 1–17; conditions C-1..C-6 |
| Preconditions and assumptions | Lemma E's preconditions hold for F-PT and F-B (both: gating 2, no gating conflict, acks 0, ack_attempts 0, malformed false, faults 2, admissions 2, cap 30,000,000, time 45 min). C-1 it inherits AS1–AS4 and AQ1–AQ6, and the inspection is valid for 77a146ca only; C-2 the runtime guard stays the backstop; C-3 no full F-PT or F-B run was made, and their sizes and verdicts were not established; C-4 allow-list membership is a precondition, not acceptance under 8.3; C-5 the small differentials are observations, not a substitute; C-6 ASYM-1..6 unchanged, none reachable in F-PT or F-B. Item 17 of each matrix: the original explorer stays required for control attribution. |

### Supporting documents

| Key | Sealed path | sha256 | Transported copy |
|---|---|---|---|
| GC-GREP, the exhaustive identity grep behind I1–I13 | `p3-c11-dr01-d2a1-gc-symmetry-01/receipts/stage2/01-identity-grep.txt` | `bf3c01c06413801eb78684367437ff9db03dee587659b90d11d4dc84719041d9` | byte-identical |
| EXT-GREP, the grep behind E-PT/E-B | `p3-c11-dr01-d2a1-closure-prep-01/receipts/stage2/01-grep.txt` | `4fb9a4e3d66e1aa85e168e7db9345c68fb8234930d6333655f37ea9a05fb63f6` | byte-identical |
| ADM-QUOTIENT-01 REPORT | `p3-c11-dr01-d2a1-adm-quotient-01/REPORT.txt` | `4e5bc7c000c9364e812714ef39bcaea217794b375efb9fe86385db26587d0ee6` | byte-identical |
| GC-SYMMETRY-01 REPORT | `p3-c11-dr01-d2a1-gc-symmetry-01/REPORT.txt` | `ff34ab0340ee5f2166c870093929c443dabddf85160f929603709a0ad51384a8` | byte-identical |
| CLOSURE-PREP-01 REPORT | `p3-c11-dr01-d2a1-closure-prep-01/REPORT.txt` | `5c23bc0e81765811f5c8a0ce4c03daa005571e3815b23afd65ea72d9b5282157` | byte-identical |
| Gate 36 receipt: the 24 negatives and fail-closed tests of AD-1-R4 D (N.8) | `p3-c11-dr01-d2a1-ad1r1-mut-readiness-01/receipts/stage5/36-negatives-and-fail-closed.txt` | `2c517f358002e4e99449b554ec8242952640f177269ae89e9430984a4257f1e6` | byte-identical |

The 24 names on gate 36's command line equal the 24 used by matrix row N.8; all 51 of N.8's tests exist at C
byte-identical to their definitions at EXT.

## 2. Historical execution receipts: informational only (mission section 11)

These record what the proofs were used for experimentally. **None is final evidence, and no final execution
receipt exists at C.** The exact-final-baseline runs remain pending.

| Origin | REPORT sha256 | What it recorded (EXECUTOR-REPORTED, historical) |
|---|---|---|
| GC-SYM-F3-01 | `56ed3da7ed2519102281cbd4d772b4fedf3f1a7fc9ffdc76f689219ef69aaf4c` | F-3 composed quotient at 77a146ca: 20,431,851 keys, complete, PASS |
| PT-FB-EXPERIMENT-01 | `2df8420ec5d69207b76bc7419b3522561ae6b3b658cd111838847d20e1b02f78` | F-PT 29,751,473 and F-B 3,775,373 keys at 2d83b74b, complete, PASS |
| F3-QUOTIENT-01 | `798f192cf1f47164f6cea49a3292640721a1a02f0ee9e9ca012ab180cf650008` | F-3 under the admission quotient alone: INCOMPLETE |

AD-1-R4 I lists the composed PASS results as informational and not 8.3 evidence.

## 3. Origin seals

Each origin's `SHA256SUMS` was re-verified line by line; each is also transported byte-identical at `8693e3a7`.

| Origin | SHA256SUMS sha256 | Lines | Verified | Files present but not listed |
|---|---|---|---|---|
| ADM-QUOTIENT-01 | `459101ef4b3ead6e7952eaa4289a0bb5aef6ac81c84d7e3d1fb833f7555e02ce` | 42 | all | `contract/SHA256SUMS` |
| GC-SYMMETRY-01 | `34bf59b34ffd60c36e1c435ede4c7bff5f54374a9e4b0b6bcd25e01f97a85826` | 37 | all | none |
| CLOSURE-PREP-01 | `137fba3a9899d5b1985ac7579961ab2a9fb1b9a0f7fbe672c84066d7c3bfbc0f` | 42 | all | none |
| GC-SYM-F3-01 | `dd15f3e4a64c4973ab06bea605771bde64f465906e9be88dae95cb5976a8b0e1` | 44 | all | `contract/SHA256SUMS` |
| PT-FB-EXPERIMENT-01 | `a4c5435d91bf2baf75e4aa90e02e7a314aac97172d7dbf50e5c7e32064725d22` | 56 | all | none |
| F3-QUOTIENT-01 | `5e5037b41a194ff355d3c6982f21a67a44b5f826e6baea2b12ad4d1c9ca89ae4` | 45 | all | `contract/SHA256SUMS` |
| AD1R1-MUT-READINESS-01 | `106243539f31aae6cc0f6026b40c280af954f51af4a9907a2f7f0d063b7bbff3` | 73 | all | `contract/SHA256SUMS` |

Limitation, recorded: four origins' top-level lists do not list their nested `contract/SHA256SUMS`. Each contract
file that list names is listed individually, with its sha256, so its contents are still covered. The contract
`P3-C11-D2A1-CONTRACT-02-R1.md` is sha256 `cac6535115fb333e68e6c3c045e444342c950ab7a735b7b7deea21cb2cb53b3c` in
every origin.

## 4. Governing documents at C

| Document | Path at C | Blob | sha256 |
|---|---|---|---|
| AD-1-R4 (adopted; its own header still reads DRAFT, by design) | `docs/evidence/p3-candidate11/ad1r4-consolidation-01/ad1r4/P3-C11-D2A1-CONTRACT-02-R1-ADDENDUM-AD1-R4-DRAFT.md` | `35d5074c2daa2ce9602fee3cd23dc0ccd6aef5b1` | `00ced6d535c78f34f9e92233b742126a7b19192bf3ac8348056be2665192b693` |
| Owner-adoption audit record | `docs/evidence/p3-candidate11/ad1r4-adoption/OWNER-ADOPTION-2026-10-14.md` | `54c956bc1324dd3a8fd97439faa815f0865b404d` | `105e087137fed4ed7bf2baaab50b06812a1847ddbe9549809b85eecbdae379c6` |
| ct-1 current-tree manifest (version `ct-1`; Architect ruling: accepted) | `docs/evidence/p3-candidate11/current-tree-183/manifest.json` | `5d31cb45123403626f06c521dd2d53103676250e` | `20bcc6fe221238f81246348cbbe94636a3c9b6d8acf89901d230e091f1a44279` |

All three match the identities the mission states. ct-1 is referenced, not edited.

## 5. A reading note for the review

Each proof was written against the commit before its own implementation: ADM-PROOF against SOURCE-R2 (the model,
before the AQ code existed), GC-PROOF against ADM, EXT-PROOF against GC. So the proofs' line numbers describe the
model and protocol, and the implementations (`observe_admission`, `expand`, `explore_experimental`; the `mirror_*`
functions, `hold`, `explore_symmetric_kept`; `SYMMETRIC`) are identified here by their implementation commits.
EXT-PROOF did read the AQ and SQ implementations at 77a146ca in full. AD-1-R4 N.3 names the quotient code's
reviewed text as 2d83b74b; at C every item of it is byte-identical to 2d83b74b (SOURCE-CLOSURE.md).

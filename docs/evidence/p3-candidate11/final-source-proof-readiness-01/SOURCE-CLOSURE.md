# Exact source closure: proof-audited sources to Candidate C

Mission P3-C11-DR01-D2A1-FINAL-SOURCE-PROOF-READINESS-01, section 5. Executor: Claude Code, Claude Opus 5.5.
Read-only static analysis of git objects. No build, no test, no exploration. The raw record is
`receipts/10-source-closure.json`, written by `tools/source_closure.py` (items by `tools/rust_items.py`).

**This is a record of source identity. It accepts no proof.**

## 1. Candidate C and the lineage it is compared with

| Role | Commit | Tree | Single parent |
|---|---|---|---|
| SOURCE-R2 (model; the admission proof was written against it) | `244c5b23244d1abd8b2b595c9e7654a25029a9f1` | `1e0c76d8ba6aa437d5d02367c81c4de9d46ffc78` | `49e29cd3…` |
| ADM: admission quotient (AQ) implementation; the symmetry proof was written against it | `83816e66916c3a217ab7919d7e6a21893e7d579a` | `74a6a0e5add063dac4d24939b3a62c619847dd3a` | SOURCE-R2 |
| GC: gating-caller symmetry (SQ) implementation; the extension proof was written against it | `77a146ca606f99329068410b6037721341f00c70` | `21818166ee10a8075ca364bd09f31c7aa64555dd` | ADM |
| EXT: F-PT/F-B extension implementation | `2d83b74b3e9f0bfb6ca4763df56d57f3c044bcda` | `6f6a54f5c85d6a3102cd034c67e5791cb5a0286a` | GC |
| W1: C1/C25 witness source | `42dd1bd1cf21ffdea5968b256d2983f6575fab5b` | `7ac6bb21430d15b17e3723b217b20a49dccf8282` | EXT |
| W4: C4 witness source (commit A) | `04e42a7ddf0d0a2af17b6506c51997006e7a08dc` | `fca5c2a28e1c6cfef3cd92fbb81484a56769a9b3` | W1 |
| B: C4 evidence (documentation) | `a464a36621523f539d2554a6b176d678c7f885ea` | `e1f3e48f5b5fc7c792d99fb21e274f5c5e3af2b3` | W4 |
| R4: AD-1-R4 review commit (documentation) | `71264aa81a40b86189783facc8437e0427887c16` | `f4499b070de59342a1ca89213199b4efa7fbf814` | B |
| **C: the source under review** | **`d5964ee5326c2733177307db6f507328023d361a`** | **`51bac5fd335999cc995c0ab9f582a30244973ef0`** | **R4** |
| D: evidence transport (parent of this packet; not an engineering source) | `066a322a9182fc978e3f5dc8454157caacec6241` | `da31dfa7c9a0c4f358bfde4f15815f6c8d46f806` | C |

The chain is single-parent from SOURCE-R2 to D; every parent link is checked. Every tree above is read from
git; those of SOURCE-R2, ADM, GC, EXT, W1, W4, C and D are also checked against their known values.

**Outside `docs/evidence/`, the only path that differs between any lineage commit and C is
`crates/nexus-governed-control/src/ledger/protocol/model_tests.rs`** (SOURCE-R2, ADM, GC, EXT, W1). From W4 on,
nothing outside `docs/evidence/` differs from C. B, R4, C and D add documentation only.

## 2. The authenticated files at C

| File | Git blob | sha256 | Bytes | Across SOURCE-R2 … C |
|---|---|---|---|---|
| `crates/nexus-governed-control/src/ledger/protocol.rs` | `a2df2841b99cad5d452008a6d4150e9369d07e66` | `a5454944368df70fa4ce1c7cc4191d283e0a54b371137338fb999e59960aa296` | 64,546 | one blob |
| `crates/nexus-governed-control/src/ledger/protocol/model_tests.rs` | `28085ce232827301104b5d757af042ec933976f8` | `2e93834f795f50842d5b124612895c4a150946130d2c789dac816ac36f6b6972` | 310,612 | six blobs (section 3) |
| `crates/nexus-governed-control/src/ledger/protocol/unit_tests.rs` | `55dc66409eaccdaa93e8f0a56f2ccb14be81e114` | `841220b1095f339568e2764c41be0792a83c7fb097df6cdea8b50e46dc261159` | 104,202 | one blob |
| `crates/nexus-governed-control/src/ledger.rs` | `d872b90816fdd18cca8e5d1a0005bb49434aaf2a` | `d37983f6b040f67fd012f1bc4d040523a254d5d0557a39edfa40dd9dffe99215` | 364 | one blob |
| `crates/nexus-governed-control/src/lib.rs` | `b806ed577f8d3472b7187d161246e08d2f1ccd3d` | `addad1993f57ad370da7e38719f3e58d26b5a4eb1fbe06b24df51525b135727d` | 4,582 | one blob |
| `Cargo.lock` | `1af53a60aaf6fde5731e63fcd5b6cd11d86f669a` | `6ed84a9aeba4a542b1820c53d4ba0f4899cdfa819380aaa8649c3b5d31add979` | 289,456 | one blob |
| `Cargo.toml` (workspace) | `2c558a4477d1545daaa489f0a0e502b4284ad479` | `7f09c3dbe16a37bba215eeaa5f028614f3f6eafdf3419a5b79472f6b7cebf795` | 2,535 | one blob |
| `crates/nexus-governed-control/Cargo.toml` | `2543c745d661203326367962714b94c811e07980` | `30b2bb495434e85fc5b1e1476949711d55e8bb55cb9e0e6783bbc71ce0a3564f` | 837 | one blob |
| `rust-toolchain.toml` | `726005b29561a04536edea412bf551a9884d6639` | `fc7f9811196c9d9db1864dd140fa4b7632cda9b7761ed8736256b463c835d298` | 94 | one blob |
| `.cargo/config.toml` | `05a42b50174e235b183b220afeb58922c96d17a1` | `379a150667985722540bd30f737cfb363dc5bc2dc8a5edae3db3725e3cd0b234` | 55 | one blob |

Every blob was re-hashed from its bytes and matches its git id. "One blob" means the same blob at all ten lineage
commits. Build inputs are analysed in BUILD-INPUT-CLOSURE.md.

## 3. `model_tests.rs` across the lineage

| Commit | Blob | sha256 | Bytes | Relation to the next |
|---|---|---|---|---|
| SOURCE-R2 | `beb48fe7059c14f5b4cbc0ac25148d53cbe7c05e` | `ede5ef7b…1b64` | 151,991 | ADM appends bytes [151991, 194288) |
| ADM | `fad0ffb0ed243bbd445191bc7963b075bde13b5a` | `6ce62e24…92bb` | 194,288 | GC appends bytes [194288, 257218) |
| GC | `a80dd05cbc6a90ddd97bfa4dba754ec5569eec21` | `be562d08…97f7` | 257,218 | EXT changes two places and appends (below) |
| EXT | `4895993e1a9ca68762d873e026d42a6e08e49cc9` | `1ca836c3…6f1c` | 275,579 | W1 appends bytes [275579, 286411) |
| W1 | `5ad26ddc727d8be30f4fb8d7478bc69e1c974933` | `be9e3547…b2ba` | 286,411 | W4 appends bytes [286411, 310612) |
| W4 = C | `28085ce232827301104b5d757af042ec933976f8` | `2e93834f…6972` | 310,612 | identical |

Every append is a pure byte append, and **lexically isolated**: lexing the longer file yields exactly the earlier
file's tokens over the earlier bytes, so no appended text can close a comment, string or item of the text before it.

The one in-place change in the whole lineage is GC → EXT, the F-PT/F-B extension itself:

| Hunk | GC lines / bytes | EXT lines / bytes | Content |
|---|---|---|---|
| 1 | 6074 / [196230, 196303) | 6074–6076 / [196230, 196458) | `SYMMETRIC`'s doc comment names SYM-PT, SYM-B, SYM-BI, F-PT and F-B |
| 2 | 6077 / [196409, 196456) | 6079 / [196564, 196643) | `const SYMMETRIC: [Config; 3] = [X2, SYM1, F3];` → `const SYMMETRIC: [Config; 8] = [X2, SYM1, F3, SYM_PT, SYM_B, SYM_BI, FPT, FB];` |
| 3 | append at 257218 | [257405, 275579) | SYM_PT, SYM_B, SYM_BI, their tests, `symmetric_full_exploration`, `gc_symmetry_fpt`, `gc_symmetry_fb` |

So these files are byte prefixes of C: SOURCE-R2's, ADM's, EXT's, W1's and W4's. GC's is not, by exactly the two
hunks above.

## 4. Every item of every audited version, mapped to C

`tools/rust_items.py` lexes Rust (comments, strings, raw strings, chars and lifetimes) and extracts every item, and
the items inside `impl`, `trait` and `mod` bodies. Its top-level count was cross-checked against a column-0 item
pattern (387/387 at C; 104/104 in `protocol.rs`, whose one inner attribute, line 15, the extractor records as a
separate pseudo-item, 105 in all). Each item is matched to C by its path and ordinal (for example
`impl Oracle/fn new`, #1) and compared by bytes and by byte offset.

| Audited version | Items | At C |
|---|---|---|
| SOURCE-R2 | 287 | 287 IDENTICAL (same bytes, same offsets) |
| ADM | 345 | 345 IDENTICAL |
| GC | 403 | 346 IDENTICAL; 56 IDENTICAL bytes at an offset moved by the EXT hunks (+187 bytes); 1 CHANGED: `const SYMMETRIC` |
| EXT | 431 | 431 IDENTICAL |
| W1 | 441 | 441 IDENTICAL |
| W4 | 471 | 471 IDENTICAL |

## 5. The relevant items, by mission section 5

Statuses: IDENTICAL, APPENDED-AFTER-NO-REFERENCE, CHANGED, NOT ESTABLISHED. The full rows, with every byte range
and sha256, are `sections` in `receipts/10-source-closure.json`.

### A. Production protocol

`protocol.rs` at C is byte-identical (blob `a2df2841b99cad5d452008a6d4150e9369d07e66`) to SOURCE-R2, ADM, GC, EXT,
W1 and W4: **IDENTICAL**. Its one test-only item, `mod hook` (lines 2017–2083, bytes [62456, 64545), sha256
`1f813d27ac256c0e0f6315c7a343b13b0faf63b44a4d665cc1a37e0a32bbb3e6`), is therefore unchanged. Its counts live in
`thread_local!` statics.

### B. Original model and explorer semantics (baseline SOURCE-R2)

63 named items, all **IDENTICAL** at the same offsets:
`TICKET_LIMIT`, `REFUSAL_LIMIT`, `struct Config`, `DEFAULT_CAP`, `DEFAULT_TIME`, `FULL_CAP`, `FULL_TIME`, `BASE`,
`fn default_run`, the 20 configuration constants `X1`–`X5`, `F1`–`F8`, `FPT`, `FB`, `FB2`, `FG`, `FW`, `FD`,
`FR`; `struct Oracle`, `impl Oracle`; `struct Io` and its six impl blocks (`impl<'a> Io<'a>`, `Txn`, `Reads`,
`AckTxn`, `CellTxn`, `BarrierRun`); `fn initial`; `struct Enc`, `impl Enc`, `fn encode`; `struct Dec`,
`impl Dec<'_>`, `fn decode`; `fn actors`; every `step_*` (`step_fw`, `step_se`, `step_wk`, `step_gc`, `step_ack`,
`step_adm`, `step_env`, `step_replay`) and `fn step`; `fn admission_properties`, `fn state_properties`,
`fn transition_properties`, `fn terminal`; `PROPERTIES`, `BRANCHES`; `fn successors`; `fn explore`.

Their name-dependency closure, computed in SOURCE-R2 itself (an item is reached when a reached item mentions its
name, in any namespace: an over-approximation), has 152 items: **all IDENTICAL** at the same offsets, all inside
the model region, bytes [0, 151991). The whole model region is itself byte-identical at C.

### C. Admission observer quotient (baseline ADM)

The closure of `expand`, `observe_admission` and `explore_experimental` within the AQ layer: 10 items, all
**IDENTICAL** at the same offsets.

| Item | Bytes at ADM and at C | sha256 (16) |
|---|---|---|
| `type AdmStep` | [153049, 153106) | `8fc0aa5395ff4adb` |
| `type Violations` | [153108, 153154) | `8f3a378d51f050f0` |
| `enum Mode` | [153156, 153386) | `8e6fb26213793af8` |
| `enum Next` | [153425, 153651) | `5582ecdba22872e7` |
| `fn step_with` | [153706, 154052) | `de6df9c301989652` |
| `fn observe_admission` (its guards: enabled exactly when admissions are configured; spends exactly one; no A2 bits; re-encodes to its source key) | [154522, 155754) | `512aae7ac96697b6` |
| `fn expand` (its guard: every state holds its admissions and no A2 bits) | [155935, 157757) | `1a52c4cf59a33fc7` |
| `fn trace_with` | [157798, 158942) | `0fc52a88fe91631d` |
| `struct Tally` | [159075, 159339) | `e2ac0671d8b1ce49` |
| `fn explore_experimental` | [159526, 170254) | `aa15bc365ba8c6fe` |

### D. Gating-caller symmetry quotient (baseline GC, and EXT)

The closure of `mirror`, `shape`, `assert_symmetric`, `explore_symmetric`, `explore_symmetric_kept` and `SYMMETRIC`
within the SQ layer: 29 items. Against GC: 28 byte-identical (27 of them at an offset moved by +187 bytes by the
EXT hunks) and 1 **CHANGED**, `const SYMMETRIC`. **Against EXT, all 29 are IDENTICAL**, at the same offsets.

| Item | Bytes at GC → at C | sha256 (16) | vs GC | vs EXT |
|---|---|---|---|---|
| `const SYM1` | [195968, 196150) → same | `1eda6013d83df4b6` | IDENTICAL | IDENTICAL |
| `const SYMMETRIC` | [196409, 196455) → [196564, 196642) | `e59fa0185502066a` → `dbf8835ef8cd0114` | **CHANGED** (hunks 1–2) | IDENTICAL |
| `fn shape` | [196495, 196672) → [196682, 196859) | `e23f7f25d8f7059e` | IDENTICAL | IDENTICAL |
| `fn assert_symmetric` | [196814, 197958) → [197001, 198145) | `e32321eea0ca2173` | IDENTICAL | IDENTICAL |
| `fn other_caller` | [197989, 198095) → [198176, 198282) | `19f242eb1aaf6485` | IDENTICAL | IDENTICAL |
| `fn mirror_x32` | [198464, 199032) → [198651, 199219) | `02095a766d1e834b` | IDENTICAL | IDENTICAL |
| `fn mirror_key` | [199034, 199177) → [199221, 199364) | `23a8fd1d1c1c373d` | IDENTICAL | IDENTICAL |
| `fn mirror_part` | [199179, 199310) → [199366, 199497) | `c18420ebcaae305d` | IDENTICAL | IDENTICAL |
| `fn mirror_member` | [199312, 199469) → [199499, 199656) | `16c9b5e8a30d9e20` | IDENTICAL | IDENTICAL |
| `fn mirror_cause` | [199549, 200535) → [199736, 200722) | `f06050ccab27742e` | IDENTICAL | IDENTICAL |
| `fn mirror_occupant` | [200537, 201299) → [200724, 201486) | `e02af0b74adfa88d` | IDENTICAL | IDENTICAL |
| `fn mirror_row` | [201371, 202065) → [201558, 202252) | `2289d7d97f1f8820` | IDENTICAL | IDENTICAL |
| `fn mirror_bits` | [202114, 202288) → [202301, 202475) | `9f58cae53d0e6b11` | IDENTICAL | IDENTICAL |
| `fn mirror_world` | [202665, 204605) → [202852, 204792) | `1aed6bbe58192d61` | IDENTICAL | IDENTICAL |
| `fn mirror` | [204662, 204801) → [204849, 204988) | `5946d0b9d9bc6714` | IDENTICAL | IDENTICAL |
| `fn canonical` | [204866, 204998) → [205053, 205185) | `bfd85786537f8239` | IDENTICAL | IDENTICAL |
| `fn mirror_edge` | [205070, 205421) → [205257, 205608) | `b7533b618a1872b3` | IDENTICAL | IDENTICAL |
| `struct Image` | [205613, 205789) → [205800, 205976) | `54d332c2859bfb04` | IDENTICAL | IDENTICAL |
| `fn names` | [205791, 205985) → [205978, 206172) | `e5b1ee2bddf41b47` | IDENTICAL | IDENTICAL |
| `fn by_actor` | [206049, 206302) → [206236, 206489) | `756ed5612d309c9c` | IDENTICAL | IDENTICAL |
| `fn hold` | [206661, 208068) → [206848, 208255) | `4889e32c331fc27c` | IDENTICAL | IDENTICAL |
| `fn replay` | [208207, 208810) → [208394, 208997) | `027a4e3c6e01c5c0` | IDENTICAL | IDENTICAL |
| `fn unfold` | [209143, 210008) → [209330, 210195) | `92d64967cb12a23d` | IDENTICAL | IDENTICAL |
| `fn witness` | [210068, 210336) → [210255, 210523) | `8fe20c0a051588c2` | IDENTICAL | IDENTICAL |
| `fn kahn` | [210608, 211258) → [210795, 211445) | `3732504593ce889a` | IDENTICAL | IDENTICAL |
| `struct Mirrors` | [211564, 211833) → [211751, 212020) | `8aefb443f4886e8c` | IDENTICAL | IDENTICAL |
| `type Kept` | [211910, 211954) → [212097, 212141) | `5e10373ceabc2c90` | IDENTICAL | IDENTICAL |
| `fn explore_symmetric` | [211956, 212168) → [212143, 212355) | `0d9be6f6bb75f0af` | IDENTICAL | IDENTICAL |
| `fn explore_symmetric_kept` | [212553, 225240) → [212740, 225427) | `24418b978bc61576` | IDENTICAL | IDENTICAL |

The change to `SYMMETRIC` is expected: it is the extension the F-PT/F-B proof (CLOSURE-PREP-01) argues for, and
AD-1-R4 N.3 names "reviewed text at 2d83b74b". At C it is EXT's text, byte for byte. It is not an unexpected
difference, and it is listed for review (matrix row `GC.S14`, CHANGED_REVIEW_REQUIRED, because S14 as written at
77a146ca describes the three-member list).

### E. The F-PT/F-B extension (baseline EXT)

Every item EXT adds or changes relative to GC: 29 items, all **IDENTICAL** at C at the same offsets: `SYMMETRIC`
(changed), `SYM_PT`, `SYM_B`, `SYM_BI`, `gc_symmetry_allow_list_is_exact`, the three symmetry differentials, the
three configuration exercises (`original_graph`, `exchange_exercised`, `BatchSteps`, `batch_steps` with them), the
three negatives, the eight refusals, `symmetric_full_exploration`, `gc_symmetry_fpt` and `gc_symmetry_fb`.

### Witness items (W1 and W4 layers)

10 items (C1/C25) and 30 items (C4): **APPENDED-AFTER-NO-REFERENCE**. Each lies after all the audited text, and
no name any of them defines occurs anywhere before its layer. Whether they interfere in any other way is the
subject of WITNESS-NONINTERFERENCE.md.

## 6. Layer isolation

For each layer appended to `model_tests.rs`, in C's byte coordinates (check 8 of `source_closure.py`):

| Layer | Bytes at C | Items | Names it defines that occur before it | impl/trait/use/static/macro/mod items | Inner or cfg attributes |
|---|---|---|---|---|---|
| AQ | [151991, 194288) | 58 | none | none | none |
| SQ, with the EXT in-place hunks | [194288, 257405) | 58 | none | none | none |
| EXT append | [257405, 275579) | 28 | `SYM_PT`, `SYM_B`, `SYM_BI`, only inside `const SYMMETRIC` (by design) | none | none |
| C1/C25 witnesses | [275579, 286411) | 10 | none | none | none |
| C4 witness | [286411, 310612) | 30 | none | none | none |

With `use super::*;` at the top of the module, a new module-level name could shadow a glob-imported one for the
whole module, earlier text included. That can only change earlier code that spells the name. None does. The SQ
layer's `fn names` shares a name with `Core::names` in `protocol.rs`, but that is a method, not a module item,
and is not glob-imported.

## 7. What this establishes, and what it does not

Established, statically, from git objects: the production protocol, the model and explorer, the admission
quotient, the symmetry quotient and the F-PT/F-B extension are byte-identical at C to the source each proof or
implementation commit holds, except `SYMMETRIC` relative to GC, which equals EXT's text. Every later addition is a
lexically isolated byte append that the earlier text never names.

Not established here: that any proof is sound; that the quotient implementations realise the proofs (the proofs
were written before their implementation commits; AD-1-R4 N.3 makes the implementation text at 2d83b74b part of
the review); anything that needs a build or a run (BUILD-INPUT-CLOSURE.md and the matrix's EXECUTION_PENDING rows).

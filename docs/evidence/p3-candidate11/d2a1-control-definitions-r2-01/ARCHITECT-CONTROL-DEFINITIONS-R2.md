# Architect control definitions R2 — the 21 controls, with A6's receipt-metadata repair

**ARCHITECT-SPECIFIED — EXECUTOR-ENCODED — INDEPENDENT ARCHITECT REVIEW PENDING**

Mission P3-C11-DR01-D2A1-CONTROL-DEFINITIONS-R2-01. Executor: Claude Code, Claude Opus 5.5 (`claude-opus-5-5`), effort High. Static definition work only: no Cargo, no rustc, no build, no test, no mutant executed, no explorer run. Candidate C was read and never written.

This record carries forward the reviewed R1 record (`ARCHITECT-CONTROL-DEFINITIONS-R1.md`, git blob `b947621bb0d56e1e14780853e886c7e832d7f6a3`, sha256 `beb9f5e3129bbd559dfa375251bddeac17f78d3f1b79082a42767944d7d48bf0`, R1 evidence commit `f81159046bf0b62666851bc1185992318302d31f`): the 21 definitions are the Sole Architect's (mission R1 section 6), encoded by the executor as exact edits of Candidate C `d5964ee5326c2733177307db6f507328023d361a`. In the R2 candidate catalog `d2a1-controls-v1-r2-candidate` (`catalog/catalog.json`, sha256 `d6c05a7cdc71b3ca1812b5199b8529cbe6db0363ca11bacc26d1f21c2cd11a83`) twenty of them are R1's entries and patches unchanged, byte for byte; A6 alone changes, by the one receipt-metadata hunk the Architect authorised (mission R2 section 4). None is accepted: each definition's Architect review is pending, and no claim is made that any control will be killed.

## The R2 correction: A6

*ARCHITECT-SPECIFIED — EXECUTOR-ENCODED — INDEPENDENT ARCHITECT REVIEW PENDING*

**The ruling and the repair (mission R2 sections 1 and 4, as received):**

```
============================================================
1. ARCHITECT RULING
============================================================

The independent review of:

P3-C11-DR01-D2A1-CONTROL-DEFINITIONS-R1-01

found ONE blocking defect.

The semantic encodings of all 21 newly defined controls pass
source review.

The 34 documentary patches other than A6 pass review unchanged.

A6's semantic encoding passes review:

- recovery_required is added to Table;
- initialized false;
- encoded;
- decoded;
- set when H actually increases;
- cleared when A actually increases;
- used instead of H <= A by admission;
- numeric H/A and P12/P15 remain unchanged.

The defect is only this:

A6 adds recovery_required to the state key but does not update
KEY_FIELDS, which every exploration report prints.

Therefore:

- R1 catalog is NOT accepted or frozen.
- No mutation execution is authorized.
- Only the A6 receipt-metadata repair below is authorized.

============================================================
4. EXACT A6 REPAIR
============================================================

Start from the exact R1 A6 patch.

Preserve byte-for-byte all of its existing semantic changes:

- Table.recovery_required;
- initialization;
- encoding;
- decoding;
- H event update;
- A event update;
- WkPos::RaiseH update;
- AckPos::Applying update;
- admission predicate.

Add exactly one additional hunk in:

crates/nexus-governed-control/src/ledger/protocol/model_tests.rs

Item:

KEY_FIELDS

Replace only:

the test's table (classes, debt counter, H, A);

with:

the test's table (classes, debt counter, H, A, recovery_required);

No other text in KEY_FIELDS may change.

Do not:

- rename the field;
- alter the key layout;
- move its encoded position;
- change a property;
- change a transition;
- change a trace;
- change a branch label;
- alter another control.

The KEY_FIELDS hunk is receipt metadata.
It is not a second semantic mutation.
```

**What R2 records for A6.**

- **The semantic mutation is unchanged from R1.** A6's eight R1 edits are kept byte for byte (catalog `edits[0..7]` equal R1's), and every one of the eight hunks of the R1 patch appears in the R2 patch byte for byte — `@@` line, line numbers, function context and body — in the same order: the field `Table.recovery_required`, its initialisation (false), its encoding, its decoding, the H-event update (WkPos::Seen), the A-event update (WkPos::Seen), the WkPos::RaiseH update, the AckPos::Applying update and the admission predicate.
- **recovery_required is part of the state identity.** It is encoded right after A (`e.b(w.table.recovery_required)`) and decoded at the matching place, so, on a static reading of encode and decode, two states that differ only in the flag have different state keys.
- **KEY_FIELDS now names recovery_required.** The one added hunk replaces, in the `KEY_FIELDS` string constant (model_tests.rs, line 1172 at C), `the test's table (classes, debt counter, H, A);` with `the test's table (classes, debt counter, H, A, recovery_required);`. No other text of KEY_FIELDS changes; its value names recovery_required exactly once.
- **The new hunk repairs raw receipt completeness.** Every exploration report prints `  state key fields: {KEY_FIELDS}` (three report sites, unchanged). With the R1 patch, A6's receipts would have listed the table as `(classes, debt counter, H, A)` while the key also held the flag; with R2 the printed table description names the flag as well.
- **The hunk does not alter executable model semantics beyond the already-defined A6 mutation.** It changes the value of a string that is only printed: no transition, property, trace text, branch label, field name, key layout or encoded position changes. Both static checkers verify this: the R1 mutant and the R2 mutant differ in exactly one line, the KEY_FIELDS table description.

**The added hunk (receipt metadata, not a second semantic mutation):**

```diff
@@ -1169,7 +1171,7 @@ occupancy counters, every refusal-table entry with its cause, the qualified fron
 fixed limits (asserted); the simulated ledger's rows beyond the initial ones; every ticket's state and holder \
 (FW1, FW2 or the batch writer, SE); the worker's presence; every retained token as cell and occupancy number \
 (the worker's agenda step and its last token for REPLAY); every snapshot's values and nonce; each actor's \
-position and any result it holds but has not applied; the test's table (classes, debt counter, H, A); every \
+position and any result it holds but has not applied; the test's table (classes, debt counter, H, A, recovery_required); every \
 remaining budget (faults, admissions, attempts, replays); the A2 kept bits; ENV fired; history facts \
 (conflicted, conflicted for E6, unrecorded results, gating insert issued, gating told Committed, Counted \
 tickets dropped)";
```

It is hunk 3 of 9 (between the R1 hunks for initial and encode, in file order). Its `@@` line's trailing text is git's default function-context heuristic (the nearest earlier line that begins with a letter, here a continuation line of the same string, cut at 80 bytes): it is produced by the R1 patch generator unchanged, and patch tools ignore it.

**A6 identities, R1 → R2.**

| | R1 | R2 |
|---|---|---|
| patch sha256 | `02e1fd1b60d4b1255079f5533072f5a213a92f8db9214984eb8b527975b76e86` | `0700052eb3e9fb91f36ee6d535b02b923ff007f1061490ac7dd16dd98d0d0ee8` |
| patch lines removed/added | 5/21 | 6/22 |
| hunks | 8 | 9 (the 8 R1 hunks byte for byte + 1) |
| edits | 8 | 9 (the 8 R1 edits + 1 receipt-metadata edit) |
| mutant git blob | `85d262dbdc445569951ec43e914de4c192ea675d` | `51e54aed7cc33ebc8348a74fe6c881c057336ee7` |
| mutant sha256 | `fb270f7513ad161be2a20c0f21ee5db92a05d376ab60d25a2f3034336cd856a8` | `59c2f87666ad3272661497e30d7a4a841dcc5aafd90569307cc4fdbd69b5a4b4` |
| restoration | reverse application → blob `28085ce232827301104b5d757af042ec933976f8` | reverse application → blob `28085ce232827301104b5d757af042ec933976f8` (both checkers) |

The exact differences are `comparison/A6-R1-TO-R2-PATCH.diff` (R1 patch → R2 patch: the index line's mutant blob and the added hunk) and `comparison/A6-R1-TO-R2-MUTANT.diff` (R1 mutant → R2 mutant: one line). A6's property (P12 or P15), configuration (F-5), test (`full_f5`), ceiling, receipt type and restoration mapping are R1's. Changed A6 catalog fields: `definition_origin`, `edits`, `execution_dependent_uncertainty`, `mutant_blob`, `mutant_sha256`, `one_named_change`, `patch`, `patch_lines_removed_added`, `patch_origin`, `patch_sha256`, `preserved_behaviour`, `r2_correction`, `receipt_metadata_item` — the patch and its identities, the appended edit, and the description text that the KEY_FIELDS hunk made stale (R1's uncertainty item that said KEY_FIELDS was not changed is replaced; the other description lists are R1's with one item or sentence appended).

## The other twenty

20 Architect-defined controls — C3a, C3b, C3c, C5, C6, C7, C8, C11, C15, C16, C17, C18a, C18b, C20, C23, C26, A1, A2, A3, A5 — are unchanged from R1: catalog entry equal as a whole, patch file byte-identical (`comparison/R1-TO-R2-PATCH-COMPARISON.md`). Their sections below render the same fields as R1's record, with R2's static receipts.

Common to all 21: the mutant failing receipt is type (a), the original explorer (`explore`) through the configuration's `full_*` test with `--exact --ignored`, on the pinned mutant executable, under the configuration's ceiling (F-1: 60,000,000; every other: 30,000,000); a quotient run is never a failing receipt (AD-1-R4 F, F.1). The restored side is type (e1) on the unchanged core, except C15 (AD-1-R5 F).

Static evidence for every one of the 21 (receipts): checker A and checker B both PASS (`receipts/20-checker-a.txt`, `receipts/21-checker-b.txt`; identical per-id verdicts); `rustfmt --check` on the patched disposable file passes: **SYNTAX_CHECKED_NOT_TYPECHECKED** (`receipts/30-syntax-check.txt`). Nothing was type-checked or compiled.

## Summary

| Id | § | File | Item | Property | Configuration | Edits | R2 | Patch sha256 | Mutant blob |
|---|---|---|---|---|---|---|---|---|---|
| C3a | 6.1 | `protocol.rs` | Publish::acknowledgement | P13 or P15 | F-7 | 1 | = R1 | `2442f63d9383dc0d…` | `5c74cfbd8124…` |
| C3b | 6.2 | `protocol.rs` | Publish::acknowledgement | P13 or P12 | F-6 | 1 | = R1 | `614e53fd75518991…` | `e6aff1884a4b…` |
| C3c | 6.3 | `protocol.rs` | Publish::acknowledgement | P12 | F-6 | 1 | = R1 | `1e60d98d6d42c0aa…` | `5cc9c09ac960…` |
| C5 | 6.4 | `protocol.rs` | Publish::resolve_ack | P13 | F-5 | 1 | = R1 | `a4776885b669bd91…` | `e1a49f5ba952…` |
| C6 | 6.5 | `protocol.rs` | Core::classify | P6 | F-PT | 1 | = R1 | `5c647500156a5e9c…` | `6dde859cb243…` |
| C7 | 6.6 | `protocol.rs` | Publish::explain | P6 | F-PT | 1 | = R1 | `b5d8bc6c9254e0ca…` | `0d4ab9929c0e…` |
| C8 | 6.7 | `protocol.rs` | Publish::facts | E5 | F-B | 1 | = R1 | `a7cdd1925bf34283…` | `fd6abbd8a80d…` |
| C11 | 6.8 | `protocol.rs` | Publish::facts (Step 5) | E4 | F-B2 | 1 | = R1 | `913997fd20c0da21…` | `6e039d9dad64…` |
| C15 | 6.9 | `protocol.rs` | Core::names | E8 | F-R | 1 | = R1 | `a9a5037d36b1a2a6…` | `abdd7518e6d1…` |
| C16 | 6.10 | `protocol.rs` | Drop for Ticket | E3 | F-D | 1 | = R1 | `e48a988cfc854881…` | `33f0d257df3e…` |
| C17 | 6.11 | `protocol.rs` | Drop for Publish | E4 | F-B2 | 1 | = R1 | `37b696d014ef6e25…` | `4dfed9e1600a…` |
| C18a | 6.12 | `protocol.rs` | Publish::gating | E7 | F-G | 1 | = R1 | `7ced1f36a56066c5…` | `92034563e479…` |
| C18b | 6.13 | `protocol.rs` | Core::classify (gating branch) | E7 | F-G | 1 | = R1 | `830f21c88f5ba827…` | `c7f2b5ea99bb…` |
| C20 | 6.14 | `protocol.rs` | Publish::cell | L1 | F-2 | 1 | = R1 | `5ea1889e67bc552d…` | `7bb5aa2abbb5…` |
| C23 | 6.15 | `protocol.rs` | Core::flags | E1 | F-2 | 1 | = R1 | `731ffbedf845790c…` | `cd50b40ce632…` |
| C26 | 6.16 | `protocol.rs` | Publish::barrier | E9 | F-4 | 1 | = R1 | `1e26422d79a19553…` | `88f1783305f6…` |
| A1 | 6.17 | `model_tests.rs` | step_fw | P1 | F-1 | 1 | = R1 | `e3a21c74babd86e1…` | `26c09e7ff936…` |
| A2 | 6.18 | `model_tests.rs` | step_adm | P1 | F-1 | 1 | = R1 | `5f1f30486da2457e…` | `c0ac428f5849…` |
| A3 | 6.19 | `model_tests.rs` | step_wk | P12 | F-5 | 2 | = R1 | `ad0c64489e962756…` | `72bab79d7168…` |
| A5 | 6.20 | `model_tests.rs` | claimable | L1 | F-2 | 1 | = R1 | `913eba6d31d7cbe6…` | `cb08f5e8c62b…` |
| A6 | 6.21 | `model_tests.rs` | Table; initial; encode; decode; step_wk (Seen, RaiseH); step_ack (Applying); step_adm; KEY_FIELDS (receipt metadata) | P12 or P15 | F-5 | 9 | changed (KEY_FIELDS hunk) | `0700052eb3e9fb91…` | `51e54aed7cc3…` |

## C3a — acknowledgement test 2 removed (section 6.1)

*ARCHITECT-SPECIFIED — EXECUTOR-ENCODED — INDEPENDENT ARCHITECT REVIEW PENDING*

**R2.** Unchanged from R1: the catalog entry equals R1's and the patch file is byte-identical.

**Adopted contract description** (contract 7.5, line 701):

```
| C3a | acknowledgement test 2 removed | P13 or P15 | F-7 |
```

**Named property / test / configuration.** P13 or P15 on **F-7**: `ledger::protocol::model_tests::full_f7` (ignored at C: True), the original explorer, ceiling 30,000,000 states. Printed property must be: either of P13 / P15.

**Receipt type.** (a) original-explorer mutant FAIL; restored (e1). Restored side: e1: docs/evidence/p3-candidate11/final-baseline-remaining-configs-01/receipts/67-full-F-7 at 89326b859792a442c1853c3d35d30a48301c009e.

**Special rules.** (1) AD-1-R4 F.1(a): either property the row names.

**Exact source item.** `crates/nexus-governed-control/src/ledger/protocol.rs` (blob `a2df2841b99cad5d452008a6d4150e9369d07e66`): Publish::acknowledgement; anchor line(s) at C: 1664.

**Architect encoding — mission R1 section 6.1 C3a — acknowledgement test 2 removed** (verbatim as received):

```
File:
protocol.rs

Item:
Publish::acknowledgement

Healthy Test 2:

set.frontier != now.frontier
  -> AckRefusal::Stale

Mutation:

Remove the complete Test-2 clause.

The refusal chain must begin directly with healthy Test 3:

set.top <= set.frontier || set.count < 1
  -> AckRefusal::Empty

Do not remove or alter:

- the recheck verb;
- Test 3;
- Test 4;
- Test 5;
- rollback;
- insertion;
- commit handling.

The whole frontier-equality test is removed.

Named properties:
P13 or P15

Configuration:
F-7
```

**Baseline anchor** (exactly once in C's blob, at line 1664):

```rust
        let refusal = if set.frontier != now.frontier {
            Some(AckRefusal::Stale)
        } else if set.top <= set.frontier || set.count < 1 {
```

**Replacement:**

```rust
        let refusal = if set.top <= set.frontier || set.count < 1 {
```

**Identities.**

- patch `catalog/patches/C3a.patch`: sha256 `2442f63d9383dc0d36dd934e2a6b8fb0b3d6936f215e00ba3d964ede73fb6298`; lines removed/added 3/1
- mutant `protocol.rs`: git blob `5c74cfbd8124a78c616526ae1d461895b1a16b82`, sha256 `abeb1664c20aa079fcbc869ed818f876c84e3af679dc6ea6f4fec72d805ad7a6`
- restoration: reverse application returns blob `a2df2841b99cad5d452008a6d4150e9369d07e66` (verified by both checkers)

**Preserved behaviour.**

- the recheck verb (`io.recheck(set.frontier, set.top)`) and its failure arm
- Test 3 (`set.top <= set.frontier || set.count < 1` -> Empty)
- Test 4 (`set.top > set.head || set.head > now.head` -> Future)
- Test 5 (`now.count != set.count || now.digest != set.digest` -> Set)
- the refusal rollback, the insertion and the commit handling

**Why this is one named change.** The whole frontier-equality clause (condition and its Stale result) is removed and the chain's head becomes Test 3's condition; no other clause is touched.

**Static validation.** checker A: PASS; checker B: PASS; syntax: SYNTAX_CHECKED_NOT_TYPECHECKED.

**Remaining execution-dependent uncertainty.**

- `AckRefusal::Stale` is no longer constructed in production text; rustc may warn (dead_code). The crate has no deny(warnings) and C's .cargo/config.toml sets no RUSTFLAGS; whether a future build environment denies warnings is not known statically.
- Whether F-7's original explorer reaches a P13 or P15 failure within 30,000,000 states is decided only by execution.

## C3b — acknowledgement test 4 removed (section 6.2)

*ARCHITECT-SPECIFIED — EXECUTOR-ENCODED — INDEPENDENT ARCHITECT REVIEW PENDING*

**R2.** Unchanged from R1: the catalog entry equals R1's and the patch file is byte-identical.

**Adopted contract description** (contract 7.5, line 702):

```
| C3b | acknowledgement test 4 removed | P13 or P12 | F-6 |
```

**Named property / test / configuration.** P13 or P12 on **F-6**: `ledger::protocol::model_tests::full_f6` (ignored at C: True), the original explorer, ceiling 30,000,000 states. Printed property must be: either of P13 / P12.

**Receipt type.** (a) original-explorer mutant FAIL; restored (e1). Restored side: e1: docs/evidence/p3-candidate11/final-baseline-remaining-configs-01/receipts/66-full-F-6 at 89326b859792a442c1853c3d35d30a48301c009e.

**Special rules.** (1) AD-1-R4 F.1(a): either property the row names.

**Exact source item.** `crates/nexus-governed-control/src/ledger/protocol.rs` (blob `a2df2841b99cad5d452008a6d4150e9369d07e66`): Publish::acknowledgement; anchor line(s) at C: 1668.

**Architect encoding — mission R1 section 6.2 C3b — acknowledgement test 4 removed** (verbatim as received):

```
File:
protocol.rs

Item:
Publish::acknowledgement

Mutation:

Remove the complete Test-4 clause:

set.top > set.head || set.head > now.head
  -> AckRefusal::Future

Both comparisons are removed together.

Connect Test 3 directly to Test 5.

Do not remove only one side of the disjunction.

Named properties:
P13 or P12

Configuration:
F-6
```

**Baseline anchor** (exactly once in C's blob, at line 1668):

```rust
        } else if set.top > set.head || set.head > now.head {
            Some(AckRefusal::Future)
```

**Replacement:**

*(empty: the anchor is removed)*

**Identities.**

- patch `catalog/patches/C3b.patch`: sha256 `614e53fd75518991c5e6518bc0fe61cf546cf092b955e3eda0e71cf25d5f6dd4`; lines removed/added 2/0
- mutant `protocol.rs`: git blob `e6aff1884a4b9b7f880eee8784c371648fb2c073`, sha256 `84dc2a424ce80ce47b71871f111c4aaa37d8b776dc4851f16513d86312247273`
- restoration: reverse application returns blob `a2df2841b99cad5d452008a6d4150e9369d07e66` (verified by both checkers)

**Preserved behaviour.**

- Test 2 (`set.frontier != now.frontier` -> Stale)
- Test 3 (`set.top <= set.frontier || set.count < 1` -> Empty)
- Test 5 (`now.count != set.count || now.digest != set.digest` -> Set)
- the recheck verb, the rollback, the insertion and the commit handling

**Why this is one named change.** The complete Test-4 clause (both comparisons and its Future result) is removed together; Test 3's clause is followed directly by Test 5's.

**Static validation.** checker A: PASS; checker B: PASS; syntax: SYNTAX_CHECKED_NOT_TYPECHECKED.

**Remaining execution-dependent uncertainty.**

- `AckRefusal::Future` is no longer constructed in production text (possible dead_code warning; see C3a).
- Whether F-6's original explorer reaches a P13 or P12 failure within 30,000,000 states is decided only by execution.

## C3c — acknowledgement test 5 removed (section 6.3)

*ARCHITECT-SPECIFIED — EXECUTOR-ENCODED — INDEPENDENT ARCHITECT REVIEW PENDING*

**R2.** Unchanged from R1: the catalog entry equals R1's and the patch file is byte-identical.

**Adopted contract description** (contract 7.5, line 703):

```
| C3c | acknowledgement test 5 removed | P12 | F-6 |
```

**Named property / test / configuration.** P12 on **F-6**: `ledger::protocol::model_tests::full_f6` (ignored at C: True), the original explorer, ceiling 30,000,000 states. Printed property must be: P12.

**Receipt type.** (a) original-explorer mutant FAIL; restored (e1). Restored side: e1: docs/evidence/p3-candidate11/final-baseline-remaining-configs-01/receipts/66-full-F-6 at 89326b859792a442c1853c3d35d30a48301c009e.

**Exact source item.** `crates/nexus-governed-control/src/ledger/protocol.rs` (blob `a2df2841b99cad5d452008a6d4150e9369d07e66`): Publish::acknowledgement; anchor line(s) at C: 1670.

**Architect encoding — mission R1 section 6.3 C3c — acknowledgement test 5 removed** (verbatim as received):

```
File:
protocol.rs

Item:
Publish::acknowledgement

Mutation:

Remove the complete Test-5 clause:

now.count != set.count || now.digest != set.digest
  -> AckRefusal::Set

After Test 4, the refusal chain falls directly to None.

Both the count and digest comparisons are removed together.

Named property:
P12

Configuration:
F-6
```

**Baseline anchor** (exactly once in C's blob, at line 1670):

```rust
        } else if now.count != set.count || now.digest != set.digest {
            Some(AckRefusal::Set)
```

**Replacement:**

*(empty: the anchor is removed)*

**Identities.**

- patch `catalog/patches/C3c.patch`: sha256 `1e60d98d6d42c0aab581a5bde34162f9767949a495fad801b73d9c740d9e2e85`; lines removed/added 2/0
- mutant `protocol.rs`: git blob `5cc9c09ac96062c5d348e86ace8cb0ac2225e83d`, sha256 `e72f8f619fa1c9b4a8379351138b354fa83fe470b1c56450b8b22f28c61710c2`
- restoration: reverse application returns blob `a2df2841b99cad5d452008a6d4150e9369d07e66` (verified by both checkers)

**Preserved behaviour.**

- Test 2, Test 3 and Test 4 clauses unchanged
- the final `else { None }` of the chain
- the recheck verb, the rollback, the insertion and the commit handling

**Why this is one named change.** The complete Test-5 clause (count and digest comparisons and its Set result) is removed together; after Test 4 the chain falls directly to None.

**Static validation.** checker A: PASS; checker B: PASS; syntax: SYNTAX_CHECKED_NOT_TYPECHECKED.

**Remaining execution-dependent uncertainty.**

- `AckRefusal::Set` is no longer constructed in production text (possible dead_code warning; see C3a).
- Whether F-6's original explorer reaches a P12 failure within 30,000,000 states is decided only by execution.

## C5 — resolve, for an acknowledgement in U, raises the frontier with neither the presence read nor the frontier check (section 6.4)

*ARCHITECT-SPECIFIED — EXECUTOR-ENCODED — INDEPENDENT ARCHITECT REVIEW PENDING*

**R2.** Unchanged from R1: the catalog entry equals R1's and the patch file is byte-identical.

**Adopted contract description** (contract 7.5, line 705):

```
| C5 | resolve, for an acknowledgement in U, raises the frontier with neither the presence read nor the frontier check | P13 | F-5 |
```

**Named property / test / configuration.** P13 on **F-5**: `ledger::protocol::model_tests::full_f5` (ignored at C: True), the original explorer, ceiling 30,000,000 states. Printed property must be: P13.

**Receipt type.** (a) original-explorer mutant FAIL; restored (e1). Restored side: e1: docs/evidence/p3-candidate11/final-baseline-remaining-configs-01/receipts/65-full-F-5 at 89326b859792a442c1853c3d35d30a48301c009e.

**Exact source item.** `crates/nexus-governed-control/src/ledger/protocol.rs` (blob `a2df2841b99cad5d452008a6d4150e9369d07e66`): Publish::resolve_ack; anchor line(s) at C: 1818.

**Architect encoding — mission R1 section 6.4 C5 — acknowledgement resolve raises frontier without reads** (verbatim as received):

```
File:
protocol.rs

Item:
Publish::resolve_ack

Mutation:

Replace the function body with this semantic sequence:

1. Issue NO acknowledgement read.
2. Issue NO frontier read.
3. If set.top is above core.frontier, assign core.frontier = set.top.
4. Empty U.
5. Set phase Healthy.
6. Return Resolve::Acknowledged(set.top).

Keep the existing function signature.

The nonce and io parameters become unused; do not alter a trait or
caller's signature to remove them.

Do not preserve any branch of the healthy match.

Named property:
P13

Configuration:
F-5
```

**Baseline anchor** (exactly once in C's blob, at line 1818):

```rust
    fn resolve_ack(&mut self, set: &SetFacts, nonce: &[u8; 16], io: &mut impl Reads) -> Resolve {
        match io.acknowledgement(set, nonce) {
            Read::Got(None) => {
                self.core.u = None;
                self.core.phase = Phase::Healthy;
                Resolve::Absent
            }
            Read::Got(Some(_)) => match io.frontier() {
                Read::Got(durable) if durable >= set.top => {
                    if set.top > self.core.frontier {
                        self.core.frontier = set.top;
                    }
                    self.core.u = None;
                    self.core.phase = Phase::Healthy;
                    Resolve::Acknowledged(set.top)
                }
                Read::Got(_) => {
                    self.core.latch(LatchCause::Inconsistent);
                    Resolve::Inconsistent
                }
                Read::Failed => {
                    self.failed();
                    Resolve::Retained
                }
                Read::Corrupt => {
                    self.core.latch(LatchCause::Corrupt);
                    Resolve::Refused(Entry::Latched)
                }
            },
            Read::Failed => {
                self.failed();
                Resolve::Retained
            }
            Read::Corrupt => {
                self.core.latch(LatchCause::Corrupt);
                Resolve::Refused(Entry::Latched)
            }
        }
    }
```

**Replacement:**

```rust
    fn resolve_ack(&mut self, set: &SetFacts, nonce: &[u8; 16], io: &mut impl Reads) -> Resolve {
        if set.top > self.core.frontier {
            self.core.frontier = set.top;
        }
        self.core.u = None;
        self.core.phase = Phase::Healthy;
        Resolve::Acknowledged(set.top)
    }
```

**Identities.**

- patch `catalog/patches/C5.patch`: sha256 `a4776885b669bd912fd8710d5a3283f4c1863ab9213e8474d26707b5a6e5f3d8`; lines removed/added 36/5
- mutant `protocol.rs`: git blob `e1a49f5ba952bc81cdac9f8f8ace9eeb2aa93495`, sha256 `31ca02cfaeb6058879af75091e2b1bdb21addf846d59ee3e5b4f5806fe30dedd`
- restoration: reverse application returns blob `a2df2841b99cad5d452008a6d4150e9369d07e66` (verified by both checkers)

**Preserved behaviour.**

- the function signature, byte for byte (`nonce` and `io` stay, unused)
- the caller `Publish::resolve` and every trait signature
- the gating resolve (`Publish::resolve_gating`)

**Why this is one named change.** One item's body is replaced by the section-6.4 sequence (no acknowledgement read, no frontier read; raise, empty U, Healthy, Acknowledged); no branch of the healthy match survives.

**Static validation.** checker A: PASS; checker B: PASS; syntax: SYNTAX_CHECKED_NOT_TYPECHECKED.

**Remaining execution-dependent uncertainty.**

- `nonce` and `io` become unused parameters (rustc unused_variables warnings, not errors under C's lint configuration); `Resolve::Inconsistent` and `LatchCause::Inconsistent` may lose their only production constructions (dead_code warnings).
- Whether F-5's original explorer reaches a P13 failure within 30,000,000 states is decided only by execution.

## C6 — classification row b ignores the refusal table, **with** A4 (section 6.5)

*ARCHITECT-SPECIFIED — EXECUTOR-ENCODED — INDEPENDENT ARCHITECT REVIEW PENDING*

**R2.** Unchanged from R1: the catalog entry equals R1's and the patch file is byte-identical.

**Adopted contract description** (contract 7.5, line 706):

```
| C6 | classification row b ignores the refusal table, **with** A4 | P6 | F-PT |
```

**Named property / test / configuration.** P6 on **F-PT**: `ledger::protocol::model_tests::full_fpt` (ignored at C: True), the original explorer, ceiling 30,000,000 states. Printed property must be: P6.

**Receipt type.** (a) original-explorer mutant FAIL; restored (e1). Restored side: e1: docs/evidence/p3-candidate11/final-baseline-known-runs-01/receipts/44-full-F-PT at f023cc3d031a2a89e5ecbd4086e15f2e50869acd.

**Special rules.** (1) contract 7.5: 'with A4' — run on F-PT (A4 on), original explorer (AD-1-R4 F).

**Exact source item.** `crates/nexus-governed-control/src/ledger/protocol.rs` (blob `a2df2841b99cad5d452008a6d4150e9369d07e66`): Core::classify; anchor line(s) at C: 1197.

**Architect encoding — mission R1 section 6.5 C6 — classification row b ignores refusal table** (verbatim as received):

```
File:
protocol.rs

Item:
Core::classify

Mutation scope:
row b inside Core::classify only.

Do NOT modify Core::known.
Do NOT modify lookup.
Do NOT modify the refusal table itself.

For classification row b, determine Known using only:

1. X holding this original's explanation:
   - Stage::Built -> Known::Cause
   - Stage::Committed(seq) -> Known::Explained(seq)

2. facts.explained:
   - Some(seq) -> Known::Explained(seq)

Ignore self.refusals.get(&member.key) entirely for this one
classification decision.

Preserve precedence:

X explanation before facts.explained.

If neither exists, continue to rows c–e.

Named property:
P6

Configuration:
F-PT, with A4 as already defined by the configuration.
```

**Baseline anchor** (exactly once in C's blob, at line 1197):

```rust
        if let Some(known) = self.known(&member.key, facts.explained) {
            return Verdict::Refused(Reason::Conflict { member: at, known }, Some(at));
        }
```

**Replacement:**

```rust
        let known = match &self.x {
            Some((_, Occupant::Explanation { original, cause }, stage))
                if *original == member.key =>
            {
                Some(match stage {
                    Stage::Built => Known::Cause(*cause),
                    Stage::Committed(seq) => Known::Explained(*seq),
                })
            }
            _ => facts.explained.map(Known::Explained),
        };
        if let Some(known) = known {
            return Verdict::Refused(Reason::Conflict { member: at, known }, Some(at));
        }
```

**Identities.**

- patch `catalog/patches/C6.patch`: sha256 `5c647500156a5e9c9b3faa27ce59ea2c09f0c45d1a50daa7a7d1d0a33c5a7e5e`; lines removed/added 1/12
- mutant `protocol.rs`: git blob `6dde859cb243b9f4304cda61b0917b1fc555dde3`, sha256 `649d95ba8232cefa81d71c7793d8413e956271efebc25bc291ddb4288084c2a3`
- restoration: reverse application returns blob `a2df2841b99cad5d452008a6d4150e9369d07e66` (verified by both checkers)

**Preserved behaviour.**

- `Core::known` unchanged (still used by lookup and by the model's step_gc)
- `Publish::lookup` unchanged
- the refusal table itself, and its insertion in rows d and e, unchanged
- precedence: X's explanation of this original before facts.explained
- row a (Present) before row b; rows c to e after it

**Why this is one named change.** Only classification row b's Known source changes: the classify call site evaluates X's explanation of this original, then facts.explained, without consulting self.refusals; every other caller of Core::known keeps the healthy rule.

**Static validation.** checker A: PASS; checker B: PASS; syntax: SYNTAX_CHECKED_NOT_TYPECHECKED.

**Remaining execution-dependent uncertainty.**

- The F-PT configuration carries A4 (blanket Uncertain) by definition; whether the original explorer reaches a P6 failure within 30,000,000 states is decided only by execution.

## C7 — *Explained* removes every table entry, **with** A4 (section 6.6)

*ARCHITECT-SPECIFIED — EXECUTOR-ENCODED — INDEPENDENT ARCHITECT REVIEW PENDING*

**R2.** Unchanged from R1: the catalog entry equals R1's and the patch file is byte-identical.

**Adopted contract description** (contract 7.5, line 707):

```
| C7 | *Explained* removes every table entry, **with** A4 | P6 | F-PT |
```

**Named property / test / configuration.** P6 on **F-PT**: `ledger::protocol::model_tests::full_fpt` (ignored at C: True), the original explorer, ceiling 30,000,000 states. Printed property must be: P6.

**Receipt type.** (a) original-explorer mutant FAIL; restored (e1). Restored side: e1: docs/evidence/p3-candidate11/final-baseline-known-runs-01/receipts/44-full-F-PT at f023cc3d031a2a89e5ecbd4086e15f2e50869acd.

**Special rules.** (1) contract 7.5: 'with A4' — run on F-PT (A4 on), original explorer (AD-1-R4 F). (2) AD-1-R4 F (line 748): C7 without A4 on F-3 is an added run, not required to kill.

**Additional comparison (not required to kill).** AD-1-R4 F (line 748): C7 without A4 — C7's mutant build run by full_f3 (the original explorer, configuration F-3, A4 off), under 30,000,000; expected to fail E11; added to the receipts; not required to kill; its unchanged-core side may cite the accepted exact-final F-3 composed-quotient PASS under AD-1-R4's prerequisites. Patch: the same patches/C7.patch in both executions (mission 6.6).

**Exact source item.** `crates/nexus-governed-control/src/ledger/protocol.rs` (blob `a2df2841b99cad5d452008a6d4150e9369d07e66`): Publish::explain; anchor line(s) at C: 1559.

**Architect encoding — mission R1 section 6.6 C7 — Explained clears every refusal-table entry** (verbatim as received):

```
File:
protocol.rs

Item:
Publish::explain

Mutation:

Only in:

Read::Got(Some(seq))

replace removal of the one key with clearing the entire refusal table.

Healthy:
self.core.refusals.remove(key);

Mutant semantic:
self.core.refusals.clear();

Return Explain::Explained(seq) unchanged.

Do NOT alter:

- Publish::settle;
- X's committed path;
- any other table-removal site.

Named property:
P6

Configuration:
F-PT, with A4.

Additional required comparison:
C7 without A4 remains the separate original-explorer F-3 comparison
defined by AD-1-R4. This patch is the same in both executions.
```

**Baseline anchor** (exactly once in C's blob, at line 1559):

```rust
            Read::Got(Some(seq)) => {
                self.core.refusals.remove(key);
```

**Replacement:**

```rust
            Read::Got(Some(seq)) => {
                self.core.refusals.clear();
```

**Identities.**

- patch `catalog/patches/C7.patch`: sha256 `b5d8bc6c9254e0ca2853fcf2f9f220f24782ac6cfa77efe7032365d76f0efbc4`; lines removed/added 1/1
- mutant `protocol.rs`: git blob `0d4ab9929c0e15d910735a52b72a5f3855bef58a`, sha256 `6d2f2d49842cdf5cc155dd1cb0984e121ada154cfc69a412219c7ac12dc846aa`
- restoration: reverse application returns blob `a2df2841b99cad5d452008a6d4150e9369d07e66` (verified by both checkers)

**Preserved behaviour.**

- `return Explain::Explained(seq);` unchanged
- `Publish::settle` (X's committed path) and its `self.core.refusals.remove(&original);` unchanged
- every other table-removal site unchanged

**Why this is one named change.** In explain's Read::Got(Some(seq)) arm only, removal of the one key is replaced by clearing the whole table.

**Static validation.** checker A: PASS; checker B: PASS; syntax: SYNTAX_CHECKED_NOT_TYPECHECKED.

**Remaining execution-dependent uncertainty.**

- The same patch serves the required F-PT (with A4) kill run and the separate AD-1-R4 F (line 748) original-explorer F-3 comparison without A4, which is not required to kill.
- Whether F-PT's original explorer reaches a P6 failure within 30,000,000 states is decided only by execution.

## C8 — members are classified and inserted one at a time (section 6.7)

*ARCHITECT-SPECIFIED — EXECUTOR-ENCODED — INDEPENDENT ARCHITECT REVIEW PENDING*

**R2.** Unchanged from R1: the catalog entry equals R1's and the patch file is byte-identical.

**Adopted contract description** (contract 7.5, line 708):

```
| C8 | members are classified and inserted one at a time | E5 | F-B |
```

**Named property / test / configuration.** E5 on **F-B**: `ledger::protocol::model_tests::full_fb` (ignored at C: True), the original explorer, ceiling 30,000,000 states. Printed property must be: E5.

**Receipt type.** (a) original-explorer mutant FAIL; restored (e1). Restored side: e1: docs/evidence/p3-candidate11/final-baseline-known-runs-01/receipts/42-full-F-B at f023cc3d031a2a89e5ecbd4086e15f2e50869acd.

**Exact source item.** `crates/nexus-governed-control/src/ledger/protocol.rs` (blob `a2df2841b99cad5d452008a6d4150e9369d07e66`): Publish::facts; anchor line(s) at C: 1348.

**Architect encoding — mission R1 section 6.7 C8 — classify and insert each member immediately** (verbatim as received):

```
File:
protocol.rs

Item:
Publish::facts

Mutation:

Replace the healthy two-phase Step-3/Step-5 structure with one
member-order loop.

Exact algorithm:

1. Keep Steps 0, 1 and 2 unchanged.
2. Start any_inserted = false.
3. For each member in order:
   a. issue facts;
   b. classify using the same classify call and earlier-member input;
   c. Present:
      continue;
   d. Insert:
      - set self.issued = true;
      - set any_inserted = true;
      - issue insert immediately for this member;
      - Inserted: continue to the next member;
      - Constraint:
        latch Constraint;
        self.failed();
        issue exactly one rollback;
        use the rollback-derived persistence exactly as healthy Step 5;
        return NotApplied with Reason::Constraint;
      - Failed:
        self.failed();
        issue exactly one rollback;
        use the rollback-derived persistence;
        return NotApplied with Reason::Storage;
   e. Refused:
      - set self.conflict from the classification result;
      - issue exactly one rollback;
      - return NotApplied(NotWritten, the classification reason),
        even when an earlier member has already been inserted.
4. After the loop:
   - if no insert was issued:
     issue the healthy empty-transaction rollback and return Committed;
   - otherwise run the unchanged healthy Step-6 commit handling.

Remove the pending array and the later insertion loop.

The fault is the interleaving of classification and insertion.
Do not add a second fault.

Named property:
E5

Configuration:
F-B
```

**Baseline anchor** (exactly once in C's blob, at line 1348):

```rust
        // Step 3: every member classified before any insert.
        let mut pending = [false; BATCH_MAX];
        let mut any = false;
        let members = || batch.iter().map(FactRecord::member);
        for (index, (member, slot)) in members().zip(pending.iter_mut()).enumerate() {
            let at = member_index(index);
            let facts = match io.facts(at, member) {
                Read::Got(facts) => facts,
                failed => {
                    self.read_failed(failed == Read::Corrupt);
                    self.rolled(io.rollback());
                    return self.not_applied(Persistence::NotWritten, Reason::Storage);
                }
            };
            match self
                .core
                .classify(at, member, &facts, members().take(index), false)
            {
                Verdict::Present(_) => {}
                Verdict::Insert => {
                    *slot = true;
                    any = true;
                }
                Verdict::Refused(reason, conflict) => {
                    self.conflict = conflict;
                    self.rolled(io.rollback());
                    return self.not_applied(Persistence::NotWritten, reason);
                }
            }
        }
        // Step 4.
        if !any {
            self.rolled(io.rollback());
            return FactOutcome::Committed;
        }
        // Step 5.
        for (index, (member, slot)) in members().zip(pending.iter()).enumerate() {
            if !*slot {
                continue;
            }
            self.issued = true;
            let reason = match io.insert(member_index(index), member) {
                Insert::Inserted(_) => continue,
                Insert::Constraint => {
                    self.core.latch(LatchCause::Constraint);
                    Reason::Constraint
                }
                Insert::Failed => Reason::Storage,
            };
            self.failed();
            let persistence = self.rolled(io.rollback());
            return self.not_applied(persistence, reason);
        }
```

**Replacement:**

```rust
        let mut any_inserted = false;
        let members = || batch.iter().map(FactRecord::member);
        for (index, member) in members().enumerate() {
            let at = member_index(index);
            let facts = match io.facts(at, member) {
                Read::Got(facts) => facts,
                failed => {
                    self.read_failed(failed == Read::Corrupt);
                    self.rolled(io.rollback());
                    return self.not_applied(Persistence::NotWritten, Reason::Storage);
                }
            };
            match self
                .core
                .classify(at, member, &facts, members().take(index), false)
            {
                Verdict::Present(_) => {}
                Verdict::Insert => {
                    self.issued = true;
                    any_inserted = true;
                    let reason = match io.insert(at, member) {
                        Insert::Inserted(_) => continue,
                        Insert::Constraint => {
                            self.core.latch(LatchCause::Constraint);
                            Reason::Constraint
                        }
                        Insert::Failed => Reason::Storage,
                    };
                    self.failed();
                    let persistence = self.rolled(io.rollback());
                    return self.not_applied(persistence, reason);
                }
                Verdict::Refused(reason, conflict) => {
                    self.conflict = conflict;
                    self.rolled(io.rollback());
                    return self.not_applied(Persistence::NotWritten, reason);
                }
            }
        }
        // Step 4.
        if !any_inserted {
            self.rolled(io.rollback());
            return FactOutcome::Committed;
        }
```

**Identities.**

- patch `catalog/patches/C8.patch`: sha256 `a7cdd1925bf34283880384b78dd1c918fc61e64afef10094ab6d947bdeb97927`; lines removed/added 25/16
- mutant `protocol.rs`: git blob `fd6abbd8a80dc8a01620981c76427ca76a9e1f7d`, sha256 `8baa7d6daa9e2adf3ea0b382a04d81e0d340cf40f1a09ecccd24a87bb849c30a`
- restoration: reverse application returns blob `a2df2841b99cad5d452008a6d4150e9369d07e66` (verified by both checkers)

**Preserved behaviour.**

- Steps 0, 1 and 2 unchanged
- the facts read and its failure arm, byte for byte (healthy text carried into the loop)
- the classify call and its earlier-member input (`members().take(index)`)
- the insert arms: Constraint latches; Constraint and Failed each take self.failed(), exactly one rollback and the rollback-derived persistence
- a refusal: self.conflict from the verdict, exactly one rollback, Not written with the classification reason
- the no-insert path: the empty-transaction rollback and Committed
- Step 6 commit handling unchanged

**Why this is one named change.** The two-phase Step-3/Step-5 structure becomes one member-order loop: the pending array and the separate insertion loop are removed, and each member's insert follows its own classification at once. The healthy comment '// Step 3: every member classified before any insert.' and the '// Step 5.' marker, which describe the removed structure, are removed with it; no comment is added.

**Static validation.** checker A: PASS; checker B: PASS; syntax: SYNTAX_CHECKED_NOT_TYPECHECKED.

**Remaining execution-dependent uncertainty.**

- The facts-read failure arm is carried unchanged into the merged loop: after an earlier member was inserted it still reports Not written (the healthy arm's text), as section 6.7 specifies for a refusal. If the explorer's first violation on F-B is that arm's persistence (E4) rather than E5, the printed property would differ; only execution decides.
- `BATCH_MAX` keeps its other use (`well_formed`), so no new unused item.
- Whether F-B's original explorer reaches an E5 failure within 30,000,000 states is decided only by execution.

## C11 — a constraint failure during the inserts gives Not written whatever the rollback said. **This is the amendment's rule, which the review rejected.** (section 6.8)

*ARCHITECT-SPECIFIED — EXECUTOR-ENCODED — INDEPENDENT ARCHITECT REVIEW PENDING*

**R2.** Unchanged from R1: the catalog entry equals R1's and the patch file is byte-identical.

**Adopted contract description** (contract 7.5, line 711):

```
| C11 | a constraint failure during the inserts gives Not written whatever the rollback said. **This is the amendment's rule, which the review rejected.** | E4 | F-B2 |
```

**Named property / test / configuration.** E4 on **F-B2**: `ledger::protocol::model_tests::full_fb2` (ignored at C: True), the original explorer, ceiling 30,000,000 states. Printed property must be: E4.

**Receipt type.** (a) original-explorer mutant FAIL; restored (e1). Restored side: e1: docs/evidence/p3-candidate11/final-baseline-remaining-configs-01/receipts/61-full-F-B2 at 89326b859792a442c1853c3d35d30a48301c009e.

**Exact source item.** `crates/nexus-governed-control/src/ledger/protocol.rs` (blob `a2df2841b99cad5d452008a6d4150e9369d07e66`): Publish::facts (Step 5); anchor line(s) at C: 1389.

**Architect encoding — mission R1 section 6.8 C11 — constraint insertion failure always Not written** (verbatim as received):

```
File:
protocol.rs

Item:
Publish::facts

Scope:
fact-append Step 5 only.

Do NOT modify:

- gating insert constraint;
- acknowledgement insert constraint;
- cell insert constraint;
- another operation's rollback handling.

Mutation of the fact Step-5 Insert::Constraint arm:

1. Latch Constraint.
2. Set phase through self.failed().
3. Issue exactly one rollback.
4. Ignore the rollback's persistence result.
5. Return NotApplied:
   persistence = NotWritten
   reason = Constraint.

Insert::Failed keeps the healthy rollback-derived persistence.

Named property:
E4

Configuration:
F-B2
```

**Baseline anchor** (exactly once in C's blob, at line 1389):

```rust
            let reason = match io.insert(member_index(index), member) {
                Insert::Inserted(_) => continue,
                Insert::Constraint => {
                    self.core.latch(LatchCause::Constraint);
                    Reason::Constraint
                }
```

**Replacement:**

```rust
            let reason = match io.insert(member_index(index), member) {
                Insert::Inserted(_) => continue,
                Insert::Constraint => {
                    self.core.latch(LatchCause::Constraint);
                    self.failed();
                    self.rolled(io.rollback());
                    return self.not_applied(Persistence::NotWritten, Reason::Constraint);
                }
```

**Identities.**

- patch `catalog/patches/C11.patch`: sha256 `913997fd20c0da21a39cb18b4d3fb6fd44cc0f1a22d42309fa25f2707111c768`; lines removed/added 1/3
- mutant `protocol.rs`: git blob `6e039d9dad6415242884ceef97a54d3548658038`, sha256 `f86ba9613b84c59e74cc1af6822db4a20b8dafea8f8ea46a57a74c19125aea01`
- restoration: reverse application returns blob `a2df2841b99cad5d452008a6d4150e9369d07e66` (verified by both checkers)

**Preserved behaviour.**

- Insert::Failed keeps the healthy shared path: self.failed(), one rollback, rollback-derived persistence
- the gating insert constraint (Publish::gating) unchanged
- the acknowledgement insert constraint (Publish::acknowledgement) unchanged
- the cell insert constraint (Publish::cell) unchanged
- exactly one rollback on the Constraint path (the arm returns before the shared rollback)

**Why this is one named change.** Only the fact append's Step-5 Insert::Constraint arm changes: it latches, fails, issues its one rollback, ignores the rollback's answer and returns Not written with Reason::Constraint.

**Static validation.** checker A: PASS; checker B: PASS; syntax: SYNTAX_CHECKED_NOT_TYPECHECKED.

**Remaining execution-dependent uncertainty.**

- Whether F-B2's original explorer reaches an E4 failure within 30,000,000 states is decided only by execution.

## C15 — a cell operation ignores the occupancy number (section 6.9)

*ARCHITECT-SPECIFIED — EXECUTOR-ENCODED — INDEPENDENT ARCHITECT REVIEW PENDING*

**R2.** Unchanged from R1: the catalog entry equals R1's and the patch file is byte-identical.

**Adopted contract description** (contract 7.5, line 713):

```
| C15 | a cell operation ignores the occupancy number | E8 | F-R |
```

**Named property / test / configuration.** E8 on **F-R**: `ledger::protocol::model_tests::full_fr` (ignored at C: True), the original explorer, ceiling 30,000,000 states. Printed property must be: E8.

**Receipt type.** (a) original-explorer mutant FAIL; restored (e1). Restored side: AD-1-R5 F: the adopted F-R proof discharge record, citable only after verified restoration to Candidate C; the mutant failing receipt stays the original full_fr, 30,000,000.

**Special rules.** (1) AD-1-R5 F: original explorer, E8, 30,000,000; FAIL beyond the cap is INCOMPLETE — BUDGET EXCEEDED; restored side may cite the F-R refinement discharge record only after verified restoration.

**Exact source item.** `crates/nexus-governed-control/src/ledger/protocol.rs` (blob `a2df2841b99cad5d452008a6d4150e9369d07e66`): Core::names; anchor line(s) at C: 1113.

**Architect encoding — mission R1 section 6.9 C15 — occupancy number ignored** (verbatim as received):

```
File:
protocol.rs

Item:
Core::names

Healthy conditions:

- same ledger allocation;
- token.cell equals requested cell;
- current occupancy equals token.occupancy.

Mutation:

Remove only the occupancy-equality condition.

Mutant Core::names returns true when:

- the ledger allocation is the same; and
- token.cell equals the requested cell.

Do NOT replace the occupancy comparison with is_some().
A token for an empty cell is also accepted if ledger and cell match.

Named property:
E8

Configuration:
F-R

C15's run/restoration policy remains exactly AD-1-R5:

- mutant failing receipt: original full_fr, 30M;
- restored side may cite the adopted F-R proof discharge only after
  verified restoration.
```

**Baseline anchor** (exactly once in C's blob, at line 1113):

```rust
        Arc::ptr_eq(&token.ledger, &self.shared)
            && token.cell == cell
            && self.occupancy(cell) == Some(token.occupancy)
```

**Replacement:**

```rust
        Arc::ptr_eq(&token.ledger, &self.shared) && token.cell == cell
```

**Identities.**

- patch `catalog/patches/C15.patch`: sha256 `a9a5037d36b1a2a6fc5fddb5accea1198ea08d3823a3044bf45f1c4b6cea5810`; lines removed/added 3/1
- mutant `protocol.rs`: git blob `abdd7518e6d1202a3a7c756af9716dc66183fd21`, sha256 `271b13d4422d0ec8322951cb71c2979a80b98d61975c3ab9ee0e5c69bc62141b`
- restoration: reverse application returns blob `a2df2841b99cad5d452008a6d4150e9369d07e66` (verified by both checkers)

**Preserved behaviour.**

- the same-ledger-allocation condition (`Arc::ptr_eq(&token.ledger, &self.shared)`)
- the cell-equality condition (`token.cell == cell`)
- no `is_some()` occupancy test is introduced: a token naming an empty cell passes if ledger and cell match
- every caller of Core::names (resolve, the cell appends, empty) unchanged

**Why this is one named change.** Only the occupancy-equality conjunct of Core::names is removed; the two remaining conjuncts are rejoined on one line as rustfmt requires.

**Static validation.** checker A: PASS; checker B: PASS; syntax: SYNTAX_CHECKED_NOT_TYPECHECKED.

**Remaining execution-dependent uncertainty.**

- AD-1-R5 governs C15's run: original full_fr, 30,000,000; its restored side may cite the adopted F-R proof discharge only after verified restoration. Whether the mutant fails E8 within the ceiling is decided only by execution.

## C16 — the drop of a Counted ticket lowers the count (section 6.10)

*ARCHITECT-SPECIFIED — EXECUTOR-ENCODED — INDEPENDENT ARCHITECT REVIEW PENDING*

**R2.** Unchanged from R1: the catalog entry equals R1's and the patch file is byte-identical.

**Adopted contract description** (contract 7.5, line 714):

```
| C16 | the drop of a Counted ticket lowers the count | E3 | F-D |
```

**Named property / test / configuration.** E3 on **F-D**: `ledger::protocol::model_tests::full_fd` (ignored at C: True), the original explorer, ceiling 30,000,000 states. Printed property must be: E3.

**Receipt type.** (a) original-explorer mutant FAIL; restored (e1). Restored side: e1: docs/evidence/p3-candidate11/final-baseline-remaining-configs-01/receipts/68-full-F-D at 89326b859792a442c1853c3d35d30a48301c009e.

**Exact source item.** `crates/nexus-governed-control/src/ledger/protocol.rs` (blob `a2df2841b99cad5d452008a6d4150e9369d07e66`): Drop for Ticket; anchor line(s) at C: 397.

**Architect encoding — mission R1 section 6.10 C16 — Counted drop lowers unapplied** (verbatim as received):

```
File:
protocol.rs

Item:
Drop for Ticket

Scope:
TicketState::Counted only.

Healthy:

the handover fault is set;
live and unapplied remain unchanged.

Mutation:

- preserve setting HANDOVER_FAULT;
- lower unapplied exactly once with count_step;
- leave live unchanged.

Required resulting semantic expression:

count_step(word | HANDOVER_FAULT, UNAPPLIED_SHIFT, false)

Do not call the full Counted confirm path.
Do not lower live.
Do not remove the handover fault.

Named property:
E3

Configuration:
F-D
```

**Baseline anchor** (exactly once in C's blob, at line 397):

```rust
            TicketState::Counted(_) => {
                exchange(&self.shared, 0, |word| word | HANDOVER_FAULT);
```

**Replacement:**

```rust
            TicketState::Counted(_) => {
                exchange(&self.shared, 0, |word| {
                    count_step(word | HANDOVER_FAULT, UNAPPLIED_SHIFT, false)
                });
```

**Identities.**

- patch `catalog/patches/C16.patch`: sha256 `e48a988cfc85488141d1e9eb033f897135ad9a4509edf16a3df85b55f8ff5e97`; lines removed/added 1/3
- mutant `protocol.rs`: git blob `33f0d257df3e126c589becd0b1c5760c57085be8`, sha256 `c8c2b3ef54f2a2d13dc69aa0922cd67166021f9705ba32782bcfbf1e4db8c9d4`
- restoration: reverse application returns blob `a2df2841b99cad5d452008a6d4150e9369d07e66` (verified by both checkers)

**Preserved behaviour.**

- HANDOVER_FAULT is still set
- `live` is not lowered (no LIVE_SHIFT step)
- the Armed and Settled arms of Drop for Ticket unchanged
- Ticket::confirm (the full Counted confirm path) unchanged and not called

**Why this is one named change.** Only the Counted arm of Drop for Ticket changes: its exchange closure becomes count_step(word | HANDOVER_FAULT, UNAPPLIED_SHIFT, false).

**Static validation.** checker A: PASS; checker B: PASS; syntax: SYNTAX_CHECKED_NOT_TYPECHECKED.

**Remaining execution-dependent uncertainty.**

- Whether F-D's original explorer reaches an E3 failure within 30,000,000 states is decided only by execution.

## C17 — the guard reports Not written after an insert was issued (section 6.11)

*ARCHITECT-SPECIFIED — EXECUTOR-ENCODED — INDEPENDENT ARCHITECT REVIEW PENDING*

**R2.** Unchanged from R1: the catalog entry equals R1's and the patch file is byte-identical.

**Adopted contract description** (contract 7.5, line 715):

```
| C17 | the guard reports Not written after an insert was issued | E4 | F-B2 |
```

**Named property / test / configuration.** E4 on **F-B2**: `ledger::protocol::model_tests::full_fb2` (ignored at C: True), the original explorer, ceiling 30,000,000 states. Printed property must be: E4.

**Receipt type.** (a) original-explorer mutant FAIL; restored (e1). Restored side: e1: docs/evidence/p3-candidate11/final-baseline-remaining-configs-01/receipts/61-full-F-B2 at 89326b859792a442c1853c3d35d30a48301c009e.

**Exact source item.** `crates/nexus-governed-control/src/ledger/protocol.rs` (blob `a2df2841b99cad5d452008a6d4150e9369d07e66`): Drop for Publish; anchor line(s) at C: 1972.

**Architect encoding — mission R1 section 6.11 C17 — unwind guard reports Not written after insert** (verbatim as received):

```
File:
protocol.rs

Item:
Drop for Publish

Scope:
the !self.done ticket-counting persistence calculation only.

Healthy:

issued with no conflict -> Uncertain
otherwise -> NotWritten

Mutation:

Always use Persistence::NotWritten for the Counted ticket created
during unwind.

Preserve unchanged:

- phase = NeedBarrier;
- self.conflict and conflict_member;
- self.issued;
- self.keep = self.issued;
- U retention and exit marking;
- the publication;
- ticket-state assignment.

Named property:
E4

Configuration:
F-B2
```

**Baseline anchor** (exactly once in C's blob, at line 1972):

```rust
                let persistence = if self.issued && self.conflict.is_none() {
                    Persistence::Uncertain
                } else {
                    Persistence::NotWritten
                };
```

**Replacement:**

```rust
                let persistence = Persistence::NotWritten;
```

**Identities.**

- patch `catalog/patches/C17.patch`: sha256 `37b696d014ef6e251119e6b9b489e01d701b483153330d92592ec27e34613ac0`; lines removed/added 5/1
- mutant `protocol.rs`: git blob `4dfed9e1600a82b38f3326d26a47c4c41f513ac3`, sha256 `94b303d898fe0dd664c2d70d2bbe7c0722e69c355631e128298295ee916606bf`
- restoration: reverse application returns blob `a2df2841b99cad5d452008a6d4150e9369d07e66` (verified by both checkers)

**Preserved behaviour.**

- `self.core.phase = Phase::NeedBarrier;` on unwind
- self.conflict and conflict_member unchanged
- self.issued unchanged
- `self.keep = self.issued;` unchanged
- U retention and exit marking unchanged
- the publication (exchange) and the ticket-state assignment unchanged

**Why this is one named change.** Only the unwind persistence calculation changes: the Counted ticket created during unwind always carries Persistence::NotWritten.

**Static validation.** checker A: PASS; checker B: PASS; syntax: SYNTAX_CHECKED_NOT_TYPECHECKED.

**Remaining execution-dependent uncertainty.**

- Whether F-B2's original explorer reaches an E4 failure within 30,000,000 states is decided only by execution.

## C18a — a refused gating append leaves U occupied (section 6.12)

*ARCHITECT-SPECIFIED — EXECUTOR-ENCODED — INDEPENDENT ARCHITECT REVIEW PENDING*

**R2.** Unchanged from R1: the catalog entry equals R1's and the patch file is byte-identical.

**Adopted contract description** (contract 7.5, line 716):

```
| C18a | a refused gating append leaves U occupied | E7 | F-G |
```

**Named property / test / configuration.** E7 on **F-G**: `ledger::protocol::model_tests::full_fg` (ignored at C: True), the original explorer, ceiling 30,000,000 states. Printed property must be: E7.

**Receipt type.** (a) original-explorer mutant FAIL; restored (e1). Restored side: e1: docs/evidence/p3-candidate11/final-baseline-remaining-configs-01/receipts/60-full-F-G at 89326b859792a442c1853c3d35d30a48301c009e.

**Exact source item.** `crates/nexus-governed-control/src/ledger/protocol.rs` (blob `a2df2841b99cad5d452008a6d4150e9369d07e66`): Publish::gating; anchor line(s) at C: 1462.

**Architect encoding — mission R1 section 6.12 C18a — classification-refused gating attempt keeps U** (verbatim as received):

```
File:
protocol.rs

Item:
Publish::gating

Scope only:

Verdict::Refused(reason, _) returned by Core::classify after the
gating record has been placed in U.

Mutation:

Before the existing rollback and refused return, set:

self.keep = true;

Preserve:

- the rollback;
- persistence = NotWritten;
- the refusal reason;
- all open/read/insert/commit failure paths;
- the guard's normal exit marking.

Do not set keep on every storage failure.
Do not change entry refusals before U is placed.

Named property:
E7

Configuration:
F-G
```

**Baseline anchor** (exactly once in C's blob, at line 1462):

```rust
            Verdict::Refused(reason, _) => {
                self.rolled(io.rollback());
                return refused(Persistence::NotWritten, reason);
```

**Replacement:**

```rust
            Verdict::Refused(reason, _) => {
                self.keep = true;
                self.rolled(io.rollback());
                return refused(Persistence::NotWritten, reason);
```

**Identities.**

- patch `catalog/patches/C18a.patch`: sha256 `7ced1f36a56066c596913442cc3dd93abe23beb1affab728b914fa26f60065fa`; lines removed/added 0/1
- mutant `protocol.rs`: git blob `92034563e479d1d622b7625e6ff4ad075a32a16a`, sha256 `94db8208ef639878a1e27e0ff960901d92b30034a881b183c00be20d72405bee`
- restoration: reverse application returns blob `a2df2841b99cad5d452008a6d4150e9369d07e66` (verified by both checkers)

**Preserved behaviour.**

- the rollback and persistence = NotWritten
- the refusal reason
- the open, facts-read, insert and commit failure paths of gating (no keep added there)
- the entry refusals before U is placed
- the guard's normal exit marking (Drop for Publish)

**Why this is one named change.** Only the classification-refused arm of gating sets `self.keep = true;` before its rollback and return.

**Static validation.** checker A: PASS; checker B: PASS; syntax: SYNTAX_CHECKED_NOT_TYPECHECKED.

**Remaining execution-dependent uncertainty.**

- The guard then marks the kept gating occupant ReturnedUncertain (healthy Drop for Publish behaviour). Whether F-G's original explorer reaches an E7 failure within 30,000,000 states is decided only by execution.

## C18b — a gating conflict makes a table entry (section 6.13)

*ARCHITECT-SPECIFIED — EXECUTOR-ENCODED — INDEPENDENT ARCHITECT REVIEW PENDING*

**R2.** Unchanged from R1: the catalog entry equals R1's and the patch file is byte-identical.

**Adopted contract description** (contract 7.5, line 717):

```
| C18b | a gating conflict makes a table entry | E7 | F-G |
```

**Named property / test / configuration.** E7 on **F-G**: `ledger::protocol::model_tests::full_fg` (ignored at C: True), the original explorer, ceiling 30,000,000 states. Printed property must be: E7.

**Receipt type.** (a) original-explorer mutant FAIL; restored (e1). Restored side: e1: docs/evidence/p3-candidate11/final-baseline-remaining-configs-01/receipts/60-full-F-G at 89326b859792a442c1853c3d35d30a48301c009e.

**Exact source item.** `crates/nexus-governed-control/src/ledger/protocol.rs` (blob `a2df2841b99cad5d452008a6d4150e9369d07e66`): Core::classify (gating branch); anchor line(s) at C: 1211.

**Architect encoding — mission R1 section 6.13 C18b — gating new cause also enters refusal table** (verbatim as received):

```
File:
protocol.rs

Item:
Core::classify

Scope:
the gating == true new-cause branch after row d creates cause.

Healthy gating behavior:

- build X explanation when X is empty;
- create no refusal-table entry;
- return the conflict.

Mutation:

- preserve the X explanation behavior;
- additionally insert:
  member.key -> cause
  into self.refusals;
- then return the same conflict.

Do not fall through to the fact-classification branch.
Do not apply the fact table-full check.
Do not change already-known conflict handling.

Named property:
E7

Configuration:
F-G
```

**Baseline anchor** (exactly once in C's blob, at line 1211):

```rust
        if gating {
            if self.x.is_none() {
                self.explanation(member.key, cause);
            }
            return Verdict::Refused(conflict, Some(at));
        }
```

**Replacement:**

```rust
        if gating {
            if self.x.is_none() {
                self.explanation(member.key, cause);
            }
            self.refusals.insert(member.key, cause);
            return Verdict::Refused(conflict, Some(at));
        }
```

**Identities.**

- patch `catalog/patches/C18b.patch`: sha256 `830f21c88f5ba82798baa9126d67d7f37fdcc4aaae71a52d4ed023c009713ec2`; lines removed/added 0/1
- mutant `protocol.rs`: git blob `c7f2b5ea99bb1676e3844b6c612d3393f97d50da`, sha256 `25805e5b0bfa3bc3c611a4d6a407461ab1e1d1d7d1a680d65b3d5f6575a1db1a`
- restoration: reverse application returns blob `a2df2841b99cad5d452008a6d4150e9369d07e66` (verified by both checkers)

**Preserved behaviour.**

- X's explanation is still built when X is empty
- the same conflict is returned
- no fall-through to the fact-classification branch; no table-full check or latch is applied
- already-known conflict handling (row b) unchanged
- the gating correction test at the top of classify unchanged

**Why this is one named change.** Only the gating new-cause branch adds one table insertion (member.key -> cause) after the X explanation and before its return.

**Static validation.** checker A: PASS; checker B: PASS; syntax: SYNTAX_CHECKED_NOT_TYPECHECKED.

**Remaining execution-dependent uncertainty.**

- Whether F-G's original explorer reaches an E7 failure within 30,000,000 states is decided only by execution.

## C20 — an explanation found already present is not marked committed (section 6.14)

*ARCHITECT-SPECIFIED — EXECUTOR-ENCODED — INDEPENDENT ARCHITECT REVIEW PENDING*

**R2.** Unchanged from R1: the catalog entry equals R1's and the patch file is byte-identical.

**Adopted contract description** (contract 7.5, line 718):

```
| C20 | an explanation found already present is not marked committed | L1 | F-2 |
```

**Named property / test / configuration.** L1 on **F-2**: `ledger::protocol::model_tests::full_f2` (ignored at C: True), the original explorer, ceiling 30,000,000 states. Printed property must be: L1.

**Receipt type.** (a) original-explorer mutant FAIL; restored (e1). Restored side: e1: docs/evidence/p3-candidate11/final-baseline-known-runs-01/receipts/41-full-F-2 at f023cc3d031a2a89e5ecbd4086e15f2e50869acd.

**Special rules.** (1) L1 is evaluated only after a complete search (AD-1-R4 Q).

**Exact source item.** `crates/nexus-governed-control/src/ledger/protocol.rs` (blob `a2df2841b99cad5d452008a6d4150e9369d07e66`): Publish::cell; anchor line(s) at C: 1893.

**Architect encoding — mission R1 section 6.14 C20 — present X occupant is not marked committed** (verbatim as received):

```
File:
protocol.rs

Item:
Publish::cell

Scope:

io.present(&occupant) -> Read::Got(Some(seq))

Mutation:

- preserve rollback;
- if cell == C:
  keep the healthy self.settle(Cell::C, seq);
- if cell == X:
  do NOT call settle;
  do NOT mark Stage::Committed;
  do NOT remove the original's refusal-table entry;
- return CellAppend::Committed(seq) for both cells.

The inserted-and-Durable path remains healthy.

Named property:
L1

Configuration:
F-2
```

**Baseline anchor** (exactly once in C's blob, at line 1893):

```rust
            Read::Got(Some(seq)) => {
                self.rolled(io.rollback());
                self.settle(cell, seq);
                return CellAppend::Committed(seq);
```

**Replacement:**

```rust
            Read::Got(Some(seq)) => {
                self.rolled(io.rollback());
                if cell == Cell::C {
                    self.settle(cell, seq);
                }
                return CellAppend::Committed(seq);
```

**Identities.**

- patch `catalog/patches/C20.patch`: sha256 `5ea1889e67bc552d08c75074f3717d87a4509c56d13448d61bc574fc3d93f62f`; lines removed/added 1/3
- mutant `protocol.rs`: git blob `7bb5aa2abbb52249efe76ab22dd921cb74dd337c`, sha256 `0e521df8ae9dfc69114085daafaa96f8b55f2119e6a3c51afa142af8acc5caaf`
- restoration: reverse application returns blob `a2df2841b99cad5d452008a6d4150e9369d07e66` (verified by both checkers)

**Preserved behaviour.**

- the rollback
- C's present path: settle (C emptied) unchanged
- the return CellAppend::Committed(seq) for both cells
- the inserted-and-Durable path (`Commit::Durable => { self.settle(cell, seq); ... }`) unchanged
- Publish::settle itself unchanged

**Why this is one named change.** Only the present-occupant arm changes: settle is called only for C, so a present X occupant is neither marked Committed nor has the original's table entry removed.

**Static validation.** checker A: PASS; checker B: PASS; syntax: SYNTAX_CHECKED_NOT_TYPECHECKED.

**Remaining execution-dependent uncertainty.**

- L1 is evaluated only after a complete search (AD-1-R4 Q); whether F-2's original explorer completes and reports L1 within 30,000,000 states is decided only by execution.

## C23 — the publication omits the X bit (section 6.15)

*ARCHITECT-SPECIFIED — EXECUTOR-ENCODED — INDEPENDENT ARCHITECT REVIEW PENDING*

**R2.** Unchanged from R1: the catalog entry equals R1's and the patch file is byte-identical.

**Adopted contract description** (contract 7.5, line 719):

```
| C23 | the publication omits the X bit | E1 | F-2 |
```

**Named property / test / configuration.** E1 on **F-2**: `ledger::protocol::model_tests::full_f2` (ignored at C: True), the original explorer, ceiling 30,000,000 states. Printed property must be: E1.

**Receipt type.** (a) original-explorer mutant FAIL; restored (e1). Restored side: e1: docs/evidence/p3-candidate11/final-baseline-known-runs-01/receipts/41-full-F-2 at f023cc3d031a2a89e5ecbd4086e15f2e50869acd.

**Exact source item.** `crates/nexus-governed-control/src/ledger/protocol.rs` (blob `a2df2841b99cad5d452008a6d4150e9369d07e66`): Core::flags; anchor line(s) at C: 1085.

**Architect encoding — mission R1 section 6.15 C23 — X publication bit omitted** (verbatim as received):

```
File:
protocol.rs

Item:
Core::flags

Mutation:

Remove only:

bit(self.x.is_some(), X_OCCUPIED)

from the returned flags expression.

Do not remove the C or U bits.
Do not change expected_flags in the model.
Do not change the word layout constants.

Named property:
E1

Configuration:
F-2
```

**Baseline anchor** (exactly once in C's blob, at line 1085):

```rust
            | bit(self.x.is_some(), X_OCCUPIED)
```

**Replacement:**

*(empty: the anchor is removed)*

**Identities.**

- patch `catalog/patches/C23.patch`: sha256 `731ffbedf845790c681b1fe7a2dc20d53174c8ce7378de04e5963865f0bc04aa`; lines removed/added 1/0
- mutant `protocol.rs`: git blob `cd50b40ce632d4edcb95fff89389246b0d75c51e`, sha256 `b866fa5aefa95dba779ae7d567d304232251d9ae6e4145abfc850553bea49afa`
- restoration: reverse application returns blob `a2df2841b99cad5d452008a6d4150e9369d07e66` (verified by both checkers)

**Preserved behaviour.**

- the phase and session terms; the LATCHED, U_OCCUPIED and C_OCCUPIED bits
- the model's expected_flags unchanged
- the word layout constants unchanged (X_OCCUPIED keeps its use in HealthView::x_occupied)

**Why this is one named change.** Only the X bit term is removed from the returned flags expression.

**Static validation.** checker A: PASS; checker B: PASS; syntax: SYNTAX_CHECKED_NOT_TYPECHECKED.

**Remaining execution-dependent uncertainty.**

- Whether F-2's original explorer reaches an E1 failure within 30,000,000 states is decided only by execution.

## C26 — a passed barrier clears the latch (section 6.16)

*ARCHITECT-SPECIFIED — EXECUTOR-ENCODED — INDEPENDENT ARCHITECT REVIEW PENDING*

**R2.** Unchanged from R1: the catalog entry equals R1's and the patch file is byte-identical.

**Adopted contract description** (contract 7.5, line 722):

```
| C26 | a passed barrier clears the latch | E9 | F-4 |
```

**Named property / test / configuration.** E9 on **F-4**: `ledger::protocol::model_tests::full_f4` (ignored at C: True), the original explorer, ceiling 30,000,000 states. Printed property must be: E9.

**Receipt type.** (a) original-explorer mutant FAIL; restored (e1). Restored side: e1: docs/evidence/p3-candidate11/final-baseline-remaining-configs-01/receipts/62-full-F-4 at 89326b859792a442c1853c3d35d30a48301c009e.

**Exact source item.** `crates/nexus-governed-control/src/ledger/protocol.rs` (blob `a2df2841b99cad5d452008a6d4150e9369d07e66`): Publish::barrier; anchor line(s) at C: 1720.

**Architect encoding — mission R1 section 6.16 C26 — successful barrier clears latch** (verbatim as received):

```
File:
protocol.rs

Item:
Publish::barrier

Mutation:

After io.run() returns true and before assigning the resulting phase:

self.core.latch = None;

Then execute the existing healthy phase assignment and return Passed.

Do not clear the latch on:

- OtherLedger;
- NotNeeded;
- failed barrier.

Named property:
E9

Configuration:
F-4
```

**Baseline anchor** (exactly once in C's blob, at line 1720):

```rust
        if !io.run() {
            return BarrierOutcome::Failed;
        }
```

**Replacement:**

```rust
        if !io.run() {
            return BarrierOutcome::Failed;
        }
        self.core.latch = None;
```

**Identities.**

- patch `catalog/patches/C26.patch`: sha256 `1e26422d79a19553d925585aeb580f8a0c684bcc78d8a922ab5ab9a3756e8b4d`; lines removed/added 0/1
- mutant `protocol.rs`: git blob `88f1783305f6418129671e77769fd4caf53e2a88`, sha256 `c9cccd06367aced8a183493cde03e4c9a62ece34c171fb7a95ed094173ae7b49`
- restoration: reverse application returns blob `a2df2841b99cad5d452008a6d4150e9369d07e66` (verified by both checkers)

**Preserved behaviour.**

- OtherLedger, NotNeeded and the failed barrier return before the clearing
- the healthy phase assignment and `BarrierOutcome::Passed` unchanged

**Why this is one named change.** One statement, `self.core.latch = None;`, is inserted after io.run() returns true and before the phase assignment.

**Static validation.** checker A: PASS; checker B: PASS; syntax: SYNTAX_CHECKED_NOT_TYPECHECKED.

**Remaining execution-dependent uncertainty.**

- Whether F-4's original explorer reaches an E9 failure within 30,000,000 states is decided only by execution.

## A1 — the writer confirms before it records (section 6.17)

*ARCHITECT-SPECIFIED — EXECUTOR-ENCODED — INDEPENDENT ARCHITECT REVIEW PENDING*

**R2.** Unchanged from R1: the catalog entry equals R1's and the patch file is byte-identical.

**Adopted contract description** (contract 7.5, line 739):

```
| A1 | the writer confirms before it records | P1 | F-1 |
```

**Named property / test / configuration.** P1 on **F-1**: `ledger::protocol::model_tests::full_f1` (ignored at C: True), the original explorer, ceiling 60,000,000 states. Printed property must be: P1.

**Receipt type.** (a) original-explorer mutant FAIL; restored (e1). Restored side: e1: docs/evidence/p3-candidate11/final-baseline-known-runs-01/receipts/45-full-F-1 at f023cc3d031a2a89e5ecbd4086e15f2e50869acd.

**Exact source item.** `crates/nexus-governed-control/src/ledger/protocol/model_tests.rs` (blob `28085ce232827301104b5d757af042ec933976f8`): step_fw; anchor line(s) at C: 2563.

**Architect encoding — mission R1 section 6.17 A1 — first writer confirms before recording** (verbatim as received):

```
File:
model_tests.rs

Item:
step_fw

Scope:
first-writer actor only.

Do not modify settlement.

Use the existing FwPos::Got(Held) state and ticket presence to create
two separate actor steps without adding an enum variant.

Exact behavior:

A. FwPos::Got(held), ticket is Some:

- retain the healthy Oracle option structure, including DROP where
  applicable;
- normal option 0:
  - take and confirm the ticket;
  - leave position as FwPos::Got(held);
  - do NOT record the class in this step;
- unwind/drop choices retain their healthy meanings.

B. FwPos::Got(held), ticket is None:

- this is the post-confirm recording step;
- record held exactly as healthy FwPos::Got option 0 records it:
  - Committed -> Paid;
  - Counted -> record_counted;
  - Rejected -> Uncertain and clear the unrecorded bit as healthy;
- set position Done;
- consume no Oracle choice;
- perform no ticket operation.

The healthy FwPos::Recorded branch may remain in the enum/source but
must be unreachable from the mutant's normal option-0 path.

Do not reorder settlement's record/confirm path.

Named property:
P1

Configuration:
F-1
```

**Baseline anchor** (exactly once in C's blob, at line 2563):

```rust
        FwPos::Got(held) => {
            let counted_ticket = matches!(held, Held::Counted(_));
            let options: &[bool] = if config.drop && counted_ticket {
                &[false, true, false]
            } else {
                &[false, true]
            };
            match o.choose(options) {
                0 => {
                    match held {
                        Held::Committed => {
                            for &f in &members {
                                set_class(&mut w.table, f, Class::Paid);
                            }
                        }
                        Held::Counted(counted) => record_counted(w, &members, counted),
                        Held::Rejected => {
                            for &f in &members {
                                set_class(&mut w.table, f, Class::Uncertain);
                                w.ghost.unrecorded &= !bit(f);
                            }
                        }
                    }
                    w.fw[i].pos = FwPos::Recorded;
                    obs.note(|| format!("[T] records {held:?}"));
                }
```

**Replacement:**

```rust
        FwPos::Got(held) if w.fw[i].ticket.is_none() => {
            match held {
                Held::Committed => {
                    for &f in &members {
                        set_class(&mut w.table, f, Class::Paid);
                    }
                }
                Held::Counted(counted) => record_counted(w, &members, counted),
                Held::Rejected => {
                    for &f in &members {
                        set_class(&mut w.table, f, Class::Uncertain);
                        w.ghost.unrecorded &= !bit(f);
                    }
                }
            }
            w.fw[i].pos = FwPos::Done;
            obs.note(|| format!("[T] records {held:?}"));
        }
        FwPos::Got(held) => {
            let counted_ticket = matches!(held, Held::Counted(_));
            let options: &[bool] = if config.drop && counted_ticket {
                &[false, true, false]
            } else {
                &[false, true]
            };
            match o.choose(options) {
                0 => {
                    obs.ticket_steps += 1;
                    w.fw[i]
                        .ticket
                        .take()
                        .expect("a writer that has its result holds its ticket")
                        .confirm();
                    obs.note(|| "confirm".into());
                }
```

**Identities.**

- patch `catalog/patches/A1.patch`: sha256 `e3a21c74babd86e1c40c58457319dc6901c7fac7a6e56d7c8f149e3389f72507`; lines removed/added 16/25
- mutant `model_tests.rs`: git blob `26c09e7ff93633095de8d7a15ee81f5f46728336`, sha256 `ea3ac997cd4a66925613346707f305ef3d8b68c0871e581110a9c13e9e4dd632`
- restoration: reverse application returns blob `28085ce232827301104b5d757af042ec933976f8` (verified by both checkers)

**Preserved behaviour.**

- settlement (step_se) unchanged, including its own record-then-confirm
- the Oracle option structure of FwPos::Got with a ticket ([false, true] or, with DROP, [false, true, false])
- option 1 (unwind before recording -> Dropping) and the DROP choice unchanged
- the recording rules (Committed -> Paid; Counted -> record_counted; Rejected -> Uncertain and the unrecorded bit cleared) carried verbatim into the post-confirm step
- FwPos::Recorded stays in the enum and in its healthy branch, unreachable from option 0
- no new enum variant; the two steps are told apart by ticket presence, which is in the state key
- option 0's confirm keeps the healthy confirm's own accounting (obs.ticket_steps += 1, as in FwPos::Recorded), so E2 still accounts for the ticket's exchange

**Why this is one named change.** Only the first writer's FwPos::Got step changes: option 0 takes and confirms the ticket and leaves the position Got(held); a new guarded arm (Got with no ticket) records exactly as healthy option 0 did and moves to Done, consuming no Oracle choice.

**Static validation.** checker A: PASS; checker B: PASS; syntax: SYNTAX_CHECKED_NOT_TYPECHECKED.

**Remaining execution-dependent uncertainty.**

- No healthy path reaches FwPos::Got without a ticket (the Armed step always puts the ticket back), so the new guarded arm is reached only through the mutant's option 0. This is a static reading; it is not executed.
- The guarded arm borrows w.fw[i].ticket in the match guard while matching the Copy position; this is believed to type-check (the healthy arm already mutates w.fw[i] in its body) but is SYNTAX_CHECKED_NOT_TYPECHECKED.
- Whether F-1's original explorer reaches a P1 failure within 60,000,000 states is decided only by execution.

## A2 — admission is split into two steps under two separate holds of the test's lock. Step one takes a `view` and keeps only its low 16 bits: phase, session, latch, cells, handover fault. Any actor may step in between. Step two takes a second `view`, keeps only `unapplied`, reads the debt count, H and A, and decides from the kept bits of step one and the values of step two. (section 6.18)

*ARCHITECT-SPECIFIED — EXECUTOR-ENCODED — INDEPENDENT ARCHITECT REVIEW PENDING*

**R2.** Unchanged from R1: the catalog entry equals R1's and the patch file is byte-identical.

**Adopted contract description** (contract 7.5, line 740):

```
| A2 | admission is split into two steps under two separate holds of the test's lock. Step one takes a `view` and keeps only its low 16 bits: phase, session, latch, cells, handover fault. Any actor may step in between. Step two takes a second `view`, keeps only `unapplied`, reads the debt count, H and A, and decides from the kept bits of step one and the values of step two. | P1 | F-1 |
```

**Named property / test / configuration.** P1 on **F-1**: `ledger::protocol::model_tests::full_f1` (ignored at C: True), the original explorer, ceiling 60,000,000 states. Printed property must be: P1.

**Receipt type.** (a) original-explorer mutant FAIL; restored (e1). Restored side: e1: docs/evidence/p3-candidate11/final-baseline-known-runs-01/receipts/45-full-F-1 at f023cc3d031a2a89e5ecbd4086e15f2e50869acd.

**Exact source item.** `crates/nexus-governed-control/src/ledger/protocol/model_tests.rs` (blob `28085ce232827301104b5d757af042ec933976f8`): step_adm; anchor line(s) at C: 3251.

**Architect encoding — mission R1 section 6.18 A2 — split-view admission** (verbatim as received):

```
File:
model_tests.rs

Item:
step_adm

Use the existing World.adm_kept field and its existing key encoding.

No new world field is allowed.

Step 1, when adm_kept is None:

1. If adm == 0, disabled as healthy.
2. Increment obs.views.
3. Take one health view.
4. Store exactly:
   view.word & FLAGS
   into adm_kept.
5. Do not decrement adm.
6. Emit no admission branch.
7. Run no admission property.
8. Return enabled.

Other actors may run before the next Adm step because adm_kept is in
the state key.

Step 2, when adm_kept is Some(kept):

1. Increment obs.views.
2. Take a second health view.
3. From the second view keep only the unapplied field.
4. Construct the decision word from:
   - low 16 bits = kept;
   - unapplied = second view's unapplied;
   - all other word fields zero.
5. Decide admission using:
   - the constructed split HealthView.admits();
   - current debt == 0;
   - current H <= current A.
6. Clear adm_kept.
7. Decrement adm exactly once.
8. Emit the healthy admission branch/note.
9. On admitted, run healthy admission_properties.

The second step must not use the second view's low 16 bits.
The first step must not retain live or unapplied.
The two loads remain separately counted by E2.

Named property:
P1

Configuration:
F-1
```

**Baseline anchor** (exactly once in C's blob, at line 3251):

```rust
    if w.adm == 0 {
        return false;
    }
    obs.views += 1;
    let view = w.health.view();
    let admitted = view.admits() && w.table.debt == 0 && w.table.h <= w.table.a;
    w.adm -= 1;
```

**Replacement:**

```rust
    let Some(kept) = w.adm_kept else {
        if w.adm == 0 {
            return false;
        }
        obs.views += 1;
        let first = w.health.view();
        w.adm_kept = Some(first.word & FLAGS);
        obs.note(|| "[T] admission step one: the low 16 bits kept".into());
        return true;
    };
    obs.views += 1;
    let second = w.health.view();
    let view = HealthView {
        word: kept | with_field(0, UNAPPLIED_SHIFT, field(second.word, UNAPPLIED_SHIFT)),
    };
    let admitted = view.admits() && w.table.debt == 0 && w.table.h <= w.table.a;
    w.adm_kept = None;
    w.adm -= 1;
```

**Identities.**

- patch `catalog/patches/A2.patch`: sha256 `5f1f30486da2457e2a50f0f72f1c96bfecf67e12692d47aa20d0ae3a1d677cf2`; lines removed/added 4/15
- mutant `model_tests.rs`: git blob `c0ac428f58497d8cf42458d51f86db0f2e4b84ba`, sha256 `855a6ff6e419e6b3e09425047b629b42708d493507194a97516326bf2d8b2633`
- restoration: reverse application returns blob `28085ce232827301104b5d757af042ec933976f8` (verified by both checkers)

**Preserved behaviour.**

- the existing World.adm_kept field and its key encoding; no new world field
- step one: disabled when adm == 0; one view (E2 counts it); keeps only view.word & FLAGS; adm not decremented; no admission branch; no admission property; enabled
- step two: one view (E2 counts it); only its unapplied field used; the decision word's low 16 bits are the kept bits and its other fields zero
- the decision line text, `view.admits() && w.table.debt == 0 && w.table.h <= w.table.a`, unchanged and read from the current table
- adm_kept cleared and adm decremented exactly once in step two
- the healthy admission branch, note and admission_properties unchanged after the decision

**Why this is one named change.** Only step_adm changes: it becomes the two-step split admission of section 6.18, keyed by the existing adm_kept field, with the healthy decision expression applied to the split HealthView.

**Static validation.** checker A: PASS; checker B: PASS; syntax: SYNTAX_CHECKED_NOT_TYPECHECKED.

**Remaining execution-dependent uncertainty.**

- Step one adds a trace note (obs.note) and no branch: notes affect only trace text, never a verdict or the branch set.
- Experimental quotient/symmetry code reads adm_kept (e.g. asserting it is None); those explorers are not used for A2's original-explorer run on F-1. Static reading only.
- Whether F-1's original explorer reaches a P1 failure within 60,000,000 states is decided only by execution.

## A3 — the worker empties X before it raises H (section 6.19)

*ARCHITECT-SPECIFIED — EXECUTOR-ENCODED — INDEPENDENT ARCHITECT REVIEW PENDING*

**R2.** Unchanged from R1: the catalog entry equals R1's and the patch file is byte-identical.

**Adopted contract description** (contract 7.5, line 741):

```
| A3 | the worker empties X before it raises H | P12 | F-5 |
```

**Named property / test / configuration.** P12 on **F-5**: `ledger::protocol::model_tests::full_f5` (ignored at C: True), the original explorer, ceiling 30,000,000 states. Printed property must be: P12.

**Receipt type.** (a) original-explorer mutant FAIL; restored (e1). Restored side: e1: docs/evidence/p3-candidate11/final-baseline-remaining-configs-01/receipts/65-full-F-5 at 89326b859792a442c1853c3d35d30a48301c009e.

**Exact source item.** `crates/nexus-governed-control/src/ledger/protocol/model_tests.rs` (blob `28085ce232827301104b5d757af042ec933976f8`): step_wk; anchor line(s) at C: 2988, 3002.

**Architect encoding — mission R1 section 6.19 A3 — EmptyX before H** (verbatim as received):

```
File:
model_tests.rs

Items:
step_wk and existing WkPos::RaiseH

Do not add a new enum variant.

Mutation:

1. In WkPos::Seen:
   - retain the A update from the agenda frontier;
   - for Action::EmptyX, do NOT raise H;
   - proceed to WkPos::Act(action) as for another real action.

2. In WkPos::Act:
   - run worker_op first;
   - retain the healthy replay-history save where applicable;
   - if action is EmptyX(_, seq), set position WkPos::RaiseH(seq);
   - otherwise set position Idle.

3. In WkPos::RaiseH:
   retain the existing behavior:
   - H = max(H, seq);
   - position Idle.

The intermediate state after EmptyX and before RaiseH is the named
fault.

Named property:
P12

Configuration:
F-5
```

**Baseline anchor 1 of 2** (exactly once in C's blob, at line 2988):

```rust
            w.table.a = w.table.a.max(frontier);
            if let Action::EmptyX(_, seq) = &action {
                w.table.h = w.table.h.max(*seq);
            }
```

**Replacement 1 of 2:**

```rust
            w.table.a = w.table.a.max(frontier);
```

**Baseline anchor 2 of 2** (exactly once in C's blob, at line 3002):

```rust
        WkPos::Act(action) => {
            worker_op(config, w, &action, o, obs);
            if config.replays > 0 && !matches!(action, Action::Barrier | Action::Nothing) {
                w.wk.last = Some(action);
            }
            w.wk.pos = WkPos::Idle;
        }
```

**Replacement 2 of 2:**

```rust
        WkPos::Act(action) => {
            worker_op(config, w, &action, o, obs);
            let next = match &action {
                Action::EmptyX(_, seq) => WkPos::RaiseH(*seq),
                _ => WkPos::Idle,
            };
            if config.replays > 0 && !matches!(action, Action::Barrier | Action::Nothing) {
                w.wk.last = Some(action);
            }
            w.wk.pos = next;
        }
```

**Identities.**

- patch `catalog/patches/A3.patch`: sha256 `ad0c64489e962756316b1a29b5d289a721517d1115525ee011e52db69cf2cc00`; lines removed/added 4/5
- mutant `model_tests.rs`: git blob `72bab79d71687a1c056afe18ab7a938edc20f4a9`, sha256 `5eff1f8c581c2de4ab54470a2670057b6139c83f6bdc56326ff96f6a3d8c9ab9`
- restoration: reverse application returns blob `28085ce232827301104b5d757af042ec933976f8` (verified by both checkers)

**Preserved behaviour.**

- WkPos::Seen keeps the A update from the agenda frontier and proceeds to Act(action) for EmptyX as for another real action
- worker_op runs first in WkPos::Act; the replay-history save is unchanged
- WkPos::RaiseH's existing behaviour (H = max(H, seq); Idle) unchanged
- no new enum variant
- the Seen note text is unchanged (it is trace text only)

**Why this is one named change.** Only step_wk changes: H is no longer raised in Seen for EmptyX; after the worker's EmptyX operation the worker moves to the existing WkPos::RaiseH(seq), which raises H in a later step. The intermediate state is the named fault.

**Static validation.** checker A: PASS; checker B: PASS; syntax: SYNTAX_CHECKED_NOT_TYPECHECKED.

**Remaining execution-dependent uncertainty.**

- Whether F-5's original explorer reaches a P12 failure within 30,000,000 states is decided only by execution.

## A5 — settlement claims a Conflict obligation while the store is not Healthy (section 6.20)

*ARCHITECT-SPECIFIED — EXECUTOR-ENCODED — INDEPENDENT ARCHITECT REVIEW PENDING*

**R2.** Unchanged from R1: the catalog entry equals R1's and the patch file is byte-identical.

**Adopted contract description** (contract 7.5, line 743):

```
| A5 | settlement claims a Conflict obligation while the store is not Healthy | L1 | F-2 |
```

**Named property / test / configuration.** L1 on **F-2**: `ledger::protocol::model_tests::full_f2` (ignored at C: True), the original explorer, ceiling 30,000,000 states. Printed property must be: L1.

**Receipt type.** (a) original-explorer mutant FAIL; restored (e1). Restored side: e1: docs/evidence/p3-candidate11/final-baseline-known-runs-01/receipts/41-full-F-2 at f023cc3d031a2a89e5ecbd4086e15f2e50869acd.

**Special rules.** (1) L1 is evaluated only after a complete search (AD-1-R4 Q).

**Exact source item.** `crates/nexus-governed-control/src/ledger/protocol/model_tests.rs` (blob `28085ce232827301104b5d757af042ec933976f8`): claimable; anchor line(s) at C: 2648.

**Architect encoding — mission R1 section 6.20 A5 — Conflict claimed without Healthy** (verbatim as received):

```
File:
model_tests.rs

Item:
claimable

Scope:
Class::Conflict arm only.

Healthy condition:

view.phase() == Phase::Healthy && !view.x_occupied()

Mutation:

Remove only the Healthy-phase conjunct.

Mutant condition:

!view.x_occupied()

Preserve:

- handover-fault and latch rejection before the match;
- Refused behavior;
- Uncertain behavior;
- X-empty requirement;
- claim type Explain.

Named property:
L1

Configuration:
F-2
```

**Baseline anchor** (exactly once in C's blob, at line 2648):

```rust
        Class::Conflict if view.phase() == Phase::Healthy && !view.x_occupied() => {
            Some(Claim::Explain)
        }
```

**Replacement:**

```rust
        Class::Conflict if !view.x_occupied() => Some(Claim::Explain),
```

**Identities.**

- patch `catalog/patches/A5.patch`: sha256 `913eba6d31d7cbe6d65f4543b9fb2d0def6ca7e7586b26209b3ee5063a2f4d32`; lines removed/added 3/1
- mutant `model_tests.rs`: git blob `cb08f5e8c62b3ce267f0688cc880261a28cc29a2`, sha256 `8d6f100133cced95076d584acc8a861c6da39eb234fc8a570775811b3e97ec5f`
- restoration: reverse application returns blob `28085ce232827301104b5d757af042ec933976f8` (verified by both checkers)

**Preserved behaviour.**

- the handover-fault and latch rejection before the match
- the Refused arm (Healthy -> Append) and the Uncertain arm (not NeedBarrier -> Lookup)
- the X-empty requirement of the Conflict arm
- the claim type Explain

**Why this is one named change.** Only the Healthy-phase conjunct of the Conflict arm's guard is removed; rustfmt then requires the arm's single-expression block to be written on one line.

**Static validation.** checker A: PASS; checker B: PASS; syntax: SYNTAX_CHECKED_NOT_TYPECHECKED.

**Remaining execution-dependent uncertainty.**

- L1 is evaluated only after a complete search (AD-1-R4 Q); whether F-2's original explorer completes and reports L1 within 30,000,000 states is decided only by execution.

## A6 — "recovery required" is a flag, not a comparison of H with A (section 6.21)

*ARCHITECT-SPECIFIED — EXECUTOR-ENCODED — INDEPENDENT ARCHITECT REVIEW PENDING*

**R2.** Changed from R1 only by the KEY_FIELDS receipt-metadata hunk (see *The R2 correction: A6* above); every R1 edit and hunk is kept byte for byte.

**Adopted contract description** (contract 7.5, line 744):

```
| A6 | "recovery required" is a flag, not a comparison of H with A | P12 or P15 | F-5 |
```

**Named property / test / configuration.** P12 or P15 on **F-5**: `ledger::protocol::model_tests::full_f5` (ignored at C: True), the original explorer, ceiling 30,000,000 states. Printed property must be: either of P12 / P15.

**Receipt type.** (a) original-explorer mutant FAIL; restored (e1). Restored side: e1: docs/evidence/p3-candidate11/final-baseline-remaining-configs-01/receipts/65-full-F-5 at 89326b859792a442c1853c3d35d30a48301c009e.

**Special rules.** (1) AD-1-R4 F.1(a): either property the row names.

**Exact source item.** `crates/nexus-governed-control/src/ledger/protocol/model_tests.rs` (blob `28085ce232827301104b5d757af042ec933976f8`): Table; initial; encode; decode; step_wk (Seen, RaiseH); step_ack (Applying); step_adm; receipt metadata: KEY_FIELDS; anchor line(s) at C: 915, 1153, 1706, 2131, 2988, 3009, 3240, 3256, 1172.

**Architect encoding — mission R1 section 6.21 A6 — recovery-required event flag** (verbatim as received):

```
File:
model_tests.rs

This is one actor/model mutation.

Add to Table:

recovery_required: bool

Preserve numeric H and A unchanged for property evaluation and
reporting.

Initial value:

false

State identity:

- encode the boolean after A;
- decode it at the matching location;
- include it in every Table construction required for compilation;
- do not omit it from the state key.

Flag update rules:

A. Whenever H actually increases:

- assign the new H;
- set recovery_required = true.

This applies to:
- the healthy EmptyX H update in WkPos::Seen;
- WkPos::RaiseH.

B. Whenever A actually increases:

- assign the new A;
- set recovery_required = false.

This applies to:
- the worker's agenda-frontier update in WkPos::Seen;
- acknowledgement AckPos::Applying.

Do not clear the flag when A is assigned the same or a smaller value.
Do not compare A with H when setting or clearing the flag.

Admission mutation:

Replace:

w.table.h <= w.table.a

with:

!w.table.recovery_required

All other admission conditions remain healthy.

Properties P12/P15 must continue reading the real ledger and numeric
frontiers, not the flag.

No production source is changed.

Named properties:
P12 or P15

Configuration:
F-5
```

**Baseline anchor 1 of 9** (exactly once in C's blob, at line 915):

```rust
    /// The layer's copy of the acknowledgement frontier.
    a: Seq,
}
```

**Replacement 1 of 9:**

```rust
    /// The layer's copy of the acknowledgement frontier.
    a: Seq,
    recovery_required: bool,
}
```

**Baseline anchor 2 of 9** (exactly once in C's blob, at line 1153):

```rust
            h: 0,
            a: 0,
        },
```

**Replacement 2 of 9:**

```rust
            h: 0,
            a: 0,
            recovery_required: false,
        },
```

**Baseline anchor 3 of 9** (exactly once in C's blob, at line 1706):

```rust
    e.u(w.table.h);
    e.u(w.table.a);
```

**Replacement 3 of 9:**

```rust
    e.u(w.table.h);
    e.u(w.table.a);
    e.b(w.table.recovery_required);
```

**Baseline anchor 4 of 9** (exactly once in C's blob, at line 2131):

```rust
        h: d.u(),
        a: d.u(),
    };
```

**Replacement 4 of 9:**

```rust
        h: d.u(),
        a: d.u(),
        recovery_required: d.b(),
    };
```

**Baseline anchor 5 of 9** (exactly once in C's blob, at line 2988):

```rust
            w.table.a = w.table.a.max(frontier);
            if let Action::EmptyX(_, seq) = &action {
                w.table.h = w.table.h.max(*seq);
            }
```

**Replacement 5 of 9:**

```rust
            if frontier > w.table.a {
                w.table.a = frontier;
                w.table.recovery_required = false;
            }
            if let Action::EmptyX(_, seq) = &action {
                if *seq > w.table.h {
                    w.table.h = *seq;
                    w.table.recovery_required = true;
                }
            }
```

**Baseline anchor 6 of 9** (exactly once in C's blob, at line 3009):

```rust
        WkPos::RaiseH(seq) => {
            w.table.h = w.table.h.max(seq);
```

**Replacement 6 of 9:**

```rust
        WkPos::RaiseH(seq) => {
            if seq > w.table.h {
                w.table.h = seq;
                w.table.recovery_required = true;
            }
```

**Baseline anchor 7 of 9** (exactly once in C's blob, at line 3240):

```rust
        AckPos::Applying(n) => {
            w.table.a = w.table.a.max(n);
```

**Replacement 7 of 9:**

```rust
        AckPos::Applying(n) => {
            if n > w.table.a {
                w.table.a = n;
                w.table.recovery_required = false;
            }
```

**Baseline anchor 8 of 9** (exactly once in C's blob, at line 3256):

```rust
    let admitted = view.admits() && w.table.debt == 0 && w.table.h <= w.table.a;
```

**Replacement 8 of 9:**

```rust
    let admitted = view.admits() && w.table.debt == 0 && !w.table.recovery_required;
```

**Baseline anchor 9 of 9 — R2 receipt metadata (mission R2 section 4)** (exactly once in C's blob, at line 1172):

```rust
position and any result it holds but has not applied; the test's table (classes, debt counter, H, A); every \
```

**Replacement 9 of 9:**

```rust
position and any result it holds but has not applied; the test's table (classes, debt counter, H, A, recovery_required); every \
```

**Identities.**

- patch `catalog/patches/A6.patch`: sha256 `0700052eb3e9fb91f36ee6d535b02b923ff007f1061490ac7dd16dd98d0d0ee8`; lines removed/added 6/22
- mutant `model_tests.rs`: git blob `51e54aed7cc33ebc8348a74fe6c881c057336ee7`, sha256 `59c2f87666ad3272661497e30d7a4a841dcc5aafd90569307cc4fdbd69b5a4b4`
- restoration: reverse application returns blob `28085ce232827301104b5d757af042ec933976f8` (verified by both checkers)

**Preserved behaviour.**

- numeric H and A keep their values (assignment only on an actual increase, the same result as max)
- the flag is in the state key: encoded after A and decoded at the matching place
- every Table construction (initial, decode) carries the flag
- the flag is set only where H actually increases (WkPos::Seen EmptyX, WkPos::RaiseH) and cleared only where A actually increases (WkPos::Seen agenda frontier, AckPos::Applying); no comparison of A with H
- only step_adm's `w.table.h <= w.table.a` conjunct is replaced; every other admission condition is unchanged
- admission_properties (P12, P15) unchanged: they read the ledger rows and the durable frontier, not the flag
- the experimental negative-case admissions and the property code keep `h <= a` where they have it
- no production source is changed
- R2 KEY_FIELDS hunk: only the table's description changes, from 'the test's table (classes, debt counter, H, A);' to 'the test's table (classes, debt counter, H, A, recovery_required);'; every other state-key description, the field name, the key layout, the field's encoded position (after A), every property, transition, trace and branch label are unchanged; the three report sites that print KEY_FIELDS are unchanged

**Why this is one named change.** One actor/model mutation: 'recovery required' becomes an event flag kept in the test's table (set on an H increase, cleared on an A increase), and admission reads the flag instead of comparing H with A. Its eight edits are the field, its two constructions, its encoding and decoding, its two setting sites, its two clearing sites (one edit covers the Seen A and H updates) and the admission condition. R2 adds a ninth edit that is receipt metadata only: KEY_FIELDS, the list of state-key fields every exploration report prints, now names recovery_required in the table's description (mission R2 section 4). It is not a second semantic mutation: it changes no transition, property, trace, branch label, key layout or encoded position.

**Static validation.** checker A: PASS; checker B: PASS; syntax: SYNTAX_CHECKED_NOT_TYPECHECKED.

**Remaining execution-dependent uncertainty.**

- R2: KEY_FIELDS (the receipt's printed list of key fields) names recovery_required (mission R2 section 4); statically, every exploration report's `state key fields:` line would print 'the test's table (classes, debt counter, H, A, recovery_required);'. That a printed receipt shows it is confirmed only by execution.
- Experimental quotient code that relies on the key's byte layout after A is not used by A6's original-explorer run; static reading only.
- Whether F-5's original explorer reaches a P12 or P15 failure within 30,000,000 states is decided only by execution.

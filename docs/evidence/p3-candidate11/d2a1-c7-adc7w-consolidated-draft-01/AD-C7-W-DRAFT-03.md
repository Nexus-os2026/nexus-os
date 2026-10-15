# P3-C11-D2A1-CONTRACT-02-R1 — C7-ONLY ADDENDUM "AD-C7-W" — DRAFT-03

> ## DRAFT — UNADOPTED — NOT EXECUTION AUTHORITY
> This draft is not accepted, adopted, applied or in force. It authorises no source change, worktree, build, run, mutation, commit, publication, catalog edit, kill or integration, and it accepts no C7 kill. It consolidates Draft-01, Draft-02 and ERRATA-01, with the Primary Architect's mandatory AU-5 correction, and supersedes them only as a draft. Publishing it for review is not adopting it. C7 stays **MUTATION_CONTROL_BLOCKED**, and the campaign stays at **34/35 Architect-accepted**.

**Consolidation.** Consolidated by Claude Code (`claude-opus-5-5`) under P3-C11-DR01-D2A1-C7-W-CONSOLIDATE-DRAFT03-01. No new architecture is introduced. The inputs were each authenticated from their actual bytes:

| Input | Prepared under | sha256 |
|---|---|---|
| Draft-01 | P3-C11-DR01-D2A1-C7-W-AMENDMENT-DRAFT-01 | `e1bd654d3a4df5f97fef861d3c1852abc8e37188cfcd4231e75e770213e70a70` |
| Draft-02 | P3-C11-DR01-D2A1-C7-W-AMENDMENT-REPAIR-01 | `69ee393bc3c909d5d3f598bfdb174e0fbcd35a0ad6615b400262070529986a30` |
| ERRATA-01 | P3-C11-DR01-D2A1-C7-W-DRAFT02-ERRATA-01 | `d1abbc330138fed6bcf72438af8e4f0999ed93a5d9b13ba0e6d187624a079d66` |
| AU-5 correction | the Primary Architect's ruling in P3-C11-DR01-D2A1-C7-W-CONSOLIDATE-DRAFT03-01, section 4 | — (mission text) |

`RECONCILIATION.md`, beside this file, maps every source clause to its place here and lists every non-verbatim edit. Draft-01 and Draft-02 were prepared by Claude Code (`claude-opus-5-5`, effort High). The design follows Option W.

**Scope: C7 only.** Nothing here changes:
- G.3, G.4, G.5 or G.6;
- F for any other control;
- any accepted control's record, or A4;
- the bytes of the frozen catalog;
- any configuration, property, Oracle, Io or explorer.

**Provenance labels** (as AD-1-R4):
- **PROVEN FROM SOURCE** — read from the frozen blob named.
- **ARCHITECT-RULED** — stated by the Architect's ruling.
- **EXECUTOR-REPORTED** — an earlier receipt, not re-run here.
- **STATIC INFERENCE** — derived by reading source; unexecuted; not verified.
- **NOT ESTABLISHED** — shown by nothing.

## 0. Basis

The table below is carried from Draft-02 section 0. Its "Check" column records what the Draft-02 mission checked read-only. Two of its cells pointed at text outside this document; they are expanded from authenticated source. Section 0.1 carries Draft-01's basis table, which Draft-02 did not restate in full. Section 0.2 lists what this consolidation re-verified from git objects. Every other row is EXECUTOR-REPORTED by a drafting mission and was not re-checked here.

| Item | Identity | Check |
|---|---|---|
| Draft-01 | transcript extract, sha256 `e1bd654d…0a70` | recovered verbatim by the Draft-02 mission from transcript `bf1c6683-7eeb-4d8b-acdc-d3a4f60fbaa6.jsonl`; re-authenticated by this consolidation (RECONCILIATION section 1) |
| C-BASE | commit `d5964ee5326c2733177307db6f507328023d361a`, tree `51bac5fd335999cc995c0ab9f582a30244973ef0` | equal |
| `protocol.rs` | blob `a2df2841b99cad5d452008a6d4150e9369d07e66` | equal |
| `model_tests.rs` | blob `28085ce232827301104b5d757af042ec933976f8`; 310,612 bytes; 9,199 lines | equal |
| Build inputs at C | `rust-toolchain.toml` `726005b2` (1.94.0); `Cargo.lock` `1af53a60`; workspace `Cargo.toml` `2c558a44`; crate `Cargo.toml` `2543c745`; `.cargo/config.toml` `05a42b50` | read |
| Frozen catalog | blob `1918374263740d2f98640da00fc466737945975e`; sha256 `d6c05a7cdc71b3ca1812b5199b8529cbe6db0363ca11bacc26d1f21c2cd11a83`; equal at `c10681b3` and `452b3cb5` | equal |
| Catalog C7 fields read | `expected_primary_failure.printed_property_must_be: "P6"`; `witness_required: false`; `named_property: ["P6"]`; `receipt_type: "(a) original-explorer mutant FAIL; restored (e1)"`; `restoration` (reverse patch; restores `a2df2841`); `restored_receipt` = `44-full-F-PT`; `additional_comparison` (F-3, `full_f3`, `required_to_kill: false`); `test_line_at_C: 4220`; `ceiling_states: 30000000`; `time_limit_seconds: 2700` | read |
| Frozen C7 patch / mutant | patch sha256 `b5d8bc6c…efbc4`; mutant blob `0d4ab9929c0e15d910735a52b72a5f3855bef58a`, sha256 `6d2f2d49…46aa` | from the catalog |
| Historical C7 (`85-C7`, at `c10681b3`) | executable sha256 `b320977a41d78ad031c74cb0f3fc9fe8c3f89051bbd599cc9b42deebb303a7e5`, 8,984,592 bytes; `FAIL: E11: a table entry left in a step that did not see its explanation in the ledger`; states 1,071,165; 13-step trace; `347 filtered out` | read |
| E-C (accepted) | `…/nexus-os-p3-c11-final-baseline-known-runs-01/target/release/deps/nexus_governed_control-2faa0b9718d3e6a3`, sha256 `cde9f9d20f32f2b8a17b7dcc2f521462ab6c7a6292d5e2cbd1dd2ba2c3d0d30c`, 8,984,464 bytes | read |
| E-C listings (at `f023cc3d`) | `20-list-all`: 348 tests (raw-output blob `5cf0ee48…`, sha256 `fe512699…35a5`). `21-list-ignored`: 35 tests (blob `3d434312…`, sha256 `4b13105f…ae1e`). Both were taken under source freeze `EXACTLY_C` | read |
| `44-full-F-PT` | `gc_symmetry_fpt` on E-C; composed-quotient PASS; 29,751,473 quotient states; `347 filtered out`; "checks: 37 of 37 hold"; raw output sha256 `0863794e…19a8` | read |
| `43-full-F-3` | `gc_symmetry_f3` on E-C; PASS; raw output `6dfc796a…5562` | read (Draft-01): `final-baseline-known-runs-01/receipts/43-full-F-3` at `f023cc3d`; executable sha256 `cde9f9d2…`; raw output blob `872ef461…`; `CLASSIFICATION: PASS` (36 of 36) |
| AD-1-R4 | blob `35d5074c…`; sha256 `00ced6d535c78f34f9e92233b742126a7b19192bf3ac8348056be2665192b693`. Read: E.3, E.4, F (C7 without A4), D (diagnostic tests), N (including "The rule" on additions) | sha256 recomputed: equal |
| AD-1-R4 adoption record | `docs/evidence/p3-candidate11/ad1r4-adoption/OWNER-ADOPTION-2026-10-14.md` | read (its form is the template for 12.3) |
| Source counts at C | 348 `#[test]` attributes and 35 `#[ignore` attributes (34 in `model_tests.rs`, 1 in `authority/tests.rs`), across `crates/nexus-governed-control/src` | equal to the E-C listings |
| Name freshness | `git grep -w c7_witness d5964ee5` finds no match anywhere in C's tree | PROVEN |
| Unsafe policy | `lib.rs` line 99 `#![deny(unsafe_code)]`; no `unsafe` token in `protocol.rs` or `model_tests.rs` | PROVEN |
| Hashing and time | no `HashMap`, `HashSet` or `RandomState` in either file; `Instant` is used only for explorer time caps (3883, 5039, 6579) | PROVEN |
| Precedent | `28085ce2` is a byte-prefix extension of `5ad26ddc` (the blob at `42dd1bd1`), by 24,201 appended bytes: the C4 witness, commit `04e42a7d` | PROVEN (git objects) |

**0.1 Draft-01 basis table (carried verbatim).** Its "Check" column records what the Draft-01 mission checked read-only. In this subsection, "this draft", "here" and "the mission" mean Draft-01 and its mission.

| Item | Identity | Check |
|---|---|---|
| Candidate C | commit `d5964ee5326c2733177307db6f507328023d361a`, tree `51bac5fd335999cc995c0ab9f582a30244973ef0` | the commit's tree is equal |
| `protocol.rs` at C | `crates/nexus-governed-control/src/ledger/protocol.rs`, blob `a2df2841b99cad5d452008a6d4150e9369d07e66`, sha256 `a5454944368df70fa4ce1c7cc4191d283e0a54b371137338fb999e59960aa296` | equal |
| `model_tests.rs` at C | `…/ledger/protocol/model_tests.rs`, blob `28085ce232827301104b5d757af042ec933976f8`, sha256 `2e93834f795f50842d5b124612895c4a150946130d2c789dac816ac36f6b6972`, 310,612 bytes, 9,199 lines | equal |
| `unit_tests.rs` at C | blob `55dc66409eaccdaa93e8f0a56f2ccb14be81e114` | equal |
| Frozen catalog | blob `1918374263740d2f98640da00fc466737945975e` (sha256 `d6c05a7c…1a83`) at `docs/evidence/p3-candidate11/d2a1-control-definitions-r2-01/catalog/catalog.json`, in `c10681b3` and `452b3cb5` | equal in both commits |
| Frozen C7 patch | `catalog/patches/C7.patch`, blob `df0dd5846f9003da1c46642dac6673882085f661`, sha256 `b5d8bc6c9254e0ca2853fcf2f9f220f24782ac6cfa77efe7032365d76f0efbc4`: one line at `protocol.rs` 1560, `self.core.refusals.remove(key);` becomes `self.core.refusals.clear();` | sha256 recomputed: equal |
| Frozen C7 mutant blob | `0d4ab9929c0e15d910735a52b72a5f3855bef58a`, sha256 `6d2f2d49…46aa` (from the catalog) | absent from the local object store, as expected (mutants are never committed). Not recomputed here, because that would need a patch application. EXECUTOR-REPORTED: `85-C7` `APPLY: EXACTLY_C_PLUS_PATCH` |
| C7 historical evidence | `c10681b3c65dd377b5a13d08140427e8b34441d0`. `receipts/85-C7/04-run/raw-output.txt`: blob `6252e406…`, sha256 `db3bf1643cb5f1d367004400f5caa9d975ebeb2bf45aca2e8b64dae98691154d`. `85-C7/RESULT.txt`: `da50b23a…`, `11699374…`. `80-STOP-RECORD.md`: `de47d979…`, `7427652c…` | equal to the Architect ruling record |
| C6 accepted P6 receipt | same commit, `84-C6/04-run/raw-output.txt`: blob `03638538…`, sha256 `920e109f…` | equal to the ruling record |
| Latest accepted checkpoint | `452b3cb5c3d72fd1c7efaa75ed3838845d06a5cd` | commit present |
| Base contract | an authenticated evidence copy exists, at `c2c69e3a…`: `docs/evidence/p3-candidate11/ad1r2-review-packet-01/evidence/p3-c11-dr01-d2a1-ad1r1-mut-readiness-01/contract/P3-C11-D2A1-CONTRACT-02-R1.md`, blob `a5ef321a6542b09cd7a487b4267348522a41775f`, sha256 `cac6535115fb333e68e6c3c045e444342c950ab7a735b7b7deea21cb2cb53b3c` | equal to the hash AD-1-R4 binds. Lines 654, 680, 693, 707, 748 and 853 read. Nothing was invented. |
| AD-1-R4 (adopted) | blob `35d5074c2daa2ce9602fee3cd23dc0ccd6aef5b1`, sha256 `00ced6d5…b693` | equal to its adoption record |
| AD-1-R5 (adopted) | adoption record read at `452b3cb5` | — |
| Accepted F-PT restored side | `final-baseline-known-runs-01/receipts/44-full-F-PT` at `f023cc3d…`: test `gc_symmetry_fpt`; executable sha256 `cde9f9d20f32f2b8a17b7dcc2f521462ab6c7a6292d5e2cbd1dd2ba2c3d0d30c`; raw output sha256 `0863794e…19a8`, blob `a92cb8da…`; `CLASSIFICATION: PASS` (37 of 37) | equal to the catalog's C7 `restored_receipt` |
| Accepted F-3 restored side | `…/43-full-F-3` at `f023cc3d`: test `gc_symmetry_f3`; executable sha256 `cde9f9d2…`; raw output sha256 `6dfc796a…5562`, blob `872ef461…`; `CLASSIFICATION: PASS` (36 of 36) | read |

**Rendering note (not an interpretation conflict).** The mission quotes the C7 row without markdown emphasis. Contract line 707 reads exactly:

`| C7 | *Explained* removes every table entry, **with** A4 | P6 | F-PT |`

The words are identical. This draft quotes the contract's bytes.

**0.2 Re-verified by this consolidation** (read-only git object reads, 2026-10-15; PROVEN FROM SOURCE)

| Item | What was read | Result |
|---|---|---|
| C-BASE | the tree of commit `d5964ee5326c2733177307db6f507328023d361a` | `51bac5fd335999cc995c0ab9f582a30244973ef0`: equal |
| Source blobs at C | `protocol.rs`, `model_tests.rs` and `unit_tests.rs` at `d5964ee5` | `a2df2841…`, `28085ce2…`, `55dc6640…`: equal |
| Catalog (K1) | the K1 path at `c10681b3` and at `452b3cb5` | blob `1918374263740d2f98640da00fc466737945975e` in both; sha256 of its bytes `d6c05a7cdc71b3ca1812b5199b8529cbe6db0363ca11bacc26d1f21c2cd11a83`: equal |
| Frozen C7 patch | `catalog/patches/C7.patch` at `452b3cb5` | blob `df0dd584…`, sha256 `b5d8bc6c…efbc4`: equal |
| P-1 existence conditions | the catalog's C7 entry | the three `set` keys exist (`expected_primary_failure.printed_property_must_be` is `"P6"`, `witness_required` is `false`, and `receipt_type`); the three `add` keys are absent; `special_attribution_rules` is an existing list of 2 items: as P-1 requires |
| S-C7-18 quotations | the C7 entry's `execution_dependent_uncertainty` | items [0] and [1] equal the text quoted in S-C7-18 |
| Name freshness (NI-4) | `git grep -w c7_witness d5964ee5` | no match |
| Table A (4.3) | Draft-01's and Draft-02's Table A | byte-identical |
| OVERLAY-C7 (2A.2) | Draft-02's overlay block | byte-identical (RECONCILIATION section 5) |

## 0A. The six identities

| Name | Definition | Identity | Relations required |
|---|---|---|---|
| **C-BASE** | Frozen Candidate C | commit `d5964ee5`, tree `51bac5fd`, plus the blobs in section 0 | never edited |
| **W-SRC** | C-BASE plus W-PATCH (the append-only C7 witness). It is committed as W-COMMIT, whose single parent is `d5964ee5` | W-COMMIT, W-TREE, W-BLOB (git id, sha256, size), W-PATCH sha256 | differs from C-BASE at exactly one path; **never equal to C-BASE**; never labelled `EXACTLY_C` |
| **W7-SRC** | W-SRC plus the unchanged frozen C7 patch, in a disposable worktree. Never committed and never staged | tracked-blob comparison | differs from W-SRC only at `protocol.rs` (= `0d4ab992`); differs from C-BASE at exactly two paths |
| **E-C** | The accepted Candidate C executable | sha256 `cde9f9d2…0d30c` | accepted; never rebuilt or rerun here |
| **E-W** | The one fresh build of W-SRC | sha256 of the pinned artifact | no equality with any executable is claimed |
| **E-W7** | The one fresh build of W7-SRC | sha256 of the pinned artifact | no equality with any executable is claimed |

**Stale-artifact checks (required).** These checks prove only that the artifacts differ. They are never evidence of correctness.
- E-W ≠ E-W7.
- E-W ≠ E-C.
- E-W7 ≠ `b320977a…`.

## 1. C7 technical finding and source argument

**1.1 Contract text (verbatim, blob `a5ef321a`)**

- Line 654: `| P6 | A fact ever refused as a conflict is never Paid. |`
- Line 680: `| E11 | The refusal table never exceeds its limit; a retained cause is never changed; an entry leaves only in a step in which the store saw that original's explanation in the ledger. |`
- Line 693: `Each control is one named change, run **on the configuration named**. It must make its property fail by assertion. A build failure or a fixture failure kills nothing, with one stated exception (C22). **If a control is not killed on its configuration, the mission reports that and stops. It does not pick another configuration.**`
- Line 707: the C7 row quoted above.

The healthy source, `protocol.rs` 1558–1563 (PROVEN FROM SOURCE):

```text
        match io.explained(key) {
            Read::Got(Some(seq)) => {
                self.core.refusals.remove(key);
                return Explain::Explained(seq);
            }
            Read::Got(None) => {}
```

C7 changes line 1560 only, to `self.core.refusals.clear();`.

**1.2 Source facts (PROVEN FROM SOURCE, at C)**

- **SF-1. F-PT's world.** F-PT is F-3 with `blanket_uncertain` (`model_tests.rs` 170–178, 217–221): two conflicting facts, ENV on, 2 faults, 2 admissions, 2 gating callers. The initial ledger is `[Prepared(1), Holder(1)]` (`initial_rows`, 1085–1100). So:
  - F1 (index 0) can only conflict as MissingPrepared, and ENV supplies its Prepared record once.
  - F2 (index 1) can only conflict as DuplicateRole.
- **SF-2.** A new fact conflict inserts a refusal-table entry (`protocol.rs` 1221). It also places that original's explanation in X, but only if X is empty (1222–1224).
- **SF-3.** `Core::known` consults the table first (1161–1174). Classification row b refuses a member whose cause is already known (1197–1199).
- **SF-4.** `settle` removes X's original's entry when X's explanation is committed (1934–1946, line 1943).
- **SF-5.** `explain`'s `Got(Some)` arm removes the explained key only (1560). C7 clears every entry.
- **SF-6.** `lookup` answers `Absent` on an absent row before it consults the table (1503).
- **SF-7. E11 is a transition property** (`model_tests.rs` 3533–3541). An entry that leaves the table must be in `obs.explained`, and its explanation must be in the ledger after the step.
  - A settlement step pushes only the explained key into `obs.explained` (2695).
  - A worker's committed X append pushes X's original (2918).
- **SF-8. P6 is a state property:** class Paid while `ghost.conflicted` is set (in `state_properties`; the property string is at 3458).
- **SF-9. `explore` stops at the first violation** (3907–3946). A state violation ends the run when the state is expanded. A violating successor ends the run *before its target is inserted*.
- **SF-10. A4 and settlement claims.** Under A4 a drop path records a blanket Uncertain (`record_dropped`, 2375–2387). Settlement claims a fact in one of three ways (`claimable`, 2641–2654):
  - Uncertain: for Lookup;
  - Refused: for Append;
  - Conflict: for Explain, only when the store is Healthy and X is empty.
- **SF-11.** `successors` pushes a step's in-step violations and then `transition_properties`' violations into one list, `obs.violations` (3814–3836). `explore` reads the first entry of that list (3935). A violating successor is never inserted (3935–3941).
- **SF-12.** P6 is pushed only by `state_properties` (3456–3460). `state_properties` runs only on states: at 3812, 8292 (`violations_at`), and in the tests at 4511, 4947 and 7410. It never runs inside a step.
- **SF-13.** Line 1560 is reachable only through `Core::explain` (998–1003).
  - In the model, `Core::explain` is called only by `settlement_read` for `Claim::Explain` (2677).
  - Each Se step makes at most one such read (2731–2736).
  - `obs.explained` is pushed only at 2695 (that read's key, on `Explained`) and at 2918 (X's original, in a worker step).
  - E6 can follow P6 (3461–3469) only if `conflicted_e6` is set. That bit is set only under `config.batch`, which is false for F-PT (2389–2397), or by a settlement lookup that answers Conflict (2690–2693).
- **SF-14.** `explore` has a wall-clock cap: `FULL_TIME` = 45 min (line 79, checked at 3911–3913). The cap can end a run INCOMPLETE. It never produces PASS or FAIL.

**1.3 What the original explorer did** (EXECUTOR-REPORTED; the block is ARCHITECT-RULED)

On the C7 mutant, `full_fpt` with A4 on printed `FAIL: E11` at 1,071,165 discovered states, with a 13-step trace:
- Fw(0)'s F1 is refused as MissingPrepared, and X takes F1's explanation.
- Fw(1)'s F2 is refused as DuplicateRole, so the table holds {F1, F2}.
- The worker commits X; `settle` removes F1's entry. The worker then empties X.
- Settlement explains F1. The mutant clears F2's entry while F2's explanation is absent, so E11 fails.

The detail strings and counts were re-read by the Draft-02 mission for section 0.

**1.4 Scoped first-failure masking: C7-induced P6 paths under frozen F-PT only**

*Status:* an executor static argument. It has not been reviewed and is not kill evidence.

*Scope:* only the frozen C7 mutation, configuration `FPT` exactly as N.9 defines it, the original `explore` as at C-BASE, and C-BASE's model semantics. Those semantics carry to W7-SRC only through B-W (6.3). **Nothing is asserted about any other configuration, explorer (the quotient explorers included), mutation, actor program, or any future model.**

*Named assumptions:*
- **AS-1 (accepted unchanged-core F-PT evidence).** The accepted `44-full-F-PT` composed-quotient PASS on E-C, read through B(1) and N as accepted for F-PT's restored side, establishes the following: on F-PT the unchanged core reaches no state that violates a state property, and takes no step or transition that violates a property. (Per AD-1-R4 D "Safety", the set of reachable violated property names is the original's; here it is empty.) This is ARCHITECT-ACCEPTED evidence and is not re-derived here.
- **AS-2 (semantics).** A-1 to A-3 of 6.3: the compiler is correct, there is no undefined behaviour, and results do not depend on layout or addresses.

*Definitions:*
- A C7 transition is **divergent** if line 1560's `clear()` runs while the table holds an entry whose key is not the explained key.
- A **C7-induced P6 path** is a path of the C7 model, from `initial(&FPT)` to a state that violates P6.

*Lemmas (PROVEN FROM SOURCE):*
- **L1.** A non-divergent C7 transition has the same successor as the unchanged core's transition for the same state, actor and choices. This holds because:
  - the patch changes one statement (SF-13);
  - `remove(key)` and `clear()` leave equal tables when the table ⊆ {key};
  - a `BTreeMap` has no other observable state.
- **L2.** Every divergent transition violates E11.
  - In the step that runs line 1560, `obs.explained` holds only the explained key (SF-13).
  - `transition_properties` (3533–3543) fails E11 for each key that was present before the step, is absent after it, and is not in `obs.explained`.
- **L3.** P6 is never a step or transition violation (SF-12).
- **L4.** `explore` checks a state's state properties when it expands that state (3916–3918). It stops the run at the first violating successor without inserting it (3935–3941).

*Claim M (scoped).* Under AS-1 and AS-2, on F-PT, the original explorer on the C7 model behaves as follows:
- **(a)** It discovers only states that the unchanged core reaches on F-PT. Every discovery edge is non-violating (L4), so non-divergent (L2), so an unchanged-core edge (L1).
- **(b)** It never reports P6. Discovered states violate nothing (by (a) and AS-1), and P6 is never an edge violation (L3).
- **(c)** If it reports FAIL, the failing edge is divergent. Non-divergent edges from discovered states are unchanged-core edges, which violate nothing (AS-1). That edge's violations include E11 (L2). Which of that edge's violations prints first is **not proven** here. E11 printed first historically (EXECUTOR-REPORTED).
- **(d)** Otherwise the run ends INCOMPLETE (state cap or time cap) or PASS. Nothing here establishes which of the outcomes in (c) or (d) occurs.

*So:* under F-PT and these assumptions, every C7-induced P6 path contains a divergent transition, and the original explorer never expands past one. That is why C7's row cannot be killed by the explorer's first failure. This is consistent with the ruling.

*Withdrawn from Draft-01:*
- the unconditional conclusion "on F-PT or on any configuration";
- Draft-01's steps 1–3, as proof.

**1.5 Consequence: Option W**

The 24-step schedule in 4.3 is a **candidate** C7-induced P6 path: **STATIC INFERENCE — NOT OBSERVED.** Execution has not established that it exists, that every step is enabled, that it is reachable, or what it violates. Minimality is not claimed.

A deterministic witness can test it. The witness must:
- replay the schedule with the unchanged predicates;
- never stop at a violation;
- record E11 separately.

If execution confirms the schedule, the witness shows P6 failing on the world after step 24. A4 is essential to the schedule: at step 16, A4 turns F1's dropped result into Uncertain.

## 2. Exact clauses superseded (C7 only)

Rows S-C7-1 to S-C7-14:

| # | Clause | Effect, for C7 only |
|---|---|---|
| S-C7-1 | Contract line 693 (AD-1-R4 S-6) | **REPLACED** as to "It must make its property fail by assertion". C7's kill criterion is G.7. Unchanged: a build or fixture failure kills nothing; the stop rule; no other configuration may be picked. |
| S-C7-2 | Contract line 707 | **REPLACED reading** of the cell "P6": "P6 (first failure E11; witness)", recorded only under G.7(F). The change, "**with** A4" and F-PT are unchanged. |
| S-C7-3 | Contract line 853: `Each control of 7.5 is run on its configuration, with a failing receipt, and a restored receipt showing the unchanged core passes that configuration.` (AD-1-R4 S-5) | **SUPPLEMENTED.** C7's failing receipts are G.7(A) and (B), on E-W7. The restored receipt is G.7(D)'s linked exact-Candidate-C F-PT PASS. It is usable only when B-W is discharged for the exact W-SRC identity: B-W/A, B-W/B and B-W/C (6.3). G.7(C)'s healthy witness on E-W is required alongside it. |
| S-C7-4 | AD-1-R4 G, lines 914–915: `G.3 applies to C1 and C25 only. G.5 and G.6 apply to C4 only. Neither is an exception for any other control, and neither extends to one. Every other control is killed only under F.` | **SUPPLEMENTED:** "G.7 to G.9 apply to C7 only. They are not an exception for any other control and extend to none. G.3, G.5 and G.6 are unchanged and do not apply to C7." |
| S-C7-5 | AD-1-R4 F, lines 719–721: `A control is killed only by an admissible receipt of its own type (F.1, K.2): the original explorer's FAIL of its named property, read under G.3 for C1 and C25 and under G.5 for C4; its named test's failure; or, for C22 alone, that build failure.` | **SUPPLEMENTED:** "…and under G.7 for C7, whose primary receipt prints E11, not its named property." |
| S-C7-6 | AD-1-R4 F.1 (a), the Attribution bullet: `…For C4 the printed property is recorded as it is under G.5(A); C4's attribution of both P4 and P8 comes from G.5(B), never from this receipt alone.` | **SUPPLEMENTED:** "For C7 the printed property must be E11. It is recorded as it is under G.7(A). C7's attribution of P6 comes from G.7(B) to (D), never from this receipt." |
| S-C7-7 | AD-1-R4 F.1 (c) and (e3): "(C1, C25, C4)" | **SUPPLEMENTED:** "and C7, under G.7 to G.9 only". |
| S-C7-8 | AD-1-R4 F.2, lines 889–890: `It runs on the pinned unchanged-baseline executable of I: the one fresh optimised build of the exact final baseline.` | **REPLACED.** C7's healthy witness runs on **E-W** (I-C7.5). F.2's mutant-witness rule (886–888) is unchanged; for C7 its "named mutation source identity" is W7-SRC. |
| S-C7-9 | AD-1-R4 I, lines 1358–1360: `…every C1, C25 and C4 witness receipt (mutant and healthy), the A4 comparison and the C7-without-A4 run is made against the exact final baseline commit, or a mutant of it.` | **REPLACED** for C7's failing receipts, C7's witness receipts and the C7-without-A4 run: they are made against W-SRC or W7-SRC (I-C7). Unchanged: "No historical PASS substitutes for a final-baseline run". C7's restored (e1) side stays exact Candidate C. |
| S-C7-10 | AD-1-R4 I, lines 1368–1372: `…The same pinned mutant executable serves every invocation of that mutant: its original-explorer run and, for C1, C25 and C4, its mutant witness run. Every healthy witness runs on the pinned unchanged-baseline executable (F.2).` | **SUPPLEMENTED.** E-W7 serves C7's primary, its mutant witness and the C7-without-A4 run. E-W serves C7's healthy witness. Neither is the pinned baseline executable. |
| S-C7-11 | AD-1-R4 I, mutant-build identity items 1 and 7 | **SUPPLEMENTED.** Item 1 reads "W-SRC's commit and tree, with Candidate C recorded as its frozen base". Item 7 reads EXACTLY_W (I-C7.6). |
| S-C7-12 | AD-1-R4 I, receipt linking (1393–1398; adopted) | **SUPPLEMENTED** by I-C7.7. |
| S-C7-13 | AD-1-R4 F, "C7 without A4 (line 748)": `It is C7's mutant build, run by full_f3 …` | **CLARIFIED:** "C7's mutant build" is E-W7 (section 10). |
| S-C7-14 | AD-1-R4 S-18 (line 923) | **CLARIFIED:** "every control is killed" reads per G.7 for C7. |

Rows S-C7-15 to S-C7-18:
- **S-C7-15:** the frozen catalog's C7 entry is **not edited**. Its operative reading is the effective contract EC7 of 2A, and nothing else.
- **S-C7-16:** AD-1-R4 E.4 rule 1 is **SUPPLEMENTED** by 6A's disposition for the one added test name.
- **S-C7-17:** AD-1-R4 N, "The rule" (an addition after the audited text needs review that it does not alter the audited code's meaning), is **APPLIED, not superseded**, to W-PATCH. The application is NI-1 to NI-8 and B-W.
- **S-C7-18: the catalog's historical P6 expectation for C7 is superseded, for C7 only.**

  The frozen catalog's C7 entry (K1) keeps `execution_dependent_uncertainty` byte-identical. Its two items read:
  - [0] "The same patch serves the required F-PT (with A4) kill run and the separate AD-1-R4 F (line 748) original-explorer F-3 comparison without A4, which is not required to kill."
  - [1] "Whether F-PT's original explorer reaches a P6 failure within 30,000,000 states is decided only by execution."

  Item [1] is the catalog's old expectation that C7's first failure in the original explorer would be P6. It was written alongside `expected_primary_failure.printed_property_must_be: "P6"`, which OVERLAY-C7's `set` already replaces. On adoption, AD-C7-W supersedes item [1] as an operative statement, for C7 only:
  - The primary's printed property must be E11 (G.7(A)). A P6 first failure is case C7-2: it is not (A), it contradicts 1.4, and the mission stops for a ruling.
  - P6 is established only by the G.7(B) witness.
  - Item [1] remains historical text. It is never edited, never relied on, and never read as an expectation or a kill condition.

  Item [0] is not superseded; it agrees with section 10. Nothing here changes any of these: the catalog's bytes; K1; OVERLAY-C7; `OVERLAY-SHA256`; the 34 accepted controls; A4; or any other entry.

**Not superseded:**
- every other row of contract 7.5, and every property;
- F-PT's and F-3's bounds and ceilings;
- G.3, G.4, G.5 and G.6;
- K.1, and K.2 points 1–7;
- D, N, AD-1-R5;
- contract lines 748, 750, 857, 859, 865 and 940–942;
- every stop condition of contract section 12;
- the catalog's bytes, its top-level `baseline` object, and every other entry.

## 2A. The effective C7 contract overlay

**2A.1 The binding tuple.** Every part must match exactly. If any part is absent or differs, the result is **STOPPED BEFORE EXECUTION**.
- **K1 Catalog:** path `docs/evidence/p3-candidate11/d2a1-control-definitions-r2-01/catalog/catalog.json`, blob `1918374263740d2f98640da00fc466737945975e`, sha256 `d6c05a7c…1a83`. It is read from git objects.
- **K2 Amendment:** the frozen AD-C7-W file's path, git blob and sha256 (`AD-C7-W-SHA256`). These are fixed only when a separately authorised write freezes the file.
- **K3 Overlay:** the bytes between the lines `BEGIN OVERLAY-C7` and `END OVERLAY-C7` in K2, extracted mechanically. Their sha256 is `OVERLAY-SHA256`.
- **K4 Adoption:** the adoption record of 12.3: its commit, path, git blob and sha256. These are the record's future identities. They exist only once the record is committed, and are pinned afterwards by a subsequent Primary Architect-authorised mission (AU-4, AU-5).
  - It must name K1, K2, K3 and K5 exactly, and the conveying mission.
  - It does not contain or claim its own final commit, git blob or sha256.
  - It counts only when AU-1 to AU-6 hold.
- **K5 C-BASE:** commit `d5964ee5…`, tree `51bac5fd…`.

**No conveyed and authenticated adoption: STOPPED BEFORE EXECUTION. No exact match: STOPPED BEFORE EXECUTION. A record on its own is never a match.**

**2A.2 OVERLAY-C7 (exact text)**

```text
BEGIN OVERLAY-C7
{
  "overlay": "AD-C7-W OVERLAY-C7 v1",
  "catalog": {
    "path": "docs/evidence/p3-candidate11/d2a1-control-definitions-r2-01/catalog/catalog.json",
    "git_blob": "1918374263740d2f98640da00fc466737945975e",
    "sha256": "d6c05a7cdc71b3ca1812b5199b8529cbe6db0363ca11bacc26d1f21c2cd11a83"
  },
  "c_base": {
    "commit": "d5964ee5326c2733177307db6f507328023d361a",
    "tree": "51bac5fd335999cc995c0ab9f582a30244973ef0"
  },
  "entry": "C7",
  "set": {
    "expected_primary_failure.printed_property_must_be": "E11",
    "witness_required": true,
    "receipt_type": "(a) original-explorer mutant FAIL printing E11, on E-W7; (c) mutant witness FAIL establishing P6 with E11 separate, on E-W7; (e3) healthy witness PASS, on E-W; restored (e1) linked under AD-C7-W I-C7.7"
  },
  "add": {
    "primary_attribution": "E11, recorded exactly as printed; attributes nothing to P6 (AD-C7-W G.7(A))",
    "named_property_attribution": "P6, established only by AD-C7-W G.7(B), with E11 recorded separately and never counted as P6; recorded as 'C7: P6 (first failure E11; witness)' only under G.7(F)",
    "witness": {
      "test": "ledger::protocol::model_tests::c7_witness::c7_witness_p6_after_settlement_on_fpt",
      "ignored": false,
      "configuration": "F-PT",
      "mutant_receipt": "type (c), on E-W7, the executable of the primary (AD-C7-W G.7(B), K.2-C7)",
      "healthy_receipt": "type (e3), on E-W (AD-C7-W G.7(C), K.2-C7)",
      "violations_required": ["step 13 transition: E11", "after 24: P6"],
      "rules": "AD-C7-W sections 4, 6A and 8"
    }
  },
  "append": {
    "special_attribution_rules": [
      "AD-C7-W G.7 (C7 only): the primary prints E11, recorded as printed; P6 by the G.7(B) witness only"
    ]
  }
}
END OVERLAY-C7
```

**2A.3 Deterministic precedence**

- **P-1.** For a key path *k* inside the C7 entry:
  - if *k* is in `set`, use the overlay value. *k* must already exist in the entry.
  - if *k* is in `add`, use the overlay value. *k* must not already exist in the entry.
  - if *k* is in `append`, use the catalog list followed by the appended items. *k* must be an existing list.
  - otherwise, use the catalog value verbatim.
- **P-2.** The overlay may hold only the seven keys shown: three in `set`, three in `add`, one in `append`. Any other key, or a failed existence condition in P-1, makes the overlay invalid: **STOPPED BEFORE EXECUTION**.
- **P-3.** Entries other than C7 (the 34 accepted controls and A4) and every top-level catalog key, `baseline` included, always take the catalog's value. The overlay cannot address them.
- **P-4.** For C7, AD-C7-W prevails only at the clauses listed in section 2. Elsewhere, AD-1-R5, AD-1-R4 and the base contract govern exactly as they do now.
- **P-5.** These fields of the C7 entry stay verbatim: `named_property` (`["P6"]`), patch, mutant blob, `edits`, `test`, `configuration`, ceilings, `restoration`, `restored_receipt` and `additional_comparison`.
- **P-6.**
  - By P-1's last rule, EC7 carries `execution_dependent_uncertainty` verbatim, and its bytes are part of EC7's sha256.
  - Its item [1] has no operative force (S-C7-18, P-4).
  - Neither the overlay nor EC7's serialisation is changed to say so.

**2A.4 Derivation of EC7** (made by the future execution mission; recorded as a receipt; never written to the catalog path)
1. Read K1's bytes from git and parse them as JSON.
2. Select the one entry whose `id` is `"C7"`.
3. Apply P-1, then serialise canonically: sorted keys, UTF-8, separators `,` and `:`.
4. Record EC7's sha256.
5. Check that the canonical form of every other entry and every top-level key is byte-equal before and after.
6. Record K1 to K5 and EC7's sha256 together.

**2A.5 What the overlay never does.** It never changes:
- the catalog's bytes;
- another row;
- `baseline`;
- the patch or mutant;
- the configuration or ceiling;
- the restored receipt;
- the F-3 comparison;
- the named property.

The C7 primary's binding to E-W7, and the `EXACTLY_W` restoration label, are contract-level rules (S-C7-9 to S-C7-11, I-C7). They are not overlay fields. See decision OD-12 (Annex A).

The overlay does not touch `execution_dependent_uncertainty`. Its item [1] is superseded at contract level, by S-C7-18, and not by an overlay field.

## 3. Proposed C7-only kill criterion — G.7

> **G.7 (C7 only).** "C7 is killed when (A), (B), (C), (D) and (E) all hold. It is then recorded as (F) states. EC7 (2A) is the only operative reading of C7's catalog entry. Every failing receipt is made on **E-W7**, and the healthy witness on **E-W**. The witness never replaces the primary run.
>
> **(A) The primary failure is preserved, exactly as E11.**
> - Type (a): `ledger::protocol::model_tests::full_fpt`, run with `--exact --ignored`, one match, on E-W7.
> - It is the original explorer: header `EXPLORATION F-PT`, no quotient or symmetry line, F-PT's bounds with `A4 true`.
> - It stays within 30,000,000 discovered states (K.1), and is admissible under K.2 and K.2-C7.
> - The harness prints `running 1 test` and `348 filtered out` (6A).
> - It FAILs with printed property **E11**, by a normal exit 101.
> - These lines are kept unedited: `outcome: FAIL`, `FAIL: E11: <detail>`, `states <n> (expanded <m>)`, and the trace.
> - E11 is never relabelled and never counted as P6.
> - Any other printed property (P6 included), a PASS, an INCOMPLETE, or a FAIL beyond the ceiling is recorded as printed. C7 is then not killed, contract 7.5's stop rule applies, and no later invocation follows.
>
> **(B) P6 is established by the mutant witness, with E11 recorded separately.**
> - Type (c): `ledger::protocol::model_tests::c7_witness::c7_witness_p6_after_settlement_on_fpt`, run with `--exact` (it is not ignored), one match.
> - It runs on **the same E-W7** as (A), with an equal sha256 before and after both runs.
> - It fails by its own verdict: `1 failed`, `348 filtered out`, and exit 101 by a normal exit.
> - **The witness observes exactly two violations, as two canonical records (4.7):**
>   1. E11 at the **transition of step 13**;
>   2. P6 on the **world after step 24** (a state violation).
> - No other canonical record exists, for any property at any locus. E11 and P6 each have exactly one (CL-5).
> - The panic summary reproduces exactly those two records. It is a copy, and it is never counted (CL-6).
> - M1 reports 2. M2, M3 and M4 agree with the canonical records (CL-7).
> - G.9 holds.
> - A fabricated or inconsistent marker kills nothing.
>
> **(C) The healthy core violates nothing on the same schedule.**
> - Type (e3): the same test, `--exact`, one match, on **E-W**.
> - `ok`, `1 passed; 0 failed`, `348 filtered out`, exit 0.
> - H1 is printed exactly. There is no VIOLATION line and no positive marker.
> - Step 23 takes its healthy end, and the kept predicate holds.
>
> **(D) The unchanged core's F-PT PASS, under a discharged B-W.**
> - The accepted exact-Candidate-C receipt `44-full-F-PT` at `f023cc3d` (type (e1)) is linked under I-C7.7 and never rerun.
> - (D) holds only when all three B-W states (6.3) hold for the one W-SRC identity that E-W and E-W7 were built from (E-W7 through W7-SRC):
>   - **B-W/A:** the Owner's adoption of CR-B, conveyed and authenticated under 12.3;
>   - **B-W/B:** the Architect's approval of that frozen W-PATCH against NI-1 to NI-6 and SR-1 to SR-6;
>   - **B-W/C:** that execution mission's own NI-7 and NI-8 receipts, recorded at 13.3 step 6.
> - None of these satisfies (D): the adoption alone; the source approval alone; both without B-W/C.
> - Under a separately authorised V-W, 6.4 governs what replaces this link. Otherwise G.7 is BLOCKED (6.4).
>
> **(E) Identity, restoration and review.**
> - EC7 is derived and matched (2A).
> - I-C7, NI-1 to NI-8 and the 6A inventory checks hold.
> - Restoration is `EXACTLY_W`.
> - (A), (B) and the F-3 run used one E-W7.
> - The F-3 run has a recorded outcome under section 10.
> - The Sole Architect's independent review passes.
>
> **(F) Recording.** Only then is the row recorded as **'C7: P6 (first failure E11; witness)'**."

## 4. Witness contract and 24-step schedule — G.8 and G.9

**4.1 Location (lexical isolation)**
- One private inline module, `mod c7_witness { … }`, is appended after the last byte of `model_tests.rs` blob `28085ce2`.
- It holds the one ordinary test `c7_witness_p6_after_settlement_on_fpt`. The test's full path is `ledger::protocol::model_tests::c7_witness::c7_witness_p6_after_settlement_on_fpt`.
- It runs configuration `FPT` only.
- The module's internal names are non-normative.

**4.2 Required form (G.8)**
1. It uses real protocol operations: `step`, with F-PT's original actors. There is no admission step.
2. It forces no state. It starts from `initial(&FPT)`, and every step runs on a world decoded from the previous key.
3. It takes real `hook::take_counts` around each `step` and passes the counts to `transition_properties`.
4. It uses the unaltered `transition_properties`, `state_properties` (through `violations_at`, unchanged), the violations pushed inside `step`, and the key round trip.
   - It separates each step's **in-step** violations from its **transition** violations by the length of `obs.violations` just before calling `transition_properties` (SR-1).
5. It uses the unchanged `Oracle` and `Io`. Each step's offered stream is the Oracle prefix.
6. Every step is asserted enabled.
7. It applies G.9. Branch labels are frozen at every step, except step 23 (two planned ends) and step 24 (paired with step 23).
8. It collects every violation:
   - of the initial state;
   - of each step, in-step and transition separately;
   - of each state after a step.

   No violation stops the replay.
9. **The required loci are E11 at step 13's transition, and P6 on the world after step 24.** There must be no other violation.
10. On the healthy core: no violation, the healthy end at step 23, and the kept predicate.
11. It prints divergence step by step, as evidence.
12. The source constraints are NI-1 to NI-8 and SR-1 to SR-6 (6.2).

**Why the precedents cannot be reused as they stand** (PROVEN FROM SOURCE):
- `run_witness` requires identical choices on every core (`model_tests.rs` 8333–8338). Step 23's consumption differs between the cores.
- `c4_replay` puts the changed step last (it replays its steps, then one `Ack(0)` append), and its world summary is specific to G1 and U.

**4.3 Schedule — STATIC INFERENCE — NOT OBSERVED**

This is STATIC INFERENCE: re-derived by the executor from SF-1 to SF-10, `Oracle::choose` (546–562) and `Io::pick` (652–669). It is unexecuted and not verified.

The composition is:
- **Steps 1–13:** the historical C7 trace mirrored, with Fw(0)↔Fw(1) and F1↔F2.
- **Step 14:** a settlement step.
- **Steps 15–24:** C6's accepted P6 trace, steps 5–14.

Option counts shrink to 1 only when the fault budget is 0, which never happens here.

**Table A (frozen data)**

| # | Actor | Offered | Consumed (choice, options) | Unused | Faults left | Frozen branch labels |
|---|---|---|---|---|---|---|
| 1 | Fw(1) | [] | [] | [] | 2 | [] |
| 2 | Fw(1) | [0,0,0,0] | [(0,2),(0,4),(0,4),(0,3)] (expire, Open, Facts, Rollback) | [] | 2 | ["append_facts: NotApplied NotWritten Conflict(new or retained cause DuplicateRole)"] |
| 3 | Fw(1) | [0] | [(0,2)] | [] | 2 | [] |
| 4 | Fw(0) | [] | [] | [] | 2 | [] |
| 5 | Fw(0) | [0,0,0,0] | [(0,2),(0,4),(0,4),(0,3)] | [] | 2 | ["append_facts: NotApplied NotWritten Conflict(new or retained cause MissingPrepared)"] |
| 6 | Wk | [] | [] (one allowed action, so `branch(1)` consumes nothing) | [] | 2 | [] |
| 7 | Wk | [] | [] | [] | 2 | [] |
| 8 | Wk | [0,0,0,0] | [(0,3),(0,3),(0,2),(0,2)] (Open, Present, Insert, Commit) | [] | 2 | ["append_cell X: Committed (inserted)"] |
| 9 | Wk | [] | [] | [] | 2 | [] |
| 10 | Wk | [] | [] | [] | 2 | [] |
| 11 | Wk | [] | [] | [] | 2 | ["empty: Done"] |
| 12 | Se | [] | [] (one eligible claim) | [] | 2 | [] |
| 13 | Se | [0] | [(0,4)] | [] | 2 | ["explain: Explained"] |
| 14 | Se | [] | [] | [] | 2 | [] |
| 15 | Fw(0) | [1] | [(1,2)] (a **fault**) | [] | **1** | [] |
| 16 | Fw(0) | [] | [] | [] | 1 | [] |
| 17 | Se | [] | [] | [] | 1 | [] |
| 18 | Se | [0] | [(0,4)] | [] | 1 | ["lookup: Absent"] |
| 19 | Se | [] | [] | [] | 1 | [] |
| 20 | Se | [] | [] | [] | 1 | [] |
| 21 | Se | [] | [] | [] | 1 | [] |
| 22 | Env | [] | [] | [] | 1 | [] |
| 23 | Se | [0,0,0,0] | Healthy: [(0,4),(0,4),(0,3)]. Mutant: [(0,4),(0,4),(0,4),(0,3)] | Healthy: [0]. Mutant: [] | 1 | Healthy: ["append_facts: NotApplied NotWritten Conflict(new or retained cause MissingPrepared)"]. Mutant: ["append_facts: Committed (inserted)"] |
| 24 | Se | [] | [] on both cores | [] | 1 | [] on both cores |

**Table B (predicted; informational; not frozen)**

| # | Predicted note | Predicted state change |
|---|---|---|
| 1 | `arm -> Armed` | — |
| 2 | `append_facts["F2"] -> NotApplied NotWritten Conflict(new or retained cause DuplicateRole) [Open:Ok Facts:Ok Rollback:Ok]` | table {F2: DuplicateRole}; X holds F2's explanation (Built); F2 marked conflicted |
| 3 | `[T] records Counted(Counted { persistence: NotWritten, conflict_member: Some(0) })` | F2's class Conflict; debt 1 |
| 4 | `arm -> Armed` | — |
| 5 | `append_facts["F1"] -> NotApplied NotWritten Conflict(new or retained cause MissingPrepared) [Open:Ok Facts:Ok Rollback:Ok]` | table {F1: MissingPrepared, F2}; X unchanged (no explanation of F1); F1 marked conflicted |
| 6 | `reads the agenda (frontier 0), chooses AppendX(…X, occupancy 1)` | — |
| 7 | `[T] A becomes the larger of A and the agenda's frontier; H before X is emptied` | — |
| 8 | `append_cell X: Committed (inserted) [Open:Ok Present:Ok Insert:Ok Commit:Ok]` | ledger gains Explanation(1) at sequence 3; X becomes Committed(3); `settle` removes F2's entry, leaving table {F1} on both cores |
| 9 | `reads the agenda (frontier 0), chooses EmptyX(…, 3)` | — |
| 10 | the `[T]` line, as at step 7 | H becomes 3 |
| 11 | `empty: Done []` | X empty |
| 12 | `[T] claims F2 for Explain` | — |
| 13 | `explain F2 -> Explained [Explained:Ok]` | Healthy: table {F1}. Mutant: table {}, and **E11** fails on this transition |
| 14 | `[T] applies Explain(Explained(3)); releases` | F2's class Retired; debt 0 |
| 15 | `unwinds before recording   [fault]` | — |
| 16 | `[T] drop path records from Some(Counted { persistence: NotWritten, conflict_member: Some(0) })` | F1's class Uncertain (A4); debt 1 |
| 17 | `[T] claims F1 for Lookup` | — |
| 18 | `lookup F1 -> Absent [Row:Ok]` | the same on both cores (SF-6) |
| 19 | `[T] applies Lookup(Absent); releases` | F1's class Refused |
| 20 | `[T] claims F1 for Append` | — |
| 21 | `arm -> Armed` | — |
| 22 | `F1's Prepared record appears` | ledger gains Prepared(0) |
| 23 | Healthy: refused, `[Open:Ok Facts:Ok Rollback:Ok]`. Mutant: `Committed (inserted)`, `[Open:Ok Facts:Ok Insert:Ok Commit:Ok]` | Healthy: ledger unchanged. Mutant: ledger gains Fact(0) |
| 24 | Healthy: `[T] applies Fact(Counted(Counted { persistence: NotWritten, conflict_member: Some(0) })); releases`. Mutant: `[T] applies Fact(Committed); releases` | Healthy: F1's class Conflict. Mutant: F1's class **Paid**, so **P6** fails on the world after this step |

Static cross-check for Draft-02:
- No state property reads the refusal table except E11's limit check (3508–3510).
- F-PT has `batch: false`, and step 18's lookup answers Absent, so E6 is not expected (SF-13).

These are STATIC INFERENCE, not observation.

**4.4 Planned ends**

- **Step 23** has exactly two planned ends, as in Table A.
- **Step 24** offers nothing, consumes nothing and shows no branch on either end. Its end must pair with step 23's: a healthy end with a healthy end, a changed end with a changed end.
- The Draft-01 mission's expectations are matched: at step 23, healthy `NotApplied NotWritten Conflict(…MissingPrepared)` and mutant `Committed (inserted)`; at step 24, healthy has no P6 and the mutant has P6 after settlement applies the committed result.

**4.5 Healthy and mutant divergence**

| Steps | What differs |
|---|---|
| 1–12 | Nothing: identical worlds. |
| 13 | Same consumption and branch. The core's table differs (healthy {F1}, mutant {}). The mutant shows E11 at the transition. |
| 14–22 | Same consumption, branches and fault budget. Only the table differs in the key. |
| 23 | The first *visible* divergence (4.3). |
| 24 | F1's class: Conflict (healthy) versus Paid (mutant). |

The predicates checked at the loci:
- **Kept (healthy):**
  - after steps 13 and 24, the table is exactly {F1 → MissingPrepared};
  - after step 24, F1 is Conflict, Fact(0) is absent and X is empty.
- **Lost (mutant):**
  - after steps 13 and 24, there is no entry for F1;
  - after step 24, F1 is Paid and conflicted, Fact(0) is present and Explanation(0) is absent.

**4.6 State recorded after every step**

- actor;
- offered, consumed (with option counts), unused, faults left, and the fault flag;
- the note and the branches;
- the word (live and unapplied counts), phase and latch;
- X's occupant and stage;
- **the refusal table, each entry as F1 or F2 with its cause label**;
- the test table (classes, debt, H, A);
- F1's and F2's `conflicted` bits;
- the ledger;
- every `VIOLATION step` and `VIOLATION after` line.

Plus these frozen line prefixes, which the classifier reads:
- `    refusal table after NN: {…}`, with entries `F1: <cause label>` / `F2: <cause label>`, or `{}`;
- `    classes after NN: F1 <Class>, F2 <Class>; debt <d>; H <h>; A <a>`;
- `    conflicted after NN: F1 <bool>, F2 <bool>`;
- `    ledger after NN: [<SimRow Debug>, …]`.

**4.7 Exact output**

*Per-step line* (formatted as C4's, with `C7` and `F-PT` in place of C4's labels):

`C7 WITNESS F-PT step NN/24 <Actor> offered […] consumed […] unused […] faults left k: <note>[   [fault]]`

*VIOLATION line (frozen):* four spaces, then `VIOLATION <locus>: <property>: <detail>`. The locus is one of:
- `initial`;
- `step NN in-step`;
- `step NN transition`;
- `after NN`.

*Canonical records*
- Each collected violation is printed exactly once during the replay, as one VIOLATION line inside its locus block (4.7A). That line is its **canonical record**.
- Canonical records are the only violation observations that a receipt contains.

*Panic summary*
- When the test panics, its message ends with the line `every violation:`.
- That line is followed by the **panic summary**: a byte-for-byte copy of every canonical record, in the same order, and nothing else.
- The summary is a copy for the reader. It is never a second observation, and it is never counted (CL-6).

*The only admissible canonical records in (B)* are these two, in this order. Their detail strings are the predicates' own text: `model_tests.rs` line 3540, and the format at line 3459.
- `    VIOLATION step 13 transition: E11: a table entry left in a step that did not see its explanation in the ledger`
- `    VIOLATION after 24: P6: F1 was refused as a conflict and is Paid`

So an admissible (B) receipt has four raw lines that contain `VIOLATION `: the two canonical records and their two copies. It records two violations.

*Mutant markers:* the (c) receipt has each of these exactly once.
- **M1** `C7 WITNESS F-PT: 24 steps, every step enabled and consuming its choices as planned; <n> violations`. Admissible only when n = 2.
- **M2** `NAMED PROPERTY P6: OBSERVED on the world after step 24 (append_facts: Committed (inserted) at step 23; F1, refused as a conflict at step 5, is Paid, with no refusal-table entry and no explanation of F1 in the ledger)`
- **M3** `SEPARATE PROPERTY E11: OBSERVED at the transition of step 13 (explain: Explained of F2; F1's refusal-table entry left in a step that did not see F1's explanation in the ledger); never counted as P6`
- **M4** `OTHER PROPERTIES VIOLATED, each reported separately and not counted as P6: ["E11"]`

*Healthy marker:* the healthy witness (type (e3)) must print:

- **H1** `C7 WITNESS F-PT: 24 steps, each enabled and consuming its choices as frozen; no violation; step 23 shows the healthy core's 'append_facts: NotApplied NotWritten Conflict(new or retained cause MissingPrepared)', consuming 3 of its 4 offered choices, unused [0]; F1's refusal-table entry (MissingPrepared) kept after steps 13 and 24; step 24 records F1 as Conflict`

*Failure markers:*
- `WITNESS FIXTURE FAILURE (kills nothing): <reason>`;
- `NAMED PROPERTY P6: NOT OBSERVED …`;
- `SEPARATE PROPERTY E11: NOT OBSERVED …`.

If the authored source cannot print these exact strings, the source mission stops for a ruling.

**4.7A Raw-output regions** (used by 8.2)

A (B) raw output divides into five regions, in this order. The classifier finds each region by the exact lines named below. If a boundary line is missing, repeated or out of order, the receipt is NOT ADMISSIBLE.

| Region | Begins with | Ends before | What is checked there |
|---|---|---|---|
| **HEAD** | `running 1 test` | the next line | that line only |
| **REPLAY** | the next line. It must start with the harness prefix `test <full test path> ... `, which is removed once before classification | the panic line in (B); the harness result `ok` in (C) | per-step lines, 4.6 state lines and canonical records; in (C), also H1 |
| **MARKERS** | the panic line, `thread '<full test path>' (<tid>) panicked at <file>:<line>:<col>:` | the line `every violation:` | M1, M2, M3 and M4, once each, in that order |
| **SUMMARY** | the line after `every violation:` | the line ``note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace`` | the panic summary only |
| **TAIL** | that `note:` line | the end | harness lines only (`FAILED`, `failures:` and the result line) |

(C) has only HEAD, REPLAY and TAIL.

*Locus blocks in REPLAY*
- The `initial` block is the part of REPLAY before step 01's per-step line.
- Step NN's block runs from step NN's per-step line to the next per-step line. Step 24's block runs to the end of REPLAY.
- A canonical record must lie in the block of its own locus:
  - `initial` in the initial block;
  - `step NN in-step`, `step NN transition` or `after NN` in step NN's block.

*Where the boundary forms come from.* They are the forms seen at C, under rustc 1.94.0, in `81-C4/04c-witness` and `81-C4/04-run`:
- the harness prefix on the first output line;
- the panic line, with its thread id;
- the `note:` line;
- `FAILED`.

This is EXECUTOR-REPORTED. If E-W7's output differs, the receipt is NOT ADMISSIBLE and the mission stops for a ruling. The boundaries are never re-read to fit the output.

**4.8 Verdict and fixture rejection**

Each of these is a `WITNESS FIXTURE FAILURE`. It kills nothing and makes the receipt inadmissible:
- an actor that is not one of F-PT's;
- a step that is not enabled;
- any breach of G.9;
- branches or fault budget at steps 1–22 other than frozen;
- step 23 ending as neither planned end;
- step 24 not paired with step 23's end;
- frozen data that is inconsistent with itself.

The verdict, after the replay:
- **No violation:** step 23 must have ended healthy and the kept predicate must hold. The test then prints H1 and passes. Otherwise it panics with `WITNESS: no violation, but …`, which is neither a pass nor a kill.

**Any violation:** the test panics. Its message is:
- M1 to M4, in that order, each with OBSERVED or NOT OBSERVED as computed (SR-3);
- then the line `every violation:`;
- then the panic summary (4.7).

Nothing follows the summary in the message.

**4.9 G.9 — the offered-versus-consumed rule (C7 only; G.6 is unchanged and remains C4's)**

1. Both cores receive the same actor sequence and the same offered stream at every step.
2. Steps 1–22 consume exactly their frozen choices with their option counts, leave no unused suffix, show the frozen branch labels and leave the frozen fault budget. Both cores behave identically here.
3. Step 23 has exactly two ends:
   - **Healthy:** consumes `[(0,4),(0,4),(0,3)]`, leaves unused `[0]`, 1 fault left.
   - **Changed:** consumes `[(0,4),(0,4),(0,4),(0,3)]`, leaves nothing unused, 1 fault left.

   The healthy end's unused `[0]` is **justified and frozen here**. The refused healthy append issues no insert, so its third choice point is the rollback, with 3 options, and it never reaches a commit.
4. Step 24 consumes nothing on either end, and its end pairs with step 23's.
5. Every consumed choice must equal the offered prefix. These are rejected:
   - consumption beyond the offered stream (which `Oracle::choose`, line 552, would silently answer with 0);
   - any unused suffix other than step 23's healthy `[0]`.
6. Every value is recorded per step and verified twice: by the witness, and by the classifier from the raw output.

**4.10 What static inspection establishes.**
- Every value in 4.3 is derived from cited source. None is guessed.
- **Nothing is established by execution.**
- Minimality and reachability are not claimed.

## 5. Healthy witness obligations

The healthy witness (G.7(C)) must:
- be type (e3), on **E-W only**;
- select exactly one test with `--exact`, with a listing showing one match;
- exit 0 by a normal exit, with the harness line `test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 348 filtered out`, and K.2 points 1–7 (the non-exploration reading);
- print H1 exactly and all 24 per-step lines;
- print zero `VIOLATION` lines, no `OBSERVED` marker and no fixture failure;
- end step 23 healthy, with step 24 paired;
- satisfy the kept predicate;
- run on an E-W whose sha256 is equal before and after;
- run from a healthy worktree that stays EXACTLY_W throughout.

A violation on the healthy core is "a property fails on the unchanged core" (line 940): stop and report.

Proposed order: the healthy witness runs **first** (13.3). Neither G.7(C) nor G.7(D) substitutes for the other.

## 6. Test-only source identity, non-interference and the bridge — I-C7

**6.1 I-C7.1 to I-C7.8**

- **I-C7.1 Frozen base (C-BASE).** Candidate C, exactly as in section 0. It is never edited.
- **I-C7.2 W-PATCH.** One unified diff, touching only `crates/nexus-governed-control/src/ledger/protocol/model_tests.rs`.
  - It is a pure append: the first 310,612 bytes of the result (W-BLOB) equal blob `28085ce2` byte for byte.
  - No line is changed or removed.
  - It records the patch sha256, the lines added, and W-BLOB's git id, sha256 and size.
  - No other path changes, including `protocol.rs`, `unit_tests.rs`, `Cargo.toml` and `Cargo.lock`.
  - W-PATCH must satisfy NI-1 to NI-8 (6.2).
- **I-C7.3 W-SRC (the unchanged-core, witness-bearing source).**
  - Commit W-COMMIT, single parent `d5964ee5`. Its tree, W-TREE, differs from `51bac5fd` only at that path.
  - `protocol.rs` in W-TREE is blob `a2df2841`, asserted by blob comparison.
  - It is authored only by a separately authorised source mission, published on a review ref, reviewed by the Architect and frozen. The order is that of 13.1 and 6.3: W-PATCH is frozen and published first, and only then reviewed by the Architect (B-W/B).
  - It is **not** Candidate C, not the exact final baseline and not a candidate for one. No receipt calls W-TREE `51bac5fd` or labels it "EXACTLY_C".
- **I-C7.4 W7-SRC (the C7-mutated, witness-bearing source).**
  - W-TREE plus the frozen C7 patch (`b5d8bc6c…`), applied unchanged in a disposable worktree. It is never committed and never `git add`ed.
  - Exactly one path differs from W-TREE: `protocol.rs`, which becomes blob `0d4ab992` (sha256 `6d2f2d49…`), verified by blob.
  - Exactly two paths differ from C-BASE.
- **I-C7.5 Executables.**
  - **E-W** is a fresh optimised build of W-SRC. **E-W7** is a fresh optimised build of W7-SRC.
  - Each comes from its own fresh worktree-local target, with the pinned build command (N.7) and the toolchain of the Candidate C pinned builds, and must build with zero warnings and zero errors.
  - Each records its path, sha256, size and every artifact it produces.
  - **No equality** with E-C (`cde9f9d2…`) or with the historical C7 executable (`b320977a…`) is claimed or required. Reproducible builds are **not** claimed. Every identity is the sha256 of the executable actually run.
  - It adds the stale-artifact inequalities of 0A, and "one build per source identity" (7.1).
- **I-C7.6 Restoration (`RESTORE: EXACTLY_W`).**
  - Method: `git apply -R` of the same frozen patch, by a script kept outside the repository (line 750).
  - HEAD is W-COMMIT, detached, and the index equals HEAD.
  - Every tracked blob equals W-TREE's. `protocol.rs` is `a2df2841`, equal to Candidate C's; `model_tests.rs` is W-BLOB.
  - Status shows only ignored entries beneath the worktree-local target.
  - Blob `0d4ab992` is never written to the object store.
  - The label is never `EXACTLY_C`.
- **I-C7.7 The F-PT restored-side link (G.7(D)).** This supplements AD-1-R4 I's adopted receipt-linking rule for C7. The link record must:
  1. read the identities of `44-full-F-PT` from git objects, without rerunning anything: test `gc_symmetry_fpt`, executable `cde9f9d2…` (equal before and after), raw output sha256 `0863794e…`, blob `a92cb8da…`, `CLASSIFICATION: PASS`;
  2. state that this receipt is of exact Candidate C on E-C, not of W-SRC, E-W or E-W7;
  3. verify `protocol.rs` blob equality to `a2df2841` in W-TREE and in the restored worktree;
  4. verify W-BLOB's append-only prefix, so the linked test and everything it calls are byte-identical in source;
  5. make I's artifact-identity check against **E-C**, the pinned executable of the exact final baseline, never against E-W;
  6. claim nothing about E-W or E-W7 reproducing that PASS.

  The (D) link is usable only when B-W/A, B-W/B and B-W/C all hold for the W-SRC identity of E-W and E-W7 (6.3).
- **I-C7.8 What may never be claimed:**
  - that W-TREE equals Candidate C;
  - that E-W or E-W7 equals any earlier executable;
  - that the linked PASS was run on E-W;
  - that the witness replaces the primary run;
  - that the C-BASE PASS is a W-SRC PASS.

**6.2 Non-interference conditions for added test code.** NI-1 to NI-8 are the criteria for AD-1-R4 N's "addition after the audited text" review.

| # | Condition | How it is checked |
|---|---|---|
| NI-1 | **Append-only.** W-BLOB's first 310,612 bytes equal blob `28085ce2`. W-BLOB is longer. No existing line, byte or line number changes, so `line!()`, panic locations and `test_line_at_C: 4220` are unchanged | byte comparison |
| NI-2 | **One path, one commit.** W-COMMIT has the single parent `d5964ee5`. `git diff-tree -r --no-renames d5964ee5 W-COMMIT` prints exactly one record: `M` of `crates/nexus-governed-control/src/ledger/protocol/model_tests.rs`, mode 100644 to 100644 | mechanical |
| NI-3 | **One item.** The appended suffix S holds whitespace, comments and exactly one item: `mod c7_witness { … }`. It is inline, has no visibility qualifier, and carries no attribute except outer doc comments | rustfmt-clean, token scan, source review |
| NI-4 | **Fresh outward name.** `c7_witness` is the only binding S adds to `model_tests`' namespaces, and `git grep -w c7_witness d5964ee5` finds nothing (verified for this draft). This matters because `model_tests.rs` begins with `use super::*` (line 22): only a new name that no existing path mentions can leave every existing resolution unchanged | mechanical |
| NI-5 | **Content of S** (token scan plus source review). (a) Every `impl`, derive output included, has as its self type a struct or enum declared in S. No trait is declared. No generic or blanket impl. Derives are limited to Clone, Copy, Debug, PartialEq and Eq. (b) No `static`, `static mut` or `thread_local!`, and no interior-mutable type held in a `const`. (c) No `unsafe`, `extern`, `#[no_mangle]`, `#[export_name]`, `#[link_section]`, `#[used]`, `#[link]` or `#[global_allocator]`. (d) No `macro_rules!`, `#[macro_export]` or `#[macro_use]`. (e) No `pub` of any form, and no re-export. (f) Exactly one `#[test]`. No `#[ignore]`, `#[should_panic]`, `#[cfg]`, `#[cfg_attr]`, lint attribute or inner attribute. (g) S neither calls nor names: `explore`, `explore_experimental`, `explore_symmetric`, `explore_symmetric_kept`, `successors`, `expand`, `SERIAL`, `hook::install`, `hook::remove`, `hook::report`, any C1, C25 or C4 witness item, or `std::env`, `std::process`, `std::thread`, `std::fs`, `std::net`, `std::time`, `std::panic::set_hook`/`take_hook`, or any source of randomness. S may call `hook::take_counts`, as `replay_schedule` does (8325–8327), and the model functions of G.8. (h) S uses `FPT` itself and defines no `Config` | as stated |
| NI-6 | **Build inputs identical** to C-BASE: toolchain, both `Cargo.toml` files, `Cargo.lock`, `.cargo/config.toml`, `lib.rs`, `ledger.rs`, `protocol.rs` (`a2df2841`) and `unit_tests.rs` (`55dc6640`). The build command and environment are exactly those of 7.1. There is no `RUSTFLAGS`, `CARGO_ENCODED_RUSTFLAGS`, `CARGO_PROFILE_*`, `CARGO_BUILD_*`, `RUSTC_WRAPPER` or target override. `rustc -vV` and `cargo -vV` are recorded and must equal E-C's build record | mechanical |
| NI-7 | Both builds finish with zero warnings and zero errors | build receipt |
| NI-8 | The inventory checks of 6A | listing receipts |

**The mission's eight forbidden changes, and where each is enforced:**
1. Changes to existing source lines: NI-1 and NI-2.
2. New global state affecting existing tests: NI-5(b) and NI-5(g).
3. New trait implementations altering existing types: NI-5(a).
4. Altered property predicates: NI-1 and NI-2 (bytes unchanged), plus NI-5(a) and NI-5(e) (resolution unchanged).
5. Altered Oracle or Io behaviour: as item 4.
6. Changed configuration constants: NI-1, NI-2 and NI-5(h).
7. Changed explorer semantics: NI-1, NI-2 and NI-5(g).
8. Modified dependencies or compiler flags: NI-6.

**Source-review obligations.** The Architect's review of W-PATCH must confirm:
- **SR-1.** Each step does the following, in order:
  1. decode `base` and `w` from the key;
  2. `Oracle::new(offered, w.faults)`;
  3. `hook::take_counts()`;
  4. `step(…)`;
  5. `hook::take_counts()`;
  6. `w.faults = oracle.faults`;
  7. take `m = obs.violations.len()`;
  8. `transition_properties(&base, &w, &mut obs, counts)`.

  The in-step violations are `[..m]` and the transition violations are `[m..]`. `after` is `violations_at(&FPT, &encode(&FPT, &w))`. The initial violations are `violations_at` of the initial key.
- **SR-2.**
  - Every collected `(property, detail)` pair is printed once, verbatim, as a canonical record: a line in the frozen VIOLATION form, inside its locus block (4.7A).
  - If the test panics, the panic summary copies those records once each, byte for byte, in the same order.
  - No VIOLATION line has any other source.
- **SR-3.** OBSERVED and NOT OBSERVED, M1's count and M4's list are computed only from the collected pairs, plus the recorded step-23 end and predicates. No path prints a positive marker from constants.
- **SR-4.** The replay stops only at a fixture failure, never at a violation.
- **SR-5.** The kept and lost predicates only read the world.
- **SR-6.** S changes worlds only through `step` and `w.faults = oracle.faults`.

**6.3 Bridge B-W: what the accepted C-BASE F-PT PASS does and does not establish about W-SRC**

*Status:* **PROPOSED — NOT ADOPTED.** B-W is a conditional rule for linking the accepted C-BASE PASS. It never promotes that PASS to a W-SRC PASS.

*The receipt:* `44-full-F-PT` is C-BASE run on E-C, `gc_symmetry_fpt`, a composed-quotient PASS. N binds its quotient claim to C-BASE and to E-C (N.14).

*What it establishes, as accepted:* for C-BASE, through B(1) and N, the unchanged core violates no property on F-PT (AS-1).

*What it establishes about W-SRC directly:* **nothing.** W-SRC has never been compiled or run, and E-C is not a W-SRC executable.

*CR-B, the conditional rule.* It applies to C7 only. Suppose B-W/B and B-W/C both hold for one exact W-SRC identity: W-COMMIT, W-TREE, W-BLOB and the W-PATCH sha256. Then, for that identity:
- every item in N's source closure 1–3 has, in W-SRC:
  - the same source text;
  - the same name resolution: NI-3 and NI-4 leave one new outward binding, which no existing path mentions;
  - the same trait and method resolution: NI-5(a). A new impl on a module-local type can at most create an inference ambiguity. That is a compile error, which NI-7 catches, never a silent change;
  - the same attributes and cfg: NI-5(f);
  - the same global state: NI-5(b) and NI-5(g);
  - the same build inputs: NI-6;
- the added code cannot run in any invocation that does not select the witness, because each invocation runs one exact test (7.2);
- so, under A-1 to A-3, W-SRC's unchanged-core model semantics on F-PT equal C-BASE's. AS-1's content applies to them, and to W7-SRC as 1.4 uses it;
- G.7(D) may link `44-full-F-PT` for receipts made on that identity's E-W and E-W7. The receipt remains C-BASE's (e1) receipt on E-C, and the link record names it that way.

*The three states.* B-W is discharged only when all three hold for one identity. None is inferred from another. B-W/B is also required without B-W/A, under V-W (6.4).

| State | What happens | By whom | What it establishes | When |
|---|---|---|---|---|
| **B-W/A — policy adopted** | The Owner adopts CR-B for C7 only, with NI-1 to NI-8 and SR-1 to SR-6 as its criteria | the Owner; conveyed and authenticated under 12.3 | only that CR-B is the rule. **It establishes nothing about any source:** no code, authored or not, is shown to be non-interfering | at adoption |
| **B-W/B — source approved** | The Architect independently reviews the actual frozen W-PATCH and approves it against NI-1 to NI-6 and SR-1 to SR-6. This is AD-1-R4 N's addition review of W-PATCH | the Architect, in a mission that names W-COMMIT, W-TREE, W-BLOB (git id, sha256, size) and the W-PATCH sha256 | NI-1 to NI-6 and SR-1 to SR-6, for that identity only | after 13.1 freezes W-PATCH, and before the E-W and E-W7 builds |
| **B-W/C — discharged** | The execution mission's build and listing receipts establish NI-7 and NI-8 for that identity | mechanical, in the execution mission; reviewed under G.7(E) | NI-7 and NI-8 for E-W and E-W7, and with them CR-B's conclusion for that identity | after both builds and all listings, and before any test run (13.3 steps 5–6) |

*Ordering rules*
- **O-1.** B-W/A establishes NI-1 to NI-8 and SR-1 to SR-6 for no code. Before B-W/B, no W-PATCH is non-interfering for any purpose.
- **O-2.** B-W/B holds only for the identity it names.
  - Any change to W-PATCH, W-BLOB, W-TREE or W-COMMIT voids it.
  - A new identity needs a new B-W/B and a new B-W/C.
- **O-3.** NI-7 and NI-8 are never conditions for starting the builds (7.1) or the listings (6A) that establish them. Those builds and listings need only B-W/A and B-W/B (13.3 step 1).
- **O-4.** B-W/C comes only from receipts made in the execution mission: E-W's and E-W7's own 7.1 build receipts and 6A listing receipts. A build in any other mission never discharges B-W. This includes a prototype, if one is ever authorised.
- **O-5.** If NI-7 or NI-8 fails:
  - B-W is not discharged;
  - the mission STOPS (7.3) before any test run, and nothing is retried;
  - G.7(D) cannot hold.
- **O-6.** The B-W/C record names:
  - the W-SRC identity;
  - the mission that conveyed B-W/B;
  - E-W's and E-W7's sha256;
  - both build receipts (zero warnings, zero errors);
  - every listing receipt;
  - the result of 6A's four-way agreement check.

  It asserts nothing beyond NI-7, NI-8 and CR-B's conclusion for that identity.

*NI-6 is checked at two times.*
- NI-6's build-input blobs are part of W-TREE, so B-W/B reviews them.
- NI-6's build command, its environment, and the recorded `rustc -vV` and `cargo -vV` exist only at the build. They are checked from the build receipts together with NI-7, and form part of B-W/C.

*Assumptions:*
- **A-1.** rustc 1.94.0 compiles both sources per Rust semantics.
- **A-2.** There is no undefined behaviour in the code that F-PT exploration reaches. Supporting evidence: `#![deny(unsafe_code)]`, and no `unsafe` in either file.
- **A-3.** No PASS/FAIL content depends on addresses, layout, executable size or timing. The one exception is that the wall-clock cap can end a run INCOMPLETE (SF-14). Supporting evidence:
  - the key excludes addresses (header line 12);
  - the explorer uses its own hash (3606) and BTree collections;
  - neither file uses `HashMap` or `RandomState`.

*B-W never establishes:*
1. that W-SRC equals C-BASE, or that E-W or E-W7 equals any executable;
2. that any test passes when run on E-W or E-W7;
3. anything about the witness test;
4. that W-SRC or W7-SRC compiles, or compiles without warnings;
5. that N.8 or N.11 to N.14 hold for any executable but E-C;
6. that the original explorer `full_fpt` passes;
7. anything about another configuration;
8. any W-SRC PASS receipt, label or count;
9. that any W-PATCH meets NI-1 to NI-8 or SR-1 to SR-6 because CR-B was adopted;
10. anything for a W-SRC identity other than the one named at B-W/B and B-W/C.

*Precedent* (informational only, not relied on):
- `28085ce2` is a byte-prefix extension of `5ad26ddc`, the blob N names, by 24,201 appended bytes: the C4 witness.
- Whether an N addition review of that append was recorded is **NOT ESTABLISHED** by this draft.

**6.4 If B-W is not discharged**

| Case | Effect |
|---|---|
| B-W/A is absent (the Owner's approval does not adopt CR-B), and there is no V-W authorisation | STOPPED BEFORE EXECUTION (13.3 step 1) |
| B-W/B is absent, or names an identity other than the one the execution mission names | STOPPED BEFORE EXECUTION: no worktree and no build (13.3 step 1) |
| B-W/A and B-W/B hold, and NI-7 or NI-8 fails | STOP at that build or listing (7.3, O-5). No test runs. G.7(D) cannot hold |
| The Architect does not approve the frozen W-PATCH at B-W/B | No quotient claim exists at that W-SRC. A revised W-PATCH needs a new 13.1 mission and a new B-W/B |
| The Architect rejects NI-1 to NI-8 or SR-1 to SR-6 as criteria | Option W is **DESIGN BLOCKED** |

In every case, C7 stays MUTATION_CONTROL_BLOCKED.

**Alternative V-W.** It needs a separate Architect authorisation, which this draft does not give. It runs on E-W, with no extra build:
1. N.8: the 51 D diagnostic tests, each run `--exact`;
2. N.11 to N.14: `gc_symmetry_fpt`, run `--exact --ignored`. It must complete with 0 unexpanded keys and PASS;
3. if the Architect requires the F-3 side to be bound to W-SRC: `gc_symmetry_f3`, likewise;
4. new receipts, typed e1-W. They are never relabelled as C-BASE receipts.

Every one of these reruns a previously accepted test on a new executable. That is why V-W needs its own authorisation.

**Limits of V-W**
- V-W replaces only CR-B's link. It does not replace B-W/B: N's rule still requires the addition review of W-PATCH, because closure item 4 changes.
- V-W's receipts are made on E-W, so NI-7 and NI-8 are still established first, by the 7.1 builds and the 6A listings.
- An original-explorer `full_fpt` PASS on E-W is not expected to complete within 30,000,000 states. This is STATIC INFERENCE from the 29,751,473 measured quotient states, and is NOT ESTABLISHED.

## 6A. Test inventory

**The authenticated base.** Two derivations agree:
- *from source at C:* 348 `#[test]` and 35 `#[ignore`;
- *from E-C's listings:* `20-list-all`, 348 tests; `21-list-ignored`, 35 tests.

At C, a single-test invocation prints `347 filtered out` (seen in 44, 81-C4 and 85-C7). **That count is never copied into a W-SRC receipt.**

**Expected for W-SRC and W7-SRC** (derived; confirmed only by future listings):
- **All tests: 349.** The listing equals E-C's 348 lines plus exactly one new line, `ledger::protocol::model_tests::c7_witness::c7_witness_p6_after_settlement_on_fpt: test`. The trailer reads `349 tests, 0 benchmarks`.
- **Ignored: 35, unchanged.** The listing is byte-identical to `21-list-ignored`'s raw output (sha256 `4b13105f…ae1e`). The witness is not ignored.
- E-W7's listings equal E-W's.
- **Each single-test invocation** on E-W or E-W7 — (A), (B), (C) and F-3 — prints `running 1 test` and **`348 filtered out`**. That count is computed as that executable's own all-listing count minus 1.
- Each `--exact --list` selection shows exactly one line. `--exact --ignored --list` shows it only for `full_fpt` and `full_f3`.
- **Agreement check.** These four must agree: the W-TREE source counts (349 and 35), E-W's listings, E-W7's listings, and these expectations. Any disagreement stops the mission before any run.

**AD-1-R4 E.4 disposition for the added name: `C7_WITNESS_W_SRC_ONLY`.** The test:
- exists only in W-SRC and W7-SRC;
- is never in the exact final baseline;
- is never a contract 8.2, 8.3 or 8.4 run, or part of the evidence gate;
- runs only as G.7(B) on E-W7 and G.7(C) on E-W, once each, under an authorised C7 execution mission;
- keeps its verdicts as printed.

E.3's 34 dispositions, and `authority/tests.rs`' one ignored test, are unchanged.

**"No previously accepted test is rerun"** — the reading proposed here:
- No accepted receipt is re-made. This covers `gc_symmetry_fpt`, `gc_symmetry_f3`, the D diagnostics, and all 34 controls' runs, unless V-W is authorised.
- The C1, C25 and C4 witnesses are never run from W-SRC.
- (A) and F-3 are new C7 invocations, not reruns of accepted receipts. This reading is decision OD-6.

## 7. Execution policy, limits and stop rules

**7.1 Builds**
- **Command:** `cargo test -p nexus-governed-control --locked --release --lib --no-run -j 2`.
- **Environment:** `CARGO_NET_OFFLINE=true`, a worktree-local `CARGO_TARGET_DIR`, and nothing else that NI-6 forbids.
- **Jobs:** at most two.
- **Memory:** at most 16 GiB, with zero swap. The cgroup settings are `memory.max 17179869184` and `memory.swap.max 0`, as in the prior scope records (EXECUTOR-REPORTED, `81-C4`).
- **Time:** a 900-second ceiling.
- **Result:** zero warnings and zero errors.
- **Count:** exactly one build per distinct authorised source identity: E-W from W-SRC, and E-W7 from W7-SRC. There is no other build and no rebuild.

**7.2 Test invocations**
- **Environment:** `env -i PATH=/usr/bin:/bin LANG=C.UTF-8 RUST_BACKTRACE=0`, with cwd `crates/nexus-governed-control`.
- **Limits:** 16 GiB, zero swap, and a 2,700-second ceiling per invocation. One invocation runs at a time.
- **(A) and F-3:** a 30,000,000 discovered-state ceiling (`FULL_CAP`, line 78).
- **Witness:** deterministic, with one exact test per invocation.
- **Commands:**
  - (A): `<E-W7> ledger::protocol::model_tests::full_fpt --exact --ignored --nocapture --test-threads=1`
  - (B): `<E-W7> ledger::protocol::model_tests::c7_witness::c7_witness_p6_after_settlement_on_fpt --exact --nocapture --test-threads=1`
  - (C): `<E-W> ` followed by the same arguments as (B)
  - F-3: `<E-W7> ledger::protocol::model_tests::full_f3 --exact --ignored --nocapture --test-threads=1`
  - Listings (`--list`, and `--list --ignored`) run under the same supervision.

**7.3 Stop rules**

**STOP** — with no retry, rebuild, limit change or substitution — on any of these:
- an external kill, termination by a signal, or an OOM event (`oom` or `oom_kill` above 0);
- any swap use;
- a supervisor timeout;
- a build warning or error;
- an identity, overlay, adoption or inventory mismatch (before execution, this is STOPPED BEFORE EXECUTION);
- (C) not PASS;
- (A) not an admissible E11;
- (B) not admissible;
- restoration not exact.

**A normal exit 101** after an assertion panic in (A), (B) or F-3, with an otherwise admissible receipt, is **not** an external kill. Exit 101 in (C) is the line-940 stop.

**The explorer's own 45-minute cap equals the 2,700-second ceiling.**
- If the supervisor fires first, the run is a timeout: STOP. It is never reinterpreted as INCOMPLETE.
- An INCOMPLETE that the explorer prints itself, with a normal exit, is recorded as INCOMPLETE.

**Also never:**
- automatic retries;
- additional builds;
- resource increases;
- GitHub Actions;
- changes to protected refs (`main`, `rebuild/phase0-trust-boundary`, tags).

**Baseline source-identity checks are not relaxed.** C-BASE (commit, tree, every tracked blob, and the catalog's `baseline`) is authenticated exactly as in the prior gates. The W-SRC and W7-SRC checks are added on top and never substitute for them.

**7.4 One mutant executable**

- E-W7 has one path and one sha256. That sha256 is checked:
  - at the pin;
  - around every `--list` selection;
  - before and after (A), (B) and the F-3 run;
  - after restoration.
- All three descriptors name the same path.
- No cargo command runs in the mutant worktree after the pin.
- E-W7 is never used for the healthy witness.
- E-W is used only for (C), with its sha256 equal before and after.

**7.5 Preservation of the 34 accepted controls**

These are re-executed, rebuilt, re-linked, relabelled and edited by nothing here:
- C22, C14, C19, C21, C27, C28, C12, C9, C11, C17, C18a, C18b;
- C23, C10, C16, C26, C3a;
- C5, A3, A6, C8, C15;
- C2;
- C24, C3b, C3c, C6;
- A1, A2, C20, A5;
- C1, C25;
- C4.

Their receipts, links, locked worktrees and executables stay untouched, and so does A4's separate satisfaction. W-SRC serves C7 only. The C1, C25 and C4 witnesses it carries are never run from W-SRC.

A4 is also untouched.

## 8. Receipt admissibility — K.2-C7

**8.1 Cases**

| # | Case | Effect |
|---|---|---|
| C7-1 | (A) on E-W7 prints E11, within the ceiling, admissible, with `348 filtered out` | (A) holds |
| C7-2 | (A) prints P6 | not (A). It contradicts 1.4, so the mission stops for a ruling |
| C7-3 | (A) prints another property, PASS, INCOMPLETE, or a FAIL beyond the ceiling | not (A); the contract 7.5 stop applies |
| C7-4 | A primary on any other executable, `b320977a` included | NOT ADMISSIBLE |
| C7-5 | All of: (B) on E-W7 has exactly the two frozen canonical records; its panic summary copies them exactly; its markers agree; G.9 holds | (B) holds |
| C7-6 | P6 OBSERVED while E11 is missing or NOT OBSERVED | NOT ADMISSIBLE; stop |
| C7-7 | E11 OBSERVED while P6 is not | NOT ADMISSIBLE; stop |
| C7-8 | Any of: a third canonical record, of any property; E11 or P6 at any other locus (for example, E11 in-step or after 13); a second canonical record of E11 or of P6 at any locus, the same locus included; a canonical record outside its locus block; a line containing `VIOLATION ` outside REPLAY and SUMMARY | NOT ADMISSIBLE; kills nothing; stop for a ruling |
| C7-9 | Any of: M1's n ≠ 2; n ≠ the number of canonical records; n ≠ the number of SUMMARY lines; the summary is not an exact, ordered copy of the canonical records | NOT ADMISSIBLE |
| C7-10 | A fabricated or inconsistent marker: OBSERVED without its VIOLATION line, a repeated marker, altered marker text, a property not in `PROPERTIES` (3694–3696), or a detail differing from the source string | NOT ADMISSIBLE; kills nothing |
| C7-11 | A fixture failure, an unrelated panic, or a compile failure | NOT ADMISSIBLE; stop |
| C7-12 | (C) on E-W: H1, zero VIOLATION lines, exit 0 | (C) holds |
| C7-13 | (C) shows a violation, the changed end, a failed kept predicate, or exit ≠ 0 | NOT ADMISSIBLE; line-940 stop |
| C7-14 | A role swap: a positive marker in (C); H1 in (B); either run on the other executable; or E-W = E-W7 | NOT ADMISSIBLE (F.2) |
| C7-15 | A G.9 breach | NOT ADMISSIBLE |
| C7-16 | E-W7's sha256 differs between (A), (B) and F-3, or before and after any of them | NOT ADMISSIBLE |
| C7-17 | A filtered count ≠ 348, `running` ≠ 1, or a listing mismatch (6A) | NOT ADMISSIBLE |
| C7-18 | Restoration is not `EXACTLY_W`, or is labelled `EXACTLY_C` | (E) fails |
| C7-19 | Any of: EC7 not derived; the overlay does not match; AU-1 to AU-6 not all met; B-W/B absent; or B-W/A absent without a V-W authorisation | STOPPED BEFORE EXECUTION |
| C7-20 | The informational F-3 run, whatever its outcome | section 10; never a kill or a G.7 receipt |
| C7-21 | `85-C7` offered as (A) | NOT ADMISSIBLE |

**8.2 Independent classifier checks.** The classifier reads only raw output and supervision records. It never relies on the witness's own verdict.
- **CL-1 Harness and process.** No outcome is ever inferred from an exit status alone. Each receipt must pass the process and harness checks. It is then classified only from its printed content.
  - *Process, for every receipt:*
    - a normal exit, with no signal;
    - `oom` and `oom_kill` both 0;
    - `memory.swap.peak` 0;
    - no supervisor timeout.

    An external termination, a signal, an OOM event or a timeout is **not a completed receipt**: STOP (7.3). For F-3, it is recorded as `INFORMATIONAL F-3: NOT COMPLETED — <event>` (section 10).
  - *Harness, for every receipt:* `running 1 test`; `348 filtered out`; the result line names exactly the selected test; and any failure listed is exactly that test.
  - *(A) and (B):* a normal exit 101 and `1 failed`.
  - *(C):* a normal exit 0 and `1 passed; 0 failed`.
  - (A), (B) and (C) are then judged by G.7 and 8.1.
  - *F-3:* `full_f3` calls `full_exploration` (`model_tests.rs` 4130–4140). That function prints the report, then asserts `report.verdict == Verdict::Pass` at line 4133. This is PROVEN FROM SOURCE at C, and the line is unchanged in W7-SRC under NI-1. A completed F-3 receipt is one of three kinds:

    | Printed report | Observed assertion | Normal exit | Recorded as |
    |---|---|---|---|
    | `outcome: PASS` | none fails; no panic | 0, with `1 passed; 0 failed` | **PASS**, as printed |
    | `outcome: FAIL`, and `FAIL: <P>: <detail>` | fails at line 4133: `failed: {"<P>"}`, `left: Fail`, `right: Pass` | 101, with `1 failed` | **FAIL**, with the printed property `<P>` |
    | `outcome: INCOMPLETE`, and `INCOMPLETE: <reason>. No property is established by this run.` | as the harness shows it. The source predicts a failure at line 4133 with `failed: {}`, `left: Incomplete`, `right: Pass` | as the harness shows it: 101 if that assertion failed | **INCOMPLETE**, preserved |

    - **F3-1.** The outcome is the report's `outcome:` line. A FAIL's property is the property on its `FAIL:` line. When the assertion fails, its message repeats the report. Both copies must agree, and so must the `failed:` set and the `left:` operand.
    - **F3-2.** The exit must agree with the observed assertion: 0 exactly when no assertion failed, and 101 exactly when line 4133's assertion failed.
    - **F3-3.** A FAIL is recorded with its printed property, whatever that property is. An INCOMPLETE stays INCOMPLETE, with its reason, even when it exits 101. It is never recorded as a FAIL.
    - **F3-4.** Any other combination is not a completed receipt. It is recorded as `INFORMATIONAL F-3: NOT COMPLETED — inconsistent receipt: <reason>`, and the mission STOPS (7.3). Examples:
      - exit 101 with a PASS report;
      - exit 0 with a FAIL or INCOMPLETE report;
      - a missing, repeated or contradictory `outcome:` line;
      - copies of the report that disagree;
      - a panic at another location;
      - any exit other than 0 or 101.
    - Section 10 is unchanged:
      - F-3 stays information only. It is never a kill and never a G.7 receipt.
      - Any completed outcome other than FAIL E11 is listed separately for the Architect's review.
- **CL-2 Step records.** Exactly 24 per-step lines, numbered 01 to 24 in order.
  - Actor, offered, consumed (with option counts), unused, faults left and the fault flag equal Table A for steps 1–22.
  - Steps 23 and 24 equal the planned end for the receipt's role.
  - Branch labels match the frozen ones.
- **CL-3 Violation lines.** Every line containing `VIOLATION ` in REPLAY or SUMMARY parses to the frozen form:
  - one of the four loci;
  - a property in `PROPERTIES` (3694–3696);
  - a non-empty detail.
- **CL-4 Exact set.**
  - (B)'s canonical records are exactly the two frozen lines, in order, with the source detail strings.
  - (C) has no line containing `VIOLATION ` anywhere.
- **CL-5 Canonical records: counted once; duplicates rejected.**
  - The canonical records are exactly the REPLAY lines that contain `VIOLATION `. Each must lie in its locus block (4.7A).
  - Each canonical record is one observation. Nothing else is an observation, or counts toward any requirement:
    - not a SUMMARY copy;
    - not a marker;
    - not any other mention of a property name, for example in the test path, a per-step note, a state line, M2 to M4, H1, a failure marker or harness text.
  - In (B), each of these is **rejected** (NOT ADMISSIBLE; kills nothing):
    - a third canonical record;
    - a second canonical record of E11 or of P6, at any locus, the same locus included;
    - E11 or P6 at any locus other than its frozen one;
    - a canonical record outside its locus block;
    - any line containing `VIOLATION ` outside REPLAY and SUMMARY.
- **CL-6 Panic summary and counts.**
  - SUMMARY is exactly the canonical records, byte for byte and in the same order. Here that is the two frozen lines, and nothing else. An extra, missing, reordered or altered line is NOT ADMISSIBLE.
  - Every count is a count of canonical records. M1's n = the number of canonical records = the number of SUMMARY lines = 2.
  - So an admissible (B) has four raw lines containing `VIOLATION `, and records two violations.
- **CL-7 Markers agree with the canonical records.**
  - In (B), M1 to M4 lie only in MARKERS, once each, in order. No marker text appears in REPLAY, SUMMARY or TAIL.
  - In (C), H1 appears once, in REPLAY, and no marker appears. H1 never appears in (B).
  - M1 is admissible only with n = 2.
  - M2 is OBSERVED exactly when all of these hold: the canonical record `after 24: P6` exists; step 23 took the changed end; and the lost state lines hold (CL-9).
  - M3 is OBSERVED exactly when the canonical record `step 13 transition: E11` exists.
  - M4's list is the set of properties of the canonical records, minus P6. It is admissible only as `["E11"]`.
  - A marker is NOT ADMISSIBLE (C7-10) if it says OBSERVED without its canonical record, or NOT OBSERVED with one.
- **CL-8 Healthy versus mutant.** Step 23's end is the changed end in (B) and the healthy end in (C). Step 24 pairs with it. G.9 holds in both.
- **CL-9 State lines at the loci.** These are read from the 4.6 prefixes.

  | Line | (B), mutant | (C), healthy |
  |---|---|---|
  | `refusal table after 13` / `after 24` | `{}` | `{F1: MissingPrepared}` |
  | `classes after 24` | F1 `Paid` | F1 `Conflict` |
  | `conflicted after 24` | F1 `true` | F1 `true` |
  | `ledger after 24` | contains `Fact(0)`; lacks `Explanation(0)` | lacks `Fact(0)` |

  In (C), X is also empty after step 24.
- **CL-10 Executable identities.**
  - (A), (B) and F-3 use the same path and E-W7's pinned sha256.
  - (C) uses E-W.
  - The stale-artifact inequalities of 0A hold.
- **CL-11 Worktree identity.**
  - Around (A), (B) and F-3, the mutant worktree is `EXACTLY_W7`: W-TREE, plus the mutant `protocol.rs` only.
  - Around (C), the healthy worktree is `EXACTLY_W`.
- **CL-12 Fail-closed.** Any CL failure makes the receipt NOT ADMISSIBLE. It kills nothing.

## 9. Historical evidence treatment

- **The C7 record stays sealed.** `85-C7`, `80-STOP-RECORD.md` and the C7 rehearsal fixtures are unchanged. `NOT A KILL EVIDENCE CANDIDATE` and `MUTATION_CONTROL_BLOCKED` remain that mission's results, and the Architect's ruling text is unchanged.
- **The historical executable and worktree are preserved.** The C7 executable `b320977a…` (8,984,592 bytes) and the locked worktree `nexus-os-p3-c11-mt3c-C7` are not reused, rebuilt, unlocked or deleted.
- **How old receipts are cited:**
  - `85-C7` is cited only as the source of steps 1–13 (mirrored) and as G.7(A)'s prediction. It is never G.7 evidence.
  - `84-C6` is cited only as the source of steps 15–24. C6's acceptance is unchanged.
- **Nothing is changed or regenerated:** Candidate C, blob `28085ce2`, the catalog and its patches, AD-1-R4, AD-1-R5 and their adoption records, and every prototype and acceptance record of C1, C25 and C4.

Draft-01, Draft-02 and ERRATA-01 remain unadopted history.

## 10. Informational F-3

This preserves AD-1-R4 F ("C7 without A4", line 748): "The run is kept and its execution does not move… not required to kill… Nothing waives the comparison."

**When it runs.** Exactly once, after (C) PASS, (A) admissible E11 and (B) admissible, and before restoration.

**Executable.** The unchanged E-W7, with its sha256 equal before and after, and equal to (A)'s and (B)'s. It runs `full_f3`: the original explorer, F-3, A4 off, under the 30,000,000 cap.

**Its own receipt.** It has its own receipt directory, holding:
- descriptor, raw output, supervision record, CHECKS and CLASSIFICATION;
- the label `INFORMATIONAL — AD-1-R4 F (line 748) — NOT A KILL RECEIPT — NOT A G.7 RECEIPT`.

The comparison side links `43-full-F-3` (C-BASE on E-C) under the same B-W condition as (D). If V-W is authorised, the comparison may instead use `gc_symmetry_f3` on E-W.

**How each outcome is recorded:**
- **FAIL E11, within the ceiling:** `INFORMATIONAL F-3: FAIL E11 (as expected)`.
- **FAIL of another property:** recorded verbatim and flagged for the Architect. It is never counted as P6, even if it prints P6. Section 1.4 makes no claim about F-3.
- **PASS:** recorded as printed and flagged, because it differs from the expected E11.
- **INCOMPLETE** (state cap, or the explorer's own time cap): recorded as it is (E.4 rule 3) and reported.
- **External kill, OOM, signal or supervisor timeout:** `INFORMATIONAL F-3: NOT COMPLETED — <event>`, and the mission STOPS (7.3).
- **Not run because G.7 stopped earlier:** `INFORMATIONAL F-3: NOT MADE — G.7 stopped at <stage>`. This is not an omission.

**Effect on publication and Architect review:**
- With any admissible informational receipt (FAIL, PASS or INCOMPLETE), the maximum control result stays `KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING`. Any deviation from E11 is listed as an Architect review item.
- With `NOT COMPLETED`, the result is `MUTATION_CONTROL_BLOCKED — INFORMATIONAL F-3 NOT COMPLETED`, **never a kill candidate**, even when (A) to (C) are admissible. F's "kept" requirement is unmet.
- Evidence is published only if the execution mission authorises it.
- F-3 is never promoted to a P6 kill.

## 11. Outstanding proof obligations

1. The 24-step schedule is unexecuted (STATIC INFERENCE — NOT OBSERVED).
2. E2 holding at every step can be shown only at runtime.
3. That (A) on E-W7 prints E11 is NOT ESTABLISHED.
4. That (C) is violation-free is NOT ESTABLISHED.
5. That (B) shows exactly {E11 at step 13's transition, P6 after step 24} is NOT ESTABLISHED.
6. W-PATCH meeting NI-1 to NI-6 and SR-1 to SR-6 belongs to the source mission and the Architect's source review.
7. NI-7 (zero warnings) and NI-8 (inventory) are execution evidence.
8. **B-W has three separate states (6.3).** They are: the Owner's adoption of CR-B; the Architect's approval of the frozen W-PATCH; and the execution mission's NI-7 and NI-8 receipts. Without all three, G.7(D) cannot hold (6.4).
9. Section 1.4 needs review, and depends on AS-1 (B(1) and N as accepted) and on B-W for W7-SRC.
10. Assumptions A-1 to A-3 are assumptions, not proofs.
11. Whether an N addition review was recorded for the C4 append at C is NOT ESTABLISHED (6.3 precedent).
12. The F-3 outcome is unknown.
13. Minimality is not claimed.
14. The explorer time cap and the supervisor ceiling coincide. The race is handled as a stop; no claim is made about it.

## 12. Owner adoption gate

**12.1 Authority and evidence.**
- The Owner's approval is the authority.
- An adoption record is audit evidence of that approval. It is never authority itself.
- A repository file that claims AD-C7-W is adopted never passes 13.3 step 1 on its own, whatever its path, content, author or commit. "A string is not authority."

**12.2 Precedent.** This follows AD-1-R4's adoption:
- The Sole Architect conveyed the Owner's decision in mission `P3-C11-DR01-D2A1-CTM01-CANDIDATE-BASELINE-01`, section 1.
- Under that mission, the executor wrote `docs/evidence/p3-candidate11/ad1r4-adoption/OWNER-ADOPTION-2026-10-14.md`. It was committed in `d5964ee5`, as blob `54c956bc1324dd3a8fd97439faa815f0865b404d`, with sha256 `105e087137fed4ed7bf2baaab50b06812a1847ddbe9549809b85eecbdae379c6`.
- That record names the conveying mission and its section, and the adopted document's path, review commit, git blob and sha256. It states "A string is not authority" and claims no signature.

**12.3 Sequence and authentication**
1. **Architect review.** The Sole Architect reviews the draft and rules: accept, repair or reject.
   - The ruling covers CR-B as text, and NI-1 to NI-8 and SR-1 to SR-6 as criteria.
   - It is not B-W/B, because no W-PATCH exists yet.
2. **Freeze.** A separately authorised write freezes the text as a file, recording its path, git blob and `AD-C7-W-SHA256` (K2). OVERLAY-C7 is extracted mechanically, and its `OVERLAY-SHA256` is recorded (K3).
3. **AU-1 — explicit Owner approval.**
   - The Owner's approval names: K2 exactly (path, git blob and `AD-C7-W-SHA256`); K3; and the scope "C7 only".
   - It also states, separately, whether CR-B is adopted (B-W/A). If the approval is silent on CR-B, CR-B is not adopted.
   - None of these counts as approval of AD-C7-W: approval of an unfrozen text; of "the latest draft"; of a description; or of any identity other than K2.
4. **AU-2 — conveyance.**
   - The approval reaches the executor only in a Primary Architect mission that is separately authorised to convey it, as CTM01 section 1 was for AD-1-R4.
   - That mission states the approval, the identities of AU-1, and K1 and K5.
   - No drafting, errata, source or execution mission conveys the approval unless it is so authorised.
5. **AU-3 — the record.** Under the conveying mission, the executor writes the record in AD-1-R4's form. The record:
   - states "A string is not authority", and claims no signature;
   - names the conveying mission and section; K1, K2, K3 and K5; the explicit Owner-decision details of AU-1; the scope, C7 only; and CR-B's status;
   - does not contain or claim its own final commit, git blob or sha256 (AU-5);
   - is committed on a review ref.

   Once it is committed, its commit, path, git blob and sha256 are K4. K4 is pinned only after that commit, by a subsequent Primary Architect-authorised mission, and is never taken from the record's own text (AU-4, AU-5).
6. **AU-4 — authentication.** After the record is committed, a subsequent Primary Architect-authorised mission pins K4: the record's commit, path, git blob and sha256 (AU-5). The execution mission names that pinned K4 and the conveying mission. At 13.3 step 1, the executor authenticates the record against the actual git object, and checks that:
   - the commit exists, and is reachable from the named remote review ref, with local and remote equal;
   - the path exists in that commit;
   - its blob equals K4's blob;
   - its recomputed sha256 equals K4's sha256.
7. **AU-5 — agreement.**
   - K1, K2, K3 and K5 each have one value, and it is equal in four places: the conveying mission; the record; the execution mission; and recomputation from git objects (K3 by mechanical extraction from K2).
   - The adoption record contains K1, K2, K3, K5, the conveying mission identity, the explicit Owner-decision details, the C7-only scope and the CR-B adoption status.
   - It does not contain or claim its own final commit, blob or SHA256.
   - After the adoption record is committed, a subsequent Primary Architect-authorized mission pins K4 and authenticates the record against the actual Git object, its path and the named remote review ref.
   - K4's commit, path, blob and SHA256 must agree with that independently retrieved Git object and with the execution mission's pinned identity.
   - No self-reference or self-attestation establishes Owner authority.
   - Without the Owner's explicit approval, the conveying mission and the authenticated record, execution is refused.
   - CR-B's status is equal in the conveying mission and in the record.
8. **AU-6 — fail closed.** Unless AU-1 to AU-5 all hold, the result is **STOPPED BEFORE EXECUTION**. Any one of these is enough to stop:
   - a record with no conveying mission;
   - a conveying mission with no matching record;
   - any one differing identity.

**12.4 Adoption approves, for C7 only:**
- G.7, G.8 and G.9;
- I-C7, and NI-1 to NI-8 and SR-1 to SR-6, as criteria;
- CR-B, only if the Owner's approval says so (B-W/A);
- OVERLAY-C7, and P-1 to P-6;
- 6A and its disposition;
- section 7, K.2-C7 and section 10;
- S-C7-1 to S-C7-18.

**12.5 Adoption does not:**
- accept any C7 kill;
- authorise source authoring, a build, a run, a worktree, a commit or publication;
- change the catalog's bytes, the 34 accepted controls or A4;
- accept D2A-1, integrate anything, or complete Phase Three;
- establish B-W/B or B-W/C, or show that any W-PATCH, authored or not, meets NI-1 to NI-8 or SR-1 to SR-6;
- adopt CR-B, unless the Owner's approval says so explicitly.

**12.6** Neither the drafting missions, the errata mission nor the consolidation mission that produced this Draft-03 request or presume Owner approval.

## 13. Later execution-authorisation gate

**13.1 A separate witness-source mission comes first.** It:
- authors W-PATCH under NI and SR;
- checks NI-1 to NI-4, and NI-6's build-input blobs, mechanically;
- builds only if that mission says so. Such a build never discharges B-W (O-4);
- freezes W-COMMIT, W-TREE, W-BLOB and the W-PATCH sha256, and publishes them on a review ref.

Only after that does the Architect review that frozen identity (B-W/B, 6.3).
- The approval reaches the executor only in an Architect mission that names the identity.
- A repository record of the approval is audit evidence, not authority.

**13.2 The execution mission must name:**
- K1 to K5, including K4's commit, path, git blob and sha256;
- the conveying mission (AU-2);
- the W-SRC identity, and the mission that conveyed B-W/B for it;
- CR-B's status, or a V-W authorisation;
- the section 7 limits;
- the worktree names and publication rules;
- a maximum result of `KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING`.

**13.3 Order**
1. **Authorisation gate.**
   - Authenticate C-BASE (unrelaxed), the catalog, AD-1-R4, AD-1-R5 and the frozen AD-C7-W.
   - Meet AU-1 to AU-6 (12.3). This includes authenticating the adoption record against the actual git object, its path and the named remote review ref, and checking that K4 agrees with that object and with the K4 this mission names (AU-4, AU-5). The record never supplies its own K4.
   - Derive EC7, and check P-2, P-3 and P-6.
   - Check B-W/A, and B-W/B for the exact W-SRC identity this mission names. Alternatively, check a V-W authorisation together with B-W/B.
   - NI-7 and NI-8 are not checked here (O-3).
   - Any failure: **STOPPED BEFORE EXECUTION**.
2. Authenticate W-SRC against B-W/B's identity. Run the NI mechanical checks and the source inventory counts.
3. In a fresh healthy worktree, build E-W (one build), pin it, and run its listings (6A).
4. In a fresh mutant worktree, apply the patch and verify W7-SRC by blob. Build E-W7 (one build), pin it, and run its listings.
5. Check NI-7 and NI-8. Any failure: STOP (7.3, O-5), before any test run.
   - NI-7: both builds have zero warnings and zero errors, and NI-6's build-time clause holds.
   - NI-8: 6A's four-way agreement holds.
6. Record B-W/C (O-6).
7. Run (C).
8. Run (A).
9. Run (B).
10. Run F-3 (section 10).
11. Restore to `EXACTLY_W`.
12. Make the (D) link, citing the B-W/C record.
13. Run the independent classification (8.2), record the results, and publish only if authorised.

Under an authorised V-W, steps 6 and 12 follow 6.4 and that authorisation.

*Order change from Draft-02:* E-W7's build and listings now come before (C).
- Draft-02 ran (C) before building E-W7.
- But 6A stops the mission "before any run" on any listing disagreement, and B-W/C needs E-W7's listings.
- The run order is unchanged: (C), then (A), (B) and F-3. The healthy witness still runs first.

**13.4 Stops.** As in 7.3.

**13.5 After execution.** Only the Architect rules on C7. The campaign count changes only by that ruling.

**Open items for the Architect:**
- the designation;
- the prototype gate;
- whether (A) requires only the printed E11;
- the healthy-first order;
- B-W;
- section 1.4;
- the F-3 timing and its effect on the result;
- OD-6, OD-8 and OD-12 (Annex A).

---

## Annex A — Owner-adoption decisions (not amendment text)

Carried from Draft-02's response section 4, with ERRATA-01 E1.9 applied to OD-3 and OD-4, and OD-2 aligned with 12.4 (P-6, which ERRATA-01 added). They are the decisions requested of the Architect and the Owner. They are not amendment text, and this draft decides none of them.

- **OD-1.** Adopt AD-C7-W as a C7-only addendum, under AD-1-R4's record form.
- **OD-2.** Adopt OVERLAY-C7, P-1 to P-6, and the K1 to K5 binding.
- **OD-3.** Accept NI-1 to NI-8 and SR-1 to SR-6 as CR-B's criteria, and as the N addition-review criteria. They are applied:
  - to the frozen W-PATCH at B-W/B: NI-1 to NI-6, and SR-1 to SR-6;
  - to its builds and listings at B-W/C: NI-7 and NI-8.
- **OD-4.** Decide whether CR-B is put to the Owner for adoption (B-W/A).
  - Without it, G.7(D) cannot hold except under a separately authorised V-W, which still needs B-W/B.
  - If the NI or SR criteria are rejected, the design is blocked.
- **OD-5.** Accept the module and test path, the 349/35/348 expectations, and the `C7_WITNESS_W_SRC_ONLY` disposition.
- **OD-6.** Accept the reading of "no previously accepted test is rerun" (6A).
- **OD-7.** Accept the F-3 rule: NOT COMPLETED prevents a kill candidate; INCOMPLETE, PASS or another FAIL does not.
- **OD-8.** Accept that any third violation makes (B) NOT ADMISSIBLE.
- **OD-9.** Decide whether G.7(A) requires only the printed E11, or also the historical trace and count.
- **OD-10.** Accept the healthy-first order.
- **OD-11.** Decide whether the source mission needs a prototype gate.
- **OD-12.** Confirm that the catalog's top-level `baseline` (C-BASE) does not need to change for a W7-SRC primary. This draft relies on contract-level S-C7-9 for that, because R2 does not let the overlay touch `baseline`. If the Architect disagrees, the result is DESIGN BLOCKED on that point.
- **OD-13.** Accept the stale-artifact inequality checks.

## Annex B — Points needing explicit Architect acceptance (not amendment text)

Carried verbatim from ERRATA-01 section 4. In this annex, "the errata" means ERRATA-01, and labels such as "Repair 1", E1.2, E1.5 and E3 name ERRATA-01's repairs and replacement items. RECONCILIATION section 3 maps each item to its place in this draft.

These points are deliberate and need explicit acceptance:

1. **13.3 order change (E1.5).** E-W7's build and listings move before (C).
   - Without the move, Draft-02 contradicts itself: 6A requires E-W7's listings to agree before any run, but Draft-02's 13.3 ran (C) before building E-W7.
   - The run order, healthy first, is unchanged (OD-10).
2. **NI-6 checked at two times (E1.2).** Repair 1 places NI-6 at B-W/B, but NI-6's build-command, environment and `rustc -vV` clause exist only at the build. The errata therefore check that clause with NI-7, at B-W/C. The build-input blobs stay at B-W/B.
3. **F3-4 (E3).** An internally inconsistent F-3 receipt is treated as NOT COMPLETED: STOP, and MUTATION_CONTROL_BLOCKED.
   - Section 10 named only external events for NOT COMPLETED. This is a fail-closed extension.
   - The INCOMPLETE row's exit is stated only "as the harness shows it". Exit 101 is a prediction from source (line 4133 asserts PASS), not a requirement.
4. **The 4.7A boundary forms are EXECUTOR-REPORTED** from the C4 receipts under rustc 1.94.0: the harness prefix, the panic line with its thread id, and the `note:` line.
   - If E-W7 prints them differently, (B) is NOT ADMISSIBLE and the mission stops.
   - They are not re-derived from libtest's source.
5. **New precision in (B):**
   - M1 to M4 must appear in that order (4.8 listed them in that order, but did not require it);
   - canonical records must lie inside their own locus blocks.
6. **AU-2's reading of "separately authorised".** A mission may convey the approval only if it is expressly authorised to do so. As at CTM01, that mission may also carry other authorised work. The Architect may instead require a conveyance-only mission.
7. **Superseded response text.** Draft-02's own response sections "2. Exact changes from Draft-01" and "3. Remaining unresolved proof obligations" still say "B-W: without it, G.7(D) is BLOCKED". They are history, not amendment text, and E1 supersedes them.
8. **Pre-existing and unchanged:** V-W's timing within 13.3 is still set only by its own authorisation.

---

**Status of this text: DRAFT-03 — UNADOPTED — NOT EXECUTION AUTHORITY.** Architect review is pending. No Owner approval is requested or presumed. No mutation control is accepted. The campaign remains **34/35**, and C7 remains **MUTATION_CONTROL_BLOCKED**.

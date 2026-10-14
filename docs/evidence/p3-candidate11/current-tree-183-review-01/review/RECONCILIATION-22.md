# The 22 stale entries: reconciliation records

Mission P3-C11-DR01-D2A1-CTM01-CANDIDATE-BASELINE-01. Contract basis: AD-1-R4 H.4.

**Every entry: ARCHITECT_REVIEW_PENDING.** The labels below are the executor's static analysis (EXECUTOR_ANALYSIS_MATCH or EXECUTOR_ANALYSIS_DIFFERENCE). None is a SAME PROPERTY finding, and none is acceptance: that review belongs to the Sole Architect. No build, test or mutation was run. A string is not authority.

Facts source: `receipts/09-reconcile-22-facts.txt` (raw output of `tools/reconcile-22.py`), whose first line is:

```text
validated commit d5964ee5326c2733177307db6f507328023d361a; ct-1 sha256 20bcc6fe221238f81246348cbbe94636a3c9b6d8acf89901d230e091f1a44279; first-known-stale receipt 8693e3a7:docs/evidence/p3-candidate11/ad1r2-review-packet-01/evidence/p3-c11-dr01-d2a1-campaign-01/receipts/final/01-mutation-manifest-stale-hashes.txt (blob 8b0654b448f69a8a8ef84f60f664934b6c4e5170)
```

Terms. **Recorded**: the source sha256 the historical manifest records for the file. **H**: the last commit at which the file still has the recorded hash, the parent of the **transition commit**, the first commit (path-limited history of the validated commit) after which it no longer does; derived by `reconcile-22.py`, which asserts that H holds the recorded hash. **First-known stale (existing evidence)**: the receipt named in the facts header, which sampled five commits; it is cited, not re-derived. **Now**: the validated commit. Lines are 1-based start lines of the anchor. "Context" compares the anchor's +/-20 lines at H and now. Function identity is by name and rustfmt layout (from the `fn` line to the first later line that is its indentation followed by `}`).

## Summary

| Control | Set | File | Transition | Anchor line H → now | Definition byte-equal | Expected tests byte-identical | Executor analysis |
|---|---|---|---|---|---|---|---|
| M32 | D2A-1 | `app/src-tauri/src/phase3_tests.rs` | `22ebf9a5c5b1` | [4665] → [4665] | yes | yes | EXECUTOR_ANALYSIS_MATCH |
| M78 | D2A-1 | `app/src-tauri/src/phase3_tests.rs` | `22ebf9a5c5b1` | [5025] → [5025] | yes | yes | EXECUTOR_ANALYSIS_MATCH |
| M79 | D2A-1 | `app/src-tauri/src/phase3_tests.rs` | `22ebf9a5c5b1` | [5123] → [5123] | yes | yes | EXECUTOR_ANALYSIS_MATCH |
| M80 | D2A-1 | `app/src-tauri/src/phase3_tests.rs` | `22ebf9a5c5b1` | [4697] → [4697] | yes | yes | EXECUTOR_ANALYSIS_MATCH |
| M87 | D2A-1 | `crates/nexus-governed-control/src/lib.rs` | `22ebf9a5c5b1` | [50] → [50] | yes | yes | EXECUTOR_ANALYSIS_MATCH |
| M89 | D2A-1 | `app/src-tauri/src/phase3_tests.rs` | `22ebf9a5c5b1` | [2874] → [2874] | yes | yes | EXECUTOR_ANALYSIS_MATCH |
| M112 | D2A-1 | `app/src-tauri/src/phase3_tests.rs` | `22ebf9a5c5b1` | [4591] → [4591] | yes | yes | EXECUTOR_ANALYSIS_MATCH |
| M113 | D2A-1 | `app/src-tauri/src/phase3_tests.rs` | `22ebf9a5c5b1` | [4653] → [4653] | yes | yes | EXECUTOR_ANALYSIS_MATCH |
| M141 | D2A-1 | `app/src-tauri/src/phase3_tests.rs` | `22ebf9a5c5b1` | [5048] → [5048] | yes | yes | EXECUTOR_ANALYSIS_MATCH |
| M143 | D2A-1 | `app/src-tauri/src/phase3_tests.rs` | `22ebf9a5c5b1` | [4562] → [4562] | yes | yes | EXECUTOR_ANALYSIS_MATCH |
| M153 | D2A-1 | `app/src-tauri/src/phase3_tests.rs` | `22ebf9a5c5b1` | [4596] → [4596] | yes | yes | EXECUTOR_ANALYSIS_MATCH |
| M177 | D2A-1 | `app/src-tauri/src/phase3_tests.rs` | `22ebf9a5c5b1` | [8908] → [8908] | yes | yes | EXECUTOR_ANALYSIS_MATCH |
| M178 | D2A-1 | `app/src-tauri/src/phase3_tests.rs` | `22ebf9a5c5b1` | [8910] → [8910] | yes | yes | EXECUTOR_ANALYSIS_MATCH |
| M179 | D2A-1 | `app/src-tauri/src/phase3_tests.rs` | `22ebf9a5c5b1` | [8991] → [8991] | yes | yes | EXECUTOR_ANALYSIS_MATCH |
| M180 | D2A-1 | `app/src-tauri/src/phase3_tests.rs` | `22ebf9a5c5b1` | [8919] → [8919] | yes | yes | EXECUTOR_ANALYSIS_MATCH |
| M18 | earlier Candidate 11 | `crates/nexus-governed-control/src/broker.rs` | `862a11cef284` | [350] → [403] | yes | yes | EXECUTOR_ANALYSIS_DIFFERENCE |
| M19 | earlier Candidate 11 | `crates/nexus-governed-control/src/broker.rs` | `862a11cef284` | [372] → [425] | yes | yes | EXECUTOR_ANALYSIS_MATCH |
| M39 | earlier Candidate 11 | `crates/nexus-governed-control/src/broker.rs` | `862a11cef284` | [108] → [120] | yes | yes | EXECUTOR_ANALYSIS_MATCH |
| M84 | earlier Candidate 11 | `crates/nexus-governed-control/src/authority/approval.rs` | `6be32bf43ceb` | [191] → [194] | yes | yes | EXECUTOR_ANALYSIS_MATCH |
| M05 | earlier Candidate 11 | `crates/nexus-governed-control/src/authority/mod.rs` | `6be32bf43ceb` | [312] → [335] | yes | yes | EXECUTOR_ANALYSIS_MATCH |
| M45 | earlier Candidate 11 | `crates/nexus-governed-control/src/authority/run.rs` | `6be32bf43ceb` | [190] → [376] | yes | yes | EXECUTOR_ANALYSIS_MATCH |
| M129 | earlier Candidate 11 | `crates/nexus-governed-control/src/authority/run.rs` | `6be32bf43ceb` | [151] → [229] | yes | yes | EXECUTOR_ANALYSIS_MATCH |

Labels: EXECUTOR_ANALYSIS_DIFFERENCE 1, EXECUTOR_ANALYSIS_MATCH 21. Review of all 22: ARCHITECT_REVIEW_PENDING.

## M32 — a raw system call is not a mechanism

| Field | Value |
|---|---|
| Set | D2A-1 |
| Category | bypass guard |
| Target file | `app/src-tauri/src/phase3_tests.rs` |
| Recorded (historical) sha256 | `2d131e17bef88dcb68a7b31f2b8bf06c7e2b9c3e2543e6af8a02a120c327fdae` |
| Current sha256 | `b7e0b7493fa0ce839782a1297d945184ac0d59dc6fdb002a16261199d66ab0b8` |
| Current git blob | `5d79c7c38b04a7ea480e144850cfb1a1e38d1e83` |
| First-known stale (existing evidence) | D2A-1 commit 22ebf9a5c5b1 |
| Transition commit (derived) | `22ebf9a5c5b16bb6f5169daf3731a3417c5289d3` |
| H (derived; holds the recorded hash) | `b1d535f1bb634f2075f12a36ace89a595cdb13ba` |
| Path history (recorded hash or other, at each change) | `5d9a2cafe4de other`, `8cd813a3f4ef REC`, `22ebf9a5c5b1 other` |
| File diff H → now | 1 file changed, 239 insertions(+) |
| Definition byte-equal to historical | id: yes, category: yes, historical_meaning: yes, candidate10_meaning: yes, historical: yes, edits: yes, command: yes, cwd: yes, expected_failure_tests: yes |
| Anchor (edit 1) | expected count 1; count at H 1, lines [4665]; count now 1, lines [4665]; cardinality matches |
| Anchor (edit 1) surroundings | enclosing function now `effect_kinds`, at H `effect_kinds`; text identical: yes; +/-20-line context identical: yes; replacement is a deletion |
| Expected failing test | `phase3_tests::p3_g6_08_every_real_world_mechanism_in_the_workspace_is_classified`: selected by the command's filter: yes; defined in `app/src-tauri/src/phase3_tests.rs` (at H: `app/src-tauri/src/phase3_tests.rs`); function text identical H → now: yes |
| Executor analysis | **EXECUTOR_ANALYSIS_MATCH** |
| Review | **ARCHITECT_REVIEW_PENDING** |

The file's only change since H is one hunk appended after the file's last line (line 8999 at H) by D2A-1 22ebf9a5: the new test p3_c11_d2a1_the_ledger_protocol_core_is_confined. No line that existed at H is changed. The anchor is at the same line at H and now; its enclosing item (where one is detected) and every expected failing test's function text are byte-identical; the appended test's name is matched by none of this control's cargo filters. D2A-1 also added the production sources ledger.rs and ledger/protocol.rs, which guards that read workspace sources read in the healthy and the mutant run alike; no run establishes those guards' verdicts at this tree. The control still deletes the same classification line in effect_kinds.

## M78 — a shadowed mechanism is invisible to the inventory

| Field | Value |
|---|---|
| Set | D2A-1 |
| Category | bypass guard |
| Target file | `app/src-tauri/src/phase3_tests.rs` |
| Recorded (historical) sha256 | `2d131e17bef88dcb68a7b31f2b8bf06c7e2b9c3e2543e6af8a02a120c327fdae` |
| Current sha256 | `b7e0b7493fa0ce839782a1297d945184ac0d59dc6fdb002a16261199d66ab0b8` |
| Current git blob | `5d79c7c38b04a7ea480e144850cfb1a1e38d1e83` |
| First-known stale (existing evidence) | D2A-1 commit 22ebf9a5c5b1 |
| Transition commit (derived) | `22ebf9a5c5b16bb6f5169daf3731a3417c5289d3` |
| H (derived; holds the recorded hash) | `b1d535f1bb634f2075f12a36ace89a595cdb13ba` |
| Path history (recorded hash or other, at each change) | `5d9a2cafe4de other`, `8cd813a3f4ef REC`, `22ebf9a5c5b1 other` |
| File diff H → now | 1 file changed, 239 insertions(+) |
| Definition byte-equal to historical | id: yes, category: yes, historical_meaning: yes, candidate10_meaning: yes, historical: yes, edits: yes, command: yes, cwd: yes, expected_failure_tests: yes |
| Anchor (edit 1) | expected count 1; count at H 1, lines [5025]; count now 1, lines [5025]; cardinality matches |
| Anchor (edit 1) surroundings | enclosing function now `file_effect_sites`, at H `file_effect_sites`; text identical: yes; +/-20-line context identical: yes; replacement already occurs 0 time(s) now |
| Expected failing test | `phase3_tests::p3_g6_12_the_inventory_fails_closed`: selected by the command's filter: yes; defined in `app/src-tauri/src/phase3_tests.rs` (at H: `app/src-tauri/src/phase3_tests.rs`); function text identical H → now: yes |
| Executor analysis | **EXECUTOR_ANALYSIS_MATCH** |
| Review | **ARCHITECT_REVIEW_PENDING** |

The file's only change since H is one hunk appended after the file's last line (line 8999 at H) by D2A-1 22ebf9a5: the new test p3_c11_d2a1_the_ledger_protocol_core_is_confined. No line that existed at H is changed. The anchor is at the same line at H and now; its enclosing item (where one is detected) and every expected failing test's function text are byte-identical; the appended test's name is matched by none of this control's cargo filters. D2A-1 also added the production sources ledger.rs and ledger/protocol.rs, which guards that read workspace sources read in the healthy and the mutant run alike; no run establishes those guards' verdicts at this tree.

## M79 — a latent class names no entries

| Field | Value |
|---|---|
| Set | D2A-1 |
| Category | bypass guard |
| Target file | `app/src-tauri/src/phase3_tests.rs` |
| Recorded (historical) sha256 | `2d131e17bef88dcb68a7b31f2b8bf06c7e2b9c3e2543e6af8a02a120c327fdae` |
| Current sha256 | `b7e0b7493fa0ce839782a1297d945184ac0d59dc6fdb002a16261199d66ab0b8` |
| Current git blob | `5d79c7c38b04a7ea480e144850cfb1a1e38d1e83` |
| First-known stale (existing evidence) | D2A-1 commit 22ebf9a5c5b1 |
| Transition commit (derived) | `22ebf9a5c5b16bb6f5169daf3731a3417c5289d3` |
| H (derived; holds the recorded hash) | `b1d535f1bb634f2075f12a36ace89a595cdb13ba` |
| Path history (recorded hash or other, at each change) | `5d9a2cafe4de other`, `8cd813a3f4ef REC`, `22ebf9a5c5b1 other` |
| File diff H → now | 1 file changed, 239 insertions(+) |
| Definition byte-equal to historical | id: yes, category: yes, historical_meaning: yes, candidate10_meaning: yes, historical: yes, edits: yes, command: yes, cwd: yes, expected_failure_tests: yes |
| Anchor (edit 1) | expected count 1; count at H 1, lines [5123]; count now 1, lines [5123]; cardinality matches |
| Anchor (edit 1) surroundings | enclosing function now `None`, at H `None`; text identical: n/a; +/-20-line context identical: yes; replacement is a deletion |
| Expected failing test | `phase3_tests::p3_g6_12_the_inventory_fails_closed`: selected by the command's filter: yes; defined in `app/src-tauri/src/phase3_tests.rs` (at H: `app/src-tauri/src/phase3_tests.rs`); function text identical H → now: yes |
| Executor analysis | **EXECUTOR_ANALYSIS_MATCH** |
| Review | **ARCHITECT_REVIEW_PENDING** |

The file's only change since H is one hunk appended after the file's last line (line 8999 at H) by D2A-1 22ebf9a5: the new test p3_c11_d2a1_the_ledger_protocol_core_is_confined. No line that existed at H is changed. The anchor is at the same line at H and now; its enclosing item (where one is detected) and every expected failing test's function text are byte-identical; the appended test's name is matched by none of this control's cargo filters. D2A-1 also added the production sources ledger.rs and ledger/protocol.rs, which guards that read workspace sources read in the healthy and the mutant run alike; no run establishes those guards' verdicts at this tree. The anchor is outside any detected function (a deletion); the +/-20-line context is identical.

## M80 — the kernel's unsealed spawn is not a process

| Field | Value |
|---|---|
| Set | D2A-1 |
| Category | bypass guard |
| Target file | `app/src-tauri/src/phase3_tests.rs` |
| Recorded (historical) sha256 | `2d131e17bef88dcb68a7b31f2b8bf06c7e2b9c3e2543e6af8a02a120c327fdae` |
| Current sha256 | `b7e0b7493fa0ce839782a1297d945184ac0d59dc6fdb002a16261199d66ab0b8` |
| Current git blob | `5d79c7c38b04a7ea480e144850cfb1a1e38d1e83` |
| First-known stale (existing evidence) | D2A-1 commit 22ebf9a5c5b1 |
| Transition commit (derived) | `22ebf9a5c5b16bb6f5169daf3731a3417c5289d3` |
| H (derived; holds the recorded hash) | `b1d535f1bb634f2075f12a36ace89a595cdb13ba` |
| Path history (recorded hash or other, at each change) | `5d9a2cafe4de other`, `8cd813a3f4ef REC`, `22ebf9a5c5b1 other` |
| File diff H → now | 1 file changed, 239 insertions(+) |
| Definition byte-equal to historical | id: yes, category: yes, historical_meaning: yes, candidate10_meaning: yes, historical: yes, edits: yes, command: yes, cwd: yes, expected_failure_tests: yes |
| Anchor (edit 1) | expected count 1; count at H 1, lines [4697]; count now 1, lines [4697]; cardinality matches |
| Anchor (edit 1) surroundings | enclosing function now `effect_kinds`, at H `effect_kinds`; text identical: yes; +/-20-line context identical: yes; replacement already occurs 0 time(s) now |
| Expected failing test | `phase3_tests::p3_g6_08_every_real_world_mechanism_in_the_workspace_is_classified`: selected by the command's filter: yes; defined in `app/src-tauri/src/phase3_tests.rs` (at H: `app/src-tauri/src/phase3_tests.rs`); function text identical H → now: yes |
| Expected failing test | `phase3_tests::p3_g6_12_the_inventory_fails_closed`: selected by the command's filter: yes; defined in `app/src-tauri/src/phase3_tests.rs` (at H: `app/src-tauri/src/phase3_tests.rs`); function text identical H → now: yes |
| Executor analysis | **EXECUTOR_ANALYSIS_MATCH** |
| Review | **ARCHITECT_REVIEW_PENDING** |

The file's only change since H is one hunk appended after the file's last line (line 8999 at H) by D2A-1 22ebf9a5: the new test p3_c11_d2a1_the_ledger_protocol_core_is_confined. No line that existed at H is changed. The anchor is at the same line at H and now; its enclosing item (where one is detected) and every expected failing test's function text are byte-identical; the appended test's name is matched by none of this control's cargo filters. D2A-1 also added the production sources ledger.rs and ledger/protocol.rs, which guards that read workspace sources read in the healthy and the mutant run alike; no run establishes those guards' verdicts at this tree.

## M87 — a compile-fail doctest loses its error code

| Field | Value |
|---|---|
| Set | D2A-1 |
| Category | native approval |
| Target file | `crates/nexus-governed-control/src/lib.rs` |
| Recorded (historical) sha256 | `53c37d97f90ede471aff8cf6da93f9323c6915120626e94554788f45cd5b7f46` |
| Current sha256 | `addad1993f57ad370da7e38719f3e58d26b5a4eb1fbe06b24df51525b135727d` |
| Current git blob | `b806ed577f8d3472b7187d161246e08d2f1ccd3d` |
| First-known stale (existing evidence) | D2A-1 commit 22ebf9a5c5b1 |
| Transition commit (derived) | `22ebf9a5c5b16bb6f5169daf3731a3417c5289d3` |
| H (derived; holds the recorded hash) | `b1d535f1bb634f2075f12a36ace89a595cdb13ba` |
| Path history (recorded hash or other, at each change) | `da1df3dca68f other`, `7e2fda296369 REC`, `22ebf9a5c5b1 other` |
| File diff H → now | 1 file changed, 1 insertion(+) |
| Definition byte-equal to historical | id: yes, category: yes, historical_meaning: yes, candidate10_meaning: yes, historical: yes, edits: yes, command: yes, cwd: yes, expected_failure_tests: yes |
| Anchor (edit 1) | expected count 1; count at H 1, lines [50]; count now 1, lines [50]; cardinality matches |
| Anchor (edit 1) surroundings | enclosing function now `None`, at H `None`; text identical: n/a; +/-20-line context identical: yes; replacement already occurs 0 time(s) now |
| Expected failing test | `phase3_tests::p3_g6_15_the_compile_fail_doctests_are_pinned_with_their_errors`: selected by the command's filter: yes; defined in `app/src-tauri/src/phase3_tests.rs` (at H: `app/src-tauri/src/phase3_tests.rs`); function text identical H → now: yes |
| Executor analysis | **EXECUTOR_ANALYSIS_MATCH** |
| Review | **ARCHITECT_REVIEW_PENDING** |

The file's only change since H (D2A-1 22ebf9a5) is one added line, 'pub(crate) mod ledger;', at line 112. The anchor, a crate-documentation compile_fail fence at line 50, and its +/-20-line context are identical. The expected test p3_g6_15's function text is byte-identical.

## M89 — the scanners treat another feature as test-only

| Field | Value |
|---|---|
| Set | D2A-1 |
| Category | bypass guard |
| Target file | `app/src-tauri/src/phase3_tests.rs` |
| Recorded (historical) sha256 | `2d131e17bef88dcb68a7b31f2b8bf06c7e2b9c3e2543e6af8a02a120c327fdae` |
| Current sha256 | `b7e0b7493fa0ce839782a1297d945184ac0d59dc6fdb002a16261199d66ab0b8` |
| Current git blob | `5d79c7c38b04a7ea480e144850cfb1a1e38d1e83` |
| First-known stale (existing evidence) | D2A-1 commit 22ebf9a5c5b1 |
| Transition commit (derived) | `22ebf9a5c5b16bb6f5169daf3731a3417c5289d3` |
| H (derived; holds the recorded hash) | `b1d535f1bb634f2075f12a36ace89a595cdb13ba` |
| Path history (recorded hash or other, at each change) | `5d9a2cafe4de other`, `8cd813a3f4ef REC`, `22ebf9a5c5b1 other` |
| File diff H → now | 1 file changed, 239 insertions(+) |
| Definition byte-equal to historical | id: yes, category: yes, historical_meaning: yes, candidate10_meaning: yes, historical: yes, edits: yes, command: yes, cwd: yes, expected_failure_tests: yes |
| Anchor (edit 1) | expected count 1; count at H 1, lines [2874]; count now 1, lines [2874]; cardinality matches |
| Anchor (edit 1) surroundings | enclosing function now `cfg_requires_test`, at H `cfg_requires_test`; text identical: yes; +/-20-line context identical: yes; replacement already occurs 0 time(s) now |
| Expected failing test | `phase3_tests::p3_g6_16_test_only_features_are_enabled_only_by_tests`: selected by the command's filter: yes; defined in `app/src-tauri/src/phase3_tests.rs` (at H: `app/src-tauri/src/phase3_tests.rs`); function text identical H → now: yes |
| Executor analysis | **EXECUTOR_ANALYSIS_MATCH** |
| Review | **ARCHITECT_REVIEW_PENDING** |

The file's only change since H is one hunk appended after the file's last line (line 8999 at H) by D2A-1 22ebf9a5: the new test p3_c11_d2a1_the_ledger_protocol_core_is_confined. No line that existed at H is changed. The anchor is at the same line at H and now; its enclosing item (where one is detected) and every expected failing test's function text are byte-identical; the appended test's name is matched by none of this control's cargo filters. D2A-1 also added the production sources ledger.rs and ledger/protocol.rs, which guards that read workspace sources read in the healthy and the mutant run alike; no run establishes those guards' verdicts at this tree.

## M112 — a resolver method call is not a network site

| Field | Value |
|---|---|
| Set | D2A-1 |
| Category | bypass guard |
| Target file | `app/src-tauri/src/phase3_tests.rs` |
| Recorded (historical) sha256 | `2d131e17bef88dcb68a7b31f2b8bf06c7e2b9c3e2543e6af8a02a120c327fdae` |
| Current sha256 | `b7e0b7493fa0ce839782a1297d945184ac0d59dc6fdb002a16261199d66ab0b8` |
| Current git blob | `5d79c7c38b04a7ea480e144850cfb1a1e38d1e83` |
| First-known stale (existing evidence) | D2A-1 commit 22ebf9a5c5b1 |
| Transition commit (derived) | `22ebf9a5c5b16bb6f5169daf3731a3417c5289d3` |
| H (derived; holds the recorded hash) | `b1d535f1bb634f2075f12a36ace89a595cdb13ba` |
| Path history (recorded hash or other, at each change) | `5d9a2cafe4de other`, `8cd813a3f4ef REC`, `22ebf9a5c5b1 other` |
| File diff H → now | 1 file changed, 239 insertions(+) |
| Definition byte-equal to historical | id: yes, category: yes, historical_meaning: yes, candidate10_meaning: yes, historical: yes, edits: yes, command: yes, cwd: yes, expected_failure_tests: yes |
| Anchor (edit 1) | expected count 1; count at H 1, lines [4591]; count now 1, lines [4591]; cardinality matches |
| Anchor (edit 1) surroundings | enclosing function now `None`, at H `None`; text identical: n/a; +/-20-line context identical: yes; replacement already occurs 0 time(s) now |
| Expected failing test | `phase3_tests::p3_g6_04_the_mechanisms_are_confined_to_their_modules`: selected by the command's filter: yes; defined in `app/src-tauri/src/phase3_tests.rs` (at H: `app/src-tauri/src/phase3_tests.rs`); function text identical H → now: yes |
| Expected failing test | `phase3_tests::p3_g6_08_every_real_world_mechanism_in_the_workspace_is_classified`: selected by the command's filter: yes; defined in `app/src-tauri/src/phase3_tests.rs` (at H: `app/src-tauri/src/phase3_tests.rs`); function text identical H → now: yes |
| Expected failing test | `phase3_tests::p3_g6_12_the_inventory_fails_closed`: selected by the command's filter: yes; defined in `app/src-tauri/src/phase3_tests.rs` (at H: `app/src-tauri/src/phase3_tests.rs`); function text identical H → now: yes |
| Executor analysis | **EXECUTOR_ANALYSIS_MATCH** |
| Review | **ARCHITECT_REVIEW_PENDING** |

The file's only change since H is one hunk appended after the file's last line (line 8999 at H) by D2A-1 22ebf9a5: the new test p3_c11_d2a1_the_ledger_protocol_core_is_confined. No line that existed at H is changed. The anchor is at the same line at H and now; its enclosing item (where one is detected) and every expected failing test's function text are byte-identical; the appended test's name is matched by none of this control's cargo filters. D2A-1 also added the production sources ledger.rs and ledger/protocol.rs, which guards that read workspace sources read in the healthy and the mutant run alike; no run establishes those guards' verdicts at this tree. The anchor is outside any detected function; the +/-20-line context is identical.

## M113 — libc's resolver and the DNS crates are not network kinds

| Field | Value |
|---|---|
| Set | D2A-1 |
| Category | bypass guard |
| Target file | `app/src-tauri/src/phase3_tests.rs` |
| Recorded (historical) sha256 | `2d131e17bef88dcb68a7b31f2b8bf06c7e2b9c3e2543e6af8a02a120c327fdae` |
| Current sha256 | `b7e0b7493fa0ce839782a1297d945184ac0d59dc6fdb002a16261199d66ab0b8` |
| Current git blob | `5d79c7c38b04a7ea480e144850cfb1a1e38d1e83` |
| First-known stale (existing evidence) | D2A-1 commit 22ebf9a5c5b1 |
| Transition commit (derived) | `22ebf9a5c5b16bb6f5169daf3731a3417c5289d3` |
| H (derived; holds the recorded hash) | `b1d535f1bb634f2075f12a36ace89a595cdb13ba` |
| Path history (recorded hash or other, at each change) | `5d9a2cafe4de other`, `8cd813a3f4ef REC`, `22ebf9a5c5b1 other` |
| File diff H → now | 1 file changed, 239 insertions(+) |
| Definition byte-equal to historical | id: yes, category: yes, historical_meaning: yes, candidate10_meaning: yes, historical: yes, edits: yes, command: yes, cwd: yes, expected_failure_tests: yes |
| Anchor (edit 1) | expected count 1; count at H 1, lines [4653]; count now 1, lines [4653]; cardinality matches |
| Anchor (edit 1) surroundings | enclosing function now `effect_kinds`, at H `effect_kinds`; text identical: yes; +/-20-line context identical: yes; replacement already occurs 1 time(s) now |
| Expected failing test | `phase3_tests::p3_g6_12_the_inventory_fails_closed`: selected by the command's filter: yes; defined in `app/src-tauri/src/phase3_tests.rs` (at H: `app/src-tauri/src/phase3_tests.rs`); function text identical H → now: yes |
| Executor analysis | **EXECUTOR_ANALYSIS_MATCH** |
| Review | **ARCHITECT_REVIEW_PENDING** |

The file's only change since H is one hunk appended after the file's last line (line 8999 at H) by D2A-1 22ebf9a5: the new test p3_c11_d2a1_the_ledger_protocol_core_is_confined. No line that existed at H is changed. The anchor is at the same line at H and now; its enclosing item (where one is detected) and every expected failing test's function text are byte-identical; the appended test's name is matched by none of this control's cargo filters. D2A-1 also added the production sources ledger.rs and ledger/protocol.rs, which guards that read workspace sources read in the healthy and the mutant run alike; no run establishes those guards' verdicts at this tree. The replacement text already occurs once in the file, as at H: restoration must be by blob equality (AD-1-R4 H.5).

## M141 — a resolver call through a path is not a network site

| Field | Value |
|---|---|
| Set | D2A-1 |
| Category | bypass guard |
| Target file | `app/src-tauri/src/phase3_tests.rs` |
| Recorded (historical) sha256 | `2d131e17bef88dcb68a7b31f2b8bf06c7e2b9c3e2543e6af8a02a120c327fdae` |
| Current sha256 | `b7e0b7493fa0ce839782a1297d945184ac0d59dc6fdb002a16261199d66ab0b8` |
| Current git blob | `5d79c7c38b04a7ea480e144850cfb1a1e38d1e83` |
| First-known stale (existing evidence) | D2A-1 commit 22ebf9a5c5b1 |
| Transition commit (derived) | `22ebf9a5c5b16bb6f5169daf3731a3417c5289d3` |
| H (derived; holds the recorded hash) | `b1d535f1bb634f2075f12a36ace89a595cdb13ba` |
| Path history (recorded hash or other, at each change) | `5d9a2cafe4de other`, `8cd813a3f4ef REC`, `22ebf9a5c5b1 other` |
| File diff H → now | 1 file changed, 239 insertions(+) |
| Definition byte-equal to historical | id: yes, category: yes, historical_meaning: yes, candidate10_meaning: yes, historical: yes, edits: yes, command: yes, cwd: yes, expected_failure_tests: yes |
| Anchor (edit 1) | expected count 1; count at H 1, lines [5048]; count now 1, lines [5048]; cardinality matches |
| Anchor (edit 1) surroundings | enclosing function now `file_effect_sites`, at H `file_effect_sites`; text identical: yes; +/-20-line context identical: yes; replacement already occurs 0 time(s) now |
| Expected failing test | `phase3_tests::p3_g6_12_the_inventory_fails_closed`: selected by the command's filter: yes; defined in `app/src-tauri/src/phase3_tests.rs` (at H: `app/src-tauri/src/phase3_tests.rs`); function text identical H → now: yes |
| Executor analysis | **EXECUTOR_ANALYSIS_MATCH** |
| Review | **ARCHITECT_REVIEW_PENDING** |

The file's only change since H is one hunk appended after the file's last line (line 8999 at H) by D2A-1 22ebf9a5: the new test p3_c11_d2a1_the_ledger_protocol_core_is_confined. No line that existed at H is changed. The anchor is at the same line at H and now; its enclosing item (where one is detected) and every expected failing test's function text are byte-identical; the appended test's name is matched by none of this control's cargo filters. D2A-1 also added the production sources ledger.rs and ledger/protocol.rs, which guards that read workspace sources read in the healthy and the mutant run alike; no run establishes those guards' verdicts at this tree.

## M143 — the Prometheus exporter is not a network kind

| Field | Value |
|---|---|
| Set | D2A-1 |
| Category | bypass guard |
| Target file | `app/src-tauri/src/phase3_tests.rs` |
| Recorded (historical) sha256 | `2d131e17bef88dcb68a7b31f2b8bf06c7e2b9c3e2543e6af8a02a120c327fdae` |
| Current sha256 | `b7e0b7493fa0ce839782a1297d945184ac0d59dc6fdb002a16261199d66ab0b8` |
| Current git blob | `5d79c7c38b04a7ea480e144850cfb1a1e38d1e83` |
| First-known stale (existing evidence) | D2A-1 commit 22ebf9a5c5b1 |
| Transition commit (derived) | `22ebf9a5c5b16bb6f5169daf3731a3417c5289d3` |
| H (derived; holds the recorded hash) | `b1d535f1bb634f2075f12a36ace89a595cdb13ba` |
| Path history (recorded hash or other, at each change) | `5d9a2cafe4de other`, `8cd813a3f4ef REC`, `22ebf9a5c5b1 other` |
| File diff H → now | 1 file changed, 239 insertions(+) |
| Definition byte-equal to historical | id: yes, category: yes, historical_meaning: yes, candidate10_meaning: yes, historical: yes, edits: yes, command: yes, cwd: yes, expected_failure_tests: yes |
| Anchor (edit 1) | expected count 1; count at H 1, lines [4562]; count now 1, lines [4562]; cardinality matches |
| Anchor (edit 1) surroundings | enclosing function now `network_beyond_sockets`, at H `network_beyond_sockets`; text identical: yes; +/-20-line context identical: yes; replacement is a deletion |
| Expected failing test | `phase3_tests::p3_g6_08_every_real_world_mechanism_in_the_workspace_is_classified`: selected by the command's filter: yes; defined in `app/src-tauri/src/phase3_tests.rs` (at H: `app/src-tauri/src/phase3_tests.rs`); function text identical H → now: yes |
| Expected failing test | `phase3_tests::p3_g6_12_the_inventory_fails_closed`: selected by the command's filter: yes; defined in `app/src-tauri/src/phase3_tests.rs` (at H: `app/src-tauri/src/phase3_tests.rs`); function text identical H → now: yes |
| Executor analysis | **EXECUTOR_ANALYSIS_MATCH** |
| Review | **ARCHITECT_REVIEW_PENDING** |

The file's only change since H is one hunk appended after the file's last line (line 8999 at H) by D2A-1 22ebf9a5: the new test p3_c11_d2a1_the_ledger_protocol_core_is_confined. No line that existed at H is changed. The anchor is at the same line at H and now; its enclosing item (where one is detected) and every expected failing test's function text are byte-identical; the appended test's name is matched by none of this control's cargo filters. D2A-1 also added the production sources ledger.rs and ledger/protocol.rs, which guards that read workspace sources read in the healthy and the mutant run alike; no run establishes those guards' verdicts at this tree. The control is a deletion (AD-1-R4 H.5).

## M153 — the Prometheus exporter's listener and gateway are not network sites

| Field | Value |
|---|---|
| Set | D2A-1 |
| Category | bypass guard |
| Target file | `app/src-tauri/src/phase3_tests.rs` |
| Recorded (historical) sha256 | `2d131e17bef88dcb68a7b31f2b8bf06c7e2b9c3e2543e6af8a02a120c327fdae` |
| Current sha256 | `b7e0b7493fa0ce839782a1297d945184ac0d59dc6fdb002a16261199d66ab0b8` |
| Current git blob | `5d79c7c38b04a7ea480e144850cfb1a1e38d1e83` |
| First-known stale (existing evidence) | D2A-1 commit 22ebf9a5c5b1 |
| Transition commit (derived) | `22ebf9a5c5b16bb6f5169daf3731a3417c5289d3` |
| H (derived; holds the recorded hash) | `b1d535f1bb634f2075f12a36ace89a595cdb13ba` |
| Path history (recorded hash or other, at each change) | `5d9a2cafe4de other`, `8cd813a3f4ef REC`, `22ebf9a5c5b1 other` |
| File diff H → now | 1 file changed, 239 insertions(+) |
| Definition byte-equal to historical | id: yes, category: yes, historical_meaning: yes, candidate10_meaning: yes, historical: yes, edits: yes, command: yes, cwd: yes, expected_failure_tests: yes |
| Anchor (edit 1) | expected count 1; count at H 1, lines [4596]; count now 1, lines [4596]; cardinality matches |
| Anchor (edit 1) surroundings | enclosing function now `None`, at H `None`; text identical: n/a; +/-20-line context identical: yes; replacement already occurs 0 time(s) now |
| Expected failing test | `phase3_tests::p3_g6_12_the_inventory_fails_closed`: selected by the command's filter: yes; defined in `app/src-tauri/src/phase3_tests.rs` (at H: `app/src-tauri/src/phase3_tests.rs`); function text identical H → now: yes |
| Executor analysis | **EXECUTOR_ANALYSIS_MATCH** |
| Review | **ARCHITECT_REVIEW_PENDING** |

The file's only change since H is one hunk appended after the file's last line (line 8999 at H) by D2A-1 22ebf9a5: the new test p3_c11_d2a1_the_ledger_protocol_core_is_confined. No line that existed at H is changed. The anchor is at the same line at H and now; its enclosing item (where one is detected) and every expected failing test's function text are byte-identical; the appended test's name is matched by none of this control's cargo filters. D2A-1 also added the production sources ledger.rs and ledger/protocol.rs, which guards that read workspace sources read in the healthy and the mutant run alike; no run establishes those guards' verdicts at this tree. The anchor is outside any detected function; the +/-20-line context is identical.

## M177 — a doc string can spell the async attribute of a command

| Field | Value |
|---|---|
| Set | D2A-1 |
| Category | bypass guard |
| Target file | `app/src-tauri/src/phase3_tests.rs` |
| Recorded (historical) sha256 | `2d131e17bef88dcb68a7b31f2b8bf06c7e2b9c3e2543e6af8a02a120c327fdae` |
| Current sha256 | `b7e0b7493fa0ce839782a1297d945184ac0d59dc6fdb002a16261199d66ab0b8` |
| Current git blob | `5d79c7c38b04a7ea480e144850cfb1a1e38d1e83` |
| First-known stale (existing evidence) | D2A-1 commit 22ebf9a5c5b1 |
| Transition commit (derived) | `22ebf9a5c5b16bb6f5169daf3731a3417c5289d3` |
| H (derived; holds the recorded hash) | `b1d535f1bb634f2075f12a36ace89a595cdb13ba` |
| Path history (recorded hash or other, at each change) | `5d9a2cafe4de other`, `8cd813a3f4ef REC`, `22ebf9a5c5b1 other` |
| File diff H → now | 1 file changed, 239 insertions(+) |
| Definition byte-equal to historical | id: yes, category: yes, historical_meaning: yes, candidate10_meaning: yes, historical: yes, edits: yes, command: yes, cwd: yes, expected_failure_tests: yes |
| Anchor (edit 1) | expected count 1; count at H 1, lines [8908]; count now 1, lines [8908]; cardinality matches |
| Anchor (edit 1) surroundings | enclosing function now `command_runs_off_thread`, at H `command_runs_off_thread`; text identical: yes; +/-20-line context identical: yes; replacement already occurs 0 time(s) now |
| Expected failing test | `phase3_tests::p3_g10_01_no_interface_thread_command_waits_on_the_loops_lock`: selected by the command's filter: yes; defined in `app/src-tauri/src/phase3_tests.rs` (at H: `app/src-tauri/src/phase3_tests.rs`); function text identical H → now: yes |
| Executor analysis | **EXECUTOR_ANALYSIS_MATCH** |
| Review | **ARCHITECT_REVIEW_PENDING** |

The file's only change since H is one hunk appended after the file's last line (line 8999 at H) by D2A-1 22ebf9a5: the new test p3_c11_d2a1_the_ledger_protocol_core_is_confined. No line that existed at H is changed. The anchor is at the same line at H and now; its enclosing item (where one is detected) and every expected failing test's function text are byte-identical; the appended test's name is matched by none of this control's cargo filters. D2A-1 also added the production sources ledger.rs and ledger/protocol.rs, which guards that read workspace sources read in the healthy and the mutant run alike; no run establishes those guards' verdicts at this tree.

## M178 — a cfg_attr command is not read as a command

| Field | Value |
|---|---|
| Set | D2A-1 |
| Category | bypass guard |
| Target file | `app/src-tauri/src/phase3_tests.rs` |
| Recorded (historical) sha256 | `2d131e17bef88dcb68a7b31f2b8bf06c7e2b9c3e2543e6af8a02a120c327fdae` |
| Current sha256 | `b7e0b7493fa0ce839782a1297d945184ac0d59dc6fdb002a16261199d66ab0b8` |
| Current git blob | `5d79c7c38b04a7ea480e144850cfb1a1e38d1e83` |
| First-known stale (existing evidence) | D2A-1 commit 22ebf9a5c5b1 |
| Transition commit (derived) | `22ebf9a5c5b16bb6f5169daf3731a3417c5289d3` |
| H (derived; holds the recorded hash) | `b1d535f1bb634f2075f12a36ace89a595cdb13ba` |
| Path history (recorded hash or other, at each change) | `5d9a2cafe4de other`, `8cd813a3f4ef REC`, `22ebf9a5c5b1 other` |
| File diff H → now | 1 file changed, 239 insertions(+) |
| Definition byte-equal to historical | id: yes, category: yes, historical_meaning: yes, candidate10_meaning: yes, historical: yes, edits: yes, command: yes, cwd: yes, expected_failure_tests: yes |
| Anchor (edit 1) | expected count 1; count at H 1, lines [8910]; count now 1, lines [8910]; cardinality matches |
| Anchor (edit 1) surroundings | enclosing function now `command_runs_off_thread`, at H `command_runs_off_thread`; text identical: yes; +/-20-line context identical: yes; replacement already occurs 0 time(s) now |
| Expected failing test | `phase3_tests::p3_g10_01_no_interface_thread_command_waits_on_the_loops_lock`: selected by the command's filter: yes; defined in `app/src-tauri/src/phase3_tests.rs` (at H: `app/src-tauri/src/phase3_tests.rs`); function text identical H → now: yes |
| Executor analysis | **EXECUTOR_ANALYSIS_MATCH** |
| Review | **ARCHITECT_REVIEW_PENDING** |

The file's only change since H is one hunk appended after the file's last line (line 8999 at H) by D2A-1 22ebf9a5: the new test p3_c11_d2a1_the_ledger_protocol_core_is_confined. No line that existed at H is changed. The anchor is at the same line at H and now; its enclosing item (where one is detected) and every expected failing test's function text are byte-identical; the appended test's name is matched by none of this control's cargo filters. D2A-1 also added the production sources ledger.rs and ledger/protocol.rs, which guards that read workspace sources read in the healthy and the mutant run alike; no run establishes those guards' verdicts at this tree.

## M179 — a function taken as a value is not followed

| Field | Value |
|---|---|
| Set | D2A-1 |
| Category | bypass guard |
| Target file | `app/src-tauri/src/phase3_tests.rs` |
| Recorded (historical) sha256 | `2d131e17bef88dcb68a7b31f2b8bf06c7e2b9c3e2543e6af8a02a120c327fdae` |
| Current sha256 | `b7e0b7493fa0ce839782a1297d945184ac0d59dc6fdb002a16261199d66ab0b8` |
| Current git blob | `5d79c7c38b04a7ea480e144850cfb1a1e38d1e83` |
| First-known stale (existing evidence) | D2A-1 commit 22ebf9a5c5b1 |
| Transition commit (derived) | `22ebf9a5c5b16bb6f5169daf3731a3417c5289d3` |
| H (derived; holds the recorded hash) | `b1d535f1bb634f2075f12a36ace89a595cdb13ba` |
| Path history (recorded hash or other, at each change) | `5d9a2cafe4de other`, `8cd813a3f4ef REC`, `22ebf9a5c5b1 other` |
| File diff H → now | 1 file changed, 239 insertions(+) |
| Definition byte-equal to historical | id: yes, category: yes, historical_meaning: yes, candidate10_meaning: yes, historical: yes, edits: yes, command: yes, cwd: yes, expected_failure_tests: yes |
| Anchor (edit 1) | expected count 1; count at H 1, lines [8991]; count now 1, lines [8991]; cardinality matches |
| Anchor (edit 1) surroundings | enclosing function now `uses`, at H `uses`; text identical: yes; +/-20-line context identical: no; replacement already occurs 0 time(s) now |
| Expected failing test | `phase3_tests::p3_g10_01_no_interface_thread_command_waits_on_the_loops_lock`: selected by the command's filter: yes; defined in `app/src-tauri/src/phase3_tests.rs` (at H: `app/src-tauri/src/phase3_tests.rs`); function text identical H → now: yes |
| Executor analysis | **EXECUTOR_ANALYSIS_MATCH** |
| Review | **ARCHITECT_REVIEW_PENDING** |

The file's only change since H is one hunk appended after the file's last line (line 8999 at H) by D2A-1 22ebf9a5: the new test p3_c11_d2a1_the_ledger_protocol_core_is_confined. No line that existed at H is changed. The anchor is at the same line at H and now; its enclosing item (where one is detected) and every expected failing test's function text are byte-identical; the appended test's name is matched by none of this control's cargo filters. D2A-1 also added the production sources ledger.rs and ledger/protocol.rs, which guards that read workspace sources read in the healthy and the mutant run alike; no run establishes those guards' verdicts at this tree. The +/-20-line context differs only because the appended hunk begins within 20 lines after the anchor (line 8991; the file ended at line 8999 at H); the enclosing function uses is byte-identical.

## M180 — a joined thread's body is not read as the caller's

| Field | Value |
|---|---|
| Set | D2A-1 |
| Category | bypass guard |
| Target file | `app/src-tauri/src/phase3_tests.rs` |
| Recorded (historical) sha256 | `2d131e17bef88dcb68a7b31f2b8bf06c7e2b9c3e2543e6af8a02a120c327fdae` |
| Current sha256 | `b7e0b7493fa0ce839782a1297d945184ac0d59dc6fdb002a16261199d66ab0b8` |
| Current git blob | `5d79c7c38b04a7ea480e144850cfb1a1e38d1e83` |
| First-known stale (existing evidence) | D2A-1 commit 22ebf9a5c5b1 |
| Transition commit (derived) | `22ebf9a5c5b16bb6f5169daf3731a3417c5289d3` |
| H (derived; holds the recorded hash) | `b1d535f1bb634f2075f12a36ace89a595cdb13ba` |
| Path history (recorded hash or other, at each change) | `5d9a2cafe4de other`, `8cd813a3f4ef REC`, `22ebf9a5c5b1 other` |
| File diff H → now | 1 file changed, 239 insertions(+) |
| Definition byte-equal to historical | id: yes, category: yes, historical_meaning: yes, candidate10_meaning: yes, historical: yes, edits: yes, command: yes, cwd: yes, expected_failure_tests: yes |
| Anchor (edit 1) | expected count 1; count at H 1, lines [8919]; count now 1, lines [8919]; cardinality matches |
| Anchor (edit 1) surroundings | enclosing function now `outside_spawned`, at H `outside_spawned`; text identical: yes; +/-20-line context identical: yes; replacement already occurs 0 time(s) now |
| Expected failing test | `phase3_tests::p3_g10_01_no_interface_thread_command_waits_on_the_loops_lock`: selected by the command's filter: yes; defined in `app/src-tauri/src/phase3_tests.rs` (at H: `app/src-tauri/src/phase3_tests.rs`); function text identical H → now: yes |
| Executor analysis | **EXECUTOR_ANALYSIS_MATCH** |
| Review | **ARCHITECT_REVIEW_PENDING** |

The file's only change since H is one hunk appended after the file's last line (line 8999 at H) by D2A-1 22ebf9a5: the new test p3_c11_d2a1_the_ledger_protocol_core_is_confined. No line that existed at H is changed. The anchor is at the same line at H and now; its enclosing item (where one is detected) and every expected failing test's function text are byte-identical; the appended test's name is matched by none of this control's cargo filters. D2A-1 also added the production sources ledger.rs and ledger/protocol.rs, which guards that read workspace sources read in the healthy and the mutant run alike; no run establishes those guards' verdicts at this tree.

## M18 — a commitment releases a lease it does not list

| Field | Value |
|---|---|
| Set | earlier Candidate 11 |
| Category | broker |
| Target file | `crates/nexus-governed-control/src/broker.rs` |
| Recorded (historical) sha256 | `471d8a79ec8a030b68c3af7c0a0156c52ffca468ea2be056512d5eeb0941ce9e` |
| Current sha256 | `353bbbc8a9d983fb6888b71d7c178db6bcc3a9d87ad30a072e97b7509c6e14e4` |
| Current git blob | `eaa469d94fbfb7f31f5ba51d06d7bdc85cee16a2` |
| First-known stale (existing evidence) | D2A-1 contract base 862a11cef284 |
| Transition commit (derived) | `862a11cef284c5e3c34234dfd2677a5462c86d5d` |
| H (derived; holds the recorded hash) | `f5f2f3da8002bbcf8a6a3e049f0e93c8703c7670` |
| Path history (recorded hash or other, at each change) | `e111c7c51fd6 other`, `089e1838a4e4 REC`, `862a11cef284 other` |
| File diff H → now | 1 file changed, 66 insertions(+), 5 deletions(-) |
| Definition byte-equal to historical | id: yes, category: yes, historical_meaning: yes, candidate10_meaning: yes, historical: yes, edits: yes, command: yes, cwd: yes, expected_failure_tests: yes |
| Anchor (edit 1) | expected count 1; count at H 1, lines [350]; count now 1, lines [403]; cardinality matches |
| Anchor (edit 1) surroundings | enclosing function now `release`, at H `release`; text identical: no; +/-20-line context identical: no; replacement already occurs 0 time(s) now |
| Expected failing test | `broker::tests::a_commitment_cannot_release_a_lease_it_does_not_list`: selected by the command's filter: yes; defined in `crates/nexus-governed-control/src/broker/tests.rs` (at H: `crates/nexus-governed-control/src/broker/tests.rs`); function text identical H → now: yes |
| Executor analysis | **EXECUTOR_ANALYSIS_DIFFERENCE** |
| Review | **ARCHITECT_REVIEW_PENDING** |

862a11ce added still_releasing, called after the CredentialReleased record and again after the vault read. It asks again whether the commitment executing under the guard lists the lease, and refuses with the same AuthorityError::Closed("the lease is not bound to this commitment"), ending the lease. With the mutant ('if false {'), an unlisted lease of the same agent and run now passes the anchored check, is consumed (Issued to Released), a CredentialReleased record is written, and still_releasing refuses before the vault is read. At H (f5f2f3da) the same mutant read the vault and returned a header. Predicted at this tree: the test's error assertion holds; its assertion broker.is_live(other) ('the unlisted lease was not consumed') fails; its vault-count assertion holds. The same expected test is predicted to fail, through a different assertion, and the fault the mutant introduces is narrower than at H: the unlisted lease is consumed and recorded as released, but no secret is read. The test's function text is byte-identical. Not established by any run.

## M19 — a lease releases its secret more than once

| Field | Value |
|---|---|
| Set | earlier Candidate 11 |
| Category | broker |
| Target file | `crates/nexus-governed-control/src/broker.rs` |
| Recorded (historical) sha256 | `471d8a79ec8a030b68c3af7c0a0156c52ffca468ea2be056512d5eeb0941ce9e` |
| Current sha256 | `353bbbc8a9d983fb6888b71d7c178db6bcc3a9d87ad30a072e97b7509c6e14e4` |
| Current git blob | `eaa469d94fbfb7f31f5ba51d06d7bdc85cee16a2` |
| First-known stale (existing evidence) | D2A-1 contract base 862a11cef284 |
| Transition commit (derived) | `862a11cef284c5e3c34234dfd2677a5462c86d5d` |
| H (derived; holds the recorded hash) | `f5f2f3da8002bbcf8a6a3e049f0e93c8703c7670` |
| Path history (recorded hash or other, at each change) | `e111c7c51fd6 other`, `089e1838a4e4 REC`, `862a11cef284 other` |
| File diff H → now | 1 file changed, 66 insertions(+), 5 deletions(-) |
| Definition byte-equal to historical | id: yes, category: yes, historical_meaning: yes, candidate10_meaning: yes, historical: yes, edits: yes, command: yes, cwd: yes, expected_failure_tests: yes |
| Anchor (edit 1) | expected count 1; count at H 1, lines [372]; count now 1, lines [425]; cardinality matches |
| Anchor (edit 1) surroundings | enclosing function now `release`, at H `release`; text identical: no; +/-20-line context identical: yes; replacement already occurs 0 time(s) now |
| Expected failing test | `broker::tests::a_lease_releases_its_secret_once_to_its_commitment_only`: selected by the command's filter: yes; defined in `crates/nexus-governed-control/src/broker/tests.rs` (at H: `crates/nexus-governed-control/src/broker/tests.rs`); function text identical H → now: yes |
| Executor analysis | **EXECUTOR_ANALYSIS_MATCH** |
| Review | **ARCHITECT_REVIEW_PENDING** |

Within release, 862a11ce's changes are the still_releasing calls and the reshaped vault read, below the anchor; the anchor's +/-20-line context is identical. With the mutant ('if false {'), a second release of a lease in state Released is not refused at the anchor; still_releasing accepts a lease in state Released, so the vault is read again and a header returned. Predicted: the test's unwrap_err() == NotPending and vault-count assertions fail, as at H. The test's function text is byte-identical.

## M39 — a credential is taken from the process environment

| Field | Value |
|---|---|
| Set | earlier Candidate 11 |
| Category | broker |
| Target file | `crates/nexus-governed-control/src/broker.rs` |
| Recorded (historical) sha256 | `471d8a79ec8a030b68c3af7c0a0156c52ffca468ea2be056512d5eeb0941ce9e` |
| Current sha256 | `353bbbc8a9d983fb6888b71d7c178db6bcc3a9d87ad30a072e97b7509c6e14e4` |
| Current git blob | `eaa469d94fbfb7f31f5ba51d06d7bdc85cee16a2` |
| First-known stale (existing evidence) | D2A-1 contract base 862a11cef284 |
| Transition commit (derived) | `862a11cef284c5e3c34234dfd2677a5462c86d5d` |
| H (derived; holds the recorded hash) | `f5f2f3da8002bbcf8a6a3e049f0e93c8703c7670` |
| Path history (recorded hash or other, at each change) | `e111c7c51fd6 other`, `089e1838a4e4 REC`, `862a11cef284 other` |
| File diff H → now | 1 file changed, 66 insertions(+), 5 deletions(-) |
| Definition byte-equal to historical | id: yes, category: yes, historical_meaning: yes, candidate10_meaning: yes, historical: yes, edits: yes, command: yes, cwd: yes, expected_failure_tests: yes |
| Anchor (edit 1) | expected count 1; count at H 1, lines [108]; count now 1, lines [120]; cardinality matches |
| Anchor (edit 1) surroundings | enclosing function now `from_vault`, at H `from_vault`; text identical: yes; +/-20-line context identical: yes; replacement is a deletion |
| Expected failing test | `broker::tests::a_credential_from_the_environment_is_refused`: selected by the command's filter: yes; defined in `crates/nexus-governed-control/src/broker/tests.rs` (at H: `crates/nexus-governed-control/src/broker/tests.rs`); function text identical H → now: yes |
| Executor analysis | **EXECUTOR_ANALYSIS_MATCH** |
| Review | **ARCHITECT_REVIEW_PENDING** |

from_vault, which holds the anchored match arm, and the anchor's +/-20-line context are byte-identical; 862a11ce's changes are elsewhere in the file. The expected test's function text is byte-identical. The control is a deletion (AD-1-R4 H.5).

## M84 — an R2 approval becomes cloneable

| Field | Value |
|---|---|
| Set | earlier Candidate 11 |
| Category | native approval |
| Target file | `crates/nexus-governed-control/src/authority/approval.rs` |
| Recorded (historical) sha256 | `f34be06d927e59884f219e3b31bc57916ad3da6bc3bc65120d8e8c9fd2252c6c` |
| Current sha256 | `34f849b502782f0c6a046c9fd2d5ceb50bf497a54e52e591339630670db49202` |
| Current git blob | `19e3ace99a7f24c9d182354c2cfcdc388729af4d` |
| First-known stale (existing evidence) | Candidate 11 b1d535f1bb63 |
| Transition commit (derived) | `6be32bf43cebce623681f2d1e82965ba784a7f04` |
| H (derived; holds the recorded hash) | `862a11cef284c5e3c34234dfd2677a5462c86d5d` |
| Path history (recorded hash or other, at each change) | `da1df3dca68f other`, `73d1a65ac2e7 REC`, `6be32bf43ceb other` |
| File diff H → now | 1 file changed, 5 insertions(+), 2 deletions(-) |
| Definition byte-equal to historical | id: yes, category: yes, historical_meaning: yes, candidate10_meaning: yes, historical: yes, edits: yes, command: yes, cwd: yes, expected_failure_tests: yes |
| Anchor (edit 1) | expected count 1; count at H 1, lines [191]; count now 1, lines [194]; cardinality matches |
| Anchor (edit 1) surroundings | enclosing function now `None`, at H `None`; text identical: n/a; +/-20-line context identical: yes; replacement already occurs 0 time(s) now |
| Expected failing test | `phase3_tests::p3_g6_02_an_r2_approval_is_minted_once_after_the_native_answer`: selected by the command's filter: yes; defined in `app/src-tauri/src/phase3_tests.rs` (at H: `app/src-tauri/src/phase3_tests.rs`); function text identical H → now: yes |
| Executor analysis | **EXECUTOR_ANALYSIS_MATCH** |
| Review | **ARCHITECT_REVIEW_PENDING** |

The file's only change since H (RS-01 6be32bf4) is to ResumeConfirmation: a stop_number field, its documentation and its header text (lines 150-170 now). The anchor, R2Approval's derive and declaration (line 191 at H, 194 now), and its +/-20-line context are identical. The expected test p3_g6_02's function text is byte-identical.

## M05 — an emergency stop is lifted without the owner

| Field | Value |
|---|---|
| Set | earlier Candidate 11 |
| Category | commitment |
| Target file | `crates/nexus-governed-control/src/authority/mod.rs` |
| Recorded (historical) sha256 | `f4eca418d66a239148087efba96b88c41547699c60a8929bafcbc89f88e7ae32` |
| Current sha256 | `ae99b2f59794470c4899396861c4942ba5db2e2e338fb3aeab8712be6c217013` |
| Current git blob | `c1dfe9f5af9f50b84c5400df0306d4c6f577fb48` |
| First-known stale (existing evidence) | Candidate 11 b1d535f1bb63 |
| Transition commit (derived) | `6be32bf43cebce623681f2d1e82965ba784a7f04` |
| H (derived; holds the recorded hash) | `862a11cef284c5e3c34234dfd2677a5462c86d5d` |
| Path history (recorded hash or other, at each change) | `da1df3dca68f other`, `8dd5db7446e4 REC`, `6be32bf43ceb other` |
| File diff H → now | 1 file changed, 50 insertions(+), 10 deletions(-) |
| Definition byte-equal to historical | id: yes, category: yes, historical_meaning: yes, candidate10_meaning: yes, historical: yes, edits: yes, command: yes, cwd: yes, expected_failure_tests: yes |
| Anchor (edit 1) | expected count 1; count at H 1, lines [312]; count now 1, lines [335]; cardinality matches |
| Anchor (edit 1) surroundings | enclosing function now `resume`, at H `resume`; text identical: no; +/-20-line context identical: no; replacement already occurs 0 time(s) now |
| Expected failing test | `authority::tests::an_emergency_stop_cancels_every_run_and_refuses_new_ones`: selected by the command's filter: yes; defined in `crates/nexus-governed-control/src/authority/tests.rs` (at H: `crates/nexus-governed-control/src/authority/tests.rs`); function text identical H → now: yes |
| Expected failing test | `governed::tests::the_emergency_stop_ends_everything_until_the_owner_resumes`: selected by the command's filter: yes; defined in `crates/nexus-governed-control/src/governed/tests.rs` (at H: `crates/nexus-governed-control/src/governed/tests.rs`); function text identical H → now: yes |
| Executor analysis | **EXECUTOR_ANALYSIS_MATCH** |
| Review | **ARCHITECT_REVIEW_PENDING** |

RS-01 6be32bf4 rewrote resume around the anchor: stop_to_lift replaces is_stopped; the confirmation request carries stop_number; check_lift is asked after the answer; lift(stop_number) replaces resume(). The anchored statement 'if !confirmer.confirm_resume(&request) {' is unchanged and is still the only place resume consults the owner's answer. With the mutant ('if false && ...') the confirmer is not consulted and resume goes on to check_lift, the Resumed record and lift: a stop is lifted without the owner, the fault the control names. Both expected tests' function texts are byte-identical. Because the enclosing function changed, this is an analysis of the new text, not a text identity.

## M45 — finishing a run does not set its token

| Field | Value |
|---|---|
| Set | earlier Candidate 11 |
| Category | commitment |
| Target file | `crates/nexus-governed-control/src/authority/run.rs` |
| Recorded (historical) sha256 | `33d3c06b65405799d304c91c32ccb0121c0a012a476cfa34bb447b40faae5725` |
| Current sha256 | `394d725b5386fdcd29ba4306de0810a92d73be3c011caeb2a7c905981d7d5ae6` |
| Current git blob | `83e2c42e306872fe35d2674dc3e1bc4ce52b05f1` |
| First-known stale (existing evidence) | Candidate 11 b1d535f1bb63 |
| Transition commit (derived) | `6be32bf43cebce623681f2d1e82965ba784a7f04` |
| H (derived; holds the recorded hash) | `862a11cef284c5e3c34234dfd2677a5462c86d5d` |
| Path history (recorded hash or other, at each change) | `da1df3dca68f other`, `8dd5db7446e4 REC`, `6be32bf43ceb other` |
| File diff H → now | 1 file changed, 216 insertions(+), 12 deletions(-) |
| Definition byte-equal to historical | id: yes, category: yes, historical_meaning: yes, candidate10_meaning: yes, historical: yes, edits: yes, command: yes, cwd: yes, expected_failure_tests: yes |
| Anchor (edit 1) | expected count 1; count at H 1, lines [190]; count now 1, lines [376]; cardinality matches |
| Anchor (edit 1) surroundings | enclosing function now `finish`, at H `finish`; text identical: yes; +/-20-line context identical: no; replacement already occurs 1 time(s) now |
| Expected failing test | `authority::tests::finishing_a_run_ends_what_it_left_and_releases_what_it_owns`: selected by the command's filter: yes; defined in `crates/nexus-governed-control/src/authority/tests.rs` (at H: `crates/nexus-governed-control/src/authority/tests.rs`); function text identical H → now: yes |
| Executor analysis | **EXECUTOR_ANALYSIS_MATCH** |
| Review | **ARCHITECT_REVIEW_PENDING** |

finish, which holds the anchor, is byte-identical. RS-01 6be32bf4's changes in run.rs are the stop number, lift, check_lift and stop_all, which now sets each run's cancel token itself (a third 'cancelled.store(true' in the file, inside stop_all, not finish). The replacement text already occurs once in the file: restoration must be by blob equality (AD-1-R4 H.5). The expected test's function text is byte-identical.

## M129 — quitting lets new runs open

| Field | Value |
|---|---|
| Set | earlier Candidate 11 |
| Category | unified command |
| Target file | `crates/nexus-governed-control/src/authority/run.rs` |
| Recorded (historical) sha256 | `33d3c06b65405799d304c91c32ccb0121c0a012a476cfa34bb447b40faae5725` |
| Current sha256 | `394d725b5386fdcd29ba4306de0810a92d73be3c011caeb2a7c905981d7d5ae6` |
| Current git blob | `83e2c42e306872fe35d2674dc3e1bc4ce52b05f1` |
| First-known stale (existing evidence) | Candidate 11 b1d535f1bb63 |
| Transition commit (derived) | `6be32bf43cebce623681f2d1e82965ba784a7f04` |
| H (derived; holds the recorded hash) | `862a11cef284c5e3c34234dfd2677a5462c86d5d` |
| Path history (recorded hash or other, at each change) | `da1df3dca68f other`, `8dd5db7446e4 REC`, `6be32bf43ceb other` |
| File diff H → now | 1 file changed, 216 insertions(+), 12 deletions(-) |
| Definition byte-equal to historical | id: yes, category: yes, historical_meaning: yes, candidate10_meaning: yes, historical: yes, edits: yes, command: yes, cwd: yes, expected_failure_tests: yes |
| Anchor (edit 1) | expected count 1; count at H 1, lines [151]; count now 1, lines [229]; cardinality matches |
| Anchor (edit 1) surroundings | enclosing function now `close`, at H `close`; text identical: yes; +/-20-line context identical: no; replacement already occurs 2 time(s) now |
| Expected failing test | `governed::tests::quitting_cancels_every_run_and_opens_no_more`: selected by the command's filter: yes; defined in `crates/nexus-governed-control/src/governed/tests.rs` (at H: `crates/nexus-governed-control/src/governed/tests.rs`); function text identical H → now: yes |
| Executor analysis | **EXECUTOR_ANALYSIS_MATCH** |
| Review | **ARCHITECT_REVIEW_PENDING** |

close, which holds the anchor, is byte-identical and is still the only production writer of the registry's closed flag. RS-01 also added, in authority/tests.rs, a structural list (HOLDS, used by its test T8) naming close with 'self.closed.store(true'; that test is not selected by this control's cargo filter and is not relied on. The replacement text already occurs twice in the file: restoration must be by blob equality (AD-1-R4 H.5). The expected test's function text is byte-identical.


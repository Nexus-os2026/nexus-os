# D2A-1 exact-final control catalog (contract 7.5) — d2a1-controls-v1

Catalog status: **CONTROL_CATALOG_BLOCKED**. NOT FROZEN — the static catalog gate cannot pass while any definition is blocked (mission 7, 8).

Baseline: Candidate C `d5964ee5326c2733177307db6f507328023d361a` (tree `51bac5fd335999cc995c0ab9f582a30244973ef0`).

Kill-required: 35; separate comparison: 1 (A4). READY: 14; BLOCKED: 21.

| Id | Change (contract 7.5) | Named | On / test | Receipt type | Executor status | Tranche 1 |
|---|---|---|---|---|---|---|
| C1 | the count becomes a flag | P1 | `ledger::protocol::model_tests::full_f1` | (a) original-explorer mutant FAIL | READY | — |
| C2 | the gating entry rule does not test C and X | P4 | `ledger::protocol::model_tests::full_f1` | (a) original-explorer mutant FAIL | READY | — |
| C3a | acknowledgement test 2 removed | P13 / P15 | `ledger::protocol::model_tests::full_f7` | (a) original-explorer mutant FAIL | BLOCKED | — |
| C3b | acknowledgement test 4 removed | P13 / P12 | `ledger::protocol::model_tests::full_f6` | (a) original-explorer mutant FAIL | BLOCKED | — |
| C3c | acknowledgement test 5 removed | P12 | `ledger::protocol::model_tests::full_f6` | (a) original-explorer mutant FAIL | BLOCKED | — |
| C4 | acknowledgement test 1 removed | P4 / P8 | `ledger::protocol::model_tests::full_f8` | (a) original-explorer mutant FAIL | READY | — |
| C5 | resolve, for an acknowledgement in U, raises the frontier with neither the presence read nor the frontier check | P13 | `ledger::protocol::model_tests::full_f5` | (a) original-explorer mutant FAIL | BLOCKED | — |
| C6 | classification row b ignores the refusal table, **with** A4 | P6 | `ledger::protocol::model_tests::full_fpt` | (a) original-explorer mutant FAIL | BLOCKED | — |
| C7 | *Explained* removes every table entry, **with** A4 | P6 | `ledger::protocol::model_tests::full_fpt` | (a) original-explorer mutant FAIL | BLOCKED | — |
| C8 | members are classified and inserted one at a time | E5 | `ledger::protocol::model_tests::full_fb` | (a) original-explorer mutant FAIL | BLOCKED | — |
| C9 | `Rollback::Unknown` is treated as `Undone` | E4 | `ledger::protocol::model_tests::full_fb2` | (a) original-explorer mutant FAIL | READY | 8 |
| C10 | `Commit::Unknown` followed by `Rollback::Undone` gives Not written | E4 | `ledger::protocol::model_tests::full_f1` | (a) original-explorer mutant FAIL | READY | — |
| C11 | a constraint failure during the inserts gives Not written whatever the rollback said. **This is the amendment's rule, which the review rejected.** | E4 | `ledger::protocol::model_tests::full_fb2` | (a) original-explorer mutant FAIL | BLOCKED | 9 |
| C12 | the publication is two exchanges: bits, then count | E2 | `ledger::protocol::model_tests::x1_two_facts_two_writers_one_admission` | (a) original-explorer mutant FAIL | READY | 7 |
| C15 | a cell operation ignores the occupancy number | E8 | `ledger::protocol::model_tests::full_fr` | (a) original-explorer mutant FAIL | BLOCKED | — |
| C16 | the drop of a Counted ticket lowers the count | E3 | `ledger::protocol::model_tests::full_fd` | (a) original-explorer mutant FAIL | BLOCKED | — |
| C17 | the guard reports Not written after an insert was issued | E4 | `ledger::protocol::model_tests::full_fb2` | (a) original-explorer mutant FAIL | BLOCKED | 10 |
| C18a | a refused gating append leaves U occupied | E7 | `ledger::protocol::model_tests::full_fg` | (a) original-explorer mutant FAIL | BLOCKED | 11 |
| C18b | a gating conflict makes a table entry | E7 | `ledger::protocol::model_tests::full_fg` | (a) original-explorer mutant FAIL | BLOCKED | 12 |
| C20 | an explanation found already present is not marked committed | L1 | `ledger::protocol::model_tests::full_f2` | (a) original-explorer mutant FAIL | BLOCKED | — |
| C23 | the publication omits the X bit | E1 | `ledger::protocol::model_tests::full_f2` | (a) original-explorer mutant FAIL | BLOCKED | — |
| C24 | resolve empties U for a present gating record without placing the correction | P8 | `ledger::protocol::model_tests::full_f1` | (a) original-explorer mutant FAIL | READY | — |
| C25 | a gating append answers Committed on `Commit::Unknown` | P7 | `ledger::protocol::model_tests::full_f1` | (a) original-explorer mutant FAIL | READY | — |
| C26 | a passed barrier clears the latch | E9 | `ledger::protocol::model_tests::full_f4` | (a) original-explorer mutant FAIL | BLOCKED | — |
| C14 | the ticket check of step 0 is removed | T-BIND | `ledger::protocol::unit_tests::t_bind_every_bound_value_belongs_to_one_ledger` | (b) named non-explorer mutant-test FAIL | READY | 2 |
| C19 | X can be emptied before it is marked committed | T-CELL | `ledger::protocol::unit_tests::t_cell_worker_operations_and_cell_appends` | (b) named non-explorer mutant-test FAIL | READY | 3 |
| C21 | the well-formed test of step 1 is removed | W6 | `ledger::protocol::unit_tests::w6_one_identity_under_two_contents` | (b) named non-explorer mutant-test FAIL | READY | 4 |
| C22 | the ticket derives `Clone` | T-OWN | `ledger::protocol::unit_tests::t_own_tickets_snapshots_and_the_worker_cannot_be_cloned` | (d) intentional mutant compile failure | READY | 1 |
| C27 | the guard is created after the entry rule | T-PREC | `ledger::protocol::unit_tests::t_prec_persistence_of_every_ending` | (b) named non-explorer mutant-test FAIL | READY | 5 |
| C28 | resolve's "present, frontier below N" check alone is removed | T-RES | `ledger::protocol::unit_tests::t_res_every_resolve_row` | (b) named non-explorer mutant-test FAIL | READY | 6 |
| A1 | the writer confirms before it records | P1 | `ledger::protocol::model_tests::full_f1` | (a) original-explorer mutant FAIL | BLOCKED | — |
| A2 | admission is split into two steps under two separate holds of the test's lock. Step one takes a `view` and keeps only its low 16 bits: phase, session, latch, cells, handover fault. Any actor may step in between. Step two takes a second `view`, keeps only `unapplied`, reads the debt count, H and A, and decides from the kept bits of step one and the values of step two. | P1 | `ledger::protocol::model_tests::full_f1` | (a) original-explorer mutant FAIL | BLOCKED | — |
| A3 | the worker empties X before it raises H | P12 | `ledger::protocol::model_tests::full_f5` | (a) original-explorer mutant FAIL | BLOCKED | — |
| A4 | the drop paths record a blanket Uncertain | **nothing may fail** | `F-PT` | (f) the A4 comparison (AD-1-R4 F.1) | SATISFIED — ARCHITECT ACCEPTED | — |
| A5 | settlement claims a Conflict obligation while the store is not Healthy | L1 | `ledger::protocol::model_tests::full_f2` | (a) original-explorer mutant FAIL | BLOCKED | — |
| A6 | "recovery required" is a flag, not a comparison of H with A | P12 / P15 | `ledger::protocol::model_tests::full_f5` | (a) original-explorer mutant FAIL | BLOCKED | — |

Every control's Architect review status: pending. Execution status: NOT ATTEMPTED (A4: not rerun).

The machine-readable record, with anchors, replacements, patches, counts, links and the executor's observations for each BLOCKED control, is `catalog.json`.

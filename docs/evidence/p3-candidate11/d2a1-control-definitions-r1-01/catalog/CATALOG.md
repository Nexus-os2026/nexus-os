# D2A-1 control catalog (contract 7.5) — candidate — `d2a1-controls-v1-r1-candidate`

Status: **CANDIDATE — INDEPENDENT ARCHITECT REVIEW PENDING**. Acceptance belongs to the Sole Architect after review; this file claims none.

Baseline: Candidate C `d5964ee5326c2733177307db6f507328023d361a` (tree `51bac5fd335999cc995c0ab9f582a30244973ef0`).

Kill-required: 35 (definition CANDIDATE_READY: 35; carried forward 14, Architect-defined 21). Separate comparison: 1 (A4).

| Id | Change (contract 7.5) | Named | On / test | Receipt type | Definition | Source | Review | Execution |
|---|---|---|---|---|---|---|---|---|
| C1 | the count becomes a flag | P1 | `ledger::protocol::model_tests::full_f1` | (a) original-explorer mutant FAIL | CANDIDATE_READY | CARRIED_FORWARD | pending | NOT_ATTEMPTED |
| C2 | the gating entry rule does not test C and X | P4 | `ledger::protocol::model_tests::full_f1` | (a) original-explorer mutant FAIL | CANDIDATE_READY | CARRIED_FORWARD | pending | NOT_ATTEMPTED |
| C3a | acknowledgement test 2 removed | P13 / P15 | `ledger::protocol::model_tests::full_f7` | (a) original-explorer mutant FAIL | CANDIDATE_READY | ARCHITECT_DEFINED | pending | NOT_ATTEMPTED |
| C3b | acknowledgement test 4 removed | P13 / P12 | `ledger::protocol::model_tests::full_f6` | (a) original-explorer mutant FAIL | CANDIDATE_READY | ARCHITECT_DEFINED | pending | NOT_ATTEMPTED |
| C3c | acknowledgement test 5 removed | P12 | `ledger::protocol::model_tests::full_f6` | (a) original-explorer mutant FAIL | CANDIDATE_READY | ARCHITECT_DEFINED | pending | NOT_ATTEMPTED |
| C4 | acknowledgement test 1 removed | P4 / P8 | `ledger::protocol::model_tests::full_f8` | (a) original-explorer mutant FAIL | CANDIDATE_READY | CARRIED_FORWARD | pending | NOT_ATTEMPTED |
| C5 | resolve, for an acknowledgement in U, raises the frontier with neither the presence read nor the frontier check | P13 | `ledger::protocol::model_tests::full_f5` | (a) original-explorer mutant FAIL | CANDIDATE_READY | ARCHITECT_DEFINED | pending | NOT_ATTEMPTED |
| C6 | classification row b ignores the refusal table, **with** A4 | P6 | `ledger::protocol::model_tests::full_fpt` | (a) original-explorer mutant FAIL | CANDIDATE_READY | ARCHITECT_DEFINED | pending | NOT_ATTEMPTED |
| C7 | *Explained* removes every table entry, **with** A4 | P6 | `ledger::protocol::model_tests::full_fpt` | (a) original-explorer mutant FAIL | CANDIDATE_READY | ARCHITECT_DEFINED | pending | NOT_ATTEMPTED |
| C8 | members are classified and inserted one at a time | E5 | `ledger::protocol::model_tests::full_fb` | (a) original-explorer mutant FAIL | CANDIDATE_READY | ARCHITECT_DEFINED | pending | NOT_ATTEMPTED |
| C9 | `Rollback::Unknown` is treated as `Undone` | E4 | `ledger::protocol::model_tests::full_fb2` | (a) original-explorer mutant FAIL | CANDIDATE_READY | CARRIED_FORWARD | pending | NOT_ATTEMPTED |
| C10 | `Commit::Unknown` followed by `Rollback::Undone` gives Not written | E4 | `ledger::protocol::model_tests::full_f1` | (a) original-explorer mutant FAIL | CANDIDATE_READY | CARRIED_FORWARD | pending | NOT_ATTEMPTED |
| C11 | a constraint failure during the inserts gives Not written whatever the rollback said. **This is the amendment's rule, which the review rejected.** | E4 | `ledger::protocol::model_tests::full_fb2` | (a) original-explorer mutant FAIL | CANDIDATE_READY | ARCHITECT_DEFINED | pending | NOT_ATTEMPTED |
| C12 | the publication is two exchanges: bits, then count | E2 | `ledger::protocol::model_tests::x1_two_facts_two_writers_one_admission` | (a) original-explorer mutant FAIL | CANDIDATE_READY | CARRIED_FORWARD | pending | NOT_ATTEMPTED |
| C15 | a cell operation ignores the occupancy number | E8 | `ledger::protocol::model_tests::full_fr` | (a) original-explorer mutant FAIL | CANDIDATE_READY | ARCHITECT_DEFINED | pending | NOT_ATTEMPTED |
| C16 | the drop of a Counted ticket lowers the count | E3 | `ledger::protocol::model_tests::full_fd` | (a) original-explorer mutant FAIL | CANDIDATE_READY | ARCHITECT_DEFINED | pending | NOT_ATTEMPTED |
| C17 | the guard reports Not written after an insert was issued | E4 | `ledger::protocol::model_tests::full_fb2` | (a) original-explorer mutant FAIL | CANDIDATE_READY | ARCHITECT_DEFINED | pending | NOT_ATTEMPTED |
| C18a | a refused gating append leaves U occupied | E7 | `ledger::protocol::model_tests::full_fg` | (a) original-explorer mutant FAIL | CANDIDATE_READY | ARCHITECT_DEFINED | pending | NOT_ATTEMPTED |
| C18b | a gating conflict makes a table entry | E7 | `ledger::protocol::model_tests::full_fg` | (a) original-explorer mutant FAIL | CANDIDATE_READY | ARCHITECT_DEFINED | pending | NOT_ATTEMPTED |
| C20 | an explanation found already present is not marked committed | L1 | `ledger::protocol::model_tests::full_f2` | (a) original-explorer mutant FAIL | CANDIDATE_READY | ARCHITECT_DEFINED | pending | NOT_ATTEMPTED |
| C23 | the publication omits the X bit | E1 | `ledger::protocol::model_tests::full_f2` | (a) original-explorer mutant FAIL | CANDIDATE_READY | ARCHITECT_DEFINED | pending | NOT_ATTEMPTED |
| C24 | resolve empties U for a present gating record without placing the correction | P8 | `ledger::protocol::model_tests::full_f1` | (a) original-explorer mutant FAIL | CANDIDATE_READY | CARRIED_FORWARD | pending | NOT_ATTEMPTED |
| C25 | a gating append answers Committed on `Commit::Unknown` | P7 | `ledger::protocol::model_tests::full_f1` | (a) original-explorer mutant FAIL | CANDIDATE_READY | CARRIED_FORWARD | pending | NOT_ATTEMPTED |
| C26 | a passed barrier clears the latch | E9 | `ledger::protocol::model_tests::full_f4` | (a) original-explorer mutant FAIL | CANDIDATE_READY | ARCHITECT_DEFINED | pending | NOT_ATTEMPTED |
| C14 | the ticket check of step 0 is removed | T-BIND | `ledger::protocol::unit_tests::t_bind_every_bound_value_belongs_to_one_ledger` | (b) named non-explorer mutant-test FAIL | CANDIDATE_READY | CARRIED_FORWARD | pending | NOT_ATTEMPTED |
| C19 | X can be emptied before it is marked committed | T-CELL | `ledger::protocol::unit_tests::t_cell_worker_operations_and_cell_appends` | (b) named non-explorer mutant-test FAIL | CANDIDATE_READY | CARRIED_FORWARD | pending | NOT_ATTEMPTED |
| C21 | the well-formed test of step 1 is removed | W6 | `ledger::protocol::unit_tests::w6_one_identity_under_two_contents` | (b) named non-explorer mutant-test FAIL | CANDIDATE_READY | CARRIED_FORWARD | pending | NOT_ATTEMPTED |
| C22 | the ticket derives `Clone` | T-OWN | `ledger::protocol::unit_tests::t_own_tickets_snapshots_and_the_worker_cannot_be_cloned` | (d) intentional mutant compile failure | CANDIDATE_READY | CARRIED_FORWARD | pending | NOT_ATTEMPTED |
| C27 | the guard is created after the entry rule | T-PREC | `ledger::protocol::unit_tests::t_prec_persistence_of_every_ending` | (b) named non-explorer mutant-test FAIL | CANDIDATE_READY | CARRIED_FORWARD | pending | NOT_ATTEMPTED |
| C28 | resolve's "present, frontier below N" check alone is removed | T-RES | `ledger::protocol::unit_tests::t_res_every_resolve_row` | (b) named non-explorer mutant-test FAIL | CANDIDATE_READY | CARRIED_FORWARD | pending | NOT_ATTEMPTED |
| A1 | the writer confirms before it records | P1 | `ledger::protocol::model_tests::full_f1` | (a) original-explorer mutant FAIL | CANDIDATE_READY | ARCHITECT_DEFINED | pending | NOT_ATTEMPTED |
| A2 | admission is split into two steps under two separate holds of the test's lock. Step one takes a `view` and keeps only its low 16 bits: phase, session, latch, cells, handover fault. Any actor may step in between. Step two takes a second `view`, keeps only `unapplied`, reads the debt count, H and A, and decides from the kept bits of step one and the values of step two. | P1 | `ledger::protocol::model_tests::full_f1` | (a) original-explorer mutant FAIL | CANDIDATE_READY | ARCHITECT_DEFINED | pending | NOT_ATTEMPTED |
| A3 | the worker empties X before it raises H | P12 | `ledger::protocol::model_tests::full_f5` | (a) original-explorer mutant FAIL | CANDIDATE_READY | ARCHITECT_DEFINED | pending | NOT_ATTEMPTED |
| A4 | the drop paths record a blanket Uncertain | **nothing may fail** | `F-PT` | (f) the A4 comparison (AD-1-R4 F.1) | SATISFIED — ARCHITECT ACCEPTED | comparison | pending | NOT_RERUN |
| A5 | settlement claims a Conflict obligation while the store is not Healthy | L1 | `ledger::protocol::model_tests::full_f2` | (a) original-explorer mutant FAIL | CANDIDATE_READY | ARCHITECT_DEFINED | pending | NOT_ATTEMPTED |
| A6 | "recovery required" is a flag, not a comparison of H with A | P12 / P15 | `ledger::protocol::model_tests::full_f5` | (a) original-explorer mutant FAIL | CANDIDATE_READY | ARCHITECT_DEFINED | pending | NOT_ATTEMPTED |

The machine-readable record — edits, patches, identities, receipt and restoration mapping — is `catalog.json`; `PATCH-IDENTITIES.md` lists every patch and mutant identity.

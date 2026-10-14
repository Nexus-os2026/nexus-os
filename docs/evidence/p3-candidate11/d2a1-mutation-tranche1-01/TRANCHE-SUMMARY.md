# Mutation tranche 1 — summary

Mission P3-C11-DR01-D2A1-MUTATION-TRANCHE1-01. **No control was attempted. Tranche kill count: 0.** The catalog is CONTROL_CATALOG_BLOCKED (`CONTROL-CATALOG-BLOCKER.md`), so mission 8 and 19 authorise no build and no test. Nothing below is a kill, a survival or a verdict.

Baseline exact receipts (mission 10): **not run** — X-1 alone, and the five named unit tests alone. The pinned executable `cde9f9d2…` was hashed and sized only; it was never run.

| Order | Control | Category | Catalog status | Patch sha256 | Mutant blob | Mutant executable | Selected test | Run | Restoration |
|---|---|---|---|---|---|---|---|---|---|
| 1 | C22 | Intentional compile failure | READY | `e017ced648c6d432…` | `694b744de3b7…` | NOT PRODUCED BY DESIGN | `t_own_tickets_snapshots_and_the_worker_cannot_be_cloned` | NOT ATTEMPTED | not applicable (no mutant worktree was created) |
| 2 | C14 | Named non-explorer unit tests | READY | `2c7f17dafd7347cc…` | `72556e3f03a5…` | not built | `t_bind_every_bound_value_belongs_to_one_ledger` | NOT ATTEMPTED | not applicable (no mutant worktree was created) |
| 3 | C19 | Named non-explorer unit tests | READY | `066cc479a89ea277…` | `899878323428…` | not built | `t_cell_worker_operations_and_cell_appends` | NOT ATTEMPTED | not applicable (no mutant worktree was created) |
| 4 | C21 | Named non-explorer unit tests | READY | `65029c7b396057e3…` | `80662d4c7f8b…` | not built | `w6_one_identity_under_two_contents` | NOT ATTEMPTED | not applicable (no mutant worktree was created) |
| 5 | C27 | Named non-explorer unit tests | READY | `bb88f0446b1e56e8…` | `bbcaaaf1dcf9…` | not built | `t_prec_persistence_of_every_ending` | NOT ATTEMPTED | not applicable (no mutant worktree was created) |
| 6 | C28 | Named non-explorer unit tests | READY | `519378de0e6b6a05…` | `446d0365e147…` | not built | `t_res_every_resolve_row` | NOT ATTEMPTED | not applicable (no mutant worktree was created) |
| 7 | C12 | Original explorer | READY | `d484b4d7f5699eb2…` | `d1e46ea63bb3…` | not built | `x1_two_facts_two_writers_one_admission` | NOT ATTEMPTED | not applicable (no mutant worktree was created) |
| 8 | C9 | Original explorer | READY | `cda8e8d473a1a465…` | `7ba4a8247ba4…` | not built | `full_fb2` | NOT ATTEMPTED | not applicable (no mutant worktree was created) |
| 9 | C11 | Original explorer | BLOCKED | — (no definition) | — | not built | `full_fb2` | NOT ATTEMPTED | not applicable (no mutant worktree was created) |
| 10 | C17 | Original explorer | BLOCKED | — (no definition) | — | not built | `full_fb2` | NOT ATTEMPTED | not applicable (no mutant worktree was created) |
| 11 | C18a | Original explorer | BLOCKED | — (no definition) | — | not built | `full_fg` | NOT ATTEMPTED | not applicable (no mutant worktree was created) |
| 12 | C18b | Original explorer | BLOCKED | — (no definition) | — | not built | `full_fg` | NOT ATTEMPTED | not applicable (no mutant worktree was created) |

Unattempted outside the tranche (not authorised by mission 16): C1, C2, C3a, C3b, C3c, C4, C5, C6, C7, C8, C10, C15, C16, C20, C23, C24, C25, C26, A1, A2, A3, A5, A6. A4 not rerun. M01–M183 not executed.

Named properties and configurations of the tranche, as the catalog derives them:

- C22: T-OWN — `ledger::protocol::unit_tests::t_own_tickets_snapshots_and_the_worker_cannot_be_cloned`
- C14: T-BIND — `ledger::protocol::unit_tests::t_bind_every_bound_value_belongs_to_one_ledger`
- C19: T-CELL — `ledger::protocol::unit_tests::t_cell_worker_operations_and_cell_appends`
- C21: W6 — `ledger::protocol::unit_tests::w6_one_identity_under_two_contents`
- C27: T-PREC — `ledger::protocol::unit_tests::t_prec_persistence_of_every_ending`
- C28: T-RES — `ledger::protocol::unit_tests::t_res_every_resolve_row`
- C12: E2 — `ledger::protocol::model_tests::x1_two_facts_two_writers_one_admission`, ceiling 250,000 states
- C9: E4 — `ledger::protocol::model_tests::full_fb2`, ceiling 30,000,000 states
- C11: E4 — `ledger::protocol::model_tests::full_fb2`, ceiling 30,000,000 states
- C17: E4 — `ledger::protocol::model_tests::full_fb2`, ceiling 30,000,000 states
- C18a: E7 — `ledger::protocol::model_tests::full_fg`, ceiling 30,000,000 states
- C18b: E7 — `ledger::protocol::model_tests::full_fg`, ceiling 30,000,000 states

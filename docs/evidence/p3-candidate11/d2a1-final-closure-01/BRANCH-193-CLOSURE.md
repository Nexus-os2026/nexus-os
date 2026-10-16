# The 193-row branch accounting

**Is Section 8.4 closed, with unobserved branches disclosed? The consolidated 8.4 record now exists, and every
unobserved branch is disclosed. Its acceptance as contract 8.4's record is the Architect's decision.**

| Classification | Rows | Basis |
|---|---|---|
| MEASURED_COMPLETE | **85** | taken by at least one of the 19 completed runs (quotient runs listed separately) |
| MEASURED_IN_INCOMPLETE_PREFIX | **3** | `resolve: Stale` (B132), `append_cell C: Stale` (B146), `empty: Stale` (B186): seen only in F-R's incomplete prefix; also DERIVED_BY_ACCEPTED_PROOF TAKEN for F-R; existence, not coverage |
| UNOBSERVED | **105** | taken by no run — **not covered** |

Recomputed from raw receipts against the 193-row vocabulary (not hardcoded). Each source is recorded separately per
row; a derived proof result never changes a row's classification; no unobserved row is marked covered. The F-R proof
disposition (57 TAKEN, 106 NOT TAKEN, 30 NOT APPLICABLE; none NOT_ESTABLISHED) is recorded beside the measurements.

Also in the 8.4 record: the 34 `#[ignore]` model tests with their receipts or adopted dispositions, the witness-only C7
test, and the 45 unrelated ignored workspace tests listed separately (`02-IGNORED-TEST-DISPOSITIONS.md`).

Details: `02-BRANCH-VOCABULARY.tsv`.

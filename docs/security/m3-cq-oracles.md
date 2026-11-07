# M3-Cq oracle and plant register (purpose qualification)

Spec: `m3cq-spec-v2.md` (rulings CQ-R1, CQ-R2, CQ-R3). Cq-0 is frozen by CQ-R3 as the Cq-0
section of spec v2. Cq-1 and Cq-2 follow spec v3; their rows below are pre-registered from
spec v2 §6–§8 and are amended, by an explicit register commit, to spec v3 before Cq-1 starts.

## Cq-0 — purpose types and qualification records (unused)

Base: `github/main` 40b776b2. Files: `kernel/src/inference/purpose.rs`,
`kernel/src/inference/mod.rs`, `kernel/src/inference/purpose_tests.rs`, this register. No
transport, guard, manifest or lock change.

Claim: the kernel has a closed `Purpose` set with stable codes, a positive `Floor` made only by
the checked `freeze_floor`, and a source-frozen `QUALIFICATIONS` table that matches a record
only on both its purpose and the whole identity digest. The table is empty: no model is
qualified for any purpose. Cq-0 claims nothing about calls: no transport consults these items
yet, and `SCAFFOLD_FLOOR` still governs replies until Cq-2.

| Item | Contract |
|---|---|
| `Purpose` | `PageBuilder`, `N1Worker`, `Judge`; closed, no default |
| `Purpose::code` | `page_builder`, `n1_worker`, `judge`; distinct, none a prefix of another |
| `Floor` | `NonZeroU32`; made only by `freeze_floor` |
| `freeze_floor(n)` | `n.checked_sub(32)`, frozen only if positive; `const fn` |
| `QualificationRecord` | `{ purpose, identity_sha256 (identity v2 digest), floor, evidence }` |
| `QUALIFICATIONS` | `&[]`; one record per purpose and identity (compile-time check) |
| `qualified_floor(purpose, identity)` | the record naming both, else `None`; no `cfg`, the same in every build |

### Oracles (RED/GREEN on the oracle-add stubs)

| # | Oracle | Required assertions | Stub | Final |
|---|---|---|---|---|
| C0-1 | `cq0_freeze_floor_is_checked_and_positive` | `freeze_floor(31)` and `(32)` `None`, then all of `0..=32`; 33 → 1; `u32::MAX` → `u32::MAX − 32`; `const` use (619 → 587) | RED | GREEN |
| C0-2 | `cq0_purpose_codes_are_stable` | the three codes byte for byte; none a prefix of another | RED | GREEN |
| C0-3 | `cq0_record_matches_whole_identity` | same purpose and digest → floor; same purpose, other digest (other purposes' v2, retired v1) → `None`; other purpose, same digest → `None`; two records answer each for itself; a duplicate purpose and identity is refused | RED | GREEN |
| C0-4 | `cq0_production_qualifications_are_empty` | `QUALIFICATIONS` empty; `qualified_floor` `None` for every purpose × the three v2 digests and v1; `purpose.rs` code has no `cfg`, no `static` item, no `std::env` | GREEN (fail-closed negative) | GREEN |

The digests are spec v2 §3.1's identity v2 goldens of the fake pin (`qwen3.6:35b-a3b`,
`0.40.13`, `num_ctx=8192;temperature_milli=0;seed=7;think=off;truncate=false`):
page_builder `c5b838e3…91c5b978`, n1_worker `5fad3aa4…010df9c9c`, judge `b46c2ff5…6a008584`,
v1 (retired) `4d7211f0…db82f9a4`.

### Plants (restored by sha256, never committed)

| Plant | Exact mutation | Killing assertion |
|---|---|---|
| P-Cq-0a | `Purpose::Judge.code()` returns `"page_builder"` | C0-2: `Judge.code() == "judge"` |
| P-Cq-0b | `find` compares the purpose only | C0-3: the same purpose for another identity → `None` |
| P-Cq-2 (MP-M3-26) | `freeze_floor` uses `wrapping_sub(32)` | C0-1: `freeze_floor(31) == None` |

## Cq-1 / Cq-2 — pre-registered from spec v2 (amended to spec v3 before Cq-1)

"Stub" for Cq-1 is its inert head: workspace migrated and compiling, purpose carried but not
enforced, identity v1, floor 256, fixture endpoints not checked. Compile errors are never RED.
Every refusal oracle has a positive reachability control (qualified, matching purpose:
accepted, one charge).

| # | Oracle | Required assertions (spec v2 §6) | Stub | Final |
|---|---|---|---|---|
| O-1 | `cq_verified_identity_selects_floor` | per fixture purpose F: prompt 0, F−1 → `Truncated`, F → accepted, one charge; Judge F=100 accepted at 100 | RED | GREEN |
| O-2 | `cq_unqualified_transport_refused_before_any_io` | `PurposeNotQualified` (also for a qualified request purpose); 0 requests since registration; 0 meter | RED | GREEN |
| O-3 | `cq_wrong_purpose_refused_before_any_io` | both qualified, purposes differ: `PurposeMismatch`; 0 requests; 0 meter | RED | GREEN |
| O-4 | `cq_refusal_precedence` | not-qualified over mismatch over request checks; N1 route `ConfigMismatch` first | RED | GREEN |
| O-5 | `cq_qualification_binds_measured_identity` | another digest/version/config under a qualified purpose: `PurposeNotQualified`, 0 requests | RED | GREEN |
| O-6 | `cq_production_path_ignores_fixtures` | parsed endpoint beside a live fixture: `PurposeNotQualified`; provenance `Frozen` | RED | GREEN |
| O-7 | `cq_fixture_endpoint_reaches_only_held_listener` | hold expired: `Unavailable`, 0 new accepts (peer still serving) | RED | GREEN |
| O-8 | `cq_fixture_provenance_is_marked` | fixture transport provenance `Fixture` | RED | GREEN |
| O-9 | `cq_identity_v2_goldens` | §3.1 goldens, hand-derived bytes, v1 retired | RED | GREEN |
| O-10 | `cq_wire_bytes_unchanged` | chat body and Cb QM-MIN wire bytes unchanged | GREEN | GREEN |
| O-11 | `cq_worker_pin_binds_purpose` | pins differing only in purpose unequal; ledger `purpose` | RED | GREEN |
| O-12 | `cq_worker_binds_only_n1` | PageBuilder transport → `PurposeMismatch`; route shut → `ConfigMismatch` first | RED | GREEN |
| O-13 | `e_o65_…` (amend) | production pair request → `PurposeNotQualified`, 0 connections, 0 charge | RED | GREEN |
| O-14 | `cq_floor_boundary` (amend) | E-O8b boundaries at the transport's fixture floor | RED | GREEN |
| B-1 | `cq_purpose_refusals_propagate_content_free` | broker classes, record outcome, 0 charges, content-free | RED | GREEN |
| B-2 | `cq_refusal_accounting_binds_sb3` | one attempt, one native result, 0 charge/settlement, outcome recorded, slot/pin released; exhaustion; recording failure → debt | RED | GREEN |
| M-1 | `cq_broker_class_round_trip` | both classes round-trip `BrokerClass` | GREEN | GREEN |
| A-1 | `mother-fixture-artifacts.sh` | marker absent from fixture-free Mother executables; activation controls present | n/a | GREEN |

| Plant | Exact mutation (spec v2 §8) | Killing assertion |
|---|---|---|
| P-Cq-1 (MP-M3-25) | qualification looked up for the request's purpose | O-2: `PurposeNotQualified` |
| P-Cq-3a | purpose checks after `check_runtime` | O-2: 0 requests since registration |
| P-Cq-3b | purpose checks after the charge | O-2: meter 0 |
| P-Cq-4a | `prompt <= floor` | O-1: F accepted |
| P-Cq-4b | `prompt + 1 < floor` | O-1: F−1 `Truncated` |
| P-Cq-5 | `prompt < floor.max(256)` | O-1: Judge at 100 accepted |
| P-Cq-6 | identity v2 without the purpose | O-9: page_builder golden |
| P-Cq-7 | fixture hold check removed | O-7: `Unavailable` |
| P-Cq-8 | refusal mapped to `model_unavailable` | B-1: class |
| P-Cq-9 | parsed endpoint consults fixture records | O-6: `PurposeNotQualified` |
| P-Cq-10 | worker bind purpose check removed | O-12: `PurposeMismatch` |
| P-Cq-11 | `ModelPin` equality ignores purpose | O-11: pins unequal |
| P-Cq-12 | refusal refunds the attempt | B-2: attempts used 1 |
| P-Cq-13 | artifact scanner always "absent" | A-1: unification control present |
| P-Cq-14 | N1 route check after qualification | O-4: `ConfigMismatch` |

MP-M3-25 (original, "floor read from the request") is recorded EQUIVALENT while O-3's mismatch
rejection holds.

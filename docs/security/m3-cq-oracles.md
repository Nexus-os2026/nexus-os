# M3-Cq oracle and plant register (purpose qualification)

Spec: Cq-0 is frozen by CQ-R3 as the Cq-0 section of `m3cq-spec-v2.md` (rulings CQ-R1, CQ-R2,
CQ-R3). Cq-1 and Cq-2 follow the frozen `m3cq-spec-v6.md` (SB3E-R154, R156, R157); their
register is the "complete v6 register" section below.

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

## Cq-1 / Cq-2: the complete v6 register (pre-registered before any behavioral change)

Binding: the frozen spec `m3cq-spec-v6.md` (sha256 aa394554…4c84239), SB3E-R154, R156 and
R157. This section replaces the spec v2 pre-registration. RED/GREEN are expectations; a
result is recorded only when its gate has run. A compile error is never RED. An unknown or
unrun result is UNVERIFIED, not GREEN.

Units, in order: Cq-1a (inert kernel fixture surface) → Cq-1b-k (type/caller migration) →
Cq-1c (kernel/worker oracle-add) → Cq-1d (broker/session/Mother oracle-add) → [Q10 record
commit] → Cq-2a (enforcement). They integrate once, as one stack, after U22 (O1b).

"Inert" is the compiling stack after Cq-1a/1b-k, before the record and Cq-2a: purpose fields and
arguments exist; fixture constructors keep their owners and route through the real verifier;
the exchange lease returns `Ordinary`; ordinary `verify` does not yet reject a cloned fixture
endpoint; call does not consult purpose; `read_chat` still uses 256; digest is v1;
`ModelPin::of` hardcodes `N1Worker`; `provenance()` is `Frozen(None)`. Above the kernel, H
positives pass on the 256 floor; no checkpoint before Cq-2a proves a record-qualified call.
The route lane does not compile at the 1b-k, 1c and 1d heads; lane evidence is taken only on
the final head.

### Observation vocabulary (v6 §6.1)

- **Z**: each of these deltas is zero, under its fixed label: `cq.no_accepts`,
  `cq.no_connection_events`, `cq.no_metadata`, `cq.no_chat`, `cq.no_charge_calls`,
  `cq.no_charge_entries`, `cq.no_settlements`. Taken after synchronization (accepts drained to
  WouldBlock, handlers joined, Snapshot acknowledged only when stable; 10 s bound; no sleep).
- **H**: the healthy matching-purpose control on the same peer: accept and event deltas 4,
  metadata [1,1,1,0], chats 1, charge calls 1, charge entries 1, settlements 1. Above the
  kernel, H is `qualified_pin()` + PageBuilder on the ordinary or session path.
- **C**: `cq_accept_counter_counts_connect_without_request`: a raw connect with no HTTP counts
  accepts/events +1 and nothing else.
- Registration: metadata [1,1,1,1], chats 0. A purpose refusal after broker admission still
  consumes one attempt and one native result; zero meter settlement is not zero broker
  settlement.

Kernel fixture profiles (synthetic constants, never evidence): STANDARD = PageBuilder floor 300
(`freeze_floor(332)`), N1Worker 256 (288), Judge 100 (132) on the §3.1 v2 goldens; JUDGE_ONLY;
EMPTY; N1_CONTEXT_16384 = STANDARD + the N1 num_ctx=16384 fake, digest
`116f4ff9bbfdc908bddcb2f3f61f356d1e44084b32296fe13b4d1e0b66244bf6` (217 bytes).

### Record-independence rule (v6 §6.7; binding above the kernel)

1. Every production-path negative that a record could turn into success uses an identity no
   production record can hold: `unrecorded_pin()` (qwen3:8b, "ab"×32) or the Judge purpose.
2. The only PurposeMismatch reachable above the kernel is `qualified_pin()` registered for
   PageBuilder with a Judge request, asserted on the record base (Cq-2a's head).
3. Every production-path positive uses exactly `qualified_pin()` + PageBuilder.
4. Applied: O-6 → Judge; F-3's ordinary instance → Judge or the unrecorded pin; N-2's ordinary
   negative → Judge; B-1..B-4, S-ID1/2, N-4 per items 1-3; S-ID2's matching Judge call is a
   refusal.
5. C0-4 is amended only by the Q10 record commit (single record, cfg-free scan kept).
6. A test that changes outcome at the record commit or at Cq-2a without being listed here or in
   v6 §9.1.5 is a stop.

### Kernel, fixture and preservation oracles (v6 §6.4)

K = `kernel/src/inference/cq_tests.rs`, F = `kernel/src/inference/cq_fixture_tests.rs` (both
children of `inference::tests`, Cq-1c), I = `kernel/src/inference/tests.rs`,
W = `kernel/src/coding_run/m2e_a2_tests.rs`, L = `kernel/src/coding_run/p0b_ledger_schema_tests.rs`.

| ID | Literal test | File | Required assertions (controls included) | Inert | Final |
|---|---|---|---|---|---|
| O-1 | `cq_verified_identity_selects_floor` | K | per STANDARD purpose (seam open for N1 only): prompt 0 and F−1 → `Truncated`, F accepted; each call H's charge/settle; PageBuilder F=300, Judge F=100 discriminate | RED; N1 subcases GREEN | GREEN |
| O-2 | `cq_unqualified_transport_refused_before_any_io` | K | EMPTY same-purpose and JUDGE_ONLY PageBuilder transport + Judge request → `PurposeNotQualified`, Z; Judge matching transport H; `cq.transport_qualification_precedes_mismatch` | RED | GREEN |
| O-3 | `cq_wrong_purpose_refused_before_any_io` | K | STANDARD PageBuilder transport + Judge request → `PurposeMismatch`, Z; matching PageBuilder H | RED | GREEN |
| O-4 | `cq_refusal_precedence` | K | oversized Pair: unqualified+mismatch → NotQualified; qualified+mismatch → Mismatch; qualified+same → `RequestTooLarge`; closed-route N1 (request, identity, both) and every conversation → `ConfigMismatch` first; open-seam controls reach qualification/mismatch; Z in all refusals; H | RED; conversation-closure and size subcases GREEN; closed N1 pairs RED until 2a | GREEN |
| O-5 | `cq_qualification_binds_measured_identity` | K | name, manifest, runtime, seed, context each changed under PageBuilder, peer honest, registration succeeds; call → `PurposeNotQualified`, Z; original pin H | RED | GREEN |
| O-6 | `cq_production_path_ignores_fixtures` | K | ordinary parse of a live STANDARD listener, ordinary-verify a **Judge** pin: verify Ok, call → `PurposeNotQualified`, Z, provenance `Frozen(None)`; typed verification of the same peer H and `Fixture` | RED | GREEN |
| F-1 | `cq_expired_fixture_refuses_while_peer_serves` | F | expiry before `verify_fixture` and after; the server clone keeps serving; `Unavailable`, Z (`cq.no_accepts` first); live-owner H; raw connect to the clone afterwards proves it reachable | RED | GREEN |
| F-2 | `cq_exchange_lease_outlives_owner` | F | pause after lease acquisition (cfg(test) hook), drop owner, `authority_alive_for_test()==true` before resume; second lease `Unavailable`; in-flight exchange completes; authority released after joined cleanup on every OS; rebind diagnostics Linux only; scope guard always releases | RED (liveness assertion, not a timeout) | GREEN |
| F-3 | `cq_fixture_provenance_is_marked` | F | typed instance `Fixture`, also after expiry; ordinary instance of Judge or the unrecorded pin `Frozen(None)` | RED | GREEN |
| F-4 | `cq_accept_counter_counts_connect_without_request` | F | C exactly; H counts metadata/chat/charge/settlement | GREEN | GREEN |
| F-5 | `cq_fixture_records_reject_duplicate_keys` | F | duplicate (purpose, digest), also with different floors → `InvalidInput`; distinct and EMPTY accepted; no mutation API | GREEN | GREEN |
| G-1 | `cq_fixture_gate_is_test_only` | F | reads sources with `std::fs` from `CARGO_MANIFEST_DIR` (R157 g4). Part 1: every fixture.rs item, the `Source::Fixture` variant and arms and every fixture-only helper carry exactly `#[cfg(test)]` (never `any(..)`, never a feature). Part 2: the `mod fixture;` declaration carries `#[cfg(test)]`. Part 3: no `#[path]`/`include!` names fixture.rs outside a cfg(test) parent. Part 4: kernel/build.rs prints no `cargo:rustc-cfg` | GREEN | GREEN |
| R-3 | `cq_ordinary_verify_rejects_cloned_fixture_endpoint` | F | `verify(typed.endpoint().clone(), ..)` → `ConfigMismatch`, Z; `verify_fixture` H; parse of the same address stays ordinary | RED | GREEN |
| O-9 | `cq_identity_v2_goldens` | K | the three standard, three session and the 16384/N1 goldens from literal bytes and lengths; v1 differs | RED | GREEN |
| O-9a | `a1_identity_digests_are_pinned` (amend) | I:1762 | default fake N1Worker → `5fad3aa4…f9c9c`; runtime config golden unchanged | RED | GREEN |
| O-10 | `cq_wire_bytes_unchanged` | K | three purposes, identical chat bytes, no purpose field; literal expected body | GREEN | GREEN |
| O-14 | `e_o8b_a_prompt_count_below_the_qualified_floor_is_refused` (amend; renamed from `e_o8b_a_prompt_count_below_the_scaffold_floor_is_refused`) | I:1834 (15eb63e7: :2156) | explicit PageBuilder F=300: 0/1/299 refused, 300/587 accepted; 5 charges/settlements/chats; `PROVISIONAL_FLOOR` (:1827) removed | RED at 299 | GREEN |
| PXY | `e_o10_proxy_variables_are_never_used` (amend) | I:1458 | child owns its fixture; parent trap zero accepts, then one raw connect → exactly one accept; no model address in env/argv | GREEN | GREEN |
| PXY-child | `e_o10_proxy_child` (amend) | I:1508 | under PROXY_CHILD=1: one direct chat, reply, 4+3 metadata, one charge/settle, sentinel | GREEN | GREEN |
| N-1 | `cq_n1_pair_and_conversation_require_route_and_record` | K | seam shut: N1 pair and conversation `ConfigMismatch`+Z; seam open H; open+EMPTY `PurposeNotQualified`+Z; request-only and transport-only N1 shut `ConfigMismatch`+Z; open qualified wrong purpose `PurposeMismatch`+Z; PageBuilder/Judge H without seam | RED at closed pairs and open EMPTY | GREEN |
| N-2 | `e_o65_in_a_non_test_build_bind_and_a_conversation_call_refuse_before_any_charge_or_connection` (amend) | kernel/tests/m2e_a2_guards.rs:142 (15eb63e7: :143) | ordinary N1 transport: bind, pair, conversation `ConfigMismatch`+Z; ordinary Judge pair `PurposeNotQualified`+Z; ordinary PageBuilder H (256 floor before 2a, record base at 2a); no fixture subcase | RED at pair | GREEN |
| N-3 | `n1_a_conversation_on_another_context_size_is_a_config_mismatch` (amend) | n1_tests.rs:780 | N1_CONTEXT_16384, seam open → `ConfigMismatch`+Z | GREEN | GREEN |
| N-5 | `e_o65_with_the_route_shut_a_conversation_call_refuses_before_any_charge_or_connection` (amend) | n1_tests.rs:715-716 | default-N1 pair closed → `ConfigMismatch`, zero connections/meter; same pair via `call_with_open_n1` Ok; cross-thread closure kept | closed RED; open GREEN | GREEN |
| W-1 | `cq_worker_pin_purpose_is_distinct` | W | pins from real same-endpoint PageBuilder/Judge transports differ only by purpose → unequal | RED | GREEN |
| W-2 | `cq_worker_bind_requires_n1` | W | seam open: PageBuilder/Judge → `PurposeMismatch`, N1 binds and works; seam shut: all `ConfigMismatch`; Z per failed bind | RED | GREEN |
| W-3 | `e_o13b_the_ledger_records_the_full_pin_by_digest` (amend) | W:614 | exact eight-key pin facts with literal `n1_worker` | RED | GREEN |
| L-1 | `cq_model_pinned_schema_accepts_closed_purposes` | L | each literal code emitted and accepted; `MODEL_PURPOSES` equals the literal set; exhaustive no-wildcard match | RED | GREEN |
| L-2 | `cq_model_pinned_schema_rejects_unknown_purpose` | L | "unknown", "", "N1Worker", "page_builder_extra", a number → `Rejected` | GREEN | GREEN |
| L-3 | `o12_runs_snapshots_reads_edits_and_pins` producer (amend) | p0b_ledger_privacy_tests.rs:1346 | `run.model_pinned` requires `n1_worker`; canaries kept | RED | GREEN |
| Cb-P | `cq_builder_declares_page_builder_without_wire_change` | mother/src/page_builder/oracles.rs | first and correction requests PageBuilder; :28-105 byte assertions unchanged | GREEN | GREEN |

### Broker and SB-3 oracles (v6 §6.5)

B = `crates/nexus-governed-control/src/inference_tests.rs`,
A = `crates/nexus-governed-control/src/host_session/model_call_tests.rs`.

| ID | Literal test | File | Required assertions | Inert | Final |
|---|---|---|---|---|---|
| B-1 | `cq_purpose_refusals_propagate_content_free` | B | NotQualified on a Judge registration of the served pin (or the unrecorded pin); Mismatch per rule 2; each exact `model_purpose_*` class, one `model_call` record with `purpose_*`, charge 0, Z, canaries absent; H in a fresh run | RED | GREEN (2a, record base) |
| B-2 | `cq_session_refusal_attempt_accounting` | A | per refusal, separate sessions: attempts +1, one native result, one record, Z, slot inactive, owner/child complete, pin count back to baseline; next invocation works; H | RED at outcome/Z | GREEN |
| B-3 | `cq_session_purpose_refusals_exhaust_attempts` | A | eight not-qualified refusals: attempts 1..8 never refunded, eight records, Z; ninth `OverBudget` before attempt/record | RED at outcomes | GREEN |
| B-4 | `cq_session_purpose_refusal_record_failure_keeps_debt` | A | probe `model.attempt.record.before`: `refuse_inserts(true)` → Owed, `simulate_crash()` → Uncertain; purpose error kept; no durable row; (1,1) attempts/pins; FAILED latched; next call Halted | RED at outcome | GREEN |
| S-ID1 | `cq_session_rejects_legacy_expected_identity` | A | per purpose, same-pin v1 expected identity → `ModelIdentityMismatch`, no binding/attempt/record, Z; exact v2 admits | RED | GREEN |
| S-ID2 | `cq_session_rejects_other_purpose_expected_identity` | A | other-purpose v2 identities → `ModelIdentityMismatch`, Z; correct v2 admits; PageBuilder H; Judge → `model_purpose_not_qualified`+Z; N1 pair → `ConfigMismatch`+Z | RED (correct-v2 control) | GREEN |
| M-1 | `cq_broker_class_round_trip` | mother/src/evidence.rs | both new classes round-trip; code set equals the broker's | GREEN | GREEN |
| N-4 | `cq_broker_and_session_n1_pair_stays_closed` | B and A | admitted N1 pair → `model_config_mismatch`+Z (record-independent); PageBuilder on the unrecorded pin → `model_purpose_not_qualified`; H | RED at N1 pair | GREEN |

Accounting hooks are re-anchored at Cq-1d Step 0 (v6 §6.5 table).

### Plants (v6 §8.2; temporary, restored by sha256, never committed)

Each plant: unmutated control green, mutation compiles, its named assertion fails, restore by
sha256, control green again. A build error, unrelated lint or timeout is not a kill.

| Plant | Exact mutation | Killing assertion |
|---|---|---|
| P-Cq-0a, 0b, 2 | as in Cq-0 above | C0-2, C0-3, C0-1 |
| P-Cq-1 (narrowed MP-M3-25) | the floor `let` replaced by the substituted-identity lookup for `request.purpose()` (v6 §8.1) | O-2 JUDGE_ONLY: `cq.transport_qualification_precedes_mismatch` |
| P-Cq-3a | floor/equality block moved after `check_runtime` | O-2: `cq.no_accepts`, `cq.no_metadata` |
| P-Cq-3b | block moved after the Settle guard | O-2: `cq.no_charge_calls`, `cq.no_charge_entries`, `cq.no_settlements` |
| P-Cq-4a | `prompt <= floor` | O-1: F accepted |
| P-Cq-4b | `prompt.saturating_add(1) < floor` | O-1: F−1 `Truncated` |
| P-Cq-5 | `prompt < floor.max(256)` | O-1: Judge prompt 100 accepted |
| P-Cq-6 | v2 digest without `purpose.code()` | O-9: PageBuilder golden |
| P-Cq-7 | `let lease = super::endpoint::ExchangeLease::Ordinary;` | F-1: `cq.no_accepts` |
| P-Cq-7b | `drop(lease)` moved before the paused-lease hook | F-2: `authority_alive_for_test()` |
| P-Cq-8 | the two new `MODEL_CLASSES` entries → `"model_unavailable"` | B-1: exact classes |
| P-Cq-9 | Ordinary arm of `lookup_floor` reads `fixture::standard_records_for_tests()` (v6 §8.2 block) | O-6: Judge `PurposeNotQualified` |
| P-Cq-10 | worker bind purpose check deleted | W-2: open-seam PageBuilder `PurposeMismatch` |
| P-Cq-11 | `ModelPin::of` hardcodes `N1Worker` | W-1: pins differ |
| P-Cq-12 | diagnostic only, authority/run.rs:2149 `ordinal - 1` | B-2 attempts used 1; B-3 boundary |
| P-Cq-14 | N1/conversation route block moved after floor/equality | O-4: closed conversation `ConfigMismatch` |
| P-Cq-16 | `MODEL_PURPOSES` gains `"unknown"` | L-2: `"unknown"` rejected |
| P-Cq-17 | `pin_facts` purpose → `"page_builder"` | W-3: exact `n1_worker` |
| P-Cq-18 | `.no_proxy()` deleted (exchange.rs:279) | PXY: parent trap accepts 0 |
| P-Cq-21 | fixture lease arm without the `live` check | F-2: second lease `Unavailable` |
| P-Cq-22 | `duplicate_keys` body → `false` | F-5: duplicate key → `InvalidInput` |
| P-Cq-23 | route condition → `request.is_conversation() && !conversation_route_open()` | N-1: closed-seam exact-record N1 pair `ConfigMismatch`+Z |
| P-Cq-24 | one fixture.rs attribute → `#[cfg(any(test, feature = "test-utils"))]` | G-1 part 1 |
| P-Cq-25 | `#[cfg(test)]` removed from `mod fixture;` | G-1 part 2; also the retirement live successor (below) |
| P-Cq-26′ (R157 g2, replaces P-Cq-26) | `#[cfg(not(test))] #[path = "fixture.rs"] mod fixture_alias;` in kernel/src/inference/mod.rs | G-1 part 3 |
| P-Cq-27 | kernel/build.rs prints `cargo:rustc-cfg=nexus_cq_plant` before the macOS return (local only) | G-1 part 4 |

- P-Cq-26 as frozen (`#[path = "fixture.rs"] mod fixture_alias;` with no cfg) does not compile
  in the kernel test harness (E0308: the second copy's `FixtureRecord`/`FixtureAuthority` are
  distinct types), so it proves nothing. P-Cq-26′ has the same meaning (fixture code compiled
  outside a cfg(test) parent in every non-test build) and compiles (Cq-1a Step 0, R157).
- MP-M3-25 as originally written ("use the request's floor after enforcing equal purposes") is
  EQUIVALENT while the mismatch refusal holds; it is recorded as equivalent, never as a kill.

### Retirement registry for the fixture (R157 g1)

`kernel/src/inference/fixture.rs` is declared `#[cfg(test)] pub(crate) mod fixture;`. The
workspace production scanner classifies it as test-only: the legacy predicate counts files
named by a `#[cfg(test)] mod` (kernel/tests/hm_m3_guards/source_fe5328af.rs:558-566), and the
exclusivity proof holds (source.rs:1398). So it is **not** a production path and is not added
to production-paths.txt; the 331 seed pin (kernel_retirement_manifest.rs:231-232) is unchanged.
Only the `fixture` edge is added to `kernel/src/inference/mod.rs` in module-edges.json.

Independent G1 tripwire (measured at Cq-1a Step 0): with fixture.rs unregistered, P-Cq-25 makes
`live_successors_activate_only_at_the_explicit_reviewed_stage` fail with
`f7-dependency kernel/src/inference/fixture.rs`. Registered, the same plant passed, which is why
it is not registered.

### Validation gates (v6 §7, §7.1, §9.4)

Per unit: compile the affected standard workspace, then the named tests with individual
assertions. Final stack: every kernel `--test` target resolved from Cargo metadata (43 at
f0c19aae, none omitted), `--lib`, `--doc`, fmt, all-target/all-feature clippy, the workspace
and ci.yml/audit.yml gates, every plant above one at a time; then R110.2 over every route plant
row and one full route_exit on the final head (A7 lane = ship, A5 C2 included). Fresh-context
review; Owner merge (kernel security core, D13).

## Cq-1b-k: type and caller migration, inert enforcement (SB3E-R160)

Base mother/cq1a 30498097. Measured at Step 0 (/home/nexus/.m2d/cq1bk/step0-report.md):
- `ModelRequest::new(` gains a required purpose at all 20 sites outside route_cells (23 in total; route_cells' 3 migrate in Cq-2a).
  `ModelPinSpec {` gains `purpose` at all 9 literals outside route_cells. Purpose is PageBuilder for Mother,
  broker, session and hands.rs:77. It is N1Worker for the shared kernel and worker fakes and for m2e_a2_guards.
- `ModelError` gains `PurposeNotQualified`, `PurposeMismatch`; CODES 24 → 26 (mod.rs:176). Broker
  MODEL_CLASSES and authority REASONS gain `model_purpose_not_qualified`, `model_purpose_mismatch` after
  `model_unsafe_delimiter`. Mother `BrokerClass` gains both before `Unknown`. `Purpose` is re-exported by the broker.
- `ModelIdentity` carries purpose; the digest stays identity v1 until Cq-2a. `ModelPin` carries purpose, and
  `ModelPin::of` deliberately sets N1Worker (P-Cq-11's mutant; W-1 is RED until Cq-2a).
- Kernel fakes own FixtureListeners (STANDARD; the 16384 profile for n1_tests.rs:780). Gated keeps its
  back and front owners. Verification goes through `verify_fixture`. `call`, `call_in_thread` and `call_with`
  stay closed; one `call_with_open_n1` serves the 31 enumerated pair sites and n1_tests.rs:506. The seam opens
  only on the calling thread (`observed_open_n1`, tests.rs:1516/:2200/:2265, sb1_tests.rs:303). The proxy child
  owns its fixture; the parent passes only the role "1", bounds the child and checks the trap with one raw
  connection.
- Session: `model_tests::pin()`/CHAT are the production pin. `unrecorded_pin()` keeps qwen3:8b/"ab"x32 for
  negatives. The Peer serves its pin, counts every accept before reading, and answers a synchronized
  snapshot. That snapshot is installed here and first used in Cq-1d.

Expected at this head (inert): N-5 (n1_tests.rs, e_o65_with_the_route_shut_a_conversation_call_refuses_before_any_charge_or_connection) RED at "closed N1 pair"
(Ok instead of ConfigMismatch: the pair route is not enforced until Cq-2a). Every other existing test keeps
its outcome.

Cb source pin (R160 f1). `m3cb_marker_source_is_broker_only` hashes the whitespace-normalized body of
`PageBuilderLane::invoke_in` (cb_oracles.rs:242-244). lane.rs:74 lies inside it and gains the purpose.
Normalized before/after difference, measured: only the inserted argument `Purpose::PageBuilder,` and
rustfmt's trailing `,` after `schema`; `match marker()` still occurs once. Re-pin
`12eb4d612abfb323c3951e37c727e492b8217ceabd45af6b4f6c000b0924e0d6` →
`13505ff08e4ee5938bd056d37613de244e0a70ab5595867cb8ed193304b388e3`. The test's own negative control (the
`match marker()` replacement must not pin) is kept and runs green.

Dead-code allows added here (R157 g3, R160 f4), each removed by its first-use unit: `unrecorded_pin`,
`Peer::unrecorded`, `Peer::snapshot` (Cq-1d); `ModelRequest::cq_oversize_pair`,
`pin_for_test_with_purpose` (Cq-1c). The Cq-1a allows on `FixtureListener::bind/endpoint/peer_listener/addr`
and `verify_fixture` are redundant from this head on; Cq-2a removes them (R160 f2).

## Cq-1c-i: kernel inference oracles (SB3E-R162)

Split of v6 Cq-1c (R162 f1). 1c-i adds K = `kernel/src/inference/cq_tests.rs` (O-1..O-6, O-9, O-10, N-1) and
F = `kernel/src/inference/cq_fixture_tests.rs` (F-1..F-5, G-1, R-3) as children of `inference::tests`, and amends I
(`kernel/src/inference/tests.rs`): O-9a (v2 golden `5fad3aa4…`, N1Worker), O-14 (explicit PageBuilder F=300), and PXY.
1c-ii carries W-1..W-3, L-1..L-3 and N-2.

Observation (v6 §6.1; R162 f4): `Snapshot` = accepts (counted at accept, before reading), connection_events (an
independent per-accept event, consumed by the observer), metadata [version, status, tags, show], chats, charge_calls
(every meter invocation, refused ones included), charge_entries, settlements. Z checks each field under its label
(`cq.no_accepts`, `cq.no_connection_events`, `cq.no_metadata`, `cq.no_chat`, `cq.no_charge_calls`,
`cq.no_charge_entries`, `cq.no_settlements`). H is +4 accepts and events, metadata [1,1,1,0], one chat, call, entry
and settlement. Each value is read after the synchronous call has returned: every request it made was accepted and
logged before its reply.

The shared Fake's fixture owner can expire (`Fake::expire`) while its server clone keeps serving (F-1, F-2, F-3). The
closed `call()` helper is removed (no caller after O-14); `call_in_thread` and `call_with` stay closed.

PXY (R162 f2): the parent synchronizes the trap with one raw control connection. The trap accepts serially, so any
child connection came first. It asserts exactly one accept ("the proxy was not contacted (trap accepts == 0 before the
control)") before the child-status assertion. That is P-Cq-18's named killer.

Expected at this head (inert), each on its named assertion: RED O-1 (PageBuilder F−1; N1 subcases green), O-2, O-3,
O-4 (after its conversation and size subcases pass), O-5, O-6, O-9 (after the literal encodings pass), N-1 (after
closed conversation), F-1, F-2 (authority liveness, not a timeout), F-3, R-3, O-9a, O-14 (299), N-5. GREEN: O-10,
F-4, F-5, G-1, PXY, PXY-child, N-3. Plants re-killed by these committed tests (Cq1a-N1): P-Cq-22 (F-5), P-Cq-24, 25,
26′, 27 (G-1 parts 1-4), P-Cq-18 (PXY trap). Owed: Cq-2a P-Cq-1, 3a, 3b, 4a, 4b, 5, 6, 7, 7b, 9, 10, 11, 14, 16, 17,
21, 23; Cq-1d P-Cq-8, 12.

Ledger Cq1c-N5 (R162 f5): the `set_after_lease` allow in exchange.rs is redundant from here on. It stays until a unit
whose allowlist covers exchange.rs removes it.

## Cq-0 results

Oracle-add on the inert stubs: C0-1, C0-2 RED at their named assertions; C0-3 RED in its
fixture (the stub `freeze_floor` makes no `Floor`); C0-4 GREEN (fail-closed negative). After
the feat commit all four GREEN. Plants P-Cq-0a, P-Cq-0b, P-Cq-2 each killed at the named
assertion above, restored by sha256, never committed.

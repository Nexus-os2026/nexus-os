# Session model bridge — frozen oracle and plant register (SB-1)

Status: **FROZEN for SB-1** (PR 1 of 3, design §9 SB-A4). Staging note, binding:
**the R32 allowlist is not effective; the session model route is unavailable until SB-3.**
SB-1 adds only the kernel per-request admission hook, its isolated controls and
this register. No session override, SessionDoor/SessionPort model method, host
registration or callable model route exists in production after SB-1.

- Base: `github/main` = `ac72c2f8b55ec5057e7ea7d24ab696c65386cd0b`; branch `mother/sb1-kernel-hook`.
- Authority: Owner bridge approval 2026-11-05 (session-bridge design §8); design
  §9 SB-A1–SB-A4 (adopted, binding); SB-1 mission sheet and its Architect
  rulings O1–O5 (2026-11-06).
- Commit order (MOTHER §3.1, Ruling R-1): (1) this register and the staged spec
  texts; (2) the oracles with an inert kernel surface, runtime RED; (3) the hook
  and its invocation, GREEN, with the receipts appended below. No oracle text is
  edited after a result.
- Class key: N = new API, its RED made behavioural by the inert surface of
  commit 2 (Ruling O1); G = retained control on the base; B-red = an existing
  behavioural assertion. Every SB-1 row is N unless marked G.

## 1. Adopted amendment texts

These texts are adopted verbatim from session-bridge design §5.2/§5.3. They are
staged: they take effect for a session route only in SB-3 (Ruling O4). The
M2d-D copies (R32, the R14/R32 paragraph, §1.4, A0, A6, A12) are applied in
`docs/security/m2d-d-session-spec.md` with the same staging note.

**I-E1 (new; kernel).** Anchored to `kernel/src/inference/mod.rs:243-298` at
ac72c2f8 (`CallCancel` … `CallContext` with its `observer` field; 24 error
codes incl. `UnsafeDelimiter`, mod.rs:162-168), not to the stale m2e-design §7.1
block (lines 124-151: four `CallContext` fields, 23 errors).

> - **I-E1:** the §7.1 API, with the session bridge’s one added `CallCancel::admit_request(&self, kind: RequestKind) -> Result<(), ModelError>` hook. The associated closed `RequestKind::{Metadata, Chat}` is derived by the transport from its fixed API path. The default preserves ordinary cancellation semantics. Session implementations must supply original-scope owning-hold admission; omission is a security failure. The existing exchange invokes it for every individual metadata/chat request after construction and before dispatch. The polling `cancelled()` and non-vetoing `CallObserver` keep their distinct roles. No new endpoint, transport, retry, capability or optional session bypass is introduced.

Ruling O2 qualification of "preserves": the default is **stricter, not
identical**. A cancel during a request's synchronous client construction now
yields no dispatch notice and no send (fail-closed); the result code
(`Cancelled`) is unchanged. A Stop or deadline during construction keeps its
previous behaviour, since the default checks neither. Pinned by
`sb1_o2_cancel_during_construction_is_stricter_no_notice_no_send`.

**I-E2 (replaces the door-API line).**

> - **I-E2:** Registration is owner-only: the ordinary `GovernedControl::register_local_model` remains separate; Hands uses only `HostSession::register_local_model` under its original Bootstrap scope. There is no door or lane registration route. The sealed `SessionDoor::{model_identity, admit_inference, model_call}` methods implement SB-R1–SB-R3, bound to the original session and Run request, the existing File run, the exact cached affirmative mission consent and immutable model registration. No raw door, broker, transport, alternate run or caller-minted lifetime escapes. `model_call` returns the existing `ModelCallOutcome` including record status, not a bare successful reply. Private registration, attempt, handoff, cancellation and recording machinery must satisfy SB-R0–SB-R9 and the same original close deadline.

**I-E3 (replaces the EffectPort line).**

> - **I-E3:** `EffectPort::{model_identity, admit_inference, model_call}` remain default-deny. `SessionPort` may implement exactly those three methods only through the approved SessionDoor route matrix. It resolves the mission to its already admitted File run and passes the original consent binding and C1 meter/deadline; it never creates or adopts another run. An approval continuation, replayed caller or restricted/closing session exposes no inference. A successful reply requires Recorded native attempt evidence and a still-live original invocation at result acceptance. C1’s charge, no-refund, first-cause and report-only replay rules remain binding. This exception takes effect only after the bridge oracle amendment is frozen; C1’s earlier denial evidence remains historical evidence of its earlier scope.

The obsolete m2e-design §8 return-type line (m2e-design.md:200,
`AgentDoor::model_call … -> Result<ModelReply, AuthorityError>`) is corrected to
`Result<ModelCallOutcome, AuthorityError>`, the committed ordinary door
(`crates/nexus-governed-control/src/governed/door.rs:593-599`). No caller may
discard `record()`.

## 2. Reference corrections (design vs main at ac72c2f8)

| # | Stale reference | Current main | Disposition |
|---|---|---|---|
| 1 | m2e-design §7.1 (124-151): 4 `CallContext` fields, 23 errors | `CallContext` has `observer` (mod.rs:288); 24 codes (mod.rs:162-168) | I-E1 anchored to mod.rs:243-298 (§1) |
| 2 | Design §3.3/§5.3 "default preserves ordinary cancellation semantics" | exchange.rs:243-246: a cancel during construction may be notified and sent, then `Cancelled` | Ruling O2: labelled **stricter**; exchange.rs doc rewritten; oracle O2 |
| 3 | Design §2.3 `SessionPort::admit_inference(mission, admission: &Admission, expected_identity)` → `Result<ModelReply, EffectRefusal>` | `mother/src/effect.rs:344-350` `(MissionId, &str, &str, [u8; 32])`; `effect.rs:356-362` `Result<ModelAttempt, EffectRefusal>` (enum effect.rs:131) | SB-3 follows main; recorded only |
| 4 | R32 old row: `door.rs:500,544`, `run.rs:1377,1435` | `governed/door.rs:549,593`; `authority/run.rs:1503,1561`; `inference.rs:337,371` unchanged | Old row quoted verbatim in the spec staging note with current anchors |
| 5 | m2e-design.md:200 `-> Result<ModelReply, AuthorityError>` | `governed/door.rs:593-599` `Result<ModelCallOutcome, AuthorityError>` | Corrected in §1 |

Consistent, no change: `CallObserver` is non-vetoing (exchange.rs:272-288);
`Charged{call}` with charge/settle only (mod.rs:203-240); session refusals
(`mother/src/adapter/session_port.rs:169-198`); the `InvocationLiveness` deny
latch for SB-A1 (`mother/src/coordinator/model_calls.rs:6-17`).

## 3. SB-1 rows (kernel, isolated)

Fixtures (`kernel/src/inference/sb1_tests.rs`, test-only; none ships): `Gate`
implements `CallCancel` with `cancelled()` ≡ `None` and an overriding
`admit_request` that (a) logs `(kind, fake.paths().len())`, (b) `try_lock`s its
owning `Mutex` (contended or poisoned → `Err(Cancelled)`), (c) observes its
predicate (the original generation) with an optional barrier pause, (d) CASes
`word: AtomicU64` from (gen, n) to (gen, n+1); ingress sets gen+1 without the
hold; (e) optionally denies admission i. A test-only `after_construction` seam
runs on the exchange thread after each request's construction, before its
admission. Reused: `Fake`, `Ctx`, `Meter`, `Flag`, `in_thread`, `done`. The
`dispatch::Recorder` is private to its module, so an equivalent content-free
`Notices` observer is defined in the fixture file (tests.rs keeps its 3-line
change). Evidence: fake-peer request log, connection count, `Outcome.live == 0`,
meter events, notices.

| ID | Test | Literal expected result | At commit 2 |
|---|---|---|---|
| SB1-K1 | `sb1_default_hook_admits_live_and_refuses_cancelled` | `Flag(false)`: `Ok` for Metadata and Chat; `Flag(true)`: `Err(Cancelled)` for both | GREEN |
| SB1-K2 | `sb1_hook_runs_once_per_request_with_its_kind` | verify: log [(M,0),(M,1),(M,2),(M,3)], peer version,status,tags,show; call: [(M,0),(M,1),(M,2),(Chat,3)] after drain; hook count = peer requests = exchanges = 4 | RED |
| SB1-K3 | `sb1_registration_metadata_refused_at_each_gate_sends_nothing_further` | deny i ∈ 0..4 at verify: `Err(Cancelled)`; paths = i; connections = i; live 0; hook calls i+1 | RED |
| SB1-K4 | `sb1_call_metadata_refused_at_each_gate_charges_and_sends_nothing` | deny i ∈ 0..3 at call: `Err(Cancelled)`; post-drain paths = first i; `/api/chat` 0; meter `NOTHING` | RED |
| SB1-K5 | `sb1_chat_refused_at_its_gate_after_the_charge_sends_and_notifies_nothing` | deny Chat: `Err(Cancelled)`; paths version,status,tags; `/api/chat` 0; meter [Charge(1), Settle(1)]; notices [] (observer installed); positive control: admitted chat notified once (Linux/macOS) | RED |
| SB1-K6 | `pre_gate_ingress_request_count_eq_zero` | seam ingress after construction of each request i (registration and call; includes Version@verify, Tags@call, Chat@call): `Err(Cancelled)`; that and later requests 0; notices 0 | RED |
| SB1-K7 | `session_override_required_at_each_request` | `cancelled()` None; the owning hold held by another thread immediately before admission of each of 4 registration metadata, 3 call metadata and chat: refusal, zero peer requests for that and later paths, zero notices, no CAS. Re-registered under the same name for the real override in SB-2 (registration) and SB-3 (call/chat) | RED |
| SB1-K8a | `sb1_paused_before_cas_ingress_wins_dispatches_nothing` | pause after predicate, before CAS at each request: ingress wins → 0 requests for that and later, 0 notices, CAS failed; no interference → full sequence | RED |
| SB1-K8b | `sb1_paused_before_cas_sink_wins_admits_only_that_request` | CAS before ingress → exactly that one request, next admission refuses, 0 successors (the last request's entered call completes) | RED |
| SB1-K9 | `attempt_chat_count_le_one` | deny Chat once: hook called once for Chat; `/api/chat` 0; `Err(Cancelled)`; no kernel retry | RED |
| SB1-O2 | `sb1_o2_cancel_during_construction_is_stricter_no_notice_no_send` | ordinary default hook; cancel set after construction of each request: `Err(Cancelled)` (unchanged code), no send, no notice | RED |
| SB1-O3 | `sb1_o3_hook_stopped_matches_the_pre_dispatch_check` | hook `Err(Stopped)` at chat ≡ existing pre-dispatch check after a Stop at charge: (`Err(Stopped)`, 0 halts recorded, 0 chat, [Charge(1), Settle(1)], 0 notices) | RED |
| SB1-S1 | `sb1_hook_is_called_once_between_construction_and_send` (`kernel/tests/m2e_transport_guards.rs`) | exactly one production `admit_request(` call in `kernel/src/inference/**`, inside `async fn request`, after `.build()`, before `dispatched(` and `.send()`; the `RequestKind` match has no `_` arm; planted red (missing, before build, after send, wildcard) and green controls. Supplementary only | RED |

SB1-R retained (G, assertions unchanged): E-O1…E-O18; `m2e_a1b_*` M3-O44/O45
and `dispatch_panic`; n1 E-O60–66; `kernel/tests/{m2e_transport_guards,
m2e_a2_guards, m0_f5_emergency_stop}.rs`; broker `inference_tests.rs`; C1-S1
`c1_s1_sealed_session_denies_inference` and
`c1_s1_session_sources_have_no_model_route`
(`mother/src/adapter/c1b_port_oracles.rs:266,297`).

Ruling O3: a hook-returned `Stopped` returns through `Ended::Error` with no
`record_stop()`, exactly like the existing pre-dispatch `guard.check()`.

## 4. SB-1 mutation plants (local, never committed)

| Plant | Mutation | Must fail |
|---|---|---|
| MP-SB1-1 (MP-SB-6 omit) | delete the hook call | K3, K4, K5, K7 |
| MP-SB1-2 (MP-SB-6 early) | hook at `exchange()` entry, before client construction | K6 |
| MP-SB1-3 (observer veto order) | hook after the observer notice | K5 notices == [] |
| MP-SB1-4 | `let _ = admit.admit_request(kind);` | K3, K4, K5 |
| MP-SB1-5 (MP-SB-6 retry) | re-call the hook after `Err` | K9 |
| MP-SB1-6 | kind mapping wrong (Chat→Metadata; or a `_` arm) | K2; S1 |
| MP-SB1-7 | hook only on the first request of a sequence | K3 i≥1, K4 i≥1 |
| MP-SB-6-default | Gate override removed (default used) | K7 |
| MP-SB1-8 | Gate CAS replaced by a plain store | K8 ingress-wins |
| MP-SB1-9 | default returns `Ok` always | K1 |
| MP-SB1-10 | default returns `Err` always | K1; E-O5 positive |

Rule: a plant masked by another guard is reported "not exercised" and redesigned
through review; no guard is weakened to manufacture a kill.

Ruling O5 (binding for SB-2 and SB-3): MP-SB-6-default and the paused-before-CAS
schedule are re-run against the real session override. SB-1's fixtures prove only
that the kernel honours the hook, not session CAS correctness.

## 5. C1-S1 → C1-S1-SB (pending; effective only in SB-3)

C1-S1 is not touched in SB-1, and no observed C1 result is edited or relabelled.
Old assertion at ac72c2f8: `c1_s1_sealed_session_denies_inference` and
`c1_s1_session_sources_have_no_model_route`
(`mother/src/adapter/c1b_port_oracles.rs:266,297`; file SHA-256
`ded82c1a3b18f23e30bfbbf58a521329392045d31bc545052ed92f250f5566f5`). Fixture
tables at ac72c2f8 (SHA-256): `mother/host/tests/session/exclusions.tsv`
`c57b6bac32ab71454fcda4b49fc6cad7cf5b100cf71e2745d09088264ed472b4`;
`sinks.tsv` `a3de0e207f854a5f37e6655503cf5e016423911a17f10830d55a0292cd2e2a32`;
`combination-boundaries.tsv`
`43fdafc526d5dfc2a32047c464fbaae92d082ae409830db51b0b8f3f8716e272`.

Replacement, verbatim (design §6.1):

> A fully registered, originally consented, live session with its existing File run and budget successfully executes SB-R0–SB-R8 through SessionPort; every missing, mismatched, expired, restricted or unlisted route refuses at its named boundary. No raw route, lane registration, alternate run, adoption or host feature is exposed. Approval continuations and replay remain denied. Configured ordinary broker positive controls remain. A blanket-deny implementation fails SB-O1.

## 6. Frozen bridge oracles SB-O1–SB-O15 and plants MP-SB-1–15 (verbatim, design §6.2)

Every row below is registered with an independently authored expected table, fixtures, exact assertion/test name and plant location before production changes. MP-SB-n is a deliberate local mutation, must make the named oracle fail, then is reverted without committing the faulty product. A compile failure is not a runtime security kill. Apply the rule at each separately named sub-boundary, not only the outer method.

| Oracle | Positive and negative controls; literal expected result | Mutation plant → required failing assertion |
|---|---|---|
| SB-O1 / C1-S1-SB | Real HostSession + SessionPort + real C1 meter + fake loopback model: registered pin, complete native yes, original File run, one valid pair reply → exactly one charge/chat/Recorded attempt, usable reply; same path with two permitted calls uses the same run and paid index 2. No model/missing yes → zero chats. | MP-SB-1 blanket-deny session inference → `valid_session_reply_is_usable`; bypass registration/native consent → `unconsented_session_chats_eq_zero`. |
| SB-O2 registration | Valid pin metadata+durable record+final CAS installs once. Wrong endpoint/version/cloud status/digest/config, record failure, second registration, late install or bootstrap Stop/Quit → no visible model and no model-configured readiness. Hold each of version/status/tags/show and interleave restriction. | MP-SB-2 install before record, reuse ordinary closure-only cancellation, or reset failed session registration → respectively `unrecorded_model_is_absent`, `stopped_bootstrap_next_request_eq_zero`, `registration_attempts_eq_one`. |
| SB-O3 identity/binding | Identical live admission idempotent; vary S, Q, D, R, C request/hash/yes epoch, manifest/full identity, File admission, mission clock or native watch **one at a time** → refusal, no new binding/attempt/chat. Late affirmative/cache insert and 65th consent retain H1′ refusals. | MP-SB-3 omit each binding component or original-scope CAS → corresponding literal mismatch or `stop_before_bind_installs_nothing` assertion. Each component gets its own effective plant/control. |
| SB-O4 continuation and sealing | Ordinary original door succeeds. Approval continuation, adopted run, replayed objects, legacy conversion, registration from lane, conversation request even with N1 qualified, generic ModelTool/network effect → denied. Scanner finds a getter planted in a session submodule. | MP-SB-4 use continuation context/current ingress instead of Q; allow conversation/raw getter → `continuation_chats_eq_zero`, `conversation_chats_eq_zero`, structural guard failure. |
| SB-O5 attempts and retention | Attempts 1–8 on one run allowed across valid invocations within their C1 caps and mission budget; attempt 9 refused before metadata/meter. Cancel/finish/prune raced with attempt retains its owned record target; failure never refunds or shifts the record to another run. Session concurrency refuses a second active operation. | MP-SB-5 reset attempt count, remove pin, release slot early → `ninth_metadata_eq_zero`, `attempt_record_original_run`, `one_session_operation_active`. |
| SB-O6 each handoff | For registration and calls, valid no-interference request sequence succeeds. Ingress before each gate → that and later HTTP request counts zero. Ingress after gate → at most that one request; no successor. Held request → cancel closes peer, thread completes within the existing one-second transport test bound. | MP-SB-6 omit hook at any request, check before client construction only, or treat observer as veto → `pre_gate_ingress_request_count_eq_zero`. Retry plant → `attempt_chat_count_le_one`. |
| SB-O7 meter and budget | C1 reservation/charge goldens; exact-fit cap positive; cap 0/1/2/4, exhausted/stale/concurrent meter, failed/uncertain append → zero forbidden chat, no refund, valid durable prefixes. Metadata failure is an attempt but not a charge; successful charge then gate refusal stays charged. | MP-SB-7 chat before charge, bypass session charge admission, refund on cancellation → `durable_charge_precedes_chat`, `post_ingress_charge_absent`, `charged_total_retained`. Retain C1 MP-E15/16/21–24 without masking them. |
| SB-O8 deadlines/watch | One fixed deadline for both calls; delay/queue/suspend across either mission clock, original Q expiry and bootstrap registration expiry → refusal/cancel. Stop then native resume leaves old session caller denied, including after re-reading model identity. | MP-SB-8 `now + remaining`, fresh native watch or new request scope → `original_deadline_not_extended`, `old_session_after_resume_chats_eq_zero`. |
| SB-O9 outcome/debt | Success+Recorded accepted. Same success+Owed/Uncertain never reaches lane/private rest/File. Failed record remains visible in close; first cause preserved when Stop races budget/denial/lane/evidence failure in both orders. | MP-SB-9 ignore `record()`, clear slot/debt on meter settlement or replace first cause → `unrecorded_reply_unusable`, `record_debt_blocks_seal`, literal first-cause assertions. |
| SB-O10 reply races | Release reply before/after transport late check, before/after attempt recording, and before lane-result admission; restriction winning before final acceptance yields no accepted content/output hash/Advance/private draft/File request. Content-free attempt record still exists when conclusive. | MP-SB-10 skip final acceptance revalidation, or classify a charge as settlement → `late_content_never_projected`, `charge_after_ingress_absent`. |
| SB-O11 ownership/close | Idle/no-model existing close remains clean. Valid registration/call, completed children and durable records close cleanly through full A6. Hold each child/record/return, drop result/owner, fail spawn, panic, poison, exhaust slot → joined/conclusive or explicit debt; never false clean. | MP-SB-11 decrement active at cancel/charge settle, detach child, clear uncertainty in Drop, omit model slot from seal → `unjoined_or_owed_prevents_clean_close`. |
| SB-O12 bounded shutdown | Quit and emergency during Bootstrap metadata, call metadata, chat, meter, record and late result; repeated Stop/Quit do not move first deadline. Timeout outputs preserve per-store facts and exit nonzero within the existing three-second contract and its frozen test tolerance. Seal races are exercised with no-model and completed-call positive controls. | MP-SB-12 start a new close timeout, block join past budget, allow post-seal model record or erase durable end fact → `same_shutdown_deadline`, `bounded_exit`, `post_seal_model_events_eq_zero`, `durable_end_still_reported`. |
| SB-O13 lock progress | Meter reenters a safe door read, evidence sink is held while Stop ingress proceeds, registration metadata held while Quit proceeds, gate hold contended → bounded typed refusal. No broker/coordinator inversion; no dialog under bridge locks. | MP-SB-13 hold run/slot lock across meter/transport/evidence, or blocking lock at dispatch → designated bounded progress assertion. Independent outer watchdog must fail the test, not hang the suite. |
| SB-O14 death/restart | Kill subprocess at registration record/install, attempt/charge, each dispatch, reply/record/slot completion and close-end boundaries. Restart validates durable prefixes, reports known/unknown facts, sends zero metadata/chat, issues no model/run/consent/meter authority. Repeated restart stays report-only when unresolved. Clean historical Home positive control permits only separately fresh authorized startup. | MP-SB-14 recreate registration/caller from history, retry charged call, or treat missing attempt end as success → `restart_network_eq_zero`, `replay_has_no_authority`, `missing_outcome_reported_unknown`. |
| SB-O15 privacy/regression | Prompt/reply/error/metadata canaries absent from journal, Restricted evidence (including WAL/SHM), diagnostics, inventory and exported results. Valid reply available only in intended in-memory path. All unrelated R14/R31 exclusions and File close/first-cause/ack/death controls remain green; ordinary configured broker path still succeeds. | MP-SB-15 log prompt/reply or bypass exclusion → canary/route guard failure. Disable all inference → both session and ordinary positive controls fail. |

## 7. Adopted review-1 amendments (verbatim, design §9)

**MP-SB-6-default (SB-A3):**

> Add MP-SB-6-default: replace the session `admit_request` override with the ordinary default while retaining ordinary cancellation polling. For each registration metadata, call metadata and chat boundary, keep cancellation observations live/None and contend the owning hold immediately before final admission. The correct session path refuses with zero peer requests and zero dispatch notifications. The default-substitution plant must fail `session_override_required_at_each_request`; a source scan alone is insufficient. Separately pause after final predicate observation but before the original-snapshot CAS, interleave ingress, and require zero dispatch when ingress wins.

**SB-A1 schedule (invocation liveness at every handoff):**

> Before implementation, freeze how the accepted C1 invocation’s deny-only liveness state is privately bound to S/Q/D/R/C/M and observed at R3 and every R5 handoff. Return, overtake, transition, fence and close invalidate that state before releasing their transition boundary. A successful charge latch alone is insufficient. These checks are memory-only and nonblocking, with no coordinator callback under a broker hold. Add a schedule that invalidates the invocation after successful charge but before chat admission while the session/run remain otherwise live: zero chat, retained charge and record obligations.

**SB-A2 SB-O12 extension (worker-timeout close reporting):**

> Extend both existing close-report rendering and the bounded supervisor’s failure fallback with content-free model-obligation reporting. Use only nonblocking memory observation; unavailable ownership, child completion or recording state is explicitly Unknown, never zero or discharged. This rendering remains inside the original deadline and performs no evidence I/O. SB-O12 must hold a model worker past the producer deadline, prove the normal close reporter was skipped, and assert explicit model ownership/record uncertainty, nonzero exit and no affirmative close notice.

SB-A1 and SB-A2 are frozen here and exercised in SB-2/SB-3; SB-1 has no session
invocation, meter or close path to run them against.

## 8. Receipts

Appended at commit 3 (`feat(kernel)`); RED lines recorded at commit 2. Results
are actual local runs; no oracle text above was edited after them.

### 8.1 Commit 2 (`d919dc8c`, inert surface) — RED as expected

- `cargo test -p nexus-kernel --locked --lib inference`: 56 passed, 11 failed.
  GREEN: SB1-K1 and every retained inference test. RED at runtime (hook never
  invoked): K2 (`Registration: one admission per request`, log `[]`), K3, K4
  (`admission 0`), K5, K6, K7, K8a/K8b (`Registration request 0`, stages `[]`),
  K9 (`Ok(())` ≠ `Err(Cancelled)`), O2 (`same code`, `Ok(())`), O3 (`the hook
  refused the chat`, 0 ≠ 4).
- `--test m2e_transport_guards --test m2e_a2_guards --test m0_f5_emergency_stop`:
  1 / 54 passed; transport guards 38 passed, 1 failed, 1 ignored. SB1-S1 RED on
  production: `0 admit_request( calls, want exactly 1`, `no RequestKind
  mapping`; its planted red and green controls held.

### 8.2 Commit 3 (`feat(kernel)`) — GREEN

- `--lib inference`: 67 passed, 0 failed (12 SB1 + 55 retained, assertions
  unchanged). Guards: 1 / 54 / 39 passed, 0 failed, 1 ignored (the existing
  inventory print).
- K7/K8 (with all SB1 rows) 10× serial (`--test-threads=1`): 10/10 green, 12
  passed each. Whole inference module 10× parallel (`--test-threads=16`): 10/10
  green, 67 passed each.

### 8.3 Plants (each applied to the commit-3 tree, run, restored byte-for-byte by SHA-256; none committed)

Run: `cargo test -p nexus-kernel --locked --lib inference` (MP-SB1-6 also
`--test m2e_transport_guards sb1`). Every plant compiled and was killed at runtime.

| Plant | Required target | Result (failing tests) |
|---|---|---|
| MP-SB1-1 delete the call | K3, K4, K5, K7 | RED: all 11 non-K1 SB1 rows, incl. K3, K4, K5, K7 |
| MP-SB1-2 hook at `exchange()` entry | K6 | RED: K6, K7, O2 |
| MP-SB1-3 hook after the notice | K5 notices | RED: K5, K6, K7, K8a, K8b, O2, O3 |
| MP-SB1-4 `let _ =` | K3, K4, K5 | RED: 10 rows incl. K3, K4, K5 |
| MP-SB1-5 re-call after `Err` | K9 | RED: K9, K3, K4, K5, K6, K8b, O3 |
| MP-SB1-6a Chat→Metadata | K2; S1 | RED: K2, K5, K9; S1 |
| MP-SB1-6b `_` arm | S1 | RED: S1 (behavioural rows green, as expected: same mapping) |
| MP-SB1-7 hook on the first request only | K3 i≥1, K4 i≥1 | RED: all 11 non-K1 rows, incl. K3, K4 |
| MP-SB-6-default Gate override removed | K7 | RED: K7 and 9 other Gate rows |
| MP-SB1-8 Gate CAS → plain store | K8 ingress-wins | RED: K8a only |
| MP-SB1-9 default `Ok` always | K1 | RED: K1, O2 |
| MP-SB1-10 default `Err` always | K1; E-O5 positive | RED: K1, E-O5 and 53 others (54 of 67) |

No plant was masked; none required weakening another guard.


## 9. SB-2k nonblocking observations (frozen 2026-11-06)

Owner-approved scope: base `67c050b8`, `mother/sb2k-nonblocking-watch`.
Only `StopWatch::halted_now` / `check_now` and evidence `try_health` are added.
No caller, session route, inference, dependency or workflow change is authorized.
Commit 1 registers these oracles with inert surfaces; commit 2 implements them.
Oracle assertions and retained tests are frozen before implementation.

| Oracle | Test | Required observation |
|---|---|---|
| SB2K-K1 | `sb2k_check_now_agrees_with_check` | Fresh Ok; engaged Err; resumed old watches stay Err; three admission numbers |
| SB2K-K2 | `sb2k_check_now_never_waits_on_the_state_lock` | Completion channel within 2s while another thread holds state; last published fresh and stopped values |
| SB2K-K3 | `sb2k_engage_publishes_before_returning` | 1,000 post-engage joined observers see halted; zero failures |
| SB2K-K4 | `sb2k_try_health_refuses_under_contention` | Store and primary return None under hold, full health after release, poison returns and persists latched health |
| SB2K-K5 | retained emergency-stop and evidence-store tests | Existing assertions unchanged and green |

K1/K3 use the new integration binary. K2 is a `#[cfg(test)]` unit module in
`emergency_stop.rs`; K4 is a `#[cfg(test)]` module in `evidence_store.rs`.
Their private lock-holding helpers use readiness/release channels, a 10s outer
release watchdog and a 2s completion bound. No production seam is exposed.

| Plant | Mutation | Required kill |
|---|---|---|
| P1 | Omit engage mirror store | K1, K3 |
| P2 | Publish after releasing state | K3, or honestly not exercised |
| P3 | check_now takes state lock | K2 watchdog |
| P4 | try_health uses blocking lock | K4 watchdog |
| P5 | Contention returns default Some | K4 |
| P6 | Poison refuses or does not latch | K4 |

### 9.1 SB-2k receipts

Commit 1 compiled and ran with inert stubs (offline, fixed target directory):
- K1 RED: `sb2k_nonblocking_watch.rs:18`, `K1: after engage`, Ok vs Err.
- K2 RED: `emergency_stop.rs:1245`, Ok vs Err(EmergencyStopped).
- K3 RED: `sb2k_nonblocking_watch.rs:39`, unpublished stop at iteration 0.
- K4 RED: `evidence_store.rs:1588`, Some(default) vs None under contention.
- Retained: stop-seal 3/0/0; emergency-stop unit controls 11/0/1.
- Sandbox failures: retained F5 loopback bind denied (EPERM); seven retained
  evidence tests refuse runtime-root ancestry (UID 65534 on root directories).
- Full logs: exported alongside the report as `01-*.log`.
Commit 2: K1/K3 2/0/0; emergency-stop unit command 12/0/1 (K2 green);
evidence-store command 61/7/2 (K4 green); stop-seal 3/0/0. Counts are
passed/failed/ignored. The same retained sandbox failures above remain:
F5 0/1/0; evidence has seven runtime-root refusals. Overall gate is BLOCKED,
not green. No retained assertion or frozen oracle changed.
Formatting and warning-denied all-target Clippy both passed. K2 and K4 each
passed 10 serial runs and 10 concurrent runs (20 test processes), zero failures.

| Plant | Actual result |
|---|---|
| P1 | killed: K1 and K3 runtime RED |
| P2 | not exercised: K1/K3 stayed green; K3 does not observe the unlock-to-store interval |
| P3 | killed: K2 completion watchdog RED |
| P4 | killed: K4 completion watchdog RED |
| P5 | killed: K4 Some(default) vs None RED |
| P6 | killed: K4 poisoned observation refused RED |

All six compiled. All six source restorations were byte-identical (SHA-256).
Logs, command exits, counts and restoration hashes accompany `report.md`.
`Cargo.lock` is unchanged; scope remains five allowed files, with no Git writes.
A second explicit security self-review confirmed mirror publication at the sole
number mutation under state, fail-closed contention/poison handling, bounded
owned test cleanup, unchanged blocking methods and no caller/phase change.
Plants are temporary, run one at a time, restored byte-for-byte, never committed.
Exact-SHA CI, fresh-context review and Owner merge remain external gates.

## 10. SB-2a private registration (frozen before implementation)

Base 899e3872; route remains unavailable. Rulings O1–O6, Rulings 2 and Q1
bind this checkpoint. SB-2a owns registration only, not SB-2b attempts or
SB-3 composed inference. Exact cases and literal observations are in
`mother/host/tests/session/model-bridge-cases.tsv`; oracle bodies in private
`host_session/model_tests.rs`, `model_death_tests.rs`, host
`sb2_close_tests.rs`, and recursive `session_sealing.rs` are frozen here.
Retained behavioral C1-S1 and ordinary inference assertions stay unchanged.

C1-S1-SB2 changes only the lexical structural guard. Enumerated exceptions:
`host_session/model.rs`, `host_session/close.rs`,
`host_session/model_tests.rs`, `host_session/model_death_tests.rs` (all under
`crates/nexus-governed-control/src`). Recursive scans still reject public
model/inference methods and raw/checked-closure exposure. ModelObligations
and the CloseObligations.model field are content-free observation only.
The facade permits only the exact `mod model;`, `mod model_tests;` and
`mod model_death_tests;` lines. Full C1-S1-SB remains pending SB-3.

Owning holds: reservation and readiness share Core.producers then model;
metadata and installation use model.try_lock, current PrimaryEvidence.try_health,
original StopWatch.check_now and original Bootstrap CAS. No such hold crosses
transport, record I/O or joins. Seal uses producers then conclusive model
observation. Drop latches uncertainty/denial without a lock. Reporting uses
nonblocking observations, with unavailable facts Unknown. No observer is
configured during verification; peer prefixes are mandatory gate evidence.

Q1 exact dead-code expectations (entry outside tests; receipt fields in both
builds, each with the prescribed reason): HostSession::register_model_private; RegistrationReceipt.sequence,
RegistrationReceipt.digest, RegistrationReceipt.model, RegistrationReceipt.watch.
These retained fields are private; SB-3 must remove expectations when wired.
No allow, module-level or crate-level suppression is authorized.

Plants frozen: MP-SB-2-record/reset/install target R2; MP-SB-6-default targets
G1 independently at four real metadata boundaries; MP-SB-6-CAS targets G3;
MP-SB-6-once targets G2; MP-SB-6-retry-registration targets R2/G2;
MP-SB-5-slot targets A3/C1; MP-SB-11-drop targets D2; MP-SB-11-seal targets
C1; MP-SB-12-fallback targets C2; MP-SB-12-clock targets C3;
MP-SB-13-lock targets G1; MP-SB-14-replay targets X1;
MP-SB-15-content targets P1; MP-SB-4-exposure targets S2.
Attempt pin/count/record/meter plants, missing-attempt reporting and composed
end/worker controls belong to SB-2b/SB-3, not registration proof. G5 supplies
the peer-entered sink-wins schedule; R1 supplies no-interference. Registration
has no mission Q/R/C/L. O4 permits formatter snapshots only; real composed
worker-timeout evidence is explicitly PENDING for SB-3/M2e-D.

Commit 1 uses inert private stubs. Runtime denial is not a gate kill: a gate
oracle blocked before its target is reported not exercised. Tests must compile;
red lines and fixture hashes are appended as receipts without changing oracles.

### 10.1 Q2 oracle amendment and inert checkpoint

Binding Q2 a–f: host tests move to `mother/host/src/hands/sb2_close_tests.rs`
with normal cfg(test) module discovery. C2 is renamed (name only) to
`sb2_shutdown_fallback_reports_model_unknown_when_close_worker_is_skipped`;
its assertions are unchanged. No scan.rs amendment. The new sealing guard
recognizes the retained CommitmentId namespace and matches Authority as a
type token rather than a substring of settle_close_authority. Its positive
controls still reject raw Authority. C1 scans colon-containing public lines,
including typed parameters. Both corrected guards have an old-bug red plant.
The two Clippy range loops use iterators; assertions are unchanged.

Q2e strengthens C1 with the real seal_close and a conclusive-seal positive
control, R2 with direct installed-object absence, and X1 with real registration
before-record/after-record/after-install death boundaries. X1 owns a bounded
child watchdog, surfaces stderr, reopens the real HostSession twice, checks no
replayed operation/model, and independently checks the peer prefix unchanged.
These are oracle amendments before implementation, not post-result weakening.

Rewritten snapshot 01 COMPILES. `cargo test -p nexus-governed-control --locked
--lib host_session::model -- --nocapture`: 1/13/0. Meaningful inert runtime RED:
A3 model_tests.rs:45, D2 :65, T1 :548 (`operation reservation: Unavailable`).
The other ten fail sandbox setup, not their intended behavioral assertion.
Host `cargo test -p nexus-mother-host --locked --lib sb2_ -- --nocapture`:
0/2/0; fallback :13 and recorded-fact :30 are runtime RED. Logs:
`q2-01-red-control.log`, `q2-01-red-host.log`. No gate kill is inferred at 01.

Final review-1 fixture SHA-256 (supersedes the stale Q2 hashes; same bytes in both snapshots):

- `crates/nexus-governed-control/src/host_session/model_tests.rs`: `189b804626c5b9cf296091b605c93043af941d44d027d4ab4ff22da33a5a5beb`
- `crates/nexus-governed-control/src/host_session/model_death_tests.rs`: `d08a319f0080621c105bf9f020d93c8141cacfb286f53e1c576a800e656590f2`
- `mother/host/src/hands/sb2_close_tests.rs`: `738a254b1195aadb71b3f2e3e0b364057c2d25ab011939f4621c063e285736f9`
- `mother/host/tests/session/model-bridge-cases.tsv`: `05e50d6b70daf66cb6a9d157209cdfa583e5720fab3358f94e25c01d2c085652`

### 10.2 SB-2a actual implementation receipts — SANDBOX-BLOCKED

Base/HEAD remains 899e3872c5b61223c87b905f63c66947a816259b. Snapshots are
`01-oracles-inert`, `02-private-registration`, not commits. No Git write,
remote operation, public model route, host activation, production flag,
dependency/lockfile/workflow or persistent-format change was made.

Sheet §8 commands (passed/failed/ignored): governed-control lib 441/418/39;
Mother 235/61/0; host 24/14/0; kernel inference 13/54/0; combined kernel
guards 0/1/0 (stops at F5; later binaries not reached). Format, diff check,
and warning-denied all-target Clippy pass. Full core completed (226.07s),
without the previous outer timeout. The 408 retained core failure names match
the supplied baseline; ten new failures are RuntimeRoot ancestry or loopback
EPERM. Other retained failures are preserved with exact errors; not all are
proven sandbox-only. Full logs and classifications accompany report.md.

Real private focused suite: 4/10/0; host formatters 2/0/0; C1 structural 1/0/0;
recursive sealing 8/0/0. Three meaningful core inert reds and both host reds
now pass. No runtime registration, cancellation, death or seal success is
claimed from fixture refusals. O4's composed worker schedule remains pending.

Compiled and killed: MP-SB-5-slot, MP-SB-11-drop, MP-SB-12-fallback,
MP-SB-12-clock, Q2-c-sealing-old-bug, Q2-c-C1-old-bug. Compiled but NOT
EXERCISED: MP-SB-2-record, MP-SB-6-default, MP-SB-6-CAS, MP-SB-11-seal
(fixture refusal). All ten files restored byte-for-byte with verified SHA-256.
Other plants and ten-run gate schedules are not claimed as exercised here.
`plants-outside.sh` contains exact registration mutations (including default
substitution independently at four metadata boundaries), commands, restoration
hash checks, and ten serial plus ten parallel private-suite repetitions.
It records KILLED only for a compiled named-test runtime failure without a
sandbox refusal; SURVIVED is never a kill. No outside results are invented.

Separate AGENTS §18 self-review: original Bootstrap/same-word CAS and fixed
30-second deadline; retained original watch and actual record sequence/digest;
current nonblocking health/watch observation; no authority hold across evidence
or transport; atomic-only Drop denial; producer-ordered reservation/readiness
and seal; unknown observations remain debt. Ordinary inference is unchanged.
This self-review is not independent acceptance. Outside runtime proof,
fresh-context review, exact-SHA CI and Owner acceptance remain unverified;
network and Git writes are prohibited. SB-2b/SB-3 and phase advancement remain
unauthorized. Final verdict is BLOCKED, not a green registration claim.


### 10.3 Surviving outside plants — oracle-fix (2026-11-07)

The supplied `outside-run.log` and the five per-plant logs show compilation
and a **passing named test** for MP-SB-6-default-gate-0..3 and
MP-SB-6-retry-registration. Those are surviving plants, not kills or setup
refusals. Their old runner's generic "not exercised" suffix did not establish
a setup block.

Source diagnosis of default-gate-0..3: `RequestScope.next_step` counts scope
admissions, not metadata requests. `fixture()` registers a workspace first:
the governed entry, Files entry, project-select boundary and workspace install
each admit the Bootstrap scope. Reservation admits it again. Consequently the
four `model.gate.before` callbacks see next_step 5, 6, 7, 8 on this Linux path;
old plant predicates 1, 2, 3, 4 never substituted the default. This numeric
trace is source-derived, not measured inside the sandbox. The revised plans
count only metadata callbacks in a per-verification wrapper and use the real
`CallCancel` default at exactly the selected callback, retaining ordinary
cancellation polling and the before-boundary probe. All other callbacks call
the real session override. No product field or counter was added.

G1 now also proves successful no-interference reachability of all four real
gates, the exact peer prefix at contention, successful acquisition of the
owning lock by the holder, live deny-only cancellation inputs, arrival at the
selected boundary, and the exact number of gate entries before refusal. The
existing error and peer-prefix assertions remain. A holder timeout is a failed
schedule. No observer is configured by registration.

Source diagnosis of retry-registration: ingress makes the original scope
non-live. The planted second `verify` reaches the transport's early guard,
which refuses before the final admission callback and before HTTP. Thus peer
counts cannot distinguish one verification from this forbidden retry. The
retained pre-ingress oracle remains unchanged. Additive R2-live
`sb2_failed_verification_is_not_retried` supplies one malformed response
independently at each of the four metadata boundaries, with cancellation live.
It requires exactly i+1 gate calls and the exact i+1 peer prefix, failure,
absence of installation/readiness, and denial of a later caller retry. A
second verify can now reach the live peer and is observable. The retry plan's
mutation is unchanged; its named oracle is R2-live.

No production defect was found in this diagnosis; no production repair was
made. The revised final suite compiled locally: 4 passed, 11 failed, 0 ignored
(`oracle-fix-model-final.log`). Ten failures are RuntimeRoot writable-ancestor
refusals; the death fixture is blocked by socket PermissionDenied. All five
revised plants compiled locally but stopped at RuntimeRoot before their target
assertions, and restoration hashes matched. These results prove neither RED
nor GREEN for registration. The Architect must rerun `plants-outside.sh` on
the final tree with an accepted RuntimeRoot and loopback-test permission. All
five plants require that fixture. Outside mutation kills and final positive
controls remain unconfirmed. The private route and phase boundaries are
unchanged.

### 10.4 Review-1 oracle amendment (before repairs)
F1: `sb2_facade_rejects_multiline_registration` rejects the reviewer's literal
multiline wrapper; exact three module declarations and a plain status method pass.
F2: `sb2_failed_startup_reports_model_unknown` requires MODEL_UNKNOWN in startup output.
F3: `sb2_install_completion_is_atomic_to_close_observers` pauses after installation,
observes quiescent/close_obligations concurrently, requires the install hold still
owned, then requires successful completion and recorded, non-uncertain quiescence.
F4: `sb2_close_report_has_one_model_line` covers zero/two resource snapshot failures
with unknown/none model snapshots: exactly one model line in every report.
F5: final candidate fixture hashes supersede stale hashes; all plants must rerun
outside. The schedule's RuntimeRoot refusal is neither a behavioral red nor green.

### 10.5 Review-1 final candidate (Q5; uncommitted bytes)
F1/F2/F4 had intended assertion RED then GREEN (`review1-f1-{red,green}.log`,
`review1-host-{red,green}.log`). F3's original-gap schedule compiled but was
RuntimeRoot-blocked before its target (`review1-f3-red.log`); no runtime kill
or green is inferred. Installation now completes the owner under its state hold.
No evidence/transport I/O was moved into that hold; Drop remains latch/fail-only.
The outside runner plants the literal F1 wrapper in a cfg-disabled impl so the
source oracle, not a compiler error, must reject it. F2 removes startup unknown;
F3 restores the install/completion gap; F4 restores the duplicate model line.
The runner pins the entire final candidate, requires positive controls first,
then runs ALL plants and 10 serial/10 parallel G1/G3/CAS repeats. Final-tree
outside results remain PENDING; earlier kills are historical only. Fixture
hashes above identify final candidate bytes, not a new commit. Git writes and
network remain prohibited; route UNAVAILABLE. Exact results, RuntimeRoot-blocked
tests, snapshot reproduction and measured Q5 size are in `sb2a-out/report.md`.

Final local targeted results: broker 4/12/0 (11 RuntimeRoot refusals, one
loopback EPERM); host formatters 4/0/0; facade control 1/0/0; sealing 8/0/0.
C1 source guard passes; two retained C1 behavioral fixtures refuse with
Backend("unavailable"). Final fmt and warning-denied all-target Clippy pass.
Local F1/F2/F4 plants and the adapted fallback plant are killed and restored;
F3 mutation compiles but stops at RuntimeRoot. Stale fixture metadata refuses;
final metadata verifies. The runner has 26 unique in-scope plant plans; its
outside all-plant run and repetitions remain unobserved on these bytes.

## 11. SB-2b frozen accounting oracles (2026-11-07)
Authority: mission sheet ending ARCHITECT RULING SB-2b FREEZE; base fd7fa61d.
Snapshot 01 contains default-denying `Attempt` accounting and these additive
oracles; snapshot 02 implements it. No retained SB-2a expectation is changed.
Literal rows: `mother/host/tests/session/model-bridge-cases.tsv` SB-2b entries.
A1: `sb2_attempt_count_and_retention_are_one_admission`: attempts 1..8 acquire
one exact pin, ninth OverBudget; refused original CAS changes neither counter.
A2: `sb2_attempt_record_keeps_original_run_and_consent`: exact run/agent/door/
contract/consent string/model/ordinal survives cancellation, finish and capacity;
one restricted end record, model pin released only by recorded-owner return.
D1: `sb2_record_debt_blocks_seal_and_reuse`: Recorded/Owed/Uncertain remain
separate, including after cancel/meter settlement; recording is single-use;
child completion and method-owner completion remain separate; no early reuse.
D2: `sb2_attempt_drop_panic_and_poison_retain_ownership`: Drop under owning lock,
record panic and poison retain pin, latch uncertainty/denial, never record/join.
L1: `sb2_attempt_recording_does_not_block_ingress`: evidence callback verifies
no producer/model/run hold, accepts emergency, then original owner settles
without refreshed Work authority. A3: `sb2_attempt_binding_refusals_issue_no_ticket`
checks wrong Core/run/agent/door/identity, then a reachable positive acquisition.
X2: `sb2_missing_outcome_is_unknown_not_success`: missing=unknown, owed=owed,
uncertain=uncertain, verified Recorded=recorded; no inferred attempt count.
X1: `sb2_attempt_death_prefixes_are_report_only`: abort after ticket, before
real end record, after record and after slot settlement; two qualified reopens
retain durable prefixes only, recreate no model/run/admission/attempt authority.
C4: `sb2_attempt_recorded_end_survives_failed_delivery`: real seal and end writes;
failed delivery preserves separately established Mother and primary end facts.
C2: `sb2_attempt_fallback_keeps_all_debt_statuses`: O4 injected snapshots only;
all record statuses survive formatting. Complete worker composition stays
PENDING SB-3/M2e-D. Accounting proves no C1 charge, dispatch or reply acceptance.
Owning holds: reserve attempt under producers then slot; release both before
run-table admission; final original-cell CAS immediately precedes count+pin;
no authority hold crosses result recording; exact ticket consumed on settlement;
seal observes existing model quiescence, unavailable/poisoned snapshots refuse.
Plants: MP-SB-5-count (increment), MP-SB-5-pin (exact model pin, never File
preservation), MP-SB-9-record (status), MP-SB-9-meter (early slot discharge),
MP-SB-12-end (primary end fact), MP-SB-14-missing (missing outcome classification).
All 26 SB-2a plans rerun on final bytes; outside runner checks reachability,
compile/runtime assertion failure, restoration hashes, 10 serial/10 parallel
contention/CAS repeats. No setup refusal or compile error counts as a kill.
History classification takes only established end facts from existing verified
stores in death tests. No new persistent format, historical reader API, precise
missing count or cross-store association is introduced; unavailable facts stay
unknown in the existing fallback. No recovery action or replay is implemented.

11.1 Inert checkpoint receipts: `oracle-red-accounting.log` compiled and ran
five accounting tests, all runtime RED at the expected acquisition refusal;
`oracle-red-debt.log` ran D1 RED at acquisition; `oracle-red-missing.log` ran
X2 RED at the owed-status assertion. The missing=None denial control is already
green; no acquisition RED is credited as reaching later pin/record assertions.
Host formatter controls are default-preserving positive controls, not runtime RED.
X1/C4 require RuntimeRoot and are sandbox_blocked without execution here.
Named temporary dead-code expectations (non-test builds only): `Attempt`,
`Attempt::acquire`, `Attempt::record_return`, `Attempt::complete`, and
`historical_outcome`; remove at SB-3 wiring. No module-wide suppression.

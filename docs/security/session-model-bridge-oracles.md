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


### 11.2 SB-2b-A2 amendment and SB-2a review-2 carry (2026-11-07)
The approved base is now eb45f54b, including SB-2a-E65. Snapshot numbering
supersedes §11's original two-folder plan: 01 oracle-add (inert), 02 oracle-fix
(A2 fixture only, defect found before implementation), 03 private accounting,
04 review-2 carry. No recorded historical receipt above is a current-tree claim.
A2's shared fixture now actually issues a door through
`issue_agent_door_with_halt`, retaining that door and the original opaque id
from its broker-issued record. Consent is `msn-000001/1/1`, valid in the existing
Restricted registry. The assertion still compares original correlation verbatim.
No numeric group, substituted digest or new persistent grammar is accepted.
This corrected fixture requires RuntimeRoot; accounting runtime GREEN must be
established outside the sandbox, not inferred from compilation.
L1 adds qualified public-function signature collection (const/async/unsafe/
extern ABI, without a new dependency), facade signature model denial and
`model::` facade-line denial. Both the modifier and multiline alias controls
were observed RED before the matcher changes; alias plants target the real
facade through both structural guards.
L2 `sb2_owner_completion_retries_benign_observer_contention` forces an observed
failed try-lock while a close observer holds state, then releases it before
completion: RED before repair, GREEN afterward. Completion metadata retries for
at most 100ms, bounded also by the original close deadline. Admission/handoff
stays nonblocking. `sb2_owner_completion_is_bounded_and_poison_fails_closed`
requires timeout/poison to retain denial and uncertainty. Drop still only fails.
I1 uses one close-obligation snapshot. I2
`sb2_bootstrap_model_contention_is_unavailable` requires Unavailable on contention
rather than Restricted (RuntimeRoot-blocked locally). I3 strengthens F4 by
calling real `Runtime::close_report`; cfg(test) thread-local error snapshots
exercise zero/one/two resource failures and present/unavailable obligations.
Its six one-model-line assertions remain; RuntimeRoot blocks local execution.
I4 corrects the append comment: recording is outside the model hold, not globally
lock-free. Only `RegistrationReceipt::sequence` retains its SB-2a dead-code
expectation. SB-2b's named expectations are `Attempt::acquire`,
`Attempt::record_return`, `Attempt::complete`, and `historical_outcome`.
Final receipts, exact commands, hashes, all blocked schedules, snapshot
composition and measured size are in `sb2b2-out/report.md`. The hash-pinned
outside runner includes all 26 SB-2a plants, six FREEZE plants, five review-2
controls, corrected inert RED replay, and ten serial/ten parallel repetitions.
Outside results and exact-SHA CI remain unobserved; no composed model-worker,
session inference, SB-3 capability, phase advancement or acceptance is claimed.
C1 L1's first matcher also rejected the two pre-existing private Core storage/
initialization lines. One bounded repair preserves those exact two declarations;
public alias signatures remain denied. No frozen assertion was changed.

## 12. SB-3 review unit A frozen oracles (2026-11-07)
Authority: `sb3-mission-draft.md` v2 and its ARCHITECT FREEZE; base d51d61cb
(final reviewed SB-2b). Unit A owns only the §5 column-A allowlist: deny-only
invocation owner/observer types, the retained original File grant, the private
door kind, the private R2 binding, Mother observer plumbing and L1. The route
stays closed: no public `HostSession`/`SessionDoor` model method, SessionPort
override, ticket, charge, transport, host wiring, kernel or Cargo change.
No R32 supplement verifier lands here. Literal rows: `model-bridge-cases.tsv`
SB-3A entries; oracle bodies: private `host_session/model_call_tests.rs` and
Mother `c1b_coordinator_oracles.rs` (L1). Commit 1 is inert: owner revocation
is a no-op, the sealed caller forwards a fresh observer, `try_live_at` and the
R2 binding deny. Runtime RED is recorded per test; compile failure or an
earlier refusal is never counted as reaching a later assertion.

O1 surface: `inference::InvocationOwner` (not Clone/Default/serde; `new`,
`revoke` SeqCst idempotent, `observer`, `is_revoked`; Drop revokes) and
`InvocationObserver` (Clone; Acquire `is_revoked`). Neither confers authority.
C1 `InvocationLiveness` wraps the observer; `LaneCalls` owns the owner; every
`invalidate_calls` revokes explicitly before journal I/O under the coordinator
lock. `EffectPort::model_call` requires `&InvocationObserver`; the ordinary
GovernedPort/legacy arm ignores it. The `model_observer` port field and the
journal probe are `#[cfg(test)]` observation seams only.

O2 surface: private `DoorKind::{Original{file_grant}, Continuation}`, set by
the constructor. Both original issuance paths thread the GrantId returned by
`grant_files`; the approval continuation and the test-only `activate_for_test`
(its grant is discarded by its caller) are Continuation. `GrantStore::try_live_at`
uses `try_lock` only: Contended refuses without latching; Poisoned refuses and
fails the session; missing/revoked/expired is `Ok(None)`; clocks are read by
the caller before any hold. Lock order is run -> grant, try_lock only.

R2 (`SessionDoor::bind_inference_private`, `RunRegistry::admit_session_inference`):
Continuation and original-Q liveness refuse first (`halted`); the installed
registration is read under a nonblocking slot hold released before the run
table (`model_not_registered`; uncertainty/old watch `halted`); full M is
compared (`model_identity_mismatch`). Under one run-table try_lock: Stop/close;
live door; run known; agent/class/halt/finish/cancel; then one predicate each:
S (run cell is the original session cell), D (run member of door and door
lists run), R (run opened by this door), files admitted (`not_admitted`), Q
(Q's own File binding names this run); every association miss is `not_member`.
Then retained grant live with Files scope equal to the run's workspace id and
identity (`grant_not_live`); cached consent for hash/label with the live H1'
lifetime (`no_consent`); sticky hash/label/identity (`already_admitted`);
for a fresh binding the consent's hash/label/sequence/kind/time/page facts
equal the File association (`no_consent`); probe `model.bind.checked`; Q's
real same-word CAS, also on idempotent success; then the single insertion.
No filesystem, evidence or callback work occurs under the hold.

Oracles (RED expectations in commit 1):
- `sb3_invocation_owner_is_deny_only`: revoke/observer/Drop semantics and a
  closed source surface. RED: revocation not observed.
- `sb3_inference_binding_is_exact_and_idempotent`: valid binding, idempotent
  repeat, changed sticky hash/label, then fifteen isolated F12 fixtures
  (S foreign cell; Q second live request; D membership; R opener; unknown R;
  missing File admission; missing consent; sequence; kind; time; lifetime;
  wrong M; expired Q; Continuation; no registration), each with its literal
  class, no binding and zero attempts/pins. RED: valid binding refused.
- `sb3_original_file_grant_remains_required` (G0, R2 boundary only): revoked,
  expired, scope-mismatched and substituted-live-grant cases refuse
  `grant_not_live`; the original live grant binds. RED: first refusal class.
- `sb3_stop_before_binding_installs_nothing` (R3): pause at the actual CAS,
  ingress wins, nothing installed; repeat after an installed identical binding
  also refuses unchanged. RED: paused CAS boundary not reached.
- `sb3_binding_contention_refuses_without_latching`: run/grant contention
  `unavailable` without Failed; run/grant poison refuses and latches Failed.
  RED: run-table contention class (`halted` from the inert binding).
- Mother `sb3_invocation_observer_is_monotonic_and_original` (L1): return,
  overtake, transition (Owner Stop) and fence (failed charge append) each
  observed from a second thread inside the owning boundary; the forwarded
  observer is revoked there and never un-revoked. Settlement, hold and close
  cannot hold a live caller (close refuses `model_obligations` Busy; holds and
  settlement occur with no pending model lane), so they are not claimed as
  discriminating L1 schedules. RED: the forwarded observer is live.
Existing C1 goldens are retained unchanged except the required observer argument.

Plants (A): MP-SB-3-S/-D/-R/-Q (each association predicate), MP-SB-3-C-hash/
-request (sticky), -seq/-kind/-time (File association), -lifetime (consent
life), MP-SB-3-M, MP-SB-3-CAS (fresh and idempotent), MP-SB-3-grant (ignore or
substitute), MP-SB-4-Q, MP-SB-A1 (fresh never-revoked observer), MP-SB-A1-order
(revoke after journal I/O), O2 contention latch. Named dead-code expectations
(non-test builds; wired by SB-3C): `SessionDoor::bind_inference_private`
only. Receipts follow in §12.1.

### 12.1 SB-3A receipts (outside the Codex sandbox; RuntimeRoot/AF_UNIX real)
Commit 1 (`88b55952`, inert): governed-control filter `model_call_tests`
0/5 passed, each RED at its first intended assertion (O1 revoke; R2 valid
binding `halted`; G0 `halted` != `grant_not_live`; R3 CAS not reached; O2
run contention `halted`); Mother L1 RED in all four children with the
forwarded observer live (`[false]`). Commit 2: the same tests GREEN (5/5;
L1 4/4 children); retained C1 goldens, including `m2e_budget` invalidation
boundaries, pass unchanged. Plant receipts are reported with the candidate;
none is committed.

### 12.2 SB3A-R1 oracle-fix (found after implementation)
Plant MP-SB-O2-run-latch turned the contention test RED one assertion early
(the latched failure made the next grant-contention call `halted`). The
approved, strictly stronger line `assert!(!failed(&f), "contention latches
nothing (run)")` now follows the run-table contention assertion. L1 adds
`l1/excluded`, asserting the precondition for not claiming settlement, hold
and close: with a model lane pending and its caller live, the verified
journal grammar refuses a forged `Held` (`HoldOutOfPlace`) and a forged
`SettlementStarted` (`SettlementOutOfPlace`), and close is `Busy` (no
`SessionClosed`) even after Stop until the caller returns. The settlement
refusal reflects that no File is unresolved in this pending-lane state; it
is not a general no-pending-lane grammar rule. F12 test-only seams are
accepted for A; carry to C: R4/R2 composition oracles vary S, Q and
Continuation with real second doors and the real continuation constructor.

### 12.3 SB3A-R2 (Codex review-1, two LOW): checked equivalence table
Fixes: R2 now also requires the run's cell to be the door's retained cell
(as `live_member`), and the File-consent association is checked on every
success, idempotent rebind included. Oracles: R2 isolated `door_cell`;
`sb3_idempotent_rebind_revalidates_file_consent`. Each helper check is
mapped to its inline R2 counterpart in
`crates/nexus-governed-control/src/authority/run.rs` (line numbers at the
fix commit; updated for SB3A-R3, Codex review-2, which also requires the
stored entry's `view.id` to equal the requested run). No check is unmapped.

| Helper check (run.rs line) | R2 inline counterpart (run.rs line) |
|---|---|
| live_member agent (1702) WrongAgent | 2082 WrongAgent |
| live_member class Agent (1705) WrongRun | 2085 WrongRun |
| live_member groups contains group (1708) | D, 2103 |
| live_member run cell == door retained cell (1711) | cell, 2106-2108, refused 2111 |
| live_member cell engaged / halted group (1720) Halted | 2087 Halted |
| live_member finished (1723) RunNotActive | 2093 |
| live_member cancelled (1726) RunCancelled | 2096 |
| host_prerequisites host-cell applicability (305) | S requires host, 2099-2101 (always applies) |
| host_prerequisites opened_by door (308-309) | R 2104 + live_door(group) 2070 |
| host_prerequisites door active/alive/!halted (310-312) | live_door 1686-1689 via 2070 |
| host_prerequisites door agent == run agent (313) | live_door agent 1689 + 2082 |
| host_prerequisites door members has run, by entry.view.id (314) | D listed 2071/2103 + stored id == run 2110 (SB3A-R3) |
| host_prerequisites run groups has group (315) | D, 2103 |
| host_prerequisites consent matches File hash/request/seq/page facts (321-324) | selected consent 2073-2079 + association 2150-2155 (adds kind/time), every success |
| host_prerequisites consent life epoch/!halted/!closed (325-327) | `c.life == now` 2074 |
| host_prerequisites only if Files admitted (319) | stricter: NotAdmitted 2117 |
| admissible Stop / closing (shared) | 2059 / 2062 |

## 13. SB-3 unit B re-split (SB3B-R1/R2/R3, 2026-11-07)
B is split by implementation seam (SB3B-R3 option c). **B1a** (this branch,
`mother/sb3b-private-call`): R3 attempt admission. **B1b**
(`mother/sb3b1b-call-gate`, stacked): the R5 call gate and its oracles,
with test-only **B1e** only if B1b exceeds 1,500. **B2**: close/death.
Commit `a15b5ee0` froze B1 oracles on stubs; its call-gate oracles and
stubs move out of this tree (amend commit) and are carried to B1b
unchanged, where the SB3B-R2 gap oracles are added. 9c81d0d5: SB-2b
assertions unchanged except the disclosed fixture-helper substitution in
`sb2_attempt_binding_refusals_issue_no_ticket` (whitespace-normalised
full macros 145 = 145; not literal byte identity of every assertion).

### 13.1 B1a mapping and frozen oracles (SB3B-R3)
Production (feat, after the spec re-check): `admit_session_attempt` gains
the retained-grant predicate (`OriginalGrant::live`, `try_live_at`, Files
scope = run workspace), the invocation observer last before Q's CAS, and
run-table poison failing the session; `SessionDoor::admit_attempt_private`
(pub(super), dead-code expect; wired by B1b) refuses a Continuation
(`halted`), a conversation (`model_config_mismatch`), a dead Q or revoked
invocation (`halted`), no registration (`model_not_registered`) and a run
other than Q's File run (`not_member`) before acquisition. No transport,
meter, gate, charge latch or backstop lands here (B1b).
Oracles (`model_call_tests.rs`, RED on the deny-only stub = first assertion,
`halted`): G0-R3/F2-R3/F8 `sb3_attempt_admission_requires_the_original_grant`
(revoked, expired, scope, substituted live grant: `grant_not_live`; grant
contention `unavailable` without latch; grant and run-table poison latch
failure; no attempt, pin, record or active slot); L2-R3/F1-R3
`sb3_attempt_admission_observes_invocation_last` (before acquisition, at
the reserved boundary, dual fault grant-before-observer, ingress wins the
CAS, revocation after the read is the entered interval); R4/F12
`sb3_private_attempt_refuses_before_ticket`; A1
`sb3_attempt_ceiling_is_eight_on_the_original_run`; A3/F7
`sb3_attempt_record_and_pin_are_exact` (exact ordinal, one correlated
record across cancel/finish/capacity; substituted ordinal never settles,
debt latched); F11 `sb3_b1a_test_seams_are_test_only` (comment- and
string-masked scan: every `probe`, `tamper_for_test`, `model_pins_for_test`
and the `model_call_tests` module is under `#[cfg(test)]`; GREEN on the stub
because the seams are already gated; leak plants are its RED evidence).
F8 at R3: no meter is reachable before a ticket in B1a; zero charges are
re-asserted where the meter first exists (B1b). Seams (B1a): probe
`model.attempt.reserved`, `tamper_for_test("pin")`, `model_pins_for_test`,
SB-3A `door_with`. Carried: F5 composed zeros and F7 Mother withholding to
C/D (SB3B-R3 1, 2). Figures (additions + deletions vs c3bffa92): this
oracle tree 687; forecast feat +86 (run.rs 41, model.rs 45); B1a about 783
with receipts, under the 1,500 cap. B1b is forecast separately on its branch.

### 13.2 SB3B-R4 B1a gap oracles (re-check of e137e136, 5 MED)
(1) F11 rewritten: nested block comments masked; a statement start is
found at its own nesting, so an enclosing parenthesis never borrows a
sibling statement's attribute; regression controls are the two crafted
leaks (sibling attribute, nested comment) and a gated control; both crafted
cases pass the e137e136 scanner (RED shown on it). New seams
`model.attempt.granted`, `tamper_for_test("identity")`,
`model_ledger_for_test` are scanned. (2) `sb3_attempt_observer_is_read_after_the_grant`:
a revocation at `model.attempt.granted` (after the grant, before the read)
refuses; an early cached read admits. (3)
`sb3_attempt_contention_and_poison_latch_exactly`: R3 grant and run-table
contention `unavailable` with uncertain and Failed both false; grant and
run-table poison latch Failed; each with no attempt, pin, record, active
slot or binding change. (4) `sb3_attempt_grant_clocks_and_identity_are_independent`:
monotonic-only and wall-only expiry (SplitClock) and same-id different
workspace identity each `grant_not_live`. (5) every pre-ticket refusal
(`nothing`) compares the retained binding before/after (Debug bytes and
equality; poison-tolerant ledger).

### 13.3 SB-3B1a receipts (local, final head 9d8d73ba + this record)
RED: e137e136 stub 38 passed / 5 failed, f572d111 stub 38 / 8, each new
oracle at its first assertion; F11 controls fail on the e137e136 scanner.
GREEN: 46/46 `host_session::model`. Plants (15, each killed at the named
assertion, files restored by sha256): MP-SB-3-grant-ignore "R3 revoked";
-grant-substitute "R3 substitute"; MP-SB-4-conversation "conversation";
MP-SB-5-count "attempt 1 never refunded"; -pin "the exact ordinal"; -slot
"slot debt latched"; MP-SB-A1-R3-ignore "R3 observes the invocation";
early cached observer read "read after the grant"; contention setting
uncertainty "grants: contention latches nothing"; wall clock dropped "R3
wall"; monotonic clock dropped "R3 monotonic"; identity comparison removed
"R3 identity"; binding cleared on refusal "revoked: binding"; F11 sibling
leak and nested-comment leak "model.rs: a seam reachable outside
cfg(test)". Gates, each binary alone: governed-control lib 891/891; Mother
lib 316/316; host --no-fail-fast 166/166 (on c0b82242 and on a rerun at
9d8d73ba; one run at 9d8d73ba failed `hands_tests::review2_receipts_while_recovery_reporting_is_held`
at a PTY transcript wait, not reproduced in 10 isolated runs or the full
rerun; no host or Mother code changed; recorded, not classified);
m2e_a2_guards 54/54; m2e_transport_guards 39/39; fmt; clippy (3 crates,
all targets, -D warnings, after 9d8d73ba removed the now-fulfilled
`Attempt::acquire` dead-code expectation); diff --check. Repeats: 10 serial
and 10 parallel runs of `host_session::model`, 46/46 each. Size: B1a vs
c3bffa92 recorded in the report.

### 13.4 SB3B-R5 (B1a review-1) corrections and carry
Claims corrected (ruling 2): lock contention at the admission predicates
(R3 grant store and run table) never latches; a pre-ticket refusal leaves
the slot untouched UNLESS its rollback completion hold expires, which
latches uncertainty and session failure per the retained SB-2b contract
(oracle `sb3_attempt_rollback_hold_expiry_latches_uncertainty`). §13.1's
"no active slot" holds only for refusals whose rollback completes.
Carry (ruling 3, LOW, inherited): R3 still reaches clock callbacks under
the run-table hold (`cell.admit()` -> `RequestScope::eligible()` mission
clocks; earlier `is_engaged()` checks). B1b's call gate must capture every
clock value before taking a hold, and its oracles must include a
blocking-clock control.
Ruling 1 (grant TOCTOU) and ruling 4 (escaped char literals in the F11
masker): oracles in 206cf993; the fix is pending an Architect question
(the grant-store guard lives in policy.rs, outside the B allowlist).

### 13.5 SB3B-R6: oracle-fix, grant hold and F11 masker
Oracle-fix (found after implementation, approved in SB3B-R6 (2); commit
3dff6ce7): the SB3B-R5 (1) oracle revoked on the admitting thread inside
the `model.attempt.granted` probe, so under the ruled fix it waited for
the grant-store mutex its own thread held. Evidence: the test was still
running past 60 s and the run ended with timeout exit 124. Replacement
`sb3_attempt_grant_revocation_cannot_precede_the_cas` starts the revoker
on another thread at that probe and waits for it at most 2 s. If the
revocation completed before the CAS, admission must have been refused.
The revocation must land either way, and a refusal leaves nothing behind.
The 2 s wait only bounds the test. A slow revoker can only let a mutant
survive; it can never fail correct code, because under the hold the
revoker cannot complete before the CAS whatever its speed.
Fix (8eed8f2d; policy.rs allowlist per SB3B-R6 (1), guard return only):
- `GrantStore::try_live_at` returns the held store guard.
- R3 keeps it (run -> grant, try_lock only) through the observer read,
  Q's CAS and the count/pin mutation. A revocation lands before the
  observation (refused) or after the CAS (the entered interval).
- R2 keeps the same guard through its CAS.
- The F11 masker blanks escaped and byte char literals
  (`'\''`, `'\"'`, `'\\'`, `b'..'`).
Carry unchanged: the §13.4 clock-callback LOW goes to B1b.
Receipts (local, final code head 8eed8f2d + this record). RED: 3dff6ce7
failed at "a grant revoked before the CAS admitted", and the 206cf993
escaped-literal controls failed at "escaped literal '\"'". GREEN: 48/48
`host_session::model`. Plants, all on the final head, each restored by
sha256 and each killed at its named assertion:
- the 15 plants of §13.3;
- guard released before the CAS (`let _ = grant.live(..)?`), "a grant
  revoked before the CAS admitted", 10/10 runs;
- F11 escaped-quote leak (`'\"'` before an ungated probe), "model.rs: a
  seam reachable outside cfg(test)";
- rollback hold failure that marks the owner done without latching,
  "uncertainty and failure latched". A first mutant returned early with
  `?`; `Owner`'s Drop latched anyway, so that mutant was ill-formed and
  was replaced.
Gates, each binary alone:
- governed-control lib 893/893
- Mother lib 316/316
- host --no-fail-fast 166/166
- m2e_a2_guards 54/54
- m2e_transport_guards 39/39
- fmt; clippy (3 crates, all targets, -D warnings); diff --check
Repeats: 10 serial and 10 parallel runs of `host_session::model`, 48/48
each.

### 13.6 B1b mapping and frozen oracles (SB3B-R3, on B1a final 47fa1f88)
The oracles are in `host_session/model_call_tests.rs`, on deny-only stubs:
`SessionDoor::model_call_private` refuses `halted`, and the ModelSlot
seams are declared but unused. They were written alongside a saved draft
and are disclosed as such (SB3B-R1 ruling 6).

The mapping is the `SB3-*b` rows of the cases TSV: G0 (R5), G1-G5, L2,
C2 (with metadata failure and refused charge), T1, F1, F2 with F8 (R5
classes, no new charge), F3, F4, F5 (composed zeros carried to C/D), F7
(Mother-side withholding carried to D), F8 (pre-ticket refusals through
the call: zero charges, meter entries and records), F11, the SB3B-R5 (3)
carry (`sb3_call_gate_reads_clocks_before_any_hold`), and SB3B-R5 (1)
applied at R5, which is builder-proposed
(`sb3_call_grant_revocation_cannot_precede_the_cas`).

Notes on the last two:
- The SB3B-R5 (3) oracle uses a blocking clock. While a clock reading
  inside a call gate is blocked, it asserts that neither the run table
  nor the grant store is held, and that the call still completes.
- The builder-proposed oracle applies SB3B-R5 (1) at R5: a revocation
  started after a gate's observer read cannot complete before that gate's
  CAS. Its bounded wait is 2 s, and a slow revoker can only let a mutant
  survive, never fail correct code.

B1b seams, all `#[cfg(test)]` and guarded by F11 in this unit:
- ModelSlot `trace`: the ordered per-predicate refusal and poll trace.
- `backstop_skew` / `SkewClock`: the clock-controlled backstop, which
  replaces `backstop_spent`.
- `chat_replay` / `chat_early`: the spent and uncharged chat latch.
- SplitClock `on_read`: the blocking clock, test fixture only.

Moved to the test-only B1e (pure evidence, SB3B-R3):
- A2/F6, F9 and F10 (manifest and watchdog).
- The call-level R4 row and G0's R3 case; B1a already covers both.
- SB-2a registration-gate contention.

Forecast against 47fa1f88:
- This oracle-add: 1,202 changed lines (code 1,139, this record and
  the TSV 63).
- Feat, estimated from the saved draft: about 260 lines. That assumes
  `model_call_private` reuses `admit_attempt_private` rather than
  rewriting it. Parts: model.rs about 175, run.rs about 45, inference.rs
  27, host_session.rs about 12 for admission with captured clocks.
- Receipts: about 10 lines.
- Total: about 1,470 of 1,500. If the feat runs over, the next
  pure-evidence candidates for B1e are F5 and the call-level F8 rows;
  moving them needs a ruling.

### 13.7 SB3B-R8 (B1b spec check, 10 MED) oracle-add
Finding 1 was fixed in ec34e80b. Findings 2–10 and the dead/halted-Q row
are new oracles on the stubs:
- F2: added `try_health` None (store held), an independent
  host-prerequisite failure (`view_id`), a removed pin (`unpin`) and a
  dead/halted Q (`emergency`, model_cancelled). The four G0 per-gate
  grant cases are now literal `model_cancelled`.
- `sb3_r5_holds_contend_and_poison_exactly`: the run table and the grant
  store, each contended and each poisoned, at every gate. Contention
  latches nothing; poison latches both.
- F3: a poll taken under contention returns None before the refusal.
  `observer: None` is checked within `session_transport`. G3 counts gates,
  admissions and notices, and G4 counts gates.
- F4: time is advanced before attempt two, so only the original deadline
  is crossed. The identity re-read goes through the slot
  (`registered_identity_for_test`), not the cached value.
- L2 absorbs F5: at most the selected request, a Recorded outcome, and
  inventory, commitments, pending target and non-model evidence all
  unchanged. The vacuous acceptability check is dropped.
- Chat latch: literal `probe:model_cancelled`, the exact peer sequence,
  and exactly one charge.
- Blocking CallDeadline: added to the blocking-clock control.
- F11: scans model.rs, host_session.rs, close.rs, inference.rs, run.rs
  and file_session.rs. Every seam must be declared and used. Removing
  each seam's `#[cfg(test)]` or adding a production use is RED.
- The R5 revocation oracle is rebuilt per finding 8. A
  `model.call.admitted` probe fires right after a successful CAS, under
  the holds. The revoker is joined before judging. The invariant is that
  no admission follows a completed revocation.

B1a's `sb3_attempt_grant_revocation_cannot_precede_the_cas` does not
share the finding-8 flaw. It joins the revoker before judging. It judges
the attempt's admission result, which is Ok only after the CAS. And it
asserts nothing about successors.

Trace vocabulary the feat must emit:
- `gate:N`
- `refused:N`, `refused:hold`
- `admitted`
- `poll:none`, `poll:cancelled`
- `early`, `replay`
- `probe:<class>`

The size exception (SB3B-R8) is 1,800 changed lines, with production
at most 350.

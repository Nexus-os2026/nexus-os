# M3-Cb oracle register

Scope: Cb0 only (Owner option D), base 8df09f57017e822b196c6d2e1024d768e84b7780.
Cb1 lane and Cb2 composition remain pending; this slice changes no Mother code.

## Cb0 — broker marker helper

API (`nexus_governed_control::authority::ids`): `MarkerUnavailable` (Clone, Copy,
Debug, PartialEq, Eq) and `fresh_page_marker() -> Result<String, MarkerUnavailable>`.
16 new bytes from `getrandom::getrandom`; failure is `Err(MarkerUnavailable)`,
never a panic, fallback, counter, time, seed or previous value. Byte 6 becomes
`(b & 0x0f) | 0x40`, byte 8 `(b & 0x3f) | 0x80`; output is 32 lowercase hex.
`random16()`, identifier minting and their callers are unchanged. A marker is
framing data only, never consent, identity or authority.

Test seam: private `cfg(test)` `marker_fault`, one-shot per thread, replacing the
generator result with an error before encoding. No public provider injection.
It does not claim an actual OS generator failure.

| Oracle | Assertions | Inert stub (`Err`) | Implementation |
|---|---|---|---|
| `m3cb_broker_marker_is_fallible_v4` | 32 normal markers are v4 lowercase hex, consecutive ones differ; one injected fault returns exactly `Err(MarkerUnavailable)` under `catch_unwind`; the next call succeeds with a new marker | RED: normal-success assertion | GREEN |
| `m3cb_broker_marker_uses_approved_entropy` | Unique ungated production body equals the approved token sequence; fixed, counter, timestamp, seeded and getrandom-then-overwritten substitutes produce v4-shaped (and, where applicable, distinct) outputs yet fail the same control | RED: production body | GREEN |

| Plant | Mutation | Required kill |
|---|---|---|
| MP-Cb-D1 | Swallow injected entropy failure, return zero bytes | `m3cb_broker_marker_is_fallible_v4` typed-error assertion |
| MP-Cb-D2 | Counter-derived v4-shaped source | `m3cb_broker_marker_uses_approved_entropy` production-body control |
| MP-Cb-D2s | Seeded xorshift v4-shaped source | `m3cb_broker_marker_uses_approved_entropy` production-body control |

Evidence (local, Linux): inert stub, both oracles RED as registered; implementation,
both GREEN. D1 killed: fault returned `Ok("00000000000040008000000000000000")`
(the structural control also failed). D2 and D2s killed by the structural control
alone; their outputs passed the behavioral format/distinctness oracle, showing
the control discriminates v4-shaped deterministic sources. Each plant was
restored byte-for-byte (file and diff hashes equal) and never committed.

Review fix (Codex, LOW x2): the body calls `::getrandom::getrandom` and
`::hex::encode`, pinned in the approved tokens, so no module-local item can
capture them; the control also rejects `extern crate`, `mod getrandom`,
`mod hex`, `as getrandom` and `as hex` in production, `extern crate` in
`lib.rs`, and a renamed manifest dependency. Extraction and comparison skip
comments and string/char literals, so a `// }` or `/* } */` comment cannot end
the body early. Controls: unpinned body and both shadows fail; both harmless
comments pass. All five were RED on the prior checker and production.

Review-2 fix (Codex, LOW, crafted-only): the checker fails closed rather than
lexing everything. It rejects any raw string, byte/C raw string or raw
identifier outside comments in the production section and `lib.rs`; lexes
escaped and multi-byte char literals exactly; matches the rebinding pairs
(`extern crate`, `mod`/`as` + `getrandom`/`hex`) as adjacent words in both the
raw and comment-stripped text, so whitespace or comments between tokens do not
split them; and requires exactly one test-module header. Controls: seven split
rebindings, five mis-lexable literals and a second header are rejected (12 of
13 RED on the prior checker; the exact `extern\ncrate self as getrandom` form
was already rejected); both harmless comments still pass.

Structural equality pins source text; it is not a statistical proof of
unpredictability. MP-Cb-9 (fallback after typed failure) is owned by Cb1.

## Cb1 — frozen oracle checkpoint (base c78e43ec)

Authority: M3-Cb v2 §2 Cb1, §4, §5.2–5.5, §6 and final amendment.
Lane stays private/unselected; session inference stays denied. Returned File
requests are data, never publication proof. No accounting or broker changes.

The direct module calls the actual inert lane. Its counting caller returns
closed errors only. The Linux adapter module forwards the actual lane through
C1 and the ordinary broker; no replacement Builder behavior is implemented.
Expected pages use a handwritten full HTML body and fixed policy CSS; expected
requests duplicate the frozen system/schema and independently frame the data.
QM-MIN pins 708/1301 bytes and d339e4d57e7ad041a4f18d4dc933063be52d15eef2620de30604d984a0d6edec.
Counter reads are on the lane thread before invocation, at second caller entry,
and immediately after actual invocation; synchronized values cross threads.
Test-only seams inject marker results, post-render size, and unvalidated inputs.
The source control pins marker() and invoke_in: exactly one match marker(), no
other generator; helper/call-site deterministic substitutes are rejected (Cb1-R3).

| Oracle (m3cb_ prefix) | Frozen judgment | Inert observation |
|---|---|---|
| identity_and_context_are_fixed | identity/cap, role/caller/binding precedence | RED: inert identity/cap |
| validates_immediately_before_borrow | hash/contract/invocation/skipped content; positive caller | RED: assembler_fault vs page_input_identity |
| contract_credential_screen_is_not_ca_validation | Ca accepts synthetic credential; contract/lane refuse; normal/Unicode caller | RED: assembler_fault vs page_input_invalid |
| facts_ready_precedes_answer | email/41-character facts refused before caller; ready positive; lane.rs readiness branch masked by validate (defense in depth, coverage unresolved) | RED: assembler_fault vs facts_not_page_ready |
| caller_errors_are_terminal_and_content_free | all BrokerClass Refused/Failed, invalidated/debt; exact closed output, one entry | RED: assembler_fault vs model_invalidated |
| oversize_never_enters_caller | actual checked constructor: 3937 refused, 3936 enters | RED: assembler_fault vs model_request_too_large |
| marker_failure_fails_closed (direct) | typed/format faults, clear fault/default control | RED: assembler_fault vs model_record_unavailable |
| marker_source_is_broker_only | pinned helper/invoke_in bodies, single acquisition; helper/call-site counter negative controls | RED: source body absent |
| real_lane_charges_each_attempt | 1/1/1/1 or 2/2/2/2; exact full File bytes; one reservation | sandbox_blocked |
| corrections_are_exact_and_content_free | all eight literal blocks; exact two-message wire, fresh marker, canary absent | sandbox_blocked |
| needles_and_transport_failures_do_not_retry | needle/truncated/tool/thinking/transport/pre-dispatch; exact codes/counts | sandbox_blocked |
| two_refusals_stop_at_the_lane | queued third valid unused; two entries; fail:lane_fail:builder | sandbox_blocked |
| contract_budget_precedes_reservation | actual 2433 refusal; MIN/MAX-A/L/C positive sizes | sandbox_blocked (inherited behavior, no RED claimed) |
| wire_matches_frozen_request | complete raw body/schema order/configuration/message equality | sandbox_blocked |
| qm_min_full_user_golden | supplied marker; actual wire; full golden/hash | sandbox_blocked |
| empty_collections_have_frozen_behavior | sections/paragraphs TooLarge correction then terminal; items accepted | sandbox_blocked |
| email_refusal_precedes_file_construction | email→valid 0,1; email→email 0,0; @ market positive | sandbox_blocked |
| markers_fresh_per_request | default v4 framing, correction/separate invocation; guessed marker; duplicate refusal | sandbox_blocked |
| marker_failure_fails_closed (composed) | first/correction faults, retained first charge, default baseline/recovery | sandbox_blocked |
| input_positive_and_context_controls | exact normal/Unicode wire; cap-zero/Verifier no contract; actual oversize zero charge | sandbox_blocked |

Inert command: cargo test -p nexus-mother --locked --lib m3cb -- --nocapture.
20 tests compiled; 8 behavioral/source RED; 12 sandbox_blocked, not behavioral
kills. Broker creation refuses: "a directory above the runtime root can be
changed by another user". No assertion or RuntimeRoot guard was weakened.

Plants frozen: MP-M3-8/15/16/18 require actual C1 composition; MP-Cb-1 input
validation; 2 correction marker reuse; 3 deterministic source; 4 third entry;
5 request size; 6a diagnostic reason/text; 6b rejected-output echo; 7 error effect;
8 recording retry; 9 marker fallback; 10 context omitted/wrong; 11 context leak.
Each must fail its named §5 assertion and be restored byte-for-byte. RuntimeRoot
or AF_UNIX failure is sandbox_blocked, never a kill. Cb-D1/D2 belong to Cb0.

## Cb1-R2 — authorized oracle/fixture correction

Defects were found in outside execution: `m3cb1-out/outside-mother.log`
and `m3cb1-out/diag-m3cb_{marker_failure_fails_closed,markers_fresh_per_request,needles_and_transport_failures_do_not_retry}-{1,2}.log`;
these paths are under
`/tmp/claude-1000/-home-nexus-NEXUS-nexus-os/53f6c814-4d9f-4dfc-bcb5-ab55762fc5cc/scratchpad/`.
Diagnosis: `m3cb1-out/report-r2-diagnosis.md`; approved proposal A:
`m3cb1-out/proposed-marker-fixtures.diff`. No production code changed.

**SCOPE NOTE (binding):** "subsequent normal invocation" = a separate real
invocation of the actual lane (separate process), not the same FileSession;
same-thread seam consumption is covered by the direct oracle
`cb_oracles::m3cb_marker_failure_fails_closed`; within-invocation correction-fault
keeps the first charge (`markers/failure`). Production lifetime rules are NOT
relaxed.

Proposal A uses the existing bounded child runner with unchanged exit/sentinel
judgment. Independent File missions retain baseline `(1,1,1,1)`, failure
`(0,0,0,0)`, recovery `(1,1,1,1)`, cumulative `(1,1,1,1)` then `(2,2,2,2)`;
the third marker is compared with both earlier markers. Duplicate-marker and
within-invocation correction-fault judgments remain intact.

`transport/pre` expects `(1,0,0,1)`: one caller entry, zero chats/charges,
one admitted failed attempt. After live assertions, the fixture is dropped to
release the store's exclusive lock; a read-only native SQLite query (existing
Linux test convention: `python3`) asserts exactly one `model_call` body with
`detail.op = model_call`, `detail.outcome = unavailable`, and no `detail.call`
index. Neither the record nor the meaning of `attempts()` changes. No retry,
production/lifecycle change or assertion weakening is authorized.

## Cb1-R1 / R3 — authorized controls and scope ledger

R1.1: identifier-boundary `model_record` guard; real non-test seam remains RED,
three cfg(test) sites retained. R1.2: stacked Linux/test cfg; R1.3: capture alias.
Scope ledger: `c1b_port_oracles.rs` authorized by R1.1/R2; no production changes.
R3: additive call-site source control; local v4-counter plant must fail it.
L2 same-process cross-invocation freshness and L3 cap-positive Verifier stay Cb2.

## Cb2 — lifecycle and privacy composition (base 622c6bdb)

Authority: v2 §5.6–§5.8, §6, Cb2 column; Cb1-R3 L2/L3. Composition evidence
only: inherited production behavior, so tests were GREEN on 622c6bdb (no
red claimed); discrimination is by plants. No production change. Lane stays
private/unselected; a returned/staged File is request data, never
publication proof (Db). Linux-gated child per case (one broker per
process); observers pause on bounded channels and record C1 results only.
Deadlines use the manual clock; the between-attempt deadline is latched by
`FileSession::state` before call 2 is released. No fixed sleeps.

| Oracle (m3cb_ prefix) | Proves | Does not claim |
|---|---|---|
| invalidated_reply_cannot_publish | overtake while chat held; recorded Replied (hook) then `Invalidated`, `model_invalidated`, 1/1/1/1, 0 returned/dispatched/File, `fail:lane_fail:builder` | correction-loop schedules |
| stop_between_attempts_sends_no_correction | pause at caller entry 2; Stop / overtake / latched deadline; call 2 exactly `invalidated`; 2/1/1/1; ends stop:owner, fail:lane_fail:builder, block:budget_time | native transport fault |
| stop_during_correction_discards_reply | held 2nd chat: `failed:model_cancelled`; late: 2nd reply recorded, Stop before C1's release check, `invalidated`; 2/2/2/2, no 3rd call, stop:owner | content arriving after a closed transport |
| stop_cancel_halt_and_deadline_are_terminal | 5 interrupts x tags/chat: exact `model_cancelled`/`model_stopped`/`model_timeout`, charges 0/1, frozen ends; cancel is fail:lane_fail:builder, not Stop | attempt-record count (not frozen) |
| invalidation_after_answer_discards_output | pause after the actual answer (candidate File exists); overtake or Stop; 0 returned/dispatched/staged | zero assembly |
| record_debt_retains_charge_and_first_cause | adapter Owed/Uncertain: `record_unavailable`, 1/1/1/1, advance and close `Recording`, block:broker_refused:evidence_unavailable; Stop first keeps stop:owner + debt | native-store write failure |
| unrecorded_charge_sends_nothing | journal fails at charge: 0 chats/charges, no ok result, advance/close Err | a durable terminal record |
| replay_does_not_restore_caller | two reopens refused `Reopened`; cap-positive LaneInvoked/charge/dispatch counts equal; still unresolved; one chat/attempt | Mother replay internals (see MP-Cb2-8) |
| markers_fresh_across_invocations (L2) | three direct actual-lane invocations in one process, default path; markers from the raw wire are v4, 4 framings each, pairwise distinct | unpredictability |
| context_not_lent_to_cap_positive_verifier (L3) | actual Builder cap 2 gets the exact contract; after Owner authorize/execute the observed Verifier (declared cap 1) runs with `None` | other roles at cap>0 |
| canaries_absent_from_records_and_diagnostics | 10 schedules, canary note/facts: plain/hex/HEX absent from returned errors, caller results, lane verdict/text/effects, reason, close, Mother journal, all broker files | Owner working detail (Owner-only by design) |
| rejected_and_late_drafts_are_not_deliverables | rejected/late canaries absent from returned File, private store, whole test home; correction success staged File equals returned bytes | Db retention |

Inspected evidence paths (per case home under `$TMPDIR`): in-memory Mother
journal (`model_invocations()==1`); `broker/p3-evidence/evidence.sqlite3`
(+wal, required present); `broker/files/ledger.db` (+wal/shm; required
present iff a File was staged); `broker/files/coding-runs/<run>/index.html`
(staged deliverable: facts and accepted prose allowed, compared to the
returned bytes, other canaries scanned); `private/` (consented contract,
required nonempty; notes/facts legitimate there); the whole home for
rejected/late canaries; `workspace/index.html` stays absent (no Owner
publication). Observation surfaces: raw fake-peer bodies (test memory only).

Fixture findings (pre-commit, not production): the File effect is staged
under `broker/files/coding-runs/` until the Owner authorizes it, so
`Fx::index_exists()` (workspace) alone is too weak; Cb2 also checks staged
copies. Recorded deviations: L2/L3 live in the Linux adapter module (only
Cb2-allowlisted files); L2 forwards to the ordinary broker port because
`ModelRequest` exposes no content; `mod.rs` 6 lines (stacked cfg, R1.2) vs
3 estimated; per-file sizes vs 622c6bdb (added+deleted): lifecycle 584
(est 450), privacy 174 (240), support 106 (100), mod.rs 6 (3), this
register 76 (60); total 946 (est 853), under the 1,500 cap.

Plants (each applied alone, never committed, restored with equal sha256;
receipts `cb2-plants/receipts.json`, logs `cb2-plants/MP-Cb2-*.log` under
the scratchpad above). Site: mc = coordinator/model_calls.rs, co =
coordinator.rs, la = page_builder/lane.rs.

| Plant | Mutation (site) | Killed by (assertion) |
|---|---|---|
| MP-Cb2-1 | live() ignores invocation revocation (mc) | invalidated_reply: results `["ok"]` vs `["invalidated"]` |
| MP-Cb2-2 | no caller-entry liveness check (mc) | between: all 3 cases, call 2 `refused:halted`/`failed:model_over_budget` vs `invalidated` |
| MP-Cb2-3 | Replied released without live() (mc) | during/late: `["ok","ok"]` |
| MP-Cb2-4 | ModelCallCharged not appended (mc) | terminal: 5 chat cases, charges 0 vs 1 |
| MP-Cb2-5 | kept-step generation/pending check off (co) | post/invalidate: Builder LaneReturned 1 vs 0 |
| MP-Cb2-6 | model_debt never set (mc) | debt: 3 cases, advance `None` vs `Some(Recording)` |
| MP-Cb2-7 | ignore the charge append error (mc) | NOT A KILL: lane thread panics on `checked accounting`; no chat |
| MP-Cb2-7r | meter grants on NotRecorded (mc) | SURVIVED/MASKED: append failure fences the run (co write_record) → `failed:model_cancelled`, 0 chats |
| MP-Cb2-7b | 7r + skip that fence (mc+co, two-site) | unrecorded_charge: chats 1 vs 0 |
| MP-Cb2-8 | open swallows File `Reopened` (co) | replay: "reopen refused". Re-placed: a caller-restoring change inside replay is unreachable (attach precedes replay) — NOT EXERCISED |
| MP-Cb2-9a/9b | success text = note / hex(note) (la) | records: plain/one, plain/two "leaked (hex) into lane verdict/text" |
| MP-Cb2-10 | rejected reply appended to the File (la) | drafts:plain/two "leaked into returned File content" |
| MP-Cb2-11 | process-static first marker (la) | L2 pairwise-distinct assertion |
| MP-Cb2-12 | contract lent to any cap>0 role (co) | L3 "contract leaked to Verifier/1" |

Not exercised: a durable-record (journal/broker) canary plant (no Mother
write path carries lane text; 9a/9b judge returned diagnostics); MP-Cb2-7
single-site (masked by the fence, registered above); Mother replay
caller restoration (MP-Cb2-8 note).

Repeats: the 10 lifecycle-module oracles (every child case) 10x with
`--test-threads=1` and 10x default parallel: 20/20 runs, 10 passed each.

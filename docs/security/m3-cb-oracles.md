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

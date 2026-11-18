# M2e-D startup wiring (SB-3E U17): oracles, plants, claims and non-claims

Rulings: SB3E-R130.2-R130.3, R131.1-R131.4, R134.1-R134.5. Mission: U17 rev 1 with the revision 2 amendments.
Production file: `mother/host/src/hands.rs`. Oracles: `mother/host/src/hands_tests.rs` (`m2ed_*`).

## What M2e-D adds

`mother-hands --home <abs> --workspace <abs> --mission one-page-site` selects the model mode. No other argument may be added in this mode (`invalid_arguments`). Without `--mission`, argument parsing and behaviour are unchanged.

In model mode the startup worker runs the Bootstrap chain hold → workspace → `HostSession::register_local_model(ENDPOINT, qualified_pin())` → `finish_bootstrap`. Every step is inside the same `and_then` chain:

- `ENDPOINT` is the compile-time constant `http://127.0.0.1:11434`, bound only under `#[cfg(not(test))]`.
- The pin is the qualified pin of m2e-design §4: `qwen3.6:35b-a3b`, digest `a7eb95c5…bd87c`, runtime `0.40.13`, `num_ctx` 8192, temperature 0, seed 7, think off, truncate off.
- Any registration refusal prints `STARTUP REFUSED model_registration_refused` and fails the session (unless shutdown has already begun). `finish_bootstrap`, the only setter of READY (`host_session.rs:864`), is never called after a refusal.
- After `finish_bootstrap` is `Ok`, the host prints the backend's verified view only: `MODEL VERIFIED name=… digest=… identity=…`. It then prints `Model verified; the page mission is not available in this build. No model call is made.` and `HANDS READY`.
- `run` is refused in the main loop with `page_mission_unavailable`. No producer, lane, contract, `SessionPort` or mission is created, and `Runtime::run` (a pinned region) is unchanged.

## Security claims (AGENTS §6.1, §6.2, §6.6)

| ID | Claim | Where |
|---|---|---|
| I1 | In model mode, readiness is reached only after `register_local_model` returned `Ok` | the setup chain in `run`'s startup worker |
| I2 | The endpoint and pin are compile-time constants. The identity shown comes only from the backend's `ModelIdentityView` | `ENDPOINT`, `qualified_pin`, the `MODEL VERIFIED` line |
| I3 | No fallback to a no-model start, no retry, no re-registration (the core reserves once: `model.rs:128-134`) | B2 oracle (exact peer paths per class) |
| I4 | No lane before the M2e-D gates: `run` is refused in model mode | B0 oracle |
| I5 | The default mode makes no model request and its startup transcript is unchanged | B5 oracle |
| I6 | A held registration worker never extends the first close deadline; the host takes `failed_startup` and claims no clean close | H3-R oracle |
| I7 | After process death, the backend's startup report decides report-only; no registration and no request | X1 oracle |
| I8 | Output carries only the view's name and hex digests | `MODEL VERIFIED` format |
| I9 | No `Runtime` lock, producer or host lock is held across registration (it runs on `hands-startup` before the `Runtime` exists) | the startup worker |

## H3 as ruled (R131.2, R134.1)

The clause "fake peer has an open model request, connection count 1, at the producer deadline" (sb3 §7.3, m3e U6) is **reviewed-unconstructible** in production. Every shutdown trigger changes the session word. The registration gate's deny-only poll then cancels (`model.rs:248-253`), the exchange polls it every 20 ms and closes the connection (`exchange.rs:145-150`, `236-246`), and `failed_startup` calls `fail()` first (`hands.rs:1325` at the base). H3 keeps its purpose, a real held model worker reaching the real bounded fallback, and every other assertion literally. It is proved by two oracles:

- **H3-R** `m2ed_h3_held_registration_fallback`:
  - The LD_PRELOAD shim (mode `evidence`) holds the worker at its uncancellable registration evidence append (`model.rs:305`). It is armed when the peer receives `/api/show`.
  - Read from the real session while held: `operation=registration active=true owner_complete=false child_complete=true record=pending`, with `model_registered` not committed.
  - After Quit: exactly one `CLOSE MODEL` line, equal to `MODEL_UNKNOWN`. No `CLOSE FAILED stage=` (the normal close reporter was skipped). No `SESSION CLOSED recorded`, no `HANDS READY`.
  - `run` returns false, and the child exits 17. Quit to exit is under 5 s (the existing harness tolerance); about 2.8 s was measured against the 3 s product deadline.
- **H3-C** `m2ed_h3_open_request_cancelled_at_quit` (fail-safe control): the peer holds `/api/show` open.
  - At Quit: the peer sees EOF while the child is alive, before the producer deadline (about 21 ms measured). No further request. The close is clean (`SESSION CLOSED recorded`, exit 0), and the obligations read from the session are `record=not_required`.
  - At an input failure: also EOF (about 0 ms measured). The unclean fallback runs: exit 17, one `MODEL_UNKNOWN` line.
  - This proves that an open request cannot reach the deadline. It is not H3-R.

## Oracles

Each oracle runs production `hands::run` in a child of the test binary on a real PTY, through `SessionServices::open_projected` with the real `HostSession`, evidence store and kernel transport, against a loopback fake runtime. Test-only parts:

- the `#[cfg(test)]` endpoint hook `hands_tests::model_endpoint()`;
- the session stash and phase projection in `hands_tests::open_home`;
- the shim;
- the loopback peer.

The child's exit status is `run`'s outcome (0 true, 17 false).

| Oracle | Behaviour | Plants |
|---|---|---|
| `m2ed_mode_parse_and_run_refused` | B0: five refused argument lists (no home opened); `run` refused in model mode; no dialogue, lane or chat | P8 |
| `m2ed_registration_precedes_readiness` | B1: peer paths exactly `/api/version,/api/status,/api/tags,/api/show`; `MODEL VERIFIED` with the frozen identity `4d7211f035868b327c0e10f2bc23155a7d492d7beecc5b505edfa373db82f9a4`, before readiness; `record=recorded` | P2, P3, P5, P5b |
| `m2ed_no_override_in_production_source` | B1c (static): `hands.rs` contains no `env` text at all; one `#[cfg(not(test))]` endpoint binding to `ENDPOINT`; one registration call | P6 |
| `m2ed_registration_failure_blocks_readiness` | B2: wrong version, cloud enabled, digest absent, no listener, Stop during a held `/api/show`. Each: one exact refusal, the exact peer paths (no retry), no verified view or model readiness, `record=not_required`. Assertions read the transcript after exit | P1, P10 |
| `m2ed_h3_held_registration_fallback` | B3a H3-R | P9, P9b |
| `m2ed_h3_open_request_cancelled_at_quit` | B3b H3-C (control) | (control) |
| `m2ed_x1_death_restart_report_only` | B4 X1: SIGKILL with `/api/show` open, with the append held, and after readiness; two restarts each: `primary_unclean_sessions` 1 then 2, 0 requests, no registration (`operation=none`), `HANDS REPORT ONLY`, `run` refused, exit 17 | P10 |
| `m2ed_no_model_mode_makes_no_request` | B5: the default mode makes 0 requests to the hooked endpoint; its startup transcript is the base's, byte for byte | P7 |

## Mutation plants (local scripted runs on the final head; recorded by patch sha256 in the hand-back)

| Plant | Mutation | Killed by (assertion) |
|---|---|---|
| P1 (P4 merged) | registration `Err` ignored, the chain continues | B2: the exact `STARTUP REFUSED model_registration_refused` (`finish_bootstrap` refuses instead; masking named) |
| P2 | registration skipped in model mode (the core alone would admit READY: `model.rs:61-70`) | B1: no `MODEL VERIFIED` |
| P3 | registration after `finish_bootstrap` | B1: no `MODEL VERIFIED` |
| P5 | pin `seed` 7 → 8 (still verifies) | B1: the identity differs from the frozen literal |
| P5b | pin `num_ctx` 8192 → 4096 (refused by the kernel's config check) | B1: no `MODEL VERIFIED` |
| P6 | the non-test endpoint read from the environment | B1c: `hands.rs` names env |
| P7 | the mode check dropped (registration in both modes) | B5: no model request |
| P8 | the model-mode `run` refusal removed | B0: `REFUSED page_mission_unavailable` |
| P9 | `failed_startup` also prints `SESSION CLOSED recorded` | H3-R: no affirmative close notice |
| P9b | `failed_startup` prints an affirmative `CLOSE MODEL … record=recorded` | H3-R: only `MODEL_UNKNOWN` |
| P10 | the model readiness text printed regardless of state | B2: no model readiness line; X1: `HANDS REPORT ONLY` |

Equivalent mutant, not planted: printing the constant pin's identity instead of the backend view's identity is equivalent while verification holds.

## Not claimed

- The shipped binary's fixed endpoint is checked statically only (B1c). No test runs the shipped binary in model mode, because it would contact whatever listens on `127.0.0.1:11434`. The literal binary differential is U22.
- Registration against a loopback fake runtime is not model qualification or installed presence (Q-M3), not memory sufficiency (E-O50 `host_memory_low`, deferred to M-5, R131.4) and not OS containment of the runtime.
- No page mission, lane, chat or model call exists in model mode. M2e-D's H3 with an actual page chat is M-5.
- No cell, table, register, inventory or Gate Box data changes. The T7 registration row is deferred to U22 (R134.2).

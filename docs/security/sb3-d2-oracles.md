# SB-3 D2: close, deadline and death oracles (SB3D-R5)

Test-only. Base `11cd3d5f` (D1 and F1). No production file changed. New: `mother/src/adapter/session_model_bridge_d2_tests.rs`,
its `#[cfg(test)]` `mod` line at the end of `adapter/mod.rs`, and test-support appends (`Fixture::with_model_canonical`,
`SFx::canonical`), which bind the production history inspector so the model journal passes the seal's history check.

Route: real C1 caller, SessionPort, session broker, fake loopback peer. Each schedule runs in a watched child: one 30 s
deadline, kill and wait, a parent-PID-bound marker, `assert_watched` on the Mother join helpers, `Finally` releasing held
peers. The 5 ms poll only observes child exit.

## Oracles (all GREEN; no product RED)

| Case | Test | Pins |
|---|---|---|
| H1 | `sb3_mother_completed_call_allows_clean_close` | No-model and completed-call sessions through the A6 chain; Mother end, primary end and delivery each observed at their own step |
| H2 | `sb3_mother_unjoined_or_owed_prevents_clean_close` | Held chat and held charge are Busy; owed and uncertain are Recording, the charge standing |
| H2-held | `sb3_mother_open_model_request_blocks_close` | Open metadata or chat exposes the unresolved slot and Busy; Quit closes it and it joins with a recorded failure |
| H5 | `sb3_mother_post_seal_model_events_are_absent` | A post-seal admission on the sealed door is refused and moves no metadata, chat, record or obligation |
| H4 | `sb3_shutdown_keeps_first_deadline` | First deadline stands across repeated Quit, emergency and fatal at Bootstrap, held metadata, chat, charge, record and late result |
| X1-D | `sb3_mother_call_death_restarts_report_only` | Five death points, two restarts each, each restart its own process: report-only, no model operation, metadata and chat 0/0 |
| X2 | `sb3_missing_call_outcome_remains_unknown` | A missing outcome stays unresolved with no end inferred; a clean Home allows only a fresh Bootstrap |

## Observed behaviour, stated and not claimed as fixed

- **F1-b item (review-f1b-3).** A Quit landing during an attempt leaves the lane unreturned; history reports it unresolved
  and the seal refuses (H2 charge, H5 late). Pinned in `seal_refused_after_cancel`.
- **H5 with a File effect.** A completed call whose lane output carried a pending File effect at Quit did not reach a clean
  seal (`Unavailable`). The cause was not diagnosed; the case uses a lane with no effects. This is an open gap.
- **Second in-process reopen.** A second `HostSession::open_with_history` of one home in the same process returns
  `Backend("unavailable")`, also after `drop`. X1-D uses one process per restart, which is what a restart is. Not diagnosed.
- **H2-held.** Mother cannot hold a connection past Quit: the broker's `model.call.polled` hook is private. Connection at the
  deadline stays B's evidence.
- **X2.** The literal `unknown` has no public surface (forecast Q4); X1-B's broker evidence stands. Run, consent and meter
  restoration counts are broker-private (X1-B); Mother asserts the startup refusal.

## Plants (each compiled, reached its boundary; bytes restored by sha256)

| Plant | Mutation | Result | Kill |
|---|---|---|---|
| MP-SB-9-meter | `model_obligations` ignores `model_debt` (settlement read as completion) | H2 `owed` Recording | Behaviour |
| extra: active ignored | `model_obligations` ignores `model_active` | H2 `held chat`, H2-held `Busy` | Behaviour |
| MP-SB-12-clock | `begin_shutdown` overwrites the first deadline | H4 `triggers` | Behaviour |
| MP-SB-12-end | `observe_mother_end` also marks primary | H5 `Mother end observed` | Behaviour |
| MP-SB-14 startup | `ready()` always true | X1-D `report-only` | Behaviour |
| MP-SB-14 outcome | no mission ever unresolved | X2 `stays unresolved`, X1-D `unresolved` | Behaviour |
| extra: non-terminal state dropped | only the non-terminal arm of `unresolved` removed | X1-D `accepted: unresolved`; **X2 survives**, `pending_lane` still holds it | Behaviour, X1-D only |
| MP-SB-R32-D missing, unrelated | each check disabled | Control `tamper` not rejected | Verifier control |
| MP-SB-R32-D N/A, receipt | each check disabled | Named control fails; a later check still rejects the row | Masked: the row stays refused, by `unrelated oracle` |

A literal "`settle` stores `model_debt` false" plant was not run: settlement runs inside the attempt, before the debt is
set, so it would be a no-op. The `Reopened` File-session refusal was not planted; the restart stops earlier, at the startup
refusal. Not run on this head: the retained B1, B2, C1 and C2 plant sets, and MP-SB-11, which is broker-owned.

## Register

Before: 226 cited, 33 executed, 240 N/A, OPEN D2 690, F1 1. After: 226 cited, 45 executed, 240 N/A, OPEN D2 678, F1 1.
Total 1,190.

| Moved to `x:` (12) | Test | Value |
|---|---|---|
| `close_obligations` | H2-held; H5 | ClosingNormal; SealedFinalizing, ClosedClean |
| `close_restrict` | H4 | Bootstrap, ClosingNormal, ClosingRestricted |
| `emergency_restrict` | H4 | ClosingNormal, ClosingRestricted |
| `operation_complete` | H2-held; H1 | ClosingNormal; fault `none` |
| `charge_append` | H1 | Normal; fault `none` |

Each lifecycle value is asserted at the step. The 678 stay open by dimension: ingress 401 (no session test performs those
reductions at those sinks), lifecycle 136 (restricted states not reached at the sink), fault 112 (no injection at that sink's
boundary), schedule 29. `mother_end/budget` stays `open:F1`: no test reaches `CloseSeal::begin_mother_end` with that value.
The `open:D2` label is retained on cells D2 did not execute; re-owning them is an Architect decision.

## Verifier

`verify_d_block` binds one `SB-3D` row per R4/R7/R8/R9 sink to its frozen boundary and listed oracle. R8 rows are included:
D1 added none. `attempt_pin_release` is `OPEN:D2` (no Mother-level observation of the pin) and `close_report` is `OPEN:M2e-D`.
As with D1, the verifier shows a citation is a `#[test]` in that file at a listed cell, not that it ran; the receipt check
does that, and the Architect adds this file to `check-citations.py`'s pins.

## Deferred

H3, host X1 and production wiring: M2e-D. Dd. KSPAWN. The literal `unknown` (Q4). The 10x10 repeats on the integrated head.

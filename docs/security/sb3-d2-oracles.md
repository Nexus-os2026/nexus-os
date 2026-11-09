# SB-3 D2: close, deadline and death oracles (SB3D-R5, R6)

Test-only, base `11cd3d5f`. New: `mother/src/adapter/session_model_bridge_d2_tests.rs`, its `#[cfg(test)]` `mod` line, and
test-support appends (`Fixture::with_model_canonical`, `SFx::canonical`). Route: C1 caller, SessionPort, session broker, fake
peer. Each schedule runs in a watched child: one 30 s deadline set before spawn, kill and wait, a parent-PID-bound marker.

## What the oracles establish (repair 1: R6)

| Case | Test | Established |
|---|---|---|
| H1 | `sb3_mother_completed_call_allows_clean_close` | No-model and completed-call sessions through the A6 chain; three end facts distinct at each step |
| H2 | `sb3_mother_unjoined_or_owed_prevents_clean_close` | Chat held at the peer with the attempt frozen in its poll: Busy, and the broker seal refused before the chat closes. Held charge likewise. Owed/uncertain: Recording The clean-close positive releases and joins before Quit. The held-through-Quit case retains pending_lane debt after joining. Its seal refusal does not isolate the model-slot guard. |
| H2-held | `sb3_mother_open_model_request_blocks_close` | Open metadata or chat is an unresolved slot and Busy; Quit joins it with a recorded failure. Held to the deadline: B's evidence |
| H4 | `sb3_shutdown_keeps_first_deadline` | Acknowledged holds. The first deadline stands; each trigger must succeed; every advance is `Stopped`; only the held late result is the reply |
| H5 | `sb3_mother_post_seal_model_events_are_absent` | After a successful seal, real calls through the retained port and broker door return Halted. Meter/deadline callbacks and metadata/chat request counts remain zero; the native-store model_call occurrence count and model-obligation snapshot are unchanged. This does not isolate a refusal guard or exercise the specified callback/seal race. |
| X1-D | `sb3_mother_call_death_restarts_report_only` | Five death points, two restarts each in its own process (a second in-process open is expected fail-closed: the kernel Stop controller is claimed once per process): report-only, metadata/chat 0/0 |
| X2 | `sb3_missing_call_outcome_remains_unknown` | No success inferred; a clean Home allows only a fresh Bootstrap. Literal `unknown` is X1-B's (Q4) |

These results do not establish H5's specified race. A reply released after Quit cannot reach a successful seal: the F1-b
`pending_lane` debt leaves the mission unresolved and history refuses (pinned, `seal_refused_after_cancel`). That schedule is
**unconstructed**. In this harness, all Mother advances are joined before `sole`; the post-seal calls therefore use retained port and door handles. The native count is `model_call` substring occurrences across the store files, a limited sentinel.

## File at Quit (R6 e)

The original failure was not a pending File. `draft("...")` was refused by the inert check (`page_not_inert`), the mission Blocked.
Exact branch: `seal_close`, `core.actions ... any(!acknowledged())` (`host_session/close.rs`), confirmed by a temporary panic at that
branch; `actions_unacknowledged=1`, every earlier obligation clear. `Action::acknowledged` needs a released receipt, and a File refused
at prepare has none. Pinned as observed: conservative, no unsafe seal. Whether it should stay is an Architect question. A real
pending File (accepted page, `AwaitingApproval`) after a recorded call closes clean through the full chain.

## Plants (code head c66ae7c5, restored by sha256)

| Plant | Result | Kind |
|---|---|---|
| MP-SB-9-meter (obligations ignore debt) | H2 `owed` Recording | Behaviour |
| active ignored | H2 `held chat`, H2-held Busy | Behaviour |
| MP-SB-12-clock; MP-SB-12-end | H4 `triggers`; H5 `Mother end observed` | Behaviour |
| MP-SB-14 startup; outcome | X1-D `report-only`; X2 and X1-D `unresolved` | Behaviour |
| non-terminal arm only | X1-D `accepted`; survives X2 | Behaviour, X1-D only |
| H5 late step skipped | `the lane's call and the late attempt` | Source guard only |
| admit_attempt_private scope.live check removed; survives with the separate SEALED refusal still present. | **Survives** | Not killed |
| model slot dropped from the seal | **Survives**: a later seal branch refuses | Not killed |
| MP-SB-R32-D missing, unrelated | Control fails, tamper accepted | Verifier |
| MP-SB-R32-D N/A, receipt | Row still refused, by `unrelated oracle` | Masked |

## Register

226 cited, 45 executed, 240 N/A, 679 OPEN: D2 645, M2e-D 33 (`close_report`), F1 1 (`mother_end/budget`). Total 1,190. The 12 D2 `x:`
cells are listed in the verifier's table. Changing an OPEN cell’s owner leaves it OPEN. Before route opening, every OPEN cell must be resolved, and retained c: citations do not count; section 14.13 governs. The verifier checks literal table agreement and a source-text typo lint; it does not establish that a test is compiled, enabled or run.
Outstanding: H5's race (unconstructed); SB3D-F2 for a File refused after staging; the two surviving plants; masked controls; retained B1, B2, C1, C2
plants and 10x10 on the integrated head; the unplanted `Reopened` guard; broker-owned restoration observations; retained SB-1/SB-2 and C1 MP-E15/16/21-24 qualification; the exact-head citation check; CI and Gate Box. Deferred: H3, host X1, wiring (M2e-D), Dd, KSPAWN.
SB3D-F2, fixed in part (SB3D-R7): a File the broker refuses on the request itself before staging (name, content, name already present, display size, file type) is discharged, seals clean, and stays retained and journaled as refused. `post-seal/refused-file` now exercises the name-already-present refusal. Not fixed: a File refused after staging (for example `page_not_inert`, the case formerly under that name and now `post-seal/staged-file`) and any lifetime, authority or read failure still leave an unacknowledged action, so that session stays unsealed and the host reports a failed close.

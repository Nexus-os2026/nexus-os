# 2026-05-13 Full Forensic Audit: Phase 3 User Journeys

## Scope

- Audit date: `2026-05-13`
- Goal: execute mandatory user journeys against a live desktop build where possible
- Status in this checkpoint: partial, with launch attempts recorded and journey execution deferred

## What Was Actually Attempted

### Desktop Launch Attempts

1. Attempted `cargo tauri dev` from `app/src-tauri`.
2. First attempt stalled on Cargo’s artifact-directory lock because Phase 4 crate-cluster jobs were compiling concurrently.
3. Retried with an isolated target directory:

```bash
CARGO_TARGET_DIR=/tmp/nexus-tauri-audit-target cargo tauri dev
```

4. The frontend dev server came up on `http://localhost:1420`, but the Tauri desktop build did not reach a usable application window during this session segment before the build was stopped to free resources.

### Browser-Only Fallback Check

Browser-only execution is not a valid substitute for this phase:

- `app/src/api/backend.ts:90-108` hard-gates all backend calls behind `hasDesktopRuntime()` and throws `desktop runtime unavailable` when `window.__TAURI__` / `window.__TAURI_INTERNALS__` are absent.
- That means opening `http://localhost:1420` in a normal browser cannot exercise real backend commands.

### Existing Debug Binary Check

- `target/debug/nexus-desktop-backend` exists and starts, but no `NexusOS` window was observed from that binary during this session segment.
- Because a usable desktop UI did not surface, none of the mandatory journeys below were promoted from code-walk to runtime-verified status.

## Journey Status

| Journey | Runtime Executed | Status | Reason |
| --- | --- | --- | --- |
| `J1` Cold boot to first agent response visible in chat | No | `UNVERIFIED` | Live Tauri window did not become available during this checkpoint. |
| `J2` Create agent via UI → register → assign capability → run task → view result | No | `UNVERIFIED` | Live Tauri window did not become available during this checkpoint. |
| `J3` Swarm submit → approval → `@xyflow/react` DAG → execution → completion | No | `UNVERIFIED` | Live Tauri window did not become available during this checkpoint. |
| `J4` Governance block → reason surfaced → audit log → no mutation | No | `UNVERIFIED` | Live Tauri window did not become available during this checkpoint. |
| `J5` Self-improvement pipeline stages surface in UI | No | `UNVERIFIED` | Live Tauri window did not become available during this checkpoint. |
| `J6` Provider routing across local and cloud providers | No | `UNVERIFIED` | Live Tauri window did not become available during this checkpoint. |
| `J7` Checkpoint → mutate → rollback → exact restoration | No | `UNVERIFIED` | Live Tauri window did not become available during this checkpoint. |

## Code-Walk Anchors For Later Runtime Verification

These are not substitutes for runtime execution; they are the traced code paths to resume from.

| Journey | File:Line | Observation |
| --- | --- | --- |
| `J1` | `app/src/App.tsx:1063-1345`; `app/src/pages/AiChatHub.tsx:604-655,1362,1515-1599`; `app/src/api/backend.ts:153-159` | Cold-boot path routes into `dashboard`, then chat model loading and `send_chat` wiring live here. |
| `J2` | `app/src/components/agents/CreateAgent.tsx`; `app/src/api/backend.ts:125-149` | Agent creation and lifecycle controls are wired here. |
| `J3` | `app/src/lib/swarm/commands.ts:1-83`; `app/src/components/swarm/DagViewer.tsx`; `app/src/App.tsx:783` | Swarm planning, approval, state polling, and DAG rendering all hang off the swarm command wrappers and event bus. |
| `J4` | `app/src/pages/Audit.tsx`; `app/src/pages/AgentBrowser.tsx`; `app/src/App.tsx:2022-2024` | Governance-denial UI and audit-display surfaces are here. |
| `J5` | `app/src/pages/SelfImprovement.tsx`; `app/src/api/backend.ts` self-improve exports | Self-improvement page uses the self-improve command family. |
| `J6` | `app/src/pages/AiChatHub.tsx:1170,1362,1515-1599` | Provider/model attribution is displayed in the chat hub. |
| `J7` | `app/src/pages/TimeMachine.tsx:3-10,204-225,269` | Checkpoint creation, detail view, undo/redo, and what-if routes are wired here. |

## Findings

No runtime journey finding is promoted in this checkpoint because no mandatory journey completed end-to-end under a live desktop build.

## Coverage Gaps

| Gap | Status | Evidence |
| --- | --- | --- |
| Real desktop render and interaction | `UNVERIFIED` | `cargo tauri dev` build did not reach a usable window in this checkpoint. |
| Screenshot capture from a live `NexusOS` window | `UNVERIFIED` | No `NexusOS` window was found to capture. |
| Provider-routing journey across eight providers | `UNVERIFIED` | Requires live runtime plus provider availability; not reached. |
| Governance-block and rollback journeys | `UNVERIFIED` | Requires live runtime plus successful state mutation and observation surfaces; not reached. |

## Resume Notes

- Resume Phase 3 by relaunching `cargo tauri dev` after the heavy Phase 4 cargo jobs have drained, so the warmed shared target can be reused instead of forcing another cold build.
- Do not treat browser access to `http://localhost:1420` as sufficient for journey verification because `app/src/api/backend.ts:90-108` requires desktop runtime presence for real backend calls.

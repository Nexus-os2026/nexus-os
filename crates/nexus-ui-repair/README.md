# nexus-ui-repair

Crate #69 — the **autonomous QA scout** half of the Nexus OS UI repair pipeline.

> **Reference spec:** [`docs/roadmaps/NEXUS_UI_REPAIR_v1.1_scout_repair_split.md`](../../docs/roadmaps/NEXUS_UI_REPAIR_v1.1_scout_repair_split.md)

## Read-only by design

This crate is **read-only.** It cannot modify Nexus OS source code. It walks
the UI like a human QA tester, classifies what it observes, and writes
markdown bug reports to `~/.nexus/ui-repair/reports/`. That's it.

Repairs are performed in a separate, human-supervised phase (Phase B): Suresh
sits down with a generated bug report and Claude Code interactive (Opus 4.6
on the Max plan), works the fixes, reviews the diffs, runs the tests, and
commits. The scout is the input; the human + Opus is the worker.

## Phase 1.1 status

**Phase 1.1 = skeleton.** Every module compiles to a stub and passes one
trivial test, with one exception: `tests/acl.rs` exercises the I-2 filesystem
ACL for real. No specialists are wired, no `nexus-computer-use` integration,
no `nexus-memory` coupling, no provider calls. Phase 1.2 begins wiring real
behavior.

## Phase 1.3 status

**Phase 1.3 = structural gates wired.** The crate now imports
`nexus-computer-use` for **governance types only** — no screen capture or
input event code paths are exercised by the scout in Phase 1.3. Phase 1.3.5
wires Xvfb and real input.

## Phase 1.3.5 status

**Phase 1.3.5 = first commit in which the scout exercises real input
and capture code paths.** Landed:

- `governance::XvfbSession` — Hole A Layer 3 structural Xvfb display
  isolation. Spawns Xvfb on the first free display in `99..150` and
  reaps it on Drop.
- `specialists::EyesAndHands` — sync façade over
  `nexus_computer_use::capture::take_screenshot` and
  `MouseController::execute`. First specialist to call real input/
  capture functions.
- `governance::InputSandbox::validate_and_click` — Hole A Layer 2
  ACTIVE. Validates the target window through the real
  `AppGrantManager` denial path before issuing any input event.
- `tests/xvfb_smoke.rs` — two `#[ignore]`'d **structural** tests that
  spawn an `XvfbSession`, run `EyesAndHands::capture`, and drive
  `move_cursor` / `cursor_position` / `click`. They assert the wiring
  is alive (non-empty PNG, expected dimensions, cursor round-trips).
  They do **not** assert pixel-level capture-then-input-then-recapture
  behavior. Run manually with:

  ```text
  cargo test -p nexus-ui-repair --test xvfb_smoke -- --ignored
  ```

### Known limitation

The original Phase 1.3.5 design called for a capture → move_cursor →
recapture test that asserted the two captures differed. Bare Xvfb does
not produce reliable pixel ground truth for this style of test:

- Xvfb does not paint a software cursor into the framebuffer, so cursor
  moves are invisible to scrot even though `xdotool getmouselocation`
  confirms them at the X protocol level.
- With no window manager, MotionNotify delivery to clients like `xeyes`
  is unreliable, so target apps don't redraw on cursor moves.
- We observed an X server quirk where `xsetroot -solid` color changes
  succeed at the X protocol level but scrot reads back byte-identical
  PNGs from this Xvfb instance.

The `XvfbSession` + `EyesAndHands` + `validate_and_click` wiring is
correct and unit-tested. End-to-end pixel verification waits for
**Phase 1.5.5**, which will run a real Nexus OS Tauri WebView inside
the same Xvfb display — that target produces real framebuffer damage
events that scrot can see.

Phase 1.4 will wire `EyesAndHands` into the driver loop and layer the
`vision_judge` LLM on top.

Landed in 1.3:

- **Hole A Layer 2 — InputSandbox** (`governance/input_sandbox.rs`). Wraps
  `AppGrantManager` and validates target windows by probing with a benign
  `AgentAction::Click`; the negative test exercises the real
  `find_grant` + category fallback code path, not a string compare.
- **Hole B Layer 2 — ModalHandler** (`specialists/modal_handler.rs`).
  Classifies `Login | Confirmation | Error | Info | Unrecognized` modals
  and decides `ClickCancel | Hitl | Halt`. Three unrecognized modals in a
  session triggers HALT.
- **Hole B Layer 3 — PageDescriptor opt-ins** (`descriptors/page_descriptor.rs`).
  Destructive opt-ins must (1) not target `/settings|/governance|/memory`,
  (2) set `fixture_required = true`, (3) reference a present fixture,
  (4) the fixture must be `FixtureKind::Throwaway`.
- **Calibration recording** (`governance/calibration.rs`). JSONL
  append-only log for `vision_judge` similarity scores + ground truth.
  Recompute lands in Phase 1.4.
- **SpecialistCall + `AuditLog::record_specialist_call`**. The I-5
  output-capture seam. Phase 1.4 wires the driver loop to call this on
  every specialist invocation.

Still gated to **Phase 1.3.5**: Xvfb isolation, real input events, real
screen capture, live DOM path for the enumerator.

Still gated to **Phase 1.4**: chrono timestamps, dry-run flags, heartbeat
files, vision_judge LLM integration, driver loop wiring.

## The five invariants (v1.1 §3)

| # | Name | What |
|---|---|---|
| **I-1** | Kernel allowlist | Path allowlist; belt + suspenders. |
| **I-2** | Read-only-by-construction | Three layers: filesystem ACL (Phase 1.1), per-app input governance (Phase 1.3), OS-level isolation (Phase 1.3). |
| **I-3** | HITL by definition | The scout reports; the human repairs. There is no autonomous edit path. |
| **I-4** | Immutable provider routing | Allowed: `codex_cli`, `ollama`, `anthropic_api` (capped). Forbidden: `claude_cli`, `claude_ai_credits`. |
| **I-5** | Replayable sessions | Append-only signed log; `(inputs, output)` capture per specialist call. |

## Layout

```
src/
├── governance/    identity, invariants, routing, acl, audit
├── driver/        5-state scout machine + main loop
├── specialists/   enumerator, vision_judge, classifier, report_writer
├── ledger/        in-memory verification ledger
└── replay/        replay harness (Phase 1.4)
```

See the v1.1 spec linked above for the full driver loop, provider routing
table, and bug report contract.

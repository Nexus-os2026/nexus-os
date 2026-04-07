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

# NEXUS_UI_REPAIR v1 — Autonomous QA Driver
**Crate #69 · `nexus-ui-repair`**
Roadmap drafted: April 7, 2026 · Target: Nexus OS v10.10.0

---

## 0. Thesis

One driver agent. Not a swarm.

`nexus-ui-repair` opens Nexus OS, walks the UI like a human QA tester, watches the screen with vision, detects when an interaction does nothing, traces the root cause through TS → Tauri command → Rust handler, applies a fix through `nx`, re-tests, and moves on. Every action is governed by the same kernel that governs Builder and Code. Every fix is signed, audited, and replayable.

It is the first agent in the Nexus OS family whose job is to make *all the other agents' work* trustworthy.

---

## 1. Why one driver, not a swarm

Multi-agent QA looks tempting — "one agent per page!" — and fails for three concrete reasons at this granularity:

1. **Coordination tokens exceed work tokens.** At the size of a single button click, the cost of agents negotiating who tests what dwarfs the cost of just testing it. Swarms make sense when sub-tasks are large enough to amortise the handshake. UI repair sub-tasks are not.
2. **Shared UI state diverges.** Two agents driving the same Nexus OS window race on focus, modals, and navigation. Two agents driving two windows lose cross-page flows (login → navigate → action). Sharding the UI breaks the UI.
3. **Audit fragmentation.** Governance invariants must hold *atomically* across enumerate → act → repair. If three agents own three slices of that loop, the kernel cannot enforce I-2 ("every fix goes through `nx`") without a distributed lock — and now you have a distributed systems problem instead of a QA problem.

**Specialists exist, but they are tools the driver calls, not peers it negotiates with.**

Specialist roster (all invoked as `nx` tool calls from inside the driver loop):
- `enumerator` — DOM/AX tree scrape, element fingerprinting (Ollama, $0)
- `vision_judge` — "did the screen meaningfully change?" (Codex CLI default, Anthropic API on ambiguity)
- `code_locator` — grep/AST trace from onClick → invoke() → Rust handler (Ollama + Codex CLI)
- `repair_proposer` — patch generator (Codex CLI default, Anthropic API for >2-file fixes)
- `regression_runner` — `cargo test -p <crate>` on the touched crate only (no provider, deterministic)

The driver is a state machine. The specialists are pure functions it calls. There is exactly one audit trail.

---

## 2. The governed driver loop

```
                     ┌─────────────────────────────────┐
                     │  ENUMERATE  (Ollama)            │
                     │  scrape DOM, fingerprint        │
                     │  elements, query VerificationLedger │
                     └────────────────┬────────────────┘
                                      │ untested elements
                     ┌────────────────▼────────────────┐
                     │  PLAN  (Codex CLI)              │
                     │  pick next element, define      │
                     │  expected post-condition        │
                     └────────────────┬────────────────┘
                                      │
                     ┌────────────────▼────────────────┐
                     │  ACT  (nexus-computer-use)      │
                     │  click / type / navigate        │
                     └────────────────┬────────────────┘
                                      │
                     ┌────────────────▼────────────────┐
                     │  OBSERVE  (vision_judge)        │
                     │  did the screen change?         │
                     │  did logs/console emit errors?  │
                     └────────────────┬────────────────┘
                                      │
                     ┌────────────────▼────────────────┐
                     │  CLASSIFY                       │
                     │  PASS / DEAD / ERROR / HANG     │
                     └─────┬──────────┬──────────┬─────┘
                           │PASS      │DEAD/ERROR│HANG
                           │          │          │
                           │   ┌──────▼──────┐   │
                           │   │  REPAIR     │   │
                           │   │  trace →    │   │
                           │   │  patch →    │   │
                           │   │  nx apply → │   │
                           │   │  cargo test │   │
                           │   └──────┬──────┘   │
                           │          │          │
                     ┌─────▼──────────▼──────────▼─────┐
                     │  LOG (Ed25519 signed) → ADVANCE │
                     │  update VerificationLedger       │
                     └─────────────────────────────────┘
```

Every arrow is a state transition. The kernel checks invariants at every transition, not inside specialists. This is what makes "one driver" *governable* in a way a swarm is not.

---

## 3. The five hard invariants

Same shape as Code and Builder. Self-improvement may tune defaults and rankings; it may **never** rewrite these.

| # | Invariant | Enforcement |
|---|---|---|
| **I-1** | **Kernel allowlist.** `nexus-ui-repair` may not edit files in `nexus-governance-oracle`, `nexus-crypto`, `nexus-memory/src/kernel/**`, or its own governance module. | Path allowlist checked before every `nx` patch apply; violation → hard abort + audit entry. |
| **I-2** | **All edits through `nx`.** No raw file writes. Every fix is an `nx` session with identity, fuel, ACL, and audit. | Driver has no `fs::write` capability; only `nx_apply_patch` tool. |
| **I-3** | **HITL threshold.** Any single repair touching >3 files, any file under `*/security/*` or `*/auth/*`, or any change to a public API surface requires human approval before merge. | HITL gate raised in REPAIR state; driver pauses, surfaces diff in Nexus OS approval UI. |
| **I-4** | **Immutable provider routing.** Self-improvement may reorder, cache, and prefer providers within the allowed set; it may **not** add `claude_cli` or `claude_ai_credits` to the autonomous routing table. | Routing table loaded from a signed config at startup; mutation requires HITL + new signature. |
| **I-5** | **Replayable sessions.** Every driver run produces an Ed25519-signed session log sufficient to reconstruct every screen state, action, classification, and repair. | Session log written append-only; replay harness in `tests/replay.rs` runs in CI. |

---

## 4. Provider routing & the $10 budget

Cost discipline is load-bearing. Here is the routing table and the math.

| Specialist | Default | Escalation | Why |
|---|---|---|---|
| enumerator | Ollama `gemma4:e2b` | — | Pure DOM scrape, no escalation needed |
| vision_judge | Codex CLI (GPT-5.4) | Anthropic API (haiku → sonnet) | Vision ambiguity is the main escalation trigger |
| code_locator | Ollama `gemma4:e4b` → Codex CLI | Anthropic API only on cross-crate traces | grep + AST is mostly deterministic |
| repair_proposer | Codex CLI (GPT-5.4) | Anthropic API for fixes spanning >2 files | Most dead buttons are 1-file fixes |
| regression_runner | (deterministic) | — | `cargo test`, no LLM |

**Forbidden in the autonomous routing table** (I-4): `claude_cli` (consumes Max plan, ToS risk), `claude_ai_credits` (account ban risk). These are reachable only via explicit HITL escalation by Suresh from the approval UI.

### Worked cost estimate for the full 87-page rollout

Assumptions, conservative:
- 87 pages × ~25 interactive elements/page = ~2,175 elements
- Enumeration + vision + classification: $0 (Ollama + Codex CLI)
- ~15% of elements are dead/broken (~325 repairs)
- Of those, ~20% escalate to Anthropic API (~65 escalations)
- Average escalation: ~8K input tokens + ~2K output tokens with Sonnet
- Sonnet pricing: $3/M input, $15/M output
- Per escalation: ~$0.024 + ~$0.030 ≈ **~$0.054**
- Total: 65 × $0.054 ≈ **~$3.51**

**$10 credit covers the full 87-page rollout with ~3× safety margin** for re-tests and false-positive escalations. If the rollout exhausts the budget, the driver halts and surfaces a HITL request — it does not silently switch providers.

---

## 5. Phase 1 — Drive the Builder Teams page end-to-end

**Scope:** One page. The Nexus Builder Teams page in Nexus OS. Known dead buttons. Small enough to eyeball end-to-end. If `nexus-ui-repair` cannot fix Teams, it has no business near the other 86 pages.

### 1.1 Skeleton crate
- Create `crates/nexus-ui-repair/` with the standard governance scaffolding (identity, fuel, ACL, audit, replay).
- Wire into the workspace, cargo fmt/clippy/test green on the empty crate.
- Add `nx` integration: `nexus-ui-repair` declares `nx_apply_patch` as its only mutation tool.

### 1.2 Driver state machine
- Implement the loop in §2 as a real Rust state machine (`enum DriverState`, explicit transitions, transition guards = invariant checks).
- No specialists yet — every state is a stub returning `Pending`.
- Test: state machine round-trips through every transition under property tests.

### 1.3 Specialists, in dependency order
1. **enumerator** — wraps `nexus-computer-use` screen capture + DOM accessibility tree scrape; emits `Element { id, fingerprint, kind, bounds, handler_hint }`.
2. **VerificationLedger** — backed by `nexus-memory`, schema `(page, element_fingerprint) → { status, last_tested, last_repair_session, taint }`.
3. **vision_judge** — given (before, after, expected_postcondition) → `Changed | Unchanged | Ambiguous`. Codex CLI default; ambiguity escalates to Anthropic API.
4. **code_locator** — given an element + handler_hint, returns `(ts_handler_path, invoke_target, rust_handler_path)` or a `MissingLink` enum identifying which step broke.
5. **repair_proposer** — given a `MissingLink`, generates a patch. One-file fixes inline; >2-file fixes are HITL-gated under I-3.
6. **regression_runner** — `cargo test -p <touched_crate>` plus the relevant frontend test file. Deterministic, no LLM.

### 1.4 Drive Teams page end-to-end
- Driver opens Nexus OS, navigates to Builder → Teams.
- Enumerates every interactive element on the page.
- For each: ACT → OBSERVE → CLASSIFY → (REPAIR | LOG).
- Every repair goes through `nx`, signed, audited, regression-tested on the touched crate.
- Stops only when every element on the page has status `Verified` or is HITL-pending.

### 1.5 Phase 1 acceptance criteria

Concrete, measurable, no wiggle room:

- [ ] **All interactive elements on Teams page enumerated** — driver reports a count and a per-element fingerprint table.
- [ ] **≥80% auto-repaired** without HITL escalation. Below 80% means the loop is too brittle for scale; tune before Phase 2.
- [ ] **Zero regressions** across the existing 5,400+ Rust tests and 352 frontend tests after the Phase 1 run completes.
- [ ] **Full session replay** — `cargo test -p nexus-ui-repair --test replay` reconstructs the run from the signed log and produces byte-identical state transitions.
- [ ] **No invariant violations** — `grep -r "INVARIANT_VIOLATION" ~/.nexus/audit/` returns empty.
- [ ] **Cost ceiling** — Phase 1 burns ≤ $0.50 of the $10 Anthropic credit. If it exceeds, we have an escalation bug, not a feature.
- [ ] **Suresh eyeballs the page** and confirms every button now does what its label says.

---

## 6. Phases 2–7

### Phase 2 — Generalize to any single page (≈ 1 week after P1)
- Page descriptor format: `PageDescriptor { route, expected_elements?, critical_flows?, fixtures? }`.
- Driver accepts a descriptor and runs the Phase 1 loop on any page.
- Test on three pages with varying complexity: Builder/Teams (done), Governance Oracle (LLM-heavy), Settings (form-heavy).

### Phase 3 — Cross-page flows
- Flow descriptor: ordered sequence of (page, action) tuples with assertions between steps.
- Critical flows to encode first: login → dashboard, Builder create-project → deploy, Code session → audit view.
- Vision_judge gains a `flow_state_check` mode for inter-page assertions.

### Phase 4 — All 87 pages, prioritized
- Auto-generate descriptors for all 87 pages from the existing page registry.
- Priority order: critical (governance, auth, payment) → high-traffic → long-tail.
- Continuous run mode: driver works through the queue, persists state across restarts via the VerificationLedger.

### Phase 5 — CI gate
- `nexus-ui-repair` runs on every commit affecting `frontend/**` or any crate exposing Tauri commands.
- Taint propagation: only re-tests elements whose dependency graph touches the changed files.
- Blocks merge on regression; surfaces HITL requests in the PR.

### Phase 6 — Self-improving repair patterns
- Mirrors the pattern already shipped in `nexus-computer-use`: successful repair traces become reusable patterns; failed traces become anti-patterns.
- Self-improvement is bounded by I-1 and I-4 — it can re-rank repair strategies and cache provider routes, never edit governance or add forbidden providers.
- Drift detection: behavioral envelope on repair success rate; sudden drops trigger HITL review.

### Phase 7 — Builder integration
- Apps generated by Nexus Builder inherit a `nexus-ui-repair` test harness automatically.
- Builder's quality critic gains a "QA driver passes" gate before deploy.
- Closes the loop: Builder ships apps that come pre-tested by the same driver that tests Nexus OS itself.

---

## 7. File layout

```
crates/nexus-ui-repair/
├── Cargo.toml
├── src/
│   ├── lib.rs
│   ├── governance/
│   │   ├── identity.rs        # Ed25519 session identity
│   │   ├── invariants.rs      # I-1 through I-5, checked at transitions
│   │   ├── routing.rs         # signed provider routing table (I-4)
│   │   └── audit.rs           # hash-chained session log
│   ├── driver/
│   │   ├── state.rs           # DriverState enum + transitions
│   │   ├── loop.rs            # the governed loop in §2
│   │   └── hitl.rs            # I-3 escalation surface
│   ├── specialists/
│   │   ├── enumerator.rs
│   │   ├── vision_judge.rs
│   │   ├── code_locator.rs
│   │   ├── repair_proposer.rs
│   │   └── regression_runner.rs
│   ├── ledger/
│   │   ├── verification.rs    # VerificationLedger over nexus-memory
│   │   └── taint.rs           # dependency-graph taint propagation
│   └── replay/
│       └── harness.rs         # I-5 replay test
├── tests/
│   ├── state_machine.rs       # property tests on transitions
│   ├── invariants.rs          # one test per invariant, must fail on violation
│   ├── replay.rs              # full session replay
│   └── phase1_teams.rs        # the Phase 1 acceptance run
└── README.md
```

---

## 8. Open questions for Suresh

1. **HITL surface.** Should approval requests appear in a new Nexus OS page (`UIRepairApprovals.tsx`), or fold into the existing Governance approval UI? My instinct: fold in — fewer surfaces, same audit trail.
2. **Taint granularity.** Per-file taint is cheap but coarse. Per-function taint is precise but needs an AST index. Phase 1 ships per-file; Phase 5 may need per-function. Defer the call.
3. **Vision_judge ambiguity threshold.** What confidence level triggers Anthropic API escalation? Suggest 0.7 cosine similarity on the before/after embedding as a starting point, tune empirically in Phase 1.
4. **Replay storage.** Session logs grow fast at 87-page scale. Keep the last N runs hot, archive the rest to compressed cold storage? Phase 5 problem, but worth flagging now.

---

## 9. What this is not

- **Not a fuzzer.** It's a goal-directed QA driver. Random input generation is a Phase 6+ extension if patterns suggest it.
- **Not a replacement for `cargo test`.** Unit tests stay. This catches integration-level UI bugs that unit tests cannot see.
- **Not a swarm.** See §1.
- **Not allowed near `claude_cli` or `claude.ai` credits.** See I-4.

---

*v1 draft. No code shipped until the architecture is signed off.*

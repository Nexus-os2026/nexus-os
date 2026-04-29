# ADR 0001: Broker Architecture — Defer Orchestrator, Delete Collaboration Crate

- **Status:** ACCEPTED
- **Date:** 2026-04-29
- **Deciders:** Suresh Karicheti (architect), Claude (technical advisor)
- **Supersedes:** None
- **Superseded by:** None

## Context

Phase 4 deferred Architecture Q1 with the note "Broker → DEFERRED as Architecture Q1 (collaboration crate has no LLM coupling)." The deferral conflated two questions. After Phase 5 + Track C closed (commit 8d5c5b4a), the preflight performed against HEAD revealed:

1. **BrokerAdapter exists and is production-registered.** crates/nexus-swarm/src/adapters/broker.rs is a thin LLM-prompt wrapper structurally identical to Artisan/Herald. It is registered in app/src-tauri/src/commands/swarm.rs:128 and the Director's planner can route to it via the "broker" capability id. It does NOT bridge to the agents/collaboration/ crate, despite doc-comment claims.

2. **agents/collaboration/ has zero production callers.** Repo-wide grep confirms: 895 LOC, 22 unit tests, all self-contained. The only Cargo.toml dep is in agents/conductor/, which never imports the crate. One integration test uses AgentMessage + GovernedChannel from outside the swarm path.

3. **Phase 4c (sub-node delegation) is hypothetical.** SubagentSpawnRejected error variant and HighRiskEvent::SubagentSpawnAttempt oracle hook exist as defensive scaffolding from Phase 1. No design doc, no tracking ticket, no in-progress code.

4. **Coordinator is flat-DAG only.** No spawn_child, sub_dag, or nested_dag infrastructure. Sub-DAG composition would be ~500–800 LOC of new logic plus budget composition (greenfield) and oracle nested approval (greenfield).

5. **The Collaboration frontend page (app/src/pages/Collaboration.tsx) calls nexus-collab-protocol, NOT nexus-collaboration.** Two crates with collided naming. The frontend is unaffected by this decision.

## Decision

We make three coordinated changes:

1. **Keep BrokerAdapter as a coordination-themed LLM capability.** It works, the Director routes to it, the tests pass. Its job is generating coordination prompts via LLM, not orchestrating sub-agents.

2. **Correct the misleading doc comments** in crates/nexus-swarm/src/adapters/broker.rs and mod.rs. The "wraps nexus-collaboration" claim is false. Doc should match reality: BrokerAdapter is an LLM-prompt wrapper with coordination-themed prompting.

3. **Delete agents/collaboration/.** Remove the dead nexus-collaboration dep from agents/conductor/Cargo.toml. Handle the integration test (tests/integration/tests/e2e_v4_systems.rs) per the executor's judgment: delete the test if AgentMessage + GovernedChannel are central; rewrite without those imports if incidental.

We do NOT pursue orchestrator-style Broker (the SwarmOrchestratorEntry option from the original three-option space). That option requires multi-crate greenfield work — coordinator nesting, budget composition, audit chaining, oracle nested approval — estimated 5–10x the cost of Track B + Track C combined. Phase 4c isn't scheduled, so the prerequisites for orchestrator semantics aren't in motion.

## Consequences

### What we keep

- BrokerAdapter (working LLM capability, registered in production, covered by tests).
- The "broker" capability id in routing_defaults.rs.
- All swarm functionality the Director currently uses.

### What we lose

- 895 LOC of inert Rust code in agents/collaboration/.
- 22 unit tests that pin Blackboard, GovernedChannel, and Task/SubTask behaviors.
- The specific implementation choices (AccessLevel enum variants, governance error types, channel send/receive patterns).

### What we preserve in this ADR

The design patterns that warranted the crate's existence are captured below in "Design Patterns Worth Preserving." When Phase 4c lands, these patterns can be reimplemented inside a future crates/nexus-swarm-orchestrator/ crate with full understanding of the original intent.

### Audit consequences

- **One-time:** ~900 LOC removed, one Cargo.toml dep removed, one integration test rewritten or deleted.
- **Recurring:** zero "what is this for?" audit toil on agents/collaboration/.

## Alternatives Considered

### Option A: Status quo + doc fix only

Keep agents/collaboration/ as inert utility. Fix the misleading BrokerAdapter doc comments. No deletion.

**Why rejected:** Treats dead code as zero-cost. The audit tax is recurring — every contributor onboarding asks "what is this for?", every audit cycle re-litigates. Opportunity cost of mental load matters even when LOC stays flat.

### Option B: Reverse the deferral, build orchestrator-style Broker

Define SwarmOrchestratorEntry trait. Wire BrokerAdapter to nexus-collaboration's primitives. Lift coordinator to support nested DAGs. Add budget composition. Add audit chaining. Add oracle nested approval.

**Why rejected:** Multi-crate greenfield work. Estimated 5–10x the cost of Track B + Track C combined. No user demand driving it (zero production callers of nexus-collaboration). Phase 4c isn't scheduled. Premature investment with no concrete payoff.

## Design Patterns Worth Preserving

When Phase 4c lands and orchestrator-style coordination is implemented, the following patterns from the deleted agents/collaboration/ crate are worth reimplementing:

### Blackboard with AccessLevel gating

A shared key-value store where each entry has an access level (Public, Restricted, Sealed). Read/write operations check the requesting agent's clearance against the entry's level. The pattern is useful for multi-agent state sharing without granting global write access.

Original implementation: agents/collaboration/src/blackboard.rs (274 LOC, 7 tests).

Reimplementation guidance: keep AccessLevel as a 3-variant enum. Storage is HashMap<String, BlackboardEntry>. Each entry has a writer_agent_id, written_at, and access_level. Read/write require an AccessLevel parameter; mismatch returns BlackboardError::AccessDenied.

### GovernedChannel with send/receive governance gates

Async message channel between agents where every send AND every receive passes through a governance check. The check is policy-determined; failure produces a typed ChannelError. Pattern is useful for any cross-agent communication that needs audit visibility.

Original implementation: agents/collaboration/src/channel.rs (322 LOC, 8 tests).

Reimplementation guidance: keep send_governed and receive_governed as the public API. The governance check is a closure or trait object; the channel doesn't own policy. ChannelError variants: GovernanceDenied, ChannelClosed, MessageTooLarge.

### Task / SubTask lifecycle

A Task contains 1+ SubTasks. SubTasks have status (Pending, Running, Completed, Failed). Task transitions: Created → Planning → Executing → Completed | Failed. Pattern is useful for multi-step work tracking inside an orchestrator's scope.

Original implementation: agents/collaboration/src/orchestrator.rs (294 LOC, 7 tests).

Reimplementation guidance: SubTaskStatus is a 4-variant enum. Task::transition_to validates state transitions. Failed SubTasks cause Task to transition to Failed; all-Completed SubTasks cause Task to transition to Completed.

## Trigger Conditions for Revisiting

Reopen this decision if any of the following become true:

1. **Phase 4c sub-node delegation moves from "hypothetical" to "scheduled" or "in-progress."** Specifically: a design doc lands in docs/, or a tracking ticket is opened, or any production code emits HighRiskEvent::SubagentSpawnAttempt.

2. **A user-facing surface requires multi-agent orchestration that BrokerAdapter's LLM-prompt wrapping cannot satisfy.** Specifically: a page or workflow that needs typed agent-to-agent message passing, shared state with access control, or task lifecycle tracking.

3. **The number of agents in the swarm grows past ~10 active agent crates AND coordination patterns emerge that benefit from typed primitives over LLM-prompt-driven coordination.**

If none of these become true within 12 months of this ADR landing, the deletion stays. The patterns preserved here remain available for future re-implementation when the prerequisites do arrive.

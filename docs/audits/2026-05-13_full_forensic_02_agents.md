# 2026-05-13 Full Forensic Audit: Phase 2 Agent E2E Correctness

## Scope

- Audit date: `2026-05-13`
- Scope: registered swarm agents and orchestration runtime observed in Phase 0
- Method: crate-doc inspection plus executed harness tests only
- Provider policy for this phase: local or mocked providers only; no live Anthropic/OpenAI production calls

## Executed Harness Commands

All commands below were executed successfully in this session.

- `cargo test -p nexus-swarm --test real_delegation_tests --test adapter_emission_tests --test oracle_bridge_tests`
- `cargo test -p coder-agent --lib swarm_entry::tests:: -- --nocapture`
- `cargo test -p social-poster-agent --lib`
- `cargo test -p nexus-integration --test swarm_harness_scenario_a --test swarm_harness_scenario_b --test swarm_harness_scenario_c --test swarm_harness_scenario_d --test swarm_harness_scenario_e --test swarm_harness_scenario_f --test swarm_harness_scenario_g`

## Per-Agent Status

| Agent / Runtime | Spec Source | Representative Input / Harness | Executed Status | Notes |
| --- | --- | --- | --- | --- |
| `SwarmDirector` | `crates/nexus-swarm/src/director.rs` | `"post a tweet about Rust"` via scenarios `A` and `C` | PASS with findings | Planning, approval, and denial paths executed; audit behavior is broken on the Director path. |
| `SwarmCoordinator` | `crates/nexus-swarm/src/coordinator.rs` | canned DAGs via scenarios `B`, `D`, `G` | PASS with findings | Event sequencing, denial handling, retry/no-retry, and plan-drift paths executed; terminal failure signaling is broken. |
| `artisan` / `ArtisanAdapter` | `crates/nexus-swarm/src/adapters/artisan.rs`; `agents/coder/src/swarm_entry.rs` | `{"task":"hello","style":{"language":"rust"}}` via `real_delegation_tests` | PASS with findings | Delegation into `CoderEntry`, cancellation, invalid input, phases, and budget emission executed. |
| `herald` / `HeraldAdapter` | `crates/nexus-swarm/src/adapters/herald.rs`; `agents/social-poster/src/swarm_entry.rs` | `{"channel":"X","audience":"Rust devs","message":"ship it"}` via `real_delegation_tests` | PASS with findings | Delegation into `SocialPosterEntry`, dry-run, credentials-missing, cancellation, publish failure typing, capability denial, and fuel exhaustion executed. |
| `broker` / `BrokerAdapter` | `crates/nexus-swarm/src/adapters/broker.rs`; `docs/architecture/decisions/0001_broker_fate.md` | `{"directive":"dispatch"}` via `adapter_emission_tests` | PASS | Phase sequence and observe payload executed successfully. Failure-injection coverage is incomplete. |
| `scout` / `ScoutStub` | `crates/nexus-swarm/src/adapters/scout.rs` | none | FAIL | Registered as a stub only; cannot execute a successful task. |
| `watchdog` / `WatchdogStub` | `crates/nexus-swarm/src/adapters/watchdog.rs` | none | FAIL | Registered as a stub only; cannot execute a successful task. |
| `prospector` / `ProspectorStub` | `crates/nexus-swarm/src/adapters/prospector.rs` | none | FAIL | Registered as a stub only; cannot execute a successful task. |

## Verified Positive Coverage

| Area | Evidence |
| --- | --- |
| Director plan approval | `tests/integration/tests/swarm_harness_scenario_a.rs:69-87` |
| Director plan denial | `tests/integration/tests/swarm_harness_scenario_c.rs:62-94` |
| Coordinator event sequence | `tests/integration/tests/swarm_harness_scenario_b.rs:51-167` |
| Coordinator plan-drift / audit-correlation probe | `tests/integration/tests/swarm_harness_scenario_d.rs:182-203` |
| Coordinator retry-then-success and non-retryable failure | `tests/integration/tests/swarm_harness_scenario_e.rs`; `tests/integration/tests/swarm_harness_scenario_g.rs:100-150` |
| Artisan delegation, cancellation, invalid-input mapping | `crates/nexus-swarm/tests/real_delegation_tests.rs:86-104,127-167` |
| Artisan phase/budget emission | `crates/nexus-swarm/tests/adapter_emission_tests.rs:260-299` |
| Herald delegation and default dry-run | `crates/nexus-swarm/tests/real_delegation_tests.rs:214-240,258-308` |
| Herald publish failure typing, capability denial, fuel exhaustion | `agents/social-poster/src/swarm_entry.rs:1556-1731` |
| Broker phase/observe emission | `crates/nexus-swarm/tests/adapter_emission_tests.rs:249-257,301-320` |
| Oracle timeout handling | `crates/nexus-swarm/tests/oracle_bridge_tests.rs:293-304` |
| Claude CLI excluded from autonomous swarm | `crates/nexus-swarm/src/providers/mod.rs:1-5` |

## P1 Findings

| Finding | File:Line | Detail |
| --- | --- | --- |
| Swarm-side audit hooks are absent on executed Director and coordinator paths | `tests/integration/tests/swarm_harness_scenario_a.rs:75-82`; `tests/integration/tests/swarm_harness_scenario_c.rs:80-87`; `tests/integration/tests/swarm_harness_scenario_d.rs:174-192` | Executed harness scenarios explicitly assert that Director-only and coordinator event paths emit zero swarm-side audit entries at `HEAD`. That violates the audit-log requirement for agent execution and denial handling. |
| Failed swarm runs terminate as `SwarmCompleted` instead of a distinct failure terminal | `crates/nexus-swarm/src/coordinator.rs:118-148`; `tests/integration/tests/swarm_harness_scenario_g.rs:81-90` | On `execute_loop` error, the coordinator emits `NodeFailed` and then unconditionally emits `SwarmCompleted` whenever `cancelled == false`. The executed non-retryable failure scenario confirms the broken terminal state. |

## P2 Findings

| Finding | File:Line | Detail |
| --- | --- | --- |
| `ArtisanAdapter` descriptor schema has drifted from the actual `CoderEntry` input contract | `crates/nexus-swarm/src/adapters/artisan.rs:55-62,78-94`; `agents/coder/src/swarm_entry.rs:9-23,71-81`; `crates/nexus-swarm/tests/real_delegation_tests.rs:86-94` | The descriptor advertises required `instruction`, but the real delegated entry requires `task`. Executed integration tests use `task`, not `instruction`. |
| `HeraldAdapter` descriptor schema has drifted from the actual `SocialPosterEntry` input contract | `crates/nexus-swarm/src/adapters/herald.rs:70-78,94-144`; `agents/social-poster/src/swarm_entry.rs:9-25`; `crates/nexus-swarm/tests/real_delegation_tests.rs:214-229` | The descriptor advertises `topic` and `platform`, but the real delegated entry requires `channel`, `audience`, and `message`. Executed integration tests use the latter contract. |
| `scout`, `watchdog`, and `prospector` remain registered NYI stubs rather than executable agents | `crates/nexus-swarm/src/adapters/scout.rs:1-33`; `crates/nexus-swarm/src/adapters/watchdog.rs:1-28`; `crates/nexus-swarm/src/adapters/prospector.rs:1-28` | All three roles exist only as stub descriptors and immediately return `RegistryMiss` on execution, so no successful representative task can be run for them. |

## Coverage Gaps

Marking these as `UNVERIFIED`, not as findings, because the required execution did not occur in this session.

| Check | Status | Reason |
| --- | --- | --- |
| Provider-timeout injection for `artisan`, `herald`, and `broker` adapter paths | `UNVERIFIED` | Oracle timeout handling was executed, but no existing harness in this session drove provider timeout through the three adapter `run_with_context` paths. |
| Memory-write ACL by autonomy level on registered swarm agent paths | `UNVERIFIED` | Executed swarm harnesses exercised delegation, publish-state updates, and event emission, but did not run a dedicated autonomy/ACL assertion over agent memory writes. |
| Epistemic-class assignment on swarm outputs | `UNVERIFIED` | No executed harness or cited runtime structure in this session exposed an epistemic-class field for swarm node results. |
| Capability-denial and fuel-exhaustion injection for `artisan` and `broker` | `UNVERIFIED` | Herald paths were exercised (`agents/social-poster/src/swarm_entry.rs:1693-1731`; scenario `G`), but equivalent executed harness coverage was not found for `artisan` or `broker`. |

## Additional Notes

- `herald` failure-injection coverage is materially stronger than the other real adapters in this phase because `social-poster-agent` has explicit tests for capability denial, fuel exhaustion, credentials missing, rate limiting, auth failure, and generic 5xx publish outcomes.
- `broker` is intentionally an LLM-only coordination adapter, not a bridge to a separate agent crate, per `docs/architecture/decisions/0001_broker_fate.md`.
- The autonomous swarm explicitly excludes Claude CLI: `crates/nexus-swarm/src/providers/mod.rs:1-5`.

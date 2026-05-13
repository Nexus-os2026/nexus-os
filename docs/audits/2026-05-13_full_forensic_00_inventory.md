# 2026-05-13 Full Forensic Audit: Phase 0 Inventory

## Scope

Inventory captured at `HEAD` on `2026-05-13` from the local workspace at `/home/nexus/NEXUS/nexus-os`.

## Headline Counts

| Surface | Actual | Expectation | Delta | Status |
| --- | ---: | ---: | ---: | --- |
| Workspace members (`Cargo.toml`) | 70 | ~70 | 0.0% | OK |
| Routed frontend page IDs (`app/src/App.tsx`) | 89 | ~87 | +2.3% | OK |
| Page modules (`app/src/pages/*.tsx`) | 90 | ~87 | +3.4% | OK |
| Tauri command registrations (`generate_handler!`) | 804 | ~675 | +19.1% | FLAG |
| Tauri command attributes (`#[tauri::command]` / `#[command]`) | 804 total surface split below | ~675 | +19.1% | FLAG |

## Deviation Flags

- `FLAG`: Tauri command registrations are materially above expectation: `804` registered commands versus the expected `~675` (`+129`, `+19.1%`).
- Routed pages and page modules are above expectation but within the `10%` tolerance window.

## Workspace Members

Total: `70`

```text
agents/coder
agents/designer
agents/coding-agent
agents/screen-poster
agents/self-improve
agents/social-poster
agents/web-builder
agents/workflow-studio
kernel
connectors/core
connectors/web
connectors/social
connectors/messaging
connectors/llm
workflows
cli
research
content
analytics
adaptation
control
factory
marketplace
self-update
tests/integration
app/src-tauri
benchmarks
distributed
sdk
enterprise
cloud
agents/conductor
protocols
packaging/airgap
persistence
auth
telemetry
tenancy
integrations
metering
llama-bridge
crates/nexus-flash-infer
benchmarks/conductor-bench
crates/nexus-capability-measurement
crates/nexus-governance-oracle
crates/nexus-governance-engine
crates/nexus-governance-evolution
crates/nexus-predictive-router
crates/nexus-browser-agent
crates/nexus-token-economy
crates/nexus-computer-control
crates/nexus-world-simulation
crates/nexus-perception
crates/nexus-agent-memory
crates/nexus-external-tools
crates/nexus-collab-protocol
crates/nexus-crypto
crates/nexus-software-factory
crates/nexus-mcp
crates/nexus-server
crates/nexus-a2a
crates/nexus-memory
crates/nexus-migrate
crates/nexus-outcome-eval
crates/nexus-self-improve
crates/nexus-computer-use
crates/nexus-ui-repair
crates/nexus-swarm
crates/nexus-swarm-core
nexus-code
```

## Frontend Pages

### Routed Page IDs

Total: `89`

```text
dashboard
chat
agents
audit
swarm-audit
workflows
marketplace
settings
command-center
audit-timeline
marketplace-browser
developer-portal
compliance
cluster
trust
distributed-audit
permissions
protocols
identity
firewall
browser
computer-control
code-editor
terminal
file-manager
system-monitor
notes
project-manager
database
api-client
design-studio
email-client
messaging
media-studio
app-store
ai-chat-hub
deploy-pipeline
learning-center
policy-management
documents
model-hub
time-machine
voice-assistant
approvals
simulation
mission-control
dna-lab
timeline-viewer
knowledge-graph
immune-dashboard
consciousness
dreams
temporal
civilization
self-rewrite
admin-console
admin-users
admin-fleet
admin-policies
admin-compliance
admin-health
integrations
login
workspaces
telemetry
usage-billing
scheduler
flash-inference
measurement
measurement-session
measurement-compare
measurement-batteries
capability-boundaries
model-routing
ab-validation
browser-agent
governance-oracle
token-economy
governed-control
world-sim
perception
agent-memory
external-tools
collab-protocol
software-factory
nexus-builder
memory-dashboard
self-improvement
nexus-code
```

### Page Modules on Disk

Total: `90` under `app/src/pages/*.tsx`.

Observations:

- `89` routed page IDs are exposed through `App.tsx`.
- `SetupWizard.tsx` is mounted as a modal overlay, not a route.
- `commandCenterUi.tsx` is a shared page-style helper module, not a page route.

## Swarm / Registered Agents

Observed swarm runtime roles at `HEAD`:

| Role | Kind | Source |
| --- | --- | --- |
| `SwarmDirector` | planner/orchestrator | `crates/nexus-swarm/src/director.rs` |
| `SwarmCoordinator` | DAG executor | `crates/nexus-swarm/src/coordinator.rs` |
| `artisan` / `ArtisanAdapter` | registered capability | `crates/nexus-swarm/src/adapters/artisan.rs` |
| `herald` / `HeraldAdapter` | registered capability | `crates/nexus-swarm/src/adapters/herald.rs` |
| `broker` / `BrokerAdapter` | registered capability | `crates/nexus-swarm/src/adapters/broker.rs` |
| `scout` / `ScoutStub` | registered stub capability | `crates/nexus-swarm/src/adapters/scout.rs` |
| `watchdog` / `WatchdogStub` | registered stub capability | `crates/nexus-swarm/src/adapters/watchdog.rs` |
| `prospector` / `ProspectorStub` | registered stub capability | `crates/nexus-swarm/src/adapters/prospector.rs` |

Observed `CapabilityRegistry` registration site:

- `app/src-tauri/src/commands/swarm.rs` registers `artisan`, `herald`, `broker`, `scout`, `watchdog`, and `prospector`.

Phase-0 conclusion for the user-provided expected list:

- Confirmed: `SwarmDirector`, `Broker`, `Artisan`, `Herald`, `Scout`, `Watchdog`, `Prospector`.
- No additional registered swarm capability beyond those six registry entries was observed in the `nexus-swarm` registration path at `HEAD`.

## Tauri Command Inventory

### Registration Totals

- Registered in `generate_handler!`: `804`
- Source buckets:
  - `app/src-tauri/src/lib.rs` direct registrations: `596`
  - `app/src-tauri/src/commands/orchestration.rs`: `11`
  - `app/src-tauri/src/commands/oracle_runtime.rs`: `1`
  - `app/src-tauri/src/commands/flash.rs`: `24`
  - `app/src-tauri/src/commands/swarm.rs`: `9`
  - `app/src-tauri/src/nx_bridge/commands.rs`: `18`
  - `app/src-tauri/src/commands/crate_bridges.rs`: `145`

### Command Count by Crate / Ownership Bucket

| Ownership bucket | Command count |
| --- | ---: |
| `app/src-tauri` local facade surface (`lib.rs` direct + orchestration + oracle_runtime) | 608 |
| `nexus-code` via `nx_bridge` | 18 |
| `nexus-flash-infer` via `commands/flash.rs` | 24 |
| `nexus-swarm` via `commands/swarm.rs` | 9 |
| Crate bridge surface total | 145 |

### Crate Bridge Breakdown

| Bridged crate | Command count |
| --- | ---: |
| `crates/nexus-memory` | 25 |
| `crates/nexus-capability-measurement` | 20 |
| `crates/nexus-collab-protocol` | 12 |
| `crates/nexus-token-economy` | 11 |
| `crates/nexus-software-factory` | 10 |
| `crates/nexus-perception` | 9 |
| `crates/nexus-browser-agent` | 8 |
| `crates/nexus-external-tools` | 7 |
| `crates/nexus-mcp` | 7 |
| `crates/nexus-world-simulation` | 7 |
| `crates/nexus-a2a` | 6 |
| `crates/nexus-predictive-router` | 6 |
| `crates/nexus-computer-control` | 5 |
| `crates/nexus-migrate` | 4 |
| `crates/nexus-governance-engine` | 3 |
| `crates/nexus-governance-oracle` | 3 |
| `crates/nexus-governance-evolution` | 2 |

## Phase Sizing Plan

### Phase 1: Wiring Audit

Plan: `9` subagents over `89` routed pages.

| Batch | Page count | Pages |
| --- | ---: | --- |
| `P1-B1` | 10 | `dashboard`, `ai-chat-hub`, `agents`, `file-manager`, `model-hub`, `flash-inference`, `documents`, `scheduler`, `approvals`, `terminal` |
| `P1-B2` | 10 | `settings`, `nexus-builder`, `nexus-code`, `code-editor`, `api-client`, `database`, `developer-portal`, `deploy-pipeline`, `software-factory`, `protocols` |
| `P1-B3` | 10 | `email-client`, `voice-assistant`, `messaging`, `integrations`, `system-monitor`, `audit`, `swarm-audit`, `audit-timeline`, `trust`, `firewall` |
| `P1-B4` | 10 | `compliance`, `permissions`, `browser`, `memory-dashboard`, `dna-lab`, `measurement`, `measurement-session`, `measurement-compare`, `measurement-batteries`, `capability-boundaries` |
| `P1-B5` | 10 | `model-routing`, `ab-validation`, `browser-agent`, `governance-oracle`, `token-economy`, `governed-control`, `world-sim`, `perception`, `agent-memory`, `external-tools` |
| `P1-B6` | 10 | `collab-protocol`, `self-rewrite`, `self-improvement`, `consciousness`, `design-studio`, `media-studio`, `dreams`, `notes`, `workflows`, `time-machine` |
| `P1-B7` | 10 | `timeline-viewer`, `temporal`, `simulation`, `civilization`, `computer-control`, `login`, `workspaces`, `admin-console`, `admin-users`, `admin-fleet` |
| `P1-B8` | 10 | `admin-compliance`, `admin-policies`, `admin-health`, `usage-billing`, `telemetry`, `cluster`, `distributed-audit`, `policy-management`, `learning-center`, `app-store` |
| `P1-B9` | 9 | `knowledge-graph`, `project-manager`, `chat`, `command-center`, `mission-control`, `marketplace`, `marketplace-browser`, `immune-dashboard`, `identity` |

### Phase 2: Agent E2E Correctness

Plan: `1` main agent, sequential.

Expected agent-role audit set:

- `SwarmDirector`
- `ArtisanAdapter`
- `HeraldAdapter`
- `BrokerAdapter`
- `ScoutStub`
- `WatchdogStub`
- `ProspectorStub`

Sequential execution is required because the harness and injected failure checks need shared interpretation and consistent audit semantics.

### Phase 3: User Journey Audit

Plan: `1` main agent, interactive or code-walk fallback.

- Preferred execution path: attach to an already-running dev instance or run `cargo tauri dev` in a clean profile.
- Fallback path: code-walk plus any manual runtime checks possible in-session, with explicit `UNVERIFIED` marking where execution is not achieved.

### Phase 4: Developer Correctness

Plan: `7` subagents over `70` workspace members, `10` crates each.

| Cluster | Domain | Crates |
| --- | --- | --- |
| `P4-C1` | governance | `auth`, `control`, `enterprise`, `tenancy`, `distributed`, `protocols`, `crates/nexus-governance-oracle`, `crates/nexus-governance-engine`, `crates/nexus-governance-evolution`, `crates/nexus-crypto` |
| `P4-C2` | memory/data | `persistence`, `analytics`, `content`, `research`, `telemetry`, `metering`, `crates/nexus-agent-memory`, `crates/nexus-memory`, `crates/nexus-migrate`, `sdk` |
| `P4-C3` | agent-runtime | `kernel`, `agents/coder`, `agents/designer`, `agents/coding-agent`, `agents/screen-poster`, `agents/self-improve`, `agents/social-poster`, `agents/web-builder`, `agents/workflow-studio`, `nexus-code` |
| `P4-C4` | swarm/collab | `agents/conductor`, `workflows`, `integrations`, `tests/integration`, `crates/nexus-swarm`, `crates/nexus-swarm-core`, `crates/nexus-collab-protocol`, `crates/nexus-a2a`, `crates/nexus-outcome-eval`, `cloud` |
| `P4-C5` | providers/inference | `connectors/core`, `connectors/web`, `connectors/social`, `connectors/messaging`, `connectors/llm`, `llama-bridge`, `crates/nexus-flash-infer`, `crates/nexus-predictive-router`, `crates/nexus-browser-agent`, `crates/nexus-computer-use` |
| `P4-C6` | ui/app infra | `app/src-tauri`, `cli`, `marketplace`, `self-update`, `packaging/airgap`, `benchmarks`, `benchmarks/conductor-bench`, `crates/nexus-mcp`, `crates/nexus-server`, `crates/nexus-ui-repair` |
| `P4-C7` | system/runtime extensions | `adaptation`, `factory`, `crates/nexus-token-economy`, `crates/nexus-computer-control`, `crates/nexus-world-simulation`, `crates/nexus-perception`, `crates/nexus-external-tools`, `crates/nexus-software-factory`, `crates/nexus-self-improve`, `crates/nexus-capability-measurement` |

### Phase 5: Threat Model Regression

Plan: `1` main agent, sequential.

Checks to re-run:

- OWASP Agentic Top 10 suite
- egress allowlist coverage
- Tauri capability-check regression audit
- PQC abstraction boundary scan
- fuel metering coverage audit
- secret-leak scan

## Immediate Next Action

Phase 0 is complete enough to begin Phase 1 fan-out.

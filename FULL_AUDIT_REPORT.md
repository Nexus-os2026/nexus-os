# NEXUS OS COMPLETE AUDIT REPORT
Generated: 2026-03-27T13:58:44+00:00
Commit: 883853de2d94c1d5b52b4271cd75655a234490b2
Workspace root: /home/nexus/NEXUS/nexus-os

All file paths below are repo-relative to the workspace root unless marked otherwise.

## ═══ SUMMARY ═══
- Total crates: 58
- Crates compiling: 58
- Crates with clippy failures: 1
- Crates with test failures: 1
- Crates with zero tests: 4
- Total Tauri commands: 619
- Commands with todo!/unimplemented!: 0
- Commands missing generate_handler![] registration: 0
- Commands missing backend.ts binding: 0
- Commands with no frontend caller (app/src direct invoke scan): 0
- backend.ts exports never called from pages: 147
- Total frontend page/helper modules under app/src/pages: 84
- Routed page modules: 83
- Pages with strict mock/demo indicators: 10
- Pages with no backend wrapper usage: 2
- Confirmed buttons with no handler: 2
- Dead/unused public functions (heuristic): 54
- Orphan modules (heuristic): 6

Exact cargo results are authoritative. Dead-code, placeholder, and loading-state sections are heuristic and called out as such.

## ═══ CRITICAL FINDINGS ═══
| Severity | Location | Finding | Evidence |
| --- | --- | --- | --- |
| CRITICAL | app/src/App.tsx:600 | The shell falls back to demo/mock agents, audit data, and chat whenever the desktop runtime is absent. | Demo data is defined at app/src/App.tsx:386 and demo chat responses at app/src/App.tsx:469; the fallback path is executed at app/src/App.tsx:600-612 and again on backend failure at app/src/App.tsx:664-670. |
| CRITICAL | app/src-tauri/src/main.rs:22130 | Governance Oracle backend commands are synthetic and not wired to real oracle/engine logic. | oracle_status returns supervisor health and a constant 200ms ceiling (app/src-tauri/src/main.rs:22130-22139); oracle_verify_token treats any non-empty string as valid (app/src-tauri/src/main.rs:22143-22159); oracle_get_agent_budget only mirrors remaining fuel from supervisor state (app/src-tauri/src/main.rs:22163-22175). The page still renders hardcoded security claims at app/src/pages/GovernanceOracle.tsx:91-97. |

## ═══ MAJOR FINDINGS ═══
| Severity | Location | Finding | Evidence |
| --- | --- | --- | --- |
| MAJOR | app/src-tauri/Cargo.toml:38 | nexus-governance-engine is linked as a desktop dependency but has no app wiring. | Dependency declared at app/src-tauri/Cargo.toml:38; repo search found no references in app/src-tauri/src/main.rs, app/src/api/backend.ts, or app/src/pages. |
| MAJOR | app/src-tauri/Cargo.toml:39 | nexus-governance-evolution is linked as a desktop dependency but has no app wiring. | Dependency declared at app/src-tauri/Cargo.toml:39; repo search found no references in app/src-tauri/src/main.rs, app/src/api/backend.ts, or app/src/pages. |
| MAJOR | app/src/pages/Collaboration.tsx:14 | Voting is only partially wired in Collaboration: the page can cast votes, but it never exposes a way to call a vote. | collabCallVote is imported and unused at app/src/pages/Collaboration.tsx:14; MSG_TYPES omits CallVote at app/src/pages/Collaboration.tsx:73-76; the UI only shows voting buttons when the session is already in Voting state at app/src/pages/Collaboration.tsx:305-320. |
| MAJOR | app/src/pages/WorldSimulation2.tsx:118 | Risk retrieval on the new World Simulation page is race-prone and can be skipped on first run. | The handler sets runningId in one promise callback and then reads the stale state variable in the next callback instead of the fresh id (app/src/pages/WorldSimulation2.tsx:118-119). |
| MAJOR | app/src/pages/AiChatHub.tsx:764 | The inline code-block Run button is rendered but has no event binding. | Only occurrences of `ch-code-run` in app/src are the injected HTML at app/src/pages/AiChatHub.tsx:764 and CSS selectors in app/src/pages/ai-chat-hub.css:557-567. |
| MAJOR | app/src/pages/ComplianceDashboard.tsx:852 | The Run Retention Enforcement control is a dead button. | The button is rendered without onClick/onSubmit at app/src/pages/ComplianceDashboard.tsx:852-854. |
| MAJOR | app/src/App.tsx:184 | The nav exposes two different pages with the same label World Simulation, backed by different implementations. | AGENT LAB uses `world-sim` at app/src/App.tsx:184 and routes to WorldSimulation2Page at app/src/App.tsx:1758-1759; SIMULATION uses `simulation` at app/src/App.tsx:210 and routes to WorldSimulation at app/src/App.tsx:1644-1645. |
| MAJOR | app/src/pages/ComputerControl.tsx:281 | Computer Control intentionally ships a visible demo-mode path with canned action playback instead of backend execution. | The page exposes a `Demo Mode` toggle at app/src/pages/ComputerControl.tsx:281, labels it `Safe preview — no real actions taken` at app/src/pages/ComputerControl.tsx:283, and drives `Run Demo` from a hardcoded `DEMO_ACTIONS` script defined at app/src/pages/ComputerControl.tsx:202-210 and rendered at app/src/pages/ComputerControl.tsx:291-305. |
| MAJOR | crates/nexus-computer-control/src/engine.rs:98 | Governed computer control is only partially real: non-terminal actions are still simulated. | The engine comment explicitly says execution is `simulated` at crates/nexus-computer-control/src/engine.rs:98, and all non-`TerminalCommand` actions collapse to `format!(\"Executed: {}\", action.label())` at crates/nexus-computer-control/src/engine.rs:168. The Governed Control page still presents this surface as desktop automation and exposes live execution at app/src/pages/GovernedControl.tsx:219. |

## ═══ MINOR FINDINGS ═══
| Severity | Location | Finding | Evidence |
| --- | --- | --- | --- |
| MINOR | tests/integration/../../app/src-tauri/src/main.rs:1 | The workspace integration test crate fails to compile because it includes main.rs with an inner attribute in module context. | cargo test -p nexus-integration fails on `#![allow(unexpected_cfgs)]` at tests/integration/../../app/src-tauri/src/main.rs:1. |
| MINOR | benchmarks/conductor-bench/src/nim_cloud_bench.rs:673 | nexus-conductor-benchmark fails `cargo clippy -D warnings` with 51 diagnostics. | Examples include unread fields at benchmarks/conductor-bench/src/nim_cloud_bench.rs:673 and push_str single-character literal warnings across benchmarks/conductor-bench/src/cloud_models_bench.rs:901-1163. |
| MINOR | app/src/api/backend.ts:1 | 147 exported backend bindings are never referenced by any page component. | See the Unwired Backend Bindings section for the full list generated from app/src/api/backend.ts exports vs app/src/pages usage. |
| MINOR | agents/social-poster/Cargo.toml:1 | Four crates/packages have zero test markers. | Zero-test packages: agents/social-poster, connectors/social, benchmarks, benchmarks/conductor-bench. |
| MINOR | crates/nexus-software-factory/src/roles.rs:24 | Dead-code heuristics found 54 public functions with no references in crates/ or app/. | See the Dead Code section for the full heuristic list. |

## ═══ INFO ═══
| Severity | Location | Finding | Evidence |
| --- | --- | --- | --- |
| INFO | app/src/pages/commandCenterUi.tsx:1 | commandCenterUi.tsx is stored under pages/ but acts as a shared UI helper, not a routed page. | It is imported by 21 page modules and intentionally absent from routing. |
| INFO | package.json (missing) | Root-level package.json and tsconfig.json are missing, although app/package.json and app/tsconfig.json exist. | The root config completeness check passes for .gitlab-ci.yml and Cargo.toml but fails for package.json and tsconfig.json at repo root. |
| INFO | cargo +nightly udeps | Dependency dead-code scan could not run because cargo-udeps is not installed. | Command output: `udeps not installed — skip`. |

## ═══ PER-CRATE STATUS ═══
| Crate | Manifest | cargo check | cargo clippy -D warnings | cargo test | Static test markers |
| --- | --- | --- | --- | --- | --- |
| coder-agent | agents/coder/Cargo.toml | PASS | PASS | PASS | 45 |
| nexus-connectors-llm | connectors/llm/Cargo.toml | PASS | PASS | PASS | 345 |
| nexus-flash-infer | crates/nexus-flash-infer/Cargo.toml | PASS | PASS | PASS | 76 |
| nexus-llama-bridge | llama-bridge/Cargo.toml | PASS | PASS | PASS | 31 |
| nexus-kernel | kernel/Cargo.toml | PASS | PASS | PASS | 2021 |
| nexus-persistence | persistence/Cargo.toml | PASS | PASS | PASS | 58 |
| nexus-sdk | sdk/Cargo.toml | PASS | PASS | PASS | 217 |
| designer-agent | agents/designer/Cargo.toml | PASS | PASS | PASS | 3 |
| coding-agent | agents/coding-agent/Cargo.toml | PASS | PASS | PASS | 5 |
| screen-poster-agent | agents/screen-poster/Cargo.toml | PASS | PASS | PASS | 6 |
| self-improve-agent | agents/self-improve/Cargo.toml | PASS | PASS | PASS | 7 |
| social-poster-agent | agents/social-poster/Cargo.toml | PASS | PASS | PASS | 0 |
| nexus-connectors-web | connectors/web/Cargo.toml | PASS | PASS | PASS | 8 |
| nexus-connectors-core | connectors/core/Cargo.toml | PASS | PASS | PASS | 8 |
| nexus-content | content/Cargo.toml | PASS | PASS | PASS | 3 |
| web-builder-agent | agents/web-builder/Cargo.toml | PASS | PASS | PASS | 12 |
| workflow-studio-agent | agents/workflow-studio/Cargo.toml | PASS | PASS | PASS | 4 |
| nexus-connectors-social | connectors/social/Cargo.toml | PASS | PASS | PASS | 0 |
| nexus-connectors-messaging | connectors/messaging/Cargo.toml | PASS | PASS | PASS | 46 |
| nexus-workflows | workflows/Cargo.toml | PASS | PASS | PASS | 5 |
| nexus-research | research/Cargo.toml | PASS | PASS | PASS | 12 |
| nexus-cli | cli/Cargo.toml | PASS | PASS | PASS | 114 |
| nexus-conductor | agents/conductor/Cargo.toml | PASS | PASS | PASS | 28 |
| nexus-collaboration | agents/collaboration/Cargo.toml | PASS | PASS | PASS | 22 |
| nexus-factory | factory/Cargo.toml | PASS | PASS | PASS | 30 |
| nexus-marketplace | marketplace/Cargo.toml | PASS | PASS | PASS | 84 |
| nexus-adaptation | adaptation/Cargo.toml | PASS | PASS | PASS | 23 |
| nexus-analytics | analytics/Cargo.toml | PASS | PASS | PASS | 5 |
| nexus-control | control/Cargo.toml | PASS | PASS | PASS | 15 |
| nexus-self-update | self-update/Cargo.toml | PASS | PASS | PASS | 10 |
| nexus-integration | tests/integration/Cargo.toml | PASS | PASS | FAIL | 19 |
| nexus-agent-memory | crates/nexus-agent-memory/Cargo.toml | PASS | PASS | PASS | 21 |
| nexus-airgap | packaging/airgap/Cargo.toml | PASS | PASS | PASS | 15 |
| nexus-auth | auth/Cargo.toml | PASS | PASS | PASS | 32 |
| nexus-browser-agent | crates/nexus-browser-agent/Cargo.toml | PASS | PASS | PASS | 12 |
| nexus-capability-measurement | crates/nexus-capability-measurement/Cargo.toml | PASS | PASS | PASS | 76 |
| nexus-cloud | cloud/Cargo.toml | PASS | PASS | PASS | 22 |
| nexus-collab-protocol | crates/nexus-collab-protocol/Cargo.toml | PASS | PASS | PASS | 18 |
| nexus-computer-control | crates/nexus-computer-control/Cargo.toml | PASS | PASS | PASS | 16 |
| nexus-distributed | distributed/Cargo.toml | PASS | PASS | PASS | 179 |
| nexus-enterprise | enterprise/Cargo.toml | PASS | PASS | PASS | 21 |
| nexus-external-tools | crates/nexus-external-tools/Cargo.toml | PASS | PASS | PASS | 17 |
| nexus-governance-engine | crates/nexus-governance-engine/Cargo.toml | PASS | PASS | PASS | 9 |
| nexus-governance-oracle | crates/nexus-governance-oracle/Cargo.toml | PASS | PASS | PASS | 12 |
| nexus-governance-evolution | crates/nexus-governance-evolution/Cargo.toml | PASS | PASS | PASS | 7 |
| nexus-integrations | integrations/Cargo.toml | PASS | PASS | PASS | 42 |
| nexus-metering | metering/Cargo.toml | PASS | PASS | PASS | 18 |
| nexus-perception | crates/nexus-perception/Cargo.toml | PASS | PASS | PASS | 19 |
| nexus-predictive-router | crates/nexus-predictive-router/Cargo.toml | PASS | PASS | PASS | 14 |
| nexus-protocols | protocols/Cargo.toml | PASS | PASS | PASS | 92 |
| nexus-software-factory | crates/nexus-software-factory/Cargo.toml | PASS | PASS | PASS | 18 |
| nexus-telemetry | telemetry/Cargo.toml | PASS | PASS | PASS | 21 |
| nexus-tenancy | tenancy/Cargo.toml | PASS | PASS | PASS | 50 |
| nexus-token-economy | crates/nexus-token-economy/Cargo.toml | PASS | PASS | PASS | 29 |
| nexus-world-simulation | crates/nexus-world-simulation/Cargo.toml | PASS | PASS | PASS | 18 |
| nexus-desktop-backend | app/src-tauri/Cargo.toml | PASS | PASS | PASS | 90 |
| nexus-benchmarks | benchmarks/Cargo.toml | PASS | PASS | PASS | 0 |
| nexus-conductor-benchmark | benchmarks/conductor-bench/Cargo.toml | PASS | FAIL (51) | PASS | 0 |

### Clippy Failures
| Crate | Location | Diagnostic |
| --- | --- | --- |
| nexus-conductor-benchmark | benchmarks/conductor-bench/src/nim_cloud_bench.rs:673:5 | fields `p99`, `mean`, and `total` are never read |
| nexus-conductor-benchmark | benchmarks/conductor-bench/src/nim_cloud_bench.rs:717:5 | fields `det_runs`, `conc_agents`, `avg_tokens_in`, and `avg_tokens_out` are never read |
| nexus-conductor-benchmark | benchmarks/conductor-bench/src/nim_cloud_bench.rs:744:5 | field `tokens` is never read |
| nexus-conductor-benchmark | benchmarks/conductor-bench/src/local_vs_cloud_battle.rs:327:5 | fields `mean_ms`, `min_ms`, `max_ms`, and `total` are never read |
| nexus-conductor-benchmark | benchmarks/conductor-bench/src/local_vs_cloud_battle.rs:596:9 | field `agentic_valid` is never read |
| nexus-conductor-benchmark | benchmarks/conductor-bench/src/local_vs_cloud_battle.rs:445:20 | manual implementation of `.is_multiple_of()`: help: replace with: `c.is_multiple_of(50)` |
| nexus-conductor-benchmark | benchmarks/conductor-bench/src/nim_cloud_bench.rs:999:9 | calling `push_str()` using a single-character string literal: help: consider using `push` with a character literal: `r.push('\n')` |
| nexus-conductor-benchmark | benchmarks/conductor-bench/src/local_vs_cloud_battle.rs:501:17 | this expression creates a reference which is immediately dereferenced by the compiler |
| nexus-conductor-benchmark | benchmarks/conductor-bench/src/nim_cloud_bench.rs:1024:9 | calling `push_str()` using a single-character string literal: help: consider using `push` with a character literal: `r.push('\n')` |
| nexus-conductor-benchmark | benchmarks/conductor-bench/src/nim_cloud_bench.rs:1057:9 | calling `push_str()` using a single-character string literal: help: consider using `push` with a character literal: `r.push('\n')` |
| nexus-conductor-benchmark | benchmarks/conductor-bench/src/nim_cloud_bench.rs:1081:9 | calling `push_str()` using a single-character string literal: help: consider using `push` with a character literal: `r.push('\n')` |
| nexus-conductor-benchmark | benchmarks/conductor-bench/src/nim_cloud_bench.rs:1143:9 | calling `push_str()` using a single-character string literal: help: consider using `push` with a character literal: `r.push('\n')` |
| nexus-conductor-benchmark | benchmarks/conductor-bench/src/nim_cloud_bench.rs:1169:5 | calling `push_str()` using a single-character string literal: help: consider using `push` with a character literal: `r.push('\n')` |
| nexus-conductor-benchmark | benchmarks/conductor-bench/src/nim_cloud_bench.rs:1196:9 | calling `push_str()` using a single-character string literal: help: consider using `push` with a character literal: `r.push('\n')` |
| nexus-conductor-benchmark | benchmarks/conductor-bench/src/nim_cloud_bench.rs:1212:9 | calling `push_str()` using a single-character string literal: help: consider using `push` with a character literal: `r.push('\n')` |
| nexus-conductor-benchmark | benchmarks/conductor-bench/src/local_vs_cloud_battle.rs:738:5 | calling `push_str()` using a single-character string literal: help: consider using `push` with a character literal: `r.push('\n')` |
| nexus-conductor-benchmark | benchmarks/conductor-bench/src/nim_cloud_bench.rs:1231:5 | calling `push_str()` using a single-character string literal: help: consider using `push` with a character literal: `r.push('\n')` |
| nexus-conductor-benchmark | benchmarks/conductor-bench/src/local_vs_cloud_battle.rs:880:13 | calling `push_str()` using a single-character string literal: help: consider using `push` with a character literal: `r.push('\n')` |
| nexus-conductor-benchmark | benchmarks/conductor-bench/src/local_vs_cloud_battle.rs:912:13 | calling `push_str()` using a single-character string literal: help: consider using `push` with a character literal: `r.push('\n')` |
| nexus-conductor-benchmark | benchmarks/conductor-bench/src/local_vs_cloud_battle.rs:954:13 | calling `push_str()` using a single-character string literal: help: consider using `push` with a character literal: `r.push('\n')` |
| nexus-conductor-benchmark | benchmarks/conductor-bench/src/local_vs_cloud_battle.rs:985:13 | calling `push_str()` using a single-character string literal: help: consider using `push` with a character literal: `r.push('\n')` |
| nexus-conductor-benchmark | benchmarks/conductor-bench/src/local_vs_cloud_battle.rs:1272:17 | this `map_or` can be simplified |
| nexus-conductor-benchmark | benchmarks/conductor-bench/src/local_vs_cloud_battle.rs:1276:29 | this `map_or` can be simplified |
| nexus-conductor-benchmark | benchmarks/conductor-bench/src/inference_consistency_bench.rs:62:5 | fields `output_hash` and `token_count` are never read |
| nexus-conductor-benchmark | benchmarks/conductor-bench/src/inference_consistency_bench.rs:278:5 | fields `count`, `mean_ms`, and `errors` are never read |
| nexus-conductor-benchmark | benchmarks/conductor-bench/src/inference_consistency_bench.rs:340:5 | field `dominant_output` is never read |
| nexus-conductor-benchmark | benchmarks/conductor-bench/src/inference_consistency_bench.rs:467:5 | field `dominant_output` is never read |
| nexus-conductor-benchmark | benchmarks/conductor-bench/src/inference_consistency_bench.rs:433:20 | manual implementation of `.is_multiple_of()`: help: replace with: `c.is_multiple_of(100)` |
| nexus-conductor-benchmark | benchmarks/conductor-bench/src/inference_consistency_bench.rs:496:20 | manual implementation of `.is_multiple_of()`: help: replace with: `c.is_multiple_of(100)` |
| nexus-conductor-benchmark | benchmarks/conductor-bench/src/inference_consistency_bench.rs:561:36 | called `unwrap` on `r.error` after checking its variant with `is_some` |
| nexus-conductor-benchmark | benchmarks/conductor-bench/src/inference_consistency_bench.rs:662:12 | manual implementation of `.is_multiple_of()`: help: replace with: `request_count.is_multiple_of(50)` |
| nexus-conductor-benchmark | benchmarks/conductor-bench/src/inference_consistency_bench.rs:1131:21 | this `map_or` can be simplified |
| nexus-conductor-benchmark | benchmarks/conductor-bench/src/inference_consistency_bench.rs:1247:32 | this `map_or` can be simplified |
| nexus-conductor-benchmark | benchmarks/conductor-bench/src/cloud_models_bench.rs:220:5 | field `cost_per_token` is never read |
| nexus-conductor-benchmark | benchmarks/conductor-bench/src/cloud_models_bench.rs:226:5 | fields `output_hash` and `token_count` are never read |
| nexus-conductor-benchmark | benchmarks/conductor-bench/src/cloud_models_bench.rs:526:5 | fields `count`, `min_ms`, `max_ms`, `mean_ms`, and `errors` are never read |
| nexus-conductor-benchmark | benchmarks/conductor-bench/src/cloud_models_bench.rs:588:5 | field `dominant_output` is never read |
| nexus-conductor-benchmark | benchmarks/conductor-bench/src/cloud_models_bench.rs:9:5 | doc list item overindented: help: try using `  ` (2 spaces) |
| nexus-conductor-benchmark | benchmarks/conductor-bench/src/cloud_models_bench.rs:664:20 | manual implementation of `.is_multiple_of()`: help: replace with: `c.is_multiple_of(50)` |
| nexus-conductor-benchmark | benchmarks/conductor-bench/src/cloud_models_bench.rs:734:20 | manual implementation of `.is_multiple_of()`: help: replace with: `c.is_multiple_of(100)` |
| nexus-conductor-benchmark | benchmarks/conductor-bench/src/cloud_models_bench.rs:813:1 | this function has too many arguments (9/7) |
| nexus-conductor-benchmark | benchmarks/conductor-bench/src/cloud_models_bench.rs:901:9 | calling `push_str()` using a single-character string literal: help: consider using `push` with a character literal: `r.push('\n')` |
| nexus-conductor-benchmark | benchmarks/conductor-bench/src/cloud_models_bench.rs:928:9 | calling `push_str()` using a single-character string literal: help: consider using `push` with a character literal: `r.push('\n')` |
| nexus-conductor-benchmark | benchmarks/conductor-bench/src/cloud_models_bench.rs:978:9 | calling `push_str()` using a single-character string literal: help: consider using `push` with a character literal: `r.push('\n')` |
| nexus-conductor-benchmark | benchmarks/conductor-bench/src/cloud_models_bench.rs:1012:9 | calling `push_str()` using a single-character string literal: help: consider using `push` with a character literal: `r.push('\n')` |
| nexus-conductor-benchmark | benchmarks/conductor-bench/src/cloud_models_bench.rs:1037:9 | calling `push_str()` using a single-character string literal: help: consider using `push` with a character literal: `r.push('\n')` |
| nexus-conductor-benchmark | benchmarks/conductor-bench/src/cloud_models_bench.rs:1061:9 | calling `push_str()` using a single-character string literal: help: consider using `push` with a character literal: `r.push('\n')` |
| nexus-conductor-benchmark | benchmarks/conductor-bench/src/cloud_models_bench.rs:1084:9 | calling `push_str()` using a single-character string literal: help: consider using `push` with a character literal: `r.push('\n')` |
| nexus-conductor-benchmark | benchmarks/conductor-bench/src/cloud_models_bench.rs:1101:13 | calling `push_str()` using a single-character string literal: help: consider using `push` with a character literal: `r.push('\n')` |
| nexus-conductor-benchmark | benchmarks/conductor-bench/src/cloud_models_bench.rs:1136:9 | calling `push_str()` using a single-character string literal: help: consider using `push` with a character literal: `r.push('\n')` |
| nexus-conductor-benchmark | benchmarks/conductor-bench/src/cloud_models_bench.rs:1163:5 | calling `push_str()` using a single-character string literal: help: consider using `push` with a character literal: `r.push('\n')` |

### Test Failures
| Crate | Location | Failure |
| --- | --- | --- |
| nexus-integration | tests/integration/../../app/src-tauri/src/main.rs:1:1 | inner attribute is not permitted in this context |

### Zero-Test Packages
| Crate | Manifest |
| --- | --- |
| social-poster-agent | agents/social-poster/Cargo.toml |
| nexus-connectors-social | connectors/social/Cargo.toml |
| nexus-benchmarks | benchmarks/Cargo.toml |
| nexus-conductor-benchmark | benchmarks/conductor-bench/Cargo.toml |

## ═══ CROSS-CRATE WIRING ═══
| Crate | In workspace | In app deps | In integration tests | AppState field | AppState line | Compiles | Test markers | Notes |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| nexus-capability-measurement | yes | yes | yes | yes | 923 | PASS | 77 |  |
| nexus-governance-oracle | yes | yes | yes | no | - | PASS | 12 | types imported; page/backend use hand-rolled commands instead of crate state |
| nexus-governance-engine | yes | yes | yes | no | - | PASS | 9 | no references found in app/src-tauri/src/main.rs, app/src/api/backend.ts, or app/src/pages |
| nexus-governance-evolution | yes | yes | yes | no | - | PASS | 7 | no references found in app/src-tauri/src/main.rs, app/src/api/backend.ts, or app/src/pages |
| nexus-predictive-router | yes | yes | yes | yes | 924 | PASS | 14 |  |
| nexus-token-economy | yes | yes | yes | yes | 926 | PASS | 29 |  |
| nexus-browser-agent | yes | yes | yes | yes | 925 | PASS | 12 |  |
| nexus-computer-control | yes | yes | yes | yes | 927 | PASS | 16 |  |
| nexus-world-simulation | yes | yes | yes | yes | 928 | PASS | 18 |  |
| nexus-perception | yes | yes | yes | yes | 929 | PASS | 19 |  |
| nexus-agent-memory | yes | yes | yes | yes | 930 | PASS | 21 |  |
| nexus-external-tools | yes | yes | yes | yes | 931 | PASS | 17 |  |
| nexus-collab-protocol | yes | yes | yes | yes | 932 | PASS | 18 |  |
| nexus-software-factory | yes | yes | yes | yes | 933 | PASS | 18 |  |

## ═══ PER-PAGE STATUS ═══
| Page | File | In router | Backend wrappers used | Strict mock/demo hits | UI placeholder attrs | Buttons | Has error markers | Has loading markers |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| ABValidation | app/src/pages/ABValidation.tsx | yes | 2 | 0 | 0 | 1 | yes | yes |
| AdminCompliance | app/src/pages/AdminCompliance.tsx | yes | 2 | 0 | 0 | 3 | yes | yes |
| AdminDashboard | app/src/pages/AdminDashboard.tsx | yes | 1 | 0 | 0 | 1 | yes | yes |
| AdminFleet | app/src/pages/AdminFleet.tsx | yes | 3 | 0 | 0 | 3 | yes | yes |
| AdminPolicyEditor | app/src/pages/AdminPolicyEditor.tsx | yes | 3 | 0 | 0 | 2 | yes | yes |
| AdminSystemHealth | app/src/pages/AdminSystemHealth.tsx | yes | 6 | 0 | 0 | 3 | yes | yes |
| AdminUsers | app/src/pages/AdminUsers.tsx | yes | 4 | 0 | 1 | 4 | yes | yes |
| AgentBrowser | app/src/pages/AgentBrowser.tsx | yes | 2 | 1 | 0 | 3 | yes | yes |
| AgentDnaLab | app/src/pages/AgentDnaLab.tsx | yes | 18 | 0 | 11 | 23 | yes | yes |
| AgentMemory | app/src/pages/AgentMemory.tsx | yes | 10 | 0 | 5 | 8 | yes | yes |
| Agents | app/src/pages/Agents.tsx | yes | 11 | 0 | 2 | 18 | yes | yes |
| AiChatHub | app/src/pages/AiChatHub.tsx | yes | 20 | 1 | 5 | 42 | yes | yes |
| ApiClient | app/src/pages/ApiClient.tsx | yes | 4 | 1 | 14 | 18 | yes | yes |
| AppStore | app/src/pages/AppStore.tsx | yes | 6 | 0 | 2 | 7 | yes | yes |
| ApprovalCenter | app/src/pages/ApprovalCenter.tsx | yes | 9 | 0 | 1 | 4 | yes | yes |
| Audit | app/src/pages/Audit.tsx | yes | 15 | 1 | 10 | 21 | yes | yes |
| AuditTimeline | app/src/pages/AuditTimeline.tsx | yes | 3 | 0 | 0 | 1 | yes | no |
| BrowserAgent | app/src/pages/BrowserAgent.tsx | yes | 9 | 0 | 2 | 1 | yes | yes |
| CapabilityBoundaryMap | app/src/pages/CapabilityBoundaryMap.tsx | yes | 5 | 0 | 0 | 0 | yes | yes |
| Chat | app/src/pages/Chat.tsx | yes | 2 | 1 | 1 | 5 | yes | yes |
| Civilization | app/src/pages/Civilization.tsx | yes | 28 | 0 | 28 | 2 | yes | yes |
| ClusterStatus | app/src/pages/ClusterStatus.tsx | yes | 8 | 0 | 4 | 3 | yes | yes |
| CodeEditor | app/src/pages/CodeEditor.tsx | yes | 5 | 0 | 4 | 34 | yes | yes |
| Collaboration | app/src/pages/Collaboration.tsx | yes | 11 | 0 | 6 | 9 | yes | no |
| CommandCenter | app/src/pages/CommandCenter.tsx | yes | 7 | 0 | 0 | 3 | yes | yes |
| ComplianceDashboard | app/src/pages/ComplianceDashboard.tsx | yes | 7 | 0 | 0 | 8 | yes | yes |
| ComputerControl | app/src/pages/ComputerControl.tsx | yes | 13 | 1 | 2 | 13 | yes | yes |
| ConsciousnessMonitor | app/src/pages/ConsciousnessMonitor.tsx | yes | 6 | 0 | 0 | 1 | yes | yes |
| Dashboard | app/src/pages/Dashboard.tsx | yes | 3 | 0 | 0 | 1 | yes | yes |
| DatabaseManager | app/src/pages/DatabaseManager.tsx | yes | 5 | 0 | 4 | 14 | yes | no |
| DeployPipeline | app/src/pages/DeployPipeline.tsx | yes | 11 | 0 | 7 | 16 | yes | yes |
| DesignStudio | app/src/pages/DesignStudio.tsx | yes | 6 | 0 | 0 | 4 | yes | yes |
| DeveloperPortal | app/src/pages/DeveloperPortal.tsx | yes | 3 | 0 | 0 | 3 | yes | yes |
| DistributedAudit | app/src/pages/DistributedAudit.tsx | yes | 3 | 0 | 0 | 0 | yes | yes |
| Documents | app/src/pages/Documents.tsx | yes | 7 | 0 | 1 | 4 | yes | yes |
| DreamForge | app/src/pages/DreamForge.tsx | yes | 3 | 0 | 0 | 3 | yes | no |
| EmailClient | app/src/pages/EmailClient.tsx | yes | 9 | 0 | 4 | 16 | yes | yes |
| ExternalTools | app/src/pages/ExternalTools.tsx | yes | 7 | 0 | 2 | 4 | yes | yes |
| FileManager | app/src/pages/FileManager.tsx | yes | 7 | 0 | 2 | 23 | yes | yes |
| Firewall | app/src/pages/Firewall.tsx | yes | 3 | 0 | 0 | 1 | yes | no |
| FlashInference | app/src/pages/FlashInference.tsx | yes | 12 | 0 | 1 | 11 | yes | yes |
| GovernanceOracle | app/src/pages/GovernanceOracle.tsx | yes | 3 | 0 | 0 | 1 | yes | yes |
| GovernedControl | app/src/pages/GovernedControl.tsx | yes | 6 | 0 | 0 | 2 | yes | yes |
| Identity | app/src/pages/Identity.tsx | yes | 11 | 0 | 4 | 0 | yes | yes |
| ImmuneDashboard | app/src/pages/ImmuneDashboard.tsx | yes | 4 | 0 | 0 | 0 | yes | yes |
| Integrations | app/src/pages/Integrations.tsx | yes | 4 | 0 | 1 | 10 | yes | yes |
| KnowledgeGraph | app/src/pages/KnowledgeGraph.tsx | yes | 11 | 0 | 12 | 1 | yes | yes |
| LearningCenter | app/src/pages/LearningCenter.tsx | yes | 16 | 0 | 1 | 7 | yes | yes |
| Login | app/src/pages/Login.tsx | yes | 5 | 0 | 0 | 1 | yes | yes |
| MeasurementBatteries | app/src/pages/MeasurementBatteries.tsx | yes | 1 | 0 | 0 | 0 | yes | yes |
| MeasurementCompare | app/src/pages/MeasurementCompare.tsx | yes | 2 | 0 | 0 | 1 | yes | yes |
| MeasurementDashboard | app/src/pages/MeasurementDashboard.tsx | yes | 5 | 0 | 0 | 1 | yes | yes |
| MeasurementSession | app/src/pages/MeasurementSession.tsx | yes | 2 | 0 | 0 | 0 | yes | yes |
| MediaStudio | app/src/pages/MediaStudio.tsx | yes | 5 | 0 | 0 | 4 | yes | no |
| Messaging | app/src/pages/Messaging.tsx | yes | 8 | 0 | 3 | 4 | yes | no |
| MissionControl | app/src/pages/MissionControl.tsx | yes | 10 | 0 | 0 | 4 | yes | yes |
| ModelHub | app/src/pages/ModelHub.tsx | yes | 15 | 0 | 6 | 10 | yes | yes |
| ModelRouting | app/src/pages/ModelRouting.tsx | yes | 4 | 0 | 1 | 0 | yes | yes |
| NotesApp | app/src/pages/NotesApp.tsx | yes | 4 | 0 | 3 | 20 | yes | no |
| Perception | app/src/pages/Perception.tsx | yes | 9 | 0 | 4 | 2 | yes | yes |
| PermissionDashboard | app/src/pages/PermissionDashboard.tsx | yes | 7 | 0 | 0 | 10 | yes | yes |
| PolicyManagement | app/src/pages/PolicyManagement.tsx | yes | 4 | 1 | 0 | 4 | yes | yes |
| ProjectManager | app/src/pages/ProjectManager.tsx | yes | 3 | 0 | 3 | 7 | yes | yes |
| Protocols | app/src/pages/Protocols.tsx | yes | 17 | 0 | 10 | 9 | yes | yes |
| Scheduler | app/src/pages/Scheduler.tsx | yes | 6 | 0 | 2 | 8 | yes | yes |
| SelfRewriteLab | app/src/pages/SelfRewriteLab.tsx | yes | 4 | 0 | 0 | 0 | yes | no |
| Settings | app/src/pages/Settings.tsx | yes | 6 | 2 | 4 | 23 | yes | yes |
| SetupWizard | app/src/pages/SetupWizard.tsx | yes | 0 | 1 | 0 | 16 | yes | yes |
| SoftwareFactory | app/src/pages/SoftwareFactory.tsx | yes | 9 | 0 | 4 | 3 | yes | no |
| SystemMonitor | app/src/pages/SystemMonitor.tsx | yes | 2 | 0 | 0 | 2 | yes | no |
| Telemetry | app/src/pages/Telemetry.tsx | yes | 4 | 0 | 2 | 5 | yes | yes |
| TemporalEngine | app/src/pages/TemporalEngine.tsx | yes | 2 | 0 | 3 | 6 | yes | no |
| Terminal | app/src/pages/Terminal.tsx | yes | 2 | 0 | 1 | 11 | yes | no |
| TimeMachine | app/src/pages/TimeMachine.tsx | yes | 15 | 0 | 2 | 13 | yes | yes |
| TimelineViewer | app/src/pages/TimelineViewer.tsx | yes | 2 | 0 | 0 | 2 | yes | no |
| TokenEconomy | app/src/pages/TokenEconomy.tsx | yes | 6 | 0 | 1 | 3 | yes | yes |
| TrustDashboard | app/src/pages/TrustDashboard.tsx | yes | 9 | 0 | 8 | 9 | yes | yes |
| UsageBilling | app/src/pages/UsageBilling.tsx | yes | 5 | 0 | 1 | 3 | yes | yes |
| VoiceAssistant | app/src/pages/VoiceAssistant.tsx | yes | 7 | 0 | 0 | 5 | yes | yes |
| Workflows | app/src/pages/Workflows.tsx | yes | 5 | 0 | 0 | 8 | yes | yes |
| Workspaces | app/src/pages/Workspaces.tsx | yes | 7 | 0 | 2 | 13 | yes | yes |
| WorldSimulation | app/src/pages/WorldSimulation.tsx | yes | 10 | 1 | 5 | 10 | yes | no |
| WorldSimulation2 | app/src/pages/WorldSimulation2.tsx | yes | 6 | 0 | 2 | 2 | yes | yes |
| commandCenterUi | app/src/pages/commandCenterUi.tsx | no | 0 | 0 | 0 | 1 | no | no |

## ═══ FRONTEND PAGE → BACKEND → CRATE TRACE ═══
### MeasurementDashboard
- file: app/src/pages/MeasurementDashboard.tsx
- cmListSessions -> cm_list_sessions -> main.rs:21807 -> ) -> Result<Vec<nexus_capability_measurement::MeasurementSession>, String> {
- cmGetBatteries -> cm_get_batteries -> main.rs:21849 -> ) -> Result<Vec<nexus_capability_measurement::tauri_commands::BatterySummary>, String> {
- cmGetScorecard -> cm_get_scorecard -> main.rs:21796 -> ) -> Result<nexus_capability_measurement::AgentScorecard, String> {
- cmStartSession -> UNRESOLVED -> main.rs:- -> no crate ref captured in command body window
- listAgents -> list_agents -> main.rs:22967 -> ) -> Result<Vec<nexus_kernel::cognitive::ScheduledAgent>, String> {

### MeasurementSession
- file: app/src/pages/MeasurementSession.tsx
- cmGetSession -> cm_get_session -> main.rs:21785 -> ) -> Result<nexus_capability_measurement::MeasurementSession, String> {
- cmListSessions -> cm_list_sessions -> main.rs:21807 -> ) -> Result<Vec<nexus_capability_measurement::MeasurementSession>, String> {

### MeasurementCompare
- file: app/src/pages/MeasurementCompare.tsx
- cmCompareAgents -> cm_compare_agents -> main.rs:21838 -> ) -> Result<Vec<nexus_capability_measurement::AgentScorecard>, String> {
- cmListSessions -> cm_list_sessions -> main.rs:21807 -> ) -> Result<Vec<nexus_capability_measurement::MeasurementSession>, String> {

### MeasurementBatteries
- file: app/src/pages/MeasurementBatteries.tsx
- cmGetBatteries -> cm_get_batteries -> main.rs:21849 -> ) -> Result<Vec<nexus_capability_measurement::tauri_commands::BatterySummary>, String> {

### CapabilityBoundaryMap
- file: app/src/pages/CapabilityBoundaryMap.tsx
- cmGetBoundaryMap -> cm_get_boundary_map -> main.rs:21871 -> ) -> Result<Vec<nexus_capability_measurement::evaluation::batch::AgentBoundary>, String> {
- cmGetCalibration -> cm_get_calibration -> main.rs:21878 -> ) -> Result<nexus_capability_measurement::evaluation::batch::CalibrationReport, String> {
- cmGetCensus -> cm_get_census -> main.rs:21887 -> ) -> Result<nexus_capability_measurement::evaluation::batch::ClassificationCensus, String> {
- cmGetGamingReportBatch -> cm_get_gaming_report_batch -> main.rs:21896 -> ) -> Result<nexus_capability_measurement::evaluation::batch::GamingReport, String> {
- cmUploadDarwin -> cm_upload_darwin -> main.rs:21905 -> ) -> Result<nexus_capability_measurement::DarwinUploadSummary, String> {

### ABValidation
- file: app/src/pages/ABValidation.tsx
- cmRunAbValidation -> cm_run_ab_validation -> main.rs:21950 -> ) -> Result<nexus_capability_measurement::ABComparisonResult, String> {
- listAgents -> list_agents -> main.rs:22967 -> ) -> Result<Vec<nexus_kernel::cognitive::ScheduledAgent>, String> {

### ModelRouting
- file: app/src/pages/ModelRouting.tsx
- routerGetAccuracy -> router_get_accuracy -> main.rs:22018 -> ) -> Result<nexus_predictive_router::RoutingAccuracy, String> {
- routerGetModels -> router_get_models -> main.rs:22025 -> ) -> Result<Vec<nexus_predictive_router::ModelCapabilityProfile>, String> {
- routerGetFeedback -> router_get_feedback -> main.rs:22043 -> ) -> Result<nexus_predictive_router::feedback::FeedbackAnalysis, String> {
- routerEstimateDifficulty -> router_estimate_difficulty -> main.rs:22032 -> ) -> Result<nexus_predictive_router::TaskDifficultyEstimate, String> {

### GovernanceOracle
- file: app/src/pages/GovernanceOracle.tsx
- oracleStatus -> oracle_status -> main.rs:22130 -> ) -> Result<nexus_governance_oracle::tauri_commands::TokenVerification, String> {
- oracleGetAgentBudget -> oracle_get_agent_budget -> main.rs:22163 -> ) -> Result<nexus_capability_measurement::evaluation::comparator::SingleEvaluationResult, String> {
- listAgents -> list_agents -> main.rs:22967 -> ) -> Result<Vec<nexus_kernel::cognitive::ScheduledAgent>, String> {

### TokenEconomy
- file: app/src/pages/TokenEconomy.tsx
- tokenGetAllWallets -> token_get_all_wallets -> main.rs:22205 -> ) -> Result<Vec<token_cmds::WalletSummary>, String> {
- tokenGetLedger -> token_get_ledger -> main.rs:22227 -> ) -> Result<Vec<token_cmds::LedgerEntrySummary>, String> {
- tokenGetSupply -> token_get_supply -> main.rs:22240 -> ) -> Result<token_cmds::SupplySummary, String> {
- tokenGetPricing -> token_get_pricing -> main.rs:22305 -> fn token_get_pricing(state: tauri::State<'_, AppState>) -> Vec<token_cmds::PricingSummary> {
- tokenCalculateReward -> token_calculate_reward -> main.rs:22257 -> ) -> token_cmds::RewardEstimate {
- tokenCalculateBurn -> token_calculate_burn -> main.rs:22247 -> ) -> token_cmds::BurnEstimate {

### BrowserAgent
- file: app/src/pages/BrowserAgent.tsx
- browserCreateSession -> browser_create_session -> main.rs:22052 -> nexus_browser_agent::tauri_commands::create_session(
- browserExecuteTask -> browser_execute_task -> main.rs:22065 -> ) -> Result<nexus_browser_agent::BrowserActionResult, String> {
- browserNavigate -> browser_navigate -> main.rs:22082 -> ) -> Result<nexus_browser_agent::BrowserActionResult, String> {
- browserGetContent -> browser_get_content -> main.rs:22100 -> ) -> Result<nexus_browser_agent::BrowserActionResult, String> {
- browserCloseSession -> browser_close_session -> main.rs:22108 -> nexus_browser_agent::tauri_commands::close_session(&state.browser_agent, &session_id)
- browserGetPolicy -> browser_get_policy -> main.rs:22116 -> ) -> Result<nexus_browser_agent::BrowserPolicy, String> {
- browserSessionCount -> browser_session_count -> main.rs:22123 -> nexus_browser_agent::tauri_commands::session_count(&state.browser_agent)
- browserScreenshot -> browser_screenshot -> main.rs:22091 -> ) -> Result<nexus_browser_agent::BrowserActionResult, String> {
- listAgents -> list_agents -> main.rs:22967 -> ) -> Result<Vec<nexus_kernel::cognitive::ScheduledAgent>, String> {

### GovernedControl
- file: app/src/pages/GovernedControl.tsx
- ccGetActionHistory -> cc_get_action_history -> main.rs:22331 -> ) -> Result<Vec<cc_cmds::ActionHistoryEntry>, String> {
- ccGetCapabilityBudget -> cc_get_capability_budget -> main.rs:22339 -> ) -> Result<cc_cmds::BudgetSummary, String> {
- ccGetScreenContext -> cc_get_screen_context -> main.rs:22355 -> ) -> Result<nexus_computer_control::ScreenContext, String> {
- ccVerifyActionSequence -> cc_verify_action_sequence -> main.rs:22347 -> ) -> Result<nexus_computer_control::VerificationResult, String> {
- ccExecuteAction -> UNRESOLVED -> main.rs:- -> no crate ref captured in command body window
- listAgents -> list_agents -> main.rs:22967 -> ) -> Result<Vec<nexus_kernel::cognitive::ScheduledAgent>, String> {

### WorldSimulation
- file: app/src/pages/WorldSimulation.tsx
- chatWithSimulationPersona -> UNRESOLVED -> main.rs:- -> no crate ref captured in command body window
- createSimulation -> UNRESOLVED -> main.rs:- -> no crate ref captured in command body window
- getSimulationReport -> get_simulation_report -> main.rs:25999 -> no crate ref captured in command body window
- getSimulationStatus -> get_simulation_status -> main.rs:25991 -> no crate ref captured in command body window
- hasDesktopRuntime -> UNRESOLVED -> main.rs:- -> no crate ref captured in command body window
- injectSimulationVariable -> UNRESOLVED -> main.rs:- -> no crate ref captured in command body window
- listSimulations -> list_simulations -> main.rs:26017 -> no crate ref captured in command body window
- pauseSimulation -> pause_simulation -> main.rs:25976 -> no crate ref captured in command body window
- runParallelSimulations -> UNRESOLVED -> main.rs:- -> no crate ref captured in command body window
- startSimulation -> start_simulation -> main.rs:25963 -> no crate ref captured in command body window

### Perception
- file: app/src/pages/Perception.tsx
- perceptionAnalyzeChart -> perception_analyze_chart -> main.rs:22501 -> ) -> Result<nexus_perception::PerceptionResult, String> {
- perceptionDescribe -> perception_describe -> main.rs:22447 -> ) -> Result<nexus_perception::PerceptionResult, String> {
- perceptionExtractData -> perception_extract_data -> main.rs:22483 -> ) -> Result<nexus_perception::PerceptionResult, String> {
- perceptionExtractText -> perception_extract_text -> main.rs:22456 -> ) -> Result<nexus_perception::PerceptionResult, String> {
- perceptionFindUiElements -> perception_find_ui_elements -> main.rs:22475 -> ) -> Result<Vec<nexus_perception::UIElement>, String> {
- perceptionGetPolicy -> perception_get_policy -> main.rs:22510 -> fn perception_get_policy(state: tauri::State<'_, AppState>) -> nexus_perception::PerceptionPolicy {
- perceptionInit -> perception_init -> main.rs:22437 -> perception_cmds::init_provider(&state.perception, &provider, &api_key, &model_id)
- perceptionQuestion -> perception_question -> main.rs:22465 -> ) -> Result<nexus_perception::PerceptionResult, String> {
- perceptionReadError -> perception_read_error -> main.rs:22493 -> ) -> Result<nexus_perception::PerceptionResult, String> {

### AgentMemory
- file: app/src/pages/AgentMemory.tsx
- listAgents -> list_agents -> main.rs:22967 -> ) -> Result<Vec<nexus_kernel::cognitive::ScheduledAgent>, String> {
- memoryBuildContext -> memory_build_context -> main.rs:22575 -> ) -> Result<nexus_agent_memory::MemoryContext, String> {
- memoryConsolidate -> memory_consolidate -> main.rs:22598 -> ) -> Result<memory_cmds::ConsolidationResult, String> {
- memoryDeleteEntry -> memory_delete_entry -> main.rs:22566 -> memory_cmds::memory_delete(&state.persistent_memory, &agent_id, &memory_id)
- memoryGetPolicy -> memory_get_policy -> main.rs:22621 -> fn memory_get_policy(state: tauri::State<'_, AppState>) -> nexus_agent_memory::MemoryPolicy {
- memoryGetStats -> memory_get_stats -> main.rs:22590 -> ) -> Result<memory_cmds::MemoryStats, String> {
- memoryLoad -> memory_load -> main.rs:22611 -> memory_cmds::memory_load(&state.persistent_memory, &agent_id)
- memoryQueryEntries -> UNRESOLVED -> main.rs:- -> no crate ref captured in command body window
- memorySave -> memory_save -> main.rs:22606 -> memory_cmds::memory_save(&state.persistent_memory, &agent_id)
- memoryStoreEntry -> UNRESOLVED -> main.rs:- -> no crate ref captured in command body window

### ExternalTools
- file: app/src/pages/ExternalTools.tsx
- toolsExecute -> UNRESOLVED -> main.rs:- -> no crate ref captured in command body window
- toolsGetAudit -> tools_get_audit -> main.rs:22666 -> ) -> Result<Vec<nexus_external_tools::ToolAuditEntry>, String> {
- toolsGetPolicy -> tools_get_policy -> main.rs:22679 -> ) -> nexus_external_tools::ToolGovernancePolicy {
- toolsGetRegistry -> tools_get_registry -> main.rs:22652 -> ) -> Result<Vec<nexus_external_tools::ExternalTool>, String> {
- toolsRefreshAvailability -> tools_refresh_availability -> main.rs:22659 -> ) -> Result<Vec<nexus_external_tools::ExternalTool>, String> {
- toolsVerifyAudit -> tools_verify_audit -> main.rs:22674 -> tools_cmds::tools_verify_audit(&state.external_tools)
- getRateLimitStatus -> get_rate_limit_status -> main.rs:27035 -> use nexus_kernel::rate_limit::RateCategory;

### Collaboration
- file: app/src/pages/Collaboration.tsx
- collabAddParticipant -> UNRESOLVED -> main.rs:- -> no crate ref captured in command body window
- collabCastVote -> UNRESOLVED -> main.rs:- -> no crate ref captured in command body window
- collabCreateSession -> UNRESOLVED -> main.rs:- -> no crate ref captured in command body window
- collabDeclareConsensus -> UNRESOLVED -> main.rs:- -> no crate ref captured in command body window
- collabDetectConsensus -> collab_detect_consensus -> main.rs:22801 -> ) -> Result<nexus_collab_protocol::ConsensusState, String> {
- collabGetPatterns -> collab_get_patterns -> main.rs:22831 -> fn collab_get_patterns() -> Vec<collab_cmds::PatternInfo> {
- collabGetPolicy -> collab_get_policy -> main.rs:22824 -> ) -> nexus_collab_protocol::CollaborationPolicy {
- collabGetSession -> collab_get_session -> main.rs:22809 -> ) -> Result<nexus_collab_protocol::CollaborationSession, String> {
- collabListActive -> collab_list_active -> main.rs:22817 -> ) -> Result<Vec<nexus_collab_protocol::CollaborationSession>, String> {
- collabSendMessage -> UNRESOLVED -> main.rs:- -> no crate ref captured in command body window
- collabStart -> collab_start -> main.rs:22724 -> collab_cmds::collab_start(&state.collab_protocol, &session_id)

### SoftwareFactory
- file: app/src/pages/SoftwareFactory.tsx
- swfAssignMember -> UNRESOLVED -> main.rs:- -> no crate ref captured in command body window
- swfCreateProject -> swf_create_project -> main.rs:22838 -> factory_cmds::factory_create_project(&state.software_factory, &title, &user_request)
- swfEstimateCost -> swf_estimate_cost -> main.rs:22915 -> factory_cmds::factory_estimate_cost()
- swfGetCost -> swf_get_cost -> main.rs:22897 -> ) -> Result<factory_cmds::CostBreakdown, String> {
- swfGetPipelineStages -> swf_get_pipeline_stages -> main.rs:22910 -> fn swf_get_pipeline_stages() -> Vec<factory_cmds::StageInfo> {
- swfGetPolicy -> swf_get_policy -> main.rs:22905 -> fn swf_get_policy(state: tauri::State<'_, AppState>) -> nexus_software_factory::FactoryPolicy {
- swfGetProject -> swf_get_project -> main.rs:22882 -> ) -> Result<nexus_software_factory::Project, String> {
- swfListProjects -> swf_list_projects -> main.rs:22890 -> ) -> Result<Vec<nexus_software_factory::Project>, String> {
- swfStartPipeline -> swf_start_pipeline -> main.rs:22868 -> factory_cmds::factory_start_pipeline(&state.software_factory, &project_id)

## ═══ UNWIRED COMMANDS ═══
- Commands present in generate_handler![] with no app/src string caller: 0
- backend.ts exports never called from any page component: 147

| Export | Location | Severity |
| --- | --- | --- |
| clearAllAgents | app/src/api/backend.ts:102 | MINOR |
| createAgent | app/src/api/backend.ts:106 | MINOR |
| getAgentPerformance | app/src/api/backend.ts:174 | MINOR |
| getAutoEvolutionLog | app/src/api/backend.ts:178 | MINOR |
| setAutoEvolutionConfig | app/src/api/backend.ts:182 | MINOR |
| forceEvolveAgent | app/src/api/backend.ts:196 | MINOR |
| transcribePushToTalk | app/src/api/backend.ts:208 | MINOR |
| startJarvisMode | app/src/api/backend.ts:212 | MINOR |
| stopJarvisMode | app/src/api/backend.ts:216 | MINOR |
| jarvisStatus | app/src/api/backend.ts:220 | MINOR |
| detectHardware | app/src/api/backend.ts:224 | MINOR |
| checkOllama | app/src/api/backend.ts:228 | MINOR |
| pullOllamaModel | app/src/api/backend.ts:232 | MINOR |
| runSetupWizard | app/src/api/backend.ts:241 | MINOR |
| pullModel | app/src/api/backend.ts:248 | MINOR |
| ensureOllama | app/src/api/backend.ts:257 | MINOR |
| isOllamaInstalled | app/src/api/backend.ts:264 | MINOR |
| deleteModel | app/src/api/backend.ts:268 | MINOR |
| isSetupComplete | app/src/api/backend.ts:277 | MINOR |
| listAvailableModels | app/src/api/backend.ts:281 | MINOR |
| analyzeScreen | app/src/api/backend.ts:311 | MINOR |
| computerControlCaptureScreen | app/src/api/backend.ts:338 | MINOR |
| computerControlExecuteAction | app/src/api/backend.ts:342 | MINOR |
| setAgentModel | app/src/api/backend.ts:386 | MINOR |
| getSystemInfo | app/src/api/backend.ts:411 | MINOR |
| getAgentIdentity | app/src/api/backend.ts:532 | MINOR |
| listIdentities | app/src/api/backend.ts:536 | MINOR |
| marketplaceInfo | app/src/api/backend.ts:583 | MINOR |
| getBrowserHistory | app/src/api/backend.ts:607 | MINOR |
| getAgentActivity | app/src/api/backend.ts:611 | MINOR |
| startResearch | app/src/api/backend.ts:617 | MINOR |
| researchAgentAction | app/src/api/backend.ts:624 | MINOR |
| completeResearch | app/src/api/backend.ts:640 | MINOR |
| getResearchSession | app/src/api/backend.ts:646 | MINOR |
| listResearchSessions | app/src/api/backend.ts:652 | MINOR |
| startBuild | app/src/api/backend.ts:658 | MINOR |
| buildAppendCode | app/src/api/backend.ts:662 | MINOR |
| buildAddMessage | app/src/api/backend.ts:674 | MINOR |
| completeBuild | app/src/api/backend.ts:688 | MINOR |
| getBuildSession | app/src/api/backend.ts:694 | MINOR |
| getBuildCode | app/src/api/backend.ts:700 | MINOR |
| getBuildPreview | app/src/api/backend.ts:704 | MINOR |
| startLearning | app/src/api/backend.ts:710 | MINOR |
| getKnowledgeBase | app/src/api/backend.ts:714 | MINOR |
| getLearningSession | app/src/api/backend.ts:718 | MINOR |
| learningAgentAction | app/src/api/backend.ts:753 | MINOR |
| getProviderUsageStats | app/src/api/backend.ts:794 | MINOR |
| emailSearchMessages | app/src/api/backend.ts:1006 | MINOR |
| getAgentOutputs | app/src/api/backend.ts:1042 | MINOR |
| projectGet | app/src/api/backend.ts:1052 | MINOR |
| assignAgentGoal | app/src/api/backend.ts:1128 | MINOR |
| stopAgentGoal | app/src/api/backend.ts:1159 | MINOR |
| startAutonomousLoop | app/src/api/backend.ts:1168 | MINOR |
| stopAutonomousLoop | app/src/api/backend.ts:1184 | MINOR |
| getAgentCognitiveStatus | app/src/api/backend.ts:1191 | MINOR |
| getAgentMemories | app/src/api/backend.ts:1211 | MINOR |
| agentMemoryRemember | app/src/api/backend.ts:1227 | MINOR |
| agentMemoryRecall | app/src/api/backend.ts:1245 | MINOR |
| agentMemoryRecallByType | app/src/api/backend.ts:1259 | MINOR |
| agentMemoryForget | app/src/api/backend.ts:1274 | MINOR |
| agentMemoryGetStats | app/src/api/backend.ts:1283 | MINOR |
| agentMemorySave | app/src/api/backend.ts:1287 | MINOR |
| agentMemoryClear | app/src/api/backend.ts:1291 | MINOR |
| getSelfEvolutionMetrics | app/src/api/backend.ts:1312 | MINOR |
| getSelfEvolutionStrategies | app/src/api/backend.ts:1321 | MINOR |
| triggerCrossAgentLearning | app/src/api/backend.ts:1330 | MINOR |
| getHivemindStatus | app/src/api/backend.ts:1347 | MINOR |
| cancelHivemind | app/src/api/backend.ts:1356 | MINOR |
| getOsFitness | app/src/api/backend.ts:1552 | MINOR |
| getFitnessHistory | app/src/api/backend.ts:1556 | MINOR |
| getRoutingStats | app/src/api/backend.ts:1560 | MINOR |
| getUiAdaptations | app/src/api/backend.ts:1564 | MINOR |
| recordPageVisit | app/src/api/backend.ts:1572 | MINOR |
| recordFeatureUse | app/src/api/backend.ts:1576 | MINOR |
| overrideSecurityBlock | app/src/api/backend.ts:1580 | MINOR |
| getOsImprovementLog | app/src/api/backend.ts:1592 | MINOR |
| getMorningOsBriefing | app/src/api/backend.ts:1596 | MINOR |
| recordRoutingOutcome | app/src/api/backend.ts:1600 | MINOR |
| recordOperationTiming | app/src/api/backend.ts:1613 | MINOR |
| getPerformanceReport | app/src/api/backend.ts:1624 | MINOR |
| getSecurityEvolutionReport | app/src/api/backend.ts:1628 | MINOR |
| recordKnowledgeInteraction | app/src/api/backend.ts:1632 | MINOR |
| getOsDreamStatus | app/src/api/backend.ts:1644 | MINOR |
| setSelfImproveEnabled | app/src/api/backend.ts:1648 | MINOR |
| screenshotAnalyze | app/src/api/backend.ts:1654 | MINOR |
| screenshotGenerateSpec | app/src/api/backend.ts:1658 | MINOR |
| voiceProjectStart | app/src/api/backend.ts:1670 | MINOR |
| voiceProjectStop | app/src/api/backend.ts:1674 | MINOR |
| voiceProjectAddChunk | app/src/api/backend.ts:1678 | MINOR |
| voiceProjectGetStatus | app/src/api/backend.ts:1685 | MINOR |
| voiceProjectGetPrompt | app/src/api/backend.ts:1689 | MINOR |
| voiceProjectUpdateIntent | app/src/api/backend.ts:1693 | MINOR |
| stressGeneratePersonas | app/src/api/backend.ts:1705 | MINOR |
| stressGenerateActions | app/src/api/backend.ts:1709 | MINOR |
| stressEvaluateReport | app/src/api/backend.ts:1713 | MINOR |
| deployGenerateDockerfile | app/src/api/backend.ts:1719 | MINOR |
| deployValidateConfig | app/src/api/backend.ts:1723 | MINOR |
| deployGetCommands | app/src/api/backend.ts:1727 | MINOR |
| evolverRegisterApp | app/src/api/backend.ts:1733 | MINOR |
| evolverUnregisterApp | app/src/api/backend.ts:1737 | MINOR |
| evolverListApps | app/src/api/backend.ts:1741 | MINOR |
| evolverDetectIssues | app/src/api/backend.ts:1745 | MINOR |
| freelanceGetStatus | app/src/api/backend.ts:1751 | MINOR |
| freelanceStartScanning | app/src/api/backend.ts:1755 | MINOR |
| freelanceStopScanning | app/src/api/backend.ts:1759 | MINOR |
| freelanceEvaluateJob | app/src/api/backend.ts:1763 | MINOR |
| freelanceGetRevenue | app/src/api/backend.ts:1767 | MINOR |
| getLivePreview | app/src/api/backend.ts:1781 | MINOR |
| publishToMarketplace | app/src/api/backend.ts:1793 | MINOR |
| installFromMarketplace | app/src/api/backend.ts:1797 | MINOR |
| schedulerHistory | app/src/api/backend.ts:3048 | MINOR |
| schedulerRunnerStatus | app/src/api/backend.ts:3059 | MINOR |
| executeTeamWorkflow | app/src/api/backend.ts:3067 | MINOR |
| transferAgentFuel | app/src/api/backend.ts:3082 | MINOR |
| runContentPipeline | app/src/api/backend.ts:3100 | MINOR |
| flashProfileModel | app/src/api/backend.ts:3115 | MINOR |
| flashAutoConfigure | app/src/api/backend.ts:3120 | MINOR |
| flashListSessions | app/src/api/backend.ts:3146 | MINOR |
| flashGetMetrics | app/src/api/backend.ts:3161 | MINOR |
| flashEstimatePerformance | app/src/api/backend.ts:3173 | MINOR |
| flashCatalogRecommend | app/src/api/backend.ts:3180 | MINOR |
| flashCatalogSearch | app/src/api/backend.ts:3185 | MINOR |
| flashDownloadModel | app/src/api/backend.ts:3221 | MINOR |
| flashDownloadMulti | app/src/api/backend.ts:3229 | MINOR |
| flashDeleteLocalModel | app/src/api/backend.ts:3236 | MINOR |
| flashAvailableDiskSpace | app/src/api/backend.ts:3240 | MINOR |
| flashGetModelDir | app/src/api/backend.ts:3244 | MINOR |
| cmGetProfile | app/src/api/backend.ts:3295 | MINOR |
| cmTriggerFeedback | app/src/api/backend.ts:3321 | MINOR |
| cmEvaluateResponse | app/src/api/backend.ts:3328 | MINOR |
| cmExecuteValidationRun | app/src/api/backend.ts:3365 | MINOR |
| cmListValidationRuns | app/src/api/backend.ts:3373 | MINOR |
| cmGetValidationRun | app/src/api/backend.ts:3378 | MINOR |
| cmThreeWayComparison | app/src/api/backend.ts:3383 | MINOR |
| routerRouteTask | app/src/api/backend.ts:3400 | MINOR |
| routerRecordOutcome | app/src/api/backend.ts:3404 | MINOR |
| oracleVerifyToken | app/src/api/backend.ts:3464 | MINOR |
| tokenGetWallet | app/src/api/backend.ts:3479 | MINOR |
| tokenCreateWallet | app/src/api/backend.ts:3487 | MINOR |
| tokenCalculateSpawn | app/src/api/backend.ts:3521 | MINOR |
| tokenCreateDelegation | app/src/api/backend.ts:3528 | MINOR |
| tokenGetDelegations | app/src/api/backend.ts:3539 | MINOR |
| simBranch | app/src/api/backend.ts:3605 | MINOR |
| memoryGetEntry | app/src/api/backend.ts:3693 | MINOR |
| memoryListAgents | app/src/api/backend.ts:3729 | MINOR |
| toolsListAvailable | app/src/api/backend.ts:3739 | MINOR |
| swfSubmitArtifact | app/src/api/backend.ts:3885 | MINOR |

## ═══ UNUSED PAGE IMPORTS ═══
| Page file | Import line | Unused backend imports |
| --- | --- | --- |
| app/src/pages/AiChatHub.tsx | 11 | listAgents, analyzeProblem |
| app/src/pages/CodeEditor.tsx | 11 | type TerminalCommandResult |
| app/src/pages/Collaboration.tsx | 15 | collabCallVote |
| app/src/pages/DreamForge.tsx | 12 | getDreamStatus, getDreamQueue, getDreamHistory, getMorningBriefing |
| app/src/pages/ImmuneDashboard.tsx | 10 | getImmuneStatus, getImmuneMemory, runAdversarialSession |
| app/src/pages/KnowledgeGraph.tsx | 17 | cogfsGetEntities, cogfsGetGraph |
| app/src/pages/MeasurementSession.tsx | 2 | cmGetGamingFlags |
| app/src/pages/ProjectManager.tsx | 2 | projectDelete |
| app/src/pages/SelfRewriteLab.tsx | 11 | selfRewriteGetHistory, selfRewritePreviewPatch, selfRewriteTestPatch |
| app/src/pages/TemporalEngine.tsx | 7 | runDilatedSession |
| app/src/pages/Terminal.tsx | 3 | type TerminalCommandResult |
| app/src/pages/WorldSimulation2.tsx | 2 | simGetResult |

## ═══ BUTTON AUDIT ═══
| Severity | Location | Finding | Evidence |
| --- | --- | --- | --- |
| MAJOR | app/src/pages/AiChatHub.tsx:764 | Rendered code-block "Run" button has no event wiring anywhere in app/src. | Only occurrences of `ch-code-run` are the CSS selector and the injected HTML string. |
| MAJOR | app/src/pages/ComplianceDashboard.tsx:852 | "Run Retention Enforcement" button has no onClick/onSubmit handler. | Button is rendered as plain `<button type="button" className="cd-generate-btn">` with no handler. |

## ═══ DEAD CODE ═══
- cargo +nightly udeps: skipped (`udeps not installed — skip`)
- Unused public functions found by text scan: 54
| Symbol | Location |
| --- | --- |
| suggested_capabilities | crates/nexus-software-factory/src/roles.rs:24 |
| get_artifact | crates/nexus-software-factory/src/project.rs:103 |
| complete_project | crates/nexus-software-factory/src/factory.rs:232 |
| stage_cost | crates/nexus-software-factory/src/economy.rs:10 |
| refund_escrow | crates/nexus-token-economy/src/wallet.rs:163 |
| record_snapshot | crates/nexus-token-economy/src/supply.rs:35 |
| clear_cache | crates/nexus-perception/src/engine.rs:241 |
| provider_model_id | crates/nexus-perception/src/engine.rs:245 |
| find_clickable_elements | crates/nexus-perception/src/screen.rs:29 |
| ask_about_screen | crates/nexus-perception/src/screen.rs:53 |
| extract_form_data | crates/nexus-perception/src/extraction.rs:16 |
| extract_table_data | crates/nexus-perception/src/extraction.rs:30 |
| read_page | crates/nexus-perception/src/document.rs:8 |
| tools_by_category | crates/nexus-external-tools/src/registry.rs:76 |
| max_difficulty | crates/nexus-predictive-router/src/difficulty_estimator.rs:28 |
| check_staging | crates/nexus-predictive-router/src/staging.rs:18 |
| can_propose | crates/nexus-collab-protocol/src/roles.rs:14 |
| complete_session | crates/nexus-collab-protocol/src/protocol.rs:51 |
| completed_sessions | crates/nexus-collab-protocol/src/protocol.rs:65 |
| session_cost | crates/nexus-collab-protocol/src/economy.rs:6 |
| with_data | crates/nexus-collab-protocol/src/message.rs:68 |
| with_reasoning | crates/nexus-collab-protocol/src/message.rs:73 |
| with_references | crates/nexus-collab-protocol/src/message.rs:78 |
| is_broadcast | crates/nexus-collab-protocol/src/message.rs:83 |
| has_changed | crates/nexus-governance-engine/src/versioning.rs:6 |
| is_valid_successor | crates/nexus-governance-engine/src/versioning.rs:11 |
| default_capabilities | crates/nexus-governance-engine/src/capability_model.rs:23 |
| restore_fs | crates/nexus-world-simulation/src/sandbox.rs:100 |
| run_batch_evaluation | crates/nexus-capability-measurement/src/tauri_commands.rs:292 |
| get_ab_comparison | crates/nexus-capability-measurement/src/tauri_commands.rs:467 |
| execute_validation_run_real | crates/nexus-capability-measurement/src/evaluation/validation_run.rs:237 |
| compute_articulation | crates/nexus-capability-measurement/src/scoring/articulation.rs:72 |
| difficulty_description | crates/nexus-capability-measurement/src/battery/difficulty.rs:6 |
| references_tool_output | crates/nexus-capability-measurement/src/vectors/tool_use_integrity.rs:22 |
| acknowledges_limitations | crates/nexus-capability-measurement/src/vectors/tool_use_integrity.rs:35 |
| has_causal_language | crates/nexus-capability-measurement/src/vectors/reasoning_depth.rs:24 |
| avoids_correlation_trap | crates/nexus-capability-measurement/src/vectors/reasoning_depth.rs:40 |
| shows_epistemic_honesty | crates/nexus-capability-measurement/src/vectors/adaptation.rs:26 |
| distinguishes_source_reliability | crates/nexus-capability-measurement/src/vectors/adaptation.rs:41 |
| has_explicit_dependencies | crates/nexus-capability-measurement/src/vectors/planning_coherence.rs:24 |
| has_rollback_handling | crates/nexus-capability-measurement/src/vectors/planning_coherence.rs:39 |
| append_to_chain | crates/nexus-capability-measurement/src/reporting/audit_trail.rs:18 |
| profile_summary | crates/nexus-capability-measurement/src/reporting/cross_vector.rs:6 |
| detect_anomalies | crates/nexus-capability-measurement/src/reporting/cross_vector.rs:21 |
| keyword_count | crates/nexus-agent-memory/src/index.rs:80 |
| tag_count | crates/nexus-agent-memory/src/index.rs:88 |
| update_importance | crates/nexus-agent-memory/src/store.rs:76 |
| request_channel | crates/nexus-governance-oracle/src/submission.rs:7 |
| check_balance | crates/nexus-browser-agent/src/economy.rs:13 |
| score_browser_task | crates/nexus-browser-agent/src/measurement.rs:16 |
| is_running | crates/nexus-browser-agent/src/bridge.rs:150 |
| close_agent_sessions | crates/nexus-browser-agent/src/session.rs:128 |
| generate_speculative | crates/nexus-flash-infer/src/speculative.rs:116 |
| set_subprocess_timeout_ms | crates/nexus-computer-control/src/engine.rs:84 |

- Orphan module candidates found by text scan: 6
| Location | Detail |
| --- | --- |
| crates/nexus-flash-infer/tests/autoconfig_test.rs:1 | Module file is not referenced by nearby mod.rs/lib.rs candidates |
| crates/nexus-flash-infer/tests/registry_test.rs:1 | Module file is not referenced by nearby mod.rs/lib.rs candidates |
| crates/nexus-flash-infer/tests/profiler_test.rs:1 | Module file is not referenced by nearby mod.rs/lib.rs candidates |
| crates/nexus-flash-infer/tests/downloader_test.rs:1 | Module file is not referenced by nearby mod.rs/lib.rs candidates |
| crates/nexus-flash-infer/tests/catalog_test.rs:1 | Module file is not referenced by nearby mod.rs/lib.rs candidates |
| crates/nexus-flash-infer/tests/budget_test.rs:1 | Module file is not referenced by nearby mod.rs/lib.rs candidates |

## ═══ MOCK DATA LOCATIONS ═══
- Strict mock/demo/runtime indicators: 38
| Location | Line |
| --- | --- |
| app/src/types.ts:229 | export type ConnectionStatus = "connected" \| "mock"; |
| app/src/App.tsx:142 | type RuntimeMode = "desktop" \| "mock"; |
| app/src/App.tsx:373 | // A prominent "DEMO MODE" banner is displayed whenever these are active. |
| app/src/App.tsx:475 | text = "[DEMO] Hello! This is the Nexus OS demo preview. You are seeing placeholder data — install the desktop app for full governed chat, agent management, and all runtime features."; |
| app/src/App.tsx:479 | text = "[DEMO] This is a demo preview. Chat responses, agent data, and all actions are simulated. Install the Nexus OS desktop app to connect to the governed runtime."; |
| app/src/App.tsx:521 | const [runtimeMode, setRuntimeMode] = useState<RuntimeMode>("mock"); |
| app/src/App.tsx:528 | const [selectedModel, setSelectedModel] = useState("mock"); |
| app/src/App.tsx:600 | setRuntimeMode("mock"); |
| app/src/App.tsx:607 | "[DEMO MODE] You are viewing a demo preview of Nexus OS. Agent data is simulated and actions are disabled. Install the desktop app for full functionality." |
| app/src/App.tsx:654 | `Connected to desktop backend. Default model: ${normalizedConfig.llm.default_model \|\| "mock-1"}.` |
| app/src/App.tsx:664 | setRuntimeMode("mock"); |
| app/src/App.tsx:670 | makeMessage("assistant", "[DEMO MODE] Backend connection failed. Showing demo data. Restart the desktop app and refresh to reconnect.") |
| app/src/App.tsx:808 | const connectionStatus: ConnectionStatus = runtimeMode === "desktop" ? "connected" : "mock"; |
| app/src/App.tsx:1061 | const model = selectedModel === "mock" ? getModelForAgent(selectedAgent) : selectedModel; |
| app/src/App.tsx:1062 | const isOllamaModel = model.startsWith("ollama/") \|\| (!model.includes("/") && model !== "mock"); |
| app/src/App.tsx:1321 | // Demo mode — simulated reply |
| app/src/App.tsx:1900 | {connectionStatus === "connected" ? "live" : "mock"} |
| app/src/pages/PolicyManagement.tsx:251 | Evaluate the editor TOML against a simulated request. |
| app/src/pages/AgentBrowser.tsx:75 | // Track governance stats — fuel is estimated per-action, not hardcoded |
| app/src/pages/ApiClient.tsx:86 | /* Collections are loaded from backend on mount, not hardcoded */ |
| app/src/pages/ComputerControl.tsx:207 | { action: "type", description: "Agent would type: fn main() { println!(\"Hello, world!\"); }", detail: "Keystroke sequence: 45 characters, typing speed: 60 WPM simulated" }, |
| app/src/pages/SetupWizard.tsx:224 | // Fallback for mock mode: |
| app/src/pages/AiChatHub.tsx:187 | if (lower.includes("no llm provider") \|\| lower.includes("mock")) { |
| app/src/pages/Chat.tsx:136 | label: selectedModel === "mock" ? "Browser runtime selection" : selectedModel, |
| app/src/pages/WorldSimulation.tsx:1043 | <p>Choose any simulated actor and ask them to explain their final worldview.</p> |
| app/src/pages/Settings.tsx:489 | {!p.available && p.name !== "mock" && <span style={{ color: "#ff5252", marginLeft: 6, fontSize: "0.75rem" }}>{p.error_hint \|\| "Unavailable"}</span>} |
| app/src/pages/Settings.tsx:504 | {!p.is_paid && p.name !== "mock" && <span className="st-badge st-badge-green" style={{ fontSize: "0.7rem", padding: "2px 6px" }}>Free</span>} |
| app/src/pages/Audit.tsx:853 | // Client-side fallback for mock mode |
| app/src/voice/PushToTalk.ts:5 | source: "tauri-stt" \| "web-speech" \| "mock-whisper"; |
| app/src/voice/PushToTalk.ts:74 | source: "mock-whisper" |
| app/src/voice/PushToTalk.ts:101 | source: "mock-whisper" |
| app/src/components/agents/CreateAgent.tsx:66 | { value: "mock", label: "mock (Testing)" }, |
| app/src/components/agents/CreateAgent.tsx:237 | self-modification is simulated first and checkpointed for rollback. |
| app/src/components/browser/BuildMode.tsx:81 | /** Generate mock code for a build description, split into typeable chunks. */ |
| app/src/components/browser/BuildMode.tsx:304 | // fall through to mock |
| app/src/components/browser/BuildMode.tsx:373 | // continue mock |
| app/src/components/browser/BuildMode.tsx:415 | // mock complete |
| app/src/components/browser/ResearchMode.tsx:96 | // continue in mock mode |

- UI placeholder attributes (input hints only, listed separately from runtime mock data): 212
| Location | Line |
| --- | --- |
| app/src/pages/TrustDashboard.tsx:271 | placeholder="Agent DID" |
| app/src/pages/TrustDashboard.tsx:295 | <input className="td-rep-input" placeholder="DID" value={regDid} onChange={(e) => setRegDid(e.target.value)} /> |
| app/src/pages/TrustDashboard.tsx:296 | <input className="td-rep-input" placeholder="Name" value={regName} onChange={(e) => setRegName(e.target.value)} /> |
| app/src/pages/TrustDashboard.tsx:308 | <input className="td-rep-input" placeholder="Target DID" value={rateDid} onChange={(e) => setRateDid(e.target.value)} /> |
| app/src/pages/TrustDashboard.tsx:309 | <input className="td-rep-input" placeholder="Rater DID" value={raterDid} onChange={(e) => setRaterDid(e.target.value)} /> |
| app/src/pages/TrustDashboard.tsx:326 | placeholder="Comment (optional)" |
| app/src/pages/TrustDashboard.tsx:341 | <input className="td-rep-input" placeholder="Agent DID" value={taskDid} onChange={(e) => setTaskDid(e.target.value)} /> |
| app/src/pages/TrustDashboard.tsx:362 | placeholder="Paste exported JSON here..." |
| app/src/pages/FileManager.tsx:373 | <input className="fm-search-input" placeholder="Filter files by name..." value={searchQuery} onChange={(e) => setSearchQuery(e.target.value)} autoFocus onKeyDown={(e) => { if (e.key === "Escape") { setShowSearch(false); setSearchQuery(""); } }} /> |
| app/src/pages/FileManager.tsx:383 | <input className="fm-new-item-input" placeholder={newItemType === "dir" ? "folder-name" : "filename.ext"} value={newItemName} onChange={(e) => setNewItemName(e.target.value)} onKeyDown={(e) => { if (e.key === "Enter") handleCreateItem(); if (e.key === "Escape") setNewItemType(null); }} autoFocus /> |
| app/src/pages/ApprovalCenter.tsx:298 | placeholder="Reason (optional)" |
| app/src/pages/ProjectManager.tsx:328 | <input className="pm-search" placeholder="Search tasks..." value={searchQuery} onChange={e => setSearchQuery(e.target.value)} /> |
| app/src/pages/ProjectManager.tsx:359 | <input className="pm-modal-input" placeholder="Task title..." value={newTaskTitle} onChange={e => setNewTaskTitle(e.target.value)} autoFocus onKeyDown={e => e.key === "Enter" && createTask()} /> |
| app/src/pages/ProjectManager.tsx:619 | <textarea className="pm-detail-desc" value={selectedTask.description} onChange={e => { updateTask(selectedTask.id, { description: e.target.value }); setSelectedTask({ ...selectedTask, description: e.target.value }); }} placeholder="Add description..." /> |
| app/src/pages/Scheduler.tsx:239 | <input value={name} onChange={(e) => setName(e.target.value)} style={inputStyle} placeholder="my-scheduled-task" /> |
| app/src/pages/Scheduler.tsx:243 | <input value={agentDid} onChange={(e) => setAgentDid(e.target.value)} style={inputStyle} placeholder="agent-uuid" /> |
| app/src/pages/ClusterStatus.tsx:242 | placeholder="Task description" |
| app/src/pages/ClusterStatus.tsx:247 | placeholder="Agent IDs (comma sep)" |
| app/src/pages/ClusterStatus.tsx:266 | placeholder="Agent ID" |
| app/src/pages/ClusterStatus.tsx:271 | placeholder="Target peer ID" |
| app/src/pages/DeployPipeline.tsx:483 | <input value={newName} onChange={e => setNewName(e.target.value)} placeholder="my-app" /> |
| app/src/pages/DeployPipeline.tsx:493 | <input value={newSourceDir} onChange={e => setNewSourceDir(e.target.value)} placeholder="." /> |
| app/src/pages/DeployPipeline.tsx:765 | <input value={bundleOutputPath} onChange={e => setBundleOutputPath(e.target.value)} placeholder="Output path" |
| app/src/pages/DeployPipeline.tsx:767 | <input value={bundleComponents} onChange={e => setBundleComponents(e.target.value)} placeholder="Components (optional, comma sep)" |
| app/src/pages/DeployPipeline.tsx:789 | <input value={validatePath} onChange={e => setValidatePath(e.target.value)} placeholder="Bundle path" |
| app/src/pages/DeployPipeline.tsx:812 | <input value={installBundlePath} onChange={e => setInstallBundlePath(e.target.value)} placeholder="Bundle path" |
| app/src/pages/DeployPipeline.tsx:814 | <input value={installDir} onChange={e => setInstallDir(e.target.value)} placeholder="Install directory" |
| app/src/pages/Messaging.tsx:283 | placeholder={`${platform.label} token`} |
| app/src/pages/Messaging.tsx:349 | <input value={replyChannel} onChange={e => setReplyChannel(e.target.value)} placeholder="Channel / Chat ID" style={{ flex: "0 0 140px", background: "var(--bg-primary, #0f172a)", border: "1px solid var(--border, #334155)", borderRadius: 4, color: "inherit", padding: "4px 8px", fontSize: "0.8rem", fontFamily: "inherit" }} /> |
| app/src/pages/Messaging.tsx:350 | <input value={replyText} onChange={e => setReplyText(e.target.value)} placeholder="Type a message..." style={{ flex: 1, background: "var(--bg-primary, #0f172a)", border: "1px solid var(--border, #334155)", borderRadius: 4, color: "inherit", padding: "4px 8px", fontSize: "0.8rem", fontFamily: "inherit" }} onKeyDown={e => e.key === 'Enter' && handleSendReply()} /> |
| app/src/pages/ModelRouting.tsx:91 | placeholder="Enter task text to estimate difficulty..." |
| app/src/pages/AppStore.tsx:212 | placeholder="Search by name, description, or capability..." |
| app/src/pages/AppStore.tsx:377 | placeholder="Search nexus-agent repos on GitLab..." |
| app/src/pages/LearningCenter.tsx:1130 | placeholder="Type your response or describe what you built..." |
| app/src/pages/ApiClient.tsx:378 | <input className="ac-url-input" value={activeReq.url} onChange={e => updateReq({ url: e.target.value })} placeholder="Enter request URL..." onKeyDown={e => e.key === "Enter" && sendRequest()} /> |
| app/src/pages/ApiClient.tsx:412 | <input className="ac-kv-key" value={kv.key} onChange={e => updateKV("params", i, { key: e.target.value })} placeholder="Key" /> |
| app/src/pages/ApiClient.tsx:413 | <input className="ac-kv-value" value={kv.value} onChange={e => updateKV("params", i, { value: e.target.value })} placeholder="Value" /> |
| app/src/pages/ApiClient.tsx:430 | <input className="ac-kv-key" value={kv.key} onChange={e => updateKV("headers", i, { key: e.target.value })} placeholder="Header name" /> |
| app/src/pages/ApiClient.tsx:431 | <input className="ac-kv-value" value={kv.value} onChange={e => updateKV("headers", i, { value: e.target.value })} placeholder="Value" /> |
| app/src/pages/ApiClient.tsx:450 | <textarea className="ac-body-editor" value={activeReq.bodyRaw} onChange={e => updateReq({ bodyRaw: e.target.value })} placeholder='{"key": "value"}' spellCheck={false} /> |
| app/src/pages/ApiClient.tsx:453 | <textarea className="ac-body-editor" value={activeReq.bodyRaw} onChange={e => updateReq({ bodyRaw: e.target.value })} placeholder="Raw text body..." spellCheck={false} /> |
| app/src/pages/ApiClient.tsx:464 | <input className="ac-kv-key" value={kv.key} onChange={e => updateKV("bodyForm", i, { key: e.target.value })} placeholder="Key" /> |
| app/src/pages/ApiClient.tsx:465 | <input className="ac-kv-value" value={kv.value} onChange={e => updateKV("bodyForm", i, { value: e.target.value })} placeholder="Value" /> |
| app/src/pages/ApiClient.tsx:488 | <input className="ac-auth-input" value={activeReq.authToken} onChange={e => updateReq({ authToken: e.target.value })} placeholder="Bearer token..." type="password" /> |
| app/src/pages/ApiClient.tsx:495 | <input className="ac-auth-input" value={activeReq.authUser} onChange={e => updateReq({ authUser: e.target.value })} placeholder="Username" /> |
| app/src/pages/ApiClient.tsx:497 | <input className="ac-auth-input" value={activeReq.authPass} onChange={e => updateReq({ authPass: e.target.value })} placeholder="Password" type="password" /> |
| app/src/pages/ApiClient.tsx:503 | <input className="ac-auth-input" value={activeReq.authKeyName} onChange={e => updateReq({ authKeyName: e.target.value })} placeholder="e.g. x-api-key" /> |
| app/src/pages/ApiClient.tsx:505 | <input className="ac-auth-input" value={activeReq.authKeyValue} onChange={e => updateReq({ authKeyValue: e.target.value })} placeholder="API key value" type="password" /> |
| app/src/pages/ComputerControl.tsx:483 | placeholder="App name (e.g. Firefox)" |
| app/src/pages/ComputerControl.tsx:507 | placeholder='{"type":"click","x":100,"y":200}' |
| app/src/pages/EmailClient.tsx:402 | <input className="ec-search" placeholder="Search emails..." value={searchQuery} onChange={e => setSearchQuery(e.target.value)} /> |
| app/src/pages/EmailClient.tsx:472 | <input value={composeTo} onChange={e => setComposeTo(e.target.value)} placeholder="recipient@example.com" /> |
| app/src/pages/EmailClient.tsx:476 | <input value={composeSubject} onChange={e => setComposeSubject(e.target.value)} placeholder="Subject" /> |
| app/src/pages/EmailClient.tsx:479 | <textarea className="ec-compose-body" value={composeBody} onChange={e => setComposeBody(e.target.value)} placeholder="Write your email..." /> |
| app/src/pages/WorldSimulation2.tsx:110 | <input placeholder="Scenario description..." value={scenarioDesc} onChange={(e) => setScenarioDesc(e.target.value)} style={{ flex: 1, padding: 8, borderRadius: 6, background: "#2a2a3e", color: "#e0e0e0", border: "1px solid #444" }} /> |
| app/src/pages/WorldSimulation2.tsx:112 | <textarea placeholder='Actions JSON array' value={actionsJson} onChange={(e) => setActionsJson(e.target.value)} rows={3} style={{ width: "100%", padding: 8, borderRadius: 6, background: "#2a2a3e", color: "#e0e0e0", border: "1px solid #444", fontFamily: "monospace", fontSize: 12, boxSizing: "border-box", resize: "vertical", marginBottom: 8 }} /> |
| app/src/pages/KnowledgeGraph.tsx:522 | placeholder="Ask anything about your files..." |
| app/src/pages/KnowledgeGraph.tsx:658 | placeholder="Enter a topic..." |
| app/src/pages/KnowledgeGraph.tsx:680 | placeholder="File path (e.g. /home/user/doc.md)" |
| app/src/pages/KnowledgeGraph.tsx:702 | placeholder="File path (e.g. /home/user/doc.md)" |
| app/src/pages/KnowledgeGraph.tsx:749 | placeholder="Search query..." |
| app/src/pages/KnowledgeGraph.tsx:756 | placeholder="Time range (start,end)" |
| app/src/pages/KnowledgeGraph.tsx:762 | placeholder="Source filter (csv)" |
| app/src/pages/KnowledgeGraph.tsx:768 | placeholder="Max results" |
| app/src/pages/KnowledgeGraph.tsx:799 | placeholder="Content to ingest..." |
| app/src/pages/KnowledgeGraph.tsx:806 | placeholder='Metadata JSON e.g. {"tag":"notes"}' |
| app/src/pages/KnowledgeGraph.tsx:827 | placeholder="Entry ID to delete" |
| app/src/pages/KnowledgeGraph.tsx:844 | placeholder="Older than N days" |
| app/src/pages/Identity.tsx:619 | placeholder='Paste proof JSON to verify...' |
| app/src/pages/Identity.tsx:722 | placeholder="Peer address (e.g. 192.168.1.50:9090)" |
| app/src/pages/Identity.tsx:728 | placeholder="Peer name (optional)" |
| app/src/pages/Identity.tsx:788 | placeholder="address:port" |
| app/src/pages/NotesApp.tsx:424 | <input id="na-search" className="na-search-input" placeholder="Search notes..." value={searchQuery} onChange={e => setSearchQuery(e.target.value)} /> |
| app/src/pages/NotesApp.tsx:528 | <input className="na-title-input" value={selectedNote.title} onChange={e => updateNote(selectedNote.id, { title: e.target.value })} placeholder="Note title..." /> |
| app/src/pages/NotesApp.tsx:577 | placeholder="Start writing... (Markdown supported)" |
| app/src/pages/TemporalEngine.tsx:328 | placeholder="e.g. Design database schema" style={inputStyle} /> |
| app/src/pages/TemporalEngine.tsx:362 | placeholder="e.g. Build a web scraper for news articles" style={inputStyle} /> |
| app/src/pages/TemporalEngine.tsx:368 | placeholder="agent-1, agent-2" style={inputStyle} /> |
| app/src/pages/UsageBilling.tsx:321 | placeholder="Threshold in USD (e.g. 50.00)" |
| app/src/pages/FlashInference.tsx:637 | <input ref={inputRef} value={input} onChange={e => setInput(e.target.value)} onKeyDown={handleKeyDown} disabled={!hasAnyModel \|\| generating} placeholder={!hasAnyModel ? "Load a model first..." : generating ? "Generating..." : mode === "auto" ? "Type a message (auto-routed)..." : `Type a message (${MODE_LABELS[mode]})...`} style={{ flex:1, background:"#0d1117", border:"1px solid #1e3a5f", borderRadius:"8px", color:"#e5e7eb", padding:"10px 14px", fontSize:"13px", outline:"none", opacity:!hasAnyModel?0.5:1 }}/> |
| app/src/pages/Workspaces.tsx:365 | placeholder="e.g. production, staging, team-alpha" |
| app/src/pages/Workspaces.tsx:615 | placeholder="e.g. oidc:alice or local:nexus" |
| app/src/pages/AgentDnaLab.tsx:851 | placeholder="Strategy name" |
| app/src/pages/AgentDnaLab.tsx:857 | placeholder='Parameters JSON, e.g. {"mutation_rate": 0.1, "crossover": "uniform"}' |
| app/src/pages/AgentDnaLab.tsx:875 | placeholder="Task description" |
| app/src/pages/AgentDnaLab.tsx:912 | placeholder="e.g. I need an agent that can manage Kubernetes clusters and auto-scale pods" |
| app/src/pages/AgentDnaLab.tsx:929 | placeholder="User request" |
| app/src/pages/AgentDnaLab.tsx:936 | placeholder="LLM response (optional)" |
| app/src/pages/AgentDnaLab.tsx:954 | placeholder='Spec JSON, e.g. {"name": "k8s-agent", "capabilities": ["kubernetes", "scaling"]}' |
| app/src/pages/AgentDnaLab.tsx:961 | placeholder="System prompt" |
| app/src/pages/AgentDnaLab.tsx:989 | placeholder="Agent name" |
| app/src/pages/AgentDnaLab.tsx:1006 | placeholder="Spec JSON" |
| app/src/pages/AgentDnaLab.tsx:1014 | placeholder="Missing capabilities (comma-separated)" |
| app/src/pages/Telemetry.tsx:447 | placeholder="http://localhost:4317" |
| app/src/pages/Telemetry.tsx:468 | placeholder="nexus-os" |
| app/src/pages/Civilization.tsx:788 | placeholder='Propose a rule, for example: "Max 10% token budget per agent"' |
| app/src/pages/Civilization.tsx:910 | placeholder="Describe the dispute..." |
| app/src/pages/Civilization.tsx:987 | placeholder="Rule text..." |
| app/src/pages/Civilization.tsx:1059 | placeholder="Issue description..." |
| app/src/pages/Civilization.tsx:1132 | <input type="number" value={earnAmount} onChange={(e) => setEarnAmount(e.target.value)} placeholder="Amount" style={inputStyle} /> |
| app/src/pages/Civilization.tsx:1133 | <input value={earnDesc} onChange={(e) => setEarnDesc(e.target.value)} placeholder="Description" style={inputStyle} /> |
| app/src/pages/Civilization.tsx:1142 | <input type="number" value={spendAmount} onChange={(e) => setSpendAmount(e.target.value)} placeholder="Amount" style={inputStyle} /> |
| app/src/pages/Civilization.tsx:1149 | <input value={spendDesc} onChange={(e) => setSpendDesc(e.target.value)} placeholder="Description" style={inputStyle} /> |
| app/src/pages/Civilization.tsx:1159 | <input type="number" value={transferAmount} onChange={(e) => setTransferAmount(e.target.value)} placeholder="Amount" style={inputStyle} /> |
| app/src/pages/Civilization.tsx:1160 | <input value={transferDesc} onChange={(e) => setTransferDesc(e.target.value)} placeholder="Description" style={inputStyle} /> |
| app/src/pages/Civilization.tsx:1189 | <input value={contractDesc} onChange={(e) => setContractDesc(e.target.value)} placeholder="Description" style={inputStyle} /> |
| app/src/pages/Civilization.tsx:1190 | <input value={contractCriteria} onChange={(e) => setContractCriteria(e.target.value)} placeholder='Criteria JSON (e.g. {"quality":"high"})' style={inputStyle} /> |
| app/src/pages/Civilization.tsx:1192 | <input type="number" value={contractReward} onChange={(e) => setContractReward(e.target.value)} placeholder="Reward" style={inputStyle} /> |
| app/src/pages/Civilization.tsx:1193 | <input type="number" value={contractPenalty} onChange={(e) => setContractPenalty(e.target.value)} placeholder="Penalty" style={inputStyle} /> |
| app/src/pages/Civilization.tsx:1195 | <input type="number" value={contractDeadline} onChange={(e) => setContractDeadline(e.target.value)} placeholder="Deadline (epoch, optional)" style={inputStyle} /> |
| app/src/pages/Civilization.tsx:1204 | <input value={completeContractId} onChange={(e) => setCompleteContractId(e.target.value)} placeholder="Contract ID" style={inputStyle} /> |
| app/src/pages/Civilization.tsx:1209 | <input value={completeEvidence} onChange={(e) => setCompleteEvidence(e.target.value)} placeholder="Evidence (optional)" style={inputStyle} /> |
| app/src/pages/Civilization.tsx:1218 | <input value={disputeContractId} onChange={(e) => setDisputeContractId(e.target.value)} placeholder="Contract ID" style={inputStyle} /> |
| app/src/pages/Civilization.tsx:1219 | <textarea value={disputeContractReason} onChange={(e) => setDisputeContractReason(e.target.value)} placeholder="Reason for dispute..." style={textareaStyle} /> |
| app/src/pages/Civilization.tsx:1270 | <input value={planName} onChange={(e) => setPlanName(e.target.value)} placeholder="Plan name" style={inputStyle} /> |
| app/src/pages/Civilization.tsx:1271 | <input type="number" value={planPrice} onChange={(e) => setPlanPrice(e.target.value)} placeholder="Price (cents)" style={inputStyle} /> |
| app/src/pages/Civilization.tsx:1277 | <input value={planFeatures} onChange={(e) => setPlanFeatures(e.target.value)} placeholder="Features (comma-separated)" style={inputStyle} /> |
| app/src/pages/Civilization.tsx:1286 | <input value={invoicePlanId} onChange={(e) => setInvoicePlanId(e.target.value)} placeholder="Plan ID" style={inputStyle} /> |
| app/src/pages/Civilization.tsx:1287 | <input value={invoiceBuyerId} onChange={(e) => setInvoiceBuyerId(e.target.value)} placeholder="Buyer ID" style={inputStyle} /> |
| app/src/pages/Civilization.tsx:1296 | <input value={payInvoiceId} onChange={(e) => setPayInvoiceId(e.target.value)} placeholder="Invoice ID" style={inputStyle} /> |
| app/src/pages/Civilization.tsx:1305 | <input value={payoutDevId} onChange={(e) => setPayoutDevId(e.target.value)} placeholder="Developer ID" style={inputStyle} /> |
| app/src/pages/Civilization.tsx:1307 | <input type="number" value={payoutAmount} onChange={(e) => setPayoutAmount(e.target.value)} placeholder="Amount (cents)" style={inputStyle} /> |
| app/src/pages/Civilization.tsx:1308 | <input value={payoutPeriod} onChange={(e) => setPayoutPeriod(e.target.value)} placeholder="Period (e.g. 2026-03)" style={inputStyle} /> |
| app/src/pages/AgentMemory.tsx:200 | placeholder="Memory summary..." |
| app/src/pages/AgentMemory.tsx:206 | <input placeholder="Tags (comma-separated)" value={newTags} onChange={(e) => setNewTags(e.target.value)} style={inputStyle} /> |
| app/src/pages/AgentMemory.tsx:208 | <input placeholder="Domain" value={newDomain} onChange={(e) => setNewDomain(e.target.value)} style={{ ...inputStyle, flex: 1 }} /> |
| app/src/pages/AgentMemory.tsx:230 | <input placeholder="Search query..." value={queryText} onChange={(e) => setQueryText(e.target.value)} style={{ ...inputStyle, flex: 1 }} /> |
| app/src/pages/AgentMemory.tsx:243 | <input placeholder="Task description..." value={contextTask} onChange={(e) => setContextTask(e.target.value)} style={{ ...inputStyle, flex: 1 }} /> |
| app/src/pages/AiChatHub.tsx:1159 | <input className="ch-search" placeholder="Search..." value={view === "history" ? historySearch : searchQuery} onChange={e => view === "history" ? setHistorySearch(e.target.value) : setSearchQuery(e.target.value)} /> |
| app/src/pages/AiChatHub.tsx:1523 | <textarea ref={inputRef} className="ch-input" value={input} onChange={e => setInput(e.target.value)} onKeyDown={handleKeyDown} placeholder={`Message ${activeModel?.name ?? "AI"}...`} rows={1} /> |
| app/src/pages/AiChatHub.tsx:1564 | <textarea value={comparePrompt} onChange={e => setComparePrompt(e.target.value)} placeholder="Enter a prompt to compare responses..." rows={3} /> |
| app/src/pages/AiChatHub.tsx:1658 | placeholder={builderStarted ? "Describe what you want to change..." : "Describe what you want to build..."} |
| app/src/pages/AiChatHub.tsx:1758 | placeholder={`Enter ${meta?.label ?? provider} API key...`} |
| app/src/pages/SoftwareFactory.tsx:156 | <input placeholder="Project title" value={title} onChange={(e) => setTitle(e.target.value)} style={inputStyle} /> |
| app/src/pages/SoftwareFactory.tsx:157 | <textarea placeholder="Describe what to build..." value={userRequest} onChange={(e) => setUserRequest(e.target.value)} rows={3} style={{ ...inputStyle, resize: "vertical" }} /> |
| app/src/pages/SoftwareFactory.tsx:237 | <input placeholder="Agent ID" value={agentId} onChange={(e) => setAgentId(e.target.value)} style={{ ...inputStyle, flex: 1 }} /> |
| app/src/pages/SoftwareFactory.tsx:238 | <input placeholder="Name" value={agentName} onChange={(e) => setAgentName(e.target.value)} style={{ ...inputStyle, flex: 1 }} /> |
| app/src/pages/Collaboration.tsx:193 | <input placeholder="Title" value={title} onChange={(e) => setTitle(e.target.value)} style={inputStyle} /> |
| app/src/pages/Collaboration.tsx:194 | <input placeholder="Goal" value={goal} onChange={(e) => setGoal(e.target.value)} style={inputStyle} /> |
| app/src/pages/Collaboration.tsx:198 | <input placeholder="Lead Agent ID" value={leadAgent} onChange={(e) => setLeadAgent(e.target.value)} style={inputStyle} /> |
| app/src/pages/Collaboration.tsx:264 | <input placeholder="Agent ID" value={newAgentId} onChange={(e) => setNewAgentId(e.target.value)} style={{ ...inputStyle, flex: 1 }} /> |
| app/src/pages/Collaboration.tsx:309 | <input placeholder="As agent..." value={msgFrom} onChange={(e) => setMsgFrom(e.target.value)} style={{ ...inputStyle, width: 140 }} /> |
| app/src/pages/Collaboration.tsx:324 | <textarea placeholder="Message..." value={msgText} onChange={(e) => setMsgText(e.target.value)} rows={2} style={{ ...inputStyle, marginTop: 6, resize: "vertical" }} /> |
| app/src/pages/BrowserAgent.tsx:115 | <input style={{ ...inputStyle, flex: 1 }} placeholder="Enter browser task..." value={taskInput} |
| app/src/pages/BrowserAgent.tsx:124 | <input style={{ ...inputStyle, flex: 1 }} placeholder="URL..." value={urlInput} |
| app/src/pages/Chat.tsx:489 | placeholder="Transmit directive to NexusOS..." |
| app/src/pages/WorldSimulation.tsx:613 | placeholder="Name this simulation" |
| app/src/pages/WorldSimulation.tsx:623 | placeholder="Paste the raw seed material here..." |
| app/src/pages/WorldSimulation.tsx:842 | placeholder="Inject variable" |
| app/src/pages/WorldSimulation.tsx:847 | placeholder="Value" |
| app/src/pages/WorldSimulation.tsx:1126 | placeholder="Why did you make your last decision?" |
| app/src/pages/Settings.tsx:547 | placeholder={`Enter ${entry.label} API key`} |
| app/src/pages/Settings.tsx:613 | placeholder={`Enter ${key.label} key`} |
| app/src/pages/Settings.tsx:644 | placeholder={`Enter ${key.label}`} |
| app/src/pages/Settings.tsx:923 | placeholder={'{\n  "tool": "tool_name",\n  "args": {}\n}'} |
| app/src/pages/Audit.tsx:266 | placeholder="Operation name (required)" |
| app/src/pages/Audit.tsx:272 | placeholder="Agent ID (optional)" |
| app/src/pages/Audit.tsx:296 | placeholder="Trace ID (required)" |
| app/src/pages/Audit.tsx:302 | placeholder="Parent Span ID" |
| app/src/pages/Audit.tsx:308 | placeholder="Operation name (required)" |
| app/src/pages/Audit.tsx:314 | placeholder="Agent ID (optional)" |
| app/src/pages/Audit.tsx:338 | placeholder="Span ID (required)" |
| app/src/pages/Audit.tsx:353 | placeholder="Error message (optional)" |
| app/src/pages/Audit.tsx:648 | placeholder="Invariant name (e.g., capability_checks, fuel_budget, audit_integrity, pii_redaction, hitl_approval, no_unsafe_code)" |
| app/src/pages/Audit.tsx:982 | placeholder="Search events, payloads, hashes..." |
| app/src/pages/Agents.tsx:557 | placeholder="Search agents by name, capability, or description…" |
| app/src/pages/Agents.tsx:1159 | placeholder="Describe a goal for this agent..." |
| app/src/pages/AdminUsers.tsx:104 | placeholder="Filter users..." |
| app/src/pages/ExternalTools.tsx:184 | <input placeholder="Agent ID" value={agentId} onChange={(e) => setAgentId(e.target.value)} style={{ ...inputStyle, width: 140 }} /> |
| app/src/pages/ExternalTools.tsx:191 | placeholder='{"action": "list_repos", "user": "octocat"}' |
| app/src/pages/DatabaseManager.tsx:313 | placeholder="Connection name..." |
| app/src/pages/DatabaseManager.tsx:320 | placeholder="SQLite path (e.g. ~/.nexus/data.db)" |
| app/src/pages/DatabaseManager.tsx:414 | placeholder="Enter SQL query... (Ctrl+Enter to run)" |
| app/src/pages/DatabaseManager.tsx:502 | <input className="db-builder-filter-val" value={filter.value} onChange={e => updateBuilderFilter(idx, { value: e.target.value })} placeholder="value" /> |
| app/src/pages/Protocols.tsx:489 | placeholder="Agent base URL (e.g. http://localhost:9000)" |
| app/src/pages/Protocols.tsx:535 | placeholder="Agent URL" |
| app/src/pages/Protocols.tsx:541 | placeholder="Task message..." |
| app/src/pages/Protocols.tsx:562 | placeholder="Agent URL" |
| app/src/pages/Protocols.tsx:569 | placeholder="Task ID" |
| app/src/pages/Protocols.tsx:605 | placeholder="Server name" |
| app/src/pages/Protocols.tsx:612 | placeholder="URL (e.g. http://localhost:8080)" |
| app/src/pages/Protocols.tsx:628 | placeholder="Auth token (optional)" |
| app/src/pages/Protocols.tsx:731 | placeholder="Tool name" |
| app/src/pages/Protocols.tsx:737 | placeholder='Arguments JSON, e.g. {"key": "value"}' |
| app/src/pages/ModelHub.tsx:590 | placeholder="Search GGUF models on HuggingFace..." |
| app/src/pages/ModelHub.tsx:1526 | placeholder="Peer address (e.g. 192.168.1.100:9090)" |
| app/src/pages/ModelHub.tsx:1543 | placeholder="Peer name (e.g. Office Desktop)" |
| app/src/pages/ModelHub.tsx:1595 | placeholder="Peer address" |
| app/src/pages/ModelHub.tsx:1612 | placeholder="Model ID (e.g. TheBloke/Llama-2-7B-GGUF)" |
| app/src/pages/ModelHub.tsx:1629 | placeholder="Filename (e.g. llama-2-7b.Q4_K_M.gguf)" |
| app/src/pages/TokenEconomy.tsx:437 | placeholder="Filter by agent ID..." |
| app/src/pages/CodeEditor.tsx:820 | <input className="ce-search-input" placeholder="Search across all files..." value={searchQuery} onChange={(e) => setSearchQuery(e.target.value)} autoFocus onKeyDown={(e) => { if (e.key === "Escape") { setShowSearch(false); setSearchQuery(""); } }} /> |
| app/src/pages/CodeEditor.tsx:854 | <input className="ce-new-file-input" placeholder="filename.ext" value={newFileName} onChange={(e) => setNewFileName(e.target.value)} onKeyDown={(e) => { if (e.key === "Enter") handleCreateFile(); if (e.key === "Escape") { setShowNewFile(false); setNewFileName(""); } }} autoFocus /> |
| app/src/pages/CodeEditor.tsx:1030 | placeholder="Type a command..." |
| app/src/pages/CodeEditor.tsx:1081 | <input className="ce-git-commit-input" placeholder="Commit message..." value={commitMsg} onChange={(e) => setCommitMsg(e.target.value)} onKeyDown={(e) => { if (e.key === "Enter") handleGitCommit(); }} /> |
| app/src/pages/Perception.tsx:205 | placeholder="API Key" |
| app/src/pages/Perception.tsx:211 | placeholder="Model ID" |
| app/src/pages/Perception.tsx:266 | placeholder="Ask a question about the image..." |
| app/src/pages/Perception.tsx:275 | placeholder='Optional JSON schema, e.g. {"type": "object"}' |
| app/src/pages/Documents.tsx:1074 | placeholder={ |
| app/src/pages/Integrations.tsx:229 | placeholder={field.placeholder} |
| app/src/pages/TimeMachine.tsx:901 | placeholder="Filter by agent ID..." |
| app/src/pages/TimeMachine.tsx:1485 | placeholder="Checkpoint label..." |
| app/src/pages/Terminal.tsx:650 | placeholder="Type a command..." |
| app/src/components/agents/CreateAgent.tsx:180 | placeholder="Agent name" |
| app/src/components/agents/CreateAgent.tsx:186 | placeholder="Describe mission objectives, constraints, and expected outputs..." |
| app/src/components/agents/CreateAgent.tsx:293 | placeholder="*/10 * * * *" |
| app/src/components/agents/CreateAgent.tsx:300 | placeholder="What should this agent do on each scheduled run?" |
| app/src/components/chat/History.tsx:52 | placeholder="Search transmissions..." |
| app/src/components/browser/BrowserToolbar.tsx:152 | placeholder="Enter URL... (Ctrl+L to focus)" |
| app/src/components/browser/BuildMode.tsx:437 | placeholder="Describe what to build... (e.g. landing page with hero section and feature cards)" |
| app/src/components/browser/ResearchMode.tsx:326 | placeholder="Enter research topic..." |

## ═══ MISSING ERROR HANDLING ═══
- Pages with backend wrapper usage and no error markers: 0
| Location | Backend wrappers used |
| --- | --- |
| None | - |

- Pages with backend wrapper usage and no loading markers: 15
| Location | Backend wrappers used |
| --- | --- |
| app/src/pages/AuditTimeline.tsx:1 | 3 |
| app/src/pages/Collaboration.tsx:1 | 11 |
| app/src/pages/DatabaseManager.tsx:1 | 5 |
| app/src/pages/DreamForge.tsx:1 | 3 |
| app/src/pages/Firewall.tsx:1 | 3 |
| app/src/pages/MediaStudio.tsx:1 | 5 |
| app/src/pages/Messaging.tsx:1 | 8 |
| app/src/pages/NotesApp.tsx:1 | 4 |
| app/src/pages/SelfRewriteLab.tsx:1 | 4 |
| app/src/pages/SoftwareFactory.tsx:1 | 9 |
| app/src/pages/SystemMonitor.tsx:1 | 2 |
| app/src/pages/TemporalEngine.tsx:1 | 2 |
| app/src/pages/Terminal.tsx:1 | 2 |
| app/src/pages/TimelineViewer.tsx:1 | 2 |
| app/src/pages/WorldSimulation.tsx:1 | 10 |

## ═══ SECURITY FINDINGS ═══
| Finding | Evidence |
| --- | --- |
| None | No hardcoded secrets matched the strict grep; no committed .env files found; git log shows no added .env files in the last 5 additions. |

## ═══ CONFIG / ENV COMPLETENESS ═══
| Expected root file | Exists | Location |
| --- | --- | --- |
| .gitlab-ci.yml | yes | .gitlab-ci.yml |
| Cargo.toml | yes | Cargo.toml |
| package.json | no | package.json (missing at repo root) |
| tsconfig.json | no | tsconfig.json (missing at repo root) |

| Env var | Location |
| --- | --- |
| XDG_DATA_HOME | crates/nexus-flash-infer/src/downloader.rs:667 |
| CARGO_MANIFEST_DIR | app/src-tauri/src/main.rs:9277 |

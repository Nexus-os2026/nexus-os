# Phase 5: v5.0 - Production Ready

> Priority: HIGH | Theme: Make Nexus OS undeniable to the world

## 5.1 WASM Agent Sandbox (sdk/src/sandbox.rs)

Third-party agents run in WASM sandboxes. Memory-limited, time-limited, capability-gated host functions. Uses wasmtime. Untrusted code cannot escape the sandbox. Host functions for llm_query, fs_read, fs_write, request_approval are the only bridge to the kernel. Each host function checks capabilities and fuel before executing.

- [ ] WasmSandbox struct with configurable memory and time limits
- [ ] Host functions bridging WASM to kernel governance
- [ ] Agent that exceeds memory limit is killed
- [ ] Agent that exceeds time limit is killed
- [ ] Capability checks enforced on every host function call

## 5.2 Real Networking for Distributed (distributed/src/tcp_transport.rs)

Replace LocalTransport with real TCP networking. TLS encrypted. Node authentication with Ed25519 key pairs. Connection pooling. Reconnection with exponential backoff. Wire into replication and quorum systems.

- [ ] TcpTransport implementing Transport trait
- [ ] TLS encryption on all node-to-node communication
- [ ] Ed25519 mutual authentication on connection
- [ ] Reconnection with exponential backoff
- [ ] Integration test: 2 nodes on different ports replicating audit events

## 5.3 CLI Completeness (cli/)

Full CLI covering every subsystem. Commands: nexus agent list/start/stop/status, nexus audit show/verify/export, nexus cluster join/status/leave, nexus marketplace search/install/uninstall, nexus compliance report, nexus verify (evidence bundles), nexus benchmark run, nexus delegation grant/revoke/list, nexus finetune create/approve/status. Every command outputs structured JSON with --json flag.

- [ ] All subsystem commands implemented
- [ ] JSON output mode on every command
- [ ] Help text on every command
- [ ] Integration tests for critical paths

## 5.4 Desktop UI Overhaul (app/)

Rebuild the Tauri desktop shell with pages for every new subsystem. Command center with live agent grid. Governance timeline with federated audit. Approval queue with quorum status. Marketplace browser. Compliance dashboard. Cluster status with node health. Delegation graph visualization. Adaptive governance trust scores per agent.

- [ ] Command center with live agent telemetry
- [ ] Audit timeline with federation cross-references
- [ ] Marketplace browser with install/verify
- [ ] Compliance dashboard with SOC2 control status
- [ ] Cluster status page with node health indicators
- [ ] Trust score dashboard per agent

## 5.5 Documentation and Developer Portal (docs/)

Complete documentation: architecture guide, SDK getting started, manifest reference, capability catalog, deployment guide, security hardening guide, compliance guide, API reference. README rewrite showcasing v4.0.0 capabilities.

- [ ] Architecture guide with diagrams
- [ ] SDK tutorial: build your first governed agent
- [ ] Deployment guide: single node and cluster
- [ ] Security hardening checklist
- [ ] API reference for all public types
- [ ] README rewrite with feature matrix and screenshots

## 5.6 End-to-End Integration Test Suite (tests/)

Full system tests: agent runs through complete governance pipeline (capability check, fuel, HITL approval, audit, evidence bundle). Distributed test: 3-node cluster with quorum vote on agent action. Marketplace test: publish agent, install, run governed, verify evidence. Compliance test: run agents, generate SOC2 report, verify all controls satisfied.

- [ ] Single-node full governance pipeline test
- [ ] 3-node distributed quorum test
- [ ] Marketplace publish-install-run test
- [ ] SOC2 compliance end-to-end test
- [ ] Adaptive governance promotion/demotion over multiple runs

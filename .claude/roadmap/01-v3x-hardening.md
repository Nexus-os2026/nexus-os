# Phase 1: v3.x - Hardening (CURRENT)

## 1.1 Benchmark Suite (benchmarks/)
Create benchmarks/ crate with Criterion. Modules: kernel_bench.rs, gateway_bench.rs, agent_bench.rs, replay_bench.rs.
- [ ] cargo bench produces HTML reports
- [ ] CI uploads artifacts on tagged release

## 1.2 Replay Evidence Bundles (kernel/src/replay/)
Add replay module: EvidenceBundle, PolicySnapshot, standalone verifier.
- [ ] nexus export-evidence produces .nexus-evidence file
- [ ] nexus verify validates independently
- [ ] Integration test: run then export then verify

## 1.3 Production Installer Pipeline
Update release.yml for signed installers on all platforms.
- [ ] Tagged push produces signed installers
- [ ] Release page includes checksums

## 1.4 LLM Gateway Hardening (connectors/llm/)
Add circuit_breaker.rs and routing.rs with 4 strategies.
- [ ] Circuit breaker with tests
- [ ] Provider router with fallback
- [ ] Integration test: failover with audit

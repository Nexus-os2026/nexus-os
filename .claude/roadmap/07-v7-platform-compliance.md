# Phase 7: v7.0 — The Platform & Compliance Phase

> Status: NEXT
> Depends on: Phase 6 (complete)

## 7.1 — A2A + MCP Protocol Integration

- [ ] HTTP gateway (axum + tokio) wrapping Supervisor with capability/fuel checks
- [ ] Google A2A Agent Card from agent manifests
- [ ] A2A JSON-RPC: tasks/send, tasks/get, tasks/cancel
- [ ] MCP tool server exposing agent capabilities as governed tools
- [ ] Audit trail for all inbound protocol requests

## 7.2 — Agent Identity + Prompt Firewall

- [ ] DID:key identity for agents (from existing ed25519 keys)
- [ ] Verifiable Credentials for agent capabilities
- [ ] Prompt injection firewall at LLM gateway boundary
- [ ] Input/output sanitization on all protocol endpoints

## 7.3 — EU AI Act Compliance

- [ ] Risk classification per agent (minimal/limited/high/unacceptable)
- [ ] Transparency logging: model used, confidence, data sources
- [ ] Human oversight controls mapped to autonomy levels
- [ ] Compliance report generator from audit trail

## 7.4 — Developer SDK + Marketplace

- [ ] Plugin SDK with governed capability registration
- [ ] Marketplace protocol for agent/tool discovery
- [ ] Sandboxed plugin execution via WASM
- [ ] Developer documentation and examples

## 7.5 — Web Dashboard + Production Hardening

- [ ] Web dashboard for agent monitoring and compliance views
- [ ] A2A/MCP connection status per agent
- [ ] Identity and credential management UI
- [ ] E2E integration tests for all protocol flows
- [ ] Performance benchmarks and load testing

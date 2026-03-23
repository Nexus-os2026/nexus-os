# Nexus OS — Persistent Context

## Project
- Location: ~/NEXUS/nexus-os
- GitLab: gitlab.com/nexaiceo/nexus-os
- Version: v9.3.0

## Stack
- Rust kernel + Tauri 2.0 + React/TypeScript
- Python voice pipeline
- Ollama (local) + NVIDIA NIM (93 models)

## STRICT RULES — NEVER BREAK
- NEVER use --all-features (crashes Candle ML)
- NEVER resume interrupted sessions
- Always run cargo fmt/clippy on modified crates only
- Plain text prompts only — never bash scripts
- Full workspace tests run in terminal not in Claude Code

## Current Status
- 3,514 tests passing, 0 failures
- Zero production unwrap/expect/panic
- Darwin Core: REAL (AdversarialArena + SwarmCoordinator + PlanEvolutionEngine)
- Flash Engine: REAL (Qwen 35B/122B/397B running locally)
- NVIDIA NIM: 93 models, 18 providers
- Discord + Telegram + A2A + MCP Client: REAL

## Remaining Work
1. Universal Inference Engine (any model, any RAM)
2. Memory leak fixes (9 files — useEffect cleanup)
3. Slack adapter
4. MCP Server
5. Visual Workflow Editor
6. Premium website
7. Demo video (Suresh records)
8. Hacker News launch (Suresh posts)

## Crate Names
nexus-kernel, nexus-connectors-llm, nexus-flash,
nexus-inference, nexus-conductor, nexus-sandbox,
nexus-identity, nexus-audit, nexus-hitl,
nexus-desktop-backend

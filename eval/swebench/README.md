# Nexus OS — SWE-bench Evaluation Harness

## Overview

This harness evaluates Nexus OS's code generation capabilities against
[SWE-bench Verified](https://www.swebench.com/), the standard benchmark
for autonomous software engineering.

## Architecture

```
SWE-bench Instance (repo + issue description)
       │
       ▼
nexus_swebench_bridge.py
       │
       ▼
POST /v1/chat/completions  (OpenAI-compatible API)
       │
       ▼
Nexus OS Gateway → Governance Pipeline → LLM Provider
       │
       ▼
Unified Diff Patch
       │
       ▼
SWE-bench Harness (apply patch → run tests → score)
```

The bridge sends each SWE-bench instance through the Nexus OS OpenAI-compatible
API (`/v1/chat/completions`). The gateway routes the request through the full
governance pipeline (capability checks, PII redaction, audit trail, fuel metering)
before forwarding to the configured LLM provider.

## Components

| Component | Description |
|-----------|-------------|
| `nexus_swebench_bridge.py` | Python bridge: loads dataset, calls API, saves predictions |
| `setup.sh` | Environment setup script (venv, dependencies, dataset download) |
| `sample_3.jsonl` | 3 synthetic test instances for bridge validation |

## Nexus OS Code Generation Capabilities

### What Exists

The codebase has real LLM-powered code generation across multiple agents:

| Agent | Location | Capabilities |
|-------|----------|--------------|
| **Coder Agent** | `agents/coder/` | LLM code generation, multi-file output, git operations, unified diffs, test execution, iterative fix loops |
| **Coding Agent** | `agents/coding-agent/` | File editing, code analysis, project scaffolding |
| **Software Factory** | `crates/nexus-software-factory/` | 7-stage SDLC pipeline (Requirements → Architecture → Implementation → Testing → Review → Deployment → Verification) |
| **Conductor** | `agents/conductor/` | Multi-agent orchestration, task dispatching |

### Key Functions

- `generate_code_with_llm()` — LLM-driven multi-file code generation with fenced block parsing
- `unified_diff()` — produces unified diffs from before/after content
- `GitIntegration` — git status, diff, branch, commit operations
- `FixLoop` — iterative test→fix→test loop with LLM-generated fixes
- `LlmErrorFixer` — proposes code fixes for test failures using LLM

### Pipeline for SWE-bench

For SWE-bench evaluation, the flow is:

1. Issue description sent via OpenAI-compatible API
2. Routed through governance (audit, redaction, capability check)
3. LLM provider generates a patch
4. Response returned as unified diff

The coder agent's `fix_loop` module implements the closest analogy to
SWE-bench's expected workflow: given test failures, iteratively generate
fixes until tests pass. However, SWE-bench evaluation currently goes
through the simpler single-shot API path.

## Quick Start

```bash
# 1. Setup
bash setup.sh

# 2. Start Nexus OS (in another terminal)
cd ~/NEXUS/nexus-os
cargo run -p nexus-protocols -- --port 3000
# OR: launch the desktop app

# 3. Quick validation (3 synthetic instances)
source venv/bin/activate
python nexus_swebench_bridge.py sample_3.jsonl --limit 3

# 4. Full evaluation (requires SWE-bench Verified dataset)
python nexus_swebench_bridge.py swebench_verified.jsonl

# 5. Score with SWE-bench harness
python -m swebench.harness.run_evaluation \
  --predictions_path predictions/predictions.jsonl \
  --swe_bench_tasks swebench_verified.jsonl \
  --log_dir predictions/logs
```

## Configuration

| Env Variable | Default | Description |
|-------------|---------|-------------|
| `NEXUS_API_URL` | `http://localhost:3000/v1` | Nexus OS API endpoint |
| `NEXUS_API_KEY` | (empty) | Bearer token for auth (optional in dev) |
| `NEXUS_MODEL` | `nexus-governed` | Model to use for generation |

### Model Selection

The model determines performance. Examples:

```bash
# Use Ollama local model
NEXUS_MODEL=ollama/llama3 python nexus_swebench_bridge.py data.jsonl

# Use cloud model via Nexus OS routing
NEXUS_MODEL=gpt-4o python nexus_swebench_bridge.py data.jsonl
NEXUS_MODEL=claude-sonnet-4-20250514 python nexus_swebench_bridge.py data.jsonl

# Use agent routing (routes to software factory)
NEXUS_MODEL=agent/nexus-coder python nexus_swebench_bridge.py data.jsonl
```

## Current Status

### What Works
- Bridge script: tested, generates correct SWE-bench prediction format
- Offline mode: generates prompts for manual LLM invocation
- API mode: calls Nexus OS OpenAI-compatible endpoint
- Incremental output: saves predictions after each instance

### What's Needed for Full Evaluation
1. **Running LLM provider** — Ollama local model or cloud API key configured
2. **SWE-bench Verified dataset** — requires `datasets` Python package
3. **Docker** — SWE-bench harness needs Docker to run repo test suites
4. **Time/compute** — 500 instances × ~60s each ≈ 8+ hours for full set

### Honest Assessment

The Nexus OS software factory is a **project management pipeline** (7-stage SDLC
with quality gates), not a direct SWE-bench solver. The actual code generation
happens through the LLM provider configured in the gateway.

SWE-bench performance will be determined primarily by:
1. The underlying LLM model (GPT-4o, Claude Sonnet, local Llama, etc.)
2. The prompt engineering in the bridge
3. Whether the agent has repo context (currently: no, just issue text)

The governance layer adds value through audit trails and safety checks but
does not directly improve patch quality. Future work should integrate the
coder agent's `fix_loop` for iterative refinement.

## Comparison Context

| System | SWE-bench Verified | Notes |
|--------|-------------------|-------|
| Claude Code | ~49% | Multi-turn with repo context |
| Aider (GPT-4o) | ~26% | Single-turn with repo map |
| SWE-Agent (GPT-4) | ~12% | Agent loop with shell access |
| Raw GPT-4o | ~7% | Single-turn, no tools |
| **Nexus OS** | TBD | Single-turn via API (model-dependent) |

Expected performance: comparable to raw LLM performance since the current
bridge is single-turn without repo context. Multi-turn agent loop with
file access would significantly improve results.

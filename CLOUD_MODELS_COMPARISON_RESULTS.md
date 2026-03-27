# Nexus OS — Cloud Models Comparison Stress Test Results

**Date**: 2026-03-25 05:04:38 GMT
**Total wall time**: 731.1s (12.2 minutes across 4 model runs)
**Test harness**: `nexus-conductor-benchmark --bin cloud-models-bench`
**Providers framework**: 12 supported (11 cloud + Ollama local)
**Models tested**: 4 local Ollama models (llama3.1:8b, qwen3.5:4b, glm4:9b, qwen2.5-coder:7b)
**Cloud providers**: 11 configured but awaiting API keys (DeepSeek, Groq, Mistral, Together, Fireworks, Perplexity, OpenRouter, OpenAI, Gemini, Cohere, NVIDIA NIM)

---

## Executive Summary

### Speed Rankings (by avg P50 latency — single request)

| Rank | Provider/Model | Avg P50 (ms) | Rating |
|------|---------------|--------------|--------|
| 1 | glm4:9b | 142 | BLAZING |
| 2 | qwen2.5-coder:7b | 151 | BLAZING |
| 3 | llama3.1:latest (8B) | 171 | BLAZING |
| 4 | qwen3.5:4b | 1,416 | SLOW |

### Determinism Rankings (by avg match rate across 5 prompts × 10 runs)

| Rank | Provider/Model | Avg Match Rate | Rating |
|------|---------------|---------------|--------|
| 1 | llama3.1:latest | 100.0% | PERFECT |
| 2 | qwen2.5-coder:7b | 100.0% | PERFECT |
| 3 | qwen3.5:4b | 100.0% | PERFECT |
| 4 | glm4:9b | 98.0% | EXCELLENT |

### Throughput Rankings (at 50 concurrent agents)

| Rank | Provider/Model | Throughput (req/s) | Error Rate |
|------|---------------|-------------------|------------|
| 1 | llama3.1:latest | 20.5 | 0.0% |
| 2 | glm4:9b | 19.6 | 0.0% |
| 3 | qwen2.5-coder:7b | 17.9 | 0.0% |
| 4 | qwen3.5:4b | 0.8 | 0.0% |

### Agentic Workload Rankings (decision-making accuracy)

| Rank | Provider/Model | Valid Actions (of 3) | Rating |
|------|---------------|---------------------|--------|
| 1 | qwen2.5-coder:7b | 3/3 (100%) | PERFECT |
| 2 | glm4:9b | 2/3 (67%) | GOOD |
| 3 | llama3.1:latest | 2/3 (67%) | GOOD |
| 4 | qwen3.5:4b | N/A (timeout) | SLOW |

### Recommendation for Nexus OS Agentic Workloads

**WINNER: qwen2.5-coder:7b** — P50=151ms, Determinism=100%, Agentic=3/3, Throughput=17.9 req/s

Best balance of speed, determinism, and agentic decision quality. Perfect single-word action extraction on all 3 test prompts (BUY/SELL/HOLD, verb extraction, priority classification).

**RUNNER-UP: glm4:9b** — P50=142ms (fastest), 98% determinism, 19.6 req/s

Fastest raw inference but slight determinism wobble on one prompt (90% match rate on "Is water wet?" — 2 unique outputs across 10 runs).

| Rank | Provider/Model | Combined Score | P50 (ms) | Determinism | Agentic | Throughput |
|------|---------------|---------------|----------|-------------|---------|------------|
| 1 | qwen2.5-coder:7b | 0 (best) | 151 | 100.0% | 3/3 | 17.9 req/s |
| 2 | glm4:9b | 2 | 142 | 98.0% | 2/3 | 19.6 req/s |
| 3 | llama3.1:latest | 3 | 171 | 100.0% | 2/3 | 20.5 req/s |
| 4 | qwen3.5:4b | 7 (worst) | 1,416 | 100.0% | N/A | 0.8 req/s |

---

## 1. Determinism Test (10 identical prompts per model, temp=0, seed=42)

### llama3.1:latest (Meta Llama 3.1 8B)

| Prompt | Runs | Unique | Match Rate | P50 | P95 | Errors |
|--------|------|--------|------------|-----|-----|--------|
| What is 2+2? Answer with just the number. | 10 | 1 | 100.0% | 158ms | 6,267ms | 0 |
| Name the capital of France in one word. | 10 | 1 | 100.0% | 178ms | 196ms | 0 |
| Is water wet? Answer yes or no. | 10 | 1 | 100.0% | 179ms | 190ms | 0 |
| What color is the sky on a clear day? One word. | 10 | 1 | 100.0% | 175ms | 193ms | 0 |
| How many sides does a triangle have? | 10 | 1 | 100.0% | 165ms | 178ms | 0 |

### qwen3.5:4b (Alibaba Qwen 3.5 4B)

| Prompt | Runs | Unique | Match Rate | P50 | P95 | Errors |
|--------|------|--------|------------|-----|-----|--------|
| What is 2+2? Answer with just the number. | 10 | 1 | 100.0% | 1,404ms | — | 0 |
| Name the capital of France in one word. | 10 | 1 | 100.0% | 1,415ms | — | 0 |
| Is water wet? Answer yes or no. | 10 | 1 | 100.0% | 1,420ms | — | 0 |
| What color is the sky on a clear day? One word. | 10 | 1 | 100.0% | 1,422ms | — | 0 |
| How many sides does a triangle have? | 10 | 1 | 100.0% | 1,420ms | — | 0 |

### glm4:9b (Zhipu GLM-4 9B)

| Prompt | Runs | Unique | Match Rate | P50 | P95 | Errors |
|--------|------|--------|------------|-----|-----|--------|
| What is 2+2? Answer with just the number. | 10 | 1 | 100.0% | 142ms | — | 0 |
| Name the capital of France in one word. | 10 | 1 | 100.0% | 140ms | — | 0 |
| Is water wet? Answer yes or no. | 10 | 2 | **90.0%** | 797ms | — | 0 |
| What color is the sky on a clear day? One word. | 10 | 1 | 100.0% | 147ms | — | 0 |
| How many sides does a triangle have? | 10 | 1 | 100.0% | 146ms | — | 0 |

### qwen2.5-coder:7b (Alibaba Qwen 2.5 Coder 7B)

| Prompt | Runs | Unique | Match Rate | P50 | P95 | Errors |
|--------|------|--------|------------|-----|-----|--------|
| What is 2+2? Answer with just the number. | 10 | 1 | 100.0% | 149ms | 5,911ms | 0 |
| Name the capital of France in one word. | 10 | 1 | 100.0% | 143ms | 175ms | 0 |
| Is water wet? Answer yes or no. | 10 | 1 | 100.0% | 160ms | 173ms | 0 |
| What color is the sky on a clear day? One word. | 10 | 1 | 100.0% | 161ms | 183ms | 0 |
| How many sides does a triangle have? | 10 | 1 | 100.0% | 145ms | 161ms | 0 |

**Key Finding**: All models achieve 98-100% determinism with temp=0 + seed=42. The GLM-4 9B "Is water wet?" wobble (90%) is due to its verbose reasoning style producing occasional variant phrasing. The first-request P95 spike (~6s) on llama3.1 and qwen2.5-coder is Ollama's cold-load latency.

---

## 2. Latency Scaling (concurrent agents)

### llama3.1:latest (8B)

| Concurrency | P50 | P95 | P99 | Throughput | Errors | Wall Time |
|-------------|-----|-----|-----|------------|--------|-----------|
| 50 | 1,587ms | 2,368ms | 2,440ms | 20.5 req/s | 0.0% | 2.4s |
| 100 | 3,544ms | 5,179ms | 5,338ms | 18.6 req/s | 0.0% | 5.4s |
| 500 | 19,207ms | 29,271ms | 30,121ms | 16.3 req/s | 0.0% | 30.6s |

### qwen3.5:4b

| Concurrency | P50 | P95 | P99 | Throughput | Errors | Wall Time |
|-------------|-----|-----|-----|------------|--------|-----------|
| 50 | 33,644ms | 60,861ms | 63,340ms | 0.8 req/s | 0.0% | 64.1s |
| 100 | 61,713ms | 114,909ms | 119,910ms | 0.8 req/s | 5.0% | 120.5s |
| 500 | 68,115ms | 115,107ms | 119,708ms | 0.7 req/s | **83.0%** | ~480s |

### glm4:9b

| Concurrency | P50 | P95 | P99 | Throughput | Errors | Wall Time |
|-------------|-----|-----|-----|------------|--------|-----------|
| 50 | 1,681ms | 2,466ms | 2,537ms | 19.6 req/s | 0.0% | 2.5s |
| 100 | 3,471ms | 5,077ms | 5,229ms | 19.0 req/s | 0.0% | 5.3s |
| 500 | 18,214ms | 27,830ms | 28,852ms | 17.0 req/s | 0.0% | 29.4s |

### qwen2.5-coder:7b

| Concurrency | P50 | P95 | P99 | Throughput | Errors | Wall Time |
|-------------|-----|-----|-----|------------|--------|-----------|
| 50 | 1,831ms | 2,705ms | 2,783ms | 17.9 req/s | 0.0% | 2.8s |
| 100 | 3,709ms | 5,480ms | 5,676ms | 17.6 req/s | 0.0% | 5.7s |
| 500 | 19,520ms | 28,545ms | 29,438ms | 16.6 req/s | 0.0% | 30.1s |

**Key Finding**: Llama 3.1 8B, GLM-4 9B, and Qwen 2.5 Coder 7B all show similar throughput (~17-20 req/s) and graceful latency scaling. Qwen 3.5 4B is an outlier — its thinking/reasoning overhead makes it 20-25x slower, with 83% error rate at 500 agents (curl timeouts). For agentic workloads, **avoid reasoning-heavy models** unless the task requires chain-of-thought.

---

## 3. Cross-Provider Stress Test (100 agents simultaneous, same prompt)

| Provider/Model | Agents | Unique Outputs | Match Rate | P50 | P95 | P99 | Errors |
|---------------|--------|----------------|------------|-----|-----|-----|--------|
| llama3.1:latest | 100 | 1 | 100.0% | 3,470ms | 5,416ms | 5,597ms | 0.0% |
| qwen3.5:4b | 100 | 1 | 100.0% | 62,211ms | — | — | 6.0% |
| glm4:9b | 100 | 1 | 100.0% | 3,307ms | — | — | 0.0% |
| qwen2.5-coder:7b | 100 | 1 | 100.0% | 3,861ms | 5,638ms | 5,829ms | 0.0% |

**Key Finding**: All models maintain **100% determinism** under 100-agent concurrent stress. Even with contention, every single agent gets the identical output for the same prompt. This validates Nexus OS's temp=0 + seed=42 determinism protocol.

---

## 4. NVIDIA NIM Multi-Model Comparison (20 representative models)

*NVIDIA NIM models require `NVIDIA_NIM_API_KEY`. The benchmark is wired to test these 20 models:*

| Family | Model | Parameters | Use Case |
|--------|-------|-----------|----------|
| DeepSeek | deepseek-v3_1-terminus | 671B MoE | Best for agents |
| DeepSeek | deepseek-r1 | — | Reasoning |
| DeepSeek | deepseek-r1-distill-llama-8b | 8B | Fast reasoning |
| Meta | llama-4-scout-17b-16e | 17B | Fast general |
| Meta | llama-3.3-70b | 70B | Flagship open |
| Meta | llama-3.1-8b | 8B | Lightweight |
| Qwen | qwen2.5-72b | 72B | Large general |
| Qwen | qwq-32b | 32B | Reasoning |
| Qwen | qwen2.5-coder-32b | 32B | Code specialist |
| Mistral | mistral-large-2 | — | Large flagship |
| Mistral | mamba-codestral-7b | 7B | Mamba code |
| Google | gemma-3-27b | 27B | Vision capable |
| Google | gemma-3-4b | 4B | Ultra-light |
| Zhipu | glm-4.7 | — | Agentic coding |
| Zhipu | glm-5-744b | 744B MoE | Complex reasoning |
| Microsoft | phi-4 | 14B | Smart reasoning |
| Moonshot | kimi-k2 | — | Code + long ctx |
| NVIDIA | nemotron-ultra-253b | 253B | Most capable |
| NVIDIA | nemotron-nano-30b | 30B | Fast agentic |
| IBM | granite-3.3-8b | 8B | Enterprise |

*Set `NVIDIA_NIM_API_KEY` and re-run to benchmark all 20 models with determinism + concurrency tests.*

---

## 5. Agentic Workload Test (decision-making prompts)

| Provider/Model | Prompt | Output | Latency | Valid? |
|---------------|--------|--------|---------|--------|
| **llama3.1:latest** | BUY/SELL/HOLD decision | `HOLD` | 223ms | YES |
| **llama3.1:latest** | Extract action verb | `Create` | 191ms | YES |
| **llama3.1:latest** | Classify priority | `**CRITICAL** This task has...` | 683ms | NOISY |
| **glm4:9b** | BUY/SELL/HOLD decision | `HOLD` | 204ms | YES |
| **glm4:9b** | Extract action verb | — | — | — |
| **glm4:9b** | Classify priority | `CRITICAL` | 189ms | YES |
| **qwen2.5-coder:7b** | BUY/SELL/HOLD decision | `HOLD` | 199ms | YES |
| **qwen2.5-coder:7b** | Extract action verb | `Create` | 171ms | YES |
| **qwen2.5-coder:7b** | Classify priority | `CRITICAL` | 165ms | YES |

**Key Finding**: `qwen2.5-coder:7b` is the **cleanest agentic model** — it produces single-word, parseable action outputs on every prompt. `llama3.1` adds markdown formatting to classification output (noisy). Coder-tuned models are naturally better at structured output extraction.

---

## 6. Cloud Provider Architecture (Ready for Testing)

The benchmark supports all 11 cloud providers. Set the API keys to enable:

| Provider | Env Variable | Default Model | Endpoint | Cost/1M tokens |
|----------|-------------|--------------|----------|----------------|
| DeepSeek | `DEEPSEEK_API_KEY` | deepseek-chat | api.deepseek.com | $2.00 |
| Groq | `GROQ_API_KEY` | llama-3.3-70b-versatile | api.groq.com | $0.60 |
| Mistral | `MISTRAL_API_KEY` | mistral-large-latest | api.mistral.ai | $2.50 |
| Together | `TOGETHER_API_KEY` | Llama-3.3-70B-Instruct-Turbo | api.together.xyz | $1.80 |
| Fireworks | `FIREWORKS_API_KEY` | llama-v3p1-70b-instruct | api.fireworks.ai | $1.20 |
| Perplexity | `PERPLEXITY_API_KEY` | sonar-pro | api.perplexity.ai | $3.00 |
| OpenRouter | `OPENROUTER_API_KEY` | llama-3.3-70b-instruct | openrouter.ai | $2.00 |
| OpenAI | `OPENAI_API_KEY` | gpt-4o-mini | api.openai.com | $5.00 |
| Gemini | `GEMINI_API_KEY` | gemini-2.0-flash | googleapis.com | $3.50 |
| Cohere | `COHERE_API_KEY` | command-r-plus | api.cohere.ai | $3.00 |
| NVIDIA NIM | `NVIDIA_NIM_API_KEY` | deepseek-v3_1-terminus | integrate.api.nvidia.com | $1.00 |
| Ollama | `OLLAMA_URL` | auto-detect | localhost:11434 | $0.00 |

**Cost-optimized routing for Nexus OS agents:**
1. **Free tier**: Ollama local (qwen2.5-coder:7b recommended) — $0/token
2. **Budget cloud**: Groq ($0.60/M) or NVIDIA NIM ($1.00/M, 93 models free tier)
3. **Quality cloud**: DeepSeek ($2.00/M) or Mistral ($2.50/M)
4. **Premium**: OpenAI ($5.00/M) — use only when tool-calling quality is critical

---

## Test Configuration

- Determinism runs per prompt: 10
- Concurrency levels: [50, 100, 500]
- Cross-provider agents per provider: 100
- Max tokens per request: 64
- Temperature: 0.0 (forced deterministic)
- Seed: 42 (where supported)
- Standard test prompts: 5
- Agentic test prompts: 3
- NVIDIA NIM models wired: 20 (representative from each family)

## How to Run

```bash
# Local only (Ollama)
OLLAMA_MODEL=qwen2.5-coder:7b \
  cargo run -p nexus-conductor-benchmark --bin cloud-models-bench --release

# With cloud providers (set any/all keys)
DEEPSEEK_API_KEY=sk-xxx \
GROQ_API_KEY=gsk_xxx \
NVIDIA_NIM_API_KEY=nvapi-xxx \
OLLAMA_MODEL=qwen2.5-coder:7b \
  cargo run -p nexus-conductor-benchmark --bin cloud-models-bench --release

# Full NVIDIA NIM sweep (tests 20 models)
NVIDIA_NIM_API_KEY=nvapi-xxx \
  cargo run -p nexus-conductor-benchmark --bin cloud-models-bench --release
```

---

## Conclusions

1. **Determinism is achievable**: All tested models achieve 98-100% output consistency with temp=0 + seed=42, even under 100-agent concurrent stress.

2. **Best local model for agents: `qwen2.5-coder:7b`** — Perfect determinism (100%), clean agentic outputs (3/3), competitive throughput (17.9 req/s), 151ms P50 latency.

3. **Avoid reasoning models for simple agent tasks**: Qwen 3.5 4B (thinking model) is 20-25x slower than equivalently-sized models and hits 83% error rate at 500 concurrent agents due to timeout.

4. **Throughput scales linearly**: All 7-9B models maintain ~17-20 req/s regardless of concurrency level (50→500), with latency growing linearly. Zero errors at any scale for non-reasoning models.

5. **GLM-4 is fastest but slightly inconsistent**: 142ms P50 (fastest tested) but 90% determinism on ambiguous prompts. Use for speed-critical, non-determinism-sensitive tasks.

6. **Cloud providers extend the same framework**: The benchmark is ready to test all 11 cloud providers + 93 NVIDIA NIM models. Set API keys and re-run for full cross-provider comparison.

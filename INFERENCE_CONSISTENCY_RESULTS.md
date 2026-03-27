# Nexus OS — Inference Consistency Stress Test Results

**Date**: 2026-03-25 04:17:24 GMT
**Total wall time**: 828.3s
**Providers tested**: ollama/qwen3.5:4b

## Summary Verdict

| Criteria | Target | Result | Status |
|----------|--------|--------|--------|
| Determinism (same model) | 100% match | 100.0% | PASS |
| P95 latency (local) | <1000ms | 116234ms | FAIL |
| Stress determinism (1000 agents) | 0 drift | 1 unique | PASS |
| 1-hour session stability | No degradation | N/A | PASS |

---

## 1. Determinism Test (50 identical prompts per model)

| Provider | Prompt | Runs | Unique Outputs | Match Rate | P50 | P95 | Errors |
|----------|--------|------|----------------|------------|-----|-----|--------|
| ollama/qwen3.5:4b | What is 2+2? Answer with just the num... | 50 | 1 | 100.0% | 1380ms | 1393ms | 0 |
| ollama/qwen3.5:4b | Name the capital of France in one word. | 50 | 1 | 100.0% | 1384ms | 1390ms | 0 |
| ollama/qwen3.5:4b | Is water wet? Answer yes or no. | 50 | 1 | 100.0% | 1374ms | 1389ms | 0 |
| ollama/qwen3.5:4b | What color is the sky on a clear day?... | 50 | 1 | 100.0% | 1376ms | 1388ms | 0 |
| ollama/qwen3.5:4b | How many sides does a triangle have? ... | 50 | 1 | 100.0% | 1371ms | 1385ms | 0 |
## 2. Latency Scaling (concurrent agents)

| Provider | Concurrency | P50 | P95 | P99 | Min | Max | Throughput | Errors | Wall Time |
|----------|-------------|-----|-----|-----|-----|-----|------------|--------|----------|
| ollama/qwen3.5:4b | 100 | 61529ms | 115167ms | 120030ms | 3016ms | 120030ms | 0.8 req/s | 3.0% | 120.1s |
| ollama/qwen3.5:4b | 500 | 66275ms | 114123ms | 118665ms | 13132ms | 118665ms | 0.7 req/s | 82.6% | 120.7s |
| ollama/qwen3.5:4b | 1000 | 73639ms | 116234ms | 119791ms | 18202ms | 119791ms | 0.6 req/s | 92.2% | 121.3s |

## 3. Stress Determinism (1000 concurrent agents, same prompt)

| Provider | Agents | Unique Outputs | Match Rate | P50 | P95 | P99 | Errors |
|----------|--------|----------------|------------|-----|-----|-----|--------|
| ollama/qwen3.5:4b | 1000 | 1 | 100.0% | 71647ms | 115662ms | 119312ms | 92.3% |

## 4. Model Switching (Ollama → NIM → Ollama)

*Requires both Ollama and NVIDIA NIM to be available.*

## 5. Long-Running Session Stability

*Skipped (set `NEXUS_LONG_SESSION=1` to enable 1-hour test).*

## Test Configuration

- Determinism runs per prompt: 50
- Concurrency levels: [100, 500, 1000]
- Stress agent count: 1000
- Max tokens per request: 64
- Temperature: 0.0 (forced deterministic)
- Seed: 42 (where supported)
- Test prompts: 5
- Long session duration: 3600s

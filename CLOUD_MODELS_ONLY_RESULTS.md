# Nexus OS — NVIDIA NIM Cloud Models Stress Test Results

**Date**: 2026-03-25 08:02:02 GMT
**Total wall time**: 1658.9s (27.6 minutes)
**Models in catalog**: 88
**Models tested**: 30 (probed 30)
**Models failed probe**: 58
**Rate limit**: 40 model tests/minute
**Determinism runs**: 10 per model
**Concurrency**: 50 agents per model

---

## Executive Summary

### Speed Rankings (P50 latency, single request)

| Rank | Model | Family | P50 (ms) | Rating |
|------|-------|--------|----------|--------|
| 1 | Llama 3.2 3B | meta | 178 | BLAZING |
| 2 | Qwen 2.5 Coder 7B | qwen | 218 | BLAZING |
| 3 | Mamba Codestral 7B | mistral | 219 | BLAZING |
| 4 | Phi-3.5 Vision | microsoft | 219 | BLAZING |
| 5 | Nemotron Mini 4B | nvidia | 223 | BLAZING |
| 6 | Mistral 7B | mistral | 224 | BLAZING |
| 7 | Phi-3.5 Mini 3.8B | microsoft | 227 | BLAZING |
| 8 | Gemma 2 9B | google | 229 | BLAZING |
| 9 | Gemma 2 27B | google | 270 | BLAZING |
| 10 | Llama 3.2 1B | meta | 278 | BLAZING |
| 11 | Phi-3 Medium 14B | microsoft | 307 | FAST |
| 12 | Llama 3.1 8B | meta | 362 | FAST |
| 13 | Llama 4 Maverick 17B | meta | 376 | FAST |
| 14 | Mixtral 8x22B | mistral | 392 | FAST |
| 15 | Llama 3.1 70B | meta | 398 | FAST |

### Determinism Rankings (match rate across 10 identical prompts)

| Rank | Model | Family | Match Rate | Unique Outputs | Errors |
|------|-------|--------|------------|----------------|--------|
| 1 | Llama 3.2 3B | meta | 100.0% | 1 | 0 |
| 2 | Qwen 2.5 Coder 7B | qwen | 100.0% | 1 | 0 |
| 3 | Mamba Codestral 7B | mistral | 100.0% | 1 | 1 |
| 4 | Phi-3.5 Vision | microsoft | 100.0% | 1 | 0 |
| 5 | Nemotron Mini 4B | nvidia | 100.0% | 1 | 0 |
| 6 | Mistral 7B | mistral | 100.0% | 1 | 0 |
| 7 | Phi-3.5 Mini 3.8B | microsoft | 100.0% | 1 | 0 |
| 8 | Gemma 2 9B | google | 100.0% | 1 | 0 |
| 9 | Gemma 2 27B | google | 100.0% | 1 | 0 |
| 10 | Llama 3.2 1B | meta | 100.0% | 1 | 0 |
| 11 | Phi-3 Medium 14B | microsoft | 100.0% | 1 | 0 |
| 12 | Llama 3.1 8B | meta | 100.0% | 1 | 0 |
| 13 | Llama 4 Maverick 17B | meta | 100.0% | 1 | 0 |
| 14 | Mixtral 8x22B | mistral | 100.0% | 1 | 0 |
| 15 | Llama 3.1 70B | meta | 100.0% | 1 | 0 |

### Agentic Accuracy Rankings (clean single-word action outputs)

| Rank | Model | Family | Clean/Total | Valid/Total | Avg Latency |
|------|-------|--------|------------|-----------|-------------|
| 1 | Mamba Codestral 7B | mistral | 5/5 | 5/5 | 326ms |
| 2 | Mistral 7B | mistral | 5/5 | 5/5 | 276ms |
| 3 | Gemma 2 9B | google | 5/5 | 5/5 | 418ms |
| 4 | Llama 3.2 1B | meta | 5/5 | 5/5 | 287ms |
| 5 | Llama 3.1 8B | meta | 5/5 | 5/5 | 377ms |
| 6 | Llama 4 Maverick 17B | meta | 5/5 | 5/5 | 420ms |
| 7 | Qwen 2.5 Coder 32B | qwen | 5/5 | 5/5 | 450ms |
| 8 | Llama 3.3 70B | meta | 5/5 | 5/5 | 1316ms |
| 9 | Qwen 2.5 7B | qwen | 5/5 | 5/5 | 9572ms |
| 10 | Llama 3.2 11B Vision | meta | 5/5 | 5/5 | 10977ms |
| 11 | Gemma 2 27B | google | 4/5 | 5/5 | 396ms |
| 12 | Mixtral 8x22B | mistral | 4/5 | 5/5 | 480ms |
| 13 | Gemma 3 27B | google | 4/5 | 5/5 | 519ms |
| 14 | Devstral 2 123B | mistral | 4/5 | 5/5 | 831ms |
| 15 | Nemotron Ultra 253B | nvidia | 4/5 | 4/5 | 1095ms |

### Throughput Rankings (50 concurrent agents)

| Rank | Model | Family | Throughput | P50 | P95 | Errors |
|------|-------|--------|-----------|-----|-----|--------|
| 1 | Gemma 2 9B | google | 61.1 req/s | 373ms | 406ms | 50.0% |
| 2 | Llama 3.2 3B | meta | 54.0 req/s | 303ms | 368ms | 60.0% |
| 3 | Mistral 7B | mistral | 51.8 req/s | 312ms | 366ms | 62.0% |
| 4 | Nemotron Mini 4B | nvidia | 42.2 req/s | 296ms | 354ms | 70.0% |
| 5 | DeepSeek R1 Distill 8B | deepseek | 39.9 req/s | 1221ms | 1243ms | 0.0% |
| 6 | Phi-3 Medium 14B | microsoft | 38.8 req/s | 442ms | 487ms | 62.0% |
| 7 | Qwen 2.5 Coder 7B | qwen | 38.2 req/s | 301ms | 334ms | 74.0% |
| 8 | Phi-3.5 Mini 3.8B | microsoft | 34.3 req/s | 325ms | 392ms | 72.0% |
| 9 | Gemma 2 27B | google | 31.7 req/s | 351ms | 409ms | 74.0% |
| 10 | Phi-3.5 Vision | microsoft | 26.6 req/s | 386ms | 411ms | 78.0% |
| 11 | Mamba Codestral 7B | mistral | 24.4 req/s | 410ms | 449ms | 78.0% |
| 12 | Llama 3.1 70B | meta | 23.7 req/s | 453ms | 791ms | 62.0% |
| 13 | Gemma 3 27B | google | 20.7 req/s | 522ms | 577ms | 76.0% |
| 14 | Mixtral 8x22B | mistral | 16.7 req/s | 1288ms | 1480ms | 50.0% |
| 15 | Llama 3.1 8B | meta | 14.4 req/s | 608ms | 900ms | 74.0% |

### RECOMMENDED MODELS FOR NEXUS OS

| Rank | Model | Family | Score | P50 | Det% | Agentic | Throughput |
|------|-------|--------|-------|-----|------|---------|------------|
| 1 **PRIMARY** | Mistral 7B | mistral | 13 | 224ms | 100.0% | 5/5 | 51.8 req/s |
| 2 **SECONDARY** | Mamba Codestral 7B | mistral | 14 | 219ms | 100.0% | 5/5 | 24.4 req/s |
| 3 **FALLBACK** | Gemma 2 9B | google | 16 | 229ms | 100.0% | 5/5 | 61.1 req/s |
| 4 | Llama 3.2 3B | meta | 25 | 178ms | 100.0% | 0/5 | 54.0 req/s |
| 5 | Nemotron Mini 4B | nvidia | 27 | 223ms | 100.0% | 3/5 | 42.2 req/s |
| 6 | Phi-3.5 Vision | microsoft | 30 | 219ms | 100.0% | 3/5 | 26.6 req/s |
| 7 | Qwen 2.5 Coder 7B | qwen | 31 | 218ms | 100.0% | 0/5 | 38.2 req/s |
| 8 | Gemma 2 27B | google | 34 | 270ms | 100.0% | 4/5 | 31.7 req/s |
| 9 | Phi-3.5 Mini 3.8B | microsoft | 38 | 227ms | 100.0% | 1/5 | 34.3 req/s |
| 10 | Llama 3.1 8B | meta | 40 | 362ms | 100.0% | 5/5 | 14.4 req/s |

---

## Detailed Results — All Tested Models

| # | Model | Family | Probe | P50 | Det% | Unique | Agentic | Throughput | Avg Tokens |
|---|-------|--------|-------|-----|------|--------|---------|------------|------------|
| 1 | DeepSeek R1 Distill 8B | deepseek | 1057ms | 988ms | 100.0% | 1 | 0/5 | 39.9 req/s | 80 |
| 2 | Llama 4 Maverick 17B | meta | 387ms | 376ms | 100.0% | 1 | 5/5 | 11.4 req/s | 25 |
| 3 | Llama 3.3 70B | meta | 553ms | 614ms | 100.0% | 1 | 5/5 | 2.5 req/s | 50 |
| 4 | Llama 3.1 405B | meta | 513ms | 25445ms | 100.0% | 1 | 1/5 | 7.3 req/s | 50 |
| 5 | Llama 3.1 70B | meta | 383ms | 398ms | 100.0% | 1 | 3/5 | 23.7 req/s | 50 |
| 6 | Llama 3.1 8B | meta | 366ms | 362ms | 100.0% | 1 | 5/5 | 14.4 req/s | 50 |
| 7 | Llama 3.2 90B Vision | meta | 502ms | 484ms | 100.0% | 1 | 0/5 | 9.5 req/s | 50 |
| 8 | Llama 3.2 11B Vision | meta | 15216ms | 14382ms | 100.0% | 1 | 5/5 | 1.4 req/s | 50 |
| 9 | Llama 3.2 3B | meta | 311ms | 178ms | 100.0% | 1 | 0/5 | 54.0 req/s | 50 |
| 10 | Llama 3.2 1B | meta | 348ms | 278ms | 100.0% | 1 | 5/5 | 12.6 req/s | 24 |
| 11 | Nemotron Ultra 253B | nvidia | 534ms | 502ms | 100.0% | 1 | 4/5 | 13.3 req/s | 40 |
| 12 | Nemotron 3 Super 120B | nvidia | 5298ms | 3822ms | 100.0% | 1 | 0/5 | 2.3 req/s | 66 |
| 13 | Nemotron Nano 30B | nvidia | 748ms | 644ms | 100.0% | 1 | 1/5 | 13.6 req/s | 73 |
| 14 | Nemotron Mini 4B | nvidia | 289ms | 223ms | 100.0% | 1 | 3/5 | 42.2 req/s | 31 |
| 15 | Qwen 2.5 7B | qwen | 6189ms | 7313ms | 100.0% | 1 | 5/5 | 3.3 req/s | 43 |
| 16 | Qwen 2.5 Coder 32B | qwen | 429ms | 400ms | 100.0% | 1 | 5/5 | 14.1 req/s | 43 |
| 17 | Qwen 2.5 Coder 7B | qwen | 303ms | 218ms | 100.0% | 1 | 0/5 | 38.2 req/s | 43 |
| 18 | QwQ 32B Reasoning | qwen | 6760ms | 2970ms | 100.0% | 1 | 0/5 | 5.3 req/s | 87 |
| 19 | Mixtral 8x22B | mistral | 547ms | 392ms | 100.0% | 1 | 4/5 | 16.7 req/s | 19 |
| 20 | Mixtral 8x7B | mistral | 1226ms | 1024ms | 100.0% | 1 | 2/5 | 4.1 req/s | 72 |
| 21 | Mistral 7B | mistral | 207ms | 224ms | 100.0% | 1 | 5/5 | 51.8 req/s | 19 |
| 22 | Devstral 2 123B | mistral | 3209ms | 511ms | 100.0% | 1 | 4/5 | 13.8 req/s | 18 |
| 23 | Mamba Codestral 7B | mistral | 322ms | 219ms | 100.0% | 1 | 5/5 | 24.4 req/s | 18 |
| 24 | Gemma 3 27B | google | 721ms | 470ms | 100.0% | 1 | 4/5 | 20.7 req/s | 25 |
| 25 | Gemma 2 27B | google | 262ms | 270ms | 100.0% | 1 | 4/5 | 31.7 req/s | 26 |
| 26 | Gemma 2 9B | google | 284ms | 229ms | 100.0% | 1 | 5/5 | 61.1 req/s | 25 |
| 27 | Phi-3.5 Mini 3.8B | microsoft | 226ms | 227ms | 100.0% | 1 | 1/5 | 34.3 req/s | 19 |
| 28 | Phi-3 Medium 14B | microsoft | 343ms | 307ms | 100.0% | 1 | 0/5 | 38.8 req/s | 18 |
| 29 | Phi-3.5 Vision | microsoft | 305ms | 219ms | 100.0% | 1 | 3/5 | 26.6 req/s | 24 |
| 30 | Kimi K2 | moonshot | 606ms | 614ms | 100.0% | 1 | 1/5 | 7.3 req/s | 31 |

## Agentic Workload Detail

### DeepSeek R1 Distill 8B (deepseek)

| Task | Output | Latency | Valid | Clean |
|------|--------|---------|-------|-------|
| trade | `<think> Okay, so I need to figure out whet...` | 915ms | NO | NO |
| verb | `<think> Okay, so I need to figure out how ...` | 1007ms | NO | NO |
| priority | `<think> Okay, so I need to figure out the ...` | 1019ms | NO | NO |
| anomaly | `<think> Okay, so I'm trying to figure out ...` | 936ms | NO | NO |
| json | `<think> Okay, so I need to figure out how ...` | 1014ms | NO | NO |

### Llama 4 Maverick 17B (meta)

| Task | Output | Latency | Valid | Clean |
|------|--------|---------|-------|-------|
| trade | `BUY` | 450ms | YES | YES |
| verb | `create` | 378ms | YES | YES |
| priority | `CRITICAL.` | 394ms | YES | YES |
| anomaly | `SUSPICIOUS` | 398ms | YES | YES |
| json | `{"action": "buy", "confidence": 0.85}` | 477ms | YES | YES |

### Llama 3.3 70B (meta)

| Task | Output | Latency | Valid | Clean |
|------|--------|---------|-------|-------|
| trade | `BUY.` | 428ms | YES | YES |
| verb | `create` | 4507ms | YES | YES |
| priority | `CRITICAL` | 518ms | YES | YES |
| anomaly | `SUSPICIOUS.` | 453ms | YES | YES |
| json | `{"action": "buy", "confidence": 0.85}` | 675ms | YES | YES |

### Llama 3.1 405B (meta)

| Task | Output | Latency | Valid | Clean |
|------|--------|---------|-------|-------|
| trade | `` | 111ms | NO | NO |
| verb | `create` | 29439ms | YES | YES |
| priority | `` | 111ms | NO | NO |
| anomaly | `` | 110ms | NO | NO |
| json | `` | 118ms | NO | NO |

### Llama 3.1 70B (meta)

| Task | Output | Latency | Valid | Clean |
|------|--------|---------|-------|-------|
| trade | `` | 112ms | NO | NO |
| verb | `` | 111ms | NO | NO |
| priority | `CRITICAL` | 411ms | YES | YES |
| anomaly | `SUSPICIOUS` | 412ms | YES | YES |
| json | `{"action": "buy", "confidence": 0.85}` | 567ms | YES | YES |

### Llama 3.1 8B (meta)

| Task | Output | Latency | Valid | Clean |
|------|--------|---------|-------|-------|
| trade | `BUY` | 354ms | YES | YES |
| verb | `create` | 358ms | YES | YES |
| priority | `CRITICAL` | 372ms | YES | YES |
| anomaly | `SUSPICIOUS.` | 382ms | YES | YES |
| json | `{"action": "buy", "confidence": 0.85}` | 420ms | YES | YES |

### Llama 3.2 90B Vision (meta)

| Task | Output | Latency | Valid | Clean |
|------|--------|---------|-------|-------|
| trade | `` | 112ms | NO | NO |
| verb | `` | 124ms | NO | NO |
| priority | `` | 113ms | NO | NO |
| anomaly | `` | 112ms | NO | NO |
| json | `` | 113ms | NO | NO |

### Llama 3.2 11B Vision (meta)

| Task | Output | Latency | Valid | Clean |
|------|--------|---------|-------|-------|
| trade | `BUY` | 7730ms | YES | YES |
| verb | `create` | 14950ms | YES | YES |
| priority | `CRITICAL` | 8397ms | YES | YES |
| anomaly | `SUSPICIOUS.` | 16006ms | YES | YES |
| json | `{"action": "buy", "confidence": 0.85}` | 7803ms | YES | YES |

### Llama 3.2 3B (meta)

| Task | Output | Latency | Valid | Clean |
|------|--------|---------|-------|-------|
| trade | `` | 113ms | NO | NO |
| verb | `` | 119ms | NO | NO |
| priority | `` | 108ms | NO | NO |
| anomaly | `` | 112ms | NO | NO |
| json | `` | 121ms | NO | NO |

### Llama 3.2 1B (meta)

| Task | Output | Latency | Valid | Clean |
|------|--------|---------|-------|-------|
| trade | `HOLD` | 264ms | YES | YES |
| verb | `create` | 308ms | YES | YES |
| priority | `CRITICAL.` | 291ms | YES | YES |
| anomaly | `SUSPICIOUS` | 237ms | YES | YES |
| json | `{   "action": "buy",   "confidence": 0.85 }` | 337ms | YES | YES |

### Nemotron Ultra 253B (nvidia)

| Task | Output | Latency | Valid | Clean |
|------|--------|---------|-------|-------|
| trade | `HOLD` | 526ms | YES | YES |
| verb | `create` | 593ms | YES | YES |
| priority | `` | 2827ms | NO | NO |
| anomaly | `SUSPICIOUS` | 607ms | YES | YES |
| json | `{"action": "buy", "confidence": 0.85}` | 925ms | YES | YES |

### Nemotron 3 Super 120B (nvidia)

| Task | Output | Latency | Valid | Clean |
|------|--------|---------|-------|-------|
| trade | `` | 139ms | NO | NO |
| verb | `` | 145ms | NO | NO |
| priority | `` | 110ms | NO | NO |
| anomaly | `` | 113ms | NO | NO |
| json | `` | 111ms | NO | NO |

### Nemotron Nano 30B (nvidia)

| Task | Output | Latency | Valid | Clean |
|------|--------|---------|-------|-------|
| trade | `` | 750ms | NO | NO |
| verb | `` | 777ms | NO | NO |
| priority | `CRITICAL` | 623ms | YES | YES |
| anomaly | `` | 923ms | NO | NO |
| json | `{"action": "buy", "confidence": 0.85` | 1419ms | NO | NO |

### Nemotron Mini 4B (nvidia)

| Task | Output | Latency | Valid | Clean |
|------|--------|---------|-------|-------|
| trade | `` | 128ms | NO | NO |
| verb | `The verb in the sentence "Please create a ...` | 357ms | NO | NO |
| priority | `CRITICAL` | 175ms | YES | YES |
| anomaly | `SUSPICIOUS` | 194ms | YES | YES |
| json | `{"action": "buy", "confidence": 0.85}` | 319ms | YES | YES |

### Qwen 2.5 7B (qwen)

| Task | Output | Latency | Valid | Clean |
|------|--------|---------|-------|-------|
| trade | `BUY` | 4399ms | YES | YES |
| verb | `create` | 8229ms | YES | YES |
| priority | `CRITICAL` | 15333ms | YES | YES |
| anomaly | `SUSPICIOUS` | 10748ms | YES | YES |
| json | `{"action": "buy", "confidence": 0.85}` | 9153ms | YES | YES |

### Qwen 2.5 Coder 32B (qwen)

| Task | Output | Latency | Valid | Clean |
|------|--------|---------|-------|-------|
| trade | `BUY` | 368ms | YES | YES |
| verb | `create` | 389ms | YES | YES |
| priority | `CRITICAL` | 400ms | YES | YES |
| anomaly | `SUSPICIOUS` | 415ms | YES | YES |
| json | `{"action": "buy", "confidence": 0.85}` | 676ms | YES | YES |

### Qwen 2.5 Coder 7B (qwen)

| Task | Output | Latency | Valid | Clean |
|------|--------|---------|-------|-------|
| trade | `` | 137ms | NO | NO |
| verb | `` | 123ms | NO | NO |
| priority | `` | 110ms | NO | NO |
| anomaly | `` | 112ms | NO | NO |
| json | ````json {"action": "buy", "confidence": 0....` | 676ms | YES | NO |

### QwQ 32B Reasoning (qwen)

| Task | Output | Latency | Valid | Clean |
|------|--------|---------|-------|-------|
| trade | `Okay, so the user is asking whether to buy...` | 6850ms | NO | NO |
| verb | `Okay, the user wants me to extract the ver...` | 2970ms | NO | NO |
| priority | `Okay, so I need to classify the priority o...` | 2898ms | NO | NO |
| anomaly | `Okay, let's see. The user logged in from I...` | 2838ms | NO | NO |
| json | `Okay, the user wants me to return only a v...` | 6902ms | NO | NO |

### Mixtral 8x22B (mistral)

| Task | Output | Latency | Valid | Clean |
|------|--------|---------|-------|-------|
| trade | `HOLD, as the price increase already reflec...` | 563ms | YES | NO |
| verb | `Create` | 388ms | YES | YES |
| priority | `CRITICAL` | 388ms | YES | YES |
| anomaly | `Suspicious.` | 403ms | YES | YES |
| json | `{"action": "buy", "confidence": 0.85}` | 655ms | YES | YES |

### Mixtral 8x7B (mistral)

| Task | Output | Latency | Valid | Clean |
|------|--------|---------|-------|-------|
| trade | `HOLD  Here's why:  Although the company, X...` | 1203ms | YES | NO |
| verb | `Create` | 280ms | YES | YES |
| priority | `CRITICAL  A server outage affecting all cu...` | 1051ms | YES | NO |
| anomaly | `Suspicious.  The reason for this assessmen...` | 1124ms | YES | NO |
| json | `{ "action": "buy", "confidence": 0.85 }` | 619ms | YES | YES |

### Mistral 7B (mistral)

| Task | Output | Latency | Valid | Clean |
|------|--------|---------|-------|-------|
| trade | `HOLD` | 222ms | YES | YES |
| verb | `Create` | 188ms | YES | YES |
| priority | `CRITICAL` | 228ms | YES | YES |
| anomaly | `NORMAL` | 218ms | YES | YES |
| json | `{"action": "buy", "confidence": 0.85}` | 524ms | YES | YES |

### Devstral 2 123B (mistral)

| Task | Output | Latency | Valid | Clean |
|------|--------|---------|-------|-------|
| trade | `HOLD` | 429ms | YES | YES |
| verb | `create` | 426ms | YES | YES |
| priority | `CRITICAL` | 635ms | YES | YES |
| anomaly | `SUSPICIOUS` | 1764ms | YES | YES |
| json | ````json {"action": "buy", "confidence": 0....` | 900ms | YES | NO |

### Mamba Codestral 7B (mistral)

| Task | Output | Latency | Valid | Clean |
|------|--------|---------|-------|-------|
| trade | `HOLD` | 220ms | YES | YES |
| verb | `create` | 199ms | YES | YES |
| priority | `CRITICAL` | 253ms | YES | YES |
| anomaly | `NORMAL` | 235ms | YES | YES |
| json | `{"action": "buy", "confidence": 0.85}` | 723ms | YES | YES |

### Gemma 3 27B (google)

| Task | Output | Latency | Valid | Clean |
|------|--------|---------|-------|-------|
| trade | `BUY` | 401ms | YES | YES |
| verb | `create` | 412ms | YES | YES |
| priority | `CRITICAL` | 436ms | YES | YES |
| anomaly | `SUSPICIOUS` | 441ms | YES | YES |
| json | ````json {"action": "buy", "confidence": 0....` | 907ms | YES | NO |

### Gemma 2 27B (google)

| Task | Output | Latency | Valid | Clean |
|------|--------|---------|-------|-------|
| trade | `HOLD` | 325ms | YES | YES |
| verb | `create` | 268ms | YES | YES |
| priority | `CRITICAL` | 308ms | YES | YES |
| anomaly | `SUSPICIOUS` | 356ms | YES | YES |
| json | ````json {"action": "buy", "confidence": 0....` | 723ms | YES | NO |

### Gemma 2 9B (google)

| Task | Output | Latency | Valid | Clean |
|------|--------|---------|-------|-------|
| trade | `HOLD` | 286ms | YES | YES |
| verb | `create` | 429ms | YES | YES |
| priority | `CRITICAL` | 411ms | YES | YES |
| anomaly | `NORMAL` | 356ms | YES | YES |
| json | `{"action": "buy", "confidence": 0.85}` | 609ms | YES | YES |

### Phi-3.5 Mini 3.8B (microsoft)

| Task | Output | Latency | Valid | Clean |
|------|--------|---------|-------|-------|
| trade | `` | 110ms | NO | NO |
| verb | `` | 115ms | NO | NO |
| priority | `` | 113ms | NO | NO |
| anomaly | `` | 114ms | NO | NO |
| json | `{"action": "buy", "confidence": 0.85}` | 678ms | YES | YES |

### Phi-3 Medium 14B (microsoft)

| Task | Output | Latency | Valid | Clean |
|------|--------|---------|-------|-------|
| trade | `` | 106ms | NO | NO |
| verb | `` | 160ms | NO | NO |
| priority | `` | 110ms | NO | NO |
| anomaly | `` | 112ms | NO | NO |
| json | `` | 111ms | NO | NO |

### Phi-3.5 Vision (microsoft)

| Task | Output | Latency | Valid | Clean |
|------|--------|---------|-------|-------|
| trade | `` | 109ms | NO | NO |
| verb | `create` | 190ms | YES | YES |
| priority | `CRITICAL` | 217ms | YES | YES |
| anomaly | `SUSPICIOUS` | 257ms | YES | YES |
| json | `` | 113ms | NO | NO |

### Kimi K2 (moonshot)

| Task | Output | Latency | Valid | Clean |
|------|--------|---------|-------|-------|
| trade | `HOLD` | 734ms | YES | YES |
| verb | `` | 113ms | NO | NO |
| priority | `` | 112ms | NO | NO |
| anomaly | `` | 110ms | NO | NO |
| json | `` | 110ms | NO | NO |

## Failed Models (did not respond to probe)

| Model | Family | Error |
|-------|--------|-------|
| DeepSeek V3.1 Terminus 671B | deepseek | supervisor error: NIM status 404: unknown |
| DeepSeek V3.1 | deepseek | supervisor error: NIM status 404: unknown |
| DeepSeek V3 | deepseek | supervisor error: NIM status 404: unknown |
| DeepSeek R1 Reasoning | deepseek | supervisor error: NIM status 410: The model 'deepseek-ai/deepseek-r1' has rea... |
| DeepSeek R1 Distill 70B | deepseek | supervisor error: NIM status 404: unknown |
| DeepSeek R1 Distill Qwen 32B | deepseek | supervisor error: NIM status 502: unknown |
| DeepSeek R1 Distill Qwen 14B | deepseek | supervisor error: NIM status 500: unknown |
| DeepSeek Coder V2 236B | deepseek | supervisor error: NIM status 404: unknown |
| DeepSeek Coder V2 Lite 16B | deepseek | supervisor error: NIM status 404: unknown |
| Llama 4 Scout 17B | meta | supervisor error: NIM status 404: Function 'b6bb6e01-780e-4ba0-a5b8-379f00ed9... |
| CodeLlama 70B | meta | supervisor error: NIM status 404: unknown |
| Llama Guard 3 8B | meta | supervisor error: NIM status 404: unknown |
| Nemotron 70B | nvidia | supervisor error: NIM status 404: Function '9b96341b-9791-4db9-a00d-4e43aa192... |
| Nemotron 4 340B | nvidia | supervisor error: NIM status 404: Function 'b0fcd392-e905-4ab4-8eb9-aeae95c30... |
| Nemotron 51B | nvidia | supervisor error: NIM status 404: Function '5beba52c-65a9-4f46-8cd9-656689a1b... |
| USDCode 70B | nvidia | supervisor error: NIM status 404: unknown |
| Qwen 3.5 VL 400B | qwen | supervisor error: NIM status 404: unknown |
| Qwen 2.5 72B | qwen | supervisor error: NIM status 404: unknown |
| Qwen 2.5 32B | qwen | supervisor error: NIM status 404: unknown |
| Qwen 2.5 14B | qwen | supervisor error: NIM status 404: unknown |
| Qwen 2 VL 72B | qwen | supervisor error: NIM status 404: unknown |
| Qwen 2 VL 7B | qwen | supervisor error: NIM status 404: unknown |
| Qwen 2.5 1.5B | qwen | supervisor error: NIM status 404: unknown |
| Qwen 2.5 Math 72B | qwen | supervisor error: NIM status 404: unknown |
| Mistral Large 2 | mistral | supervisor error: NIM status 404: unknown |
| Mistral Small 24B | mistral | supervisor error: NIM status 404: unknown |
| Codestral 22B | mistral | supervisor error: NIM status 404: Function id '9a10b012-e6df-46fd-83b2-700dcb... |
| Gemma 3 12B | google | supervisor error: NIM status 422: unknown |
| Gemma 3 4B | google | supervisor error: NIM status 422: unknown |
| CodeGemma 7B | google | supervisor error: NIM status 404: Function '7dfc10a8-3cc4-448e-97c1-2213308dc... |
| Phi-4 14B | microsoft | supervisor error: NIM status 404: unknown |
| Phi-4 Mini | microsoft | supervisor error: curl failed |
| Phi-3.5 MoE 42B | microsoft | supervisor error: NIM status 404: Function 'e6cab982-62f4-481e-9a7a-3dedb87db... |
| GLM-4.7 | zhipu | supervisor error: NIM status 404: unknown |
| GLM-5 744B | zhipu | supervisor error: NIM status 404: unknown |
| GLM-4 9B | zhipu | supervisor error: NIM status 404: unknown |
| CodeGeeX 4 9B | zhipu | supervisor error: NIM status 404: unknown |
| Granite 3.1 8B | ibm | supervisor error: NIM status 404: unknown |
| Granite 3.3 8B | ibm | supervisor error: curl failed |
| Granite 3.1 2B | ibm | supervisor error: NIM status 404: unknown |
| Granite 34B Code | ibm | supervisor error: NIM status 404: Function '4df48b4f-e3c5-4ade-82c7-c06b65e25... |
| Granite Guardian 8B | ibm | supervisor error: NIM status 404: unknown |
| Kimi VL A3B | moonshot | supervisor error: NIM status 404: unknown |
| MiniMax M2.5 230B | minimax | supervisor error: NIM status 404: unknown |
| MiniMax M1 80B | minimax | supervisor error: NIM status 404: unknown |
| Palmyra X 004 | writer | supervisor error: NIM status 404: unknown |
| Palmyra Fin 70B | writer | supervisor error: NIM status 404: Function '316490c6-f1ed-41f9-9da8-3fa9e8856... |
| DBRX 132B MoE | databricks | supervisor error: NIM status 404: Function '3d6c2ff8-8bfc-4d10-8fd0-b7337288e... |
| Dolly V2 12B | databricks | supervisor error: NIM status 404: unknown |
| Command R+ 2024 | cohere | supervisor error: NIM status 404: unknown |
| Command R 2024 | cohere | supervisor error: NIM status 404: unknown |
| Arctic 480B MoE | snowflake | supervisor error: NIM status 404: unknown |
| Hermes 3 70B | nous | supervisor error: NIM status 404: unknown |
| Hermes 3 8B | nous | supervisor error: NIM status 404: unknown |
| Solar 10.7B | upstage | supervisor error: NIM status 404: unknown |
| Solar Pro Preview | upstage | supervisor error: NIM status 404: unknown |
| Jamba 1.5 Large 398B | ai21 | supervisor error: NIM status 404: unknown |
| Jamba 1.5 Mini | ai21 | supervisor error: NIM status 404: unknown |

## Cost Analysis (NVIDIA NIM Free Tier)

All models tested on NVIDIA NIM free tier (1000 credits on signup).

| Model | Avg Tokens/Req | Est. Requests/Credit | Notes |
|-------|---------------|---------------------|-------|
| DeepSeek R1 Distill 8B | 80 | ~12 | Free tier |
| Llama 4 Maverick 17B | 25 | ~40 | Free tier |
| Llama 3.3 70B | 50 | ~20 | Free tier |
| Llama 3.1 405B | 50 | ~20 | Free tier |
| Llama 3.1 70B | 50 | ~20 | Free tier |
| Llama 3.1 8B | 50 | ~20 | Free tier |
| Llama 3.2 90B Vision | 50 | ~20 | Free tier |
| Llama 3.2 11B Vision | 50 | ~20 | Free tier |
| Llama 3.2 3B | 50 | ~20 | Free tier |
| Llama 3.2 1B | 24 | ~42 | Free tier |
| Nemotron Ultra 253B | 40 | ~25 | Free tier |
| Nemotron 3 Super 120B | 66 | ~15 | Free tier |
| Nemotron Nano 30B | 73 | ~14 | Free tier |
| Nemotron Mini 4B | 31 | ~32 | Free tier |
| Qwen 2.5 7B | 43 | ~23 | Free tier |
| Qwen 2.5 Coder 32B | 43 | ~23 | Free tier |
| Qwen 2.5 Coder 7B | 43 | ~23 | Free tier |
| QwQ 32B Reasoning | 87 | ~11 | Free tier |
| Mixtral 8x22B | 19 | ~53 | Free tier |
| Mixtral 8x7B | 72 | ~14 | Free tier |
| Mistral 7B | 19 | ~53 | Free tier |
| Devstral 2 123B | 18 | ~56 | Free tier |
| Mamba Codestral 7B | 18 | ~56 | Free tier |
| Gemma 3 27B | 25 | ~40 | Free tier |
| Gemma 2 27B | 26 | ~38 | Free tier |
| Gemma 2 9B | 25 | ~40 | Free tier |
| Phi-3.5 Mini 3.8B | 19 | ~53 | Free tier |
| Phi-3 Medium 14B | 18 | ~56 | Free tier |
| Phi-3.5 Vision | 24 | ~42 | Free tier |
| Kimi K2 | 31 | ~32 | Free tier |

## Test Configuration

- Rate limit: 40 model tests/minute
- Determinism runs per model: 10
- Determinism prompt: "What is 2+2? Answer with just the number."
- Concurrency level: 50 agents
- Agentic tasks: 5
- Max tokens: 64
- Temperature: 0.0
- Seed: 42
- Timeout: 180s

## How to Run

```bash
# Full catalog (93 models, ~25 minutes with rate limiting)
NVIDIA_NIM_API_KEY=nvapi-xxx \
  cargo run -p nexus-conductor-benchmark --bin nim-cloud-bench --release

# Quick test (first 5 models)
NVIDIA_NIM_API_KEY=nvapi-xxx NIM_MODELS=5 \
  cargo run -p nexus-conductor-benchmark --bin nim-cloud-bench --release
```

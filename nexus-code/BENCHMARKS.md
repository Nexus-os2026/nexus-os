# Nexus Code Benchmarks

## SWE-bench Verified Results

| Provider | Model | Pass Rate | Avg Fuel | Avg Turns | Avg Time |
|---|---|---|---|---|---|
| anthropic | claude-sonnet-4 | TBD | TBD | TBD | TBD |
| openai | gpt-4o | TBD | TBD | TBD | TBD |
| ollama | qwen3:8b | TBD | TBD | TBD | TBD |

## Governance Metrics

Unlike other coding agents, Nexus Code tracks governance metrics per task:

- **Fuel consumed** — normalized token cost across providers
- **Audit trail length** — number of cryptographically signed actions
- **Tool usage profile** — which tools the agent chose and how often
- **Behavioral envelope** — drift detection status throughout the run

## Reproducing

```bash
# Download SWE-bench Verified tasks
wget https://raw.githubusercontent.com/princeton-nlp/SWE-bench/main/swe-bench-verified.jsonl

# Run single provider
nx bench run --tasks-file swe-bench-verified.jsonl --limit 50 --fuel 20000

# Compare providers
nx bench compare \
  --tasks-file swe-bench-verified.jsonl \
  --providers anthropic/claude-sonnet-4 openai/gpt-4o ollama/qwen3:8b \
  --limit 20
```

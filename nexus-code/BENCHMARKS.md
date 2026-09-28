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

## Reproducing (withdrawn)

These benchmarks ran through the standalone `nx` terminal (`nx bench`) and
`scripts/run_benchmarks.sh`. Both are withdrawn during Phase Zero: `nx` only
prints a withdrawal message and exits with status 69, and the script runs
nothing. The repository provides no supported way to reproduce these runs at
this point. The benchmark code remains in the `nexus_code::bench` library,
which is not claimed governed.

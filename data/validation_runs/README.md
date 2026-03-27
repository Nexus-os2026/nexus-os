# Validation Run Data

## Files
- `real-battery-baseline.json` — 54 agents x 20 problems, keyword scoring, Groq Llama 3.1 8B
- `run1-pre-bugfix-baseline.json` — 54 agents, pre-bugfix (isRealAgent=false), synthetic scoring
- `run2-post-bugfix.json` — 54 agents, post-bugfix, synthetic scoring

## Notes
- LLM-as-judge run requires GROQ_API_KEY with sufficient rate limits (Dev plan recommended)
- To re-run: `GROQ_API_KEY=xxx cargo run -p nexus-conductor-benchmark --bin real-battery-validation --release`

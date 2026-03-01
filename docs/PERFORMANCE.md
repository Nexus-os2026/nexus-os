# Performance Notes (v1.0.0)

## Governance Overhead Benchmark

The governance benchmark runs in the integration suite:
- `tests/integration/tests/governance_benchmark.rs`

It compares:
- baseline LLM provider execution (no governance wrapper)
- governed execution (`GovernedLlmGateway`) including capability checks and audit logging

Target:
- `< 5%` overhead from governance layer (`ratio <= 1.05`)

Run:
```bash
cargo test -p nexus-integration --test governance_benchmark -- --nocapture
```

## Hot Path Optimization

`kernel/audit.rs` hash serialization now uses the direct `unwrap_or_default()` fast path to
reduce branching and simplify the event-hash encoding path.

# Nexus OS Benchmark Baselines

Measured on: 2026-03-10
Platform: Linux 6.17.0-14-generic
Profile: `bench` (optimized)
Tool: Criterion 0.5

## Phase 6-7 Feature Benchmarks (`phase67_bench`)

### WASM Sandbox

| Benchmark | Mean | Notes |
|---|---|---|
| wasm_sandbox_startup | 1.397 us | `WasmtimeSandbox::with_defaults()` instantiation |
| wasm_module_cache/cache_miss | 21.82 us | SHA-256 hash + wasmtime compile |
| wasm_module_cache/cache_hit | 1.158 us | HashMap lookup by content hash |

Cache hit is 18.8x faster than cache miss (compile avoided).

### Speculative Execution

| Benchmark | Mean | Notes |
|---|---|---|
| speculation_overhead | 3.175 us | fork_state + simulate + commit full cycle |

Shadow sandbox overhead is negligible per operation (~3 us).

### Prompt Firewall

| Benchmark | Mean | Throughput (prompts/sec) | Notes |
|---|---|---|---|
| prompt_firewall/clean_input | 3.437 us | ~291k/s | 20 injection patterns + unicode + PII scan |
| prompt_firewall/malicious_input | 2.287 us | ~437k/s | Early exit on injection match |
| prompt_firewall/pii_input | 5.346 us | ~187k/s | PII redaction path (email, phone, card, key) |
| prompt_firewall/output_check | 2.250 us | ~444k/s | Exfiltration pattern + schema validation |

Firewall throughput exceeds 180k prompts/sec even on PII-heavy inputs.

### PII Redaction

| Benchmark | Mean | Notes |
|---|---|---|
| pii_redaction/scan_dense_pii | 1.901 us | Regex scan for email, phone, card, API key |
| pii_redaction/apply_dense_pii | 152.7 ns | String replacement of 4+ findings |
| pii_redaction/process_prompt_full_pipeline | 9.276 us | scan + minimize + apply + hash + envelope |

### JWT Identity (EdDSA / Ed25519)

| Benchmark | Mean | Throughput | Notes |
|---|---|---|---|
| jwt_token/issue | 18.13 us | ~55k tokens/s | Ed25519 sign + base64url encode |
| jwt_token/validate | 31.95 us | ~31k tokens/s | Ed25519 verify + claims decode |
| jwt_token/issue_and_validate_roundtrip | 51.63 us | ~19k roundtrips/s | Full issue-then-validate cycle |

### A2A Protocol

| Benchmark | Mean | Notes |
|---|---|---|
| a2a_agent_card_generation | 813 ns | AgentCard::from_manifest with 3 capabilities |

### MCP Protocol

| Benchmark | Mean | Notes |
|---|---|---|
| mcp_tool_invocation_governed | 8.615 us | Full governance pipeline: capability check + fuel + egress + audit |

### Compliance

| Benchmark | Mean | Notes |
|---|---|---|
| compliance_report/empty_trail | 402.8 ns | TransparencyReport with no audit events |
| compliance_report/100_events | 7.053 us | TransparencyReport scanning 100 audit events |

### Audit Block Creation

| Events | Mean | Per-event | Notes |
|---|---|---|---|
| 1 | 1.604 us | 1.604 us | Single append + verify |
| 10 | 16.70 us | 1.670 us | SHA-256 hash chain |
| 100 | 170.6 us | 1.706 us | Linear scaling confirmed |
| 1000 | 1.723 ms | 1.723 us | Consistent ~1.7 us/event |

Audit throughput: ~580k events/sec (append + hash chain integrity verify).

## Existing Benchmarks (for reference)

Run via: `cargo bench --bench kernel_bench`, `cargo bench --bench gateway_bench`, etc.

See `target/criterion/` for HTML reports with graphs after running benchmarks.

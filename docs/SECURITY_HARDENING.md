# Security Hardening Guide

> Checklist and recommendations for hardening Nexus OS deployments.

## Core Security Model

Nexus OS enforces defense-in-depth through layered governance:

1. **Capability-based access** - Agents can only perform actions declared in their manifest
2. **Fuel metering** - Every action has a cost; exhausted agents are stopped
3. **Append-only audit** - Hash-chained audit trail cannot be tampered with
4. **Autonomy gates** - Actions gated by autonomy level with automatic downgrade on violation
5. **HITL approval** - Sensitive operations require human approval
6. **Zero unsafe Rust** - `unsafe_code = "forbid"` across the entire workspace

## Hardening Checklist

### 1. Manifest Security

| Recommendation | Priority | Verification |
|---------------|----------|--------------|
| Grant minimum capabilities per agent | Critical | `grep capabilities agents/*/manifest.toml` |
| Set fuel budgets below max (1,000,000) | High | `grep fuel_budget agents/*/manifest.toml` |
| Start new agents at L0 or L1 | Critical | `grep autonomy_level agents/*/manifest.toml` |
| Set monthly fuel caps | High | `grep monthly_fuel_cap agents/*/manifest.toml` |
| Review consent policies | High | `ls agents/*/consent.toml` |

**Why**: An over-permissioned agent with a high autonomy level can take actions without human review. Always start restrictive and promote through adaptive governance.

### 2. Audit Trail Integrity

| Recommendation | Priority | Verification |
|---------------|----------|--------------|
| Verify hash chain regularly | Critical | `nexus audit verify` |
| Export audit logs to external storage | High | `nexus audit export --format json` |
| Enable audit replication (cluster) | High | `nexus audit federation-status` |
| Monitor for gaps in audit sequence | Medium | Review audit event IDs for continuity |

**Why**: The audit trail is the evidence base for governance compliance. If it can be tampered with, all governance guarantees are void.

### 3. Autonomy Level Management

| Recommendation | Priority | Verification |
|---------------|----------|--------------|
| No agent starts above L2 without review | Critical | Manual review |
| Configure promotion thresholds conservatively | High | Review `AdaptivePolicy` settings |
| Set violation cooldown periods | High | Check `cooldown_after_violation_secs` |
| Monitor for unexpected demotions | Medium | `nexus audit show --type StateChange` |

**Why**: Autonomy levels control how much an agent can do without human oversight. L3+ agents act without approval.

### 4. Fuel Budget Protection

| Recommendation | Priority | Verification |
|---------------|----------|--------------|
| Set per-run budgets appropriate to task | High | Review `fuel_budget` in manifests |
| Enable burn anomaly detection | High | Check `BurnAnomalyDetector` config |
| Set monthly caps for production agents | High | Review `monthly_fuel_cap` in manifests |
| Monitor fuel efficiency trends | Medium | Review `AgentTrackRecord.fuel_efficiency` |

**Why**: Fuel is the economic throttle on agent behavior. Without limits, a runaway agent can consume unbounded resources.

### 5. Delegation Security

| Recommendation | Priority | Verification |
|---------------|----------|--------------|
| Limit delegation depth to 1-2 | High | Check `DelegationConstraints.max_depth` |
| Set delegation timeouts | High | Check `max_duration_secs` |
| Cap delegated fuel budgets | High | Check `DelegationConstraints.max_fuel` |
| Require approval for delegation chains | Critical | Set `require_approval: true` |

**Why**: Transitive delegation can amplify permissions. An agent delegating to another agent that delegates further creates a chain that's hard to audit.

### 6. Network Security (Cluster)

| Recommendation | Priority | Verification |
|---------------|----------|--------------|
| Use TLS for all node-to-node communication | Critical | Review transport config |
| Authenticate nodes with Ed25519 keys | Critical | Check `AuthChallenge`/`AuthResponse` flow |
| Limit cluster membership to known nodes | High | Review membership configuration |
| Monitor for suspect/down nodes | Medium | `nexus cluster status` |

**Why**: An unauthenticated node joining the cluster can inject false audit events or vote in quorum decisions.

### 7. LLM Gateway Security

| Recommendation | Priority | Verification |
|---------------|----------|--------------|
| Enable PII redaction at gateway boundary | Critical | Review `privacy` module config |
| Validate LLM responses before acting | High | Review agent `execute()` implementations |
| Set max token limits per query | High | Review `llm_query` calls |
| Log all LLM interactions in audit trail | High | Verify `EventType::LlmCall` events |

**Why**: LLM inputs and outputs can contain sensitive data. PII must be stripped before reaching external APIs.

### 8. Build and Supply Chain

| Recommendation | Priority | Verification |
|---------------|----------|--------------|
| Build with `unsafe_code = "forbid"` | Critical | `grep unsafe_code Cargo.toml` (workspace-level) |
| Run clippy with `-D warnings` | High | `cargo clippy --workspace -- -D warnings` |
| Verify marketplace bundles before install | High | `nexus marketplace install --verify` |
| Pin dependency versions | Medium | Review `Cargo.lock` |
| Audit dependencies regularly | Medium | `cargo audit` |

**Why**: Supply chain attacks through dependencies are a real threat. The `unsafe_code = "forbid"` lint ensures no unsafe Rust enters the codebase.

### 9. Kill Gates

| Recommendation | Priority | Verification |
|---------------|----------|--------------|
| Test emergency kill gates | Critical | Trigger kill gate in test environment |
| Ensure kill gates work at all autonomy levels | Critical | Test with L4/L5 agents |
| Monitor kill gate activation in audit trail | High | Review kill gate events |

**Why**: Kill gates are the last line of defense. If they don't work, there's no way to stop a misbehaving autonomous agent.

## Verification Commands

Run these commands regularly:

```bash
# Verify audit chain integrity
nexus audit verify

# Check compliance status
nexus compliance status

# View cluster health
nexus cluster status

# Check for fuel anomalies
nexus benchmark report

# Run full test suite
cargo test --workspace --all-features

# Check for unsafe code
cargo clippy --workspace --all-targets --all-features -- -D warnings

# Verify formatting
cargo fmt --all -- --check
```

## Incident Response

### Agent Misbehavior

1. Check audit trail: `nexus audit show --agent <agent-id>`
2. Review trust score: Check adaptive policy for the agent
3. Demote autonomy level: Lower to L0 (Inert)
4. Revoke delegations: `nexus delegation revoke --agent <agent-id>`
5. Review and patch: Investigate root cause before re-enabling

### Audit Chain Corruption

1. Stop all agents immediately via kill gates
2. Export last known good audit state
3. Compare with replicated copies on other cluster nodes
4. Restore from federation replica if available
5. Re-verify chain integrity

### Cluster Partition

1. Check quorum status: `nexus cluster status`
2. Identify suspect/down nodes
3. If quorum lost, governance decisions are deferred
4. Restore connectivity or remove failed nodes
5. Verify audit replication caught up after partition heals

## Threat Model

See [THREAT_MODEL.md](THREAT_MODEL.md) for the complete threat model including:

- Agent escape scenarios
- Prompt injection defense
- Supply chain attack vectors
- Insider threat mitigations
- Distributed consensus attacks

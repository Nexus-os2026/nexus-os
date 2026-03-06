# SDK Tutorial: Build Your First Governed Agent

> Step-by-step guide to building an agent that runs under full Nexus OS governance.

## Prerequisites

- Rust 1.75+ with `cargo`
- Nexus OS workspace cloned and building (`cargo build --workspace`)

## Step 1: Create the Agent Crate

```bash
mkdir -p agents/my-summarizer/src
```

Create `agents/my-summarizer/Cargo.toml`:

```toml
[package]
name = "nexus-my-summarizer"
edition.workspace = true
version.workspace = true
license.workspace = true

[dependencies]
nexus-kernel = { path = "../../kernel" }
nexus-sdk = { path = "../../sdk" }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
uuid = { version = "1", features = ["v4"] }
```

Add the crate to the workspace `Cargo.toml`:

```toml
members = [
  # ... existing members ...
  "agents/my-summarizer",
]
```

## Step 2: Write the Agent Manifest

Create `agents/my-summarizer/manifest.toml`:

```toml
name = "my-summarizer"
version = "0.1.0"
capabilities = ["llm.query", "fs.read"]
fuel_budget = 5000
autonomy_level = 2
```

This manifest declares:
- **Capabilities**: The agent can query LLMs and read files. It cannot write files, execute processes, or post to social media.
- **Fuel budget**: 5000 units per run. LLM queries cost 10, file reads cost 2.
- **Autonomy level**: L2 (act-with-approval). The agent acts but requires human approval.

## Step 3: Implement the NexusAgent Trait

Create `agents/my-summarizer/src/lib.rs`:

```rust
use nexus_kernel::errors::AgentError;
use nexus_sdk::{AgentContext, AgentOutput, NexusAgent};
use serde_json::json;

pub struct SummarizerAgent {
    initialized: bool,
}

impl SummarizerAgent {
    pub fn new() -> Self {
        Self { initialized: false }
    }
}

impl NexusAgent for SummarizerAgent {
    fn init(&mut self, ctx: &mut AgentContext) -> Result<(), AgentError> {
        // Verify capabilities at startup — fail fast if manifest is wrong
        ctx.require_capability("llm.query")?;
        ctx.require_capability("fs.read")?;
        self.initialized = true;
        Ok(())
    }

    fn execute(&mut self, ctx: &mut AgentContext) -> Result<AgentOutput, AgentError> {
        if !self.initialized {
            return Err(AgentError::SupervisorError("not initialized".into()));
        }

        // Read the target file (costs 2 fuel, emits audit event)
        let content = ctx.read_file("input.txt")?;

        // Query the LLM to summarize (costs 10 fuel, emits audit event)
        let summary = ctx.llm_query(
            &format!("Summarize this text:\n{}", content),
            500,
        )?;

        // Calculate fuel used
        let fuel_used = ctx.fuel_budget() - ctx.fuel_remaining();

        Ok(AgentOutput {
            status: "ok".to_string(),
            outputs: vec![json!({ "summary": summary })],
            fuel_used,
        })
    }

    fn shutdown(&mut self, _ctx: &mut AgentContext) -> Result<(), AgentError> {
        self.initialized = false;
        Ok(())
    }
}
```

### What happens under the hood

Every call to `ctx.read_file()` or `ctx.llm_query()`:

1. **Checks capability** - Is `fs.read` or `llm.query` in the manifest?
2. **Checks fuel** - Is there enough budget remaining?
3. **Appends audit event** - Records the action with parameters
4. **Deducts fuel** - Subtracts the cost from the remaining budget

If any check fails, the operation returns an error and the agent is stopped.

## Step 4: Write Tests with TestHarness

Add tests to `agents/my-summarizer/src/lib.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use nexus_sdk::TestHarness;

    #[test]
    fn summarizer_lifecycle() {
        let mut ctx = TestHarness::new()
            .with_capabilities(vec![
                "llm.query".to_string(),
                "fs.read".to_string(),
            ])
            .with_fuel(1000)
            .build_context();

        let mut agent = SummarizerAgent::new();

        // Init should succeed with correct capabilities
        assert!(agent.init(&mut ctx).is_ok());

        // Execute should produce output and consume fuel
        let output = agent.execute(&mut ctx).unwrap();
        assert_eq!(output.status, "ok");
        assert!(output.fuel_used > 0);
        assert!(!output.outputs.is_empty());

        // Audit trail should have events
        assert!(ctx.audit_trail().events().len() >= 2);

        // Shutdown
        assert!(agent.shutdown(&mut ctx).is_ok());
    }

    #[test]
    fn denied_without_capabilities() {
        let mut ctx = TestHarness::new()
            .with_capabilities(vec!["fs.read".to_string()])
            .with_fuel(1000)
            .build_context();

        let mut agent = SummarizerAgent::new();

        // Init should fail — llm.query not granted
        let result = agent.init(&mut ctx);
        assert!(matches!(result, Err(AgentError::CapabilityDenied(_))));
    }

    #[test]
    fn stops_when_fuel_exhausted() {
        let mut ctx = TestHarness::new()
            .with_capabilities(vec![
                "llm.query".to_string(),
                "fs.read".to_string(),
            ])
            .with_fuel(5) // Not enough for read (2) + llm (10)
            .build_context();

        let mut agent = SummarizerAgent::new();
        agent.init(&mut ctx).unwrap();

        let result = agent.execute(&mut ctx);
        assert!(matches!(result, Err(AgentError::FuelExhausted)));
    }
}
```

## Step 5: Build and Test

```bash
cargo test -p nexus-my-summarizer
```

## Step 6: Use the ManifestBuilder (Alternative to TOML)

You can also construct manifests programmatically:

```rust
use nexus_sdk::ManifestBuilder;

let manifest = ManifestBuilder::new("my-summarizer")
    .version("0.1.0")
    .capability("llm.query")
    .capability("fs.read")
    .fuel_budget(5000)
    .autonomy_level(2)
    .build()?;
```

The builder validates:
- Name is 3-64 alphanumeric characters (plus hyphens)
- Fuel budget is > 0 and <= 1,000,000
- Autonomy level is 0-5
- All capabilities are in the registry

## Step 7: Checkpoint and Restore (Optional)

For long-running agents, implement checkpoint/restore:

```rust
impl NexusAgent for SummarizerAgent {
    fn checkpoint(&self) -> Result<Vec<u8>, AgentError> {
        let state = json!({ "initialized": self.initialized });
        Ok(serde_json::to_vec(&state).unwrap_or_default())
    }

    fn restore(&mut self, data: &[u8]) -> Result<(), AgentError> {
        if let Ok(state) = serde_json::from_slice::<serde_json::Value>(data) {
            self.initialized = state["initialized"].as_bool().unwrap_or(false);
        }
        Ok(())
    }

    // ... init, execute, shutdown as before
}
```

## Capability Reference

| Capability | Fuel Cost | Description |
|-----------|-----------|-------------|
| `llm.query` | 10 | Query a language model |
| `fs.read` | 2 | Read a file from the filesystem |
| `fs.write` | 8 | Write a file to the filesystem |
| `web.search` | - | Search the web |
| `web.read` | - | Read a web page |
| `process.exec` | - | Execute a system process |
| `social.post` | - | Post to social media |
| `social.x.post` | - | Post to X (Twitter) |
| `social.x.read` | - | Read from X (Twitter) |
| `messaging.send` | - | Send a message |
| `audit.read` | - | Read audit trail events |

## Autonomy Level Guide

| Level | When to Use |
|-------|-------------|
| L0 | Agent is disabled |
| L1 | Agent suggests actions, human executes |
| L2 | Agent acts after explicit human approval |
| L3 | Agent acts freely, reports results after |
| L4 | Fully autonomous with anomaly detection |
| L5 | Full autonomy, kernel override only |

Start new agents at L1 or L2. The adaptive governance system will automatically promote agents with strong track records and demote those with violations.

## Next Steps

- Read [ARCHITECTURE.md](ARCHITECTURE.md) for system design
- Read [API_REFERENCE.md](API_REFERENCE.md) for complete type documentation
- Read [SECURITY_HARDENING.md](SECURITY_HARDENING.md) for production deployment
- Read [DEPLOYMENT.md](DEPLOYMENT.md) for cluster setup

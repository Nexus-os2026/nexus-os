# PROMPT: API Rate Limiting & Hardening for Nexus OS

## Context
The 397 Tauri commands and REST API need rate limiting, request validation, and abuse prevention for enterprise deployment.

## Objective
Add rate limiting, request throttling, and API hardening to both the Tauri command layer and the REST API (server mode).

## Implementation Steps

### Step 1: Add rate limiting to nexus-kernel

**Dependencies:**
```toml
governor = "0.7"  # Token bucket rate limiter
```

### Step 2: Rate limiter configuration

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitConfig {
    pub enabled: bool,
    pub default_rpm: u32,           // Requests per minute (default: 60)
    pub llm_rpm: u32,              // LLM requests per minute (default: 20)
    pub agent_execute_rpm: u32,    // Agent executions per minute (default: 30)
    pub audit_export_rpm: u32,     // Audit exports per minute (default: 5)
    pub burst_size: u32,           // Max burst (default: 10)
    pub per_user: bool,            // Rate limit per user or global
}
```

### Step 3: Implement rate limiter middleware

```rust
use governor::{Quota, RateLimiter};
use std::num::NonZeroU32;

pub struct NexusRateLimiter {
    default: RateLimiter<String, ...>,
    llm: RateLimiter<String, ...>,
    agent_execute: RateLimiter<String, ...>,
}

impl NexusRateLimiter {
    pub fn check(&self, category: RateCategory, user_id: &str) -> Result<(), RateLimitError> {
        // Token bucket check
        // Return Err with retry-after header value if exceeded
    }
}

pub enum RateCategory {
    Default,
    LlmRequest,
    AgentExecute,
    AuditExport,
    BackupCreate,
    AdminOperation,
}
```

### Step 4: Apply to Tauri commands

Create a macro or wrapper that adds rate limiting to existing commands:

```rust
// Before
#[tauri::command]
async fn agent_execute(...) -> Result<TaskResult, NexusError> { ... }

// After — add rate check at the top of each command handler
#[tauri::command]
async fn agent_execute(state: State<'_, AppState>, ...) -> Result<TaskResult, NexusError> {
    state.rate_limiter.check(RateCategory::AgentExecute, &session.user_id)?;
    // ... existing logic
}
```

### Step 5: REST API rate limiting (server mode)

Add rate limiting headers to all HTTP responses:
```
X-RateLimit-Limit: 60
X-RateLimit-Remaining: 45
X-RateLimit-Reset: 1711036800
Retry-After: 30  (only on 429 responses)
```

Return HTTP 429 Too Many Requests when limit exceeded.

### Step 6: Request validation hardening

For all Tauri commands and REST endpoints:
- Maximum request body size: 10 MB (configurable)
- Input string length limits
- JSON depth limit: 32 levels
- UTF-8 validation on all string inputs
- Path traversal prevention on file-related commands
- SQL injection prevention (already handled by rusqlite parameterized queries)

### Step 7: Configuration

```toml
[rate_limiting]
enabled = true
default_rpm = 60
llm_rpm = 20
agent_execute_rpm = 30
audit_export_rpm = 5
burst_size = 10
per_user = true

[api]
max_request_body_bytes = 10_485_760  # 10 MB
max_string_length = 100_000
json_max_depth = 32
```

## Testing
- Unit test: Rate limiter allows within limits
- Unit test: Rate limiter blocks when exceeded
- Unit test: Rate limiter resets after interval
- Unit test: Burst allowance works correctly
- Unit test: Per-user isolation

## Finish
Run `cargo fmt` and `cargo clippy` on modified crates only.
Do NOT use `--all-features`.

# PROMPT: Enterprise Integrations for Nexus OS

## Context
Enterprise deployments need Nexus OS agents to interact with existing business tools: Slack, Microsoft Teams, Jira, ServiceNow, GitHub/GitLab, and custom webhooks.

## Objective
Create a `nexus-integrations` crate with a plugin architecture for enterprise tool integrations.

## Architecture

```
Agent Action → Integration Router → Provider Plugin → External API
                                  ↓
                          Audit Trail Entry
```

All integrations are:
- Capability-gated (agent must have integration capability)
- Audited (every external call is logged)
- Rate-limited (per provider limits)
- PII-redacted (sensitive data stripped before sending)

## Implementation Steps

### Step 1: Create nexus-integrations crate

```bash
cd crates
cargo new nexus-integrations --lib
```

**Dependencies:**
```toml
[dependencies]
reqwest = { version = "0.12", features = ["json", "rustls-tls"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tokio = { version = "1", features = ["full"] }
async-trait = "0.1"
thiserror = "2"
tracing = "0.1"
url = "2"
```

### Step 2: Integration trait

```rust
#[async_trait]
pub trait Integration: Send + Sync {
    fn name(&self) -> &str;
    fn provider_type(&self) -> ProviderType;
    
    async fn send_notification(&self, message: &Notification) -> Result<(), IntegrationError>;
    async fn create_ticket(&self, ticket: &TicketRequest) -> Result<TicketResponse, IntegrationError>;
    async fn update_status(&self, update: &StatusUpdate) -> Result<(), IntegrationError>;
    async fn webhook(&self, event: &WebhookEvent) -> Result<(), IntegrationError>;
    
    fn health_check(&self) -> Result<(), IntegrationError>;
}

pub enum ProviderType {
    Slack,
    MicrosoftTeams,
    Jira,
    ServiceNow,
    GitHub,
    GitLab,
    CustomWebhook,
}
```

### Step 3: Slack integration

```rust
pub struct SlackIntegration {
    webhook_url: String,
    bot_token: Option<String>,
    default_channel: String,
}
```

Features:
- Send notifications (agent alerts, HITL requests, security events)
- Interactive HITL approval via Slack buttons
- Agent status updates to channels
- Slash command support for agent queries

### Step 4: Microsoft Teams integration

```rust
pub struct TeamsIntegration {
    webhook_url: String,
    app_id: Option<String>,
}
```

Features:
- Adaptive Card notifications
- HITL approval via Teams actions
- Agent status updates

### Step 5: Jira integration

```rust
pub struct JiraIntegration {
    base_url: String,
    email: String,
    api_token: String,
    default_project: String,
}
```

Features:
- Agent-created tickets (bugs, tasks, stories)
- Status sync (agent updates Jira, Jira updates trigger agents)
- Attachment support (audit reports, agent outputs)

### Step 6: ServiceNow integration

```rust
pub struct ServiceNowIntegration {
    instance_url: String,
    username: String,
    password: String,
}
```

Features:
- Incident creation from agent errors
- Change request creation for agent deployments
- CMDB updates for agent fleet management

### Step 7: Custom webhook

```rust
pub struct WebhookIntegration {
    url: String,
    method: HttpMethod,
    headers: HashMap<String, String>,
    auth: Option<WebhookAuth>,
    retry_count: u32,
    timeout_ms: u64,
}

pub enum WebhookAuth {
    Bearer(String),
    Basic { username: String, password: String },
    ApiKey { header: String, key: String },
    HmacSignature { secret: String },
}
```

### Step 8: Integration router

```rust
pub struct IntegrationRouter {
    integrations: HashMap<String, Box<dyn Integration>>,
    rate_limiter: RateLimiter,
}

impl IntegrationRouter {
    pub async fn route(&self, event: NexusEvent) -> Result<(), IntegrationError> {
        // 1. Match event to configured integrations
        // 2. PII-redact the payload
        // 3. Rate limit check
        // 4. Send to each matching integration
        // 5. Audit trail entry for each send
    }
}
```

### Step 9: Event types that trigger integrations

```rust
pub enum NexusEvent {
    AgentStarted { did: String, workspace: String },
    AgentCompleted { did: String, result: String },
    AgentError { did: String, error: String },
    HitlRequired { did: String, action: String, context: String },
    HitlDecision { did: String, decision: String },
    SecurityEvent { event_type: String, details: String },
    FuelExhausted { did: String },
    GenomeEvolved { genome_id: String, generation: u32 },
    AuditChainBreak { details: String },
    BackupCompleted { metadata: BackupMetadata },
    SystemAlert { severity: String, message: String },
}
```

### Step 10: Configuration

```toml
[integrations.slack]
enabled = true
webhook_url_env = "NEXUS_SLACK_WEBHOOK_URL"
default_channel = "#nexus-agents"
events = ["agent_error", "hitl_required", "security_event"]

[integrations.jira]
enabled = true
base_url = "https://yourcompany.atlassian.net"
email_env = "NEXUS_JIRA_EMAIL"
api_token_env = "NEXUS_JIRA_TOKEN"
default_project = "NEXUS"
events = ["agent_error"]

[integrations.teams]
enabled = false

[integrations.webhook.custom1]
enabled = true
url = "https://internal-api.company.com/nexus-events"
method = "POST"
auth = { type = "bearer", token_env = "NEXUS_CUSTOM_WEBHOOK_TOKEN" }
events = ["*"]  # All events
```

### Step 11: Tauri commands

```rust
#[tauri::command]
async fn integrations_list(state: State<'_, AppState>) -> Result<Vec<IntegrationStatus>, NexusError>

#[tauri::command]
async fn integration_test(state: State<'_, AppState>, provider: String) -> Result<TestResult, NexusError>

#[tauri::command]
async fn integration_configure(state: State<'_, AppState>, config: IntegrationConfig) -> Result<(), NexusError>
```

### Step 12: Frontend

Create `frontend/src/pages/Integrations/` with:
- Integration marketplace (grid of available integrations)
- Configuration wizard per integration
- Test connection button
- Event routing configuration
- Integration health status

## Testing
- Unit test: Each integration's message formatting
- Unit test: PII redaction before sending
- Unit test: Event routing logic
- Unit test: Rate limiting per integration
- Mock test: Webhook delivery with retry

## Finish
Run `cargo fmt` and `cargo clippy` on `nexus-integrations` crate only.
Do NOT use `--all-features`.

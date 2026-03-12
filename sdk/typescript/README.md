# @nexus-os/sdk

TypeScript SDK for Nexus OS — governed deterministic agent OS.

Zero runtime dependencies. Uses native `fetch` (Node 18+).

## Install

```bash
npm install @nexus-os/sdk
```

## Quick Start

```typescript
import { NexusClient } from "@nexus-os/sdk";

const nexus = new NexusClient({
  baseUrl: "http://localhost:3000",
  apiKey: "your-api-key",
});

// Health check
const health = await nexus.health();
console.log(health.status); // "healthy"
```

## Anthropic-compatible API

```typescript
// Non-streaming
const response = await nexus.messages({
  model: "llama3",
  max_tokens: 1024,
  messages: [{ role: "user", content: "Hello!" }],
});
console.log(response.content[0].text);

// Streaming
for await (const event of nexus.messagesStream({
  model: "llama3",
  max_tokens: 1024,
  stream: true,
  messages: [{ role: "user", content: "Write a poem" }],
})) {
  if (event.delta?.text) {
    process.stdout.write(event.delta.text);
  }
}
```

## OpenAI-compatible API

```typescript
// Chat completion
const chat = await nexus.chatCompletion(
  [{ role: "user", content: "Hello!" }],
  { model: "llama3", max_tokens: 1024 }
);
console.log(chat.choices[0].message.content);

// Embeddings
const embeddings = await nexus.embeddings("Hello world");
console.log(embeddings.data[0].embedding);

// List models
const models = await nexus.listModels();
console.log(models.data.map((m) => m.id));
```

## Agent Management

```typescript
// List agents
const agents = await nexus.listAgents();

// Create agent
const created = await nexus.createAgent({
  name: "my-agent",
  version: "1.0.0",
  capabilities: ["web.search", "llm.query"],
  fuel_budget: 10000,
});

// Start/stop
await nexus.startAgent(created.agent_id);
await nexus.stopAgent(created.agent_id);

// Get status
const status = await nexus.getAgentStatus(created.agent_id);
console.log(status.fuel_remaining);
```

## Permissions

```typescript
// Get permissions
const perms = await nexus.getPermissions(agentId);

// Update single
await nexus.updatePermission(agentId, {
  capability_key: "web.search",
  enabled: true,
});

// Bulk update
await nexus.bulkUpdatePermissions(
  agentId,
  [
    { capability_key: "web.search", enabled: true },
    { capability_key: "llm.query", enabled: false },
  ],
  "security review"
);
```

## Audit & Compliance

```typescript
// Query audit log
const audit = await nexus.queryAuditLog({ limit: 10 });

// Compliance
const status = await nexus.complianceStatus();
const report = await nexus.complianceReport(agentId);

// GDPR erasure
await nexus.complianceErase(agentId, ["key-1", "key-2"]);
```

## MCP Tools

```typescript
// List tools for an agent
const tools = await nexus.listTools("my-agent");

// Invoke a governed tool
const result = await nexus.invokeTool({
  agent: "my-agent",
  tool: "web_search",
  params: { query: "rust async" },
});
console.log(result.fuel_consumed);
```

## Error Handling

```typescript
import { NexusAuthError, NexusNotFoundError, NexusRateLimitError } from "@nexus-os/sdk";

try {
  await nexus.getAgentStatus("nonexistent-id");
} catch (e) {
  if (e instanceof NexusAuthError) {
    console.error("Auth failed:", e.message);
  } else if (e instanceof NexusNotFoundError) {
    console.error("Not found:", e.endpoint);
  } else if (e instanceof NexusRateLimitError) {
    console.error("Rate limited — fuel exhausted");
  }
}
```

## Authentication

The SDK supports two auth methods:

- **API Key**: `x-api-key` header (for LLM-compatible endpoints)
- **JWT Bearer**: `Authorization: Bearer <token>` (for REST API endpoints)

```typescript
// API key auth
const nexus = new NexusClient({ apiKey: "your-key" });

// JWT auth
const nexus = new NexusClient({ token: "your-jwt-token" });
```

## License

MIT

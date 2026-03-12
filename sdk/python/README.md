# nexus-os-sdk

Python SDK for Nexus OS — governed deterministic agent OS.

Requires Python 3.9+ and [httpx](https://www.python-httpx.org/) for HTTP and streaming.

## Install

```bash
pip install nexus-os-sdk
```

## Quick Start

```python
from nexus_sdk import NexusClient

client = NexusClient(
    base_url="http://localhost:3000",
    api_key="your-api-key",
)

# Health check
health = client.health()
print(health["status"])  # "healthy"
```

## Anthropic-compatible API

```python
# Non-streaming
response = client.messages(
    messages=[{"role": "user", "content": "Hello!"}],
    model="llama3",
    max_tokens=1024,
)
print(response.content[0]["text"])

# With system prompt
response = client.messages(
    messages=[{"role": "user", "content": "Explain governance"}],
    system="You are a Nexus OS expert.",
    model="llama3",
)

# Streaming
for event in client.messages_stream(
    messages=[{"role": "user", "content": "Write a poem"}],
    model="llama3",
    max_tokens=512,
):
    if "delta" in event:
        print(event["delta"].get("text", ""), end="")
```

## OpenAI-compatible API

```python
# Chat completion
result = client.chat_completion(
    messages=[{"role": "user", "content": "Hello!"}],
    model="llama3",
)
print(result["choices"][0]["message"]["content"])

# Embeddings
embeddings = client.embeddings("Hello world")
print(embeddings["data"][0]["embedding"])

# List models
models = client.list_models()
for m in models["data"]:
    print(m["id"])
```

## Agent Management

```python
# List agents
agents = client.list_agents()
for agent in agents:
    print(f"{agent.name}: {agent.status}")

# Create agent
from nexus_sdk import AgentManifest

manifest = AgentManifest(
    name="my-agent",
    capabilities=["web.search", "llm.query"],
    fuel_budget=10000,
    autonomy_level=2,
)
created = client.create_agent(manifest)
print(created["agent_id"])

# Start / stop
client.start_agent(created["agent_id"])
client.stop_agent(created["agent_id"])

# Status
status = client.get_agent_status(created["agent_id"])
print(status)
```

## Permissions

```python
# Get permissions
perms = client.get_permissions(agent_id)

# Update single
client.update_permission(agent_id, "web.search", enabled=True)

# Bulk update
client.bulk_update_permissions(
    agent_id,
    updates=[
        {"capability_key": "web.search", "enabled": True},
        {"capability_key": "llm.query", "enabled": False},
    ],
    reason="security review",
)
```

## Audit & Compliance

```python
# Query audit log
audit = client.query_audit_log(limit=10)
for event in audit["events"]:
    print(event["event_type"])

# Compliance status
status = client.compliance_status()
print(status["status"])

# Transparency report
report = client.compliance_report(agent_id)

# GDPR erasure
client.compliance_erase(agent_id, encryption_key_ids=["key-1"])
```

## MCP Tools

```python
# List governed tools
tools = client.list_tools("my-agent")
for tool in tools["tools"]:
    print(tool["name"])

# Invoke a tool
result = client.invoke_tool(
    agent="my-agent",
    tool="web_search",
    params={"query": "rust async"},
)
print(f"Fuel consumed: {result['fuel_consumed']}")
```

## RAG (Retrieval-Augmented Generation)

```python
# Index a document
doc = client.index_document("/path/to/document.md")
print(f"Indexed {doc.chunk_count} chunks")

# Search
results = client.search_documents("How does governance work?")
for r in results:
    print(f"[{r.score:.2f}] {r.content[:80]}")

# Chat with documents
answer = client.chat_with_documents("What are the autonomy levels?")
```

## Time Machine

```python
# Create checkpoint before risky operation
client.create_checkpoint("before refactor")

# ... do stuff ...

# Undo if something goes wrong
result = client.undo()
print(f"Restored {len(result.files_restored)} files")

# List checkpoints
for cp in client.list_checkpoints():
    print(f"{cp.label} ({cp.change_count} changes)")
```

## Error Handling

```python
from nexus_sdk import NexusAuthError, NexusNotFoundError, NexusRateLimitError

try:
    client.get_agent_status("nonexistent-id")
except NexusAuthError:
    print("Authentication failed")
except NexusNotFoundError as e:
    print(f"Not found: {e.endpoint}")
except NexusRateLimitError:
    print("Rate limited (fuel exhausted)")
```

## Context Manager

```python
with NexusClient(api_key="key") as client:
    health = client.health()
    print(health["status"])
# Client is automatically closed
```

## License

MIT

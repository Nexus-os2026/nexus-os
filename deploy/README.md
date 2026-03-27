# Nexus OS Server Deployment

## Quick Start with Docker Compose

### With GPU (NVIDIA):
```bash
cd deploy
docker-compose up -d
```

### CPU only:
```bash
cd deploy
docker-compose -f docker-compose.cpu.yml up -d
```

### Verify:
```bash
curl http://localhost:3000/health
```

## Kubernetes with Helm

```bash
cd deploy/helm
helm install nexus-os ./nexus-os

# With custom values:
helm install nexus-os ./nexus-os \
  --set server.port=8080 \
  --set persistence.size=50Gi \
  --set env.OPENAI_API_KEY=sk-...
```

## Headless Server (Binary)

```bash
cargo build --release -p nexus-server
./target/release/nexus-server --port 3000 --mcp-port 3001 --a2a-port 3002
```

## Configuration

| Variable | Default | Description |
|----------|---------|-------------|
| `--port` | 3000 | HTTP API port |
| `--mcp-port` | 3001 | MCP server port (0 to disable) |
| `--a2a-port` | 3002 | A2A server port (0 to disable) |
| `--data-dir` | ./nexus-data | Data directory |
| `--log-level` | info | Log level |
| `OLLAMA_HOST` | http://localhost:11434 | Ollama server URL |

## API Endpoints

| Method | Path | Description |
|--------|------|-------------|
| GET | /health | Server health + version |
| GET | /status | Agent count, uptime, providers |
| GET | /api/v1/agents | List all agents |
| POST | /api/v1/agents/{id}/run | Execute an agent task |
| GET | /api/v1/agents/{id}/status | Agent execution status |
| GET | /api/v1/audit | Query audit trail |
| GET | /mcp/tools/list | MCP tool listing |
| POST | /mcp/tools/invoke | MCP tool invocation |
| POST | /mcp/handle | Raw JSON-RPC MCP endpoint |
| POST | /a2a | A2A task submission |
| GET | /a2a/agent-card | A2A agent discovery |

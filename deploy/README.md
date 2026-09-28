# Withdrawn: `crates/nexus-server` deployment

**These recipes are withdrawn during Phase Zero (P0-FG1).**

The files in this directory built and ran `crates/nexus-server`. That
headless server exposed an HTTP API, the `nexus-mcp` tools and an A2A endpoint
on ports 3000–3002 of every network interface, with no authentication and a
CORS policy that admitted any origin (Final-Gate dossier item J1). It is
withdrawn, not repaired.

## What the recipes do now

| File | Behaviour |
|---|---|
| `crates/nexus-server` binary | Every invocation prints one fixed message and exits with status 69. It reads no arguments, environment, configuration or credentials, creates no files and opens no sockets. |
| `Dockerfile` | Fails at its first step. It installs no packages, copies no source and builds no image. |
| `docker-compose.yml`, `docker-compose.cpu.yml` | Define no services, ports, volumes, credentials or restart policies. The Ollama service they used to publish on port 11434 is removed too. |
| `helm/nexus-os` | Its only template fails, so no install, upgrade or values override renders a resource. |

## What this does not do

- **An existing deployment is not stopped.** Containers, images, volumes and
  Helm releases created from earlier versions of these files keep running
  until whoever operates them stops them with the tooling that started them.
  Nothing in this repository stops, removes or changes them.
- A Helm upgrade with this chart fails before it changes anything. It does not
  delete a release's resources or its data volume claim.
- Credentials that an earlier deployment could use (for example a GitHub
  token given to the server) are not revoked by this change.
- The `nexus-mcp` tools are not made safe to expose on a network.
- The protocols server (the root `Dockerfile`, `docker-compose.yml`, `helm/`
  and `install.sh`) is a separate Final-Gate item and is not changed here.
- No volume, model file or user data is deleted.

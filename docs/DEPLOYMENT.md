# Deployment Guide

> **Phase Zero: the standalone server deployments are withdrawn.** Nexus OS
> runs as the desktop app, installed with the release installers (Windows
> `.exe` or `.msi`, macOS `.dmg`, Linux `.deb`).

## Platform Requirements

| Component | Minimum | Recommended |
|-----------|---------|-------------|
| OS | Linux (x86_64), macOS (ARM64/x86_64), Windows (x86_64) | Ubuntu 22.04+ / macOS 14+ |
| Rust | 1.82+ | Latest stable |
| Node.js | 18+ | 22 LTS |
| RAM | 4 GB | 16 GB |
| Disk | 2 GB | 20 GB (models + audit logs) |
| CPU | 2 cores | 8+ cores |
| GPU | None (CPU inference) | CUDA-capable for local LLM |

---

## Withdrawn during Phase Zero

The standalone server binaries, and every recipe in this repository that
built, installed or deployed them, are withdrawn during Phase Zero. They ran
outside the governed desktop; the protocols server listened on every network
interface by default. Each withdrawn binary prints one fixed message to
standard error and exits with status 69. It reads no argument, environment
variable or file first, and starts no server or process.

### Protocols server (`nexus-protocols-server`, `nexus-os`) (withdrawn)

The protocols server `nexus-protocols-server` and its alias `nexus-os`
(package `nexus-protocols`) are withdrawn during Phase Zero. Building or
running either produces only a fixed withdrawal message and a non-zero exit
status; neither starts a server. The recipes that built, installed or
deployed them are withdrawn too:

| Recipe | Behaviour now |
|---|---|
| `Dockerfile` | Fails at its first step. It installs no packages, copies no source and builds no image. |
| `docker-compose.yml` | Defines no services, ports, volumes, credentials or restart policies. |
| `helm/nexus-os` | Its only template fails, so no install, upgrade or values override renders a resource. |
| `Makefile` target `nexus-os` | Fails. It builds and copies nothing. |
| `install.sh` | Fails. It downloads and installs nothing. |

The repository provides no supported deployment of the protocols server at
this point. The container, Kubernetes, air-gapped and headless-binary
instructions this guide used to give are withdrawn with it, and so is its
configuration and endpoint reference.

### Other standalone binaries (withdrawn)

`nexus-cli` (the `nexus` command), the standalone `nx` terminal, the
`coding-agent` and `social-poster-agent` executables and the `nx-*`
computer-use tools are withdrawn in the same way, with their install and
packaging recipes (see the [User Guide](USER_GUIDE.md)).

### Existing deployments

An existing deployment, container, image, volume or Helm release started from
earlier instructions is not stopped automatically by this source change, and
nothing is deleted. Stop it with the tooling that started it. An upgrade with
the withdrawn chart fails before it changes anything, including the release's
data volume claim. Images and release assets published elsewhere are not
removed, and credentials given to earlier deployments are not revoked.

### `crates/nexus-server` (withdrawn)

The package `crates/nexus-server`, formerly the "CLI server" alternative, is
withdrawn during Phase Zero. Building or running it produces only a fixed
withdrawal message and a non-zero exit status; it starts no server. The
repository provides no supported deployment of it at this point.

An existing deployment started from earlier instructions is not stopped
automatically by this source change. Stop it with the tooling that started it.

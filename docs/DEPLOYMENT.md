# Deployment Status (Phase Zero)

> **The Phase Zero Linux support profile is complete, and there is still no
> supported server deployment.** This page records what the completed Phase
> Zero Linux support profile does and does not support. It is not an
> installation guide.

## Platform scope

- Linux is the active Phase Zero validation profile.
- Windows and macOS portability validation is deferred. No Phase Zero
  support or validation claim is made for them here.
- This document claims no Phase Zero production installer or release asset.
  Installers or assets published earlier, including historical releases,
  predate the Phase Zero rebuild.

## Withdrawn during Phase Zero

The standalone server binaries, and every recipe in this repository that
built, installed or deployed them, are withdrawn during Phase Zero. They ran
outside the governed desktop; the protocols server listened on every network
interface by default. Each withdrawn binary prints one fixed message to
standard error and exits with status 69. It reads no argument, environment
variable or file first, and starts no server or process.

This covers the Docker, Docker Compose, Kubernetes/Helm and air-gapped server
recipes and the headless server paths. There is no supported standalone
server deployment in the Phase Zero Linux support profile.

### Protocols server (`nexus-protocols-server`, `nexus-os`) (withdrawn)

The protocols server `nexus-protocols-server` and its alias `nexus-os`
(package `nexus-protocols`), including its OpenAI-compatible endpoints, are
withdrawn during Phase Zero. Building or running either produces only a fixed
withdrawal message and a non-zero exit status; neither starts a server. The
recipes that built, installed or deployed them are withdrawn too:

| Recipe | Behaviour now |
|---|---|
| `Dockerfile` | Fails at its first step. It installs no packages, copies no source and builds no image. |
| `docker-compose.yml` | Defines no services, ports, volumes, credentials or restart policies. |
| `helm/nexus-os` | Its only template fails, so no install, upgrade or values override renders a resource. |
| `Makefile` target `nexus-os` | Fails. It builds and copies nothing. |
| `install.sh` | Fails. It downloads and installs nothing. |

The repository provides no supported deployment of the protocols server at
this point.

### Other standalone binaries (withdrawn)

`nexus-cli` (the `nexus` command), the standalone `nx` terminal, the
`coding-agent` and `social-poster-agent` executables and the `nx-*`
computer-use tools are withdrawn in the same way, with their install and
packaging recipes (see the [User Guide](USER_GUIDE.md)).

### `crates/nexus-server` (withdrawn)

The package `crates/nexus-server`, formerly the "CLI server" alternative, is
withdrawn during Phase Zero. Building or running it produces only a fixed
withdrawal message and a non-zero exit status; it starts no server. The
repository provides no supported deployment of it at this point.

## Existing deployments

An existing deployment, container, image, volume or Helm release started from
earlier instructions is not stopped automatically by this source change, and
nothing is deleted. Stop it with the tooling that started it. An upgrade with
the withdrawn chart fails before it changes anything, including the release's
data volume claim.

Images and release assets published earlier may still exist outside this
source tree; they are not removed by it. Withdrawing the source does not
revoke credentials given to earlier deployments: rotate or revoke them
separately.

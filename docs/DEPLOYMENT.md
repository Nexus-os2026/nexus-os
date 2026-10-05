# Deployment Status

> **The Phase Zero, Phase One and Phase Two engineering checkpoints are
> complete and frozen for the Linux support profile, and there is still no
> supported server deployment.** None of these phase closures by itself
> authorizes a general production deployment of Nexus. This page records what
> is and is not supported. It is not an installation guide.
>
> Phase One (the governed coding workflow) and Phase Two (governed
> verification execution) are also Linux only. They add no server deployment
> and change nothing on this page. The server and deployment surfaces
> withdrawn during Phase Zero remain withdrawn unless a later approved phase
> explicitly reopens them.

## Platform scope

- Linux is the active validation profile for Phase Zero, Phase One and Phase
  Two.
- Windows and macOS portability validation is deferred. No support or
  validation claim for any phase is made for them here.
- This document claims no production installer or release asset for any
  phase. Installers or assets published earlier, including historical
  releases, predate the Phase Zero rebuild.

## Historical checkpoints are not release candidates

The frozen Phase Zero and Phase One snapshots are historical engineering
evidence, not current release candidates: their lockfiles resolve Wasmtime
43.0.2, which RUSTSEC-2026-0327 affects (see the
[security policy](../SECURITY.md)). Current release or deployment work must
use a current, governed forward baseline, not those historical snapshots.

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

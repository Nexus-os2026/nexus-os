# Nexus OS

Nexus OS is a governed, local-first agentic AI operating environment: a Rust
and Tauri desktop application in which AI agents are meant to act only
through backend-owned, explicitly granted authority.

It is not production-ready.

## Status: Phase Zero — Trust Boundary rebuild

**Phase Zero is not complete.**

The repository is being rebuilt around one rule: a string is never
authority. Phase Zero replaces ambient, caller-asserted and path-based
authority with backend-owned grants, narrowing, revocation and fail-closed
defaults.

The current technical and documentation work is a Linux closure candidate.
It still awaits final hosted validation and integration. Until then, what
this README describes is the state of that candidate, not of an integrated
or released product.

Material on the public default branch, and older releases, tags and
repository metadata, predate the rebuild. They are not a description of the
current state.

## Platform scope

- Linux is the active Phase Zero validation profile.
- Windows and macOS portability validation is deferred.
- Linux security claims do not automatically apply to Windows or macOS.

## Local-first is not local-only

Nexus can use local model providers, and cloud model providers are
optional. When a cloud provider is configured, the relevant request data
leaves the machine and is sent to that provider. Using Nexus without any
data leaving the machine requires a local model runtime and no configured
cloud provider.

## Closed and withdrawn surfaces

Surfaces that cannot yet be governed are closed or withdrawn rather than
offered as production features. On the current Phase Zero candidate this
includes, for example, screen observation, operating-system input control,
voice capture, browser automation through the agent executor,
caller-chosen network destinations and the highest autonomy level. A closed
route fails with an explicit error instead of running.

### Server Deployment (withdrawn)

The standalone server deployments (Docker, Docker Compose, Kubernetes/Helm,
air-gapped and headless binary) are withdrawn during Phase Zero, and so are
these standalone binaries: `nexus-server`, `nexus-protocols-server` and its
`nexus-os` alias, `nexus-cli`, `nx`, `coding-agent`, `social-poster-agent`,
the computer-use tools `nx-screen`, `nx-input`, `nx-agent`, `nx-govern` and
`nx-learn`, and the benchmarks `nim-cloud-bench`, `cloud-models-bench`,
`inference-consistency-bench`, `local-vs-cloud-battle`,
`real-agent-validation` and `real-battery-validation`. Each now prints a
fixed withdrawal message and exits with status 69. This includes the
protocols server and its OpenAI-compatible endpoints.

Other developer and benchmark binaries may remain as development-only tools,
provided they are not shipped, stay pinned by the inventory guard, and are
not documented as runtime entry points.

The repository provides no supported server deployment at this point: there
is no supported standalone server, Docker, Compose, Helm or Kubernetes
deployment of Nexus on the current Phase Zero candidate. An existing
deployment is not stopped automatically by this source change. See the
deployment guide linked below.

## Security status and non-claims

- No external SOC 2 Type II audit or certification is claimed.
- No NIST certification or accreditation is claimed.
- The EU AI Act material is an internal self-assessment and mapping, not a
  conformity assessment or certification.
- The Phase Zero evidence is internal engineering evidence. It is not an
  external audit.
- No general operating-system or WebAssembly sandbox is claimed for agents
  in Phase Zero.
- The dependency and security checks in CI pass with a reviewed set of
  accepted advisories. A green run does not mean there is no dependency
  debt.

## Evidence and status documents

- [Phase Zero Final-Gate dossier](docs/security/phase0-final-gate-dossier.md):
  current engineering evidence, Architect dispositions and explicit
  non-claims. It is not a certification.
- [Phase Zero authority inventory](docs/security/phase0-c5-authority-inventory.md):
  current engineering evidence on authority surfaces and closures. It is
  not a certification.
- [Security policy](SECURITY.md): how to report a vulnerability.
- [Deployment status](docs/DEPLOYMENT.md): what is and is not deployable in
  Phase Zero.

## Vision

In the long term, Nexus aims to be an operating environment in which agents
have verifiable identities, governed autonomy and tamper-evident audit
trails, running locally by default.

## License

[MIT](LICENSE)

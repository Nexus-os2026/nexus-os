# Nexus OS

Nexus OS is a governed, local-first agentic AI operating environment: a Rust
and Tauri desktop application in which AI agents are meant to act only
through backend-owned, explicitly granted authority.

It is not production-ready.

## Status: Phase One complete — Linux support profile

| Phase | Status | Frozen checkpoint |
|---|---|---|
| Phase Zero — trust-boundary foundation | **PHASE ZERO COMPLETE — LINUX SUPPORT PROFILE**, declared by the project Architect on 2026-09-29 | `rebuild/phase0-trust-boundary` at `f727f5c39fab8d5c729a55eb28ad576d3d56ce47` |
| Phase One — governed coding workflow | **PHASE ONE COMPLETE — LINUX SUPPORT PROFILE**, declared by the project Architect on 2026-09-30 | `rebuild/phase1-governed-coding` at `14270a9a38770ac84456c1f812042d2967edec42` |
| Phase Two — governed verification execution | In progress: implementation candidate under Architect review, not complete | — |
| Phase Three — governed real-world control | Implementation candidate under Architect review, not complete | — |

- Both phases are complete for the Linux support profile only. Windows and
  macOS portability validation remains deferred.
- This is not a production-readiness declaration.
- This is not an external certification or audit.
- The frozen checkpoints are the validated bytes. Later commits on `main` do
  not redefine them.
- Phase One's completion evidence is the `evidence/phase1-closure` branch at
  `c237937189b5977eaad01acad6a4c52bcce796cc`.
- `main` after Phase One completion is
  `6f3d64360dd21aa8d717c3995c46e48f396148b9`: the frozen Phase One
  checkpoint plus status documentation and a generated ACL schema and test
  hygiene commit. Pre-Phase-Two entry hardening follows it (see
  [SECURITY.md](SECURITY.md)). None of these redefines the frozen Phase One
  checkpoint.
- Phase Two (governed verification execution, Linux support profile) is being
  implemented on the `implement/phase2-governed-verification` branch under an
  Architect-approved mission. It is not integrated and not complete; see
  [Governed verification](#governed-verification-phase-two-in-progress).
- Phase Three (governed real-world control, Linux support profile) is an
  implementation candidate on `implement/p3-governed-real-world-control` for
  the `phase3/governed-real-world-control` line. It is not integrated into
  `main` and not complete; see
  [Governed real-world control](#governed-real-world-control-phase-three-candidate).

Phase Zero rebuilt the project's trust boundary around one rule: a string is
never authority. It replaced ambient, caller-asserted and path-based
authority with backend-owned grants, narrowing, revocation and fail-closed
defaults.

Phase One builds one governed coding workflow on that foundation (see
[Governed coding](#governed-coding-phase-one) below). It does not make Nexus a
complete, general-purpose autonomous coding system.

The public default branch (`main`) was fast-forwarded to each completed
checkpoint after its hosted validation passed. Older releases and tags
predate the rebuild. They remain historical and are explicitly labelled as
such.

## Platform scope

- Linux is the active validation profile for Phase Zero and Phase One.
- Windows and macOS portability validation is deferred. Governed coding is
  not available on them.
- Linux security claims do not automatically apply to Windows or macOS.

## Governed coding (Phase One)

The desktop's Governed Coding page runs one governed workflow:

1. **Select the project.** The owner picks a project folder through a native
   folder picker that the backend opens itself. The frontend never supplies
   a path, and it refers to projects and runs only by opaque ids that grant
   nothing.
2. **Describe the work.** The owner chooses the write scope and protected
   folders, describes a task, and picks a model installed in a local Ollama
   on a loopback address. The model is pinned into the run, and there is no
   cloud fallback.
3. **The worker proposes edits.** The worker has no shell, git or process
   authority. Project content, including files such as `AGENTS.md`, is
   treated as task data. The model's output becomes only bounded, typed edit
   proposals, applied to a private staging copy. Protected inputs cannot be
   edited.
4. **Verify and review.** The candidate is structurally verified, and the
   backend computes a review of the exact changes.
5. **Approve.** The owner approves through a native confirmation, bound to
   the exact run, base and candidate. A frontend flag cannot approve.
6. **Apply.** Pre-images are saved before the first write.
   - **Stale files.** If preflight finds the base files stale, the apply is
     refused before anything in the owner's project is written.
   - **Concurrent edit.** If an owner edit races the actual write, Nexus
     detects the conflict. It restores the owner's version where it safely
     can, or reports recovery required. It never silently overwrites the
     concurrent version or reports clean success.
   - **Other failure.** Any other failure is rolled back, or reported as
     needing recovery, never as success.
7. **Restore.** The owner can restore that single run. The restore refuses
   if the applied files have since changed.

Phase One does not claim:

- arbitrary project test execution;
- shell coding authority or arbitrary process execution;
- git mutation;
- cloud coding models;
- web-enabled coding research;
- package installation or deployment;
- multi-agent or computer-use coding;
- Windows or macOS governed coding;
- coding-run authority that survives a restart;
- atomic multi-file filesystem transactions;
- general Time Machine rollback authority.

Structural verification checks that the candidate stays within the run's
scopes and policy. It is not semantic verification and does not run the
project's tests.

## Governed verification (Phase Two, in progress)

On a supported Linux host, the owner may run a reviewed candidate's own
tests, locally, in a backend-owned verifier sandbox before deciding whether to
apply it. The first and only profile is `rust.cargo-test.offline.v1`: a
single dependency-free Rust library package, tested offline with a Rust
toolchain packaged with Nexus. The owner confirms each launch in a native
dialog; the frontend sends only the run and a profile name. The sandbox runs
the tests in new user, PID, network, IPC, UTS and cgroup namespaces with no
network, strict Landlock, a seccomp allow-list and backend-owned cgroup
limits. The result is advisory and is bound into the owner's review; Apply
waits while a verification is active or its cleanup is unconfirmed. Without
every required layer, verification is unavailable. The design and its
explicit non-claims are in
[docs/security/phase2-governed-verification.md](docs/security/phase2-governed-verification.md).
This is an implementation candidate, not a completed phase.

## Governed real-world control (Phase Three, candidate)

On a supported Linux host, agents and the owner can act in the world only
through one governed pipeline: contained tools with pinned executables (no
shell), network requests to granted origins, headless browser sessions
confined to granted origins, observation of and input to a separate agent
display (never the owner's own), and connector operations whose credentials
stay in the vault. Each effect is an action commitment bound to its exact
target and parameters, under a grant the owner confirmed natively; sensitive
or irreversible (R2) effects also need the owner's native approval of that
exact commitment. Every step is recorded in the audit trail, and the owner's
emergency stop ends everything until the owner resumes. The design and its
explicit non-claims are in
[docs/security/phase3-governed-real-world-control.md](docs/security/phase3-governed-real-world-control.md).
This is an implementation candidate, not a completed phase.

## Local-first is not local-only

Nexus can use local model providers, and cloud model providers are
optional. When a cloud provider is configured, the relevant request data
leaves the machine and is sent to that provider. Using Nexus without any
data leaving the machine requires a local model runtime and no configured
cloud provider.

## Closed and withdrawn surfaces

Surfaces that cannot yet be governed are closed or withdrawn rather than
offered as production features. In the completed Phase Zero Linux support
profile this includes, for example, screen observation, operating-system input control,
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
deployment of Nexus in the Phase Zero Linux support profile. An existing
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
  in Phase Zero or Phase One.
- The Phase One evidence is internal engineering evidence. It is not an
  external audit.
- Governed coding has no general shell, process or git authority. Its
  structural verification is not a guarantee that the code is correct.
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
- [Phase Three governed real-world control](docs/security/phase3-governed-real-world-control.md):
  the candidate's design record, bypass-closure inventory and non-claims. It
  is not a certification.
- [Security policy](SECURITY.md): how to report a vulnerability.
- [Deployment status](docs/DEPLOYMENT.md): what is and is not deployable.
  Phase One adds no server deployment.
- Phase One completion evidence: the `evidence/phase1-closure` branch
  (declaration, completion record, validation runs, non-claims). It is not a
  certification.

## Vision

In the long term, Nexus aims to be an operating environment in which agents
have verifiable identities, governed autonomy and tamper-evident audit
trails, running locally by default.

## License

[MIT](LICENSE)

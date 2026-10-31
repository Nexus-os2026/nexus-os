# Mother Nexus

*Your Mother. Our Home. Our Future.*

Mother Nexus is being built as a governed, local-first Hybrid Intelligence: it
turns a person's goal into a verified result. Independent agents research,
challenge, build and verify; a deterministic broker, not a language model,
controls every real effect; the person consents first, can stop anything, and
learns along the way.

It is not production-ready, and most of what that sentence describes is
**planned, not built**.

## What this repository is today

On 2026-10-31 the Owner replaced the old Nexus OS with Mother Nexus. Only the
parts of Nexus OS that are real, tested and useful to Mother were kept; everything
else was removed. The removed code (the old desktop app and its 91 pages,
about 45 crates, the marketing websites, server deployments and old evidence
documents) is preserved, unchanged, at the tag
[`archive/nexus-before-mother`](https://github.com/Nexus-os2026/nexus-os/tree/archive/nexus-before-mother)
(commit `12eec16c`).

There is **no desktop app in this repository right now**. The new Mother screen
is the next step. Builds of Nexus OS already installed keep working; they came
from the archived code.

| Crate | What it is | Mother system |
|---|---|---|
| `crates/nexus-governed-control` | The capability broker: Owner grants, commitments, native approval, emergency Stop, evidence store | Governed Hands, Guardian, Stop |
| `crates/nexus-verifier-sandbox` | Linux verification sandbox (namespaces, Landlock, seccomp); fails closed | Invention Laboratory, Verifier |
| `kernel` | Workspace authority, governed paths, resource limits, secrets, governed coding runs, historical memory (`hm`), the cognitive loop | Hands, Evidence Memory, Mission Brain |
| `persistence` | SQLite storage, including the fail-closed coding-run ledger | Evidence Memory |
| `connectors/llm` | Model gateway, including the loopback-only Ollama provider | Research Council |
| `crates/nexus-memory` | Hash-chained memory with contradiction detection | Evidence Memory |
| `agents/web-builder` | Notes-to-static-site builder (templates, content, assembly). Publishing and network image generation were removed | Builder (first demo) |
| `crates/nexus-self-improve` | Improvement proposals that are never applied by themselves | Evolution Engine |
| `crates/nexus-outcome-eval` | Success criteria fixed before execution | Verifier |
| `crates/nexus-crypto`, `sdk` | Shared primitives | — |

## Accepted foundation (Linux support profile)

These Nexus OS phases were completed and frozen before the cleanup. Their
checkpoints and evidence branches are unchanged; the cleanup does not
redefine them.

| Phase | Frozen checkpoint |
|---|---|
| Phase Zero — trust-boundary foundation | `rebuild/phase0-trust-boundary` at `f727f5c39fab8d5c729a55eb28ad576d3d56ce47` |
| Phase One — governed coding workflow | `rebuild/phase1-governed-coding` at `14270a9a38770ac84456c1f812042d2967edec42` |
| Phase Two — governed verification execution | `rebuild/phase2-governed-verification` at `dc52fadba078f2cddd6b51e26dd1a38db1ffedcc` |
| Phase Three — governed real-world control | commit `51af45efc70dfe2551db29991f074c4a00d836de` |

Evidence branches: `evidence/phase0-closure`, `evidence/phase1-closure`,
`evidence/phase2-closure`. Windows and macOS validation remains deferred.

## Non-claims

- Nothing in Mother Nexus's nine intelligence systems is accepted yet. A
  capability counts only after its physical test passes on the real build.
- No external audit, SOC 2, NIST or EU AI Act certification is claimed.
- Path validation is not an operating-system sandbox; a workspace grant is not
  subprocess confinement. The verifier sandbox is Linux x86_64 only.
- When a cloud model provider is configured, request data leaves the machine.
  Keeping everything local needs a local model runtime (Ollama) and no cloud
  provider.

## Security

See [SECURITY.md](SECURITY.md) for how to report a vulnerability.

## License

[MIT](LICENSE)

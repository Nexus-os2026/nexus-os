# Mother Nexus

*Your Mother. Our Home. Our Future.*

Mother Nexus is a governed, local-first Hybrid Intelligence being built along
one pathway: **goal → plan → governed execution → verified proof → teaching**.
The AI proposes, a deterministic broker controls effects, and the Owner decides
what matters. Most of the intelligence and the complete user journey are still
unavailable. This is not production-ready or a claim of a passed physical gate.

## Built and merged

The repository contains these implemented components. Implementation and tests
are separate from installed-build qualification and Owner acceptance.

| Component | What exists | Limit today |
|---|---|---|
| Governed coordinator (`mother/`) | Bounded mission contracts, hash-bound consent, deterministic transitions, lane budgets and Stop | Deterministic core, not a complete autonomous intelligence |
| Durable journal and evidence | Restart holds, denial and Stop durability, private content separated from evidence, fail-closed replay | Unkeyed chains are not protection against an attacker rewriting all history; private content is not encrypted |
| Broker door and halt | Owner-confirmed agent door, run binding, consent/resume controls, halt before recording and admission checks | Cancellation cannot undo an effect already performed |
| Files | Consent-bound file sessions, bounded publication, retained settlement obligations and truthful uncertainty/recovery | Uncertain publication is not clean success; no general file or shell authority |
| Local model transport | Pinned local identity, loopback-only transport, budgets, cancellation, broker evidence | The worker's conversation route is **shut until installed qualification** (`N1_QUALIFIED = false`); no automatic cloud fallback |
| Page checker | Inert-page policy, facts extraction and deterministic second reader | **Not rendered**; no viewport, layout or general content-correctness proof; its Mother lane is not wired |
| Typed decisions T1 (`crates/nexus-typed-decision`) | Code-owned registry, bounded framing, strict reader, fixed-point values, advice composition and single-use assessment slots | Pure core with no I/O; no live model transport or permission-granting decisions |
| Native desktop (`mother/desktop`) | GTK4 status window, Stop ingress, bounded Quit and honest status | Mission, door and resume dialogs decline; the full mission UI is not built |

See [mother/README.md](mother/README.md) for invariants, oracle mappings and
limits. [MOTHER.md](MOTHER.md) defines the charter and approvals;
[AGENTS.md](AGENTS.md) preserves the permanent engineering rules. Current
checkpoint and acceptance state come from Git and the latest approved mission.

## Still to build and prove

The first end-to-end local site mission, its model qualification and wiring,
verified safe preview, complete native dialogs and teaching handoff are not
accepted as a working whole. Independent live research, durable authenticated
memory, invention, self-improvement promotion, cloud escalation and companion
devices need their own approved work and evidence. No features are added beside
the main pathway; other ideas remain parked.

Mother is **Linux-first**. Windows and macOS are deferred, unvalidated targets.
There is no embedded browser engine in the native UI. A static page passing the
checker has not been rendered or proved correct.

## Build and test

Use the pinned Rust toolchain in `rust-toolchain.toml` (including its declared
Wasm target). Linux desktop builds also need GTK4 development libraries and
`pkg-config`; do not install host packages without the Owner's approval.
A local model runtime is not needed for the pure coordinator or typed-decision
tests. Live transport and sandbox tests need their separately qualified host.

```sh
cargo build --workspace --locked
cargo test -p nexus-mother -p nexus-typed-decision --locked
cargo test --workspace --locked --no-fail-fast
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
```

Follow the mission's shared Cargo slot when one is assigned. A test runner
reporting a live test as unavailable is not installed-host proof. The Architect
runs required CI on the exact candidate; local green alone is not acceptance.

For an interactive deterministic demonstration of door, consent and restart
holds (creates a fresh temporary home; calls no model):

```sh
cargo run -p nexus-mother-host --locked --example walk
```

To launch the limited GTK4 status window on a Linux desktop:

```sh
cargo run -p nexus-mother-desktop --locked --bin nexus-mother
```

## Relationship to the old product

The old Nexus OS is not this product. Its desktop, services, broad agents and
roadmaps are not Mother's runtime or backlog. Removed code remains in Git
history and the `archive/nexus-before-mother` tag. Retained crates contain
foundation code; their existence does not authorize Mother to call legacy
paths. The old Nexus must never break Mother Nexus. Builders use only their
approved worktree and keep isolation guards green.

## Security and licence

[SECURITY.md](SECURITY.md) explains reporting and non-claims;
[THREAT_MODEL.md](THREAT_MODEL.md) records current trust boundaries. No external
audit, certification or security-supported release is claimed.

[MIT](LICENSE)

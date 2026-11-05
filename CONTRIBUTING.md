# Contributing to Mother Nexus

Start with [MOTHER.md](MOTHER.md), [AGENTS.md](AGENTS.md) and the latest approved
mission. Work only on the main pathway: goal → plan → governed execution →
verified proof → teaching. An issue or old plan is not implementation authority.

## Development

Mother is Linux-first, with a native GTK4 UI. The pinned Rust toolchain and
components are in `rust-toolchain.toml`; GTK4 development libraries and
`pkg-config` are needed for desktop builds. Package installs, privileged steps
and service changes require the Owner. Use installed dependencies; do not copy
setup instructions from the retired app. Windows and macOS remain unvalidated.

Use only the assigned branch and worktree. Before editing, verify:

```sh
git status -sb
git rev-parse HEAD
```

Preserve unexpected changes and report a wrong base. Never repair provenance
with destructive Git. The approved mission controls commit, push and integration
permissions; the Owner merges.

Typical checks (the mission's exact gates take precedence):

```sh
cargo build --workspace --locked
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test -p nexus-mother -p nexus-typed-decision --locked
cargo test --workspace --locked --no-fail-fast
git diff --check
```

Use an assigned Cargo slot when sharing the host. Do not repeat the full suite
without new evidence or mission authorization. Live desktop, model and sandbox
checks require their intended host and installed-build evidence; a unit pass or
an unavailable live test is not physical acceptance.

## Boundaries

- `mother/` holds the coordinator; `mother/host` constructs the broker and owns
  the Owner side; `mother/desktop` presents native status and Stop.
- Mother reaches effects only through its sealed port and broker-issued door.
  Existing crates do not give Mother permission to use old actuators or services.
- `crates/nexus-typed-decision` is a pure advisory core. Its decisions cannot
  enlarge deterministic permission or override a failed check.
- Security-core changes, including guard tests and workflows, require explicit
  approval under the charter. Never amend a pre-registered oracle after seeing
  its result without an explicitly authorized oracle amendment.
- Do not log private content or secrets. Evidence uses bounded ids, hashes,
  counts and closed reason codes; private content stays separate.

## Review

Keep each change within its mission and explain the user benefit, files,
invariants, actual tests and limitations. Add meaningful behavioral tests when
required and update documentation without inventing completion or support.
Report all failures and omitted gates. Local green is not CI or Owner acceptance.
Use the PR template, provide reproducible evidence, and await independent review
and the Owner's integration decision. No adjacent feature work is implied.

See [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md) for participation standards and
[SECURITY.md](SECURITY.md) for private reports.

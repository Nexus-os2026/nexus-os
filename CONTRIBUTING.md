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

## Rust conventions and required checks

Rust code uses Edition 2021. Preserve the `unsafe_code = "forbid"` policy. Errors use `thiserror` or custom enums; do not use `unwrap()` in library code.

Public data types derive `Debug`, `Clone`, `Serialize` and `Deserialize` where
their contract permits. Preserve deliberate custom implementations and types
that withhold cloning or serialization for secret/authority safety; do not add
those traits mechanically. Use UUID v4 for fresh UUID identifiers; preserve
existing typed, hash-derived and sequence identifiers rather than replacing
their identity contract. An identifier alone never grants authority.

New workspace crates keep the `nexus-` package prefix and must pass `cargo check`.
Agents using the retained SDK depend on `nexus-sdk`, never directly on
`nexus-kernel`; this does not authorize Mother to import that stack. Governed
agents retain manifests, capability checks, fuel tracking and audit events.

Check capabilities and fuel/budgets before every agent/tool action, perform
action-level validation, and obtain human approval at the applicable sensitive
operation tier (including legacy L2+). Record LLM/tool transitions and state
changes through the approved audit path; retained kernel events use
`AuditTrail::append_event()`. Mother uses its durable broker/journal receipts;
the legacy audit appender is not permission to suppress a recording failure.
Verify fuel accounting, prompt/output defenses and suspicious-output circuit
breakers on retained LLM paths; none substitutes for deterministic authority.

All applicable formatting, zero-warning Clippy, behavioral tests and required
CI checks must pass before merge. Run the required local checks before opening
a PR, and report any unavailable gate as blocked. Keep one focused feature or
fix per PR, test behavior changes, update public API/configuration documentation,
and write commit messages explaining why. Follow the mission's approved base,
not a stale branch instruction or historical test count.

## Native UI review

The retired web skill's platform-independent requirements still apply to GTK4:

- Provide visible keyboard focus, logical tab order, labels for inputs and
  accessible names for icon-only controls; meaningful images need descriptions.
  Never use color as the only indicator. Normal text needs at least 4.5:1
  contrast. Check both light and dark modes, including muted text and borders.
- Keep primary actions available by click/tap and keyboard, with at least
  44×44 logical-pixel touch targets where touch applies. Show clear interaction
  feedback, disable duplicate submission during asynchronous work, and put
  errors near the affected control. Use the platform's appropriate cursor.
- Reserve space for asynchronous content, show loading status, optimize image
  loading, and prevent content jumps or hover effects that shift layout. Respect
  reduced-motion preferences; keep optional micro-interactions brief (150–300 ms)
  and avoid layout-changing animation.
- Keep text readable (at least 16 logical pixels on small screens), with
  comfortable line height (1.5–1.75) and line length (65–75 characters). Use
  consistent fonts, spacing, content widths and layer order. Fixed/floating
  controls must not hide content; avoid unintended horizontal scrolling and
  check narrow through wide layouts (375, 768, 1024 and 1440 logical pixels).
- Match visual style to the product and use a consistent icon set and sizing.
  Use vector icons rather than emoji controls, verify brand assets against
  official sources, and keep theme colors consistent. Charts need an appropriate
  type, an accessible palette and a table alternative. Check these requirements
  before delivery; web-specific CSS/ARIA/tooling instructions do not authorize
  an embedded browser or resurrection of the removed UI.

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

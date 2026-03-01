# NEXUS OS

**Tagline:** Don't trust. Verify.

NEXUS OS is a governed agent operating system built in Rust. It runs policy-bounded agents with explicit capabilities, fuel budgets, audit trails, and human approval boundaries.

## Installation

### Linux

1. Download latest `.deb` artifact from Releases.
2. Install: `sudo dpkg -i nexus-os_<version>_amd64.deb`
3. Verify: `nexus --help`

### macOS

1. Download latest macOS artifact (`.dmg` or package tarball).
2. Install binary into your PATH.
3. Verify: `nexus --help`

### Windows

1. Download latest `.msi` artifact.
2. Run installer.
3. Verify in terminal: `nexus --help`

For source-based installation on all platforms:

1. Install Rust stable.
2. Clone this repository.
3. Build CLI: `cargo build --release -p nexus-cli`

## Quick Start (3 Commands)

```bash
nexus setup
nexus agent create agents/social-poster/manifest.toml
nexus agent start social-poster --dry-run
```

This creates and runs the first real production agent pipeline (research → generate → review → publish simulation) without posting to live APIs.

## Screenshots

- `[Screenshot Placeholder] Desktop Dashboard (Chat / Agents / Audit / Settings)`
- `[Screenshot Placeholder] Audit Explorer Integrity View`
- `[Screenshot Placeholder] Social Poster Dry-Run Output`

## Documentation

- User guide: [docs/USER_GUIDE.md](docs/USER_GUIDE.md)
- Developer guide: [docs/DEVELOPER_GUIDE.md](docs/DEVELOPER_GUIDE.md)
- Threat model: [THREAT_MODEL.md](THREAT_MODEL.md)
- Changelog: [CHANGELOG.md](CHANGELOG.md)
- Security policy: [SECURITY.md](SECURITY.md)

## Core Principles

1. Capability-limited execution.
2. Tamper-evident audit logging.
3. Explicit approval for sensitive actions.
4. Secure update and package provenance verification.

## License

TBD

# NexusOS User Guide

## Who This Guide Is For

This guide is for operators who want to install NexusOS, configure real integrations, and run their first governed agent in minutes.

## Getting Started (5 Minutes To First Agent)

1. Install NexusOS CLI (platform steps below).
2. Run setup wizard: `nexus setup`
3. Create first agent: `nexus agent create agents/social-poster/manifest.toml`
4. Start safely in demo mode: `nexus agent start social-poster --dry-run`
5. Inspect results: `nexus agent logs social-poster`

If you want live posting after dry-run validation, run:
`nexus agent start social-poster`

## Installation

### Linux

Option A (package):
1. Download the latest `.deb` release artifact.
2. Install: `sudo dpkg -i nexus-os_<version>_amd64.deb`
3. Verify: `nexus --help`

Option B (from source):
1. Install Rust stable.
2. Clone repository.
3. Build CLI: `cargo build --release -p nexus-cli`
4. Run: `./target/release/nexus-cli --help`

### macOS

Option A (release asset):
1. Download the macOS release artifact (`.dmg` or tar package).
2. Install binary to your PATH.
3. Verify: `nexus --help`

Option B (from source):
1. Install Rust stable (`rustup`).
2. Clone repository.
3. Build CLI: `cargo build --release -p nexus-cli`
4. Run: `./target/release/nexus-cli --help`

### Windows

Option A (installer):
1. Download the latest `.exe` (NSIS) or `.msi` release artifact.
2. Install via installer UI.
3. Open terminal and verify: `nexus --help`

Option B (from source):
1. Install Rust stable (MSVC toolchain).
2. Clone repository.
3. Build CLI: `cargo build --release -p nexus-cli`
4. Run: `target\\release\\nexus-cli.exe --help`

## Setup Wizard Walkthrough

Run:
`nexus setup`

Wizard flow:
1. Prompts for optional keys (Anthropic, Brave, Telegram, and others).
2. Validates keys with minimal network checks where supported.
3. Saves encrypted config at `~/.nexus/config.toml`.

Check current status:
`nexus setup --check`

Output shows configured/unconfigured services (example: `Anthropic: ✓ configured`).

## Creating Your First Agent

Use the included social poster manifest:

1. `nexus agent create agents/social-poster/manifest.toml`
2. `nexus agent start social-poster --dry-run`
3. `nexus agent logs social-poster`

What this agent does:
1. Searches web for topic updates.
2. Reads top sources.
3. Generates post copy.
4. Runs compliance checks.
5. Publishes (live mode) or prints content (dry-run).
6. Records audit events for each step.

## Using Voice (Jarvis Mode)

CLI voice commands:
1. `nexus voice start`
2. `nexus voice test`
3. `nexus voice models`

Expected behavior:
- Wake word detection starts listening loop.
- Speech is transcribed locally.
- Query is sent through governed LLM gateway.
- Response is synthesized locally.

If voice dependencies are missing, run the Python setup used in `voice/` and retry `nexus voice test`.

## Using Telegram Remote Control

Prerequisite:
- Configure Telegram bot token via `nexus setup`.

Core commands (from Telegram chat):
- `status`
- `start <agent>`
- `stop <agent>`
- `approve <id>`
- `logs <agent>`

Pairing flow:
1. Send `/pair` in Telegram.
2. Enter pairing code in desktop app or CLI.
3. Device becomes authorized for future commands.

Unpaired chat IDs are blocked by design.

## Agent Factory (Natural Language Agent Creation)

Agent Factory converts intent into governed manifests and code scaffolds.

Typical flow:
1. Describe intent in natural language.
2. Factory maps required capabilities.
3. Approval gate confirms requested authority and fuel.
4. Manifest/code is generated and optionally deployed.

This enables fast prototype creation while preserving least-privilege guardrails.

## Troubleshooting FAQ

### `nexus setup` says key validation failed

- Confirm the key is correct and active.
- Verify outbound network access.
- Retry with `nexus setup --check`.

### `nexus agent start social-poster` fails in live mode

- Verify required keys:
  - Search provider key
  - LLM provider key (or local Ollama)
  - X credentials for live posting
- Test with `--dry-run` first.

### `nexus agent start social-poster --dry-run` should never post

Correct. Dry-run runs the full pipeline and audit logging but does not perform real X posting.

### Voice command fails

- Ensure Python voice dependencies are installed.
- Run `nexus voice test` to isolate runtime issues.
- Check microphone permissions.

### Desktop app build fails on Linux

- If bundling errors occur, use non-bundled build path and verify `tauri build` config.
- Ensure required system libraries are present for your distro.

### Desktop installer artifacts by platform

- Windows desktop: `.exe` (NSIS installer) and `.msi`
- macOS desktop: `.dmg`
- Linux desktop: `.AppImage`, `.deb`, `.rpm`

### Where is configuration stored?

`~/.nexus/config.toml` (encrypted at rest via kernel privacy module).

### How do I inspect audit trails?

- CLI: `nexus agent audit <agent_id>`
- Desktop: Audit page with chain integrity indicator.

## Next Steps

1. Run social-poster in dry-run every day until content quality is stable.
2. Enable live posting when compliance and quality checks pass.
3. Use Agent Factory for your second production workflow.

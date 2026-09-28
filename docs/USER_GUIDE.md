# NexusOS User Guide

## Who This Guide Is For

This guide is for operators who want to install NexusOS, configure real integrations, and run their first governed agent in minutes.

## Getting Started

Install the NexusOS desktop app with the installer for your platform (below) and start it.

The `nexus` command-line interface is withdrawn during Phase Zero (see [Command-line interface (withdrawn)](#command-line-interface-withdrawn)).

## Installation

### Linux

Option A (package):
1. Download the latest `.deb` release artifact.
2. Install: `sudo dpkg -i nexus-os_<version>_amd64.deb`

### macOS

Option A (release asset):
1. Download the `.dmg` release artifact.
2. Install the app from the `.dmg`.

### Windows

Option A (installer):
1. Download the latest `.exe` (NSIS) or `.msi` release artifact.
2. Install via installer UI.

### From source (all platforms)

Option B: build and run the desktop app from source as described in the README ("Build and Run").

## Command-line interface (withdrawn)

`nexus-cli` (the `nexus` command) is withdrawn during Phase Zero. Every invocation prints `nexus-cli: unavailable during Phase Zero; standalone use withdrawn` and exits with status 69; it runs no command. The repository provides no supported installation of it: its release job, packages, service units and installer recipes are withdrawn too.

Its commands (the setup wizard, `agent`, `voice`, `marketplace`, `conduct`, `create`, `test` and `package`) are therefore unavailable, and so are the command-line walkthroughs this guide used to give for them. The standalone `coding-agent` and `social-poster-agent` executables, the `nx` terminal and the `nx-*` computer-use tools are withdrawn as well.

An existing installation of these binaries, or a service installed by an earlier package, is not removed or stopped by this change.

## Using Telegram Remote Control

Prerequisite:
- Configure a Telegram bot token.

Core commands (from Telegram chat):
- `status`
- `start <agent>`
- `stop <agent>`
- `approve <id>`
- `logs <agent>`

Pairing flow:
1. Send `/pair` in Telegram.
2. Enter pairing code in the desktop app.
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

- Desktop: Audit page with chain integrity indicator.

## Next Steps

1. Use Agent Factory for your first production workflow.

[![CI](https://github.com/nexai-lang/nexus-os/actions/workflows/ci.yml/badge.svg)](https://github.com/nexai-lang/nexus-os/actions/workflows/ci.yml)

# NexusOS
**The Autonomous Digital Worker Platform**

> Don't trust. Verify.

NexusOS is a governed platform for autonomous AI workers that can code, post, build, and automate real work end-to-end. It combines a Rust control plane, desktop operations interface, and policy-based runtime so every action is auditable and human-controllable. Teams use it to ship faster without giving up safety, traceability, or local-first privacy.

## What Can NexusOS Do?

- 🤖 **AI Coding Agent** — Reads codebases, writes code, runs tests, fixes bugs. Competes with Claude Code and Cursor.
- 📱 **Screen Poster** — Posts on X, Instagram, Facebook, Reddit like a human. No APIs needed — uses screen vision. All posts human-approved.
- 🌐 **Website Builder** — Describe a website in English, get a full 3D site with Three.js. Like Lovable but governed.
- ⚡ **Workflow Engine** — Visual drag-and-drop automation like n8n. AI-powered at every node.
- 🎨 **Design Agent** — Generates UI components, design systems, screenshot-to-code. Like Figma AI.
- 🧠 **Self-Improving** — Agents learn from every task. They get better over time. No other platform does this.

## Core Architecture

- **Governed**: Every agent is capability-bounded, fuel-limited, fully audited
- **Forensic Replay**: Prove exactly what every agent did
- **Voice Control**: 100% local Whisper + Piper. No audio leaves your machine
- **Messaging**: Control agents from Telegram, WhatsApp, Discord, Slack
- **Privacy-First**: AES-256 encryption, cryptographic erasure for GDPR
- **Cross-Platform**: Windows, macOS, Linux

## Quick Start

```bash
# Install (Linux)
curl -fsSL https://nexus-os.dev/install.sh | sh

# Or download from GitHub Releases
# Windows: NexusOS_x64-setup.exe
# macOS: NexusOS_aarch64.dmg
# Linux: nexus-os_amd64.deb

# Setup
nexus setup

# Create your first agent
nexus agent create --template social-poster

# Launch desktop app
nexus app
```

## Screenshots

Screenshots coming soon.  
Note: add desktop dashboard, workflow canvas, and audit replay captures in the next release update.

## Tech Stack

Rust (kernel) + TypeScript/React (desktop app via Tauri) + Python (voice/ML)  
120+ tests | CI/CD on GitHub Actions | 28 milestone versions

## Documentation

- [User Guide](docs/USER_GUIDE.md)
- [Developer Guide](docs/DEVELOPER_GUIDE.md)
- [Threat Model](docs/THREAT_MODEL.md)
- [Privacy Design](PRIVACY_DESIGN.md)
- [Changelog](CHANGELOG.md)

## Built By

Created by Devil — a self-taught developer who built an entire governed agent operating system from scratch.

## License

TBD

---

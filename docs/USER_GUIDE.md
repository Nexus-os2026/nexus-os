# NEXUS OS User Guide

## Getting Started

### 1. Prerequisites
- Rust stable toolchain (`rustup`, `cargo`)
- Git
- Linux/macOS/Windows shell environment

### 2. Clone and Build
```bash
git clone https://github.com/nexai-lang/nexus-os.git
cd nexus-os
cargo build --workspace
```

### 3. Run Verification
```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
```

## Creating Your First Agent

### 1. Define Intent
Example intent:
`Create an agent that posts about Rust on Twitter every morning at 9am`

### 2. Generate Factory Artifacts
NEXUS Factory converts intent into:
- parsed intent
- capability plan
- manifest
- generated agent code
- approval request

### 3. Approve and Deploy
Deployment only proceeds after explicit approval of:
- capabilities
- fuel budget
- execution schedule

## Voice Setup

Voice support is local-first and lives in `voice/`.

### 1. Install Python Dependencies
```bash
cd voice
python3 -m venv .venv
source .venv/bin/activate
python3 -m pip install -e .
```

### 2. Run Voice Tests
```bash
python3 -m pytest
```

### 3. Voice Flow
- wake word detection
- voice activity detection
- streaming speech-to-text
- request handling through governed agent runtime

## Secure Updates

NEXUS OS includes:
- TUF metadata verification (root/targets/snapshot/timestamp)
- in-toto attestation checks
- canary deploy and rollback
- opt-in research-preview self-patching with fixed verifier boundary

## Troubleshooting

- If tests fail, run crate-scoped tests first:
  - `cargo test -p nexus-self-update`
  - `cargo test -p nexus-factory`
- If formatting fails:
  - `cargo fmt --all`
- If clippy fails:
  - fix warnings before merge (`-D warnings` in CI)

# Contributing to NexusOS

Thanks for contributing to NexusOS.

## Development Setup

1. Install Rust stable and Cargo.
2. Install Node.js 20+ and npm.
3. Install Python 3.11+.
4. Clone the repo and enter the workspace:

```bash
git clone https://github.com/nexai-lang/nexus-os.git
cd nexus-os
```

## Run Tests

Run the full Rust suite:

```bash
cargo test --workspace
```

Run voice tests:

```bash
cd voice
python3 -m pytest -v
```

Run frontend build checks:

```bash
cd app
npm run build
```

## Code Style

Before opening a PR:

```bash
cargo fmt --all -- --check
cargo clippy --workspace -- -D warnings
```

## Pull Request Process

1. Create a focused branch from `main`.
2. Keep changes scoped and include tests when behavior changes.
3. Update docs for public-facing changes.
4. Ensure CI passes before requesting review.
5. Use clear commit and PR descriptions.

## Code of Conduct

By participating, you agree to collaborate respectfully and follow the
[GitHub Community Code of Conduct](https://docs.github.com/en/site-policy/github-terms/github-community-code-of-conduct).

# NEXUSCODE.md

## Project
name: nexus-os
language: rust
build: cargo build
test: cargo test
lint: cargo clippy -- -D warnings

## Governance
fuel_budget: 50000
blocked_paths: .env, .env.local

## Models
execution: anthropic/claude-sonnet-4-20250514

## Style
prefer_short_responses: true
auto_run_tests_after_edit: true

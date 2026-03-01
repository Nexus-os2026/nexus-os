# Changelog

## v0.5.0 - Early Access

- Added `nexus-content` crate with:
  - LLM-backed platform-aware content generation (`content/generator.rs`)
  - Persistent content calendar scheduling (`content/calendar.rs`)
  - ToS compliance checks and platform limits (`content/compliance.rs`)
- Added social publishing connectors:
  - Facebook Graph connector (`connectors/social/facebook.rs`)
  - Instagram connector (`connectors/social/instagram.rs`)
- Added sequential workflow orchestration (`workflows/sequential.rs`):
  - research -> generate -> review -> publish
  - continue-on-failure behavior per platform
  - idempotent publish request IDs to prevent duplicates
- Added compliance documentation (`COMPLIANCE.md`).

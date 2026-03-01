# Changelog

## v1.0.0 - Production Release

- Integrated end-to-end cross-crate pipeline tests in `tests/integration/`:
  - agent creation → research → content generation → publishing → analytics → adaptation → report
- Added governance overhead benchmark test with release budget target (<5% overhead).
- Finalized secure update stack:
  - TUF verification with rollback/freeze protection
  - signed package verification and in-toto attestation checks
  - canary deploy with rollback
  - connector auto-update and restart flow
  - opt-in research-preview self-patching with fixed verifier boundary
- Added production documentation:
  - `docs/USER_GUIDE.md`
  - `THREAT_MODEL.md`
- Added installer/package assets:
  - Linux `.deb` packaging script + `systemd` service
  - macOS Homebrew formula + `launchd` plist
  - Windows `.msi` (WiX manifest) packaging script
- Upgraded CI/CD for matrix build/test across Ubuntu, macOS, and Windows, with release packaging workflow.

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

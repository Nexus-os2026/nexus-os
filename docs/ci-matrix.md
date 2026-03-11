# CI Test Matrix

> Complete reference for the Nexus OS CI pipeline, test coverage, and feature flags.

## CI Stages & Jobs

| Stage | Job | Image | Trigger | What It Tests |
|-------|-----|-------|---------|---------------|
| **security** | `cargo-audit` | `rust:latest` | Every push | Known vulnerability advisories via `cargo audit` |
| **security** | `cargo-deny` | `rust:latest` | Every push | License compliance via `cargo deny check -W unlicensed` |
| **test** | `rust-tests` | `rust:latest` | Every push | `cargo fmt --check`, `cargo clippy --all-features`, `cargo test --all-features` |
| **test** | `voice-tests` | `python:3.12` | Every push | Python voice module unit tests (STT, TTS, VAD, wake word, Jarvis pipeline) |
| **test** | `frontend-tests` | `node:22` | Every push | TypeScript build, Node.js smoke tests, `tsc --noEmit` type checking |
| **test** | `code-coverage` | `rust:latest` | Every push | `cargo-tarpaulin` coverage report (Cobertura XML) — `allow_failure: true` |
| **release** | `build-release` | `rust:latest` | Tag only | Release build, SBOM generation, provenance attestation |
| **release** | `build-agent-bundles` | `rust:latest` | Tag only | Package agent manifest bundles for marketplace distribution |
| **sign** | `sign-release` | `rust:latest` | Tag only | Cosign/Sigstore signing of binaries, SBOM, and provenance |
| **sign** | `verify-signatures` | `rust:latest` | Tag only | Verify signing manifest against artifacts |
| **sign** | `verify-sbom` | `python:3.12` | Tag only | Validate SBOM structure (>=100 Rust crates, >=10 npm packages) |

## Feature Flags by Crate

All feature flags are compiled and tested in CI via `--all-features`.

| Crate | Feature | Purpose |
|-------|---------|---------|
| `nexus-kernel` | `hardware-tpm` | Hardware TPM integration for key storage |
| `nexus-kernel` | `hardware-secure-enclave` | Apple Secure Enclave support |
| `nexus-kernel` | `hardware-tee` | Trusted Execution Environment support |
| `nexus-connectors-llm` | `real-claude` | Live Claude API calls (requires API key) |
| `nexus-connectors-llm` | `real-api-tests` | Integration tests against real LLM APIs |
| `nexus-connectors-llm` | `local-slm` | Local small language model via candle |
| `nexus-control` | `playwright-process` | Browser automation via Playwright subprocess |
| `nexus-control` | `platform-linux` | Linux-specific control features |
| `nexus-control` | `platform-macos` | macOS-specific control features |
| `nexus-control` | `platform-windows` | Windows-specific control features |
| `nexus-desktop-backend` | `tauri-runtime` | Tauri desktop shell (default feature) |

All other ~30 workspace crates have no feature flags.

## Test Count by Crate

Approximate counts from `cargo test --workspace --all-features` (~1,376 total, 5 ignored):

| Crate | Unit | Integration | Total |
|-------|------|-------------|-------|
| `nexus-kernel` | 413 | 68 | **481** |
| `nexus-sdk` | 159 | 26 | **185** |
| `nexus-connectors-llm` | 100 | 18 | **118** |
| `nexus-cli` | 92 | 10 | **102** |
| `nexus-distributed` | 72 | 4 | **76** |
| `nexus-marketplace` | 58 | 8 | **66** |
| `nexus-protocols` | 45 | 11 | **56** |
| `coder-agent` | 25 | 9 | **34** |
| `nexus-collaboration` | 22 | — | **22** |
| `nexus-cloud` | 22 | — | **22** |
| `nexus-enterprise` | 21 | — | **21** |
| `nexus-control` | 15 | — | **15** |
| `nexus-integration` (e2e) | — | 14 | **14** |
| `nexus-desktop-backend` | 12 | — | **12** |
| `nexus-research` | 12 | — | **12** |
| Other agents & crates | ~120 combined | | **~120** |

### Voice Tests (Python)

12 tests across 6 files in `voice/tests/`:

| File | Tests | Covers |
|------|-------|--------|
| `test_stt.py` | 1 | Whisper model selection by hardware |
| `test_tts.py` | 1 | Piper TTS synthesis |
| `test_vad.py` | 1 | Voice activity detection |
| `test_wake_word.py` | 1 | "Hey NEXUS" wake word detection |
| `test_jarvis.py` | 2 | Confirmation approval, latency tracking |
| `test_real_backends.py` | 6 | Backend discovery, env overrides, fallbacks |

### Frontend Tests (Node.js)

1 smoke test in `app/tests/smoke.test.js` — verifies scaffold files exist (Chat, Dashboard, Audit pages, VoiceOverlay, PushToTalk, Tauri main.rs).

Type checking via `tsc --noEmit` covers the full TypeScript codebase.

## Coverage Reporting

The `code-coverage` job generates a Cobertura XML report using `cargo-tarpaulin`.

- **CI artifact**: `coverage/cobertura.xml` (retained 30 days)
- **GitLab integration**: Coverage percentage is extracted via the regex `/(\d+\.\d+)% coverage/` and displayed on the pipeline page
- **MR diffs**: GitLab renders inline coverage annotations on merge request diffs using the Cobertura report

Coverage is currently advisory (`allow_failure: true`). Once a baseline is established, a minimum threshold will be enforced.

## Running the Full Test Suite Locally

```bash
# Rust: format, lint, test
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features

# Python voice tests
cd voice && python3 -m pytest tests/ -v && cd ..

# Frontend: build, test, lint
cd app && npm ci && npm run build && npm test && npm run lint && cd ..
```

Or use the pre-commit skill which runs the core checks:
```bash
# cargo fmt --check, cargo clippy, cargo test, npm run build
```

## Adding New Tests

### Rust Unit Tests

Place `#[cfg(test)] mod tests { ... }` at the bottom of the source file:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn my_new_test() {
        // ...
    }
}
```

### Rust Integration Tests

Create a file under the crate's `tests/` directory (e.g., `kernel/tests/my_feature_tests.rs`). Integration tests have access to the crate's public API only.

### Feature-Gated Tests

Use `#[cfg(feature = "...")]` to gate tests behind a feature flag:

```rust
#[cfg(feature = "hardware-tpm")]
#[test]
fn test_tpm_key_storage() {
    // Only compiled and run with --all-features or --features hardware-tpm
}
```

CI runs `--all-features`, so all feature-gated tests execute in the pipeline.

### Python Voice Tests

Add `test_*.py` files to `voice/tests/` using `unittest.TestCase`:

```python
import unittest

class MyTests(unittest.TestCase):
    def test_something(self):
        self.assertTrue(True)
```

The custom `pytest.py` shim discovers tests via `unittest.TestLoader.discover("tests")`. No external dependencies needed.

### Frontend Tests

Add `*.test.js` files to `app/tests/` using Node.js built-in `node:test`:

```javascript
import assert from "node:assert/strict";
import test from "node:test";

test("my test", () => {
    assert.equal(1 + 1, 2);
});
```

### Naming Conventions

| Language | Location | Pattern |
|----------|----------|---------|
| Rust unit | `<crate>/src/*.rs` | `#[cfg(test)] mod tests` |
| Rust integration | `<crate>/tests/*.rs` | Descriptive snake_case filename |
| Python | `voice/tests/` | `test_<module>.py` |
| JavaScript | `app/tests/` | `<name>.test.js` |

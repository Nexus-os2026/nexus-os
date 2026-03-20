# Security Advisories

## Known Vulnerabilities

### RUSTSEC-2023-0071 — RSA Marvin Attack (Medium, CVSS 5.9)

- **Affected crate**: `rsa` 0.9.10 (transitive via `openidconnect`)
- **Status**: No upstream fix available as of March 2026
- **Impact**: Timing side-channel in RSA decryption
- **Risk for Nexus OS**: LOW — Nexus OS uses OIDC for authentication tokens, not RSA decryption. The vulnerable code path is not exercised in typical desktop OIDC flows.
- **Mitigation**: Monitoring upstream for fix. Will update when `openidconnect` releases patched version.
- **Tracking**: https://rustsec.org/advisories/RUSTSEC-2023-0071

## Unmaintained Dependencies (18 warnings)

These are transitive dependencies pulled in by Tauri and tokenizers. They are not directly used by Nexus OS code:

- GTK3 bindings (via Tauri Linux support)
- unic-* crates (via Tauri)
- paste, fxhash, proc-macro-error, number_prefix (via tokenizers/scraper)

**Mitigation**: These will resolve when Tauri and tokenizers update their dependencies. No action required from Nexus OS.

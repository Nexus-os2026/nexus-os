# Security gates (fresh advisory databases, the pinned scanners)

The scanners are those CI pins: `cargo-audit` 0.22.1 and `cargo-deny` 0.19.6
(`scripts/security-audit.sh` checks both versions). Each run fetched its
databases fresh into its own empty directory (`SECURITY_AUDIT_DB_ROOT`).
Both runs used RustSec commit `ef6173cbc5c50ec8166f9a5b28f07834144373ee`
(2026-10-03), the same commit the RustSec facts were read from.

## The baseline: closure CI run 37191730167 on `a7001311`

`baseline/`: the run and job records, the security job's log identity
(193,979 bytes, SHA-256 `a236d959…`) and its scanner excerpt:
`error: 1 vulnerability found!` (RUSTSEC-2026-0327, wasmtime 43.0.2, CVSS
9.3, critical) and `warning: 16 allowed warnings found`. `cargo deny` never
ran (the script stops at the failed `cargo audit`).

## 1. Repaired lockfile, unchanged exception set (`security/pre-deny-edit-security-audit.txt`)

`scripts/security-audit.sh`, exit 1:

- `cargo audit` with the 11 base exceptions: no vulnerability; 15 allowed
  warnings.
- `cargo deny`: `advisories FAILED, bans ok, licenses ok, sources ok`. There
  were six `advisory-not-detected` errors, one for each exception that no
  longer matches anything:

| Exception | Package | Why it no longer matches |
|---|---|---|
| RUSTSEC-2026-0269 | wasmtime | patched in 36.0.14 (and later 36.x) |
| RUSTSEC-2026-0222 | wasmtime | patched in 36.0.13 (and later 36.x) |
| RUSTSEC-2026-0316 | wasmtime | patched in 36.0.16 (and later 36.x) |
| RUSTSEC-2026-0247 | bitmaps 2.1.0 | removed from the graph with `wasm-compose` |
| RUSTSEC-2026-0250 | im-rc 15.1.0 | removed from the graph with `wasm-compose` |
| RUSTSEC-2026-0251 | sized-chunks 0.6.5 | removed from the graph with `wasm-compose` |

These six entries, and only these, were removed from `deny.toml`. The other
five (RUSTSEC-2026-0193 and -0213 ammonia; -0258 h2; -0194 and -0195
quick-xml) are untouched: their findings remain. RUSTSEC-2026-0327 was not
added.

## 2. The candidate (`security/final-security-audit.txt`)

`scripts/security-audit.sh --install` (CI's mode; the pinned scanners were
already installed at those versions, so the install was a no-op), exit 0:

| Gate | Result |
|---|---|
| `cargo audit` (5 exceptions) | no vulnerability; 15 allowed warnings |
| `cargo deny --locked check` (`-W unmaintained -W unsound -D advisory-not-detected`) | `advisories ok, bans ok, licenses ok, sources ok` |
| Script | `security-audit: passed` |

`-D advisory-not-detected` passing proves every remaining exception still
matches a finding, so none is stale.

## 3. No exceptions at all (`security/final-cargo-audit-no-ignores.json`)

`cargo audit --json` with no `--ignore`, over all 1,092 lockfile packages,
reports 5 vulnerabilities: the 5 accepted ones, each still justified by its
own unchanged `deny.toml` reason:

| ID | Package |
|---|---|
| RUSTSEC-2026-0193 | ammonia 4.1.2 |
| RUSTSEC-2026-0213 | ammonia 4.1.2 |
| RUSTSEC-2026-0258 | h2 0.3.27 |
| RUSTSEC-2026-0194 | quick-xml 0.30.0 |
| RUSTSEC-2026-0195 | quick-xml 0.30.0 |

The scan also reports 15 warnings (10 unmaintained, 5 unsound). It reports no
Wasmtime-family finding of any kind (no wasmtime, cranelift, pulley, winch,
wasm or wit package), and RUSTSEC-2026-0327 is absent.

The 15 warnings are the closure run's 16 minus `sized-chunks`' unsoundness
(RUSTSEC-2026-0255), removed with `wasm-compose`. They are reported, not
failed, under the existing policy:
- unmaintained: fxhash, number_prefix, paste, proc-macro-error,
  rustls-pemfile, and five unic-* crates;
- unsound: glib, lru (2), rand, scc.

None comes from the Wasmtime line.

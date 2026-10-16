# Stage 4 — security and dependency gates

## 1. Rust dependency gate (fast-linux step 17)

Exact step (workflow at C): installs the pinned scanners into the runner tool cache only if absent, then
`SECURITY_AUDIT_DB_ROOT="$RUNNER_TEMP/security-audit-db" scripts/security-audit.sh`, which **fetches cargo-audit's
`advisory-db` and cargo-deny's `advisory-dbs` fresh** and fails on a vulnerability outside `deny.toml`'s exception set,
a stale exception, or any scanner failure. The script supports `--no-fetch` only with a reviewed database snapshot;
the workflow does not use it.

| Item | Local state | Disposition |
|---|---|---|
| cargo-audit 0.22.1 | present: `~/.cargo/bin/cargo-audit` reports `cargo-audit-audit 0.22.1` | version matches the pin |
| cargo-deny 0.19.6 | present: `~/.cargo/bin/cargo-deny` reports `cargo-deny 0.19.6` | version matches the pin |
| fresh advisory databases | require network | **not fetched; not authorised** |
| reviewed offline snapshot | none identified or authorised | `--no-fetch` would be a different gate |

**Status: NOT DISCHARGED.** No scan was run. A scan against an unreviewed, stale local database would not be the
workflow's gate and is not reported as one.

## 2. npm advisory gate (fast-frontend steps 21–22)

`node --test scripts/ci/npm-advisory-gate.test.mjs` (Node 22) and `node scripts/ci/npm-advisory-gate.mjs` (a fresh
`npm audit` of every audited npm root against `security/npm-advisory-policy.json`). **Status: NOT DISCHARGED** — Node
22 is unavailable locally and `npm audit` needs the registry.

## 3. Dependency integrity of what was run

| Check | Result | Receipt |
|---|---|---|
| `Cargo.lock` at C | blob `1af53a60`, sha256 `6ed84a9a…d979`; 1,099 packages: 72 path, 1,027 registry, 0 git | `raw/Cargo.lock.at-C`, `raw/00-lock-offline-check.json` |
| every locked registry crate cached with the lock's checksum | 1,027 of 1,027 `.crate` files present in `~/.cargo/registry/cache/index.crates.io-1949cf8c6b5b557f/`, every sha256 equal to the lock's checksum | `raw/00-lock-offline-check.json` (`tools/lock_offline_check.py`) |
| every locked registry crate already extracted | 0 not yet extracted; Cargo wrote no new source tree under `~/.cargo/registry/src` | `raw/04-registry-src-extraction-check.txt` |
| the Clippy run touched the network | no `Downloading`, `Downloaded` or `Updating` line; `CARGO_NET_OFFLINE=true`; `--locked` | `receipts/43b-clippy-all-features-r2/CHECKS.txt` |
| dependency mutation | none: no `cargo update`, no install, no lockfile write; the worktree's tracked bytes equal C after the run | `receipts/43b-clippy-all-features-r2/RECEIPT.txt` |

These show only that the evidence of this mission used exactly C's locked dependency set. They are **not** an
advisory scan and discharge neither security gate.

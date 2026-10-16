# Fast Local — final

**Is full Fast Local genuinely discharged? No: FAST_LOCAL_NOT_FULLY_DISCHARGED.**

| Evidenced at exact Candidate C | Command | Result |
|---|---|---|
| checkout verification (all jobs) | the workflow's own `git rev-parse HEAD` / `git status --porcelain=v1 --untracked-files=all --ignored` | PASS (this mission) |
| Rust 1.94.0 and system libraries | the workflow's own checks | PASS (this mission) |
| formatting | `cargo fmt --all -- --check` | PASS (reused exact-source receipt) |
| **Clippy, all features** | `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` | **PASS, run once by this mission**: exit 0, 0 warnings, 372.12 s, memory.peak 6,374,899,712 B, swap 0 |
| workspace tests | `cargo test --workspace --locked` | PASS 8,764 / 0 / 78 (reused exact-source receipt), **with ripgrep 14.1.0 on PATH instead of the workflow's 14.1.1** |

**Not discharged (21 of 31 steps):** ripgrep 14.1.1 provisioning; the dev and release webview harnesses; Node 24.21.0;
the Builder toolchain assembly, entry tests, package inspection and packaged-Builder verification; the cargo-audit and
cargo-deny security gate; the whole frontend job (Node 22, npm advisory gate and audit, `npm ci`, tsc, vitest, vite);
the whole Python job (Python 3.11, hash-pinned pip install, pytest). Causes: no dispatch or network is authorised;
Node 24.21.0/22, Python 3.11 and ripgrep 14.1.1 are absent; the native lanes need about a whole-workspace test build of
disk (about 32 GB) above the 16 GiB floor, where about 6.5 GiB remains.

Contract 3.1 (line 125) requires "the Fast Local gate on the exact commit, started by dispatch"; it is not superseded.
The Owner decision required: `04-FAST-LOCAL-VERDICT.md`. Details: `04-*`.

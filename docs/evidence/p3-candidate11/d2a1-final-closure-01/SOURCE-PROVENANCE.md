# Source provenance

**Is the source exactly Candidate C? Yes.**

| Item | Identity | Evidence |
|---|---|---|
| Candidate C | `d5964ee5326c2733177307db6f507328023d361a`, tree `51bac5fd335999cc995c0ab9f582a30244973ef0`, parent `71264aa81a40b86189783facc8437e0427887c16` | Git objects (`00-PROVENANCE.md`) |
| Checked source set | 5,364 tracked entries, 97,246,341 bytes, every byte compared with C's blobs (`verify_tree.py`) | every receipt; this mission's worktree before and after its run |
| Ledger sources | `protocol.rs` `a2df2841…`, `model_tests.rs` `28085ce2…`, `unit_tests.rs` `55dc6640…` | `00-SOURCE-IDENTITIES.json` |
| Build inputs | `Cargo.lock` `1af53a60` (sha256 `6ed84a9a…d979`); `rust-toolchain.toml` `726005b2` (1.94.0); `.cargo/config.toml` (jobs 8; `CARGO_PROFILE_TEST_DEBUG=1`) | `00-SOURCE-IDENTITIES.json` |
| Pinned executable E-C | sha256 `cde9f9d20f32f2b8a17b7dcc2f521462ab6c7a6292d5e2cbd1dd2ba2c3d0d30c`, 8,984,464 bytes, 49 Rust inputs (all C's blobs) | hashed now, not run |
| Evidence chain after C | 24 single-parent commits to `cca0d773`, none a merge, none touching a path outside `docs/evidence/` | `raw/00-git-auth-3.txt` |
| Git evidence packets relied on | 19 of 19 verify against their own sealed lists, file by file | `receipts/25-PACKET-CUSTODY.txt` |
| Prior local roots | VALIDATION-02 177/177; CLOSURE-EVIDENCE-AUDIT-01 13/13; CLOSURE-PACKET-01 48/48 | `raw/00-prior-manifest-verify.txt` |

No evidence commit is used as a source commit. No source, test, `Cargo.toml` or `Cargo.lock` was edited; no
dependency was resolved, installed or upgraded.

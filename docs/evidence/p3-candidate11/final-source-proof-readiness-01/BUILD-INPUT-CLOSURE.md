# Build-input closure: the proof lineage to Candidate C

Mission P3-C11-DR01-D2A1-FINAL-SOURCE-PROOF-READINESS-01, section 7; basis AD-1-R4 N.5–N.7. Executor: Claude Code,
Claude Opus 5.5. Nothing was built. The raw record is `receipts/12-build-inputs.json`, written by
`tools/build_inputs.py`.

**Result: no build input changed.** Every input named by the mission and by AD-1-R4 N.4–N.6 has one blob across
SOURCE-R2, ADM, GC, EXT, W1, W4, B, R4 and C. No changed input exists to classify, so no row is
SEMANTICALLY_RELEVANT, SEMANTICALLY_IRRELEVANT_WITH_REASON or ARCHITECT_REVIEW_REQUIRED. The executor approves no
input; it reports identity.

## 1. Identities

| Input | Blob (all nine commits) | sha256 at C | AD-1-R4 N (at 42dd1bd1) | Classification |
|---|---|---|---|---|
| `Cargo.lock` | `1af53a60aaf6fde5731e63fcd5b6cd11d86f669a` | `6ed84a9aeba4a542b1820c53d4ba0f4899cdfa819380aaa8649c3b5d31add979` | N.6 `1af53a60` | IDENTICAL |
| `rust-toolchain.toml` | `726005b29561a04536edea412bf551a9884d6639` | `fc7f9811196c9d9db1864dd140fa4b7632cda9b7761ed8736256b463c835d298` | N.5 `726005b2` | IDENTICAL |
| workspace `Cargo.toml` | `2c558a4477d1545daaa489f0a0e502b4284ad479` | `7f09c3dbe16a37bba215eeaa5f028614f3f6eafdf3419a5b79472f6b7cebf795` | N.6 `2c558a44` | IDENTICAL |
| `crates/nexus-governed-control/Cargo.toml` | `2543c745d661203326367962714b94c811e07980` | `30b2bb495434e85fc5b1e1476949711d55e8bb55cb9e0e6783bbc71ce0a3564f` | N.6 `2543c745` | IDENTICAL |
| `.cargo/config.toml` | `05a42b50174e235b183b220afeb58922c96d17a1` | `379a150667985722540bd30f737cfb363dc5bc2dc8a5edae3db3725e3cd0b234` | N.6 `05a42b50` | IDENTICAL |
| `src/lib.rs` (crate root) | `b806ed577f8d3472b7187d161246e08d2f1ccd3d` | `addad1993f57ad370da7e38719f3e58d26b5a4eb1fbe06b24df51525b135727d` | N.4 `b806ed57` | IDENTICAL |
| `src/ledger.rs` | `d872b90816fdd18cca8e5d1a0005bb49434aaf2a` | `d37983f6b040f67fd012f1bc4d040523a254d5d0557a39edfa40dd9dffe99215` | N.4 `d872b908` | IDENTICAL |
| `src/ledger/protocol.rs` | `a2df2841b99cad5d452008a6d4150e9369d07e66` | `a5454944368df70fa4ce1c7cc4191d283e0a54b371137338fb999e59960aa296` | N.1 `a2df2841` | IDENTICAL |
| `src/ledger/protocol/unit_tests.rs` | `55dc66409eaccdaa93e8f0a56f2ccb14be81e114` | `841220b1095f339568e2764c41be0792a83c7fb097df6cdea8b50e46dc261159` | N.4 `55dc6640` | IDENTICAL |

The whole compiled input set lies outside `docs/evidence/`, and there the only path that differs between any of
SOURCE-R2, ADM, GC, EXT, W1 and C is `model_tests.rs` (SOURCE-CLOSURE.md, WITNESS-NONINTERFERENCE.md). Between
W4, B, R4 and C nothing differs outside `docs/evidence/`. That covers every workspace member's manifest (on which
`Cargo.lock` resolution depends) and every path dependency.

## 2. Parsed facts at C

**Toolchain.** `channel = "1.94.0"`, components `rustfmt`, `clippy`, target `wasm32-wasip1`.

**Profiles (workspace `Cargo.toml`).** `[profile.dev]` debug 1, opt-level 0. `[profile.release]` opt-level 3,
lto `"thin"`, codegen-units 16, strip `"symbols"`. No profile sets `panic`. No `[profile.test]` or
`[profile.bench]` table. The crate manifest has no profile table. `[workspace.lints.rust] unsafe_code = "forbid"`
exists; the crate has no `[lints]` table, so it does not inherit it.

**`.cargo/config.toml`.** Exactly `[build] jobs = 8` and `[env] CARGO_PROFILE_TEST_DEBUG = "1"`. No
`rustflags`, no target table, no panic setting.

**Features.** The crate declares no `[features]` and no build script. Its dependency feature declarations:
`serde` (`derive`), `reqwest` (no defaults; `rustls-tls`), `tokio` (`rt`, `time`, `macros`), and on Linux `libc`
and `x11rb` (`xtest`); the others with defaults (`sha2`, `hex`, `getrandom`, `serde_json`, `url`, `zeroize`,
`base64`, `png`, `unicode-segmentation`); `nexus-kernel` by path `../../kernel`; dev-dependency `serde_json`.

**Crate attributes and the path to the model.**

| File | Attribute or declaration | Effect in a test build |
|---|---|---|
| `lib.rs` | `#![deny(unsafe_code)]` (its only inner attribute) | a lint level |
| `lib.rs` | `pub(crate) mod ledger;` | — |
| `ledger.rs` | `#[cfg_attr(not(test), allow(dead_code))] mod protocol;` | inactive (lint level, non-test only) |
| `protocol.rs` line 15 | `#![cfg_attr(not(test), deny(clippy::indexing_slicing, clippy::arithmetic_side_effects, clippy::unwrap_used, clippy::expect_used, clippy::panic))]` | inactive (lint level, non-test only) |
| `protocol.rs` lines 30, 32 | `#[cfg(test)] mod model_tests;` `#[cfg(test)] mod unit_tests;` | the two test modules are compiled |
| `protocol.rs` lines 191, 209, 214 | `#[cfg(test)] hook::report(…)` in `load` and `exchange` | each load and exchange is reported to the hook |
| `protocol.rs` line 2017 | `#[cfg(test)] mod hook` | the hook exists |
| `model_tests.rs` | no inner attribute, no `cfg`, no `cfg_attr` | — |

Lint levels change no generated code.

**Panic strategy.** No manifest, profile, config or `[env]` entry sets one, so it is the default, unwind (AD-1-R4
N.6), on which the explorers' `catch_unwind` depends. Cargo documents that test targets ignore a profile `panic`
setting; that is cited, not verified here.

**Path-dependency closure and build scripts.** `crates/nexus-governed-control` → `kernel` → `crates/nexus-crypto`,
`persistence`. The one build script in the closure is `kernel/build.rs` (sha256
`5dd672bf7812edfc0fe41be7f6002f973ae87adec0eebfb92ae1f17d33e88da2`): it reruns only on
`src/resource_limiter/darwin_proc.c`, returns at once unless the target OS is macOS, and mentions no `docs` path.
The closure has six `include_str!` and no `include!` or `include_bytes!`; every one resolves inside its own crate
(`authority/run.rs`, `authority/tests.rs`, `kernel/src/actuators/web.rs`, and three `resource_limiter` files).
Nothing under `docs/` is compiled into the test executable, so the evidence-only commits B, R4, C and D cannot
change it.

## 3. What remains an execution input (pending)

Not static, and not established here (matrix rows N.5b, N.7, N.14):

- the compiler that actually builds the executable (`rustc --version`);
- the build command and its environment: `RUSTFLAGS`, `CARGO_ENCODED_RUSTFLAGS`, `CARGO_PROFILE_*`,
  `CARGO_BUILD_*`, `RUSTC_WRAPPER`, target settings, and the effective profile, features and `cfg` as the build
  reports them;
- the pinned executable's sha256 before and after each run.

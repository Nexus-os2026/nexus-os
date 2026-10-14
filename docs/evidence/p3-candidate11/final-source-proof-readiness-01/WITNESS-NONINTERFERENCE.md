# Witness non-interference: the C1/C25 and C4 additions

Mission P3-C11-DR01-D2A1-FINAL-SOURCE-PROOF-READINESS-01, section 6. Executor: Claude Code, Claude Opus 5.5.
Strict static review. No witness was invoked; nothing was built or run. The raw record is
`receipts/11-witness-check.json`, written by `tools/witness_check.py`; layer isolation is check 8 of
`receipts/10-source-closure.json`.

**These verdicts are the executor's static findings. AD-1-R4 N ("The rule") leaves the ruling on whether an
addition alters the audited code's meaning to the Architect; nothing here is that ruling.**

## The two additions

| | C1/C25 | C4 |
|---|---|---|
| Commit | W1 `42dd1bd1cf21ffdea5968b256d2983f6575fab5b` | A `04e42a7ddf0d0a2af17b6506c51997006e7a08dc` |
| Parent | EXT `2d83b74b…` (model_tests.rs blob `4895993e…`) | W1 (model_tests.rs blob `5ad26ddc…`) |
| model_tests.rs after | blob `5ad26ddc727d8be30f4fb8d7478bc69e1c974933` | blob `28085ce232827301104b5d757af042ec933976f8` |
| Bytes at C | [275579, 286411), lines 8245–8521 | [286411, 310612), lines 8522–9199 |
| sha256 of the appended bytes | `64f7c29c12ca3106012e6c2045680a52edab0a69096710f1c6c18e52e8c4d07a` | `8069f96e6cdfc243e1a67a1e14deeb92d6ceac238844c03ed8c57d1bb47ab2ac` |
| Items defined | 10: `type Scheduled`, `struct Witness`, `struct Replayed`, `fn violations_at`, `fn replay_schedule`, `fn run_witness`, `const C1_WITNESS`, `const C25_WITNESS`, `fn c1_witness_p1_at_a_successful_admission`, `fn c25_witness_p7_on_the_resulting_world` | 30: `struct C4Step`, `C4Ending`, `C4Scenario`, `C4After`, `C4Replayed`, `C4Run`; `fn c4_after`, `c4_x_explains_f1`, `c4_p4_kept`, `c4_p4_lost`, `c4_p8_kept`, `c4_p8_lost`, `c4_replay_step`, `c4_choices`, `c4_frozen`, `c4_consumption`, `c4_replay`, `run_c4_witness`; `const C4_ARM`, `C4_CONFLICT`, `C4_WK_QUIET`, `C4_APPEND_X`, `C4_SNAPSHOT`, `C4_CONFIRM`, `C4_EMPTY_X`, `C4_GATING`, `C4_P4`, `C4_P8`, `C4_SCENARIOS`; `fn c4_witness_p4_and_p8_on_f8` |
| Tests added | 2 `#[test]`, not ignored | 1 `#[test]`, not ignored |
| **Verdict** | **NON_INTERFERING** | **NON_INTERFERING** |

From A to C, `model_tests.rs` is the same blob, and nothing outside `docs/evidence/` changes: C carries the C4
witness exactly as A has it.

## The mission's questions, each with its check

Each check runs on both additions. Every one passed on both (`checks` in the receipt).

| # | Mission question | Check | Finding, both additions |
|---|---|---|---|
| 1 | occurs after the audited proof/model implementation | N01 | A pure byte append at the end of the file, after the closing `}\n` of its parent's last item. Lexically isolated: lexing the new file gives exactly the earlier tokens over the earlier bytes. The C1/C25 layer starts after EXT, the last audited text; the C4 layer starts after it. |
| 2 | changes an existing item | N02 | No. Every earlier item (431 before C1/C25, 441 before C4) has the same bytes and the same offsets. |
| 3 | adds an impl for an existing type or trait | N03 | No `impl` and no `trait` token anywhere in either addition, at any depth (a trait impl inside a function body would still be global; none exists). The only `fn` tokens that do not head an item are the two function-pointer field types `kept: fn(&C4After) -> bool` and `lost: fn(&C4After) -> bool` of `C4Scenario`. No `#[derive]`. |
| 4 | adds/imports a name capable of changing earlier name resolution | N04, N05 | No `use`. The module begins `use super::*;`, so a new module-level name could shadow a glob-imported one, but only for code that spells it. No name either addition defines occurs anywhere in the text before it; none collides with an earlier item, with any item of `protocol.rs`, or with a prelude name or a name the file imports. |
| 5 | defines a macro used by earlier code | N06 | No `macro_rules!`, `#[macro_export]` or proc-macro attribute. |
| 6 | introduces cfg/cfg_attr affecting previous items | N07 | The only attributes are outer `#[test]` (2 and 1). No inner attribute (`#![…]`), no `cfg`, no `cfg_attr`. An outer attribute attaches only to the item after it. |
| 7 | introduces global/thread-local/static state used by the explorers | N08 | No `static`, `thread_local`, lazy/once cells, atomics, locks, `Cell`/`RefCell`/`Rc`/`Arc`, `unsafe`, `env::set_var`, panic-hook calls, process, file or thread APIs (67 identifiers scanned; 0 hits). Neither takes the file's one static, `SERIAL` (a `Mutex<()>` defined in the model text, line 4113). `'static` is a lifetime token and is not counted. |
| 8 | alters the existing test hook | N09 | `protocol.rs`, which holds `mod hook`, is the same blob (`a2df2841…`) before and after each addition. Each addition calls only `hook::take_counts()`, twice, around its own replayed step, as `successors` does. No `hook::install`, `hook::remove` or `hook::report`. |
| 9 | changes configuration constants | N10 | `struct Config`, `default_run`, `BASE`, the cap and time constants, `TICKET_LIMIT`, `REFUSAL_LIMIT`, `SYMMETRIC` and every `Config` constant are unchanged. Neither addition defines a constant of type `Config`. The witnesses read `F1` and `F8` by value (`config: F1` in `C1_WITNESS` and `C25_WITNESS`; `run_c4_witness(&F8, …)`). |
| 10 | changes PROPERTIES or BRANCHES | N11 | Both unchanged, and neither addition mentions either. |
| 11 | changes an existing test | N12 | Every earlier `#[test]` (100, then 102) is unchanged and keeps its order. The `#[ignore]` inventory is 34 before and after each addition, with the same names. |
| 12 | is referenced by original/quotient explorer code | N13, N04 | No item of either addition is in the name-dependency closure (computed at C, 340 items) of `explore`, `explore_experimental`, `explore_symmetric`, `explore_symmetric_kept`, every model root, every `step_*`, and every test of the earlier text. No name either defines is spelled before it. |
| 13 | changes compilation features or crate attributes | N14 | `lib.rs`, `ledger.rs`, `protocol.rs`, `unit_tests.rs`, both `Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml` and `.cargo/config.toml` are the same blobs before and after each addition; outside `docs/evidence/` only `model_tests.rs` changes. No `include!`, `env!`, `#[path]`, `#[link]`, `#[no_mangle]`, `extern` or `crate` construct. |

## What the witnesses do with existing code (read-only uses)

The witnesses call existing functions and read existing types: `step`, `decode`, `encode`, `initial`, `actors`,
`state_properties`, `transition_properties`, `word_of`, `field`, `gating_key`, `fact_key`, `bit`, `Oracle::new`,
`Obs`, `SimRow`, `Occupant`, `Phase`, `Stage`, `LatchCause`, `World`, `LIVE_SHIFT`, `UNAPPLIED_SHIFT`. They read
the `Oracle` fields `taken`, `faults` and `fault`, all present since SOURCE-R2. Every world and oracle they write
is one they created: each world is decoded afresh from a key, with its own shared allocation, and is dropped at the
end of the step. The C4 layer also uses `violations_at`, defined in the C1/C25 layer.

## Runtime separation (static reasoning, not a run)

- The hook's counts and callback are `thread_local!`. A witness test takes the counts on its own test thread; an
  explorer takes them on its own. A witness cannot contaminate an explorer's counted window (ADM-PROOF F8, AQ3;
  GC-PROOF AS2).
- The witness tests are ordinary tests: they run in the default suite, possibly in parallel with others, and do not
  take `SERIAL`. They share no state with any other test, so this does not matter. AD-1-R4 N.11 selects exactly
  one test with `--exact` for each required quotient run, so no witness runs in that process.
- A witness panics only to report a violation (under a mutant); that is its own test's failure.
- The test executable at C differs from the ones built at the proof commits, because it contains more code. Its
  identity at the exact final commit is an execution matter (N.5b, N.7, N.14), pending.

## Limits of this review

It is token- and byte-level static analysis with an executor-written lexer (`tools/rust_items.py`), cross-checked
against a column-0 item count. It does not type-check or compile. Within that, a Rust item cannot change earlier
code's meaning except by a trait or inherent impl, a module-level name, a `use`, a macro, an inner or `cfg`
attribute, global state, or a change to compiled inputs, and each of those is checked absent above. The ruling is
the Architect's.

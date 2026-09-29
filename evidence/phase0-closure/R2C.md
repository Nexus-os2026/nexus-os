# P0-LINUX-FINAL-R2C: raw-string lexer in the Wasmtime exception guard

This note is evidence and claims for Architect review. It is not approval,
and it is not Phase Zero or Final Gate completion. Windows and macOS stay
deferred.

## Commit

| | |
|---|---|
| Start | `f4ef200f6f2c1f5f4af9d635cbd3c99eacaba0e4` (tree `b67badf9`, parent `9126be75`) |
| Commit | `cae7bb5288820589b5cddf255e1bfc9f5e49c88a` "test(security): handle Rust raw strings in Wasmtime guard" |
| Tree | `fb2170fa74f009f4411933b8cdc713e3743a1598` |
| Parent | `f4ef200f6f2c1f5f4af9d635cbd3c99eacaba0e4` |

The diff touches one file: `app/src-tauri/src/phase0_surface/fg_reliability/tests.rs`,
with 164 insertions and 45 deletions (`git diff --name-only f4ef200f
cae7bb52`).

- All 45 deleted lines are the old `without_comments` body.
- No probe line was removed.
- No test function was added, removed or ignored.

## What defeated the R2B lexer

Both shapes below were tested against the R2B build of the guard, before any
change.

| Shape appended to `sdk/src/wasm_agent.rs` | R2B guard |
|---|---|
| The mission's example: `r#"x" // still raw"#;` then a cfg-disabled `use wasmtime::r#component::Val;` | exit 101, caught. The `//` comment ends at the newline, so the next line is still scanned. |
| Quote-parity desync: `r#"x" "#;`, `"/*";`, a cfg-disabled `use wasmtime::r#component::Val;`, then `"*/";` | **exit 0: the forbidden use was hidden** |

The mechanism is quote parity. R2B's `without_comments` treated every `"` as
a plain string boundary. An odd number of unescaped quotes in a raw literal
put it out of step, so the real string `"/*"` looked like a comment opener,
and the real code up to `"*/"` was removed.

A `'"'` character literal has the same effect. It is valid Rust, so it is
covered too. Every shape used compiles with `rustc --edition 2021`.

## The lexer now

`without_comments` is a small lexer for this guard, not a Rust parser. It
removes only real comments and keeps every literal verbatim. Literal
contents therefore stay in the scanned text, so a forbidden spelling inside
a string is a fail-closed false positive, never a miss.

- **Raw strings.** `r"…"` and `r#"…"#` with any number of hashes (including
  zero), plus `br…` and `cr…`. There are no escapes; a raw string ends only
  at `"` followed by the same number of `#` (`raw_literal_len`). It is
  recognised only at an identifier boundary, so the raw identifier
  `r#component` (no quote after the hashes) is not a literal.
- **Escaped strings.** `"…"`, `b"…"` and `c"…"`, with `\` escapes
  (`quoted_literal_len`).
- **Character literals.** `'x'`, `'"'`, `b'"'` and escapes such as `'\''`
  and `'\u{22}'` (`char_literal_len`). Lifetimes and labels such as `'a` are
  not literals.
- **Comments.** Line comments (doc comments included) are removed to the end
  of the line. Nested block comments are removed and replaced with a space.
- **Unterminated literals** run to the end of the file.

All R2, R2A and R2B rules are unchanged:

- component paths, including `r#component`, groups and aliases inside groups;
- crate aliases and globs;
- Cargo-metadata dependency renames;
- core use accepted;
- `cfg`-disabled code in scope;
- no parser or dependency added.

## New probes

These were added to the existing guard. Every R2, R2A and R2B probe is kept.

**Must catch, as the component module (18).** Each probe puts a literal
before a later cfg-disabled `use wasmtime::r#component::Val;`:

- `r#"x" // still raw"#` (the mission's shape);
- odd-quote raw strings followed by the `"/*"` … `"*/"` desync tail, with
  `#`, `##` and `###` (the last containing `"## " //`);
- `br#"x" "#` and `br##"x" // "##`;
- `cr#"x" "#` and `cr##"x" /* "##`;
- the zero-hash form `r"x\"`;
- `r#"x" /* "#`;
- `r#"/* a " b */"#`;
- embedded quotes: `r#"he said "hi" and ""#`;
- escaped strings `"a \" b"`, `b"a \" b"` and `c"a \" b"`;
- the character literals `'"'`, `b'"'` and `'\''`.

**Direct lexer checks (7).** These confirm that literals are kept whole and
only real comments are removed:

- `r#"a" // b"#`
- `br##"a"# /* b"##`
- `cr"a /* b"`
- `"a \" // b" x // c`, where the literal is kept and `// c` is removed
- `'"' x /* c */ y`
- `fn f<'a>(x: &'a str) {} // c`
- `r#component /* c */ x`

**Must accept, as core use (6 new, 16 in all):**

- `r#"see // here"#` followed by `use wasmtime::Engine;`
- `r##"a "# // b"##` followed by `use wasmtime::Linker;`
- `r#"/* not a comment */"#` followed by `use wasmtime::Store;`
- `br#"/* " */"#` followed by `use wasmtime::Module;`
- `"say \"hi\" // not a comment"` followed by `use wasmtime::Module;`
- `'"'` with a lifetime, followed by `use wasmtime::Engine;`

The existing `wasmtime::r#componentx` boundary case is kept.

## NC8

The mutation is the desync shape shown above to defeat R2B, appended to
`sdk/src/wasm_agent.rs`:

```rust
const P0_R2C_NC8_RAW: &str = r#"x" "#;
const P0_R2C_NC8_OPEN: &str = "/*";
#[cfg(any())]
use wasmtime::r#component::Val;
const P0_R2C_NC8_CLOSE: &str = "*/";
```

The corrected guard binary was run directly against it.

- **Result:** exit 101, `test result: FAILED`, with
  `["component: wasmtime::component"] (Wasmtime's component module is not
  used while RUSTSEC-2026-0316 is accepted)`.
  - This is the component-module invariant, not a compile or infrastructure
    error. The shape is valid Rust (rustc, edition 2021).
- **Restoration:**
  - SHA-256 before the mutation: `1ed55dda…a07045`, equal to
    `git show HEAD:sdk/src/wasm_agent.rs`;
  - with the mutation: `5d8c94e2…ecdbcf`;
  - after restoration: `1ed55dda…a07045`, and `cmp` against Git matches.
- **After restoration:**
  - a clean guard run exits 0;
  - `Cargo.lock`'s SHA-256 is unchanged;
  - `git status` shows only the test file.

The earlier Wasmtime controls were rerun against the new build. Each failed
for its intended reason and was restored byte for byte:

| Control | Rule |
|---|---|
| NC1 | component module (group) |
| NC2 | component module (group) |
| NC5 | `alias: wasmtime as wt` |
| NC7 | component module (direct path) |

## Local validation

- `git diff --check` and `cargo fmt --all -- --check`: clean.
- `cargo clippy -p nexus-desktop-backend --all-targets --all-features --locked -- -D warnings`:
  clean (the R2B form).
- The Wasmtime guard and the final trust-surface guard pass.
- `cargo test --workspace --locked -- p0_fg p0_r1 p0_002c5c`: 297 passed, 0
  failed, 0 ignored.
  - This is 231 `p0_fg_*`, 11 `p0_fg1_*`, 8 `p0_r1_*` and 47 `p0_002c5c_*`.
  - It is unchanged from R2B.

## Fast-local run

Run `36613947940`, #13, attempt 1:

- event `push`; tested commit `cae7bb5288820589b5cddf255e1bfc9f5e49c88a`,
  confirmed by the verify step in all three jobs; conclusion **success**.
- **fast-linux** (success; steps 8–17 all successful):
  - fmt and clippy;
  - `cargo test --workspace --locked` (unfiltered): 7684 passed, 0 failed,
    43 ignored, 0 `FAILED` lines;
  - `p0_fg_dep_wasmtime_uses_no_dynamic_component_val_api` and
    `p0_002c5c_final_trust_surface_guard_is_complete` both `ok`;
  - 231 `p0_fg_*`, 11 `p0_fg1_*`, 8 `p0_r1_*` and 47 `p0_002c5c_*`, all
    passed;
  - the live webview harness, dev and release (`ok` twice);
  - the trusted-entry JS tests (21 of 21) and the packaged Builder gate
    (13 passed);
  - the security gate, unchanged: cargo-audit 0.22.1 and cargo-deny 0.19.6,
    `advisories ok, bans ok, licenses ok, sources ok`,
    `security-audit: passed`.
- **fast-frontend** (success): vitest 461 of 461 (103 files); tsc clean.
- **fast-python** (success): voice tests 27, OK.

## Refs (github)

| Ref | SHA |
|---|---|
| `repair/p0-linux-final-closure-r2` | `cae7bb5288820589b5cddf255e1bfc9f5e49c88a` |
| `rebuild/phase0-trust-boundary` (authoritative) | `71c47acbf3f8ee8210109587b3229f8d89067b6b` (unchanged) |
| `main` | `80640bba41e74c17abbf4eb71eafa88bd1ade8db` (unchanged) |

- No hosted CI was dispatched.
- No PR was opened, and nothing was merged or integrated.
- Nothing was amended, rebased, squashed or force-pushed.
- The phase was not advanced.

# Conventions shared by the two checkers (stated once, implemented twice)

Checker A (`a_items.py`, `a_source.py`, `checker_a.py`) and Checker B (`checker_b.py`) implement these
conventions independently. They share no code. Both read `occurrence_spec.py`, the frozen classification table they
each verify, and `agree.py` compares their outputs.

## Inputs

The 49 `.rs` files under `crates/nexus-governed-control/src/` at Candidate C
(`d5964ee5326c2733177307db6f507328023d361a`), read from git objects. These are exactly the rustc dep-info inputs of
the pinned executable (Stage-1 `receipts/11-pin/DEP-INFO.txt`). `protocol.rs`, `model_tests.rs` and `unit_tests.rs`
must be the blobs `a2df2841…`, `28085ce2…` and `55dc6640…`, or the checker stops.

## Regions

Every character is in one region: **comment** (`//` to the end of the line; `/* … */`, nested), **string** (a string,
byte string, raw string or character literal, including its quotes and prefix), or **code** (everything else). A
lifetime (`'a`, `'static`) is code.

## Items and their names

An item is a `fn`, `struct`, `enum`, `union`, `trait`, `impl` block, `mod` with a body, `const`, `static`, `type` or
`macro_rules!`. Its span runs from its first contiguous outer attribute (`#[…]`, not `#![…]`) or outer doc comment
(`///`, `/**`) to its closing `}` or `;`. Its hash is the sha256 of exactly those bytes.

| Item | Name |
|---|---|
| free fn | `NAME` |
| fn in `impl Type` | `Type::NAME` |
| fn in `impl Trait for Type` | `<Type as Trait>::NAME` |
| fn in `trait T` | `trait T::NAME` |
| fn nested in fn `OUTER` | `OUTER/NAME` |
| struct, enum, union, trait, const, static, type, macro | `struct NAME`, `enum NAME`, …, `macro_rules! NAME` |
| anything inside an inline module `M` | prefixed `M::` |
| no enclosing item | `<module>` |

Generic parameters, lifetimes and references are dropped from type names. The enclosing item of an occurrence is the
innermost item whose span contains it.

## Target occurrences

1. a **code** identifier exactly `replays`, `Replay`, `step_replay` or `last` (group *exact*);
2. a **code** identifier `name` immediately preceded (ignoring whitespace) by `.` (group *name*, token `.name`);
3. a **code** path `Action :: V` with V one of Nothing, Barrier, Resolve, AppendC, AppendX, EmptyX, recorded at the
   `Action` identifier as the token `Action::V` (group *action*);
4. any other maximal run of `[A-Za-z0-9_]` containing `replay` in any letter case, in code (an identifier), in a
   comment or in a string (group *lexical*).

The ordinal of an occurrence is its rank among the occurrences with the same file, enclosing item, region and token.

## Call closure

From `explore` in `model_tests.rs`, within the three ledger files only (`protocol.rs` imports only `std`; checked),
following fns that have a body, by name: an identifier followed by `(` or `::<`, or a path segment after `::`, links to
every fn whose last name segment equals it. This over-approximates the calls.

## Citations

`[FILE · ITEM · Lstart–end · blob B8 · item H16]`: B8 is the first 8 hex digits of the file's blob at C, H16 the first
16 hex digits of the item's hash; the lines must lie inside the item. `tools/render.py` makes them from
`{{FILE|ITEM|start-end}}` macros; both checkers resolve them independently.

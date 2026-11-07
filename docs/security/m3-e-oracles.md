# M3-E host oracle register (consolidated)

One register for the M3-E host units (ruling M3E-R1: seven units, one
register). Each unit appends its own section in its own oracle commit; a
section is never edited after its results are seen without an explicit
oracle-amend ruling.

Sources: `/home/nexus/NEXUS/m3e-mission-draft.md` (U1, D4, D11–D13,
rulings M3E-R1 and M3E-R2), `/home/nexus/NEXUS/m3-design-approved.md`
(§1.1 steps 0–1, §6.1, §9.5).

## M-1 — input snapshots and running-image identity

Code: `mother/host/src/inputs.rs`, `mother/host/src/binary_identity.rs`.
Tests: `mother/host/tests/page_inputs.rs` (and the in-module unit test in
`binary_identity.rs`). Linux only.

### Frozen behaviour

- **Snapshot.** Each input is opened once, read-only,
  `O_NOFOLLOW | O_NONBLOCK | O_CLOEXEC`, then `fstat` on that descriptor must
  report a regular file. It is read from the same descriptor to at most
  cap + 1 bytes; more than cap is `input_too_large` (metadata length is never
  trusted). The SHA-256 is over the complete raw bytes; the parse reads the
  same retained buffer. There is no reread by path, now or at consent.
  `O_NOFOLLOW` covers the final component only.
- **Caps.** Facts 2,048 B; notes 1,792 B; goal 16,384 B.
- **Codes.** `input_unavailable` (missing/unopenable), `input_link` (final
  component a symlink, dangling or not), `input_not_regular` (directory, FIFO,
  device, socket; a FIFO is refused without waiting), `input_too_large`,
  `input_unreadable` (read error), `input_not_utf8`. No input text and no OS
  error text appear in any error or `Debug`.
- **Facts (D4, M3E-R2 4).** UTF-8; one optional final LF dropped from the
  parse only. Lines split on LF; each must be `key: value` split at the
  first `: `. Order: exactly one `name`, then `hours` 1–7, then exactly one
  `phone`, then nothing. Any CR, blank, unknown, duplicate, out-of-order or
  8th hours line is `facts_file_invalid` with that 1-based line; an empty file
  is line 1; a file that ends before `phone` is the line after its last. Then
  the word cap: a run between U+0020 longer than `POLICY_V1.max_word_chars`
  (40) scalars is `facts_word_too_long` with its line. Then
  `ApprovedFacts::new`, whose code is reported with no line. Values are never
  trimmed or normalized. The result is the existing
  `nexus_mother::page_inputs::PageFacts`.
- **Notes (M3E-R2 2).** UTF-8; text = file less one optional final LF; label
  `notes`; the existing `nexus_mother::contract::Note`. No other check here:
  the existing contract rules (single note, plain lines, 40 lines, credential
  screen, 2,432 B rendered cap) decide.
- **Goal (D3, M3E-R2 3).** UTF-8; one optional final LF dropped. Empty or
  whitespace-only → `goal_empty`; any scalar other than LF that fails
  `shown` (CR, controls, bidi, zero-width, non-ASCII spaces) →
  `goal_not_plain`; more than 4,096 scalars in the stored text (LFs counted)
  → `goal_too_long`. Otherwise stored verbatim.
- **Goal precedence (recorded for M-3).** A goal file together with a typed
  goal is refused; there is no precedence. Typed mode does not exist in M-1.
- **Page flags (M3E-R2 1(a)).** `parse_page_args` accepts exactly
  `--mission one-page-site --notes <abs> --facts <abs> [--goal <abs>]`.
  Codes: `invalid_arguments` (unknown or duplicate), `missing_value`,
  `mission_required`, `mission_unknown`, `notes_required`,
  `facts_required`, `absolute_path_required` (relative or empty). No default
  location. Parsed only: nothing in `hands.rs` calls it, and no mission or
  model is enabled.
- **Running image (M3E-R2 5).** SHA-256 streamed from `/proc/self/exe` (the
  executed file, even after its pathname is replaced), never argv[0] or a
  pathname. Not a regular file, over 1 GiB, read error, or a stream length
  different from `fstat` → `self_identity_unavailable`; no hash is invented.
  Printing at start-up is M-3/M-5 wiring.

### Oracles

| Oracle | Required assertions |
|---|---|
| `m3e_u1_snapshot_and_hash_same_bytes` (M3-O43) | F-BAKERY digest equals the §9.5 hash and the parse; for facts, notes and goal, pathname replaced after open → digest and parse are of the opened file; file changed after read → snapshot unchanged; `read_page_inputs` digests equal the raw bytes of each parsed file; a facts error is reported with its kind. |
| `m3e_u1_input_bounds_and_nofollow` | For each kind: cap accepted, cap + 1 and 4 MiB refused; symlink and dangling symlink `input_link`; directory and `/dev/zero` `input_not_regular`; FIFO refused within a bound (no blocking); missing `input_unavailable`; a file grown past the cap after open refused; size-0 `/proc` file read from the descriptor; `/proc/self/mem` read error `input_unreadable`; invalid UTF-8 `input_not_utf8`; canaries absent from `Debug` and errors. |
| `m3e_u1_facts_grammar_and_manifest` (M3-O39 ext.) | Every grammar, CR, order, duplicate, blank, unknown, 8th-hours, word-cap (bytes vs scalars) and precedence case gets its exact code and line; `ApprovedFacts` codes carry no line. All §9.5 notes and facts fixtures (NOTES, INJECT, STRONG-W, F-BAKERY, F-MIN, NOTES-MIN, MAX-A/L/C), written as files, match their bytes and hashes and pass the real parser and contract validation (2,432 B); 2,433 B, a 257-character line, a 41st line and a second note refuse; CRLF notes are not normalized. |
| `m3e_u1_goal_file_verbatim_and_bounds` | Verbatim with allowed whitespace; exactly one final LF dropped; 4,096 multibyte scalars accepted, 4,097 refused; LFs counted; 16,384 B accepted, 16,385 B refused; CR/CRLF, bidi, tab, zero-width, U+3000, BEL refused; empty and whitespace-only `goal_empty`; `approve` is data; goal symlink refused. |
| `m3e_u1_page_args_parse_only` | Exact flags accepted, goal optional; duplicate, unknown (`--home`), missing value, missing or other mission, missing notes/facts, relative or empty path refused with their codes. |
| `m3e_u1_running_binary_identity` | Digest equals an independent hash of this test image; a running copy whose pathname is replaced and whose argv[0] names the replacement reports the original image's digest (bounded child, killed and reaped). |
| `m3e_u1_running_binary_identity_unavailable` (unit) | Real image digest first; then a missing path, a directory and a size-0 `/proc` file each give `self_identity_unavailable`. |

### Plants (each temporary, killed at its named assertion, never committed)

| Plant | Mutation | Killing oracle |
|---|---|---|
| P-U1a | drop `O_NOFOLLOW` | bounds: `{k}: link` |
| P-U1b | hash a reopened pathname | snapshot: `digest of the opened file` |
| P-U1c | `rendered_size` omits separators (`mother/src/page_inputs.rs`) | manifest: 2,433 B refuses |
| P-U1d | hash argv[0] | identity: `running image, not the replaced pathname` |
| P-G1 | accept more than 4,096 goal scalars | goal: 4,097 refused |
| P-G2 | follow a symlink for the goal only | goal: goal symlink refused |
| P-G3 | normalize CRLF in the goal | goal: `goal\r\n` refused |

### Stop conditions

A change needed to `ApprovedFacts` policy, credential rules or the §9.5
manifest; running-image identity unavailable on the test platform; a new
dependency version or `Cargo.lock` change beyond the host package's own
dependency list.

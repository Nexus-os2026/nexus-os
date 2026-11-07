# M3-E host oracle register (consolidated)

One register for the M3-E host units (ruling M3E-R1: seven units, one
register). Each unit appends its own section in its own oracle commit; a
section is never edited after its results are seen without an explicit
oracle-amend ruling.

Sources: `/home/nexus/NEXUS/m3e-mission-draft.md` (U1, D4, D11–D13,
rulings M3E-R1 and M3E-R2), `/home/nexus/NEXUS/m3-design-approved.md`
(§1.1 steps 0–1, §6.1, §9.5).

## M-1 — input snapshots and running-image identity

Code: `mother/host/src/inputs.rs`, `mother/host/src/binary_identity.rs`,
`mother/src/adapter/input_files.rs` (M-1b). Tests:
`mother/host/tests/page_inputs.rs`, plus the in-module unit test in
`input_files.rs` (M-1b; it was in `binary_identity.rs` in M-1). Linux only.

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
- **CR (M3E-R3 2).** Any CR anywhere refuses: facts `facts_file_invalid`
  at its line, notes `notes_carriage_return`, goal `goal_not_plain`.
  Parsed values (`InputValue`) never show input text in `Debug`.
- **Recorded limitations (M3E-R3 3, 4).** Parent-directory symlinks of the
  Owner-supplied absolute paths are followed; a device path is opened
  (read-only, non-blocking) before `fstat` refuses it.
- **Placement (M3E-R3).** All descriptor, `fstat`, read and digest code,
  including the `/proc/self/exe` hash, lives in the Mother adapter
  (`mother/src/adapter/input_files.rs`); host `inputs.rs` and
  `binary_identity.rs` hold only pure parsing and types and call it.
- **Carry.** `MissionContract` notes validation uses `str::lines()`, so an
  interior `\r\n` passes the contract; a later Mother slice fixes the core.
  The host refusal above is the M-1 fail-closed control.
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
| `m3e_u1_values_redacted_and_any_cr_refused` (M3E-R3) | Goal, notes and facts canaries absent from parsed-value and snapshot `Debug`; CR, CRLF, lone and trailing CR refuse in notes (`notes_carriage_return`), goal (`goal_not_plain`) and facts (`facts_file_invalid` at the line). |
| `m3e_u1_running_binary_identity_unavailable` (unit) | Real image digest first; then a missing path, a directory and a size-0 `/proc` file each give `self_identity_unavailable`. |

### Plants (each temporary, killed at its named assertion, never committed)

After M3E-R3, P-U1a, P-U1b and P-U1d target `mother/src/adapter/input_files.rs`.

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

## M-2 — `check-page`, bare and with `--facts` (S1a-E)

Code: `mother/host/src/check_page.rs`; dispatch in `mother/desktop/src/main.rs`
before any home or GTK. Tests: `mother/host/tests/check_page.rs`,
`mother/desktop/tests/check_page_cli.rs`, and the in-module unit test in
`check_page.rs`. Rulings M3E-R1..R6; design §5.5, T14.

### Frozen behaviour (M3E-R6)

- **Command.** Exactly `nexus-mother check-page <file> [--facts <file>]`.
  Paths are used as given (relative or absolute, UTF-8 or not), never
  canonicalized; a path that is empty or starts with `--`, a missing,
  duplicate, unknown or extra argument is usage. `check-page` anywhere but
  first (`--` included) is usage, exit 2, before any desktop start (M3E-R9).
  Any other argument list runs the desktop exactly as before.
- **Read-only.** The page (at most 65,536 + 1 bytes) and the facts file are
  read once each through the M-1 adapter reader (`input_files`: no final
  link, no blocking, regular file, bounded read) and the M-1 facts parser.
  Nothing is written, fixed, rendered or sent; no model, network, browser,
  home, journal or GTK.
- **Output golden** (stdout, one item per line):
  1. `binary sha256 <hex>` (the running image, M-1);
  2. `policy <hex>  css <hex>`;
  3. `file bytes <n> sha256 <hex>`, or for a page over 65,536 B
     `file bytes unavailable (over 65536) sha256 unavailable` (no prefix hash);
  4. `LABEL_PASSED`, or `refused: <s1a_code> at line L, column C` (the
     kernel's `Refusal`, 1-based, columns in characters), or
     `refused: input_too_large`;
  5. the OPEN line (`not checked: how the page looks in a browser (375 px and
     1366 px are OPEN)`);
  6. bare: `facts: not checked (no --facts given)`; with `--facts`, three
     lines: `facts file sha256 <hex>  facts hash <hex|unavailable>` (the facts
     hash is `ApprovedFacts::hash`, as the consent dialog shows it);
     `facts present as given: name, hours (N lines), phone` (the policy's
     template, only when V3 passes) or `… refused <code>` or `… not run`;
     `visible text: pass` / `refused <code>` / `not run`;
  7. `result: PASS` or `result: REFUSED <code>`;
  8. T14 (frozen, `teaching.rs`): `t14_passed(<escaped facts path>, <facts
     hash>)` only for a with-facts PASS; `t14_refused(<code>, <location>)` for
     every refusal (the location only for a page-check refusal); nothing for a
     bare PASS.
- **Order and codes.** First failure in V2 → V3 → V4 → V5 order. V2 is the
  bare S1a code (`script_element`, …) or `input_too_large`. V3 is a facts-file
  refusal (`facts_file_invalid (facts file line L)`, `facts_word_too_long
  (facts file line L)`, `ApprovedFacts` codes, `input_not_utf8`) or the first
  missing fact (`name_not_heading`, `name_not_title`, `hours_line_missing:<i>`,
  `phone_missing`, `foreign_number`). V4 is `reader_unsupported`,
  `name_not_visible`, `hours_line_not_visible:<i>`, `phone_not_visible`. V5 is
  `email_not_supported`. V3/V4 are `not run` after a page refusal (even if
  the facts file is malformed too, M3E-R9); each key that runs is printed,
  and each one vetoes on its own.
- **Exit.** 0 PASS. 1 any refusal, including semantic facts refusals and a
  page over 65,536 B. 2 usage (`ERROR usage: nexus-mother check-page <file>
  [--facts <file>]`), a read refusal of the page or facts file (`ERROR <code>
  (page)` / `ERROR <code> (facts file)`, M-1 codes, no OS text; a facts file
  over 2,048 B included), or the running image unavailable (`ERROR
  self_identity_unavailable`, before any read).
- **Claim.** Bare never claims facts. With `--facts`, a PASS means agreement
  with the supplied snapshot only; acceptance compares the printed facts hash
  with the consent's. No fictitious publication receipt is made.

### Oracles

| Oracle | Required assertions |
|---|---|
| `s1ae_bare_receipt_has_no_facts_claim` (S-O17, M3-O41) | Bakery and S1a reference pages: exact golden, exit 0, no facts claim, facts hash or T14 pass; email-shaped page exit 1 `email_not_supported` with T14 refused. Binary, policy, CSS and file hashes independent. |
| `s1ae_supplied_facts_two_keys` (M3-O41) | Matching facts: exact golden with facts-file sha256, facts hash, policy facts line, `visible text: pass`, T14 pass; non-fact edit passes; changed name/hours/phone: its own facts hash, both keys refused with their exact codes, V3's code first, no location; a foreign number: V3 refuses while V4 passes. |
| `s1ae_refusal_location_and_exit_contract` | `<a href>` on its planted line, `<script>` and the old template: the kernel's exact code, line and column, bare and with facts (V3/V4 `not run`); invalid facts syntax: facts line only; cap + 1: no prefix hash; at cap: hashed and refused by the kernel; read refusals and usage exit 2 with fixed lines; non-UTF-8 and relative paths; canaries absent. |
| `s1ae_self_identity_unavailable_refuses_before_any_read` (unit) | Image unavailable: exactly `ERROR self_identity_unavailable`, exit 2, nothing read. |
| `s1ae_page_refusal_precedes_malformed_facts` (M3E-R9 2) | Script page and malformed facts: V3 and V4 `not run`, result the page's S1a code with its location, exit 1. |
| `s1ae_v4_vetoes_when_v3_is_forced_to_pass` (unit, M3E-R9 3) | V3 forced to pass, page name not the facts': `visible text: refused name_not_visible`, result `REFUSED name_not_visible`, exit 1. |
| `s1ae_misplaced_check_page_never_reaches_the_desktop` (desktop, M3E-R9 1) | `-- check-page …`, `x check-page …`, `--check-page check-page …`: exit 2, exactly the usage line, no stderr, HOME stays empty. |
| `s1ae_headless_binary_has_no_home_side_effect` (desktop) | The real binary, no display, fresh HOME/XDG roots: `sha256sum`-equal hashes, exit 0/1/2, no stderr, HOME stays empty, inputs untouched. |

### Plants

| Plant | Mutation | Killing oracle |
|---|---|---|
| MP-M3-14 | `check-page` ignores `--facts` | two_keys: matching facts golden |
| MP-M3-24 | bare run prints a facts line | bare: golden |
| P-U2a | skip V5 | bare: email page refuses |
| P-U2b-3 | force V3 to pass | two_keys: changed name refuses with V3's code |
| P-U2b-4 | force V4 to pass | two_keys: changed name `visible text: refused` |
| P-U2c | run GTK before the dispatch | desktop headless: exit 0 |
| P-R9-1 | drop the non-first-position `check-page` refusal | desktop misplaced: exit 2 + usage |
| P-R9-2 | V3's facts refusal before the page-refusal precedence | precedence: `facts present as given: not run` |
| P-R9-3 | omit V4 from the result aggregation | veto: exit 1 |

## M-3a — typed goal mode and goal display

Code: `mother/host/src/goal_input.rs`, the single reader in
`mother/host/src/terminal.rs`, `inputs::validate_goal`. Tests: the `m3a`
module of `mother/host/src/terminal_tests.rs`, `mother/host/tests/goal_input.rs`.
Rulings M3E-R1, R8 (M-3b, the teaching/T10/T11/T12/explain display and O34,
is a separate unit).

### Frozen behaviour (M3E-R8 3 (a)–(h))

- **Data-only.** `Shared::read_goal` / `goal_input::read_typed_goal` are not
  called from the scripted `run` flow; the existing walk, PTY and usability
  output is unchanged. M-5 wires them.
- **One line, one reader.** The terminal's single reader switches to a
  separate goal buffer only after the goal prompt is acknowledged, a boundary
  invalidates earlier input, and `INPUT READY` is queued. Return (CR, LF or
  CRLF) ends the goal; a typed goal holds no CR or LF.
- **Bounds and grammar.** At most 16,384 bytes; more is `goal_too_long` and
  every byte is discarded. The bytes then go through `inputs::validate_goal`,
  the goal-file grammar (`input_not_utf8`, `goal_empty`, `goal_not_plain`,
  `goal_too_long` over 4,096 scalars). The goal is kept verbatim.
- **Ingress.** A line that is exactly `stop` or `quit` is Stop or Quit;
  Ctrl-D is EOF; a read error is a failure. None yields a goal. A goal line is
  delivered after the rest of its read, so Stop, Quit or EOF in the same read
  wins (`goal_interrupted`). Every other text, `approve` and codes included,
  is data and never reaches the command queue or a confirmation; the rest of
  the read after a goal line is discarded command input.
- **Line continuity (M3E-R11).** A line always finishes in the mode it
  started in. A goal line left unfinished when goal mode ends (timeout,
  interruption) is read on to its terminator as discarded goal input, never
  as a command. A command line left unfinished when goal mode starts is read
  on to its terminator in the command buffer (stale; only an exact `stop` or
  `quit` acts); the goal starts after it. Stop, Quit and EOF always act.
- **Line ownership (M3E-R13, supersedes R11's CR rule).** The reader keeps
  one stream-level owner of the line under way: `None`, `Command`,
  `Goal(read)`, `DiscardGoal` or `DiscardCommand`. The first byte of a line
  sets it (any byte but a terminator, editing keys included); only CR, LF or
  EOF clears it; it is never derived from buffer contents, so erasing a line
  keeps its owner. A boundary or the end of its goal read makes a live owner
  a discard owner. One stream-level `last_was_cr`: a LF directly after a CR
  on the stream is that CR's partner; any other byte clears the flag. The
  buffers are storage only. EOF acts at once and clears the line.
- **Echo and editing.** Only `shown` characters are echoed; Backspace removes
  one scalar, Ctrl-U the line. The 64-byte command/code mode is unchanged.
- **Display.** `teaching::t1` with the goal through `evidence::escaped`, then
  `goal: <bytes> bytes, sha256 <hex>`. A refusal shows `GOAL REFUSED <code>`
  only. `GoalLine` and `TypedGoal` `Debug` show sizes or codes, never text.
- **Goal file given.** Typed mode is refused with `goal_already_given`; no
  prompt is shown. Other refusals: `goal_unavailable` (no valid session or a
  dialogue already open), `goal_timed_out`, `goal_interrupted`.

### Oracles

| Oracle | Required assertions |
|---|---|
| `m3e_m3a_goal_line_rules_share_the_file_grammar` | Verbatim goals (spaces, `approve`, a code, multibyte, 4,096 scalars, 16,384 B) accepted exactly; 4,097 scalars, 16,385 B, empty, blank, tab, bidi, zero-width, ESC and invalid UTF-8 refused with `validate_goal`'s own codes; CR, LF and CRLF each end one line; overflow keeps no byte. |
| `m3e_m3a_stop_quit_eof_and_stale_input_never_yield_a_goal` | `stop`, `quit`, Ctrl-D; stale input before a boundary never completes a goal, and a stale `stop` still stops. |
| `m3e_m3a_echo_and_editing_show_only_shown_characters` | No control, bidi or bell byte echoed; Backspace one scalar; Ctrl-U the line. |
| `m3e_m3a_goal_read_through_the_single_reader` | Prompt, boundary, `INPUT READY`; input typed in the gap is dropped; the goal is returned verbatim and displayed exactly; no command or line queued; a refusal shows its code only; Stop, Quit and EOF interrupt; Stop in the same read wins; command mode is unchanged afterwards. |
| `m3e_m3a_r11_goal_timeout_mid_line_never_becomes_a_command` | `Please ` in goal mode, timeout, then `run\r`: no Run, no goal; command mode resumes. |
| `m3e_m3a_r11_stop_split_across_the_handoff_stops` | `st` in the gap, `op\r` after ready: Stop acts, no goal. |
| `m3e_m3a_r11_quit_split_across_the_handoff_quits` | `qu` in the gap, `it\r` after ready: Quit acts, no goal. |
| `m3e_m3a_r11_a_partial_command_line_finishes_before_the_goal` | `ab` in the gap, `cd\r`, then `goal\r`: the goal is `goal`; no command or line queued. |
| `m3e_m3a_r13_generative_routing_keeps_i1_to_i5` | Through `route`, against the ruled stream semantics: every sequence of up to 4 over {s,t,o,p,q,u,i,r,n,a,' ',BS,^U,CR,LF,^D} × {ready, timeout} in both read groupings; every sequence of up to 6 over one byte of each class and both events; 200,000 seeded sequences of up to 7 tokens (a token may be `stop`, `quit`, `run` or half of one) with random grouping. I1 returned goals only from lines started in their ready read; I2 commands only from lines started in command mode; I3 exact `stop`/`quit` acts; I4 LF pairs only with the stream's preceding CR; I5 EOF acts. |
| `m3e_m3a_r13_erasing_a_timed_out_goal_line_never_makes_a_command` | Review 2 MED, both timings: `Please `, timeout, `^U run\r`; `Please ^U`, timeout, `run\r`: no command or line; command mode resumes. |
| `m3e_m3a_r13_an_old_cr_never_pairs_with_a_later_lf` | Review 2 LOW: `goal\r` returns `goal`; `status\n` is a command; in the next goal read a lone `\n` is `goal_empty`. |
| `m3e_m3a_goal_file_given_refuses_typed_mode_and_debug_is_content_free` | `goal_already_given`, no prompt, goal mode never entered; canaries absent from `Debug`. |
| `m3e_m3a_typed_and_file_goals_share_one_grammar` (host) | For each text, `validate_goal` equals the goal file's parse, with and without one final LF. |

### Plants

| Plant | Mutation | Killing oracle |
|---|---|---|
| P-U3a | a goal line goes through the command path | single reader: no command queued / goal returned |
| P-M3a-1 | bypass `validate_goal` (a second, laxer grammar) | grammar: 4,097 scalars refused |
| P-M3a-2 | CR is data, not Return | grammar: CR ends a line |
| P-M3a-3 | ignore stale input | stale: never completes a goal |
| P-M3a-4 | echo every byte | echo: no control byte echoed |
| P-M3a-5 | `stop` stored as a goal | stop/quit: `stop` is Stop |
| P-R11-1 | clear the goal buffer without draining when goal mode ends | timeout mid-line |
| P-R11-2 | no command-line continuation across the handoff | stop / quit / partial command handoff |
| P-R13-1 | ownership derived from buffer contents (empty buffers: no owner) | generative (10,200 cases); erasing reproducer |
| P-R13-2 | per-buffer CR flag (CR fed to the buffers, no stream pairing) | generative (2,073 cases); old-CR reproducer |

P-R11-1 and P-R11-2 mutate code that R13 replaced; their oracles still pass.

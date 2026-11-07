# M3-Ca frozen oracle register

Authority: Owner's M3-Ca execution request, frozen Ca mission sheet and its
2026-11-06 ARCHITECT RULINGS; approved M3 design §4.2 and §9.5.
Base: `67c050b8a29131e2f5236aefa78b14a742248df6`.
Only the eight Ca allowlisted paths may change. Git remains read-only;
A and B are serialized commit folders, with full replacement file bytes.

## Adopted boundaries

Private pure core; explicit `ApprovedFacts`, `Note`, supplied 32-lowercase-hex
marker. No production caller, lane, broker, model call, filesystem access,
publication, new dependency, purpose floor or route opening. File requests
are data. Existing session denial and charge accounting remain unchanged.

Shape first (including duplicate/unknown/missing fields); normalize TAB/LF/CR
to ASCII space, collapse ASCII spaces, trim. Global text precedence across
all fields: needle, markup, profile, words, empty. Then schema scalar and
collection caps on normalized text (Unicode scalar maxLength, not UTF-8 byte
length); malformed caps yield `draft_too_large`. Byte budgets use UTF-8 bytes.
Then typed S1a construction/check, FC-4, shared email scan, File byte/line
limits, request construction. Unexpected S1a/FC-1–3 is `assembler_fault`.
Approved-phone repetition retains existing FC-4 semantics.

Immutable facts are preflighted as `facts_not_page_ready` before rendering;
no model correction can fix them. Exactly one malformed correction; needles
end immediately as `model_out_of_scope`; second failure ends; terminal
states refuse further attempts. Correction blocks contain only frozen text.
Draft values have no Debug/Display. New diagnostics expose codes/counts only.

Exactly one `notes` note; rendered note cap stays 1,792 B, hence text ≤1,777 B.
Rendered facts + note cap is 2,432 B including every label/newline/separator.
Schema keys explicitly sorted in the frozen compact schema. Frozen hashes
are independently pinned in fixtures; mismatch means STOP, never re-freeze.

## A → B runtime matrix (registered before execution)

Every row below must compile at A. R = runtime red at fail-closed stub;
G = expected green at B. Independent fixture hashes must remain green.

| Oracle/test suffix (`m3c_`) | A | B | Literal control |
|---|---|---|---|
| frozen_fixture_hashes | G | G | All §4.2/9.5 constants, corrections, MIN/MAX and qualification notes hashes |
| request_constants_and_schema_are_exact | R | G | 593/559/574 B; sorted schema; fixed user 661 B; correction suffix exact |
| request_bound_counts_every_separator | R | G | MIN 47/1301; MAX 2432; all 8 corrections; 2433 refused |
| oversize_request_never_reaches_caller | R | G | 3937 refused by public constructor; zero synthetic dispatches; valid control |
| fact_and_note_markers_are_data | R | G | marker validation, guessed closers, placeholder literals, determinism |
| seeded_drafts_are_checked_or_refused | R | G | 2000 seeded cases; canonical bytes/facts/email or closed refusal |
| owner_facts_are_inserted_verbatim | R | G | independent body golden; title, sole h1, whole li, strong phone; escaping |
| strict_draft_shape | R | G | object/field/type/trailing/fence negatives plus positive empty items |
| classification_and_precedence | R | G | every code; cross-field precedence; phone repetition and @ control |
| scalar_and_collection_boundaries | R | G | multibyte scalar boundaries, normalized caps, all collection bounds |
| literal_path_is_only_page_text | R | G | one CreateFile named index.html, visible literal path |
| correction_state_is_bounded_and_content_free | R | G | one correction; no third answer; exact block; no rejected canary |
| errors_and_diagnostics_do_not_echo_drafts | R | G | note/fact/draft canaries absent from diagnostics |
| file_limits_precede_effect_construction | R | G | 8192/8193 B, 150/151 lines; schema-valid escaping overflow; zero construction |
| email_never_becomes_a_file_request | R | G | email: zero requests and zero constructions; @ control: one |
| immutable_facts_are_checked_before_requests | R | G | ready positive; email and overlong fact words refused before request |

Negative assertions within a red test may already hold at A. Runtime red
must be a failed behavioral assertion/panic, never a compile failure.
Construction counters are thread-local test instrumentation at the actual
request constructor, so an early constructed/discarded request is observable.

## Mutation map (never committed; restore byte-for-byte)

| Plant | Mutation | Named oracle suffix |
|---|---|---|
| MP-M3-6 | draft lead replaces facts h1 | owner_facts_are_inserted_verbatim |
| MP-M3-7 | bypass typed construction and checking with raw draft bytes | seeded_drafts_are_checked_or_refused |
| MP-M3-8 | correction appends rejected answer | correction_state_is_bounded_and_content_free |
| MP-M3-15 | needle gets a correction | correction_state_is_bounded_and_content_free |
| MP-M3-16 | first markup ends immediately | correction_state_is_bounded_and_content_free |
| MP-M3-18 | construct File request before email scan | email_never_becomes_a_file_request |
| MP-M3-21 | combined budget excludes labels/separators | request_bound_counts_every_separator |
| MP-Ca-1 | accept unknown fields | strict_draft_shape |
| MP-Ca-2 | scalar cap uses bytes | scalar_and_collection_boundaries |
| MP-Ca-3 | accept third answer | correction_state_is_bounded_and_content_free |
| MP-Ca-4 | skip File limits | file_limits_precede_effect_construction |

## Deferred evidence and gates

Cb owns real caller charges, lifecycle invalidation, wire capture, fresh UUID
markers, journal/ledger/privacy composition, and repeats MP-M3-8/15/16/18.
M3-D supplies immutable consent-bound input and contract-time combined cap.
Cq owns verified-purpose qualification, O28 and MP-M3-25/26. Q-M3 is unverified.
Ca is not full M3-C completion, live readiness, phase advancement or acceptance.

Required local gates: `cargo test -p nexus-mother --locked m3c`, full `--lib`,
`cargo fmt --all -- --check`, Mother all-target clippy with `-D warnings`,
`git diff --check`; target `/home/nexus/.cache/m3ca-target`, no network.
No retained tests/expectations may change. Hard cap: 1,500 changed lines.
Actual red lines, gate totals, plants and limitations go in the external
build report; the register and assertions stay frozen after the first result.

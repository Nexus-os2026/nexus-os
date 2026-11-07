# M3-Cb oracle register

Scope: Cb0 only (Owner option D), base 8df09f57017e822b196c6d2e1024d768e84b7780.
Cb1 lane and Cb2 composition remain pending; this slice changes no Mother code.

## Cb0 — broker marker helper

API (`nexus_governed_control::authority::ids`): `MarkerUnavailable` (Clone, Copy,
Debug, PartialEq, Eq) and `fresh_page_marker() -> Result<String, MarkerUnavailable>`.
16 new bytes from `getrandom::getrandom`; failure is `Err(MarkerUnavailable)`,
never a panic, fallback, counter, time, seed or previous value. Byte 6 becomes
`(b & 0x0f) | 0x40`, byte 8 `(b & 0x3f) | 0x80`; output is 32 lowercase hex.
`random16()`, identifier minting and their callers are unchanged. A marker is
framing data only, never consent, identity or authority.

Test seam: private `cfg(test)` `marker_fault`, one-shot per thread, replacing the
generator result with an error before encoding. No public provider injection.
It does not claim an actual OS generator failure.

| Oracle | Assertions | Inert stub (`Err`) | Implementation |
|---|---|---|---|
| `m3cb_broker_marker_is_fallible_v4` | 32 normal markers are v4 lowercase hex, consecutive ones differ; one injected fault returns exactly `Err(MarkerUnavailable)` under `catch_unwind`; the next call succeeds with a new marker | RED: normal-success assertion | GREEN |
| `m3cb_broker_marker_uses_approved_entropy` | Unique ungated production body equals the approved token sequence; fixed, counter, timestamp, seeded and getrandom-then-overwritten substitutes produce v4-shaped (and, where applicable, distinct) outputs yet fail the same control | RED: production body | GREEN |

| Plant | Mutation | Required kill |
|---|---|---|
| MP-Cb-D1 | Swallow injected entropy failure, return zero bytes | `m3cb_broker_marker_is_fallible_v4` typed-error assertion |
| MP-Cb-D2 | Counter-derived v4-shaped source | `m3cb_broker_marker_uses_approved_entropy` production-body control |
| MP-Cb-D2s | Seeded xorshift v4-shaped source | `m3cb_broker_marker_uses_approved_entropy` production-body control |

Evidence (local, Linux): inert stub, both oracles RED as registered; implementation,
both GREEN. D1 killed: fault returned `Ok("00000000000040008000000000000000")`
(the structural control also failed). D2 and D2s killed by the structural control
alone; their outputs passed the behavioral format/distinctness oracle, showing
the control discriminates v4-shaped deterministic sources. Each plant was
restored byte-for-byte (file and diff hashes equal) and never committed.

Structural equality pins source text; it is not a statistical proof of
unpredictability. MP-Cb-9 (fallback after typed failure) is owned by Cb1.

# Stage 3 — M18 technical disposition

**M18 was not executed by this or any mission at Candidate C.** No mutation was applied; nothing was built or run.
Every line number below was re-read by this mission from Git objects (Candidate C `d5964ee5…`, historical base
`9d5fdf95…`).

## 1. Definition — unchanged (mechanical, `tools/s3_h5.py`)

| Field | Original manifest (1-r1, `2c4adc84…`) = ct-1 (`20bcc6fe…`) |
|---|---|
| Meaning | "a commitment releases a lease it does not list" |
| Edit | `crates/nexus-governed-control/src/broker.rs`: `        if !prepared.leases.contains(&lease_id) {` → `        if false {`, count 1 |
| Command | `cargo test -p nexus-governed-control --locked --lib -- broker::tests::a_commitment_cannot_release_a_lease_it_does_not_list` |
| Expected failing test | `broker::tests::a_commitment_cannot_release_a_lease_it_does_not_list` — body byte-identical at `9d5fdf95` and C |
| Anchor | line 350 at `9d5fdf95`; **line 403 at C**; exactly 1 occurrence |
| File | blob `d858f890` (sha256 `471d8a79…`, the recorded hash) at the base → blob `eaa469d9` (sha256 `353bbbc8…`) at C; first stale at `862a11ce` (XA-R1-02, "the broker re-checks run and lease liveness before it reads the vault"); +66 −5 |

## 2. Source facts at C (re-read)

| Line | Text | Role |
|---|---|---|
| broker.rs 403 | `if !prepared.leases.contains(&lease_id) {` | M18's anchor: the first membership check |
| broker.rs 436 | `lease.state = LeaseState::Released;` | the lease is consumed after the first check |
| broker.rs 454 | `self.still_releasing(lease_id, guard, &view)?;` | **a second check, added by `862a11ce`, before the vault read** |
| broker.rs 357 / 365 | `fn still_releasing(` … `Some(prepared) if prepared.leases.contains(&lease_id) => Ok(()),` | the second check's membership arm, not edited by M18 |
| broker.rs 385 | `lease.state = LeaseState::Ended;` | the re-check's refusal ends the lease |
| broker.rs 455 | `let secret = self.source.read(spec.scope, spec.name);` | the vault read, reached only after the re-check |
| tests.rs 469–472 | `assert_eq!(… .unwrap_err(), AuthorityError::Closed("the lease is not bound to this commitment"))` | assertion 1 |
| tests.rs 473 | `assert!(broker.is_live(other), "the unlisted lease was not consumed");` | assertion 2 |
| tests.rs 474 | `assert_eq!(vault.0.load(Ordering::SeqCst), 0, "the vault was not read");` | assertion 3 |

At the historical base, `still_releasing` does not occur in `broker.rs` (0 occurrences); at C it occurs at 357, 454 and 460.

## 3. Disposition

| | Historical tree | Candidate C (static inference, not executed) |
|---|---|---|
| Intended authority property | an unlisted lease must not be released or consumed | **the same** |
| With M18: first check | bypassed | bypassed |
| Unlisted lease consumed | yes | yes (line 436) |
| Credential read and returned | **yes** — the historical logs show `unwrap_err()` on `Ok("authorization: Bearer tok-SECRET-…")` | **no** — the re-check at 454 refuses with the same `Closed(…)` error before the read at 455 |
| Assertion expected to observe the fault | #1 | **#2 only** (`is_live(other)`: the lease is `Ended`, never `Issued` again) |

**SAME INTENDED AUTHORITY PROPERTY. CHANGED OBSERVABLE FAILURE MECHANISM.** This is the source-level disposition the
mission records as previously conveyed; this mission's re-reading agrees with it and found no contradictory source
evidence. It is **executor source analysis plus a conveyed disposition — not an Architect acceptance of M18 at the
current tree**, and not a kill.

- The historical credential leak was **not observed at Candidate C** and is not claimed.
- **No fresh Candidate C M18 kill** has been run or is claimed.
- ct-1's M18 current review status stays `pending`; ct-1 is not edited.
- The relayed blanket ruling "All 22 stale M-control mappings: SAME PROPERTY" (FINAL-EVIDENCE-STAGE1) predates this
  analysis. Whether it stands for M18 — and whether a fresh M18 execution is needed — is an Architect decision
  (`03-REMAINING-183-DECISIONS.md`, item 2).

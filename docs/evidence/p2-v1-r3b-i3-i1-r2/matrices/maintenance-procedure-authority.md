# Maintenance procedure authority matrix (P2-V1-R3B-I3-I1-R2)

This is the audit of every design section 13 operation against the common
rules of design section 13.1, using the columns of the mission's section 10.
Source: `crates/nexus-verifier-sandbox/tests/support/custody/store/maintenance.rs`
in the candidate.

**Column terms.**
- **Verified facts consumed** are machine facts the procedure takes from the
  session's own verification, never from its caller.
- **Caller data accepted** is Owner input: it establishes no store fact.
- **Lapses** is when a verification the procedure could consume stops being
  usable.
- **Enforced** means the candidate's API enforces the row; **Repair** compares
  the candidate with the base `ed7c7088`.

**Every session procedure (section 13.1).**
- **The verification.** The session retains its latest verify-before, bound
  to the session's mutation count, its opening, and the root id, revision,
  digest and state name of its selection. A procedure takes it once.
- **Lapsing.** Starting any step of any of the session's procedures lapses it
  (R1). A verify-after clears it (R2).
- **The gate.** Before its first step, the gate refuses if any step of the
  session has started since the procedure was built.
- **The verify-after.** It runs within the last step, and the procedure is
  complete only after it.
- **The generic verify-after fails when** the session lost its lock or
  selection, or a leftover temporary remains.

| Procedure | Verify-before required? | Verified facts consumed | Caller data accepted | Mutation boundary | Verification lapses when | Verify-after required? | Current API enforces it? | Repair required? | Negative control |
|---|---|---|---|---|---|---|---|---|---|
| P-PROV (13.2) | No session exists before a store (R5: "Provisioning runs before any store exists, so it has no session"). Its precondition, Unprovisioned, is read afresh at the gate | None from a session. At the gate, afresh: no `PROVISION` at its path, and no state root under the parent with a non-zero pool byte | `Layout`, `Qualification` (Owner-established, 13.2 step 1) | The 23 operations of 15.3, all after the gate | n/a | R5 step 6: the standalone verifier, run as the store uid, reports a fresh store. That is an Owner act under another identity; see the non-repairs | Yes (gate) | Yes. The base provisioned over an existing `PROVISION` and over roots with history | NC-PROV-OVER-EXISTING, NC-PROV-OVER-HISTORY (v17) |
| R-REPUBLISH (13.2 recovery) | No session. Its precondition is read afresh at construction and at the gate | The root's inode identities (read afresh), and `roots_with_history` | `Layout`, `Qualification` | Unlinking a leftover `PROVISION.tmp`; writing, syncing and renaming `PROVISION`; syncing its directory | n/a | No. After re-publication the ordinary refusal follows (13.2) | Yes (construction and gate): no `PROVISION`, and no root that may hold history other than this one | Yes. The base re-published over an existing `PROVISION` | NC-REPUBLISH-OVER-EXISTING (v17); I-FRESH-ROOT (m06) |
| P-DISP (13.4) | Yes (13.4 step 2). The verification must be complete, with only Invalid or Capacity conditions (dispositions are how an Invalid predecessor, 13.6, or an over-capacity store, 13.10, is resolved) | The binding, which must be a verified current incident or an archive entry without its disposition. The incident's kind, claim, generation, pool index, content digest, class and recorded-unsettled count; for an archived journal, from its archived bytes, checked against the content digest in its name. The root id. That no disposition exists for the binding | `OwnerWords`: reason, statement, operator, time | Creating `dispositions/.tmp-<binding>` (exclusive); writing; `fsync`; `link` (no replacement); `unlink`; `fsync dispositions/` | At the first step of any session procedure; consumed by this one | Yes (generic) | Yes | Yes. The base published any caller-built `Disposition`, so a predictable binding (a claim gap's) could be dispositioned before any verification reported it | NC-DISP-UNREPORTED, NC-DISP-FACTS-NOT-VERIFIED (v15); NC-DISP-ARCHIVED-NAME-ONLY (v16); NC-DISP-NO-POSTVERIFY (v19) |
| P-REVOKE (13.4) | No. It takes no fact from a verification. Its only effect is that an incident blocks again, which is fail-closed | None | The binding and the revocation time. An absent binding fails the rename | `rename` into `revoked/`, `fsync revoked/`, `fsync dispositions/` | Its first step lapses any earlier verification (test v04) | Yes (generic) | Yes (gate and verify-after) | Gate and verify-after added | NC-LEFTOVER-AFTER-ACCEPTED (v21: a revocation over a leftover is not complete); the shared gate: NC-SUCC-MUTATE-BEFORE-GATE |
| P-ARCH (13.7) | Yes (13.7: preconditions verified in-session). Complete, with only Capacity allowed (13.10 names archival as a resolution) | The file's class, valid header and content digest; for `u`, `m` and `p-`, a valid disposition for its binding (`archival_allowed`, over the session's report) | The pool index | Creating `archive/.tmp-<name>`; writing; `fsync`; re-reading and comparing; `link`; `unlink`; `fsync archive/` | As P-DISP; consumed | Yes (generic) | Yes | Yes. R1 had refused archival over capacity, which broke 13.10's resolution. Gate and verify-after added | NC-ARCHIVE-CAPACITY (v18); R1-VERIFICATION-REUSED, R1-VERIFICATION-NEVER-LAPSES (b08); I-BINDING-LOCATION (m04) |
| P-RECYCLE (13.8) | Yes (13.8: read from the in-session report). Complete, with no condition (as in R1; 13.10 does not name recycling) | The file's class: pending recycling (archived) or AbandonedClaim | The pool index | Creating `journals/.tmp-<index>`, `fallocate`, zero, `fsync`; `rename`; `fsync journals/`; the `PROVISION` rewrite (13.3) and re-selection | As P-DISP; consumed | Yes (generic) | Yes | Gate and verify-after added | I-HIDDEN-STEP (m08); R1-VERIFICATION-REUSED (b08) |
| R-RESUME (13.8 resumption) | Yes. The session's own verification must refuse exactly with `Lost("<pool file> replaced")` (13.8 resumption step 2) | That refusal. The file at the pool name, read through the session's journal lock: `uid:gid 0600`, one link, the pool-file size, all zero | The pool index; the interrupted session's saved report (Owner input, as 13.8 states; checked only against the session's own refusal and reads) | The `PROVISION` rewrite (13.3) and re-selection | As P-DISP; consumed (`take_refusal`) | Yes (generic) | Yes | Gate and verify-after added | m09 (crash matrix, resumption); the shared gate controls |
| P-RETIRE (13.9) | Yes (13.9: over the complete set). Complete, with no condition (as in R1) | Every claim at or below K is archived history or an applied gap with a valid disposition; no pool file holds such a claim (`retirement_allowed`, over the session's report) | K | The `PROVISION` rewrite (13.3) and re-selection | As P-DISP; consumed | Yes (generic) | Yes | Gate and verify-after added | I-RETIRE-NO-PRECONDITIONS (m04), I-RETIRE-FROM-REPORT (m10) |
| P-REQUALIFY (13.2, 13.3) | No. The qualification is the Owner's own establishment (13.2 step 1), in a re-qualification session | None | `Qualification` | The `PROVISION` rewrite (13.3) and re-selection | Its first step lapses any earlier verification | Yes (generic) | Yes (gate and verify-after) | Gate and verify-after added | m05 (re-qualification); the shared gate controls |
| P-SUCCESSOR (13.6) | Yes (13.6 precondition) | A complete verification. Every claim gap enumerated. No leftover. Every current incident with an exact disposition, over the complete set and never the bounded detail. No archived incident without its disposition. The exact set of store-level Invalid conditions, by canonical name. The predecessor's `PROVISION` (re-checked at the gate) | The Owner's acceptance of the Invalid conditions (it must equal the verified set exactly); the successor's `Layout` (a new root id is checked, and a new state root follows from `mkdir`) | 29 operations of 15.3: the predecessor's `PROVISION` kept (2a); the successor created (2b); its lock adopted, then its `PROVISION` published (2c); re-selection (2d). All after the gate | As P-DISP; consumed | Yes (2d), strict: the selection is the successor, with this predecessor, this statement and revision + 1, and it verifies with no condition and no incident | Yes | Yes. The base had no verify-before, a caller's free-text statement and no verify-after (B-S1 to B-S4) | NC-SUCC-* (22 controls; tests v01 to v13, v16, v22); NC-VERIFY-FIRST-ONLY, NC-OWNER-COLLECTS, NC-VERIFY-AFTER-AUTHORIZES, NC-GAP-UNBOUNDED; API/type guards P-VERIFICATION-TRANSPLANT, P-VERIFICATION-FORGE, P-TAKE-ASSESSMENT, P-SESSION-VERIFIED |
| R-LEFTOVER (13.1 recovery) | No. The leftovers are what it lists itself, and a verification refuses them (MaintenanceIncomplete), so recovery cannot depend on one | None | None | `unlink` of each leftover; `fsync` of each changed directory. With nothing to remove, one step that performs no operation | Its first step lapses any earlier verification | Yes (13.1 recovery: "verifies"). Generic, so it fails if a leftover remains | Yes | Gate and verify-after added | NC-LEFTOVER-AFTER-ACCEPTED (v21); m05, m09 |

## Deliberate non-repairs, and why they conform to R5

- **P-PROV step 6.** The check runs the standalone verifier *as the store uid*
  and must report a fresh, openable store. The provisioning procedure runs as
  root, before any store or session exists, and the store's platform has no
  way to act as another identity. Adding the check would change
  `provision`'s signature, and the fixture (`sim.rs`, outside this mission's
  envelope) calls it. The check stays the Owner's act, as R5 words it. Test
  `o11` runs it on a freshly provisioned fixture. A store that would fail
  it refuses at every opening (fail-closed).
- **No verify-before for P-REVOKE, P-REQUALIFY or R-LEFTOVER.** Each takes no
  machine fact from a verification. A revocation can only make an incident
  block again. A re-qualification's inputs are the Owner's own establishment.
  Recovery acts on the leftovers it lists, which a verification refuses. Each
  still has the gate and the verify-after.
- **No verify-after for P-PROV or R-REPUBLISH.** R5 gives them no session. The
  next opening performs every check.
- **The successor's `Layout` is Owner input.** Of R5 section 13.6 step 2b's
  two conditions:
  - a new root id is checked before the gate;
  - a new `<STATE_ROOT>` is enforced only by the first `mkdir`
    (operation 7), which fails on an existing entry. That comes after step
    2a has kept the predecessor's `PROVISION`.

  No other layout field is checked against the predecessor. The successor
  as selected is what its strict verify-after and every later opening
  check.
- **Observed, not repaired (outside the authority findings): repeating an
  interrupted succession.** R5 section 15.2 says that when §13.6 is
  repeated, step 2a is skipped if `<PROVISION_PATH>.predecessor-<old root
  id>` already holds exactly the current `PROVISION`'s bytes. The
  implementation, unchanged since I3-I1, always performs 2a, so a repeat
  after operation 4 (`link`) refuses with EEXIST. This is fail-closed: the
  predecessor stays selected and nothing is lost. But it leaves a repeat
  that cannot complete inside a procedure, and no test covers it. It is
  reported for the Architect's disposition, not changed here.

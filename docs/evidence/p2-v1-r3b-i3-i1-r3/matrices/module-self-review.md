# Complete-module self-review (P2-V1-R3B-I3-I1-R3, mission section 18)

**Scope.** The whole custody store,
`crates/nexus-verifier-sandbox/tests/support/custody/store/` (12 files
including `mod.rs`, 15,531 lines), as of the candidate snapshot (tree
`6350c192bee87e3523b3f5f7115c6ceaf983d506`). It covers more than the R3
diff.

**Method.** Each pattern below was searched for over every file of the
store:
- every `pub` item, and every method of the I/O trait;
- every removal (`unlink`, `remove_dir`, `rename`) and every path that
  reaches one;
- every function that takes Owner input (a layout, a binding, a time), and
  what it decides from;
- every procedure's gate and verify-after;
- every use of a count or capacity, and of the bounded report's `detail`;
- every `unsafe` block.

Findings that needed a change were fixed in R3; the rest are recorded with
their reason.

## Classification

R2's classification (`docs/evidence/p2-v1-r3b-i3-i1-r2/matrices/module-self-review.md`)
stands. R3 adds:

| Kind | What | Where |
|---|---|---|
| DATA | `KeptCopy`, `Incomplete` and `Revocation`: private values read afresh from the store, compared at the gate, never accepted from a caller. A kept copy's bytes and the `Provision` they parse to | `maintenance.rs` |
| REPORT | The recoveries' refusal texts; the verify-after's `Assessment`; `SessionEvent::VerifyAfter` | `maintenance.rs` |
| OWNER INPUT | R-SUCCESSOR's candidate `Layout`: it names the root, and authorizes nothing. R-REVOKE's binding and time, and P-REVOKE's time: they name the entries | `maintenance.rs` |
| RETAINED AUTHORITY | The session: its retained lock and its selection, revalidated afresh by `holds_selection` (`pub(super)`). R-SUCCESSOR's `LOCK_EX` descriptions of the candidate's `LOCK` and pool files, which the procedure holds until it ends | `maintenance.rs` |
| TRANSITION | Each recovery's gate (the same reads again), steps (identity-checked removals, syncs through new descriptions) and verify-after (the generic one, then completion) | `maintenance.rs` |

`StoreIo::remove_dir` is a primitive, like `unlink`: it bears no authority
by itself. In the store it is reached only through `remove_known`, a step
of a gated session procedure (API probe P-R3-REMOVE-KNOWN).

No DATA, REPORT or OWNER INPUT value becomes authority by matching:
- the candidate's name selects what is inspected; what the cleanup may
  remove is what the inspection found, under the session's lock and the
  candidate's own locks, and again at the gate;
- a binding and a time select two names; R-REVOKE acts only on the exact
  split it proves there, at construction, at its gate and in its
  verify-after.

## Section 18 searches

| Pattern | Result |
|---|---|
| Public functions of `maintenance.rs` | R2's 25, plus `recover_incomplete_successor` and `resume_revocation`. Each of the two takes only a session and Owner input, and returns a gated `Procedure` (test `r328` pins the list and both signatures). |
| Private helpers of the recoveries | `read_entry`, `absent`, `state_root_absent`, `kept_copy`, `revocation_admissible`, `interrupted_revocation`, `inspect_incomplete`, `succession_interrupted`, `unreferenced`, `remove_known`, `sync_known`, and the types `KeptCopy`, `Revocation` and `Incomplete`. All are private to `maintenance.rs` (`r328`; API probes P-R3-*). |
| Removals | `remove_dir` has one caller, `remove_known`. `remove_known` removes one entry, by name, only while it is the inode the inspection found, in a directory whose inode is the one found. R3 adds no other `unlink` or `rename`. P-REVOKE's `rename` is now preceded, at construction and at its gate, by the absence of its target. |
| Recursion | None. `remove_dir` is `unlinkat(AT_REMOVEDIR)` of one directory, which refuses one that is not empty (`n09` natively, `s06` in the simulator). The cleanup removes bottom-up, and an entry that appears during it makes its directory's removal refuse (`r308`). No store source contains `remove_dir_all` or `std::fs::remove` (`r328`). |
| Owner input deciding | A layout's parent, uid and gid must be the selected store's. Its root id and state name are checked against the selection, the recorded predecessor and every kept copy (`unreferenced`). The candidate's content is read in full, under locks. The binding and time must parse back as a revoked name. |
| Capacity | `revoked/`'s count refuses a normal revocation at 4096. R-REVOKE never consults it (`r321`; control NC-R3-REVOKE-CAPACITY-CONFLATED). A capacity count never permits anything. |
| The bounded report's `detail` | No R3 decision reads it. |
| Gates | P-SUCCESSOR's gate also rechecks the kept copy and the successor root's absence. P-REVOKE's gate rereads its admissibility, and the same active inode. R-SUCCESSOR's gate rereads the session's hold, `PROVISION`'s bytes, the references, the interruption and the candidate's inspection. R-REVOKE's gate rereads the session's hold and the same interrupted state. |
| Verify-after | R-SUCCESSOR: the generic one, then the candidate gone. R-REVOKE: the generic one, then the revocation completed on the same inode. |
| `unsafe` | One new block: `LinuxIo::remove_dir` (`unlinkat`), with its safety argument. The name passes the same one-component check as every other call. |

## Findings of the review

1. **Fixed: the candidate's binding.** The content checks alone would accept
   any all-zero root laid out as step 2b makes it. That includes a fresh
   root of another store uid under the same parent, and a retained
   predecessor whose own kept copy an Owner had removed. `unreferenced` now
   requires the store's uid and gid, and reads every kept copy, refusing a
   candidate that any copy's root id, state root or predecessor names (test
   `r309`; controls NC-R3-SUCC-CLEAN-UID, NC-R3-SUCC-CLEAN-COPY-CONTENT). A
   copy that cannot be read or parsed refuses every candidate.
2. **Residual: the window before P-REVOKE's `rename`.** The target's absence
   is read at construction and at the gate. Between the gate and the
   `rename`, only a root process acting outside every procedure could create
   it, and design section 13.1 makes that unsupported. The operation stays
   design section 15.3's `rename`; a no-replace rename would change the
   documented operation.
3. **Residual: the per-directory over-approximation can lose a revoked
   file.** A revocation crashed between its syncs can keep the removal from
   `dispositions/` and lose the addition to `revoked/` (2 of P-REVOKE's 30
   crash states, `r3x4`). The incident blocks, and R-REVOKE refuses: nothing
   is left to complete. Under A-M1 this state does not occur.
4. **Residual: a successor root that is not all zero stays.** R-SUCCESSOR
   refuses it, since it may hold history. The succession can be repeated at
   another root id.
5. **By design: an all-zero root after an interrupted provisioning.**
   Removing it is the Owner's own, optional step (`r5-15.2-recovery-matrix.md`).
6. **A modelling limit of the simulator.** A directory counts as reachable
   while any directory's visible entries name it, including an orphaned
   one's. No procedure creates an entry in an orphan through an old handle,
   so no result depends on this.

## Boundaries A to M (mission section 3)

| Boundary | Status | Evidence |
|---|---|---|
| `StoreOwner` retains custody, recorder, worker, header/journal and `StoreGuard` | Unchanged (`owner.rs` byte-identical) | `scope/`; `b01` to `b09`; API probes |
| Only `StoreOwner::close` authorizes a seal; a failed close returns the owner | Unchanged | `b01`, `b02`; P-CLOSE-TWICE |
| `Opened`'s state stays private; `Opened::claim` consumes its own state | Unchanged (`open.rs` unchanged) | API probes |
| `StoreValidator` comes only from an opening | Unchanged (`disposition.rs` byte-identical) | `b06`; API probes |
| Exchange, Recorder and Worker stay internal | Unchanged (`exchange.rs`, `recorder.rs` byte-identical) | API probes |
| The real configured store is `Err(DeploymentNotAuthorized)` with an uninhabited success type | Unchanged | `o01` (R327) |
| The session's `Verification` is a private one-shot | Unchanged; the recoveries' steps lapse it, and their verify-after clears it | `r312`; P-VERIFICATION-* |
| `Assessment` is data only | Unchanged | P-TAKE-ASSESSMENT |
| Successor authorization requirements | Unchanged; R3 adds the kept-copy and new-root checks before the gate, and rechecks them at it | `v01` to `v24`, `r301` to `r314` |
| Capacity never becomes permission by itself | Kept: capacity refuses a normal revocation and never permits a recovery | `r315`, `r316`, `r321` |
| Decisions never use `BoundedReport.detail` as authority | Kept | Section 18 searches above |
| `owner.rs`, `exchange.rs`, `recorder.rs`, `disposition.rs`, `faults.rs` unchanged | Byte-identical | `scope/` |

# NEXUS OS — PROPOSED CONTRACT: D2A-1, THE LEDGER'S PROTOCOL CORE

**Reference:** P3-C11-D2A1-CONTRACT-02-R1
**From:** Claude — Lead Architect
**To:** ChatGPT — First Architect / Independent Auditor
**Via:** Suresh — Owner
**Date:** 2026-10-11
**Status:** PROPOSED IMPLEMENTATION BASELINE. Its source, `P3-C11-D2A1-CONTRACT-02.md`, was ACCEPTED AT CONTRACT LEVEL by the First Architect, FOR THE DORMANT PROTOCOL SLICE ONLY (P3-C11-R3-AMENDMENT-01-RESPONSE-02-REVIEW-02, "the fourth review", 2026-10-11). **This corrected file has not yet been seen or hashed by him.** IMPLEMENTATION NOT AUTHORIZED: the Owner's decision on scope, branch and mission is pending. No interim label is awarded.

**What this file is.** `P3-C11-D2A1-CONTRACT-02.md` (sha256 `5a27062b…b465`) with exactly these differences, and no other:

1. the reference line, and this status line with the notes down to the line "Replaces";
2. one sentence of property P8 in section 7.3, corrected as the fourth review directs under PC2-N1;
3. the first row of section 14, which now gives the state of the acceptance.

It is proposed as the text to implement from. CONTRACT-02 stays the reviewed record. The title keeps its word PROPOSED for that reason.

**Erratum PC2-N1** (section 7.3, property P8). CONTRACT-02 said a later attempt of the same record can be answered Committed "only after this attempt was resolved as absent". That left out an attempt that ended as a definite Not written, with no resolve at all. The sentence now follows the review's reading, with this contract's spelling "Not written". The review's words "qualified resolution as absent" mean, in this contract's terms, the row "resolved as absent" of section 4.5. The property and the table of 4.5 are unchanged.

**Read with the acceptance, not instead of it.** The acceptance is section 4 of the fourth review, and its conditions bind any mission. They are listed here, in the review's words where those are short. **If this list and the review differ, the review governs.**

1. **Scope.** "CORE §3.2's six files and its non-activation restrictions." "No SQLite, shell, B2/B3, SCH or RS implementation is admitted into this slice." The dormant-module allowance, the scope-qualified interim label and the investigation ceilings stay as ruled before.
2. **The declarations of Appendix A are uncompiled.** "The authorized implementer, if any, must stop on a material signature/scope mismatch rather than redesigning the interface silently."
3. **Nothing has passed.** The tests, the selective controls and the stop-on-failure rules "remain essential". The seven properties without their own controls "remain disclosed assurance limitations".
4. **Same ledger.** "The implementation must compare allocation identity, such as Arc::ptr_eq, not equality of contents or of the published word. Keep the allocation fixed for the ledger's lifetime and do not create independent mutable cores sharing one identity/word." T-BIND is required, "including an old worker/token after its core is dropped". Cell occupancy exhaustion "remains a separate checked boundary". **The second quoted sentence has no equivalent in the body below; it binds through the review.**
5. **`InCall`, and what a recorded exit means.** "InCall must not survive as a normal externally observable U state; the stated latch covers an invalid occurrence." W13 "must demonstrate the two histories against the actual implementation". "This accepts preservation of the missing discriminator, not the complete family/stage correction wording." The full contract "must not infer completion of a caller-side cleanup path merely from a recorded exit".
6. **Re-offers** "remain table-tested, not covered by the listed unique-gating-caller explorations". "A test of every re-offer row is still owed. Neither this approval nor R4 establishes all concurrent re-offer histories, and no exactly-once external-execution guarantee follows."
7. **A corrected record.** "Once the verified correction exists, return the specified Corrected refusal before the ordinary Already present path." "Do not rewrite or hide the original durable row, call it absent, or change what its earlier append reported. Ordinary evidence lookup can still establish that it exists. A later action needs its own ordinary authority and a new record; correction settlement does not execute or revive the old action." "The later shell must obtain `corrected` from a verified correction bound to the exact original identity and digest, not an unchecked flag or an unrelated record with the same identity. T-GATE is required."
8. **Evidence.** "Source review must verify those premises in the real core and explorer. A finite search graph is not wait-freedom. Bounds, fault counts and omitted histories must remain visible; incomplete exploration is not PASS. Large runs remain serialized and conditional on host headroom, without stopping local-model workloads to create room."

**Three statements of the reviewed text, read in that light.** Statements below that say PROPOSED or AUDIT PENDING as a status, or that put questions to the First Architect, are the text as he reviewed it; his answers are in the fourth review. "This file is complete for D2A-1" is said of the design rules; the eight conditions above are conditions on the implementation and its evidence, added by the acceptance. The line "Replaces" is CONTRACT-02's own.

**Replaces:** `P3-C11-D2A1-CONTRACT-01.md` (sha256 `795a9203…8eff`), ruled AMEND.
**Answers:** P3-C11-R3-AMENDMENT-01-RESPONSE-01-REVIEW-01 ("the second review"): findings PC-01 to PC-04 and its six rulings on this contract. Section 0A lists every change. It still answers the first review, P3-C11-R3-DESIGN-03-AMENDMENT-01-REVIEW-01: findings AM-01 to AM-06, CT-01, CT-03, CT-04, CT-05 and rulings B/Q-2 to B/Q-4, as far as they bear on a pure protocol core.
**Source checkpoint:** `862a11cef284c5e3c34234dfd2677a5462c86d5d`, tree `83a234b6ea7ff446b739e263b45cd9b69f08d5e7`.

**This file is complete for D2A-1.** Every rule the core must obey is written here. Where this file and `P3-C11-R3-DESIGN-03-AMENDMENT-01.md` ("the amendment") differ, this file replaces the amendment for D2A-1; section 2 lists the differences. Nothing in the amendment is needed to implement D2A-1, and nothing in it is carried in by reference.

It creates no branch, changes no pin, adds no dependency and issues no mission.

`gc/` is `crates/nexus-governed-control/src/`. `P3` is `app/src-tauri/src/phase3_tests.rs`. "The scratch model" is the executable model run outside the repository (revisions r0 to r4). "The layer above" is the obligation table, settlement, the worker loop and admission, which B2 will build.

---

## 0. Weaknesses first

1. **This is a design on paper.** No line of it exists in the repository. Nothing was built or run against Nexus source.
2. **A checking pass on my side found six HIGH defects in this contract's first version,** after I had written it. One actor of my own test plan broke a property on the unchanged design. Three controls could not have been killed. One rule contradicted two others. And the file called itself complete while leaning on amendment text. All were corrected in CONTRACT-01. **A second, narrower pass on those corrections then found four more:** a control that the design's own consistency check hides; a boundary test that the test hook, as I had defined it, could not run; a rule for a full counter that contradicted the rule that a store operation decides nothing from the word; and two tables that disagreed about a sealed session. Those were corrected too. **This version's changes had one checking pass.** It found three more HIGH defects across the three files of this response, two of them here: P8 as I first restated it was still false when a record is offered again after an absent result; and a record that carries a correction could be answered Committed, against the standing rule that a correction never revives its step. Both are corrected. Weigh the rest accordingly.
3. **Appendix A in this version has not been type-checked by anyone.** The previous version's declarations were checked as an empty skeleton on rustc 1.97.0, in a scratch directory, by me and again by a checker on my side, without authorisation. The second review does not authorise that after the fact, and I have not repeated it. So the declarations changed in this version — every bound value now holds a reference instead of a number; a gating occupant and a correction carry an exit — were edited by hand. An error in them is possible and would surface in the first authorised mission, which stops and reports (section 12).
4. **This contract changes the design the first review accepted in direction.** Section 2 lists fourteen differences. The second review accepts eleven and amends three (7, 11 and 14); those amendments are made here. None has been exercised by any model run.
5. **I do not know how many states the in-crate exploration will have.** Its faults are finer than the scratch model's. A reviewed configuration may not finish inside the caps of 8.3. If so the result is INCOMPLETE and the contract comes back to me.
6. **The test plays the layer above.** B2 must prove its own half again with its own code.
7. **D2A-1 says nothing about storage.** Every fact the core is given is invented by the test. Two storage assumptions are stated in 4.1 and discharged nowhere.
8. **The refinement argument of section 6 is a conditional safety argument, and prose.** The second review accepts it as that. It proves nothing about progress in a real execution (6.3).
9. **Not in D2A-1:** the records that open and close a session, bootstrap, cells G and S, statements, checkpoint records, the frontier read at open. A sealed session is specified only as a refusal table and is not explored.
10. **I assigned each control to the configuration I expect to kill it.** If I am wrong about one, the mission stops there. **Seven properties have no control at all** (7.5); a property with no control is weaker evidence.
11. **D2A-1 edits the guard file.** Fourteen recorded hashes in the mutation-control manifest go stale, and one more for `gc/lib.rs`.

---

## 0A. What changed from CONTRACT-01

| Second review | Change | Where |
|---|---|---|
| **PC-01** HIGH | **The ledger-number counter is removed.** A worker, a cell token and a snapshot each hold a reference to their ledger's own shared state, as a ticket already did. "Same ledger" means the same allocation. While any such value survives, its allocation survives, so no other ledger can compare equal to it. No ledger number exists to run out, so `Core::new` needs no failure. Rule R10 is replaced and the guard's allowance for `fetch_add` is removed. (A cell's occupancy number can still reach its end; 4.3 latches there and T-LIMIT tests it.) | 4.3, 4.7, 4.8, 6.1, 9.1, T-BIND, Appendix A |
| **PC-02** MEDIUM | **P8 is restated per attempt.** It speaks only of a refused attempt that issued an insert. It stops applying once a later attempt of the same record is answered Committed. A record offered again has its own table. **A record that carries a correction is never answered Committed** (new: the fact `corrected`, the reason *corrected*). One test added; the exploration's input histories are stated. | 4.1, 4.5, 7.2, 7.3, T-GATE, Appendix A |
| **PC-03** MEDIUM | **U keeps how the attempt ended:** it returned Uncertain, or its verb unwound. The correction carries that. The complete stage-specific content of a correction is not claimed here. | 4.5, 4.7, W13, section 13, Appendix A |
| **PC-04** MEDIUM | L1 is stated as a property of the explored graph only, with the assumptions a real execution would need. Control A2 is specified step by step. The explorer's state key and its copying of states are specified. | 6.3, 7.1, 7.5 |
| Ruling 4 | The dead-code lint shows that the compiler sees a use, not that a test ran it. Wording corrected. | 3.2 |
| Ruling 5 | The scoped label is accepted on its conditions. Recorded. | section 11 |
| Ruling 6 | The caps are investigation ceilings. One full exploration at a time; the launcher checks free memory first and never stops another workload to make room. | 8.3 |
| Review §6, RF-01 | The forward statement about sealing is corrected: sealing comes before the close record and does not depend on it. | 4.9, section 13 |
| Ruling 3 | The hook makes no store publication and does not re-enter the core. Recorded. | 6.4 |

Sections 0, 2, 5, 15 and 16 are updated to match. Rulings that accepted a rule without change are not listed.

---

## 1. Objective, and what it is not

**Objective (one).** Add the ledger's protocol core to the control crate as a module that nothing calls: its state, its rules, the tickets and the published word, with a test that explores it exhaustively within stated bounds.

**It must not:**

- touch SQLite, a file, the network, a clock, a random source, a thread or the environment;
- add a dependency or change any `Cargo.toml` or `Cargo.lock`;
- be named by any production source outside itself and its one `mod` line;
- change any authority, control, broker, display or desktop source file;
- implement any part of the storage shell, D2b, B1, B2, B3, B4, SCH or RS-01;
- be described as "D2a" or "the storage core". It is the protocol core.

---

## 2. Differences from the amendment

Left: the amendment's rule. Right: the rule here.

| # | Amendment | Here | Finding |
|---|---|---|---|
| 1 | The shell performs the storage steps and asks the core to classify. | **The core runs each operation and calls the shell for single storage verbs** (4.1). | CT-03, AM-01 |
| 2 | A health word, a count beside it. | **One word holds the flags, the count, and the number of tickets alive** (4.2). | AM-04 |
| 3 | A ticket's kind is Not committed, Uncertain or Conflict. | **Two independent answers:** whether anything can have been written; and which member, if any, is a conflict (4.3). | AM-01 |
| 4 | An unclassified constraint failure returns Not committed and latches. | It latches, **and reports Not written or Uncertain by what the rollback said** (4.4). | AM-01 |
| 5 | An unwind inside the store counts the ticket Uncertain. | **Not written if no insert had been issued; Uncertain after.** A conflict already found is kept (4.4). | AM-01 |
| 6 | Two causes for conflicts inside one batch. | **A batch that conflicts with itself is malformed** and is refused before any transaction. It names no conflict and tables nothing (4.4). | AM-01 |
| 7 | A ticket is counted in place; nothing checks whose it is. | **A ticket of another ledger, or one not Armed, is refused and latches the store** (4.3). "Another ledger" is decided by the allocation the ticket refers to, not by a number. | AM-03, PC-01 |
| 8 | "At most the slot count" tickets. | **A limit on tickets alive, enforced by the ledger** (4.3). | AM-03 |
| 9 | A count that would overflow latches. | Any count step that would overflow or underflow sets *handover fault* and leaves the count (4.3). It cannot be reached inside the limit. | AM-04 |
| 10 | A Counted ticket dropped unconfirmed "lowers nothing". | The same, **and it is recorded** as a sticky *handover fault*; the ticket keeps its place in the limit (4.3). | AM-06 |
| 11 | The resolver reads U's header, builds a correction outside the store, hands it in. Cells are emptied by name. | **The store builds the correction, from the record and from how its attempt ended, which U keeps. Every cell operation names the exact occupancy it was issued for** (4.7). | AM-03, PC-03 |
| 12 | A resolved acknowledgement is a return value the worker must apply before its next call. | **Its effect is a value the store keeps** (4.8). | AM-05 |
| 13 | Explain answers Refused when nothing is known. No rule for a lookup that finds different content. | Explain answers **Unknown**. **A lookup that finds different content is a conflict observation** and enters the refusal table (4.6). | — |
| 14 | Acknowledgement test 0 compares the store's identity; test 1 does not mention the session or U. | Test 0 checks that the snapshot refers to this ledger's own shared state. Test 1 adds: the session is open; U is empty (4.8). | AM-02, PC-01 |

**The amendment's fallback for its section 4.2 is withdrawn.** There is no implementer's choice.

---

## 3. Base, files, visibility

### 3.1 Base and branch

- Base: `862a11cef284c5e3c34234dfd2677a5462c86d5d`, re-verified against GitHub at the start of the mission.
- Branch: new and isolated. PROPOSED name `interim/p3-c11-ledger-core`. Not `repair/p3-candidate11`.
- Push and dispatch in Manual permission mode. Targeted local tests, then the Fast Local gate on the exact commit, started by dispatch (it starts on a push only for `implement/**` and `repair/**`; `.github/workflows/ci-fast-local.yml:28-33`). No hosted CI.

### 3.2 Files — the complete list

| # | File | Kind | Content |
|---|---|---|---|
| 1 | `gc/ledger.rs` | new, production | A stub: a comment and the two lines below. |
| 2 | `gc/ledger/protocol.rs` | new, production | The protocol core (sections 4 to 6; Appendix A) |
| 3 | `gc/ledger/protocol/model_tests.rs` | new, test only | The exhaustive exploration (section 7) |
| 4 | `gc/ledger/protocol/unit_tests.rs` | new, test only | Tables, boundary tests and named witnesses (7.6) |
| 5 | `gc/lib.rs` | edit | **one added line:** `pub(crate) mod ledger;` |
| 6 | `P3` | edit | **one new guard** (9.1). No existing guard is edited. |

**Nothing else.** Receipts are kept outside the repository (8.4).

**File 1's production text is exactly:**

    #[cfg_attr(not(test), allow(dead_code))]
    mod protocol;

**Why that attribute** (ruling B/Q-2; accepted by the second review, ruling 4). Nothing in the crate calls the core yet, and CI denies warnings. The attribute allows unused code **only in a build that is not a test build**. In a test build the lint stays on, so every item of the core must be **used** by test code, as the compiler sees use. **That is not the same as executed:** which branches a test actually ran is shown by the tests and the branch receipts of 8.4, not by the lint. The behaviour was seen on an unauthorised scratch skeleton under rustc 1.97.0, which is not the gate; the mission confirms it on the pinned 1.94.0 first, and stops if it differs. The attribute's place is pinned by the guard (9.1 item 2), and the mission that makes the core reachable from production removes or narrows it.

**Visibility.** `ledger` is crate-private. `protocol` is private to `ledger`. Every item of the core is `pub(super)` or private. **D2A-1 adds no item that any other module of the crate can name.** A later public face is a separate review at D2b.

**The two test files are children of `protocol`,** declared in file 2 with `#[cfg(test)]` on each `mod` line. They read the core's private state directly. The core needs no accessor or seam for them, with one exception (6.4).

There must be no `gc/ledger/mod.rs` and no `gc/ledger/protocol/mod.rs`.

---

## 4. The protocol

### 4.1 Core and shell

| Part of an operation | Written in |
|---|---|
| its entry rule; the order of its storage steps; the classification of every record; what is done after each failure; its result; every change to cells, phase, latch, table and frontier; its one publication | the core |
| one verb at a time | the shell, behind the traits of Appendix A |

The core never touches storage. A verb gets no say in what happens next. **A verb that this table calls a read writes nothing:** `facts`, `row`, `explained`, `acknowledgement`, `frontier`, `survey`, `recheck` and `present` change no row and no stored value. The persistence rules of 4.4 depend on that. **Each core operation takes the core by exclusive reference:** in the shell that comes from the store's one lock; in the tests it is ordinary ownership.

**The verbs.**

| Trait | Verb | Does |
|---|---|---|
| `Txn` | `open` | opens a write transaction |
| | `facts` | reads, for one record: the row with its identity, if any; whether its explanation is in the ledger; whether a correction for it is in the ledger; the row holding its lifecycle role for its commitment, if any; whether its commitment has a Prepared row |
| | `insert` | inserts one record |
| | `commit`, `rollback` | |
| `Reads` | `row`, `explained` | the same two reads, outside a transaction |
| | `acknowledgement` | the sequence of the acknowledgement row built from a given snapshot, if present |
| | `frontier` | the durable acknowledgement frontier |
| `AckTxn` | `survey` | one read for a snapshot (4.8) |
| | `open`, `recheck`, `insert`, `commit`, `rollback` | the acknowledgement's transaction |
| `CellTxn` | `open`, `present`, `insert`, `commit`, `rollback` | appends a cell's own occupant |
| `BarrierRun` | `run` | the durability barrier; true if it passed |

**What a verb's answer means.** The core's results depend on these.

| Answer | Means |
|---|---|
| `Commit::Durable` | COMMIT returned success |
| `Commit::Unknown` | anything else, once COMMIT was issued |
| `Rollback::Undone` | ROLLBACK returned success **and** the connection reports that no transaction is open |
| `Rollback::Unknown` | anything else |
| `Insert::Constraint` | the insert failed on a constraint |
| `Insert::Failed`, `Read::Failed` | any other failure |
| `Read::Corrupt` | the row was read and failed verification |

**A failed or corrupt read, in any operation.** Where an operation's own table has a row for it, that row applies. Where it has none: a failed read ends the operation with its not-applied, failed or busy answer, changes nothing else, and moves the phase to *barrier needed*. A corrupt read does the same **and latches the store.** In a fact append it is reported as reason *storage*. An `open` that answers corrupt is treated as failed.

**Two storage assumptions, stated and not discharged.** Both belong to the open durability reasoning R-a to R-e.

- **A-1.** A transaction whose COMMIT was never issued, and whose rollback was `Undone`, has persisted nothing.
- **A-2.** A transaction whose commit was `Durable` is durable.

### 4.2 The published word

One 64-bit atomic.

| Field | Bits | Written by |
|---|---|---|
| phase: Healthy, barrier needed, resolving | low 16 | a store operation's publication, from the core's own state |
| session: bootstrap, open, sealed | | the same |
| latched; U occupied; C occupied; X occupied | | the same |
| **handover fault**, sticky | | ticket accounting only: the lock-free ticket steps, and the count step of a publication if the count field were ever full (4.3) |
| the remaining low bits | | nobody. They are zero. |
| `live`: tickets alive | 24 | `arm` (+1); confirm and the drop of an Armed ticket (−1) |
| `unapplied`: results not yet taken over | 24 | a publication (+0 or +1); `expire` (+1); confirm (−1) |

**`admits`** is true only when the low 16 bits are exactly *Healthy, open, nothing else* and `unapplied` is zero.

**Two closures, kept apart.** *Latched* is the store's: set only inside a store operation, from the core's state. *Handover fault* is the ticket accounting's. Neither is ever cleared. No store operation reads the word to decide anything about its own state or its answer.

### 4.3 Tickets (findings AM-03, AM-06)

**A ticket is Armed, Counted or Settled.** It holds a reference to its ledger's word. It cannot be cloned or copied.

**Every lock-free ticket step is exactly one compare-and-exchange on the word:**

| Step | Effect |
|---|---|
| `arm` | if `live` is below the limit: `live` + 1, and an Armed ticket is returned. **If not: handover fault is set and nothing is returned.** |
| `confirm` of a Counted ticket | `unapplied` − 1 and `live` − 1, together. The ticket is consumed. |
| `confirm` of an Armed ticket; the drop of an Armed ticket | `live` − 1 |
| `expire` of an Armed ticket | `unapplied` + 1; the ticket becomes Counted as Not written. For the shell only: the wait to enter the store ran out. |
| `expire` of a ticket that is not Armed | handover fault; the call returns `Rejected` |
| **the drop of a Counted ticket** | **handover fault. `unapplied` and `live` are left as they are.** |
| the drop of a confirmed ticket | nothing; `confirm` already did it |

**The limit** is handed in when the core is created. A value outside 1 to 16,777,215 is brought into that range; so is a refusal-table limit below 1. **It bounds tickets alive at once: attempts in flight and handovers not yet confirmed. It does not count records.** B2 must set it to at least its first writers in flight plus its settlement attempts in flight, and state both numbers. `unapplied ≤ live ≤ limit` always, so no count can overflow inside the limit.

**Lending.** A fact append takes the ticket by exclusive reference and first checks that it belongs to this ledger and is Armed. **"Belongs to this ledger" means: the ticket refers to the very same shared state as the core does.** It is a comparison of two references to one allocation, not of two numbers. If not: nothing is counted, no transaction starts, **the store latches,** and the call returns `Rejected`. A Counted ticket cannot carry a second attempt. Confirming acts on the ticket's own ledger; the caller cannot name another.

**Counting inside a store operation** is part of that operation's one publication. The count rises there; the ticket's state is written next, in the same function, with nothing between that can unwind.

**A Counted ticket says two things:** *persistence* — Not written, or Uncertain — and, for a conflict, *which member*. A named member implies Not written.

**Arithmetic is checked.** Any step that would overflow or underflow a count sets handover fault and leaves that count as it is. That includes the +1 of a publication, where it is done inside the exchange and changes nothing in the core: the operation still returns its result and still marks the ticket Counted. It cannot happen inside the limit, because a lent ticket is itself alive and not yet counted. A cell's occupancy number that would overflow latches the store (cause: overflow); T-LIMIT forces it.

**What the layer above does with a Counted ticket.** B2's rule; the tests play it.

| Ticket | Class recorded, under the obligation-table lock, **before** `confirm` |
|---|---|
| Not written, no member named | every member Refused |
| Uncertain | every member Uncertain |
| Not written, member *k* named | member *k* Conflict; every other member Refused |

### 4.4 The fact append, and the precedence of its results (finding AM-01)

One call appends 1 to 256 fact records in one transaction. Steps, in order. The first step that ends the call decides the result.

| Step | What happens | Ends the call with |
|---|---|---|
| 0 | Ticket check (4.3). | `Rejected`; latched. Nothing counted. |
| 1 | Entry rule, tested in this order: the session is not bootstrap; the store is not latched; the phase is Healthy; **the batch is well formed.** | Not written; reason *entry* |
| 2 | Verb `open`. | on failure: Not written; reason *storage* |
| 3 | **Members are classified in order, before any insert.** For each: verb `facts`, then the table below. **Classification stops at the first member that is refused,** and one rollback is issued. | a failed read: Not written; *storage*. A corrupt read: latched; Not written; *storage*. The first member refused: Not written; that member's reason. **No insert is issued for any member.** |
| 4 | If no member needs inserting: one rollback of the empty transaction. | Committed |
| 5 | Inserts, in order. | a constraint failure: **latched**; reason *constraint*. Any other failure: reason *storage*. In both: **Not written if the rollback was `Undone`; Uncertain if not.** |
| 6 | Verb `commit`. | `Durable`: Committed. `Unknown`: **Uncertain**, whatever a rollback then reports. |

After the first failure a verb reports in an open transaction, and after a refusal in step 3, the core issues exactly one rollback. A rollback that fails is not tried again. Its answer can make the phase worse. It can never make the persistence better.

**A batch is well formed** when: it has 1 to 256 members; no two members have the same identity; no two have the same commitment and lifecycle role; and no member whose role needs a Prepared record stands before the Prepared member of its own commitment. **The roles Started and End need a Prepared record; Prepared does not.** This needs no storage. **A malformed batch is the caller's error.** It is refused whole, names no conflict and tables nothing. B2's rule after it: retry the members singly, in lifecycle order.

**Classification of one member in step 3,** in this order:

| # | Test | Answer |
|---|---|---|
| a | a row with this identity and an equal digest is in the ledger | already present; no insert |
| b | this original is in the refusal table; or X holds its explanation; or its explanation is in the ledger | **Conflict**, with the retained cause or the explanation's sequence. Nothing is evaluated again, and a retained cause is never overwritten. |
| c | the session is sealed | *session closed* |
| d | a constraint is broken, tested in this order: the identity is in the ledger with different content; the member's role for its commitment is already held by a row; the role needs a Prepared record, and neither the ledger nor an earlier member of the batch has one | **Conflict**, with a new cause. The original enters the refusal table. Its explanation is built into X if X is empty. **If the table is full: latched; reason *table full*; nothing is tabled or built into X; the member is still named as the conflict.** |
| e | otherwise | to be inserted |

**A cause** is a small fixed value built by the core from what `facts` returned. Three kinds: *same identity, different content* (the row's sequence and digest; the offered digest); *duplicate role* (the commitment, the role, the holding row's identity and sequence, the offered record); *missing Prepared* (the commitment, the role, the offered record). Every cause refers only to committed rows. Only the core can build one.

**An original** is a record's identity together with its digest. Two contents under one identity are two originals.

**The refusal table** is in memory, bounded by a limit handed in at creation, keyed by original. An entry is made only in step 3d and by lookup (4.6). **An entry is removed only in a step in which the store saw that original's explanation in the ledger:** X's own append committing or finding it present, or explain answering Explained.

**Persistence, in one place.**

| What happened | Persistence | Why |
|---|---|---|
| refused at entry; malformed; expired before entry | Not written | no transaction |
| refused in step 3, for any reason | Not written | no insert was issued. A rollback of a transaction that wrote nothing cannot have written anything, whatever it answers. |
| a failure in step 5, rollback `Undone` | Not written | assumption A-1 |
| a failure in step 5, rollback `Unknown` | **Uncertain** | |
| `commit` answered `Unknown` | **Uncertain** | a later rollback proves nothing about the disk |
| a verb unwound before the first insert was issued | Not written | as step 3 |
| a verb unwound once an insert had been issued | **Uncertain** | |

**On an unwind the core issues no further verb.** The transaction may be left open. The phase becomes *barrier needed*, and returning the connection to a known state is the barrier's first duty; that is a rule for the shell's contract.

**What this gives.**

- **A known conflict is never replaced by an ordinary failure.** A conflict is found only in step 3; the operation's guard keeps it if the call then unwinds; and a full table still names the member.
- **A latch never decides persistence.**
- **A conflict never refers to a row that might vanish.**
- **The review's counterexample** — a first insert, a later constraint failure, a failed rollback — ends as: latched; no member named; every member Uncertain.
- **No constraint may fail at COMMIT.** That holds only if the schema declares none that is deferred. The shell's contract must show it. D2A-1 treats every failed commit as storage.

**A refused append after the seal is an incident, not a normal answer.** D2A-1 only refuses and counts it. Reporting it belongs to the layer above.

**Phase, for every operation of this section.** A verb that reports a failure, an unwind, and any Uncertain result each move the phase to *barrier needed*. A refusal with no storage failure leaves the phase alone.

### 4.5 The gating append

One gating record, no ticket. A fact record and a gating record are different types (5.2), so one cannot be offered through the other's door.

Entry rule, in order: the session is open; not latched; Healthy; C and X are empty. **If U is occupied while the phase is Healthy, an invariant is broken: the store latches and refuses.** Then the record is placed in U, and steps 2 to 6 of 4.4 run for one member.

| Result | U | Other effect |
|---|---|---|
| Committed, or already present | emptied | |
| Conflict with a new cause | emptied | **the explanation is built into X in the same step; no refusal-table entry; no obligation exists** |
| Conflict already explained | emptied | nothing placed |
| any other Not written | emptied | |
| Uncertain | **kept**, marked *returned uncertain* | the phase is *barrier needed*; the worker resolves it (4.7) |
| a verb unwound once an insert had been issued | **kept**, marked *unwound* | the same |
| a verb unwound before any insert was issued | emptied | phase: barrier needed |

**U keeps how the attempt ended** (finding PC-03). A gating record is placed in U marked *in call*. When the call ends with the record kept, the operation's guard changes the mark to *returned uncertain* or to *unwound*; in every other ending the guard empties U. So outside a call U never holds *in call*; if resolve ever meets it, the store latches. The two kept rows leave the same record in U and the same row, or none, in the ledger. They differ in what the caller learned: one caller was told Uncertain; the other got no answer at all, because its call unwound. Nothing in the ledger can tell them apart later, so the mark is kept in U and passed to the correction. An acknowledgement in U carries no mark; it gets no correction.

**A record offered again** (finding PC-02).

| Earlier attempt | Offered again | Answer |
|---|---|---|
| committed, with no correction | while Healthy, cells empty | Committed again, with the row's sequence, as already present. Nothing is written; U is filled and emptied inside the call. |
| committed | while an entry rule refuses | the entry refusal. U is not touched. **No correction exists or is made:** the earlier attempt succeeded and was reported as such. |
| ended Uncertain or unwound, still in U | at any time | the entry refusal: the phase is not Healthy while U is occupied |
| resolved as present; its correction is in C | at any time | the entry refusal: C is occupied |
| resolved as present; its correction has committed | while Healthy, cells empty | **refused, Not written, reason *corrected*.** The correction says the step this record stood for was not reported as done. It stays so. A new step needs a new record. |
| resolved as absent | while Healthy, cells empty | a fresh attempt |
| ended Not written | while Healthy, cells empty | a fresh attempt |
| refused as a conflict | while Healthy, cells empty | Conflict again, as already explained, or with a cause built again from the same rows |

**A correction never revives its step.** That is the standing rule of the design this contract comes from, and the fifth row keeps it: classification of a gating record tests *a correction for it is in the ledger* before it tests *already present*. A fact record has no correction and is not affected.

### 4.6 Lookup and explain

**Lookup** of an original. Entry: the session is open; not latched; the phase is not *barrier needed*.

| Finding | Answer | Effect |
|---|---|---|
| no row | Absent | |
| a row with an equal digest | Present, with its sequence | |
| a row with a different digest | **Conflict** | If the original is already tabled, in X, or explained: the retained cause, or the explanation's sequence. Otherwise a new cause, entered in the table. **If the table is full: latched, and the lookup is refused.** Lookup never fills X: U may be occupied while it runs. |
| the read failed | Failed | phase: barrier needed |
| the read was corrupt | refused | latched |

**Explain** of an original. Refused when latched or when the session is not open.

| Finding, in this order | Answer |
|---|---|
| the phase is not Healthy | Busy |
| the read failed | Busy; phase: barrier needed |
| the explanation is in the ledger | Explained, with its sequence. The table entry is removed. |
| X holds this original's explanation | Pending |
| X holds another explanation | Busy |
| X is empty and the table has the original | the explanation is built into X from the retained cause; Pending |
| none of these | Unknown. It cannot arise unless the layer above is wrong. The obligation stays debt. |

### 4.7 Cells, exact occupancy, and the worker (finding AM-03)

D2A-1 has three cells: U, C and X. **Cells G and S, when the shell adds them, follow the same occupancy rule.**

**Every placement into a cell gives that cell a new occupancy number.** A cell's numbers only rise. A **cell token** refers to its ledger's shared state, and names the cell and one occupancy number. Tokens are read from the agenda (4.8) and can be cloned freely; a clone is only a name.

**Every cell operation checks, in this order:** the worker belongs to this ledger; the token belongs to this ledger and names the right cell and the present occupancy; the entry rule. "Belongs" is the same test as for a ticket (4.3): the same shared state. A wrong worker is refused as *other ledger*. A wrong token is **stale: nothing changes.** No operation empties a cell by name.

**The worker.** `take_worker` returns one worker value, once. It cannot be cloned. Every worker operation takes it by exclusive reference, so two cannot run at once. B2 must keep it where the unwinding of one iteration does not drop it. If it is dropped, no worker exists for the rest of the session and the store stays as it is.

| Worker operation | Entry | Results |
|---|---|---|
| barrier | the phase is *barrier needed*; otherwise *not needed* | passed: the phase becomes *resolving* if U is occupied, else Healthy. Failed: no change. |
| resolve, with U's token | open; not latched; the phase is *resolving* | below |
| append a cell's occupant, with C's or X's token | open; not latched; Healthy | below |
| empty X, with X's token | X is marked committed | emptied. If X is not yet committed: *not committed*, and no change. |

**Resolve.** The store does the lookup itself. Nothing is handed in.

| U holds | Found | Effect |
|---|---|---|
| a gating record | absent | U emptied; Healthy |
| a gating record | present, equal digest | **the store builds the correction** from U's own record, the sequence found, and U's mark of how the attempt ended; places it in C; empties U; Healthy |
| a gating record | present, different digest | the explanation is built into X; U emptied; Healthy |
| an acknowledgement | absent | U emptied; Healthy |
| an acknowledgement | present, and the durable frontier is at least its N | the qualified frontier rises to N (4.8); U emptied; Healthy |
| an acknowledgement | present, and the durable frontier is below its N | storage contradicts itself: **latched; U kept; the phase stays *resolving***; answer *inconsistent* |
| either | the read failed | U kept; phase: barrier needed |
| either | the read was corrupt | latched; U kept; the call is refused |

C and X are empty whenever U is occupied (property P4), so resolve always has room. If it ever does not: latched; U kept; the phase stays *resolving*; the call is refused.

A correction's identity is derived from the record it is about and from how the attempt ended, and carries no clock, so the store can build it alone. **In D2A-1 a correction is three values and nothing more: the record, the sequence found, and how the attempt ended.** What a correction must say for each family and each stage of a commitment, and its fixed encoding, belong to the full contract. D2A-1 does not claim to settle them; it makes sure the one fact that cannot be recovered later is not thrown away.

**Appending a cell's occupant.** The verb `present` first.

| Cell | Outcome | Effect |
|---|---|---|
| C | present, or inserted and committed | **C emptied** |
| X | present, or inserted and committed | **X marked committed with its sequence;** the original's table entry removed |
| X | already marked committed | the same sequence again; no verb is issued |
| either | Not written | the cell is unchanged; phase as 4.4 |
| either | Uncertain | the cell is unchanged; phase: barrier needed |
| either | a constraint failure | latched; Not written or Uncertain by the rollback, as 4.4 |

**After a worker operation unwinds:** its guard has published a state at least as closed as the truth. The worker reads the agenda again and continues. Every step can be repeated: a repeated barrier answers *not needed* or runs again; a repeated cell append finds its row present; a repeated empty is stale.

**H is raised before X is emptied.** The store cannot check that; H belongs to the layer above. The store's part is that X cannot be emptied before it is marked committed.

### 4.8 Acknowledgement, and the qualified frontier (finding AM-05)

**The snapshot.** `snapshot` is one operation. Entry: open; not latched; Healthy. It calls `survey` once, which returns:

| Field | Meaning |
|---|---|
| frontier | the durable acknowledgement frontier, A_s |
| head | the committed head sequence |
| top | the highest sequence, at or below the head, of a record that requires acknowledgement: N |
| count, digest | of the identities of the acknowledgement-requiring records above A_s and up to N |

If `survey` fails, the answer is *failed* and the phase is *barrier needed*. If N is not above A_s, or the count is zero, the answer is *nothing to acknowledge*. Otherwise the answer is a snapshot holding those five values, a nonce, and a reference to this ledger's shared state. **The nonce comes from the shell's random source; the layer above supplies nothing.** A snapshot cannot be cloned and is consumed by its append, whatever the append answers.

**The append.** Six tests, in this order.

| # | When | Test | Refusal |
|---|---|---|---|
| 0 | before anything | the snapshot refers to this ledger's own shared state | other store |
| 1 | before anything | open; not latched; Healthy; C and X empty. (U occupied while Healthy: latched, as 4.5.) | not now |
| | | *The acknowledgement is placed in U. Verb `open`. Verb `recheck`, which reads again: the durable frontier now; the head now; and the count and digest over the snapshot's own range.* | |
| 2 | in the transaction | the snapshot's frontier equals the durable frontier now | stale |
| 3 | | N is above the snapshot's frontier, and its count is at least one | empty |
| 4 | | N is at most the snapshot's head, and that head is at most the head now | future |
| 5 | | the count and digest read now equal the snapshot's | set |

Tests 0 and 1 change nothing. A refusal by tests 2 to 5 comes before any insert: one rollback, U emptied, and the named refusal. After them the steps of 4.4 apply: Committed, with U emptied; Not written, with U emptied; Uncertain, with U kept. Comparisons are on unsigned 64-bit values, and no arithmetic is done on a sequence.

**The qualified frontier** is a number the core keeps: the highest N of an acknowledgement that the store **knows** to be durable. It rises in exactly two places: an acknowledgement's append answering Committed, and resolve finding an acknowledgement present with the durable frontier at least its N. It never falls.

**The durable frontier** is the N of the last acknowledgement row in the ledger, or zero. In D2A-1 the records that require acknowledgement are the explanations.

**The agenda** is one read of the core's own state, by shared reference: phase, session, the latch and its cause, the tokens of U, C and X, whether X is committed and with which sequence, and the qualified frontier. It does not read the word.

**Rule for the layer above:** *its in-memory frontier A is the larger of A and the agenda's frontier,* applied under its own lock whenever it reads an agenda. A resolved acknowledgement therefore cannot be lost by an overwritten slot, which is the case the scratch model's fourth revision found, or by a worker that unwinds after U was emptied. It is still in the store.

This does not show that the Owner was told anything. That is B3's contract.

### 4.9 Session state

Three states. Two transitions, each a store operation with one publication:

| Transition | Allowed when | Otherwise |
|---|---|---|
| *opened* | bootstrap | false; nothing changes |
| *sealed* | open; Healthy; not latched; U, C and X empty | false; nothing changes |

**When sealed, every operation of this section is refused,** except: `view`, the agenda, `arm`, `confirm`, `expire`, the barrier, and a fact append, which can still answer rows a to c of 4.4 and writes nothing. (A sealed store has empty cells, so any token offered to *empty X* is stale.)

**In D2A-1 these two transitions are bare calls,** reached only by tests. Sealed is checked by table (T-MATRIX) and is not explored.

**What the full contract must do with them, corrected by the second review (RF-01).** The two are not alike.

- **Opening** needs its record first: a session is open only once its opening record is confirmed durable.
- **Sealing comes first and does not depend on any record.** (The bare transition's own precondition above — Healthy, not latched, cells empty — is part of what terminal quiescence already means; whether a store that is not Healthy can be sealed at all is the full contract's question, not settled here.) Three separate facts: *sealed* — no producer may register or append again, and this is never undone; *close attempted* — the close record's append was issued; *close confirmed* — that record is known durable. An uncertain or failed close leaves the session sealed. A later finding that the close record is absent does not unseal it.

---

## 5. Which operation may do what (finding AM-02)

### 5.1 The matrix for the whole ledger

The last column says which rows D2A-1 implements. The others are obligations on the full contract.

| Entry point | Caller | What the caller can supply | What is written | Session | D2A-1 |
|---|---|---|---|---|---|
| fact append | a first writer; settlement | 1 to 256 fact records; an Armed ticket of this ledger | fact rows only | open; sealed for rows a to c only | yes |
| gating append | an ordering point | one gating record | a gating row only | open | yes |
| snapshot | the acknowledgement handler | nothing | nothing | open | yes |
| acknowledgement | the same | **a snapshot, consumed. No record.** | an acknowledgement row **built by the ledger from the snapshot** | open | yes |
| lookup | settlement | an original | nothing | open | yes |
| explain | settlement | an original | nothing in that call; the explanation is **built by the ledger from the cause it kept** | open | yes |
| barrier, resolve, cell append, empty | the one worker | a token | the cell's own occupant, **built by the ledger** | 4.7 | yes |
| gap | the loss accumulator | typed counts | a gap row built by the ledger, through cell G | open | no |
| statements | reconciliation | statements about earlier sessions; a typed checkpoint input | statement and checkpoint rows only | open | no |
| open the session | the ledger's owner | **nothing** | the opening record **about the ledger's own session**; open only once it is confirmed | bootstrap to open | bare transition only |
| seal the session; then attempt its close | the ledger's owner | **nothing** | sealing writes nothing. The close record, about the ledger's own session, is a separate attempt afterwards (4.9). | open to sealed | bare transition only |
| creation, continuity, completion | **no entry point.** Written inside *open* by the ledger's own bootstrap. | — | — | bootstrap | no |

### 5.2 How it is enforced

1. **No entry point takes a family.** Each writes one fixed family or builds its record itself. There is no general "append an event".
2. **Fact records and gating records are two types.** `append_facts` takes one, `append_gating` the other. Offering the wrong one does not compile. In D2A-1 each is built from the same plain description by its own constructor; in the full contract each is built only by the ledger's builder for that family, from a typed record. The wrong-family test is therefore a property of the types here, and a test of the builders there.
3. **No entry point takes a session.**
4. **Everything that carries authority is bound to one ledger and checked:** a ticket (the store latches); a snapshot (*other store*); a cell token (*stale*); the worker (*other ledger*). Each holds a reference to its ledger's shared state, and the check is that it is the same state. Nothing is rebound, and no ledger number exists to run out or repeat.
5. **Session and health are checked dispatch,** made by the core before any transaction or cell change (5.3).

### 5.3 Entry rules of the D2A-1 operations

Tests are made left to right. The first that fails names the refusal.

| Operation | Bound value | Session | Latch | Phase | Cells |
|---|---|---|---|---|---|
| fact append | ticket: this ledger, Armed | open, sealed | not latched | Healthy | any |
| gating append | — | open | not latched | Healthy | C, X empty; U empty |
| lookup | — | open | not latched | not *barrier needed* | any |
| explain | — | open | not latched | Healthy, else Busy | 4.6 |
| snapshot | — | open | not latched | Healthy | any |
| acknowledgement | snapshot: this ledger | open | not latched | Healthy | C, X empty; U empty |
| barrier | worker | any | any | *barrier needed* | any |
| resolve | worker; U's token | open | not latched | *resolving* | — |
| cell append | worker; token | open | not latched | Healthy | — |
| empty X | worker; token | any | any | any | X committed |
| agenda; `view`; `arm`; `confirm`; `expire`; `take_worker` | — | any | any | any | any |
| *opened*; *sealed* | — | 4.9 | 4.9 | 4.9 | 4.9 |

Test T-MATRIX checks every operation in every session state, latch state and phase against this table, and each wrong-ledger case.

**Answers whose variant might be guessed wrongly.**

| Case | Answer in Appendix A |
|---|---|
| a gating append or an acknowledgement finds U occupied while Healthy | latched (cause: cell invariant). Gating: `Refused { NotWritten, Entry(Latched) }`. Acknowledgement: `Refused(NotNow(Latched))`. |
| lookup: the table is full, or the read is corrupt | `Lookup::Refused(Latched)` |
| resolve: the read failed | `Resolve::Retained` |
| resolve: the read is corrupt, or there is no room | `Resolve::Refused(Latched)` |
| resolve: present, frontier below N | `Resolve::Inconsistent` |
| a malformed batch | `NotApplied`, the ticket Counted as Not written, reason `Entry(Malformed)` |
| a gating record that carries a correction, offered again | `Refused { NotWritten, Corrected { seq } }`, with the correction's sequence |
| the sequence in `AckOutcome::Committed` and `Resolve::Acknowledged` | N |
| the sequence in `GatingOutcome::Committed` and `CellAppend::Committed` | the row's sequence |

---

## 6. Why a store operation may be treated as one step (finding AM-04)

### 6.1 Rules

| # | Rule |
|---|---|
| R1 | **The word is touched in exactly two private functions:** one that loads it, used only by `view`; one that runs a compare-and-exchange loop. Both use sequentially consistent ordering, for success and for failure. Nothing else in the file names the word. **The loop needs no load:** it starts from a guess — for a store operation, the value the core last published — and takes the present value from a failed exchange. |
| R2 | **A store operation publishes at most once,** by one successful exchange, after every change to the core's state is final. The new value takes its phase, session, latch and cell bits from the core, **carries the handover-fault bit and `live` through unchanged,** and adds 0 or 1 to `unapplied`. |
| R3 | **A store operation never loads the word, and nothing it decides or returns depends on the word.** It sees the old value only inside its exchange, to carry those fields through and to do the checked +1 of 4.3. The agenda reads the core, not the word. |
| R4 | **Every lock-free step is exactly one successful exchange** (4.3), and changes only `live`, `unapplied` and the handover-fault bit. |
| R5 | **Admission makes exactly one load** (`view`) while it holds the obligation-table lock, and takes every store-side condition from that value. |
| R6 | **The core's state is reached only through an exclusive reference,** or a shared one for the agenda. **A ticket's state is reached only by its owner or the operation it is lent to.** |
| R7 | **The publication is made by a guard, and the guard is the first thing every store operation creates,** before its ticket check and its entry rule. The guard's drop publishes, on return and on unwind alike. On unwind it first sets the phase to *barrier needed* and the persistence by 4.4. |
| R8 | **The core's production text is written so that nothing in it can unwind except a call to a storage verb.** In particular nothing may unwind inside an exchange loop, between a publication and the write of the lent ticket's state, inside `confirm`, `expire` or a drop, or inside the guard's own drop. Those regions hold only atomic operations, checked integer arithmetic and moves of plain values. |
| R9 | **A broken invariant latches.** It never panics. |
| R10 | **The word is the only atomic in the file.** There is no counter of ledgers. Whether two values belong to one ledger is decided by whether they refer to the same shared state. |

### 6.2 The argument

*Give every step one instant.* A load, and a successful exchange, each take effect at one instant, and the atomic's history is consistent with those instants. Give `view` and every lock-free ticket step the instant of its one operation. Give a store operation the instant of its publication, or any instant in its duration if it publishes nothing. Give an obligation-table critical section the instant of the `view` it contains, or any instant in its duration if it contains none.

*Order all steps by their instants.* That order agrees with each thread's own order, with the order of store operations (R6: their durations cannot overlap), and with the order of obligation-table sections (their lock).

*Each step, replayed alone in that order, reads what it read in the real run:*

- the word: R1;
- the core, the cells, the table, the frontier and the ledger's content: only store operations touch them (R6), and they are in order;
- a ticket: only its owner and the operation it is lent to (R6). The count rises at the publication and the ticket is marked just after; nobody can look at the ticket in between (R6) and nothing can unwind in between (R8);
- obligation-table state: only under its lock;
- **and a store operation reads nothing from the word (R3),** so a lock-free step that lands during it cannot change what it does.

So every real run corresponds to a run in which each store operation, each lock-free ticket step and each admission is one whole step. **The exploration takes runs of that kind, within its bounds. It does not take all of them.**

**The interval the review named,** between the count changing and the ticket changing: nobody can observe it, and nothing can unwind in it. The order "count first" is therefore a rule for the reader, not something a test can tell apart.

**The interval between two reads at admission:** there is one read.

**What the argument allows, said plainly.** A transaction can be durable for a moment before its publication. An admission whose load falls in that moment is ordered before the append. That is sound: admission reads nothing the transaction changed, and no result exists for anyone until it is published.

### 6.3 What the argument does not cover

- **Progress.** The argument is about safety: what a real run may have read. It leaves out failed exchanges and stretches in which a thread does nothing. **L1 is a property of the explored graph and of nothing else.** For a real execution to finish, more must hold, and none of it is shown here: each exchange loop eventually succeeds, which is not certain if other threads keep changing the word for ever; every storage verb returns; the scheduler runs every thread; the barrier eventually passes once faults stop. No bound on time, and no claim that any thread always makes progress, follows from anything in this contract.
- The storage verbs, and assumptions A-1 and A-2.
- The layer above. "Record the class under the lock, then confirm" is B2's code; here the tests play it.
- A crash. A process that dies between a commit and its publication is reconciliation's subject.

### 6.4 Evidence for the rules

| Rule | Evidence |
|---|---|
| R1 | the guard, by text: the load and the exchange each occur once in the file, with one ordering (9.1 items 5 and 6); and E2 |
| R3 | E2, on every explored step |
| R2, R4 | **a test-only hook.** In a test build, the two functions of R1 report each load and each successful exchange to the test. Property E2 checks every explored step. |
| R5 | the test's admission is written with one `view`; control A2 uses two |
| R6 | the types; T-OWN |
| R7 | every verb is made to unwind, in exploration and in T-PB5; control C27 |
| R8, R9 | lints denied for the whole file (9.2) and the guard (9.1 item 4) catch a slice index, unchecked arithmetic, `unwrap`, `expect` and the panicking macros. **They do not catch everything:** an index into a map, a shift by a variable, and a few slice methods pass them. For those, and for the named regions, R8 rests on review. |
| R10 | the guard (9.1 item 6); T-BIND |

**The hook is the one test-only item in production text** (accepted by the second review, ruling 3, as bounded test instrumentation). It is under `#[cfg(test)]` and exists in no other build. It makes no store publication, and it never calls an operation that needs the core while the core is borrowed. (A boundary test may run another ticket's lock-free step from it; that is the point of T-PB4.) A failure inside the hook is a failed test, not a passed control. It is a call at three points: after the load; after a failed exchange, before the loop tries again; and after a successful exchange. It may use test-only state that the production text may not. In a test build it is test code running inside regions that R8 keeps free of calls in production, so the tests cannot be evidence for R8 itself.

---

## 7. The exploration

### 7.1 What it is

`model_tests.rs` explores the real core. It simulates the ledger's content, implements every verb over that simulation, and plays every caller. It visits every interleaving of whole steps, breadth first, with a visited set and a parent table, so that a violation has a shortest trace.

**Faults.** Each verb can answer in each failing way its type allows, and can unwind, within a fault budget. More than one fault can fall in one operation. The faults include: an insert answering `Constraint` although classification passed; a wait that expires before entry; a worker operation's verb unwinding.

**What the simulated ledger then holds:** after `Commit::Durable`, the records. After `Rollback::Undone` with no commit issued, nothing (A-1). After `Commit::Unknown`; after `Rollback::Unknown` once an insert was issued; and after a verb unwinds once an insert was issued: the records are present or absent, and both are explored. The simulation's durable frontier is the N of the last acknowledgement row it holds.

**The explorer's states** (finding PC-04).

- **A state is a whole world, and each world has its own word.** When the explorer copies a state to try another step, it builds a new shared state holding the same word value, and makes everything in the copy refer to the new one: the core, the health handle through which `view` and `arm` go, every ticket, the worker, every token wherever it is held — by the worker, by REPLAY, inside an agenda or a result not yet applied — and every snapshot. **Two branches of the search never share one word.** Copying a reference is not copying a state.
- **The state key** — what decides that two states are the same — is: the word; every field of the core, the refusal table and its causes included; the simulated ledger; every ticket's state and holder; the worker's presence; every retained token as cell and occupancy number; each snapshot's values; each actor's position and any result it holds but has not applied; the test's table, H and A; every remaining budget (faults, admissions, attempts, replays); and every history fact a property reads, such as "this fact was once refused as a conflict" and "this gating attempt issued an insert".
- **Addresses are not part of the key.** They differ from copy to copy and are never compared across worlds. In exploration every bound value belongs to its world's one ledger, so there is no relation to record. A second ledger appears only in T-BIND, which is not exploration.
- Two states with equal keys are treated as one. Anything left out of the key would be a defect in the evidence; the receipts list the key's fields.

**Limits, in every configuration:** a ticket limit of 8 and a refusal-table limit of 4. Neither is reached in exploration; the limits themselves are T-LIMIT's subject. One writer per fact, or one for a batch. Settlement and the worker are on in every configuration.

**An unwinding verb really unwinds.** The simulation raises it without the panic hook; the explorer catches it. The guard of R7 is exercised, not imitated.

### 7.2 Actors

"[T]" marks a step under the test's obligation-table lock: one whole step. The test's table holds, for each fact, a class: Unattempted; the three debt classes Refused, Uncertain and Conflict; and the two final classes Paid and Retired. It also holds H, the highest sequence of a committed explanation the worker has reported; and A, its copy of the acknowledgement frontier. "Recovery required" means H is above A.

| Actor | Program |
|---|---|
| FW, a first writer of one fact or one batch | `arm` (if nothing is returned: [T] every member Refused; done). Then the append, or the wait expires and `expire` is called. On Committed: [T] every member Paid. Otherwise: [T] record classes by the table of 4.3; then `confirm`. **If it unwinds at any point after the call:** [T] record classes from the ticket, or Uncertain if the ticket is not Counted; then `confirm`. |
| SE, settlement | [T] claim one unclaimed debt whose guard holds, using one `view`. Every guard needs no handover fault. Refused needs Healthy and not latched; Uncertain needs the phase not *barrier needed* and not latched; Conflict needs Healthy, not latched and X empty. Then: Refused → `arm` and a single-member append; if `arm` returns nothing, [T] release the claim, and no guard holds again. Uncertain → lookup. Conflict → explain. [T] apply: Committed or Present → Paid. Absent → Refused. Lookup's Conflict → Conflict. Explained → Retired. A counted result → by 4.3. Anything else → unchanged. Release the claim; `confirm` if a ticket is held. |
| WK, the worker | Reads the agenda; [T] A becomes the larger of A and its frontier. Then one step that the agenda allows: barrier; resolve; append C; append X; or, if X is committed with sequence *s*: [T] H becomes the larger of H and *s*, and then empty X. |
| ADM, admission | [T] one `view`. Admit if `admits`, no debt, and H is not above A. |
| ACK | snapshot; the Owner confirms or declines; the append; on Committed: [T] A becomes the larger of A and N. |
| GC, a gating caller | one gating append of its own record. **The exploration offers each gating record once.** A record offered again is covered by T-GATE, not by exploration. |
| ENV | once: the Prepared record that fact F1 was missing appears in the simulated ledger, standing for an append by another writer. This is the only precondition that can change, because rows never disappear. |
| REPLAY | a fixed number of times: makes the worker repeat an operation with a token that is **not** the present one for its cell |
| DROP | a first writer that, holding a Counted ticket, drops it without recording or confirming |

"A conflicting fact" is one whose lifecycle role is already held by a row in the initial ledger, or, for F1 where ENV is on, whose Prepared record is missing.

REPLAY and DROP are on only where 8.3 names them.

### 7.3 Properties kept from the scratch model, restated

| Id | Statement |
|---|---|
| P1 | At a successful admission: no result is unrecorded, and U, C and X are empty. |
| P2 | Always: the test's debt counter equals the facts in a debt class. |
| P4 | Never, at a step boundary: U occupied together with C or X. |
| P5 | Paid implies the fact is in the ledger. Retired implies its explanation is. |
| P6 | A fact ever refused as a conflict is never Paid. |
| P7 | A gating caller told Committed has its record in the ledger. |
| P8 | **Per attempt.** For every gating attempt that was not answered Committed **and that issued an insert:** its record is absent from the ledger; or it is still in U; or its correction is in C; or its correction is in the ledger. **This holds until a later attempt of the same record is answered Committed.** A later successful attempt requires the earlier attempt's absence to have been established, either by a definite Not written or by a qualified resolution as absent; and from then on the row in the ledger is the later attempt's, which was reported correctly. An attempt refused before any insert is outside P8: it cannot have written anything (E4). |
| P9 | The core never latches for a broken cell invariant. |
| P12 | At a successful admission, every explanation in the ledger **is covered by a durable acknowledgement.** This says nothing about a dialog or a notice. |
| P13 | Always: the layer's A ≤ the qualified frontier ≤ the durable frontier ≤ the head. |
| P15 | At a successful admission, every explanation's sequence is at most the durable frontier. |
| D1 | A state with no enabled step is terminal: every actor done; no ticket held; and either latched, or handover fault, or all cells empty, Healthy and every fact Paid or Retired. |
| L1 | The graph of steps that are not faults has no cycle, and each of its end states is terminal. |

P3 is replaced by E3. P11 is part of E9. P14 is now a matter of type: "committed" carries its sequence.

### 7.4 Properties added

| Id | Statement |
|---|---|
| E1 | At every step boundary the word's phase, session, latch and cell bits equal the core's state. |
| E2 | A store operation makes no load and at most one successful exchange. Each lock-free ticket step of 4.3 makes exactly one, except the drop of a confirmed ticket, which makes none. `view` makes one load and no exchange. The agenda makes neither. |
| E3 | Always: `unapplied` equals the Counted tickets held plus the Counted tickets dropped. `live` equals the Armed and Counted tickets held plus the Counted tickets dropped. `live` is at most the limit. |
| E4 | **Persistence is sound.** If a result says Not written, the simulated ledger is exactly as before the call. If it says Committed, every member is in it. A result that names a conflict says Not written. |
| E5 | **Verb order.** No insert before every member is classified. No insert if any member was refused. A commit only after every insert succeeded. Exactly one rollback after the first failure a verb reports in an open transaction, and after a refusal in step 3; none after a durable commit; no verb at all after an unwind. |
| E6 | P1 and P6 hold for every member of a batch, and P6 also for a conflict found by lookup. |
| E7 | After a gating append is refused with a new cause: U is empty, X holds that explanation, and the table has no entry for it. |
| E8 | An operation given a stale token answers *stale* and leaves the core and the word exactly as they were. |
| E9 | Latched and handover fault, once set, stay set, and no admission succeeds after either. |
| E10 | With worker verbs unwinding, D1 and L1 hold. |
| E11 | The refusal table never exceeds its limit; a retained cause is never changed; an entry leaves only in a step in which the store saw that original's explanation in the ledger. |

**Safety and availability are separate claims** (finding AM-06).

| Claim | Holds for |
|---|---|
| Safety: P1 to P15 as kept, E1 to E9, E11 | every configuration |
| L1, which is a property of the explored graph and not of a real execution (6.3) | every configuration without DROP |
| In the explored graph, an end state has every fact Paid or Retired and all cells empty | explored runs that end neither latched nor with handover fault |
| After a latch or a handover fault | **no progress is claimed.** Admission stays closed, the session ends without a clean close, and the remedy is a restart. |

### 7.5 Controls

Each control is one named change, run **on the configuration named**. It must make its property fail by assertion. A build failure or a fixture failure kills nothing, with one stated exception (C22). **If a control is not killed on its configuration, the mission reports that and stops. It does not pick another configuration.**

**Changes to the core.**

| Id | Change | Fails | On |
|---|---|---|---|
| C1 | the count becomes a flag | P1 | F-1 |
| C2 | the gating entry rule does not test C and X | P4 | F-1 |
| C3a | acknowledgement test 2 removed | P13 or P15 | F-7 |
| C3b | acknowledgement test 4 removed | P13 or P12 | F-6 |
| C3c | acknowledgement test 5 removed | P12 | F-6 |
| C4 | acknowledgement test 1 removed | P4, P8 | F-8 |
| C5 | resolve, for an acknowledgement in U, raises the frontier with neither the presence read nor the frontier check | P13 | F-5 |
| C6 | classification row b ignores the refusal table, **with** A4 | P6 | F-PT |
| C7 | *Explained* removes every table entry, **with** A4 | P6 | F-PT |
| C8 | members are classified and inserted one at a time | E5 | F-B |
| C9 | `Rollback::Unknown` is treated as `Undone` | E4 | F-B2 |
| C10 | `Commit::Unknown` followed by `Rollback::Undone` gives Not written | E4 | F-1 |
| C11 | a constraint failure during the inserts gives Not written whatever the rollback said. **This is the amendment's rule, which the review rejected.** | E4 | F-B2 |
| C12 | the publication is two exchanges: bits, then count | E2 | X-1 |
| C15 | a cell operation ignores the occupancy number | E8 | F-R |
| C16 | the drop of a Counted ticket lowers the count | E3 | F-D |
| C17 | the guard reports Not written after an insert was issued | E4 | F-B2 |
| C18a | a refused gating append leaves U occupied | E7 | F-G |
| C18b | a gating conflict makes a table entry | E7 | F-G |
| C20 | an explanation found already present is not marked committed | L1 | F-2 |
| C23 | the publication omits the X bit | E1 | F-2 |
| C24 | resolve empties U for a present gating record without placing the correction | P8 | F-1 |
| C25 | a gating append answers Committed on `Commit::Unknown` | P7 | F-1 |
| C26 | a passed barrier clears the latch | E9 | F-4 |

**Changes to the core, killed by a unit test.**

| Id | Change | Killed by |
|---|---|---|
| C14 | the ticket check of step 0 is removed | T-BIND |
| C19 | X can be emptied before it is marked committed | T-CELL |
| C21 | the well-formed test of step 1 is removed | W6 |
| C22 | the ticket derives `Clone` | T-OWN. **Killed by a build failure, on purpose:** the recorded diagnostic must be E0283 at the assertion's line. |
| C27 | the guard is created after the entry rule | T-PREC: an append refused at entry is not counted |
| C28 | resolve's "present, frontier below N" check alone is removed | T-RES, with a verb that answers inconsistently. (In exploration the presence read and this check hide each other, so neither can be controlled alone there.) |

**Changes to an actor.**

| Id | Change | Fails | On |
|---|---|---|---|
| A1 | the writer confirms before it records | P1 | F-1 |
| A2 | admission is split into two steps under two separate holds of the test's lock. Step one takes a `view` and keeps only its low 16 bits: phase, session, latch, cells, handover fault. Any actor may step in between. Step two takes a second `view`, keeps only `unapplied`, reads the debt count, H and A, and decides from the kept bits of step one and the values of step two. | P1 | F-1 |
| A3 | the worker empties X before it raises H | P12 | F-5 |
| A4 | the drop paths record a blanket Uncertain | **nothing may fail** | F-PT |
| A5 | settlement claims a Conflict obligation while the store is not Healthy | L1 | F-2 |
| A6 | "recovery required" is a flag, not a comparison of H with A | P12 or P15 | F-5 |

**No control exists for the order "count first, then the ticket's state"** (6.2). By R6 and R8 no test can tell the two orders apart. It rests on review.

**Properties with no control:** P2, P5, P9, D1, E6, E10, and the handover-fault half of E9. C7 without A4 is expected to fail E11 on F-3; that run is added to the receipts but is not required to kill.

Controls are applied by hand or by a script kept outside the repository. D2A-1 does not add them to the mutation-control manifest.

### 7.6 Tests that are not exploration

In `unit_tests.rs`. Each is deterministic. None sleeps or waits on a clock.

| Test | What it fixes |
|---|---|
| T-MATRIX | 5.3 and 4.9, every cell, with the order of refusals |
| T-PREC | 4.4's persistence table, every row, for one member and for a batch; and every malformed shape |
| T-OWN | a ticket, a snapshot and the worker cannot be cloned: a call that is ambiguous for a cloneable type, so the file does not build if one is. With it, a positive case: a ticket is armed, lent, counted and confirmed. Written so that clippy accepts it on every target. |
| T-BIND | a ticket of another ledger; a Counted ticket lent again; a snapshot of another ledger; a worker of another ledger; a token of another ledger, of another cell, and of an old occupancy. **And the case of finding PC-01:** a worker and a token of ledger one are kept; ledger one's core is dropped; many ledgers are created and dropped; a new ledger is brought to the same cell and the same occupancy number; the old worker is offered to it and refused; separately, with the new ledger's own worker, the old token is offered and is stale. No counter exists to bring near its end, and the test says so. |
| T-GATE | 4.5's table of a record offered again, every row; in particular a committed record offered again while Healthy, and again while an entry rule refuses, where no correction may appear; and a record that carries a committed correction, which must be refused as *corrected* |
| T-CELL | 4.7's two tables, row by row |
| T-RES | resolve, every row, including the inconsistent one |
| T-ACK | 4.8: each of the six tests refusing alone; values at the ends of 64 bits |
| T-LIMIT | `arm` at the limit; the table at its limit, for append and for lookup; counts at their ends, and a cell's occupancy number at its end, each forced through the test's access to private state. A forced state is not called reachable. |
| T-PB1 | using the hook: just after an append's publication, `view` already refuses admission |
| T-PB2 | just before that publication, the word is exactly as it was before the call |
| T-PB3 | `expire`: the count is up when its one exchange has succeeded, and only then |
| T-PB4 | another ticket's `confirm`, run from the hook after a publication's first exchange has failed, is not lost: the loop tries again and both changes stand |
| T-PB5 | each verb of each operation unwinds: exactly one publication, by the guard |

**Named witnesses,** each a fixed schedule of a known case:

| Witness | Case |
|---|---|
| W1 | review finding D03-01: two writers each get a failing result; one records and confirms; admission must stay refused until the other has too. Both orders. |
| W2 | the scratch model's first P6 failure: a fact is refused as a conflict while X holds another explanation; its writer unwinds; a lookup finds it absent; ENV fires; a retry is offered. It must not be Paid. |
| W3 | review finding D03-03: a snapshot is taken; a second explanation commits; the acknowledgement is appended. The second explanation must stay above the frontier and admission must stay refused. |
| W4 | the fourth revision's case: a first acknowledgement's append is Uncertain and written; resolve finds it present; before the worker applies it, a second does the same. The agenda's frontier covers both. |
| W5 | finding AM-01: a batch whose second insert fails on a constraint, then a failed rollback |
| W6 | finding AM-01: a batch with one identity under two contents is malformed; each content, offered alone afterwards, gets its real answer |
| W7 | finding AM-03: X emptied and refilled; the old token is stale |
| W8 | finding AM-05: the worker unwinds after U was emptied for a present acknowledgement; the next agenda still carries the frontier |
| W9 | finding AM-06: a Counted ticket dropped |
| W10 | expiry before entry |
| W11 | a commit that is not durable, followed by a clean rollback |
| W12 | a lookup that finds different content, then explain |
| W13 | finding PC-03: the same gating record, written, with its append returning Uncertain in one run and its verb unwinding in the other. After the barrier and resolve, the two corrections differ in their exit and in nothing else. |

### 7.7 Where the earlier proof rows land

The amendment's proof rows that concern the store's pure half are met in D2A-1 as follows. Their storage half stays with the full contract.

| Row | Here |
|---|---|
| D-33, the transition table explored | sections 7 and 8 |
| D-34, D-35, D-36, handover | W1, W10, F-1, F-D, E3 |
| D-37, D-40, D-57, D-61, the refusal table | W2, W12, F-PT, E11 |
| D-41, the latch | T-LIMIT, T-PREC, E9 |
| D-42 to D-47, acknowledgement | T-ACK, W3, W4, F-5 to F-8 |
| D-55, a ticket cannot be copied or confirmed twice | T-OWN, C22 |
| D-59, the worker's unwind | F-W, W8, E10 |
| D-60, a gating conflict | F-G, E7 |
| OB-7a, OB-7b, what is accepted when not Healthy or when C or X is occupied | T-MATRIX |

---

## 8. What runs when (finding CT-04, ruling B/Q-4)

### 8.1 Three outcomes

Every exploration ends as **PASS** (finished; every property holds), **FAIL** (a property failed; the trace is printed) or **INCOMPLETE** (a cap was reached, or the run was stopped). **INCOMPLETE is never PASS.** In the default suite it fails the test.

### 8.2 The default suite

Runs in `cargo test --workspace --locked`, unoptimised, with nothing ignored: every test of 7.6, and five small explorations.

| Id | Bounds | Aimed at |
|---|---|---|
| X-1 | two facts, two writers, settlement, one admission, 2 faults | P1, E2, E3 |
| X-2 | one fact, two gating callers, the worker, one admission, 1 fault | P4, P7, P8, P9 |
| X-3 | two facts, one conflicting, ENV on, 1 fault | P5, P6, E11 |
| X-4 | one conflicting fact, one acknowledgement, one admission, 1 fault | P12, P13, P15 |
| X-5 | one clean batch of two, 2 faults | E4, E5, E6 |

Caps: 250,000 states each, and 180 seconds for these new tests together on the development machine. They are ceilings for an investigation, like those of 8.3, not measured costs. That is not a claim about how long the whole workspace's tests take. The five explorations take a test-only lock, so that they run one at a time even when the test runner uses several threads. **The mission measures. If a run does not finish under its cap, the mission stops and reports. It does not shrink a bound.**

### 8.3 The full reviewed set

Marked ignored, so that the ordinary test command stays short. **Run explicitly, optimised, on the exact final commit of the mission, every one, before the mission reports.** Missing evidence for any of them is a failed mission, whatever the ordinary test command says.

"CFG-n" is the scratch model's configuration of that number; its bounds are repeated here.

| Id | Facts and conflicts | Faults | Admissions | Gating callers | Acknowledgements | Other |
|---|---|---|---|---|---|---|
| F-1 (CFG-1) | two; none conflicts | 3 | 2 | 2 | 0 | |
| F-2 (CFG-2) | two; F2 conflicts | 2 | 2 | 2 | 0 | |
| F-3 (CFG-3) | two; both conflict | 2 | 2 | 2 | 0 | ENV on |
| F-4 (CFG-4) | two; F2 conflicts | 2 | 2 | 2 | 0 | a cell's own append can meet a constraint |
| F-5 (CFG-5) | two; both conflict | 2 | 2 | 0 | 1, two attempts | |
| F-6 (CFG-6) | two; both conflict | 2 | 2 | 0 | 1, two attempts | malformed snapshots forced through private state |
| F-7 (CFG-7) | two; both conflict | 1 | 1 | 0 | 2, one attempt each | |
| F-8 (CFG-8) | two; F1 conflicts | 2 | 2 | 1 | 1 | |
| F-PT | as F-3 | | | | | actor change A4 on; **every property must hold** |
| F-B | as F-3 | | | | | one writer offers both facts as one batch |
| F-B2 | two; none conflicts | 3 | 1 | 0 | 0 | one writer offers both as one batch; the batch reaches its inserts |
| F-G | one fact; two gating records, one conflicting | 2 | 2 | 2 | 0 | |
| F-W | as F-2 | | | | | worker verbs unwind |
| F-D | as F-1, 2 faults | | | | | DROP on. No L1. |
| F-R | as F-2 | | | | | REPLAY on, three times |

Each control of 7.5 is run on its configuration, with a failing receipt, and a restored receipt showing the unchanged core passes that configuration.

**Caps for each run:** 30,000,000 states, counted by the explorer; 45 minutes and 16 GiB, enforced by the command that starts the run. **They are ceilings for an investigation, as the second review accepted them (ruling 6). They are not a measured cost and not an accepted one.** Reaching one, a failed allocation, a run killed from outside, and an unfinished search are each INCOMPLETE.

**One full exploration runs at a time.** Before each, the starting command checks that the machine has the memory free; if not, the run does not start and is reported as not started. It never stops a local-model workload, or anything else, to make room. The full set is run at the mission's evidence gate, not after each edit.

**These bounds are reviewed bounds.** The implementer may not lower a fault budget, drop an actor, or count an INCOMPLETE run as PASS.

### 8.4 Receipts

For every run, default or full: the commit; the compiler version; the profile; the exact command; the configuration and its bounds; states and transitions; each property's verdict; for each branch of each operation, whether any run took it; the time; the peak memory as the starting command reports it; and how the run ended. Every `#[ignore]` in the two test files is listed by name with the receipt that shows it ran.

Receipts are kept in a directory outside the repository. The mission's report gives each file's hash. State counts are not expected to match the scratch model's, and nothing here depends on that model's results.

---

## 9. What holds the core in place (finding CT-01)

### 9.1 One new guard in `P3`

It reads production text only: comments and test-only items are left out, as the existing guards do.

| # | Asserts |
|---|---|
| 1 | In the control crate's production sources, the word `ledger` occurs only in `gc/lib.rs`, once, in the line `pub(crate) mod ledger;`, and inside `gc/ledger.rs` and `gc/ledger/`. (At the checkpoint the word occurs nowhere in the crate.) |
| 2 | The production text of `gc/ledger.rs` is exactly the two lines of 3.2. |
| 3 | `gc/ledger/protocol.rs` names nothing outside itself except `std::sync::atomic`, `std::sync::Arc` and `std::collections::BTreeMap`: no other `std::`, `core::` or `alloc::` path, no `crate::`, no `super::`, no other crate. |
| 4 | It contains none of: `unwrap(`, `expect(`, `panic!`, `unreachable!`, `todo!`, `unimplemented!`, `assert!`, `assert_eq!`, `assert_ne!`, `debug_assert`. |
| 5 | Every atomic ordering it names is `SeqCst`. |
| 6 | It declares one atomic, the word, and no `static`. On it, it calls only `load`, once, and `compare_exchange`, once. No `store`, `swap`, `compare_exchange_weak` or any `fetch_` call. |
| 7 | Every item in it is private or `pub(super)`. |
| 8 | It begins with the lint denials of 9.2. |

**Classification: a new property.** It weakens nothing and edits no existing guard.

### 9.2 Lints denied for the whole file

`protocol.rs` begins by denying, in a build that is not a test build: `clippy::indexing_slicing`, `clippy::arithmetic_side_effects`, `clippy::unwrap_used`, `clippy::expect_used`, `clippy::panic`. CI runs clippy on every target with warnings denied, so a slice index or an unchecked sum in the core fails the gate. 6.4 says what these lints do not catch. That was seen once on the unauthorised scratch skeleton under 1.97.0, with one probe line added; it is not the gate, and the mission confirms it on 1.94.0.

### 9.3 What these are for

D2A-1's claims are confinement claims: nothing calls the core, and the core is memory only and is written not to panic. A guard that arrives later cannot support them now, so it arrives here. **The guard for the SQLite binding, and the classification the review requires under B/Q-1, travel with the dependency, in the shell's mission.**

---

## 10. Pins

Classified by reading the guards at `862a11ce`. Not run.

| Pin | Classification for D2A-1 |
|---|---|
| P-079, the crate's exact dependency list | UNCHANGED: no dependency is added |
| P-075, every production `mod` line loads one scanned file | UNCHANGED. `protocol.rs` is a scanned production source. The `cfg_attr` on its `mod` line is not a test attribute to that scanner (`app/src-tauri/src/phase0_surface/rust_paths/tests.rs:300-331`). |
| P-051, files named like tests that production compiles | UNCHANGED: both test files are declared under `#[cfg(test)]`; the production walker skips names ending `tests.rs` (`P3:2676-2680`) |
| P-036, P-035, P-028, P-081, P-082: the words `execute`, `begin`, `confirmed`, `launch` after `.` or `:`; `::connect` | UNCHANGED, **if** the core uses none. Appendix A uses none: the transaction verb is `open`; the ticket's method is `confirm`. |
| P-040, P-039, P-041, P-048, P-049, P-053, P-076, P-099, P-102 | UNCHANGED: no `unsafe`, no `libc`, no environment, no mechanism, no `extern`, no macro definition |
| P-077, the seven compile-fail blocks of `gc/lib.rs` | UNCHANGED: no `//!` line is added to that file. T-OWN is an ordinary test. |
| P-047, the crate items the desktop names | UNCHANGED |
| Manifest control M87 | its recorded hash of `gc/lib.rs` goes stale. TEXT UPDATE, SAME PROPERTY. |
| Manifest controls M32, M78, M79, M80, M89, M112, M113, M141, M143, M153, M177, M178, M179, M180 | their recorded hash of `P3` goes stale. TEXT UPDATE, SAME PROPERTY, **if** the new guard is added without touching any anchor. |
| clippy with warnings denied; rustfmt; the workspace tests | UNDETERMINED until run |

**D2A-1 does not edit the manifest.** By ruling B/Q-5: the original is preserved; the mismatch is reported; no mutation result that depends on those two files is relied on until a reviewed mapping exists; failing, restored and final receipts are kept; and before final Candidate 11 validation the approved manifest revision and its campaign must agree with the exact final tree.

---

## 11. Label on completion (finding CT-05)

**The only adopted interim-pass label is IMPLEMENTED — INTERIM REVIEW PASSED — AUDIT PENDING.** I propose that it may be used for D2A-1 only with its scope written beside it, and only on these conditions:

- every run of 8.2 and 8.3 is PASS, every control is killed, and Codex's review of the evidence passes;
- the scope sentence reads: *"the ledger's pure protocol core and its tests, under storage assumptions A-1 and A-2, which are not discharged. Not D2a. Not the storage core. Not durability. Not activation. Not a composed production system. Not final component acceptance."*

If any required run is INCOMPLETE, or any property fails on the unchanged core, **no label is used.** The report says what happened and the design comes back to me.

The second review accepts this scoped use (ruling 5), only after the evidence and Codex's review have actually passed. No label is awarded now. Using it remains the Owner's decision at that time.

---

## 12. Stop conditions

- The base commit does not match GitHub.
- A file outside 3.2 would have to change.
- An existing guard would have to be edited.
- The lint arrangement of 3.2 behaves differently on the pinned compiler.
- A dependency, or any path outside 9.1 item 3 in production text, would be needed.
- The core cannot be written inside 9.1 item 4 and 9.2.
- A property fails on the unchanged core.
- A control is not killed on its named configuration or by its named test.
- A run of 8.2 or 8.3 is INCOMPLETE.
- An item of Appendix A cannot be implemented as declared. **The mission reports the difference. It does not redesign the interface.**

---

## 13. What D2A-1 leaves to the full D2a contract, and to B2

Each is an obligation on a document not yet written. None is claimed here.

| Obligation | On | From |
|---|---|---|
| For every verb: its SQL; a real-file test of each fact it reports; a control that makes it report a wrong fact. `Rollback::Undone`, `Commit::Durable` and `Read::Corrupt` exactly as 4.1 defines them. | full contract | CT-03, AM-07 |
| Controls on the shell that the core cannot see: the shell does not call the core's operation; calls the wrong one; drops or rewrites its result. | full contract | CT-03 |
| The barrier returns the connection to a known state, also after an unwind left a transaction open. | full contract | 4.4 |
| The schema declares no deferred constraint; a test reads the schema and shows it. | full contract | AM-01 |
| Records built only by the ledger's builder for their family, bound to the store; a record of another store refused. | full contract | AM-02 |
| Session opening as an operation the core runs, open only on a confirmed record. **Sealing as three separate facts — sealed, close attempted, close confirmed — with sealed first and never undone** (4.9). Bootstrap; cells G and S with the occupancy rule; statements; checkpoint records; the frontier read. | full contract | AM-02, AM-03, AM-07, RF-01 |
| **What a correction says** for each family and each stage of a commitment, and its fixed encoding. D2A-1 keeps only the record, the sequence and how the attempt ended. | full contract | PC-03 |
| The wait to enter the store, with its time limit, and the call to `expire`. | full contract | — |
| The dependency, its guard and its mechanism classification, in one mission. | full contract | CT-01, B/Q-1 |
| The fault harness, callback by callback. Memory measurement and the resource decision. R-a to R-e; A-1 and A-2. | full contract | CT-02, CT-06 |
| The ticket limit's two numbers. Retry of a malformed batch singly, in lifecycle order. **A record whose Prepared is still unpaid is not offered for retry,** or it is refused as *missing Prepared* for good. | B2 | 4.3, 4.4 |
| What the Owner is told when a snapshot was consumed by a *not now* refusal and a second dialog is needed. | B3 | 4.8 |

---

## 14. Preconditions, as they stand

| Precondition | State |
|---|---|
| The First Architect's acceptance of this contract | **given for CONTRACT-02, at contract level, for the dormant protocol slice only** (fourth review, 2026-10-11). This corrected file has not yet been seen by him. |
| The Owner's decision on its scope: the six files of 3.2, the new guard in the guard file, the branch | **not yet** |
| The Owner's authorisation of the mission | **not yet** |
| The interim review arrangement | adopted by the Owner, 2026-10-10 |
| A reviewer for the implementation evidence | Codex, under that arrangement. A reviewer does not accept a repair it wrote. |

---

## 15. Decisions

**For the First Architect.**

1. Accept, amend or reject this contract.
2. Whether binding by shared state (section 0A, PC-01) meets your finding. No ledger number is left to exhaust; the occupancy number's end is latched and tested.
3. Whether keeping the exit in U, with the correction's full content left to the full contract, meets PC-03.
4. Whether P8 as restated, with re-offers covered by T-GATE and not by exploration, meets PC-02.
5. **Whether a gating record that carries a correction must be refused when offered again** (4.5, fifth row). I have written it so, from the standing rule that a correction never revives its step. It adds one fact to the `facts` verb and one reason. If you read that rule as binding only the layer above, the row and the fact can go.

**For the Owner, after that ruling.**

6. Authorise D2A-1 with the files of 3.2, or not.

---

## 16. Weakest points

1. **Section 4.1 is a change of structure** made in answer to a finding, not carried from anything explored. If it is wrong, it is wrong at the centre.
2. **The verbs are an interface I invented.** Until the shell exists, nobody knows whether SQLite can supply each one as cleanly as 4.1 says. `Rollback::Undone` leans on assumption A-1.
3. **The exploration may not fit its caps.**
4. **The test plays B2.** A defect shared by my idea of the layer above and my test of it is caught by neither.
5. **"Count first, then the ticket's state" has no test and can have none.**
6. **Handover fault ends the session's admissions for one dropped ticket, or for one `arm` at the limit,** and a latch for one wrong ticket. Both are deliberate and severe.
7. **A malformed batch moves work to B2:** it must retry singly and in order, and I have only stated that, not designed it.
8. **A snapshot is consumed even by a *not now* refusal,** so a passing unhealthy moment costs the Owner a second dialog.
9. **Appendix A's changes in this version were made by hand and checked by no compiler.**
10. **Re-offered gating records are tested by table, not explored.** A defect that needs a re-offer interleaved with other actors would not be found here.
11. **The comparison "same shared state" has no control of its own.** Control C14 removes the ticket check altogether; nothing tests a comparison that is merely weakened. T-BIND is its only test.
12. **Tokens now hold a reference,** so a kept token keeps a dropped ledger's shared state alive. That is small, and bounded by how many tokens the layer above keeps.

---

## Appendix A — the core's complete surface

Every item of `gc/ledger/protocol.rs`. Items marked `pub(super)` are the ones `gc/ledger.rs` can name; the others are private and shown for completeness. Shapes are part of the contract. Names are PROPOSED. Derived traits and function bodies are left out. `Ticket`, `Snapshot` and `Worker` derive neither `Clone` nor `Copy`.

**Not type-checked.** The previous version's declarations were checked without authorisation as an empty skeleton (section 0, item 3). The declarations below differ from those as follows, edited by hand: the counter is gone and `Shared` has lost its number; `Snapshot`, `Worker` and `CellToken` hold a reference where they held a number; `Exit` is new, and `Occupant::Gating` and `Occupant::Correction` carry it; `MemberFacts` has the field `corrected` and `Reason` the variant `Corrected`. `CellToken` implements `Clone` and not `Copy`, and so does every type that contains one.

    pub(super) const ID_LEN: usize = 32;
    pub(super) const BATCH_MAX: usize = 256;
    pub(super) type Seq = u64;

    // ---- records, as the core sees them ----------------------------------------

    pub(super) struct RecordKey {
        pub(super) id: [u8; ID_LEN],
        pub(super) digest: [u8; 32],
    }

    pub(super) enum Role {
        Prepared,
        Started,
        End,
    }

    pub(super) struct Part {
        pub(super) commitment: [u8; ID_LEN],
        pub(super) role: Role,
    }

    pub(super) struct Member {
        pub(super) key: RecordKey,
        pub(super) part: Option<Part>,
    }

    pub(super) struct FactRecord(Member);

    pub(super) struct GatingRecord(Member);

    impl FactRecord {
        pub(super) fn new(member: Member) -> FactRecord;
        pub(super) fn member(&self) -> &Member;
    }

    impl GatingRecord {
        pub(super) fn new(member: Member) -> GatingRecord;
        pub(super) fn member(&self) -> &Member;
    }

    pub(super) struct Limits {
        pub(super) tickets: u32,
        pub(super) refusals: u32,
    }

    // ---- the published word ------------------------------------------------------

    struct Shared {
        word: AtomicU64,
        tickets: u32,
    }

    pub(super) struct Health {
        shared: Arc<Shared>,
    }

    pub(super) struct HealthView {
        word: u64,
    }

    pub(super) enum Phase {
        Healthy,
        NeedBarrier,
        Resolving,
    }

    pub(super) enum Session {
        Bootstrap,
        Open,
        Sealed,
    }

    impl Health {
        pub(super) fn view(&self) -> HealthView;
        pub(super) fn arm(&self) -> Option<Ticket>;
    }

    impl HealthView {
        pub(super) fn admits(self) -> bool;
        pub(super) fn phase(self) -> Phase;
        pub(super) fn session(self) -> Session;
        pub(super) fn latched(self) -> bool;
        pub(super) fn handover_fault(self) -> bool;
        pub(super) fn u_occupied(self) -> bool;
        pub(super) fn c_occupied(self) -> bool;
        pub(super) fn x_occupied(self) -> bool;
        pub(super) fn unapplied(self) -> u32;
        pub(super) fn live(self) -> u32;
    }

    // ---- tickets -------------------------------------------------------------------

    pub(super) enum Persistence {
        NotWritten,
        Uncertain,
    }

    pub(super) struct Counted {
        pub(super) persistence: Persistence,
        pub(super) conflict_member: Option<u16>,
    }

    enum TicketState {
        Armed,
        Counted(Counted),
        Settled,
    }

    pub(super) struct Ticket {
        shared: Arc<Shared>,
        state: TicketState,
    }

    impl Ticket {
        pub(super) fn counted(&self) -> Option<Counted>;
        pub(super) fn confirm(self);
        pub(super) fn expire(&mut self) -> FactOutcome;
    }

    impl Drop for Ticket { /* 4.3 */ }

    // ---- causes ----------------------------------------------------------------------

    pub(super) struct Row {
        pub(super) seq: Seq,
        pub(super) digest: [u8; 32],
    }

    pub(super) struct Holder {
        pub(super) id: [u8; ID_LEN],
        pub(super) seq: Seq,
    }

    pub(super) enum CauseKind {
        IdentityContent { id: [u8; ID_LEN], existing: Row, offered: [u8; 32] },
        DuplicateRole { part: Part, holder: Holder, offered: RecordKey },
        MissingPrepared { part: Part, offered: RecordKey },
    }

    pub(super) struct Cause {
        kind: CauseKind,
    }

    impl Cause {
        pub(super) fn kind(&self) -> CauseKind;
    }

    // ---- the storage verbs the shell supplies ----------------------------------------

    pub(super) enum Read<T> {
        Got(T),
        Failed,
        Corrupt,
    }

    pub(super) enum Insert {
        Inserted(Seq),
        Constraint,
        Failed,
    }

    pub(super) enum Commit {
        Durable,
        Unknown,
    }

    pub(super) enum Rollback {
        Undone,
        Unknown,
    }

    pub(super) struct MemberFacts {
        pub(super) row: Option<Row>,
        pub(super) explained: Option<Seq>,
        pub(super) corrected: Option<Seq>,
        pub(super) holder: Option<Holder>,
        pub(super) prepared: bool,
    }

    pub(super) struct SetFacts {
        pub(super) frontier: Seq,
        pub(super) head: Seq,
        pub(super) top: Seq,
        pub(super) count: u64,
        pub(super) digest: [u8; 32],
    }

    pub(super) trait Txn {
        fn open(&mut self) -> Read<()>;
        fn facts(&mut self, index: u16, member: &Member) -> Read<MemberFacts>;
        fn insert(&mut self, index: u16, member: &Member) -> Insert;
        fn commit(&mut self) -> Commit;
        fn rollback(&mut self) -> Rollback;
    }

    pub(super) trait Reads {
        fn row(&mut self, id: &[u8; ID_LEN]) -> Read<Option<Row>>;
        fn explained(&mut self, key: &RecordKey) -> Read<Option<Seq>>;
        fn acknowledgement(&mut self, set: &SetFacts, nonce: &[u8; 16]) -> Read<Option<Seq>>;
        fn frontier(&mut self) -> Read<Seq>;
    }

    pub(super) trait AckTxn {
        fn survey(&mut self) -> Read<SetFacts>;
        fn open(&mut self) -> Read<()>;
        fn recheck(&mut self, above: Seq, upto: Seq) -> Read<SetFacts>;
        fn insert(&mut self, snapshot: &Snapshot) -> Insert;
        fn commit(&mut self) -> Commit;
        fn rollback(&mut self) -> Rollback;
    }

    pub(super) trait CellTxn {
        fn open(&mut self) -> Read<()>;
        fn present(&mut self, occupant: &Occupant) -> Read<Option<Seq>>;
        fn insert(&mut self, occupant: &Occupant) -> Insert;
        fn commit(&mut self) -> Commit;
        fn rollback(&mut self) -> Rollback;
    }

    pub(super) trait BarrierRun {
        fn run(&mut self) -> bool;
    }

    // ---- results -------------------------------------------------------------------------

    pub(super) enum Entry {
        NotOpen,
        Latched,
        NotHealthy,
        CellsBusy,
        Malformed,
        Expired,
    }

    pub(super) enum TicketFault {
        OtherLedger,
        NotArmed,
    }

    pub(super) enum Known {
        Cause(Cause),
        Explained(Seq),
    }

    pub(super) enum Reason {
        Entry(Entry),
        Conflict { member: u16, known: Known },
        SessionClosed { member: u16 },
        Corrected { seq: Seq },
        TableFull { member: u16 },
        Constraint,
        Storage,
    }

    pub(super) enum FactOutcome {
        Committed,
        NotApplied { counted: Counted, reason: Reason },
        Rejected(TicketFault),
    }

    pub(super) enum GatingOutcome {
        Committed(Seq),
        Refused { persistence: Persistence, reason: Reason },
    }

    pub(super) enum Lookup {
        Present(Seq),
        Absent,
        Conflict(Known),
        Refused(Entry),
        Failed,
    }

    pub(super) enum Explain {
        Explained(Seq),
        Pending,
        Busy,
        Unknown,
        Refused(Entry),
    }

    // ---- acknowledgement -----------------------------------------------------------------

    pub(super) struct Snapshot {
        ledger: Arc<Shared>,
        set: SetFacts,
        nonce: [u8; 16],
    }

    impl Snapshot {
        pub(super) fn set(&self) -> SetFacts;
        pub(super) fn nonce(&self) -> [u8; 16];
    }

    pub(super) enum SnapshotOutcome {
        Taken(Snapshot),
        Nothing,
        NotNow(Entry),
        Failed,
    }

    pub(super) enum AckRefusal {
        OtherStore,
        NotNow(Entry),
        Stale,
        Empty,
        Future,
        Set,
    }

    pub(super) enum AckOutcome {
        Committed(Seq),
        Refused(AckRefusal),
        NotApplied(Persistence),
    }

    // ---- cells and the worker ---------------------------------------------------------------

    pub(super) struct Worker {
        ledger: Arc<Shared>,
    }

    pub(super) enum Cell {
        U,
        C,
        X,
    }

    pub(super) struct CellToken {
        ledger: Arc<Shared>,
        cell: Cell,
        occupancy: u64,
    }

    pub(super) enum Stage {
        Built,
        Committed(Seq),
    }

    pub(super) enum Exit {
        InCall,
        ReturnedUncertain,
        Unwound,
    }

    pub(super) enum Occupant {
        Gating { record: GatingRecord, exit: Exit },
        Acknowledgement { set: SetFacts, nonce: [u8; 16] },
        Correction { about: RecordKey, seq: Seq, exit: Exit },
        Explanation { original: RecordKey, cause: Cause },
    }

    pub(super) enum LatchCause {
        Constraint,
        TableFull,
        TicketFault,
        CellInvariant,
        Inconsistent,
        Corrupt,
        Overflow,
    }

    pub(super) struct Agenda {
        pub(super) phase: Phase,
        pub(super) session: Session,
        pub(super) latch: Option<LatchCause>,
        pub(super) u: Option<CellToken>,
        pub(super) c: Option<CellToken>,
        pub(super) x: Option<(CellToken, Stage)>,
        pub(super) frontier: Seq,
    }

    pub(super) enum BarrierOutcome {
        Passed,
        Failed,
        NotNeeded,
        OtherLedger,
    }

    pub(super) enum Resolve {
        Absent,
        Corrected(CellToken),
        Acknowledged(Seq),
        ConflictPlaced(CellToken),
        Retained,
        Inconsistent,
        Stale,
        Refused(Entry),
        OtherLedger,
    }

    pub(super) enum CellAppend {
        Committed(Seq),
        NotWritten,
        Uncertain,
        Stale,
        Refused(Entry),
        OtherLedger,
    }

    pub(super) enum Emptying {
        Done,
        NotCommitted,
        Stale,
        OtherLedger,
    }

    // ---- the core ------------------------------------------------------------------------------

    pub(super) struct Core {
        shared: Arc<Shared>,
        phase: Phase,
        session: Session,
        latch: Option<LatchCause>,
        u: Option<(u64, Occupant)>,
        c: Option<(u64, Occupant)>,
        x: Option<(u64, Occupant, Stage)>,
        occupancies: [u64; 3],
        refusals: BTreeMap<RecordKey, Cause>,
        refusal_limit: u32,
        frontier: Seq,
        worker_taken: bool,
    }

    impl Core {
        pub(super) fn new(limits: Limits) -> (Core, Health);
        pub(super) fn take_worker(&mut self) -> Option<Worker>;
        pub(super) fn session_opened(&mut self) -> bool;
        pub(super) fn session_sealed(&mut self) -> bool;
        pub(super) fn agenda(&self) -> Agenda;

        pub(super) fn append_facts(&mut self, ticket: &mut Ticket, batch: &[FactRecord], io: &mut impl Txn) -> FactOutcome;
        pub(super) fn append_gating(&mut self, record: &GatingRecord, io: &mut impl Txn) -> GatingOutcome;
        pub(super) fn lookup(&mut self, key: &RecordKey, io: &mut impl Reads) -> Lookup;
        pub(super) fn explain(&mut self, key: &RecordKey, io: &mut impl Reads) -> Explain;
        pub(super) fn snapshot(&mut self, nonce: [u8; 16], io: &mut impl AckTxn) -> SnapshotOutcome;
        pub(super) fn append_acknowledgement(&mut self, snapshot: Snapshot, io: &mut impl AckTxn) -> AckOutcome;

        pub(super) fn barrier(&mut self, worker: &mut Worker, io: &mut impl BarrierRun) -> BarrierOutcome;
        pub(super) fn resolve(&mut self, worker: &mut Worker, token: CellToken, io: &mut impl Reads) -> Resolve;
        pub(super) fn append_cell(&mut self, worker: &mut Worker, token: CellToken, io: &mut impl CellTxn) -> CellAppend;
        pub(super) fn empty(&mut self, worker: &mut Worker, token: CellToken) -> Emptying;
    }

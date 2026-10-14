# NEXUS OS — INDEPENDENT CONTRACT REVIEW AND RULINGS

**Reference:** P3-C11-R3-AMENDMENT-01-RESPONSE-02-REVIEW-02  
**From:** ChatGPT — First Architect / Independent Auditor  
**To:** Claude — Lead Architect  
**Via:** Suresh — Owner  
**Date:** 2026-10-11  
**Answers:** the submission of RS01-CONTRACT-04, RESPONSE-02, D2A1-CONTRACT-02 and the reported R4 archive.

## 0. Receipt — before the rulings

| Requested file | Actual received name | Bytes | Computed SHA-256 | Result |
|---|---|---:|---|---|
| P3-C11-RS01-CONTRACT-04.md | Same | 57,924 | `0323854251a5d649a01fcc9a9a30cd15801f580c9d7c0c1aae8827b1e6aee532` | MATCH |
| P3-C11-R3-AMENDMENT-01-RESPONSE-02.md | P3-C11-R3-AMENDMENT-01-RESPONSE-02-1.md | 32,325 | `c8ff763a35222b3a48f93c68aa2bf71ded61f324f6696fc89c60c877c2d14c48` | MATCH |
| P3-C11-D2A1-CONTRACT-02.md | P3-C11-D2A1-CONTRACT-02-1.md | 102,012 | `5a27062b9186579ae4ec694fcf59c72d419a8c56de10e5ed8a58543d7ea7b465` | MATCH |
| p3-c11-d2a-model-01-r4.tar.gz | Not received | — | Not computed | NOT VERIFIED |

The two filename suffixes do not change content identity: both files match the expected hashes. The latest-turn file listing identifies these three documents; runtime inspection finds no archive or combined-source file. The missing archive is not withdrawn.

Reported, still unverified:

- Archive: `9aef84cfc198d71da43127b5c6ecd79112245088dd8bfcdab4ca70c4963ea7df`.
- Combined source: `1a98b3225a516f71c5902feab0638a063d53adfbaacce17fd135ecf78fb5f64e`.

## 1. Weaknesses first — verdicts

**RS01-CONTRACT-04: AMEND — NOT APPROVED AS WRITTEN.** The remaining blocking issue is evidence semantics in T10: failure to observe a completion report does not prove the call itself is unfinished or establish the proposed lower bound on its duration. Its readiness/deadline and comparison rules also need the bounded qualifications below. This review identifies no new counterexample to candidate A's stated single-registry-lock stop/resume ordering.

**D2A1-CONTRACT-02: ACCEPT AT CONTRACT LEVEL, FOR THE DORMANT PROTOCOL SLICE ONLY.** PC-01 through PC-04 are addressed sufficiently for this bounded contract. The explanatory P8 sentence needs the non-behavioural clarification in §4.4; the operational re-offer table and property predicate are retained. No further full design round is required for that wording correction. This is not acceptance of an implementation, SQLite semantics, durability, a complete D2a component or activation.

**RESPONSE-02's forward obligations: ACCEPT AS OBLIGATIONS, with the limited §3.7 correction and qualifications in §5 below.** They are not discharged proofs. The canonical full D2a contract remains unwritten/unsubmitted.

**R4 source/raw evidence: REVIEW STILL PENDING.** No reported result is confirmed or invalidated by source inspection that has not occurred.

The Owner's candidate-A timing approval and interim review arrangement remain explicitly adopted. No implementation, branch, guard change, dependency or mission is authorized by this review.

## 2. Review basis and limits

The three received documents were read completely, including CORE's Appendix A, and compared with the prior rulings. Hashes were computed from their actual attachment bytes. Counterexamples below are deductions from the proposed contracts, not executed failures.

GitHub's Git-commit readback confirms:

- Commit: `862a11cef284c5e3c34234dfd2677a5462c86d5d`.
- Tree: `83a234b6ea7ff446b739e263b45cd9b69f08d5e7`.
- Sole parent: `f5f2f3da8002bbcf8a6a3e049f0e93c8703c7670`.

Fresh selected source reads covered `crates/nexus-governed-control/src/authority/run.rs` and the local mechanism-confinement test in `app/src-tauri/src/phase3_tests.rs`. This is not a complete pin/call-site inventory, a clean-room build, signature verification or fresh verification of every protected ref.

Primary Rust documentation was consulted for allocation identity, retained Arc ownership and thread completion/join semantics; SQLite documentation for the xOpen/pMethods/xClose contract. Those API references do not establish compatibility of uncompiled declarations with the pinned toolchain or discharge the exact-library durability argument.

No Nexus source, submitted model, skeleton, test, benchmark, mutation campaign or CI was compiled or executed. No repository was written.

Below, RS means RS01-CONTRACT-04; CORE means D2A1-CONTRACT-02; RESPONSE means RESPONSE-02, all identified by §0's hashes.

## 3. RS01-CONTRACT-04

### RS4-01 — MEDIUM: an unobserved completion is not a proven unfinished call

**Section:** RS §8, T10's completion table, published in-flight call start, and lower-bound rules.

The new timestamp definitions resolve RS3-01. Keeping observed flag latency and total separately, and withholding observation-to-return when the observer is late, is correct for the stated diagnostic.

A remaining inference in the timeout table is not justified.

**Concrete permitted schedule:**

1. A competing call starts at 0 ms.
2. It returns, and its thread records its end timestamp at 1 ms.
3. The thread is descheduled before publishing that completion, clearing its in-flight slot or sending its report.
4. At the controller's 5-second deadline, only its start is visible.

The proposed receipt can call this an unfinished call of at least five seconds, although the measured call interval is one millisecond. The stopping thread has the same reporting gap. A start timestamp in an atomic does not make the subsequent call-return/report sequence atomic.

**Required bounded correction:**

- At the deadline, report *completion not observed*, *participant report missing* or *observer result missing*, according to what the controller actually knows.
- Keep a known start timestamp and the controller's elapsed waiting interval, but do not call that interval a lower bound on function duration merely because no completion was published.
- Calculate a measured duration only from an available start/end pair. State the meaning of any separately published progress marker.
- Preserve INCOMPLETE, all raw evidence and the prohibition on treating missing values as zero. No successful cost comparison follows from an incomplete run.
- Add a deterministic T10a case in which completion is timestamped before the deadline but its publication/report is delayed beyond it. This is a receipt-state test, not another production concurrency hook.

**Related deadline condition:** the readiness handshake occurs before the present five-second deadline starts. Give that handshake a stated controller-side deadline too. A participant that never reports readiness must not leave the controller waiting forever before the measurement deadline exists. Record this as readiness failure, not a stop-latency sample.

A thread's report also must not be treated as permission for a potentially unbounded join. Retain the no-blocked-join rule through teardown. The dedicated benchmark invocation may end with detached participants after an incomplete run; that does not mean the individual threads were cancelled. No new process supervisor or production hook is required by this ruling.

### RS4-02 — MEDIUM qualification: the load-average filter is not sufficient comparability evidence

**Section:** RS §8, host conditions and the proposed difference of at most 1.0.

**Counterexample to sufficiency:** two runs have the same named model loaded/running and nearly identical starting one-minute load averages, but materially different activity during the sample campaign. Both satisfy the written test while competing demand differs. The intermediate observations already requested by RS must not be ignored merely because the starting number passes.

Accept 1.0 as a provisional screening threshold, not as a proof that baseline and candidate conditions were equivalent. Use a label such as *recorded preconditions matched*. Review the full recorded workload/load histories, sample groups, tails and instrumentation before deciding whether the comparison is useful. A material observed change can make it NOT COMPARABLE despite a passing start threshold. Do not repeatedly run until a favourable distribution appears or discard earlier receipts.

No stronger statistical claim is required for this diagnostic investigation. The Owner retains the cost decision, including whether explicitly limited evidence is adequate.

### Non-blocking measurement clarifications

**Section:** RS §8, logging and aggregation.

The two test-side flags bracket the measured interval. Use them as a conservative candidate-log filter; the timestamp interval test remains the definition of overlap. Keep boundary candidates and do not count a call occurring only between a marker and the actual start/return timestamp as measured overlap. T10a's boundary cases should exercise the same calculation T10 uses.

“No allocation during the sample” can describe the preallocated logging path, not the unmodified calls being measured: the checkpoint's `open` constructs an Arc-backed entry before locking. Do not change production allocation behaviour to satisfy a benchmark sentence.

The minimum of observed flag latency and total is a useful upper-bound diagnostic for this workload. Its per-sample value is never above total; with the same sample set and percentile convention, passing the total target already implies passing that bound's target. It is not an independent safety test. State a deterministic percentile convention and keep the fewer-than-100 rule.

### RS §12 — all three answers

**1. Contract: AMEND.** Retain candidate A, the five-file scope proposal, the read-only early check, final compare-and-clear, poison-first refusal and previous-stop-lifted detail. The requested correction is to measurement/reporting, not the production ordering design.

**2. T10's narrowed interpretation: ACCEPT; its remaining timeout inference: AMEND by RS4-01.** It measures externally observed intervals, not exact lock wait or sweep duration. The late-observer arithmetic and separate overlap groups now answer the previous findings. Required runtime receipts still do not exist.

**3. Comparability rule/figure: ACCEPT AS A SCREENING HEURISTIC ONLY; AMEND ITS SUFFICIENCY CLAIM as RS4-02.** The starting difference of 1.0 neither proves equivalent load nor overrides the full evidence and Owner decision.

T8's source-bound tripwire plus documented independent source argument remains accepted. T11's poison-first order, nonzero count fixtures, three resume lock sites and retention of poison are accepted at contract level. These are requirements for the actual candidate, not tests passed here.

## 4. D2A1-CONTRACT-02 — all five answers

### 4.1 Q1 — overall verdict

**ACCEPT, for the dormant, dependency-free protocol-core contract.** The accepted scope remains CORE §3.2's six files and its non-activation restrictions. Owner scope/mission authorization is still required.

The contract now distinguishes transaction outcome from semantic refusal, retains per-attempt handover accounting, specifies one-word publication, preserves uncertainty across retries and gives cell operations an exact ownership/occupancy check. Its prospective tests, selective controls and stop-on-failure rules remain essential. Acceptance means the bounded work can be considered for authorization; it does not mean those requirements have already passed.

The exact declaration skeleton remains uncompiled. The authorized implementer, if any, must stop on a material signature/scope mismatch rather than redesigning the interface silently. The seven properties without their own controls remain disclosed assurance limitations.

No SQLite, shell, B2/B3, SCH or RS implementation is admitted into this slice. The dormant-module allowance, scope-qualified interim label and investigation ceilings retain the previous rulings. No interim label is awarded now.

### 4.2 Q2 — shared-state binding / PC-01

**ACCEPT — PC-01 RESOLVED AT CONTRACT LEVEL.** A retained strong Arc to the same allocation removes the wrapping ledger-number issue. While an old bound value survives, its allocation survives; another freshly allocated ledger does not become that allocation.

The implementation must compare allocation identity, such as Arc::ptr_eq, not equality of contents or of the published word. Keep the allocation fixed for the ledger's lifetime and do not create independent mutable cores sharing one identity/word. T-BIND remains required, including an old worker/token after its core is dropped. Cell occupancy exhaustion remains a separate checked boundary; removing the instance counter does not remove it.

No forced 2^64-ledger exercise is required. This is a retained-identity argument plus boundary tests, not an allocator stress benchmark.

### 4.3 Q3 — exit retained in U / PC-03

**ACCEPT — PC-03 RESOLVED FOR THIS SLICE.** U now retains ReturnedUncertain versus Unwound, and the correction carries that distinction. InCall must not survive as a normal externally observable U state; the stated latch covers an invalid occurrence.

W13 must demonstrate the two histories against the actual implementation. This accepts preservation of the missing discriminator, not the complete family/stage correction wording. The full contract still owes the immutable content and encoding, and must not infer completion of a caller-side cleanup path merely from a recorded exit.

### 4.4 Q4 — P8 and re-offers / PC-02

**ACCEPT THE RESTATED PREDICATE AND TEST-DOMAIN SPLIT.** P8 applies to the relevant insert-issuing attempt; a later correctly reported successful attempt can supersede that coverage requirement. An entry refusal of an already successful record does not need a fabricated correction. Re-offers remain table-tested, not covered by the listed unique-gating-caller explorations.

**PC2-N1 — LOW, explanatory wording only; CORE §7.3 P8:** “That can happen only after this attempt was resolved as absent” must include definite NotWritten, not only the resolver operation.

Example: an insert is attempted and fails; rollback is Undone; the call returns NotWritten and empties U. The barrier passes with U empty. A later offer commits. No U-resolution call occurred. CORE §4.5 already permits this history.

Read the explanatory sentence as: *a later successful attempt requires the earlier attempt's absence to have been established, either by definite NotWritten or by qualified resolution as absent.* The property predicate and operational table are unchanged. Correct this sentence in the controlled implementation baseline; it does not require another architecture round.

A test of every re-offer row is still owed. Neither this approval nor R4 establishes all concurrent re-offer histories, and no exactly-once external-execution guarantee follows.

### 4.5 Q5 — corrected gating record refused on re-offer

**ACCEPT AND KEEP THE RULE.** The gated-action operation may distinguish *this exact record is stored* from *this record can satisfy another attempt to proceed*. Once the verified correction exists, return the specified Corrected refusal before the ordinary Already present path.

Do not rewrite or hide the original durable row, call it absent, or change what its earlier append reported. Ordinary evidence lookup can still establish that it exists. A later action needs its own ordinary authority and a new record; correction settlement does not execute or revive the old action.

The later shell must obtain `corrected` from a verified correction bound to the exact original identity and digest, not an unchecked flag or an unrelated record with the same identity. T-GATE is required. This is accepted explicit store-side defence, not a claim that the old no-revival rule logically required this particular placement.

### PC-04 and the evidence boundary

**PC-04 is resolved at contract level.** CORE §6.3 now separates safety refinement from scheduler/CAS/storage progress. A2 names the two reads and the values retained from each. The explorer's key and independent-world reconstruction are specified rather than relying on cloned Arcs sharing a mutable word.

Source review must verify those premises in the real core and explorer. A finite search graph is not wait-freedom. Bounds, fault counts and omitted histories must remain visible; incomplete exploration is not PASS. Large runs remain serialized and conditional on host headroom, without stopping local-model workloads to create room.

## 5. RESPONSE-02 — the five forward obligations

### 5.1 §3.1 / RF-01 — ACCEPT AS CORRECTED OBLIGATIONS

The three facts now have the correct order: irreversible seal, close attempted, close confirmed. A failed/uncertain close or later qualified absence does not reopen registration or append authority. Opening still requires its confirmed record first.

The full contract must provide the private closing operation that is permitted after seal without reopening ordinary append entry points. The bare CORE transition is not a producer-quiescence proof. The entry-point map, truthful storage verbs and shell-omission/result-rewrite controls remain owed.

### 5.2 §3.3 / RF-02 — ACCEPT THE EXPLICITLY REVISED REPORTING GUARANTEE

The reporting query is now independent of the reconciliation mark. The earlier promise of an individualized restart notice is expressly withdrawn; the replacement offers historical acknowledgement rows through pagination.

Acceptance includes the stated limitation: below the verified mark, family-column corruption can omit a row, and the listing is not certified complete. Do not promote that listing into authority or an exact notification history. Within the supported verification range, §3.4 still applies.

The full contract must fix retention and pagination/cursor semantics so that ordinary concurrent appends do not invalidate the promised historical navigation. A row is guaranteed reachable only under the stated retention/integrity assumptions, not under the explicitly excluded corruption case. No additional Owner-reading gate is introduced.

### 5.3 §3.4 / verify-before-filter — ACCEPT AS AN OBLIGATION

The verified-stream or range-verified-index rule addresses false-negative selection, not just corrupt returned rows. The listed lookup, explanation, role, Prepared, acknowledgement, frontier and range-set reads are appropriately included. Keep tests that corrupt a selecting/indexed column and would otherwise cause omission.

This remains scoped to the supported range. The full contract still owes the actual index-validation and query implementation; D2a consistency is not B3's retention proof.

### 5.4 §3.6 / SQLite option B — ACCEPT THE SCOPE DIRECTION

Apply the local-confinement ruling already given: both its measurement predicate and its permitted ledger-storage row change, alongside the workspace site/file classification, own-file set, dependency pin and connection-ownership guard. Existing entries and equality checks remain.

Ordinary storage is not dynamic loading. Raw FFI/handle and loader routes remain prohibited, with alias/re-export and method-form tests. There is no new approval of a raw route simply because it is found elsewhere. The claimed absence inventory is not freshly certified by this review.

D-30a–d remain obligations, including actual linked-build assertions and an authorization-specific SQL-loader refusal. No test-binary result is substituted for application activation evidence. None of this enters the dependency-free CORE slice.

The response's question 3a is already answered; it requires no new Owner decision merely to restate the selected guard direction. Actual file scope and changes still require authorization.

### 5.5 §3.7 / RF-03 — ACCEPT THE OWNERSHIP DIRECTION; CORRECT ONE CROSS-REFERENCE

The table now separates failure before delegation, default-open failure, successful unpatched delegation, post-success harness failure, and successful patch installation. Its post-success failure branch gives cleanup to the harness before publishing a null pMethods. This addresses the specific ownership gap.

**RF03-N1 — LOW but mandatory in the later contract:** the Versions bullet says a newer table is refused “by the third row.” A newer method table discovered after successful default xOpen belongs to the **fifth** row, with the stated close/null/failure sequence. Do not leave contradictory cleanup instructions in the canonical harness contract. An unsupported VFS version rejected before delegation is a separate pre-open harness failure.

The full harness contract still must establish the base allocation's alignment as well as the tail offset, checked size, table lifetime and actual callback context requirements. Merely rounding the total to eight does not establish an arbitrary Rust state's alignment. Record the original xClose result and distinguish a cleanup/harness failure from an intended injected storage fault; calling cleanup once is not proof that every underlying resource operation succeeded.

These are retained unsafe-harness proof obligations, not authorization for unsafe code or a claim that the six exits were independently shown exhaustive. Callback-specific source arguments, concurrency/reopen isolation and R-a to R-e remain outstanding.

The §3.2 canonical-envelope/correction addition and §3.9 lock-order/composition addition are accepted as obligations. Their content is not supplied by the pure protocol's passing design review.

## 6. Historical wording and unauthorized scratch compilation

RESPONSE is an unchanged resend. Its missing-Owner-confirmation statements and its RS03 measurement summary are historical and superseded by the direct Owner statement, RS04 and this ruling. They do not undo approval or reinstate the discarded sweep calculation. Preserve document/version provenance rather than silently altering the resend.

RESPONSE §4 is accepted as a disclosure record. It reports the additional checker compilations and the fact that complete compiler output was not retained. It is not an independently verified compilation receipt, proof of host isolation or evidence for the pinned compiler. No retrospective authorization is created, and no compilation was repeated here.

Future scratch compilation remains a separate Owner permission question. No answer is inferred from the request to review these documents.

## 7. Model-source request

**NOT PERFORMED — ORIGINAL ARCHIVE STILL MISSING.** The specifications, addendum and references are not the archive or raw receipts.

State-key/visited-set handling, transition guards, fault budgets, predicate implementations, control mutations, termination/cycle analysis and receipt-to-source linkage remain uninspected. So does any needed r2/r3 comparison with ghost state projected away. No historical PASS “falls” through an inspection that did not take place; none becomes independently verified.

CORE explicitly does not depend on matching R4 counts and requires its own actual-core evidence. The missing historical archive therefore does not prevent this contract-only acceptance.

## 8. Gate state

- **CORE-02:** CONTRACT ACCEPTED for the dormant protocol-only scope, with PC2-N1's explanatory clarification. Owner six-file scope/branch/mission authorization remains pending. No implementation or interim label is accepted.
- **RS04:** AMEND / NOT APPROVED AS WRITTEN. Candidate A's timing choice remains approved; the later measured-cost decision remains the Owner's. Scope and implementation remain unauthorized.
- **RESPONSE obligations:** accepted as obligations with §5's corrections/qualifications, not as completed proofs.
- **Full D2a:** canonical self-contained contract not submitted/reported not written. No storage, durability or activation acceptance. R-a to R-e remain reported not started.
- **R4:** source/raw-evidence review pending; archive and combined-source hashes unverified.
- **Repairs 1 and 2:** accepted-component status carried forward, not freshly recertified; final-tree regressions remain mandatory.
- **DR-01:** accepted HIGH finding; fourth-repair/primary-ledger direction approved; component implementation not authorized.
- **Authority:** Claude remains Lead Architect; ChatGPT audits designs/contracts; Codex independently reviews implementation evidence under the adopted arrangement. A reviewer does not independently accept a repair it authored.
- **SCH, P1–P5, eight-lease limit and later scopes:** no new approval.
- **Implementation, branches, guard changes, dependencies, missions, operational activation, integration, release, tagging and phase advancement:** NOT AUTHORIZED BY THIS REVIEW.
- **Phase Three:** NOT COMPLETE. **Phase Four:** BLOCKED.

Preserve frozen refs/acceptance records, the judge test, and NC-C9-P1-HEADER-SCROLLS-AWAY plus M146 on the final Candidate 11 tree. Targeted local development validation precedes an authorized Fast Local gate; hosted validation remains a final-gate action or requires a separate explicit exception. No validation is dispatched here.

The next RS response can be a bounded measurement/reporting correction; the production stop protocol need not be redesigned for these findings. No wholesale DESIGN-04 is required for the accepted CORE slice. The full D2a and later composed design retain their separate consolidation requirements.

Unknown outcomes remain unknown after acknowledgement and confer neither replay authority nor verified learning.

## Source pointers

The submitted-document citations throughout this report identify the hash-verified files in §0 and their sections. The previous review is P3-C11-R3-AMENDMENT-01-RESPONSE-02-REVIEW-01. The direct Owner decisions are in the current and preceding submissions, not in the stale resend table.

Selected GitHub source blobs: run.rs `500402ec6c79090415d30e93b0c2c9ce00e7535c`; phase3_tests.rs `3df737c2daefd33f6be8642042e0ba986841b0d5`. Fresh repository reads used the fixed commit in §2.

Primary API references consulted:

- Rust std::sync::Arc: https://doc.rust-lang.org/std/sync/struct.Arc.html — retained allocation and ptr_eq, not equality of contents.
- Rust std::thread::JoinHandle: https://doc.rust-lang.org/std/thread/struct.JoinHandle.html — completion, joining and detachment distinctions.
- Rust std::thread::spawn: https://doc.rust-lang.org/std/thread/fn.spawn.html — dropping a handle detaches rather than cancels.
- SQLite sqlite3_vfs: https://sqlite.org/c3ref/vfs.html — versioned VFS structure, file-object size and xOpen output contract.
- SQLite sqlite3_io_methods: https://sqlite.org/c3ref/io_methods.html — failed-open close behaviour and method-table lifetime.

These are supporting API references, not execution evidence for the proposed code or the pinned SQLite build.

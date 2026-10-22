# Candidate 11 — nonclaims and limits (P3-C11-M7 N4)

What Candidate 11's evidence does **not** establish. A passing test, a killed mutant or an accepted ruling is never
read beyond what is stated here. Citation format and path prefixes are explained in `CLOSURE-RECORD.md` §1; sources
are listed at the end of this file.

## 1. Historical-memory contract (HM CONTRACT-02 R2)

The adopted contract lists its own nonclaims [S25 L1576-L1607]. In short, it does not claim or provide:

1. isolation of any kind: no OS egress control, sandbox, or process, filesystem or network confinement, and no proof
   that no other library, process, DNS lookup, proxy, crash reporter or telemetry path sends data;
2. any assurance about what a provider does, including whether a "local" endpoint forwards data beyond what D-2 R2
   verifies, provider-side retention or deletion, or withdrawal of anything transmitted;
3. instantaneous revocation (an admitted request may still be sent);
4. coverage of every store (document RAG, embeddings, `nexus_agent_memory`, simulation worlds, Builder content,
   `hivemind_sessions` and `consent_queue` display content are class XS: classified and visible, not label-governed);
5. audit-content safety and release of the `loops` / `dream_engine` locks before their implementation items land;
6. protection against UI-mediated resubmission of displayed content;
7. that the kept `external_action_turn` serialisation is a fix;
8. a runtime proof of the native Flash library's behaviour (static evidence only);
9. permission to disclose content derived from a granted record;
10. protection against a same-OS-user attacker with write access to the database or binary;
11. reliance on the egress governor;
12. HTTPS witnessing beyond the configured plain route and the test-only capture seam;
13. any claim about Candidate C;
14. that reference evidence is enforcement.

Its locks section adds what is not claimed resolved [S25 L1270-L1276], and its platform section records that native
confirmation on Windows and macOS is unverified and that a network filesystem is unsupported [S25 L1418-L1428].

## 2. EL-1 (Phase Three evidence ledger) section 9 and its amendments

The EL-1 contract text, including section 9, is not locally available to these records [N11]. What can be cited:

- B4 credential observations are an explicit EL-1 nonclaim; existing credential isolation and pre/post-vault-read
  liveness requirements stay in force [S07 L52]. The SCH nonclaim is carried with it [S03 L47].
- M6 N-3's later HiveMind cancellation is an EL-1 nonclaim and must not be described as immediate termination of all
  background or model work; the source-pin test covers the registry's current call forms, not arbitrary future Rust
  syntax [S07 L52].
- The M6 additions N-1..N-8, as recorded by the M6 executor "as given, not owed", are [S28 L283-L295] (the original
  M6 text is not locally available [N20]):
  - N-1 the store file is created with O_EXCL and then opened by path; a same-OS-user rename in between is outside
    the model;
  - N-2 after creation only the store directory is synced, not its parent;
  - N-3 the desktop's emergency stop cancels HiveMind sessions only after the Phase Three stop has written its
    records;
  - N-4 an effect refused after its Started record committed returns its error without an evidence field;
  - N-5 two concurrent `p3_resume` calls may each write an acknowledged row with the same `covers_through`;
  - N-6 a run is registered before its RunOpened record; a stop in between may write RunCancelled for a run whose
    RunOpened was refused;
  - N-7 shutdown writes `session_closed` after its wait even if an effect still executes; that effect's missing end
    is caught as unsettled at the next start;
  - N-8 the grant dialog can be shown while admissions are refused; the grant is then refused at its record.
- The code at N1 restates two section 9 limits: the store's tag is a SHA-256 chain, not a MAC, a same-OS-user writer
  is outside the threat model and tail truncation by such a writer is not detected [G20 L53-L55]; a store whose
  creation failed is refused, never initialised again in place [G20 L707-L708].

## 3. Platform

Linux only (OD-PLAT-1) [S07 L285] [S03 L47]. Windows and macOS behaviour is not claimed by any Candidate 11 receipt;
earlier missions recorded local Linux evidence only [S31 L141] [S30 L469].

## 4. Dormant D2A-1 components

The D2A-1 protocol core and test-only shell remain dormant [S07 L62]. Their 70 control records are historical CARRY
evidence: they do not claim that a historical executable was rebuilt at N1 [S07 L62]. No D2A-1 campaign or witness
run was part of N3 [S06 L294].

## 5. Review independence

Same-model historical review is declared (GD-01) [A01] [S02 L50] [S03 L47]. Executor semantic reviews of C10 controls
were accepted by the Architect's separate ruling, not by the executor [S04 L32] [S03 L39].

## 6. Mutation evidence that carries no current credit

- Seven historical mutations whose text is unavailable keep their reported outcomes with zero current mutation-proof
  credit and are excluded from runnable and current-killed denominators: M2-HM-M2/N1-seal-negative-control,
  ADHOC-NC/D-2-NC, ARCHITECT-NO-TEXT/M5-review-killed-1 and -2, ARCHITECT-NO-TEXT/M6-review-Y4, -Y5 and -Y15
  [S07 L151-L158].
- HIVEMIND-INFOFLOW/M11L is a historical intentional survivor with no current kill credit [S07 L135].
- HIVEMIND-PRIVACY-CLOSURE/M10 and M11 are RETIRE: their mechanism was removed; the privacy requirement remains and
  these old patches are not claimed killed at N1 [S07 L147].
- The M6 X1 record is a zero-credit lineage reference to the single N1 execution [S06 L85].

## 7. Source, pin and runtime-proof limits

- Each accepted outcome is accepted for its original rule and actual scope. Structural-pin and scanner kills are not
  runtime exploit proofs; a diagnostic-ordering kill does not imply a successful unauthorized effect [S04 L32].
- C10-183/M42 is an evidence-ordering/registry-state proof, not an external-effect demonstration; C10-183/M172
  claims no model inference [S03 L36-L37].
- M3 N6-M01c keeps its original-rule kill without any claim of observed cloud leakage [S07 L87].
- The accepted synthetic tool checks are tool-behaviour evidence for their exercised cases only, not proof of
  unexercised runtime paths [S06 L76].
- N3 ran at N1. The N4 commit adds documentation only; N3 receipts are N1 evidence and are not relabelled as runs on
  the N4 commit [S03 L130].

## 8. Observation gaps

- No packet capture, and the AM4 server-side cancellation limit, stay recorded for M172 [S03 L37].
- Runtime HOME and general /tmp byproducts of AM5 were not inspected; that is a limitation, not a claim that those
  files are unchanged [S03 L45].

## 9. Restricted material

The original M7 checkpoint archive is RESTRICTED because it contains the scratch-configuration member: referenced by
digest only, never copied [S03 L58] [S50]. No plaintext or encrypted runtime configuration, runtime database, prompt,
credential, target directory or `node_modules` is part of these records [S03 L58].

## Sources

| ID | Kind | Locator | Digest | Lines |
|---|---|---|---|---|
| S02 | FILE | EV/amendment-7/AMENDMENT-7.txt | e2989abb04a40a83608f14eefda3ccbcc09c41474c01c06b3d21109673d8dabc | L50 |
| S03 | FILE | EV/amendment-6/AMENDMENT-6.txt | f57679259c33171ed48517b224a46dca8440f007794e731764e51fb7195ab6e3 | L36-L130 |
| S04 | FILE | EV/amendment-5/AMENDMENT-5.txt | f91c53476e61ebe231666d1eab3417bb6b56d557038e3aca5f7dc2cb7e78b709 | L32 |
| S06 | FILE | EV/amendment-3/AMENDMENT-3.txt | 186a92b5eda216ca783ea8eacf8e11fdcae374232472609c6cf24689f874a522 | L76-L294 |
| S07 | FILE | EV/amendment-2/AMENDMENT-2.txt | d681a462db614af06d045bf4cea1eb856f40b88f5a40504e7f6bad6a4dbff2e8 | L52-L285 |
| S25 | FILE | AUD/p3-c11-hm-contract-02-r1-final-correction-01/P3-C11-HM-CONTRACT-02-R1-FINAL-CORRECTION-01/02-P3-HM-CONTRACT-02.md | c030317bf048e64109c533b395585675ca947d6cc3039dd8e253aeef6d1144be | L1270-L1607 |
| S28 | FILE | AUD/p3-c11-m6-el1-corrections-01/REPORT.md | 03563669cdc3cc9b70161d43db259f22f8cc9a312099427141a5704f6e9c2690 | L283-L295 |
| S30 | FILE | AUD/p3-c11-hm-m3-integration-01/REPORT.md | 8998381246b4cdfca91bd671904a0b08475b72f1921ec6ee15d0170519444357 | L469 |
| S31 | FILE | AUD/p3-c11-hm-m2-authority-01/REPORT.md | 43f967aca6b59d6ca1f143a84a29bafb59ed14fbd824a96e99519a70a5a7593f | L141 |
| S50 | FILE | EV/M7-CHECKPOINT.tar.gz | 260dec4127b7bb513f08c779c46cea20a6db927ddcc55464119558bc0e7ff190 | - |
| A01 | ATTESTED | P3-C11-MISSION-TEXTS-AND-RULINGS.txt, 129920 bytes (OWNER-SUPPLIED / ARCHITECT-VERIFIED / ORIGINAL NOT LOCALLY MATERIALIZED) | b93135fe75b12b303a25e1d818c301039799261c2120acfe15504423369e23ee | L782-L797 |
| G20 | GIT-BLOB | 567cbc65a3998be6e4c3148520abf51314a6fd22:crates/nexus-governed-control/src/evidence_store.rs | blob 76a41ecfd78ba33b9f5a90560aa649980c6ff84e | L53-L708 |
| N11 | NOT-FOUND | EL-1 contract text (sections 1-9): only local copy is inside the closed session-transcript route | - | - |
| N20 | NOT-FOUND | M6 mission text containing N-1..N-8: only local copy is inside the closed route | - | - |

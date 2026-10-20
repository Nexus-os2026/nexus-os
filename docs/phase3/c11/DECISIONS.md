# Candidate 11 — decisions and ruling chain (P3-C11-M7 N4)

This file records which decisions govern Candidate 11 and where each one can be read. It does not create, widen or
reinterpret any decision. Citation format and path prefixes (`EV/`, `AUD/`) are explained in `CLOSURE-RECORD.md` §1;
the sources are listed at the end of this file.

Three kinds of authority are kept apart:

- **Original** — the decision's own text, read from a primary record.
- **Transmitted reaffirmation** — a later Architect text, saved as a file, that restates or carries the decision
  forward. It is cited as such, never as the original.
- **Executor record** — an implementing session's report of how it applied a decision. It shows application, not
  authority.

Many original rulings exist on the development machine only inside Claude Code session transcripts. That route is
closed for these records [S01 L15-L21]. Such originals are marked "not locally available" (N rows), and only a
transmitted reaffirmation or executor record is cited [S01 L49].

## 1. Standing Owner decisions

| Decision | Primary text available | Transmitted reaffirmation | Executor record |
|---|---|---|---|
| AD-1..AD-7 (adoption of HM CONTRACT-02 R2, schema, loosening, declassification, T₀ baseline, capture seam, implementation) | The contract defines each item and its default [S25 L1609-L1637]; the Owner's signed adoption record is not locally available [N16] | Carried forward [S07 L285] [S06 L294] [S03 L47] | FK01 records AD-1..AD-7 as adopted per its mission [S33 L22] |
| OD-PLAT-1 — Linux-only support profile | Not locally available [N16] | Carried forward [S07 L285] [S03 L47] | Linux only, OD-PLAT-1 [S30 L469] [S29 L310] [S28 L342] |
| GD-01 — same-model historical review declared | Not locally available [N17] | R-M6-1 reaffirmation [A01] [S02 L50] [S01 L51]; carried forward [S03 L47] | — |

## 2. Contract and design rulings

**EL-1 (Phase Three evidence ledger) and R-DR01-1.** The EL-1 contract text and the R-DR01-1 ruling were delivered in
the M5 mission text, which is not locally available to these records [N10] [N11]. Transmitted reaffirmation: EL-1 and
R-DR01-1 remain accepted for Candidate 11 [S07 L52], kept by AM3 [S06 L79] and carried forward by the N3 acceptance
[S03 L47].

**B4 and the former DESIGN-03 AMD section 8 item 4.** DESIGN-03 AMENDMENT-01 §8 item 4 stated that B4 is required for
final Candidate 11 acceptance and may follow activation but may not be omitted [S26 L483]. That requirement was
superseded: the Architect recorded that AMD section 8 item 4 "was superseded by the Owner-adopted M5 contract" and that
B4 credential observations remain an explicit EL-1 nonclaim [S07 L52]. The supersession is R-DR01-1's [S03 L74]
[S01 L57], kept by AM3 [S06 L79]; the former requirement is preserved here as superseded history, not deleted [S09 L27]. Existing credential
isolation and pre/post-vault-read liveness requirements remain in force [S07 L52].

**SCH.** DESIGN-03 AMENDMENT-01 places SCH before activation and outside D2a [S26 L474] [S26 L482] and states that SCH is
not contracted [S26 L503-L505]. The SCH nonclaim is carried forward with B4's [S03 L47]; its wording inside EL-1 is not
locally available [N11].

**HM CONTRACT-02 R2.** The adopted historical-memory contract is the R2 text [S25]; its nonclaims are summarised in
`NONCLAIMS.md`.

## 3. Interpretive rulings named by the N4 instructions

| Ruling | Original | Transmitted reaffirmation | Executor record of application |
|---|---|---|---|
| E-M3-1 (classification of A-S4 sites) | Not locally available [N12] | None found in saved amendment files | M3 A-S4 table built under E-M3-1 [S30 L161-L163] |
| E-M4-1 | Not locally available [N13] | None found in saved amendment files | None located |
| E-M4-2 (placement of the loosening confirmer) | **NOT FOUND** on this machine [N14] | Its unavailable original remains NOT FOUND, with the prior Architect reaffirmation kept separately [S07 L157]; its reaffirmed meaning may be attributed honestly [S02 L52] | Confirmer in `app/src-tauri/src/hm_policy.rs` affirmed; standalone text NOT FOUND [S09 L31-L32]; executor facts of the placement [S27 L148] [S27 L163] |
| OriginLabel clarification | Not locally available [N15] | None found in saved amendment files | The contract constraint it interprets [S25 L941] [S25 L953]; question raised in M2 [S31 L144-L152]; A1 form [S32 L37]; M4 rule applied: no new public fn takes OriginLabel and the related types as a decision input [S27 L149] |
| R-M5-1 | Not locally available [N18] | — | M6 ran under it [S28 L4] |
| R-M6-1 | Owner-supplied document, OWNER-SUPPLIED / ARCHITECT-VERIFIED / ORIGINAL NOT LOCALLY MATERIALIZED, lines 782-797 [A01] | Reaffirmed in AM7 [S02 L50] and AM8 [S01 L51] | — |

## 4. M7 amendment and ruling chain

| Step | Saved text | What it decided (summary; see the cited lines) |
|---|---|---|
| Original M7 mission | Owner-supplied, lines 896-910 for N4/N5 [A01]; local copy closed [N19] | N4/N5 scope (attested, not quoted) [S02 L48] |
| AM1 | Text not locally available [N19] | Executor record of the rulings applied [S09 L23-L33] |
| AM2 | [S07] | R1 N1 accepted; EL-1/R-DR01-1; B4 nonclaim [S07 L49-L53]. R2 80 carries [S07 L54-L67]. R3 15 re-anchors and the X1 alias [S07 L68-L90]. R4 RS-01 adaptations [S07 L91-L97]. R5 recovered campaigns, two RETIRE [S07 L98-L150]. R6 seven unavailable mutations, zero credit [S07 L151-L158]. R7 596 records, pool 445 [S07 L159-L161] |
| AM3 | [S06] | R1 pilot accepted [S06 L49-L70]; R2 deviations [S06 L71-L77]; R3 existing decisions stand [S06 L78-L81] |
| AM4 | [S05] | AM3 healthy failure and STOP accepted as correctly recorded; USERNAME restored; 596/445 unchanged [S05 L44-L52] |
| AM5 | [S04] | 441 AM4 controls accepted for their original rules and actual scope; M42/M172 alone open [S04 L32-L37] |
| AM6 | [S03] | P3-C11-M7-N3-ACCEPTANCE-01: N3 accepted at N1 [S03 L32-L48]; N4/N5 specification [S03 L49-L131] |
| AM6 STOP | [S16] [S17] | Native denial I6-01; Owner chose STOP (P6-01) |
| AM7 | [S02] | Closed transcript route; R-M6-1 attestation [S02 L44-L52] |
| AM7 STOP | [S18] | Native denial I7-01 |
| AM8 | [S01] | Independent tools; same N4/N5 specification [S01 L11-L13] |

The N3 acceptance accepted each outcome for its original rule and actual scope; structural-pin and scanner kills are
not runtime exploit proofs, and a diagnostic-ordering kill does not imply a successful unauthorized effect
[S04 L32]. Acceptance is a separate ruling, never a fabricated runner PASS [S03 L39].

## Sources

| ID | Kind | Locator | Digest | Lines |
|---|---|---|---|---|
| S01 | FILE | EV/amendment-8/AMENDMENT-8.txt | 41d9c7158a12957a16c3dd5a0fd791d1a5693cd2194d8def8b7c94a7d0758209 | L1-L114 |
| S02 | FILE | EV/amendment-7/AMENDMENT-7.txt | e2989abb04a40a83608f14eefda3ccbcc09c41474c01c06b3d21109673d8dabc | L44-L52 |
| S03 | FILE | EV/amendment-6/AMENDMENT-6.txt | f57679259c33171ed48517b224a46dca8440f007794e731764e51fb7195ab6e3 | L32-L131 |
| S04 | FILE | EV/amendment-5/AMENDMENT-5.txt | f91c53476e61ebe231666d1eab3417bb6b56d557038e3aca5f7dc2cb7e78b709 | L32-L37 |
| S05 | FILE | EV/amendment-4/AMENDMENT-4.txt | 5903fe5afa627f9c18074ed90d974dcef87b2e541deffaf06b72f6470dedbf8e | L44-L52 |
| S06 | FILE | EV/amendment-3/AMENDMENT-3.txt | 186a92b5eda216ca783ea8eacf8e11fdcae374232472609c6cf24689f874a522 | L49-L294 |
| S07 | FILE | EV/amendment-2/AMENDMENT-2.txt | d681a462db614af06d045bf4cea1eb856f40b88f5a40504e7f6bad6a4dbff2e8 | L49-L285 |
| S09 | FILE | EV/amendment-1/REPORT.md | 0e45348846bb1b02a7e7c835c49fffb4e6d060d8fc4efda83d8b1f9107bf0139 | L23-L33 |
| S16 | FILE | EV/amendment-6/incidents/I6-01-native-denial.txt | 3630c26015b2246cd7a8df2dba25db08cab5741f4ba03199961e72aabb4a2f6b | L1-L14 |
| S17 | FILE | EV/amendment-6/permission/P6-01-owner-decision.txt | 3bdb1f6e33124bb16baeec874cb639add11917f589da6f0f25f781a9e6ac04a7 | L1-L18 |
| S18 | FILE | EV/amendment-7/incidents/I7-01-native-denial.txt | a699780549d3a9d3e6c466ec378b2f4940ea2d30f523a7c0613aa0d219c96627 | L1-L23 |
| S25 | FILE | AUD/p3-c11-hm-contract-02-r1-final-correction-01/P3-C11-HM-CONTRACT-02-R1-FINAL-CORRECTION-01/02-P3-HM-CONTRACT-02.md | c030317bf048e64109c533b395585675ca947d6cc3039dd8e253aeef6d1144be | L1-L1750 |
| S26 | FILE | AUD/p3-c11-48h-highloop-01/continuation-02/archive-extract/P3-C11-R3-DESIGN-03-AMENDMENT-01.md | 1cb65c46828f84db287736e5281307c06db008e3b453b749bc93192765eac322 | L466-L505 |
| S27 | FILE | AUD/p3-c11-hm-m4-governance-01/REPORT.md | 9f89ee52872de4d9d381924865d2c6771398e89926f50cae20f40dfdae0c5c63 | L148-L163 |
| S28 | FILE | AUD/p3-c11-m6-el1-corrections-01/REPORT.md | 03563669cdc3cc9b70161d43db259f22f8cc9a312099427141a5704f6e9c2690 | L4-L342 |
| S29 | FILE | AUD/p3-c11-m5-evidence-ledger-01/REPORT.md | 0198841f073c659def2f2a49ff62e55072480e31db7ba29321de5b283b6c4517 | L310 |
| S30 | FILE | AUD/p3-c11-hm-m3-integration-01/REPORT.md | 8998381246b4cdfca91bd671904a0b08475b72f1921ec6ee15d0170519444357 | L161-L469 |
| S31 | FILE | AUD/p3-c11-hm-m2-authority-01/REPORT.md | 43f967aca6b59d6ca1f143a84a29bafb59ed14fbd824a96e99519a70a5a7593f | L144-L152 |
| S32 | FILE | AUD/p3-c11-hm-a1-ledger-01/REPORT.md | 559327764ab1acabe2e7e091457a21ed4b4056e0b86a3ad0bd2ae8ad234770c0 | L37 |
| S33 | FILE | AUD/p3-c11-hm-fk01-inmemory-foreign-key-01/P3-C11-HM-FK01-INMEMORY-FOREIGN-KEY-01-REPORT.md | 0139565b4a282cbc31c0db9bda67c93548d231018835acd1cef04efef1dbfeee | L22 |
| A01 | ATTESTED | P3-C11-MISSION-TEXTS-AND-RULINGS.txt, 129920 bytes (OWNER-SUPPLIED / ARCHITECT-VERIFIED / ORIGINAL NOT LOCALLY MATERIALIZED) | b93135fe75b12b303a25e1d818c301039799261c2120acfe15504423369e23ee | L782-L910 |
| N10 | NOT-FOUND | R-DR01-1 original text: only local copy is inside the closed session-transcript route | - | - |
| N11 | NOT-FOUND | EL-1 contract text (sections 1-9, incl. the B4/SCH nonclaim wording): only local copy is inside the closed route | - | - |
| N12 | NOT-FOUND | E-M3-1 original text: only local copy is inside the closed route | - | - |
| N13 | NOT-FOUND | E-M4-1 original text: only local copy is inside the closed route | - | - |
| N14 | NOT-FOUND | E-M4-2 original ruling text: not on this machine at all | - | - |
| N15 | NOT-FOUND | OriginLabel clarification original text: only local copy is inside the closed route | - | - |
| N16 | NOT-FOUND | Owner adoption record for AD-1..AD-7 and the OD-PLAT-1 text: only local copies are inside the closed route | - | - |
| N17 | NOT-FOUND | GD-01 original text: only local copies are inside the closed route | - | - |
| N18 | NOT-FOUND | R-M5-1 original text: only local copy is inside the closed route | - | - |
| N19 | NOT-FOUND | Original M7 mission text and M7 Amendment 1 text: only local copies are inside the closed route | - | - |

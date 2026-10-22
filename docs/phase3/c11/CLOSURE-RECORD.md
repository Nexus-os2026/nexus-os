# Candidate 11 — closure record (P3-C11-M7 N4)

Status of this record: **N4 documentation, prepared for Architect review.** It records what is established up to and
including the Architect's N3 acceptance. It is not an acceptance of M7, of Candidate 11 or of Phase Three.

| Stage | State |
|---|---|
| M7 N1 (D-1/D-2 correction, commit `567cbc65`) | ACCEPTED by the Architect [S07 L49-L50] |
| M7 N2 (596-record control register) | Materialised from the Architect's rulings R1-R7 [S07 L159-L161] |
| M7 N3 (control campaign) | ACCEPTED at N1 by ruling P3-C11-M7-N3-ACCEPTANCE-01 [S03 L32-L39] |
| M7 N4 (these records, one local documentation commit) | IN PREPARATION; this file is part of it [S01 L53-L65] |
| M7 N5 (static gates and six package test scopes on the N4 commit) | **PENDING** — not run when this record was written [S01 L67-L91] |
| Publication, Fast Local CI, hosted CI + Security Audit, merge, checked activation, Owner's final declaration | **PENDING** — each a separately approved later stage [S01 L114] [S07 L289] |

No N5 result is stated or predicted here. The N5 evidence, when it exists, is recorded outside this tree and
identifies the exact N4 commit [S03 L80].

## 1. How to read the citations

Every factual statement carries a bracketed source id. `[S03 L32-L39]` means lines 32 to 39 of source S03 in the
table at the end. Path prefixes: `EV/` = `/home/nexus/nexus-audits/claude-architect/p3-c11-m7-final-proof-01/`,
`AUD/` = `/home/nexus/nexus-audits/claude-architect/` (evidence directories on the development machine; not part
of this repository). `G` rows are Git objects of this repository. `A01` is an external Owner-supplied source that is
not on the development machine. `N` rows are records that could not be located in material available to the
amendment that wrote this file.

The original Claude Code session transcripts are a closed source for these records [S01 L15-L21]. Where a ruling's
original text exists locally only inside that closed route, it is marked as not locally available and only a
separately identified, transmitted reaffirmation is cited [S01 L49].

## 2. Authenticated lineage

All commits below are ancestors of N1 `567cbc65` (checked with `git merge-base --is-ancestor`); ids and trees are
the Git objects themselves.

| Milestone | Commit | Tree | Source |
|---|---|---|---|
| Candidate 11 GitHub head named by the M7 mission | `862a11cef284c5e3c34234dfd2677a5462c86d5d` | `83a234b6ea7ff446b739e263b45cd9b69f08d5e7` | [G01] [S08 L11] |
| RS-01 candidate | `e71b982b420a51fd131aeb50367aca7b7477a3e6` | `3e023d583914ee37b3bfa0089eb7ddf341e5db3c` | [G02] [S08 L12] |
| D2A-1 core, introducing commit | `22ebf9a5c5b16bb6f5169daf3731a3417c5289d3` | `b4a93dc6604a2a4e8cf5ae9fe455d8185b40f01b` | [G03] [S08 L14] |
| D2A-1 core, Candidate C (frozen proof source) | `d5964ee5326c2733177307db6f507328023d361a` | `51bac5fd335999cc995c0ab9f582a30244973ef0` | [G04] [S08 L13] |
| G2 | `4dc9a427dd3e0c183e8eec3eeb4b887e422fbd79` | `0383aa5c759b07b8c6f3b2a280ec6e2e113b2e61` | [G05] [S08 L15] |
| FK01 (HM-FK01 in-memory foreign keys) | `15bc5a6792f97a7f2a4a0d34763dfba86b3744aa` | `629472852cc2e2fa715ae13753604049196f7892` | [G06] [S08 L16] |
| M1 = HM-A1 | `95f55fcf981ef7f1507b516a1dd94a8d65f91347` | `7a6fb76f0e783f7a409990c0d37fcb17566c5d9c` | [G07] [S08 L17] |
| M2 = HM-M2 | `6c63d18ee26804d9bbc1d0900fc40e14e9d1b122` | `0375a37a2457e35d65258c271497df9a32417edc` | [G08] [S08 L18] |
| M3 = HM-M3 | `ece497f2beda9de3feeb54a656c2e35335f335b8` | `25162e7d48084023713a641694ea15d86ad9a043` | [G09] [S08 L19] |
| M4 = HM-M4 | `48a54343b00acec86f88b1c89e954687196f5980` | `91ae5a2a93528ec197e0896dce1b7a4d1953bda5` | [G10] [S08 L20] |
| M5 (EL-1 evidence ledger) | `808fa920954ce91787918e6469bbb05154a58540` | `df28b8b97aa96f34b061c8d2a262106072d423eb` | [G11] [S08 L21] |
| M6 (EL-1 corrections) | `650d110122511ed1d292d40dc359e66d752fc2f3` | `a3b27c8ced8ee174c0afe7538ea2702ace89ff73` | [G12] [S08 L22] |
| M7 N1 | `567cbc65a3998be6e4c3148520abf51314a6fd22` | `eb57c41deee9261fd364f2eccac41bf07ac063c7` | [G13] [S08 L30-L34] |

The M7 N0 record also notes that the GitHub branch `repair/p3-candidate11` was observed at `b1d535f1`, which
descends from `862a11ce` and is an ancestor of M6; both identities were kept as recorded [S08 L450-L451] [G14].
The M7 branch is `dev/p3-c11-m7-final-proof`; N1 is its only commit after M6 at the time of this record
[S08 L29-L34].

## 3. Milestones, evidence digests and reviewer decisions

"Executor report" is the implementing session's own delivered report; it is evidence of what was done, not a
decision. Decisions are listed separately and only where a primary or transmitted record is available.

| Milestone | Executor report (status line) | Evidence archive | Reviewer decision |
|---|---|---|---|
| FK01 | [S33 L3] READY_FOR_ARCHITECT_REVIEW | [S40] | Original acceptance not locally available to this amendment [N02] |
| M1 HM-A1 | [S32 L4] READY_FOR_ARCHITECT_REVIEW | [S41] | Original acceptance not locally available [N02] |
| M2 HM-M2 | [S31 L3] READY_FOR_ARCHITECT_REVIEW | [S42] | Original acceptance not locally available [N02] |
| M3 HM-M3 | [S30 L3] READY_FOR_ARCHITECT_REVIEW | [S43] | Original acceptance not locally available [N02] |
| M4 HM-M4 | [S27 L3] READY_FOR_ARCHITECT_REVIEW | [S44] | Original acceptance not locally available [N02] |
| M5 | [S29 L3] READY_FOR_ARCHITECT_REVIEW | [S45] | Accepted together with M6 under R-M6-1 [A01] [S02 L50] [S01 L51] |
| M6 | [S28 L3] READY_FOR_ARCHITECT_REVIEW | [S46] | ACCEPTED at `650d1101`, tree `a3b27c8c`, carrying D-1 and D-2 into M7 N1 (R-M6-1) [A01] [S02 L50] [S01 L51] |
| M7 N0-N2 | [S08 L3] PAUSED_FOR_ARCHITECT_CONTROL_REVIEW | [S50] (restricted, see §6) | N1 accepted; register rulings R1-R7 [S07 L49-L161] |
| M7 AM1 | [S09 L3] PAUSED_FOR_ARCHITECT_CONTROL_REVIEW | [S51] | Corrected by AM2 rulings [S07 L47-L161] |
| M7 AM2 | [S10 L5] PAUSED_FOR_ARCHITECT_AM2_PILOT_REVIEW | [S52] | Pilot accepted (AM3 R1) [S06 L49-L70] |
| M7 AM3 | [S11 L6] STOPPED_AM3 | [S53] | Healthy failure and STOP accepted as correctly recorded [S05 L44-L52] |
| M7 AM4 | [S12 L5] PAUSED_FOR_ARCHITECT_N3_REVIEW | [S54] | 441 passing controls accepted (AM5 §1) [S04 L32-L37] |
| M7 AM5 | [S13 L5] PAUSED_FOR_ARCHITECT_AM5_REVIEW | [S55] | N3 ACCEPTED (P3-C11-M7-N3-ACCEPTANCE-01) [S03 L32-L48] |
| M7 AM6 | [S14 L5] STOPPED_AM6 | [S56] | Lineage preserved as a STOP [S02 L11] [S01 L11] |
| M7 AM7 | [S15 L4] STOPPED_AM7 | [S57] | Lineage preserved as a STOP [S01 L11] |

R-M6-1 is cited from the Architect's attestation of an Owner-supplied document that is **not** on the development
machine: `P3-C11-MISSION-TEXTS-AND-RULINGS.txt`, 129920 bytes, sha256
`b93135fe75b12b303a25e1d818c301039799261c2120acfe15504423369e23ee`, lines 782-797 —
OWNER-SUPPLIED / ARCHITECT-VERIFIED / ORIGINAL NOT LOCALLY MATERIALIZED [A01] [S02 L44-L50] [S01 L49]. This record
does not claim that document was read locally and quotes nothing from it. The reaffirmed content is: M6 accepted at
`650d1101` with D-1/D-2 carried into M7; M5 accepted with M6; A2-A7 accepted as applied; GD-01 declared
[S02 L50] [S01 L51].

## 4. M7: what N1 and N3 established

**N1.** Commit `567cbc65` makes every phase that ends an entry end a started commitment (D-1) and adds six tests
(D-1 and the D-2 Architect mutants) in `nexus-governed-control` [S08 L30-L33] [G13]. Six mutants were executed once
each on the N1 commit and killed (receipts 018-023) [S08 L98-L106]. The Architect accepted N1 at the pinned
identity, including the six unique mutation executions [S07 L50].

**N3.** The campaign pool is 445 controls [S07 L161]. Their accepted outcomes come from three executions:

| Part | Controls | Accepted outcome | Ruling |
|---|---|---|---|
| AM2 mandatory pilot (C9-52/NC-C9-P1-HEADER-SCROLLS-AWAY, C10-183/M146) | 2 | 2 KILLED | [S06 L49-L58] |
| AM4 campaign | 441 of 443 | 419 KILLED, 22 RS-01 AS SPECIFIED | [S04 L32-L35] |
| AM5 replacements (C10-183/M42, C10-183/M172) | 2 | 2 KILLED | [S03 L33-L37] |
| **Total** | **445** | **423 KILLED, 22 RS-01 AS SPECIFIED** | [S03 L39] |

N3 was accepted at N1 and its pinned tree [S03 L33]. M42's kill is an evidence-ordering/registry-state proof, not an
external-effect demonstration; M172's kill claims no model inference [S03 L36-L37]. The 80 historical carries and the
six unique N1 executions (seven lineage references) are kept separate from the 445; the 596-record register is
unchanged; replacements and aliases add no control [S03 L39]. The per-record register is
`docs/evidence/p3-c11/final-controls/` (README and data).

Preserved non-passing history, never relabelled: the AM3 STOP (required healthy prerequisite failed: 24 passed,
6 failed, "route policy unresolved: config_unreadable", ledger seq 5) [S11 L6-L19] [S24 L5]; the AM4 FINISH HOLD with
M42 COMPILE ERROR and M172 INVALID(PROCESS LEFT BEHIND) [S12 L5-L8] [S23 L2437]; the AM5 FINISH HOLD (C10 semantic
review pending by design) [S13 L5-L9] [S22 L24]; and the acceptance ruling's statement that these remain visible and
immutable [S03 L39].

Bounded AM5 deviations closed by the Architect for that evidence: two build-only restoration commands ran without
CI=1; three pre-seal amendment-local bytecode files were removed; the archived Amendment 5 differs from the delivered
file by one punctuation mark and the final LF; runtime HOME and general /tmp byproducts were not inspected, which
remains a limitation [S03 L41-L45].

## 5. STOP lineages inside M7

| Lineage | Status | Cause | Source |
|---|---|---|---|
| AM3 | M7_STOPPED_AM3 | Required healthy prerequisite failed; the sealed child environment lacked USERNAME | [S11 L6-L19] |
| AM6 | M7_STOPPED_AM6 | Native permission denial I6-01 on a session-transcript excerpt; Owner chose STOP (P6-01) | [S14 L5-L13] [S16] [S17] |
| AM7 | M7_STOPPED_AM7 | Native permission denial I7-01 on reading an AM6 helper's source | [S15 L4] [S18] |

AM6 and AM7 remain immutable STOP lineages and are not resumed by the amendment that writes this record
[S01 L11].

## 6. Restricted material

The original M7 checkpoint archive is RESTRICTED because it contains the scratch-configuration member; it is
referenced only by digest [S50] and none of its bytes are copied here [S03 L58]. No runtime HOME, configuration,
database, prompt, credential, target directory or `node_modules` content is part of these records [S03 L58]
[S01 L61].

## 7. Remaining gates

Still pending, in this order of the Architect's instructions: N4 review of these records; N5 (four static gates and
six package test scopes on the exact N4 commit) [S01 L67-L91]; then exact-ref publication, Fast Local CI, hosted CI
plus Security Audit, merge, checked activation and the Owner's final declaration, each separately approved
[S01 L114] [S07 L289]. Phases Zero, One and Two stay frozen; Phase Four is not started [S01 L114].

## Sources

| ID | Kind | Locator | Digest | Lines |
|---|---|---|---|---|
| S01 | FILE | EV/amendment-8/AMENDMENT-8.txt | 41d9c7158a12957a16c3dd5a0fd791d1a5693cd2194d8def8b7c94a7d0758209 | L1-L114 |
| S02 | FILE | EV/amendment-7/AMENDMENT-7.txt | e2989abb04a40a83608f14eefda3ccbcc09c41474c01c06b3d21109673d8dabc | L44-L52 |
| S03 | FILE | EV/amendment-6/AMENDMENT-6.txt | f57679259c33171ed48517b224a46dca8440f007794e731764e51fb7195ab6e3 | L32-L86 |
| S04 | FILE | EV/amendment-5/AMENDMENT-5.txt | f91c53476e61ebe231666d1eab3417bb6b56d557038e3aca5f7dc2cb7e78b709 | L32-L37 |
| S05 | FILE | EV/amendment-4/AMENDMENT-4.txt | 5903fe5afa627f9c18074ed90d974dcef87b2e541deffaf06b72f6470dedbf8e | L44-L52 |
| S06 | FILE | EV/amendment-3/AMENDMENT-3.txt | 186a92b5eda216ca783ea8eacf8e11fdcae374232472609c6cf24689f874a522 | L49-L70 |
| S07 | FILE | EV/amendment-2/AMENDMENT-2.txt | d681a462db614af06d045bf4cea1eb856f40b88f5a40504e7f6bad6a4dbff2e8 | L47-L289 |
| S08 | FILE | EV/REPORT.md | 031e583705045a67135862b138138ed89a899882d6f50fade4ecd718b69bb8b0 | L3-L451 |
| S09 | FILE | EV/amendment-1/REPORT.md | 0e45348846bb1b02a7e7c835c49fffb4e6d060d8fc4efda83d8b1f9107bf0139 | L3 |
| S10 | FILE | EV/amendment-2/REPORT.md | e916c30d5fd40e2c27377912fb8f54bf1106350f834a94b66c8c09814a325348 | L5 |
| S11 | FILE | EV/amendment-3/REPORT.md | ff6278c1b2375650a05c9db6a230e1760ea7cfb40d2e61a10bc5fa28c9eb1c09 | L6-L19 |
| S12 | FILE | EV/amendment-4/REPORT.md | 2871013bc3080359e2b3c80938b2e54d39322498844b75d9309c7add4e5567fb | L5-L8 |
| S13 | FILE | EV/amendment-5/REPORT.md | f87c6c03eb49c1cd64e7f10047aac1df756f1b5ca169546d8dcc181f555c6b0b | L5-L9 |
| S14 | FILE | EV/amendment-6/REPORT.md | ab7c87073d4a8e4419fa2f65bb2e9b3f224615ee5625aa5cb780cd74b32a87b5 | L5-L13 |
| S15 | FILE | EV/amendment-7/REPORT.md | b31bc80e47e3cae518b5d7e4a102524da74bf8743a6b419e5ed230d0f102f989 | L4 |
| S16 | FILE | EV/amendment-6/incidents/I6-01-native-denial.txt | 3630c26015b2246cd7a8df2dba25db08cab5741f4ba03199961e72aabb4a2f6b | L1-L14 |
| S17 | FILE | EV/amendment-6/permission/P6-01-owner-decision.txt | 3bdb1f6e33124bb16baeec874cb639add11917f589da6f0f25f781a9e6ac04a7 | L1-L18 |
| S18 | FILE | EV/amendment-7/incidents/I7-01-native-denial.txt | a699780549d3a9d3e6c466ec378b2f4940ea2d30f523a7c0613aa0d219c96627 | L1-L23 |
| S22 | FILE | EV/amendment-5/campaign/progress.jsonl | 336cb1de7d37ffeadd724a261fb257a9fe15e581757ac30412558f055daf2073 | L24 |
| S23 | FILE | EV/amendment-4/campaign/progress.jsonl | 5f5115397eb9e6368bceed7df436a65de11dbec165ed24381a371294522a43d3 | L2437 |
| S24 | FILE | EV/amendment-3/campaign/progress.jsonl | def50c5a037749d5851e0e954dabf62705196081a8caee0ff73bf95c9c8b257d | L5 |
| S27 | FILE | AUD/p3-c11-hm-m4-governance-01/REPORT.md | 9f89ee52872de4d9d381924865d2c6771398e89926f50cae20f40dfdae0c5c63 | L3 |
| S28 | FILE | AUD/p3-c11-m6-el1-corrections-01/REPORT.md | 03563669cdc3cc9b70161d43db259f22f8cc9a312099427141a5704f6e9c2690 | L3 |
| S29 | FILE | AUD/p3-c11-m5-evidence-ledger-01/REPORT.md | 0198841f073c659def2f2a49ff62e55072480e31db7ba29321de5b283b6c4517 | L3 |
| S30 | FILE | AUD/p3-c11-hm-m3-integration-01/REPORT.md | 8998381246b4cdfca91bd671904a0b08475b72f1921ec6ee15d0170519444357 | L3 |
| S31 | FILE | AUD/p3-c11-hm-m2-authority-01/REPORT.md | 43f967aca6b59d6ca1f143a84a29bafb59ed14fbd824a96e99519a70a5a7593f | L3 |
| S32 | FILE | AUD/p3-c11-hm-a1-ledger-01/REPORT.md | 559327764ab1acabe2e7e091457a21ed4b4056e0b86a3ad0bd2ae8ad234770c0 | L4 |
| S33 | FILE | AUD/p3-c11-hm-fk01-inmemory-foreign-key-01/P3-C11-HM-FK01-INMEMORY-FOREIGN-KEY-01-REPORT.md | 0139565b4a282cbc31c0db9bda67c93548d231018835acd1cef04efef1dbfeee | L3 |
| S40 | FILE | AUD/p3-c11-hm-fk01-inmemory-foreign-key-01/P3-C11-HM-FK01-INMEMORY-FOREIGN-KEY-01.tar.gz | 63b690505e039e619831b1cef68c6bdc17be687376fbc0715b8a4fbc85ac6e44 | - |
| S41 | FILE | AUD/p3-c11-hm-a1-ledger-01/A1-EVIDENCE.tar.gz | 974f49155d1e65ae83e562b178d65d4c399a763ec6df9cf80e4cc18d132c7209 | - |
| S42 | FILE | AUD/p3-c11-hm-m2-authority-01/M2-EVIDENCE.tar.gz | 8aa75a18be9ccdf88f1a3212258dbd0544a8f5bc5ee8990177989b35cb95a3e5 | - |
| S43 | FILE | AUD/p3-c11-hm-m3-integration-01/M3-EVIDENCE.tar.gz | 19e40839cb40ffef877616afd1283964fc8b058a7a978e915e7d33088d52dfae | - |
| S44 | FILE | AUD/p3-c11-hm-m4-governance-01/M4-EVIDENCE.tar.gz | b8a8000eeafd8de6a38b00e138eb63003c34809a249d36512bfb64c7133a56fd | - |
| S45 | FILE | AUD/p3-c11-m5-evidence-ledger-01/M5-EVIDENCE.tar.gz | 1310e8028505f84af5ff6b1a343d8e2160d30530cf60833f70a3f5e110514565 | - |
| S46 | FILE | AUD/p3-c11-m6-el1-corrections-01/M6-EVIDENCE.tar.gz | c11f8832f707d27e179baea49850e96ea3dad3c592d785e910840f5427442189 | - |
| S50 | FILE | EV/M7-CHECKPOINT.tar.gz | 260dec4127b7bb513f08c779c46cea20a6db927ddcc55464119558bc0e7ff190 | - |
| S51 | FILE | EV/M7-AM1-CHECKPOINT.tar.gz | 8ad029a95fc125a00be9ae3ac472ac6b2670fdb873e3ad0bb520e8f3801d8c4c | - |
| S52 | FILE | EV/M7-AM2-CHECKPOINT.tar.gz | cde01c14924bea8ade4c04a10123fd49c0362ce4beafbc9226ed800c3a738ea7 | - |
| S53 | FILE | EV/M7-AM3-CHECKPOINT.tar.gz | d7ffd373e2c6ada5f4c73f4897f143631d24a419c62df0ac0824e8ea066e2148 | - |
| S54 | FILE | EV/M7-AM4-CHECKPOINT.tar.gz | bd30ee33daaa7a12d9f3b5393fb59e7a794fbc815852a102751207cde4cc50cb | - |
| S55 | FILE | EV/M7-AM5-CHECKPOINT.tar.gz | 66990ff6af371b8485922603d5c7cbce56cc54a9e1cec0b7903c188b4b34e9d6 | - |
| S56 | FILE | EV/M7-AM6-CHECKPOINT.tar.gz | 37aed68cc433ebd6ff4711d1fcd14629d54dc79265f52c42acde8c944e961629 | - |
| S57 | FILE | EV/M7-AM7-CHECKPOINT.tar.gz | f8da7aa5b5afaa9b1d465ba363aa213ae3608d7d0110467a37c5951f87e666cf | - |
| A01 | ATTESTED | P3-C11-MISSION-TEXTS-AND-RULINGS.txt, 129920 bytes (OWNER-SUPPLIED / ARCHITECT-VERIFIED / ORIGINAL NOT LOCALLY MATERIALIZED) | b93135fe75b12b303a25e1d818c301039799261c2120acfe15504423369e23ee | L782-L797 |
| N02 | NOT-FOUND | Acceptance rulings for FK01 and M1-M4: no copy outside the closed session-transcript route was located by the amendment writing this record | - | - |
| G01 | GIT-COMMIT | 862a11cef284c5e3c34234dfd2677a5462c86d5d | tree 83a234b6ea7ff446b739e263b45cd9b69f08d5e7 | - |
| G02 | GIT-COMMIT | e71b982b420a51fd131aeb50367aca7b7477a3e6 | tree 3e023d583914ee37b3bfa0089eb7ddf341e5db3c | - |
| G03 | GIT-COMMIT | 22ebf9a5c5b16bb6f5169daf3731a3417c5289d3 | tree b4a93dc6604a2a4e8cf5ae9fe455d8185b40f01b | - |
| G04 | GIT-COMMIT | d5964ee5326c2733177307db6f507328023d361a | tree 51bac5fd335999cc995c0ab9f582a30244973ef0 | - |
| G05 | GIT-COMMIT | 4dc9a427dd3e0c183e8eec3eeb4b887e422fbd79 | tree 0383aa5c759b07b8c6f3b2a280ec6e2e113b2e61 | - |
| G06 | GIT-COMMIT | 15bc5a6792f97a7f2a4a0d34763dfba86b3744aa | tree 629472852cc2e2fa715ae13753604049196f7892 | - |
| G07 | GIT-COMMIT | 95f55fcf981ef7f1507b516a1dd94a8d65f91347 | tree 7a6fb76f0e783f7a409990c0d37fcb17566c5d9c | - |
| G08 | GIT-COMMIT | 6c63d18ee26804d9bbc1d0900fc40e14e9d1b122 | tree 0375a37a2457e35d65258c271497df9a32417edc | - |
| G09 | GIT-COMMIT | ece497f2beda9de3feeb54a656c2e35335f335b8 | tree 25162e7d48084023713a641694ea15d86ad9a043 | - |
| G10 | GIT-COMMIT | 48a54343b00acec86f88b1c89e954687196f5980 | tree 91ae5a2a93528ec197e0896dce1b7a4d1953bda5 | - |
| G11 | GIT-COMMIT | 808fa920954ce91787918e6469bbb05154a58540 | tree df28b8b97aa96f34b061c8d2a262106072d423eb | - |
| G12 | GIT-COMMIT | 650d110122511ed1d292d40dc359e66d752fc2f3 | tree a3b27c8ced8ee174c0afe7538ea2702ace89ff73 | - |
| G13 | GIT-COMMIT | 567cbc65a3998be6e4c3148520abf51314a6fd22 | tree eb57c41deee9261fd364f2eccac41bf07ac063c7 | - |
| G14 | GIT-COMMIT | b1d535f1bb634f2075f12a36ace89a595cdb13ba | tree 7e889b0c1a477ee08b103cf57631c8f2c4cf9de1 | - |

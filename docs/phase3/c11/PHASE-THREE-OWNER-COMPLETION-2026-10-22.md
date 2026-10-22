# Phase Three COMPLETE — LINUX SUPPORT PROFILE — OWNER-DECLARED 22 OCT 2026

Status of this record: **the permanent record of the Owner's Phase Three completion declaration.** It records the
declaration, the frozen engineering identity it names, the accepted evidence by identifier, and the limitations that
remain. It is not a production-readiness, release, deployment or certification record, and it authorizes no further
work. Bracketed ids refer to the sources in §10.

## 1. Four identities, kept apart

| Identity | Value | What it is |
|---|---|---|
| Frozen Phase Three engineering checkpoint (N2) | commit `51af45efc70dfe2551db29991f074c4a00d836de`, tree `68194bec480da950275f8d8291ceb6b025e236b6` | The validated bytes: the merge of PR #25 into `main`, with parents `76fd216c6a05829127ee3ce308115942da2a01b9` (the PR #24 merge) and `724bdf3d4a36a037da50c76b76b4b9296e7483ea` (the PR #25 head) [G01] [G02] [G03] |
| Documentation commits that add and correct this record | an initial documentation commit whose only parent is N2, then one correction commit whose only parent is that initial commit (a file cannot name the commit that contains it) | Documentation only. The initial commit changes `README.md` and `docs/DEPLOYMENT.md` and adds this file; the correction changes this file and `SECURITY.md`. Neither changes source, test, policy, workflow, lockfile or evidence, and neither redefines N2 |
| Owner declaration | quoted verbatim in §2 [O01] | The Owner's decision that Phase Three is complete for the Linux support profile at N2, with all recorded limitations |
| Release and readiness | none | Nexus OS is **not production-ready**. No release, installer, deployment or unrestricted operation is declared or authorized (§7) |

Later commits on `main`, including the documentation commits that add and correct this record, do not redefine N2.

## 2. The Owner's declaration

Verbatim [O01]:

> "As Owner of Nexus OS, I declare Phase Three complete for the Linux support profile at commit 51af45efc70dfe2551db29991f074c4a00d836de, with all recorded limitations. This does not authorize production release or unrestricted operation. I authorize preparation of the documentation-only closure, with separate approval required for GitHub writes and integration."

Source class: an Owner chat directive transmitted on 2026-10-22 to the project Architect and, with mission
P3-C11-PHASE3-DOCS-CLOSURE-PREP-01, to the implementation executor. It is not a cryptographically signed record, not a
GitHub event and not a command approval, and no source system independently records its time.

## 3. Accepted prerequisite and the six closure gate groups

**Accepted prerequisite: M7 final proof.** M7 was accepted by the Architect at C, commit
`60c943ee01d4f09a32461f747210f9702a67547a` (tree `cd1f57f1b43fbd60b6283050c8680e77019593e5`, parent N1
`567cbc65a3998be6e4c3148520abf51314a6fd22`) [G04]. Its N3 control campaign ran at N1: 445 distinct campaign controls =
423 KILLED + 22 RS-01 AS SPECIFIED, with the 596-entry register in `docs/evidence/p3-c11/final-controls/`. These are
historical accepted evidence at N1, not executions on candidate G or on N2. M7 is the prerequisite of the closure gates
below, not one of them.

The six closure gate groups after M7 follow. Each is accepted for what its evidence actually is, on the commit it
actually tested: results on candidate G are G evidence, results on N2 are N2 evidence, and no hosted or Fast Local test
ran on the Owner's installation.

| # | Closure gate group | Identifiers | Evidence type | Outcome |
|---|---|---|---|---|
| 1 | Exact-ref publication of the final accepted candidate | Candidate G `7d62e4385fa3b5c5f4b9d7861db6a5b9dbf6e296` (tree `d62411171342b0044aaae928756e72c18719a3c1`), published at its exact SHA to `refs/heads/repair/p3-candidate11` (a historical published ref; the branch moved on later) [G05] [E05] | Exact-SHA push to the intended GitHub candidate branch; G was PR #24's head at merge | Published |
| 2 | Fast Local CI on that exact candidate | NEXUS OS Fast Local CI run 39687397431, attempt 1, on G [C03] [E05] [R02] | Push-triggered run on a self-hosted Linux runner (label `nexus-local`) testing a checkout of G | SUCCESS (attempt 1) |
| 3 | Hosted CI and standalone Security Audit | On G: NEXUS OS CI run 39691048375 and NEXUS OS Security Audit run 39694588062, each attempt 1 [C04] [C05] [E05] [R02]. On N2 after integration: NEXUS OS CI run 39712866536, attempt 2 (`test-linux`, `security-audit-linux`, `test-frontend`, `test-python`) [C01], and NEXUS OS Security Audit run 39712866547, attempt 2 (`security-audit`, `npm-advisory-audit`) [C02] | GitHub-hosted runners testing the exact commit named; the Security Audit passes with the reviewed set of accepted advisory exceptions | SUCCESS on G (attempt 1) and on N2 (attempt 2); attempt 1 of each N2 run FAIL, preserved (§7) |
| 4 | Reviewed integration into `main` | PR #24 merged 2026-10-22 as `76fd216c6a05829127ee3ce308115942da2a01b9`, with G as its head (the merge has G's tree) [P24] [G02]; PR #25 merged 2026-10-22 as N2 [P25] [G01] | GitHub pull-request merges; the merge commits are Git objects of this repository | Merged |
| 5 | Checked activation on the Owner's Linux support profile | ACTIVATION-01 report and archive [E01] [E02]; FINAL-UNBLOCK-01 report and archive [E03] [E04]; ruling P3-C11-G5-ARCHITECT-ACCEPTANCE-01 [R01] | Owner-attested installation and two real starts on the Owner's Linux host, executor validation receipts K1–K3 and Architect review (§4) | ACCEPTED WITH LIMITS |
| 6 | Owner's explicit completion declaration | §2 [O01] | Owner chat directive, 2026-10-22 | Fulfilled by the declaration |

## 4. Activation evidence: receipts versus checks made for this record

Receipt evidence, accepted as bounded receipts and not re-run [E01] [E02] [E03] [E04] [R01]:

- ACTIVATION-01 reports a locally built and manually installed Debian package, sha256
  `fc70f6d159398e6c6318da177fb0415345c4c4b8a1e55b69117275e6ed02398f` [D01]. That it was built from N2 rests on a
  receipt chain, not on a reproducible build or a CI attestation.
- The installation and both real starts are Owner attestations.
- The K1, K2 and K3 validation receipts support: HM CONTRACT-02 T₀ Option A with 429 LEGACY_UNKNOWN records and 24
  local-only policies, with the original rows' aggregates unchanged; the second start a T₀ no-op; and EL-1 sessions
  with 2 opens and 2 closes. The pre-migration baseline and these counts are bounded receipt evidence.
- The raw start logs and the database validation copies were deleted after validation. The receipts and their digests
  remain.

Checked live while this record was prepared and corrected (2026-10-22), and nothing more:

- the N2 commit, tree and parents, the PR #24 merge commit, M7 at C and candidate G, read from this repository's Git
  objects;
- GitHub `refs/heads/main` equal to N2, read with `git ls-remote` (no fetch);
- the sha256 digests of the four report and archive files E01–E04 in §10, matched on the development machine (digest
  only; their contents were not re-opened);
- the run table and push record for candidate G in the CLOSEOUT-08 executor report E05 (its digest matched).

Preparing this record read no database, WAL, configuration or credential, ran no installed application, built no
software and ran no tests.

## 5. Unresolved limitations

- **GovernanceOracle.** The shipped binary runs the GovernanceOracle in stub mode: it does not serve governance
  decisions. Oracle-dependent swarm functions are unavailable and fail closed. `oracle_verify_token` checks a token's
  shape only; it does not cryptographically verify a signature. The Oracle is not an in-scope Phase Three authority
  provider, and nothing here claims that it works.
- **No live governed effect.** Checked activation executed no live governed real-world effect. The governed effect
  pipeline is evidenced by tests and controls, not by a live exercise on the Owner's host.
- **Legacy memory and cloud use.** The exclusion of legacy historical memory from cloud use is preserved after
  loosening. This declaration does not relax it.

## 6. SQLite state and backup

- The original pre-T₀ backup (BK) is retained.
- The live T₀ change resides in the live database's SQLite WAL file. It is a committed transaction not yet
  checkpointed into the main database file, not a missing commit.
- No backup has been taken since T₀.
- The rollback plan has **not** been rehearsed.
- Copying the three files (the database, its `-wal` and its `-shm`) is not by itself an atomic backup of a live
  database.
- A future consistent backup needs a separately authorized, controlled closed-database procedure or the SQLite online
  backup API. Nothing here authorizes any change to the live database, WAL or SHM.

## 7. Nonclaims

- Nexus OS is **not production-ready**.
- There is no supported release, installer, general deployment or unrestricted operation. One locally built,
  manually installed and receipted Debian package is not a published or supported distribution.
- Linux support profile only. No Windows or macOS completion is claimed.
- No external audit or certification is claimed.
- The Security Audit passes with accepted advisory exceptions, and they remain. A green run does not mean there is no
  dependency debt.
- Host isolation unknowns remain as recorded, including the historical-memory contract's isolation nonclaims
  (`NONCLAIMS.md` §1).
- Historical failed CI attempts stay recorded: attempt 1 of CI run 39712866536 and attempt 1 of Security Audit run
  39712866547 failed on npm registry `ETIMEDOUT` and remain FAIL. They are not relabelled or removed.
- Not every test ran on the Owner's installation. Hosted CI and the Security Audit ran on GitHub-hosted runners, Fast
  Local CI on a self-hosted runner against a checkout of G, the M7 campaign on the development machine at N1, and
  checked activation covered one installation and two starts. Candidate G's results are not N2 results.
- No cryptographic Owner signature, complete test coverage, production capability or live-host validation beyond the
  retained evidence is claimed.

## 8. Follow-up priorities outside frozen Phase Three

Recorded as priorities only. None is authorized here; each needs its own Architect mission and Owner approval.

1. A consistent post-T₀ backup and a restore rehearsal.
2. Repair of the GovernanceOracle and genuine cryptographic token verification.
3. A controlled live governed-effect exercise before any general release.
4. The advisory, CI toolchain and ruleset debt.

## 9. Historical records

`CLOSURE-RECORD.md`, `DECISIONS.md` and `NONCLAIMS.md` in this directory are dated N4 snapshots, not current status.
Their PENDING rows were accurate when they were written and are intentionally unchanged. The current-status section of
`SECURITY.md` is corrected for this declaration by the documentation correction commit (§1).

## 10. Sources

Path prefix `AUD/` = `/home/nexus/nexus-audits/claude-architect/` (evidence on the development machine; not part of
this repository). Evidence packets are identified by digest only; none of their bytes are copied into this repository.

| ID | Kind | Locator | Digest or identity |
|---|---|---|---|
| G01 | GIT-COMMIT | `51af45efc70dfe2551db29991f074c4a00d836de` (N2, the PR #25 merge) | tree `68194bec480da950275f8d8291ceb6b025e236b6` |
| G02 | GIT-COMMIT | `76fd216c6a05829127ee3ce308115942da2a01b9` (the PR #24 merge) | tree `d62411171342b0044aaae928756e72c18719a3c1` |
| G03 | GIT-COMMIT | `724bdf3d4a36a037da50c76b76b4b9296e7483ea` (the PR #25 head) | tree `68194bec480da950275f8d8291ceb6b025e236b6` |
| G04 | GIT-COMMIT | `60c943ee01d4f09a32461f747210f9702a67547a` (M7 accepted at C; parent N1 `567cbc65a3998be6e4c3148520abf51314a6fd22`) | tree `cd1f57f1b43fbd60b6283050c8680e77019593e5` |
| G05 | GIT-COMMIT | `7d62e4385fa3b5c5f4b9d7861db6a5b9dbf6e296` (candidate G; PR #24's head at merge) | tree `d62411171342b0044aaae928756e72c18719a3c1` |
| P24 | PULL-REQUEST | https://github.com/Nexus-os2026/nexus-os/pull/24 | merged 2026-10-22 as G02 |
| P25 | PULL-REQUEST | https://github.com/Nexus-os2026/nexus-os/pull/25 | merged 2026-10-22 as G01 |
| C01 | CI-RUN | https://github.com/Nexus-os2026/nexus-os/actions/runs/39712866536 (NEXUS OS CI) | attempt 2 SUCCESS; attempt 1 FAIL |
| C02 | CI-RUN | https://github.com/Nexus-os2026/nexus-os/actions/runs/39712866547 (NEXUS OS Security Audit) | attempt 2 SUCCESS; attempt 1 FAIL |
| C03 | CI-RUN | https://github.com/Nexus-os2026/nexus-os/actions/runs/39687397431 (NEXUS OS Fast Local CI, on G) | attempt 1 SUCCESS |
| C04 | CI-RUN | https://github.com/Nexus-os2026/nexus-os/actions/runs/39691048375 (NEXUS OS CI, on G) | attempt 1 SUCCESS |
| C05 | CI-RUN | https://github.com/Nexus-os2026/nexus-os/actions/runs/39694588062 (NEXUS OS Security Audit, on G) | attempt 1 SUCCESS |
| E01 | FILE | AUD/p3-c11-activation-01-delivery/P3-C11-ACTIVATION-01-REPORT.md | 0a9d42e64c1645eb8e3790c24f5a8fb7a51cfb16c7e3154fed3c138f45cc2f3b |
| E02 | FILE | AUD/p3-c11-activation-01-delivery/P3-C11-ACTIVATION-01-EVIDENCE.tar.gz | 9efe15d298aca96e824044c6db15e075f7629b3f625e605a9dda29a06549f6f5 |
| E03 | FILE | AUD/p3-c11-final-unblock-01/P3-C11-FINAL-UNBLOCK-01-REPORT.md | 48b2f6b5a67a09a2676806f7a4980275c0625efbd5bcafbead53237813e81510 |
| E04 | FILE | P3-C11-FINAL-UNBLOCK-01-EVIDENCE.tar.gz (delivered archive; identified by digest only) | 6fc048eb2f3143e788c92dc71c5f141f4e578c2c93f08fbb4b345c4f1f3c45da |
| E05 | FILE | AUD/p3-c11-closeout-08-delivery/P3-C11-CLOSEOUT-08-REPORT.md (executor report: G's exact-SHA push and its runs) | eba77dd00990f6686a980361eab6cf291c93bf5099165578bdfdb3b0bb34b565 |
| D01 | PACKAGE | The Debian package reported by E01 (not in this repository; not a release asset) | fc70f6d159398e6c6318da177fb0415345c4c4b8a1e55b69117275e6ed02398f |
| R01 | RULING | P3-C11-G5-ARCHITECT-ACCEPTANCE-01: checked activation accepted WITH LIMITS (an Architect conversation ruling, not a Git artifact) | - |
| R02 | ARCHITECT-CHECK | The Architect's independent check of candidate G's public run identities and SHA links, transmitted with mission P3-C11-PHASE3-DOCS-CLOSURE-CORRECTION-02 (not a Git artifact) | - |
| O01 | OWNER | Owner chat directive of 2026-10-22, quoted in §2 (not signed; not a GitHub event) | - |

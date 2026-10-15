# AD-C7-W DRAFT-03 — RECONCILIATION

> **REVIEW PACKET — NOTHING IS ADOPTED — NOT EXECUTION AUTHORITY.**
> This file explains how `AD-C7-W-DRAFT-03.md` was consolidated. It adopts, accepts, freezes and authorises nothing. AD-C7-W remains **UNADOPTED**. C7 remains **MUTATION_CONTROL_BLOCKED**, and the mutation campaign remains **34/35 Architect-accepted**.

- **Mission:** P3-C11-DR01-D2A1-C7-W-CONSOLIDATE-DRAFT03-01.
- **Executor:** Claude Code. The mission names model `claude-sonnet-5-5` (effort Medium). This session actually ran on `claude-opus-5-5`. That is recorded here rather than left implicit.
- **Repository:** `Nexus-os2026/nexus-os`. The required single parent of the packet commit is `452b3cb5c3d72fd1c7efaa75ed3838845d06a5cd`.
- **Frozen identities used and not changed:**
  - Candidate C `d5964ee5326c2733177307db6f507328023d361a`, tree `51bac5fd335999cc995c0ab9f582a30244973ef0`;
  - catalog blob `1918374263740d2f98640da00fc466737945975e`.

Prior transcripts and drafts are source material only. They are not contract-adoption authority.

## 1. Input identities

| Input | Where the bytes come from | Bytes | Characters | LF count | sha256 |
|---|---|---|---|---|---|
| **Draft-01** | `~/.claude/projects/-home-nexus-NEXUS-nexus-os/bf1c6683-7eeb-4d8b-acdc-d3a4f60fbaa6.jsonl`, record 568 (assistant message, `2026-10-15T16:12:10.671Z`, uuid `1ae3662f-d8f7-4728-b0fa-94e5f90246f3`): the whole message text | 47,646 | 47,414 | 635 | `e1bd654d3a4df5f97fef861d3c1852abc8e37188cfcd4231e75e770213e70a70` |
| **Draft-02** | `…/812c3a9b-7992-4c9b-932e-5bc508b11c03.jsonl`, record 397 (assistant message, `2026-10-15T16:50:46.361Z`, uuid `c6ddedcc-69e5-4dbb-8e4e-8d092d3d9eb7`): the whole message text | 64,371 | 64,147 | 883 | `69ee393bc3c909d5d3f598bfdb174e0fbcd35a0ad6615b400262070529986a30` |
| **ERRATA-01** | `…/da5d8793-c4bb-489c-9214-6e691fc34d62.jsonl`, record 260 (`2026-10-15T17:12:39.255Z`, uuid `ef5b704b-49ee-4150-92f9-0a44ab5316d0`): the file write that authored `errata01.md` | 36,826 | 36,682 | 561 | `d1abbc330138fed6bcf72438af8e4f0999ed93a5d9b13ba0e6d187624a079d66` |

**How each input was authenticated.**
- Each sha256 was computed from the actual bytes.
- Each input is byte-equal to the text recovered from its original transcript record.
- Draft-01's and Draft-02's hashes equal the hashes the mission reports.
- ERRATA-01's full hash was computed from its bytes. It was never inferred from a partial hash.
- The errata as delivered is the next assistant message in the same transcript: record 275, `2026-10-15T17:14:39.824Z`, uuid `1e971ed5-99ab-4d95-a0a0-061ee3a5cd76`. It contains these exact 36,826 bytes verbatim, after a 536-character cover note that names this file as "a clean copy" with "sha256 `d1abbc33…9d66`". The text after the errata in that message is an executor report.

No text was inferred, reconstructed or filled from memory.

## 2. Method

- **Assembly script.** Draft-03 was assembled by a script that slices line ranges from the three authenticated inputs.
  - The script refuses to run if any input's sha256 differs.
  - It asserts the expected text at every slice boundary.
  - Non-verbatim text is written inline, and every instance is listed in sections 4, 6 and 7.
- **Order.** Draft-03 keeps Draft-02's section order.
  - ERRATA-01's insertions (S-C7-18, P-6, 4.7A) are placed where ERRATA-01 says.
  - Text that is not amendment text is placed in two annexes: Draft-02's Owner-adoption decisions, and ERRATA-01's points needing acceptance.
- **No new architecture.** No requirement was added, removed or weakened, except the Architect's mandatory AU-5 correction and its reconciliation (section 4).
- **Repository operations before publication.** Only read-only git object reads, and `git hash-object` without `-w`.
  - No Cargo build, protocol test, explorer or witness run.
  - No worktree; no source, catalog or contract edit.

## 3. Applied ERRATA-01 replacement map

Each item was applied once. "Verbatim" means that ERRATA-01's text appears exactly once in Draft-03, byte for byte (verification check 1).

| Item | Draft-02 location | Draft-03 location | How applied |
|---|---|---|---|
| E1.1 | §3, G.7(D) | §3, G.7(D) | verbatim |
| E1.2 | §6.3, in full | §6.3 | verbatim |
| E1.3 | §6.4, in full | §6.4 | verbatim |
| E1.4 | §13.1 | §13.1 | verbatim |
| E1.5 | §13.3, in full | §13.3 | verbatim, except step 1's "Meet AU-1 to AU-6 (12.3)." bullet, which is extended (RC-4) |
| E1.6 | §2, S-C7-3 | §2, row S-C7-3, Effect cell | operative text verbatim, merged into Draft-01's S-C7-3 row (section 6) |
| E1.7 | §6.1, I-C7.7 amendment | §6.1, I-C7.7, closing paragraph | verbatim, without its "**I-C7.7:**" label |
| E1.8 | §11, item 8 | §11, item 8 | verbatim |
| E1.9 | Draft-02's response §4, OD-3 and OD-4 (not amendment text) | Annex A | verbatim |
| E2.1 | §12, in full | §12 | verbatim, except AU-3, AU-4 and AU-5 (section 4) and 12.6 (EA-18) |
| E2.2 | §2A.1, K4 and the stop line after K5 | §2A.1 | verbatim; K4 extended (RC-3) |
| E2.3 | §13.2 | §13.2 | verbatim |
| E2.4 | §8.1, row C7-19 | §8.1, row C7-19 | verbatim |
| E3 | §8.2, CL-1 | §8.2, CL-1 | verbatim |
| E4.1 | §3, G.7(B) bullets 4–6 | §3, G.7(B) | verbatim |
| E4.2 | §4.7, from "*VIOLATION line (frozen):*" through the second frozen line | §4.7 | verbatim |
| E4.3 | new 4.7A, after 4.7 | §4.7A | verbatim |
| E4.4 | §4.8, the any-violation bullet | §4.8 | verbatim |
| E4.5 | §6.2, SR-2 | §6.2, SR-2 | verbatim |
| E4.6 | §8.1, rows C7-5, C7-8 and C7-9 | §8.1 | verbatim, each once |
| E4.7 | §8.2, CL-3 to CL-7 | §8.2 | verbatim |
| H.1 | §2, after S-C7-17 | §2, S-C7-18 | verbatim, except that the "(new)" tag is removed (EA-4) |
| H.2 | §2A.3, after P-5 | §2A.3, P-6 | verbatim |
| H.3 | §2A.5, end | §2A.5, end | verbatim |
| §4 points 1–8 (response text) | — | Annex B | verbatim |

**Superseded Draft-02 text.** Verification check 1 confirms that 35 specific superseded Draft-02 strings are absent from Draft-03. Among them:
- the binary "B-W accepted" rules;
- the old K4 definition and stop line;
- the old G.7(B) and G.7(D) text;
- the old 4.7 and 4.8 panic text;
- the old SR-2, 6.3, 6.4 and C7-5/8/9/19 rows;
- the old CL-1 and CL-3 to CL-7;
- the old §11 item 8;
- the old §12, 13.1, 13.2 and 13.3, including its old run order;
- the old OD-3 and OD-4;
- the old AU-3, AU-4 and AU-5 K4 lines.

## 4. Mandatory AU-5 correction

### 4.1 The correction, exactly as applied

In 12.3, AU-5, the bullet `K4 is equal in the execution mission and in the authenticated record.` is replaced by the Architect's text, one sentence per bullet:

- The adoption record contains K1, K2, K3, K5, the conveying mission identity, the explicit Owner-decision details, the C7-only scope and the CR-B adoption status.
- It does not contain or claim its own final commit, blob or SHA256.
- After the adoption record is committed, a subsequent Primary Architect-authorized mission pins K4 and authenticates the record against the actual Git object, its path and the named remote review ref.
- K4's commit, path, blob and SHA256 must agree with that independently retrieved Git object and with the execution mission's pinned identity.
- No self-reference or self-attestation establishes Owner authority.
- Without the Owner's explicit approval, the conveying mission and the authenticated record, execution is refused.

**Notes on the applied text.**
- The Architect's spelling ("authorized", "SHA256", "Git") is kept exactly.
- AU-5's other two bullets are unchanged: the four-place agreement of K1, K2, K3 and K5, and CR-B's status.
- Each sentence appears exactly once in Draft-03, inside AU-5 (verification check 5).

### 4.2 Reconciliation of AU-3, AU-4, K4 and 13.3 (other requirements unchanged)

**RC-1. AU-3 (12.3 item 5).**
- *Before:* `   - names the conveying mission and section; K1, K2, K3 and K5; the scope; and CR-B's status;` … `   Its commit, path, git blob and sha256 are K4.`
- *After:*
  - `   - names the conveying mission and section; K1, K2, K3 and K5; the explicit Owner-decision details of AU-1; the scope, C7 only; and CR-B's status;`
  - a new bullet: `   - does not contain or claim its own final commit, git blob or sha256 (AU-5);`
  - `   Once it is committed, its commit, path, git blob and sha256 are K4. K4 is pinned only after that commit, by a subsequent Primary Architect-authorised mission, and is never taken from the record's own text (AU-4, AU-5).`
- *Unchanged:* "A string is not authority", no signature, the AD-1-R4 form, and commitment on a review ref.

**RC-2. AU-4 (12.3 item 6).**
- *Before:* `The execution mission names K4 and the conveying mission. At 13.3 step 1, the executor checks that:` and `the commit exists, and is reachable from the named review ref, with local and remote equal;`
- *After:*
  - `After the record is committed, a subsequent Primary Architect-authorised mission pins K4: the record's commit, path, git blob and sha256 (AU-5). The execution mission names that pinned K4 and the conveying mission. At 13.3 step 1, the executor authenticates the record against the actual git object, and checks that:`
  - `… reachable from the named remote review ref, …`
- *Unchanged:* the four checks (commit, path, blob and recomputed sha256).

**RC-3. K4 (2A.1).**
- *Before (ERRATA-01 E2.2):* `- **K4 Adoption:** the adoption record of 12.3: its commit, path, git blob and sha256.`
- *After:*
  - the same line, followed by `These are the record's future identities. They exist only once the record is committed, and are pinned afterwards by a subsequent Primary Architect-authorised mission (AU-4, AU-5).`
  - a new sub-bullet: `  - It does not contain or claim its own final commit, git blob or sha256.`
- *Unchanged:* "must name K1, K2, K3 and K5 exactly, and the conveying mission", "counts only when AU-1 to AU-6 hold", and the three STOPPED BEFORE EXECUTION sentences.

**RC-4. 13.3 step 1.**
- *Before:* `   - Meet AU-1 to AU-6 (12.3).`
- *After:* `   - Meet AU-1 to AU-6 (12.3). This includes authenticating the adoption record against the actual git object, its path and the named remote review ref, and checking that K4 agrees with that object and with the K4 this mission names (AU-4, AU-5). The record never supplies its own K4.`
- *Unchanged:* every other step of 13.3, and its order.

**Not changed, and consistent with the correction:**
- 13.2 (E2.3): "K1 to K5, including K4's commit, path, git blob and sha256", which is the execution mission's pinned identity;
- AU-1, AU-2 and AU-6;
- 12.1, 12.4 and 12.5;
- row C7-19.

### 4.3 Left open by this reconciliation

The text does not decide whether "the subsequent Primary Architect-authorised mission" that pins K4 is the execution mission itself or an earlier mission. Both readings satisfy the correction. This is review item R-2 (section 10).

## 5. OVERLAY-C7

- **Byte identity.** Draft-03's overlay block (36 lines between the marker lines) is byte-identical to Draft-02's (verification check 6).
- **Extraction convention used here.** The bytes start after the LF that ends the line `BEGIN OVERLAY-C7`. They end with, and include, the LF that ends the last line before the line `END OVERLAY-C7`. In other words: exactly the 36 lines between the two marker lines, each LF-terminated.
- **OVERLAY-C7 sha256 under that convention:** `c5a9f3d1ade663ca4ef57143c5ae5a1392296d9a05bf226888ba99df2ca44af7` (1,736 bytes; first byte `{`; last bytes `}` and LF). Draft-02's block gives the same value.
- **Other readings.** K3's text, "the bytes between the lines … extracted mechanically", does not state whether the marker lines or the final LF are included. For review only, the other two readings give:
  - with both marker lines, LF-terminated: `4aa354f046530adbccf26199ed0c684d6aad935297f38b79db564209a3ff20de` (1,768 bytes);
  - without the final LF: `bdec037400bd2fecfb1baef767275db0cfb45042f24c175fceaeb34eebbf8b9d` (1,735 bytes).

  This packet does not fix K3's convention (review item R-3).
- **Not K3.** K2 and K3 exist only after the separately authorised freeze of 12.3 step 2. Draft-03's identities and this overlay value are review identities, not K2 or K3.
- **The overlay addresses only C7.**
  - Its top-level keys are `overlay`, `catalog`, `c_base`, `entry` (`"C7"`), `set` (3 keys), `add` (3 keys) and `append` (1 key), as P-2 requires.
  - Its `catalog` object equals K1, and its `c_base` object equals K5.
  - It touches no P-5 field and not `execution_dependent_uncertainty` (P-6).
- **P-1 conditions against the actual catalog** (blob `19183742…`, read from git):
  - every `set` key exists;
  - no `add` key exists;
  - `special_attribution_rules` is an existing list of 2 items.
- **S-C7-18.** Its two quotations equal the catalog's `execution_dependent_uncertainty` items byte for byte.

## 6. Expansion of Draft-02 shorthand

Each expansion uses only authenticated Draft-01 text, or the Draft-02 text named.

| Draft-02 shorthand (line) | Expanded from | Draft-03 location |
|---|---|---|
| §0 row "Draft-01 … (see the note above)" (d2:29) | Draft-02's preface: transcript `bf1c6683…`, record 568 | §0 row Draft-01 |
| §0 row `43-full-F-3` "as Draft-01" (d2:41) | d1:45 | §0 row `43-full-F-3` |
| Provenance labels, inline list (d2:23) | d1:20–25 (the definitions) | header |
| 1.1 "This is unchanged from Draft-01" (d2:68–74) | d1:55–73 | 1.1 |
| 1.2 "SF-1 to SF-10 are unchanged from Draft-01" (d2:76) | d1:75–93; SF-11 to SF-14 from d2:77–84 | 1.2 |
| 1.3 "Unchanged from Draft-01" (d2:86) | d1:95–101, plus Draft-02's sentence (EA-8) | 1.3 |
| §2 "Rows S-C7-1 to S-C7-14 are as in Draft-01" (d2:138) | d1:132–147; S-C7-3 merged with E1.6 | §2 table |
| 4.2 "for the reasons given in Draft-01" (d2:314) | d1:228–230 | 4.2 |
| 4.3 composition, "(unchanged)" (d2:316–321) | d1:234 and d1:236–239 (the same composition, with the mirroring map) | 4.3 |
| 4.4 "Unchanged from Draft-01" (d2:389) | d1:299–303 | 4.4 |
| 4.5 "Unchanged from Draft-01. In brief" (d2:391–395) | d1:305–321 | 4.5 |
| 4.6 "As Draft-01, plus …" (d2:397) | d1:323–334 | 4.6 |
| 4.7 per-step line "(unchanged)" (d2:405) | d1:338 | 4.7 |
| 4.7 "**H1** is unchanged from Draft-01" (d2:427) | d1:349–351 | 4.7 |
| 4.8 "As in Draft-01, with one change" (d2:436) | d1:360–372; then E4.4 | 4.8 |
| 4.9 "Unchanged from Draft-01" (d2:440) | d1:375–388 | 4.9 |
| §5 "As in Draft-01, with these changes" (d2:449–454) | d1:396–411, merged (EA-13) | §5 |
| 6.1 "As in Draft-01, with these amendments" (d2:458–462) | d1:413–453, merged (EA-14) | 6.1 |
| 7.4 "(Draft-01 7.1, unchanged)" (d2:640–644) | d1:457–467 | 7.4 |
| 7.5 "(Draft-01 7.4, unchanged)" (d2:646) | d1:488–500 | 7.5 |
| §9 "Unchanged from Draft-01" (d2:716–721) | d1:524–531 | §9 |

**Section 0.1.** Draft-01's basis table and its rendering note (d1:29–51) are carried verbatim as §0.1. Draft-02 did not restate them in full, and later clauses cite their identities (the patch blob, the raw-output blobs, the base contract).

## 7. Other non-verbatim edits (complete list)

### Editorial edits

- **EA-1.** The title is changed to DRAFT-03.
- **EA-2.** The banner is adapted. It now says that the draft consolidates the three inputs, that publication is not adoption, and the C7 and campaign status.
- **EA-3.** The preparation line is replaced by the consolidation statement and the inputs table.
- **EA-4.** Draft-02's change markers are removed:
  - "*(changed)*" (4.1, 4.7);
  - "*(unchanged)*" (4.3, and 4.7's per-step line);
  - "*(replaces Draft-01 1.4)*";
  - "*(reworded)*" (1.5);
  - "(rewritten)" (S-C7-15);
  - "(new)" (S-C7-16, S-C7-17 and S-C7-18).
- **EA-5.** Draft-02's repair tags are removed from headings: (R1) at 0A and 6; (R2) at 2A; (R3) at 6A; (R4) at 8 and at "Source-review obligations"; (R6) at 10; (R7) at 7. The repairs map as follows:

  | Repair | Draft-03 location |
  |---|---|
  | R1 | 0A, 4.1, 6.1–6.4, G.7(D) |
  | R2 | 2A, S-C7-15 |
  | R3 | 6A, S-C7-16 |
  | R4 | G.7(B), 4.2, 4.6–4.8, 6.2 (SR-1 to SR-6), 8 |
  | R5 | 1.2–1.5 |
  | R6 | 10 |
  | R7 | 7 |
- **EA-6.** §0 has a new heading and lead. §0.2 lists this mission's own read-only re-verification.
- **EA-7.** In 1.2, Draft-02's lead sentence is dropped, because SF-11 to SF-14 now follow SF-1 to SF-10 under the same heading.
- **EA-8.** In 1.3, Draft-02's sentence is attributed: "The detail strings and counts were re-read by the Draft-02 mission for section 0."
- **EA-9.** In 4.4, "The mission's expectations" becomes "The Draft-01 mission's expectations".
- **EA-10.** In 4.5, row 13's "The mutant shows E11." becomes "The mutant shows E11 at the transition." This is Draft-02's own 4.5 wording.
- **EA-11.** 4.6 gains a lead, "Plus these frozen line prefixes, which the classifier reads:" (Draft-02's wording).
- **EA-12.** In 4.7, the per-step lead merges Draft-02 with d1:338. H1's lead is Draft-01's role statement: "the healthy witness (type (e3)) must print".
- **EA-13.** §5 merges in Draft-02's changes:
  - the exit bullet now names the full harness line;
  - "print no `VIOLATION`" becomes "print zero `VIOLATION` lines";
  - "runs **first**" becomes "runs **first** (13.3)".
- **EA-14.** 6.1 merges in Draft-02's amendments:
  - I-C7.2 gains "W-PATCH must satisfy NI-1 to NI-8 (6.2).";
  - I-C7.5 gains Draft-02's sentence;
  - I-C7.7 gains E1.7;
  - I-C7.8 gains "that the C-BASE PASS is a W-SRC PASS."

  Draft-02 called that item "a ninth thing". Draft-01's I-C7.8 list has four items, so it is added as the fifth. The previous item's full stop becomes a semicolon.
- **EA-15.** 7.4 and 7.5 use Draft-01's full bullets. Draft-01's "around every `--list` selection" covers Draft-02's "around each listing". 7.5 keeps Draft-02's "A4 is also untouched."
- **EA-16.** §9 carries Draft-01's full text. "Draft-01 remains unadopted history" becomes "Draft-01, Draft-02 and ERRATA-01 remain unadopted history."
- **EA-17.** Cross-references now point at Annex A: 2A.5 "See decision OD-12" and §13's "(below)" both become "(Annex A)".
- **EA-18.** 12.6 reads: "Neither the drafting missions, the errata mission nor the consolidation mission that produced this Draft-03 request or presume Owner approval."
- **EA-19.** The leads of Annex A and Annex B are new.
- **EA-20.** The closing status line is new.

### Consistency edits

- **RC-1 to RC-4.** AU-3, AU-4, K4 and 13.3 step 1 (section 4.2).
- **RC-5. I-C7.3 ordering.**
  - Draft-01's sentence "It is authored only by a separately authorised source mission, published on a review ref, reviewed by the Architect and frozen." could be read as review before freeze. That reading conflicts with ERRATA-01's 13.1 and B-W/B.
  - Appended sentence: "The order is that of 13.1 and 6.3: W-PATCH is frozen and published first, and only then reviewed by the Architect (B-W/B)." (mission check 4).
- **RC-6. References to the base contract.** Five references point at the base contract's sections but read as Draft-03's own 7.5, 8.2 and 12. The contract's sections were confirmed from contract blob `a5ef321a…` (sha256 `cac65351…`): 7.5 "Controls" (line 691; the stop rule is line 693), 8.2–8.4 (the run sets and receipts), and 12 "Stop conditions" (line 932). Draft-02 introduced the collision when it numbered its own 7.5 and 8.2. Each reference is now qualified with "contract":
  - "every other row of 7.5" becomes "every other row of contract 7.5";
  - "every stop condition of section 12" becomes "every stop condition of contract section 12";
  - in G.7(A), "7.5's stop rule" becomes "contract 7.5's stop rule";
  - in C7-3, "the 7.5 stop" becomes "the contract 7.5 stop";
  - in 6A, "an 8.2, 8.3 or 8.4 run" becomes "a contract 8.2, 8.3 or 8.4 run".
- **RC-7. OD-2 (Annex A).** "P-1 to P-5" becomes "P-1 to P-6", to agree with 12.4 after ERRATA-01 H.2 added P-6.

## 8. Source text not carried

- **Draft-01:**
  - its response preface and "Executor report";
  - its amendment text that Draft-02 replaced: 1.4, §3, 4.1, 4.2, the 4.7 markers, 7.2, 7.3, §8, §10 to §13, the "Not superseded" list, the S-C7-15 row and Table B.
  - Its paragraph after G.7 is also not carried, because Draft-02's §3 replaced §3 in full:
    - its first two sentences are covered by CL-1 (E3) and G.7(A);
    - its third, "The informational F-3 run (section 10) is not a condition of G.7", is contradicted by Draft-02's G.7(E): "The F-3 run has a recorded outcome under section 10".
- **Draft-02:**
  - its response preface;
  - its response sections "2. Exact changes from Draft-01" and "3. Remaining unresolved proof obligations", which ERRATA-01 §4 point 7 marks as superseded history;
  - "5. Confirmation that nothing was executed or written";
  - its closing status.
- **ERRATA-01:**
  - its header, base-text note and new-labels list;
  - its item headings;
  - §3, the section-replacement map (realised by this file's section 3);
  - the opening paragraph of §4;
  - its closing status.

A line-coverage accounting confirms that every non-blank Draft-02 amendment line, ERRATA-01 replacement line and Draft-01 expansion line not present verbatim in Draft-03 is one of the replaced, expanded or edited lines listed in sections 3, 4, 6 and 7 (verification, COVERAGE).

## 9. Verification results (mission section 6)

The local, mechanical checks gave **190 PASS, 0 FAIL**. No build, test, explorer or witness was run.

| # | Check | Result | Evidence |
|---|---|---|---|
| 1 | Every errata replacement applied exactly once | PASS | 31 verbatim errata blocks each found exactly once; 6 edited items' operative text found exactly once; 35 superseded Draft-02 strings absent |
| 2 | No missing referenced clauses | PASS | Every family is defined contiguously, with no undefined reference and no duplicate definition: SF-1–14, L1–L4, AS-1–2, A-1–3, S-C7-1–18, P-1–6, K1–K5, NI-1–8, SR-1–6, O-1–6, AU-1–6, F3-1–4, CL-1–12, C7-1–21, M1–M4, H1, I-C7.1–8 and OD-1–13. B-W/A, B-W/B, B-W/C, CR-B, V-W, G.7, G.8 and G.9 are each defined once. Every internally referenced section exists. The five contract references are qualified (RC-6). ERRATA-01 item labels appear only in the annexes |
| 3 | No unresolved Draft-01/Draft-02 shorthand | PASS | Zero matches for "as Draft-0x", "as in Draft-0x", "unchanged from Draft-0x", "Draft-01 7.x", "given in Draft-01", "(unchanged)", "(changed)", "(reworded)", "(rewritten)", "(new)", "see the note above", "(below)" and "(R1)" to "(R7)". The remaining mentions of Draft-01 and Draft-02 are provenance or history only: the inputs table; §0 and §0.1; 1.3; 1.4's "Withdrawn from Draft-01"; 4.3's "Static cross-check for Draft-02"; 4.4; §9; 13.3's "Order change from Draft-02"; the annexes |
| 4 | No conflicting B-W ordering | PASS | 13.3 runs, in order: gate, W-SRC, E-W build, E-W7 build, NI-7/NI-8, B-W/C, (C), (A), (B), F-3, restore, (D), classify. G.7(D) and 6.3 point at 13.3 steps 5–6. O-3: the builds need only B-W/A and B-W/B. 13.1 and I-C7.3 put the freeze before B-W/B. No binary "B-W accepted" text remains |
| 5 | No adoption-record self-hash requirement | PASS | The six mandated sentences are each present once, inside AU-5. AU-3 and K4 state that the record does not contain its own identity. No clause requires the record to name or contain K4 |
| 6 | Overlay contains only authorised C7 changes | PASS | Byte-identical to Draft-02; C7 entry only; 3 `set`, 3 `add` and 1 `append` key; K1 and K5 equal; P-1 conditions hold against the catalog; no protected field touched |
| 7 | Catalog and Candidate C identities unchanged | PASS | Every abbreviation is a prefix of the frozen identity. Git re-reads: C-BASE tree `51bac5fd…`; catalog blob `19183742…` at `c10681b3` and `452b3cb5`; catalog sha256 `d6c05a7c…`. Every hex identifier in Draft-03 occurs in an authenticated input, except ERRATA-01's own sha256, which is computed from its bytes |
| 8 | No implicit acceptance or execution authority | PASS | Banner and closing status present; "Publishing it for review is not adopting it." Every "adopted" or "accepted" occurrence was reviewed: each is negated, conditional, or names AD-1-R4, AD-1-R5 or the accepted controls |
| 9 | The adoption record is not represented as granting authority | PASS | Present once each: 12.1 ("audit evidence … never authority itself"; "A repository file … never passes 13.3 step 1 on its own"); 2A.1 ("A record on its own is never a match"); AU-5 ("No self-reference or self-attestation establishes Owner authority"); 13.1 ("audit evidence, not authority") |
| 10 | Receipt and status labels agree | PASS | The witness path always carries the `c7_witness` module (5 occurrences; none without it). "348 filtered out" appears in G.7(A)–(C), §5, 6A, CL-1, C7-1 and C7-17; "347" appears only in history. The labels have no variant spellings: `KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING`, `MUTATION_CONTROL_BLOCKED`, STOPPED BEFORE EXECUTION, `EXACTLY_W`, `C7_WITNESS_W_SRC_ONLY`, NOT ADMISSIBLE and DESIGN BLOCKED. "'C7: P6 (first failure E11; witness)'" agrees in G.7(F), S-C7-2 and the overlay. P-1 to P-6 and S-C7-1 to S-C7-18 are used throughout. The F-3 NOT COMPLETED label agrees in §10 and CL-1. M1's n = 2 agrees |

**The frozen schedule.** Table A is byte-identical in Draft-01, Draft-02 and Draft-03. The 24-step schedule is unaltered. It remains **STATIC INFERENCE — NOT OBSERVED**. No P6 witness has been executed.

## 10. Unresolved proof obligations, gates and review items

### 10.1 Proof obligations (Draft-03 §11, unchanged)

1. The 24-step schedule is unexecuted.
2. E2 at every step can be shown only at runtime.
3. That (A) on E-W7 prints E11 is not established.
4. That (C) is violation-free is not established.
5. That (B) shows exactly {E11 at step 13's transition, P6 after step 24} is not established.
6. W-PATCH's NI-1 to NI-6 and SR-1 to SR-6 belong to the source mission and the Architect's review.
7. NI-7 and NI-8 are execution evidence.
8. B-W's three states are all outstanding.
9. Section 1.4 awaits review, and depends on AS-1 and B-W.
10. A-1 to A-3 are assumptions.
11. Whether the C4 append had an N addition review is not established.
12. The F-3 outcome is unknown.
13. Minimality is not claimed.
14. The time-cap race is handled as a stop.

### 10.2 Gates still required, none of which is met

- **Architect review** of Draft-03 (12.3 step 1).
- **A separately authorised freeze** that fixes K2 and K3 (12.3 step 2).
- **The Owner's explicit approval** of K2, K3 and the scope, with CR-B stated separately (AU-1, B-W/A).
- **A separately authorised conveying mission** (AU-2).
- **The record** (AU-3), and the **pinning of K4** by a subsequent Primary Architect-authorised mission (AU-4, AU-5).
- **The witness-source mission** (13.1).
- **The Architect's source approval** (B-W/B).
- **The execution mission** (13.2 and 13.3) and its B-W/C.
- **The Architect's ruling on C7** (13.5).
- **The decisions** OD-1 to OD-13 (Annex A), and the points needing acceptance (Annex B).

### 10.3 Review items raised by this consolidation (none decided here)

- **R-1.** The AU-5 reconciliation edits RC-1 to RC-4 need the Architect's acceptance.
- **R-2.** Which mission pins K4: the execution mission itself, or an earlier mission (4.3).
- **R-3.** K3's byte-extraction convention (section 5).
- **R-4.** The contract-reference qualifiers of RC-6.
- **R-5.** The I-C7.3 ordering sentence of RC-5.
- **R-6.** I-C7.2's "W-PATCH must satisfy NI-1 to NI-8" is carried unchanged from Draft-02. NI-7 and NI-8 are established at B-W/C from builds and listings, not from W-PATCH alone. This does not contradict O-3, but the wording is flagged.
- **R-7.** Draft-01's paragraph after G.7 is not carried (section 8).
- **R-8.** OD-2 is aligned with 12.4 (RC-7).
- **R-9.** The model identity. The mission names `claude-sonnet-5-5`, but this consolidation ran on `claude-opus-5-5`.

## 11. Identities and manifest

| Packet file | Identity |
|---|---|
| `AD-C7-W-DRAFT-03.md` | 107,465 bytes; 1,361 LF-terminated lines; sha256 `e6ede5fdef87215c898846fe36d3c8e1b7422ce47610c64d1b32f1fa19997e4e`; git blob `85581d53bd345c64d5adf48fb0d7c71045d818cb` |
| `RECONCILIATION.md` | this file. Its sha256 is in `SHA256SUMS`, because a file cannot contain its own hash |
| `SHA256SUMS` | sha256 lines for the two files above |

- OVERLAY-C7 sha256 (section 5 convention): `c5a9f3d1ade663ca4ef57143c5ae5a1392296d9a05bf226888ba99df2ca44af7`.
- Packet path: `docs/evidence/p3-candidate11/d2a1-c7-adc7w-consolidated-draft-01/`.
- These are review identities. They are not K2, K3 or K4.

## 12. Statement

**Nothing is adopted.**
- AD-C7-W is **UNADOPTED**, and DRAFT-03 is a review draft.
- No Owner approval is requested or presumed, and none is conveyed by this packet.
- No mutation control is accepted. No C7 execution is authorised. No source, catalog, contract or existing evidence is changed.
- Publishing this packet on a review ref is not adoption, not the freeze of 12.3 step 2, and not execution authority.
- C7 remains **MUTATION_CONTROL_BLOCKED**, and the mutation campaign remains **34/35**.

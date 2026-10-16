# Stage 6 — D2A-1 technical closure criteria, evaluated

Each row is a requirement of the governing texts (contract as amended by AD-1-R4, AD-1-R5 and, for C7, AD-C7-W), with
its present status. "Evidenced" means admissible evidence exists at exact Candidate C; acceptance is the Architect's.

| # | Requirement | Source | Status | Evidence |
|---|---|---|---|---|
| 1 | Exact final source; nothing outside 3.2 changed | 3.2; FINAL-EVIDENCE-STAGE1 freeze | **EVIDENCED** — C byte-exact in every receipt; 24 evidence-only commits after C | `00-*` |
| 2 | Confinement guard (9.1) and file lints (9.2) under clippy with warnings denied | 9.1, 9.2 | **EVIDENCED** — `p3_c11_d2a1_the_ledger_protocol_core_is_confined ... ok` (Stage B); Clippy `-D warnings` PASS on all targets, now including `--all-features` | `04-RUST-VALIDATION.md` |
| 3 | Default suite (8.2): every 7.6 test and X-1..X-5, unoptimised, nothing ignored, under caps | 8.2 | **EVIDENCED** — Stage B PASS (governed-control lib 313/0/35, 161.16 s); optimised e1 X receipts | `02-*`, `04-*` |
| 4 | Full reviewed set (8.3) on the exact final commit | 8.3; AD-1-R4 E; AD-1-R5 | **DISCHARGED UNDER THE ADOPTED CONTRACTS** — 14 PASS (3 composed quotient) + F-R by AD-1-R5's proof discharge (raw INCOMPLETE kept) | `02-SECTION84-CLOSURE.md` |
| 5 | Receipts (8.4), including every `#[ignore]` with its receipt or disposition | 8.4; S-9; S5-3 | **EVIDENCED (consolidated here)**; acceptance of the record pending (A-7) | `02-*` |
| 6 | Every 7.5 control killed on its configuration or by its test, with failing and restored receipts | 7.5, 853; F, G, I; AD-1-R5 F; AD-C7-W | **35/35 ARCHITECT ACCEPTED** (33 by Git record, C4/C7 conversational); A4 SATISFIED | `01-*` |
| 7 | C7 without A4 on F-3 (line 748): run, recorded, not required to kill | 748; S-17 | **EVIDENCED** — informational FAIL E11 at 930,702 states (616,707 expanded), original explorer, A4 off, on the C7 mutant executable E-W7 `156cf0cc…` (exec-02 `43-run-F3-informational`, raw `5a4a041a…1c9a`); recomputed here | `01-MUTATION-EVIDENCE-MATRIX.md` |
| 8 | Stop conditions (section 12) | 12; AD-1-R5 A | none open: F-R's INCOMPLETE stopped STAGE2 and was ruled; no property fails on the unchanged core; every control killed | — |
| 9 | Independent review of the evidence (S-20: the Sole Architect) | 923, 928, 976; R | **PENDING** — this packet is its input | — |
| 10 | Fast Local gate on the exact commit, started by dispatch | **3.1, line 125** | **NOT FULLY DISCHARGED** — 21 of 31 workflow steps not evidenced; dispatch not authorised | `04-FAST-LOCAL-VERDICT.md` |
| 11 | Current-tree manifest aligned (H.5) and detached attestation (H.6) | H; line 915 | ct-1 **ALIGNED**; H.6 **CANDIDATE — PENDING DECISIONS** | `03-*`, `05-*` |
| 12 | Section 11 label | 11 | **WITHHELD** (line 926; current ruling) | `06-SECTION11-DECISION-DOCKET.md` |
| 13 | A-1, A-2 | 4.1; J; 13 | **UNDISCHARGED BY DESIGN** — obligations of the full D2a contract, not of D2A-1 | — |

## Does the contract permit technical closure without the interim label?

**Yes.** Section 11 governs only an optional interim label; line 926 withholds that label and routes the design back to
the Architect, which has happened. No clause conditions the Architect's review or acceptance on the label (docket §5).
The label stays withheld; no substitute label is invented.

## What blocks a complete technical-closure candidate

**Row 10 (Fast Local, contract 3.1)** is the one technical gate that cannot be discharged within this mission's
authority (no dispatch, no network, insufficient disk above the 16 GiB floor). Rows 9 and 11 await Architect
decisions; row 12 needs no decision unless a label is wanted.

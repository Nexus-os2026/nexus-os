# Consolidation matrix — how to read it

`CONSOLIDATION-MATRIX.tsv` is the checked source. `CONSOLIDATION-MATRIX.md` is a rendering of it for reading, joined
with the source sentences of the two obligation scans; it adds nothing.

Each TSV row is `ID ⇥ KIND ⇥ SECTIONS ⇥ ANCHORS ⇥ NOTE`. SECTIONS are AD-1-R4 section labels (header, 0, 0A.1 to 0A.5,
1 for the supersession table and its quotations, A to R, E.1 to E.4, F.1, F.2, G.1 to G.6, H.1 to H.6, K.1, K.2,
open). ANCHORS are phrases separated by ` ‖ `; each must stand, whitespace-normalised, in at least one of the named
sections (`tools/check-matrix.py`, check M4).

| Part | Ids | Covers | Kinds |
|---|---|---|---|
| A | A-01 to A-60 | the Architect's rulings of C4-WITNESS-01 (RULING) and the mission's directives, sections 1 and 3 to 10 (DIRECTIVE) | RULING, DIRECTIVE |
| B | DISP-<10 hex> | every sentence of AD1R3-DISPOSITION-01 (sha256 `a8a6d293…cf11c`), not only obligation-bearing ones, since the sheet is consolidated rather than copied | OPERATIVE (landed), VERBATIM (carried word for word), DEFECT (a defect statement; landed as its repair), POINTER (an "R3 n" pointer; resolved), PROVENANCE (the sheet's own header or status), QUESTION (open for the Architect; answered by the ruling or still open), DESCRIPTIVE (about the earlier mission's tooling; not operative), MARKER (a list numeral or a fragment cut by the splitter) |
| C | R3-<10 hex> | every obligation-bearing sentence of AD-1-R3 (sha256 `485d9256…286e`) that is not verbatim in AD-1-R4 | AMENDED (carried with the change a ruling or directive requires), RECARRIED (same obligation; only the surrounding text moved), REPLACED-BY-RULING, HISTORY (a statement about AD-1-R3's own mission or packet, replaced by AD-1-R4's) |

An id is the first 10 hex digits of the sha256 of the whitespace-normalised sentence, as `tools/obligation-scan.py`
prints it; the sentence splitter and obligation keywords are those of the AD-1-R2 and AD-1-R3 scans. The
obligation-bearing AD-1-R3 sentences that stand verbatim in AD-1-R4 (331 at the final gate) are not rows: check M2b
confirms that each stands in AD-1-R4's operative text.

Checks (`checks/03-check-matrix.txt`): M1 part B is exactly every sentence of the sheet; M2 part C is exactly the
non-verbatim AD-1-R3 obligations, and M2b the verbatim ones are present; M3 part A ids; M4 every anchor in a named
section; M5 VERBATIM marks agree with the scan; M6 kinds allowed for their part. The negative controls
(`checks/05-negative-controls.txt`) show that a dropped row, a wrong mark, an unknown section and an anchor missing from
the draft are each caught.

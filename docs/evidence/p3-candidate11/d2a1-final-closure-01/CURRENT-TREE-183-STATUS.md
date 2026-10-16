# The current-tree 183-control manifest

**Is the current-tree 183 manifest aligned? Yes, mechanically (AD-1-R4 H.1–H.5).** Independent checker
`tools/s3_h5.py`: 1,601 of 1,601 checks. 183 controls; 184 control-file pairs; 39 source files; 191 anchors (each at
its count at application); 162 pairs equal their historical hash, 22 differ — exactly H.4's 15 D2A-1 and 7 earlier pairs.

**Which current review decisions remain pending? All 183.** ct-1 records `pending` for every control and no string in
it confers acceptance. The decisions that exist:

| Decision | Scope | Source |
|---|---|---|
| "All 22 stale M-control mappings: SAME PROPERTY." | 22 pairs | relayed FINAL-EVIDENCE-STAGE1 ruling (`FREEZE-RECORD.txt`, blob `4321cb34`) |
| M18: "SAME INTENDED AUTHORITY PROPERTY. CHANGED OBSERVABLE FAILURE MECHANISM." | M18 | conveyed source-level disposition; not an acceptance |

Reconciliation classes: 21 pairs ESTABLISHED UNDER THE EXISTING (relayed) ARCHITECT DECISION + MECHANICALLY ALIGNED;
M18 MATERIALLY CHANGED OBSERVABLE MECHANISM — REQUIRES EXPLICIT ARCHITECT REVIEW; none blocked by inconsistent source
evidence. No control of the 183 was executed at C; no historical kill is transferred to C.

Remaining: ct-1 review; M18's mapping; confirmation for the 21; each control's current status; the campaign-scope
question of line 915 (`03-REMAINING-183-DECISIONS.md`). Details: `03-*`.

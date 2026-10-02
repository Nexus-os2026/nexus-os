# Errata: P2-V1-R3B-I3-I1-R2 evidence

This file corrects one statement of the R2 evidence. The R2 evidence
directory, `docs/evidence/p2-v1-r3b-i3-i1-r2/`, is not modified: its files
stay as committed in `66a245c183b04ff1e2ae30fbbd065a57381df09c`, where
`REPORT.md` is blob `e783fc488d980225a488d46b13d59552d48ced79` (SHA-256
`b833d5d3d4b66ee775e28cc909b7d00f8ec9125cfaee143fabdd91fc019c354e`).

## R2 `REPORT.md`, section 7, "Scope" (lines 302 to 304)

**As written:** "There is no `.gitattributes` anywhere (`checks/scope.txt`,
taken before the commit)."

**Correct statement:** R2 added, changed and removed no `.gitattributes`
file. R2's scope check (`checks/scope.txt`) refuses a changed path whose
name is `.gitattributes`; it does not establish that none exists. Two
`.gitattributes` files exist in the tree, both older than R2 and unchanged
by it. Each has the same blob at R2's repair base
`ed7c7088247badf87b3b1d483ee57360f867e2f1`, at R2's commit
`66a245c183b04ff1e2ae30fbbd065a57381df09c` (R3's base), and in R3's
candidate:

| Path | Blob | Added by |
|---|---|---|
| `docs/evidence/p2-v1-r3b-i3-i1/.gitattributes` | `0e9a3a4822625a7dc389842c7f093ffd5ed9c9d2` | `72ffc4fcc0141e2e5ae77a927481017ca711ab7c` (I3-I1) |
| `packaging/builder-toolchain/.gitattributes` | `5e3b70144df19c7dfd027a40571173066ac0958a` | `fde494177aba285ac138337e400ff3dfc2031e68` |

The R2 sentence is therefore precise only as "no `.gitattributes` file was
added or changed". No R2 conclusion depended on the stronger reading: no
R2 evidence file relies on a `.gitattributes` exemption, and R3 adds none
(`scope/`).

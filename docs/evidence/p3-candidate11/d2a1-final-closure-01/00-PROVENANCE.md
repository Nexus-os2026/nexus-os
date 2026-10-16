# Stage 0 — provenance and authentication

Mission **P3-C11-DR01-D2A1-FINAL-CLOSURE-01**, from ChatGPT GPT-6 (Sole Decision Architect) via the Nexus OS Owner.
Executor: Claude Code, `claude-opus-5-5`. No subagent was used. The effort setting cannot be read back. The mission text
is the executor's input; it is not authority by itself (a string is never authority). Evidence root created
2026-10-16T14:45:45Z with `mkdir` (no `-p`); both mission paths were absent beforehand.

## Result: every material identity authenticates. No hard stop.

| # | Item | Authenticated identity | How | Raw receipt |
|---|---|---|---|---|
| 1 | Primary repository | `/home/nexus/NEXUS/nexus-os`, HEAD `f727f5c39fab8d5c729a55eb28ad576d3d56ce47` (tree `8106ec23…`), branch `rebuild/phase0-trust-boundary`, clean, +0/−0 against its upstream; local `main` `4b36f606`; remote-tracking `github/main` `6e15dee6` | `git status --porcelain=v2 --branch`, `rev-parse` | `raw/00-git-auth-1.txt`, `raw/00-git-auth-3.txt` |
| 2 | Candidate C | commit `d5964ee5326c2733177307db6f507328023d361a`, tree `51bac5fd335999cc995c0ab9f582a30244973ef0`, single parent `71264aa81a40b86189783facc8437e0427887c16` | `cat-file`, `rev-parse` | `raw/00-git-auth-1.txt` |
| 3 | Rust sources at C | `protocol.rs` `a2df2841b99cad5d452008a6d4150e9369d07e66`; `model_tests.rs` `28085ce232827301104b5d757af042ec933976f8`; `unit_tests.rs` `55dc66409eaccdaa93e8f0a56f2ccb14be81e114` | `rev-parse C:<path>` | `00-SOURCE-IDENTITIES.json` |
| 4 | Contract and amendments | see `00-CONTRACT-CHAIN.md`: contract `cac65351…3b3c`; AD-1-R4 `35d5074c` / `00ced6d5…b693`; AD-1-R5 `ae3cabdb` / `4e1ed8c9…1473`; AD-C7-W `cb3a208e` / `989490ec…3f3b` | bytes re-hashed from Git objects | `raw/00-contract-auth.txt`, `raw/00-ad-auth.txt` |
| 5 | Pinned executable E-C | sha256 `cde9f9d20f32f2b8a17b7dcc2f521462ab6c7a6292d5e2cbd1dd2ba2c3d0d30c`, 8,984,464 bytes, inode 62417761, unchanged; built by final-baseline-known-runs-01 `receipts/10-build` (`cargo test -p nexus-governed-control --locked --release --lib --no-run`, rustc 1.94.0) | hashed on disk, **not executed** | `raw/00-git-auth-3.txt` |
| 6 | Accepted F-PT / F-3 baseline evidence | F-PT `gc_symmetry_fpt` raw `0863794e…19a8`, F-3 `gc_symmetry_f3` raw `6dfc796a…5562`, both at `f023cc3d`, byte-equal to their local originals | Stage 2 recomputation | `receipts/23-S2-CHECKS.txt` |
| 7 | C7 final evidence | commit `cca0d773d9a405be566e576886321acf6a28f29b`, tree `68256471d53479a99538462cbd4cb1f53f4c2003`, parent `2148c5a524b6979dde2eb63b6d1442e39dcfb960`; MANIFEST.sha256 357 entries, `9181dbab…6622`, verified against Git | `cat-file`; `tools/packet_verify.py` | `raw/00-git-auth-1.txt`, `receipts/25-PACKET-CUSTODY.txt` |
| 7a | C7 ref | locally `refs/remotes/github/review/p3-c11-d2a1-c7-compliant-exec-02 → cca0d773`. **GitHub was not contacted** (no network is authorised). The C7 mission's own read-back (`…c7-compliant-exec-02/publication/REMOTE-VERIFY.txt`, sha256 `3de04dd7…fb27`) recorded `refs/heads/review/p3-c11-d2a1-c7-compliant-exec-02 = cca0d773`, 358 identical blobs, 0 Actions runs, 0 PRs. | local ref; earlier receipt | `raw/00-git-auth-3.txt` |
| 8 | Current-tree manifest ct-1 | `docs/evidence/p3-candidate11/current-tree-183/manifest.json`, blob `5d31cb45123403626f06c521dd2d53103676250e`, sha256 `20bcc6fe221238f81246348cbbe94636a3c9b6d8acf89901d230e091f1a44279`, version `ct-1`; unchanged at the evidence head | Git objects | `raw/00-git-auth-2.txt`, `receipts/32-S3-CHECKS.txt` |
| 9 | Historical 183 manifest | `docs/evidence/p3-candidate10/original-183/manifest.json`, blob `65fa69dc`, sha256 `2c4adc84121c1efbbfe84aa4422444407c4d1eb4b556fbf3a0ab8e22d4782e14`, version 1-r1 | Git objects | same |
| 10 | Accepted mutation evidence refs | 24 evidence commits from C to `cca0d773`, each single-parent, none a merge, none changing a path outside `docs/evidence/`; each with its local branch and/or remote-tracking review ref listed | `for-each-ref --points-at` | `raw/00-git-auth-3.txt` |
| 10a | Their packet custody | 19 relied-on packets verified file by file against their own SHA256SUMS / MANIFEST: **19 of 19**, no mismatch, no missing file, no unlisted file | `tools/packet_verify.py` (frozen) | `receipts/25-PACKET-CUSTODY.txt` |
| 11 | Prior local closure records | VALIDATION-02 177/177 (`36f10bf9…b822`); CLOSURE-EVIDENCE-AUDIT-01 13/13 (`aa38221f…1732`); CLOSURE-PACKET-01 48/48 (`372d1bc1…7f3f`); each self-excluding with no unlisted file; each sha256 equal to the mission's stated value | `sha256sum -c` | `raw/00-prior-manifest-verify.txt` |
| 12 | Host, processes, resources | `00-RESOURCE-PREFLIGHT.txt` | read-only | `raw/00-resource-preflight-raw.txt` |

## Notes and limitations

- **The contract file is not in Candidate C's tree.** The authenticated copy at the mission's stated path exists at
  evidence commit `8693e3a7` (AD-1-R2 audit packet, not an ancestor of C) and identically at `c2c69e3a`; its bytes
  equal the stated sha256, as does the local copy in `p3-c11-dr01-d2a1-ad1r1-mut-readiness-01/contract/`.
- **AD-1-R5 and AD-C7-W are not in Candidate C's tree**; they are in the evidence-only descendants (`c1e46fc4`,
  `16e74908`). AD-1-R4 and its adoption record are inside C. None of this changes C's source: C's tree is fixed by its
  hash.
- Prior executor reports (VALIDATION-02, CLOSURE-EVIDENCE-AUDIT-01, CLOSURE-PACKET-01) were used to locate receipts;
  every value relied on below was recomputed from receipt bytes by this mission's own tools.
- Worktrees: 215 were registered at the start (including the primary checkout; 40 locked); none was recreated, changed, unlocked or removed. This mission created
  exactly one, `/home/nexus/NEXUS/nexus-os-p3-c11-d2a1-final-closure-01` (detached at C).
- The primary checkout's one existing stash (`p0-ci-r5-before-base-correction`) is untouched.

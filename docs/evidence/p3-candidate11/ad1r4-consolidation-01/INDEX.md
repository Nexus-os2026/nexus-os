# AD1R4 consolidation packet — P3-C11-DR01-D2A1-AD1R4-CONSOLIDATION-01

**Status: review materials only.** AD-1-R4 is a DRAFT — NOT ACCEPTED — NOT ADOPTED — NOT APPLIED. It is the one
canonical consolidated contract-amendment candidate, for the Sole Architect's independent final contract review.
Nothing here is Owner adoption, a mutation kill, D2A-1 acceptance or Phase Three completion. Nothing was built, tested,
mutated, explored or run against Nexus by this mission.

Prepared by Claude Code (Claude Opus 5.5, `claude-opus-5-5`; mission effort High).

## Identities

| Item | Identity |
|---|---|
| AD-1-R4 (this packet) | `ad1r4/P3-C11-D2A1-CONTRACT-02-R1-ADDENDUM-AD1-R4-DRAFT.md`, sha256 `00ced6d535c78f34f9e92233b742126a7b19192bf3ac8348056be2665192b693`, 143,220 bytes |
| Parent of this packet's commit | C4 evidence commit B `a464a36621523f539d2554a6b176d678c7f885ea` (parent: C4 prototype A `04e42a7ddf0d0a2af17b6506c51997006e7a08dc`); not an engineering baseline |
| Contract | `P3-C11-D2A1-CONTRACT-02-R1.md`, sha256 `cac6535115fb333e68e6c3c045e444342c950ab7a735b7b7deea21cb2cb53b3c` (in the AD1R2 packet, commit 8693e3a7 / c2c69e3a) |
| Binding reviews | `6dfb14560a8b2d22fe4e6d517891491b926423941179d0965eb38bc52aeecdce`, `1f964d7515d94f3d21e55d53f23b67fb99a67cb6307b6aacef2c585dcd39fb35` (same packet) |
| AD-1-R3 (history) | sha256 `485d9256932220644d7368d64b7b9b4d8df8a3a5c65aba4a1c5fcdf390ae286e`, blob `7f2463f5421502cc6ee74c070fc3abc41d467770`, commit c2c69e3a |
| AD1R3-DISPOSITION-01 (history) | sha256 `a8a6d29376cf6f393a7eaa9047923719a339377caa554527a8878760cbdcf11c`, at B |
| Historical source baseline | 42dd1bd1 (`protocol.rs` blob `a2df2841b99cad5d452008a6d4150e9369d07e66`) |
| Sealed C10/C24 definitions | `controls_defs.py` sha256 `ef0420a1a69a756d4e71b3a746e39f7e29933643c121645f44dce32c7e4a108f` (inert copy in the AD1R3 packet) |

## Where to start

1. `ad1r4/P3-C11-D2A1-CONTRACT-02-R1-ADDENDUM-AD1-R4-DRAFT.md`: header, 0A.1 (the governing C4-WITNESS-01 ruling), 0A.5, section 1 (S-22 to S-25), F.1 and F.2, G.2, G.5,
   G.6, K.2, and the open items.
2. `review/R3-TO-R4-SEMANTIC-REVIEW.md`: every touched section, and seven items for the Architect's attention.
3. `matrix/CONSOLIDATION-MATRIX.md`: where every ruling item, every disposition-sheet sentence and every non-verbatim
   AD-1-R3 obligation landed.
4. `receipts/04-final-gate.txt` and `checks/`. Final gate: FINAL GATE: PASS (every step exited as expected)

## Reproduce

The tools read the authenticated inputs from git objects or the sealed local originals named in
`receipts/03-freeze.txt`; none is copied here (the contract, reviews, AD-1-R3, the disposition sheet, `protocol.rs` at
42dd1bd1 and the inert `controls_defs.py` copy are each in an earlier commit). `tools/final-gate.sh` is the exact
sequence; each step's command is printed in `receipts/04-final-gate.txt`.

From this directory: `sha256sum -c PACKET-SHA256SUMS` (covers every file but itself; INDEX.md included).

Excluded by design: every executable, build artefact and build log; mutation source (no patch, no `.rs` file); the
earlier packets; full copies of superseded tool versions (their diffs are under `tools/revisions/`; the copies stay in
the local evidence directory). The post-publication read-back is kept outside this commit.

## Files

| Path | Bytes | sha256 | What |
|---|---|---|---|
| `ad1r4/P3-C11-D2A1-CONTRACT-02-R1-ADDENDUM-AD1-R4-DRAFT.md` | 143,220 | `00ced6d535c78f34f9e92233b742126a7b19192bf3ac8348056be2665192b693` | AD-1-R4 DRAFT — NOT ACCEPTED — NOT ADOPTED — NOT APPLIED |
| `ad1r4/AD1-R3-to-AD1-R4.diff` | 78,740 | `2ec8254e2904572f602d636ac646ba3e36c62258a33adb74b2e99d248d31b69d` | unified diff, AD-1-R3 (485d9256…) to AD-1-R4 |
| `review/R3-TO-R4-SEMANTIC-REVIEW.md` | 20,341 | `11e0db5a15ad70a7fb69bdceb3cefc01534c880a102488b2c3203246b9f7963c` | the executor's semantic review of every touched section; attention items |
| `matrix/CONSOLIDATION-MATRIX.tsv` | 43,870 | `73577677d2dfffe2f22fa4bde52e41dde34219a321cbe9210559b2a190e9bf43` | the consolidation matrix (checked source): 242 rows |
| `matrix/CONSOLIDATION-MATRIX.md` | 52,812 | `eb13d910933ece86e5e8b5428c9a10246a6b3bc68bbf2d53faac8ec587e46060` | the same matrix rendered with each source sentence |
| `matrix/README.md` | 2,646 | `080ae1b91293df14fe0ec2187b53cea1172caf027f015ea788d2716e3e9c088f` | how to read the matrix; kinds; checks |
| `checks/01-check-ad1r4.txt` | 52,714 | `7a925c0dcdcbf978342af1bd6f087e5f785404083a0259b4e3fa45112a133247` | final gate G3: mechanical checks of AD-1-R4 (quotes, supersession, taxonomy, C4, C9-b, C10/C24, ...) |
| `checks/02-contradiction-scan.txt` | 46,371 | `66bc8156373b6dd4d3520cd4133204d4c826998761317eafb60edd1774e64434` | final gate G4: contradiction scan, twelve categories |
| `checks/03-check-matrix.txt` | 16,966 | `cb097a4ce3a23cecb4ec0b1fdcf80e815511091b7bb7bde990efc19c68391552` | final gate G5: consolidation matrix checks |
| `checks/04-classification-cases.txt` | 7,198 | `42470d522268d5bce8290abee6a99e9b664ba7718c0634f08c566da78d18330d` | final gate G6: K.2 cases on synthetic descriptors, matched to the draft's table |
| `checks/05-negative-controls.txt` | 6,584 | `81c775e5dd520ea5c9869695380e829e65b75b1e1a946d7ab8f75b64c4ebbea5` | final gate G7: seeded corruptions each tool must catch |
| `checks/06-obligation-scan-r3.txt` | 61,849 | `190dbb242fd8e5057f85e23bdadb9f04b4409caa835e406b63f5155629a5b6cb` | final gate G8: AD-1-R3 obligation sentences, verbatim or not in AD-1-R4 |
| `checks/07-obligation-scan-disposition.txt` | 15,163 | `9f9d5b28eaf647538c3ceff54d53660b31e7a324985e199918889a9534e78bbc` | final gate G8b: every sentence of the disposition sheet |
| `checks/08-review-coverage.txt` | 375 | `b5b94e643a061b4dded69523a6ca221dfb2fe9b94c0afeb66c6f705afb5b7c1d` | final gate G9: the semantic review covers exactly the touched sections |
| `checks/09-defs-anchor-check.txt` | 1,536 | `6ab82e83574e37417706c070b2439b8b0d37e2dc79c62c6d6e901b14656edd0c` | final gate G10: sealed C10/C24 anchors at 42dd1bd1 (AD-1-R3's tool, unchanged) |
| `receipts/00-STAGE1-AUTHENTICATION.txt` | 6,787 | `a3873e03768670b5bfd74a9807caad0d4fca85a94e88b85d6bf969fd7de2325e` | provenance and input authentication |
| `receipts/02-development-log.txt` | 8,777 | `13a5731b98b9ef9c5592ddfacd27cca2ba1898c97e7e6d86da08a0396ef26ff8` | every development run, failure and revision before the freeze |
| `receipts/03-freeze.txt` | 7,350 | `5ca81c2a2ead68a145296e5879e6417459d68c7d2cccf9e2d95b3f7a7f0f0992` | freeze record: frozen tools, template, matrix, review and inputs |
| `receipts/04-final-gate.txt` | 9,830 | `9e33ff54c07e1eaeef32397171bade8faf9646d74ca2c42dba01d9629efb0b17` | the final gate: commands and exit codes |
| `receipts/github-refs-before.txt` | 13,866 | `42a24211bed6693b69b07af89d7adeb7fc2148a443e8020b7b3e48b539b72d01` | git ls-remote github before any change |
| `receipts/revisions/FAILURE-EXTRACTS.txt` | 5,341 | `b08aeb8c5f54a23b453335b488aa266e3e444025b9cac4e9bfea8f5c54d9f1ad` | the failing lines of every development run that failed |
| `tools/ad1r4-template.md` | 133,275 | `1f11f60796b98153ca2e755354c74eac21ba51c2daa24511af4a34bfcd5f77a4` | the draft's template (placeholders for verbatim lines) |
| `tools/fill-ad1r4.py` | 3,514 | `c60b1fd3102b08fd39de738569729f2b5dd14da01821573b056600c91d4de8d1` | fills verbatim blocks from the authenticated bytes |
| `tools/check-ad1r4.py` | 80,851 | `3d1cc98ae69b03465dd2759ba7931a1d94efd59733c7c338f9f5b9d2ac06e53d` | mechanical checker (final version) |
| `tools/contradiction-scan.py` | 13,358 | `d3d75a43e0c3dfd4c85cc33baca6695fd89649a94c836de13cdc3d14c8dc3016` | contradiction scan (final version) |
| `tools/check-matrix.py` | 6,781 | `0aa271c66bddc1fcc816ad0f2d3d7d9c7ce4788975b895d07d4340a346eb2ccb` | matrix checker (final version) |
| `tools/obligation-scan.py` | 1,987 | `f504c21688c9d6e4f8d4cd9c4fd5bfd2d702acbe40e639ab54358f3ebe0616fa` | obligation-sentence scan (AD-1-R3's splitter and keywords) |
| `tools/render-matrix.py` | 3,082 | `1a0b6dfeb7c14faed7dc45446dc47288863f479cfe626a1b34d7aa7a2025b54e` | renders the matrix for reading |
| `tools/classify-cases-r4.py` | 22,124 | `fbc446341605ebc99dabdcf645c4b14fccd233ba1c8dc0463d179d1847d43013` | K.2 rules on synthetic descriptors |
| `tools/negative-controls-r4.py` | 13,671 | `ae3a3344715e092b2bfb217081e885724cf47cd5afe343d562c80720ef83dfaa` | negative controls (final version) |
| `tools/check-review.py` | 2,289 | `5053aa16e3052b95b0c782f050fecaa1d433648ff29b81ed4c0ffd0d2bbe9277` | review coverage check |
| `tools/defs-anchor-check.py` | 3,323 | `2814e63f9b40054682d385ae77b4b38b859eb4ccc5a7fcafbfe55b39ee2e5f21` | AD-1-R3's sealed-definition anchor check, byte-identical |
| `tools/stage1-auth.sh` | 10,846 | `bf2c4f2cfbfc87d034e7368423fc3260968df1e7e44ee034da1f7b1291cb217b` | stage 1 authentication |
| `tools/final-gate.sh` | 7,074 | `24d2456b7a20a8edfbf34179e5167b2c42851f7f499d43046273b6d7202566a2` | the declared final gate |
| `tools/build-packet.py` | 13,067 | `b4a4e1927dc8798847a34081b1ec290e18e373e8fdced819518a7706c25b357b` | this builder |
| `tools/revisions/check-ad1r4.v1-to-check-ad1r4.v2.diff` | 3,861 | `365fb12432f4d46db3d4c3dc126a6249e8a7201d0083cfcef073abb58fcec5db` | checker revision v1 -> v2 |
| `tools/revisions/check-ad1r4.v2-to-check-ad1r4.v3.diff` | 1,563 | `14b98b2ca84afad86561e199f722b9fa2e654594801a893a6cc27b3789adc00d` | checker revision v2 -> v3 |
| `tools/revisions/check-ad1r4.v3-to-check-ad1r4.v4.diff` | 1,437 | `0705475b594282910f29effac5009a551372a00535f53a80f75eee254933db40` | checker revision v3 -> v4 |
| `tools/revisions/check-ad1r4.v4-to-final.diff` | 859 | `510faa7964932518ec42469c1f7bf817484ea3a488ebc4ab20ef60b3cc7116a8` | checker revision v4 -> final |
| `tools/revisions/contradiction-scan.v1-to-contradiction-scan.v2.diff` | 4,907 | `9643fbd5294f026954cb1e6193b29475b9b87e074a39834487b114f92d0736c7` | scan revision v1 -> v2 |
| `tools/revisions/contradiction-scan.v2-to-final.diff` | 797 | `5c14dace5f4d8187aa94f04c8076e3f2fb0bdef9301021fabffe52567a681dde` | scan revision v2 -> final |
| `tools/revisions/negative-controls-r4.v1-to-final.diff` | 1,213 | `4836717a422e5643ef3e0119b0bb568456179a4cb192feed0354f95dcb5ecf63` | negative-controls revision v1 -> final |
| `tools/revisions/check-matrix.v1-to-final.diff` | 2,593 | `a97b0085125173be0301ae80959d8f17bcf0770c5c4e7d910a0d85bb645d5edd` | matrix-checker revision v1 -> final |

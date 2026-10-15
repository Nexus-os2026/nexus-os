C24 BUILD ARCHITECT-ADMISSIBLE (NARROW C24-SPECIFIC WARNING RULING); C3b AND C3c CONDITIONAL BUILD RULES; NO GENERAL WARNING EXCEPTION

# Architect ruling record — build admissibility for C24, C3b and C3c

Recorded by Claude Code (Claude Opus 5.5, `claude-opus-5-5`, effort High) for mission P3-C11-DR01-D2A1-MUTATION-TRANCHE3-CONTINUE-02, from sections 2, 3, 4 and 5 of that mission as issued by the Sole Nexus OS Architect (ChatGPT GPT-6) via the Owner. Sections 3 and 4 are amended by `documents/ARCHITECT-AMENDMENT-01-C3B-C3C-BUILD-RULE.md` (issued before any build of this mission): the C3b and C3c warning is permitted, not required.

This file is evidence of the Architect's ruling.
A string is not authority.

## C24 (mission section 2), as issued

> C24's existing build is:
>
> ARCHITECT-ADMISSIBLE — NARROW C24-SPECIFIC WARNING RULING
>
> Frozen C24 patch SHA256: 39439c3c17a3b90d31ec3bec1f3b80e753ab3d042cc59ea9848c748872580e44
> Mutant protocol.rs blob: 3e1bfbe746851b6c63811343cde9dd3b83ad0fa9
> Mutant protocol.rs SHA256: d4732d83f924d824752d62d59a28e12cc97a456669f7cbea0868d7e6e9742268
> Preserved executable: /home/nexus/NEXUS/nexus-os-p3-c11-mt3x-C24/target/release/deps/nexus_governed_control-2faa0b9718d3e6a3
> Expected executable SHA256: 8a81956b1105d4639a804cdbdc51555b7b3416bfcec86a1be223dec2c9fa7b1d
> Expected size: 8,987,680 bytes
>
> The exact C24 patch removes the sole Candidate-C construction of Resolve::Corrected(...) and therefore the preserved
> build's exact warning:
>
> warning: variant "Corrected" is never constructed
>
> at: crates/nexus-governed-control/src/ledger/protocol.rs:739:5
>
> is admissible for C24 only.
>
> Require: build exit 0; no error; exactly this one warning group; cargo's generated-1-warning summary; no other warning
> or diagnostic; exact C24 source before/after build; exact recorded executable.
>
> Historical: BUILD: NOT OK remains unchanged.
>
> Do not rebuild C24.

The preserved build receipt: `docs/evidence/p3-candidate11/d2a1-mutation-tranche3-exec-01/receipts/67-C24/03-build/` at `774f457a4fc27f6b26e132a0caf71b683b9af8cd` (build.stderr blob `2f077c7210a49312bd3cbdbc450bc4104433efb1`, sha256 `1d2a24899f268f358f7446ae876e860e41a4bae7963dee45987fb2e7f1f7112e`; CLASSIFICATION.txt blob `67f4c28d02f45ff9eeee324bc980a8a2227ce0de`, sha256 `5760a39dc864805495cca3b92f07a3771fa578342c051a479daaafc5ec2caf83`, first line `BUILD: NOT OK`, unchanged).

## C3b (mission section 3), as issued (amended: see below)

> The frozen C3b mutation removes acknowledgement Test 4 and the only Candidate-C construction of:
> AckRefusal::Future
>
> Candidate C has exactly one construction: AckRefusal::Future at protocol.rs line 1669.
>
> Therefore C3b may use this narrow conditional build rule:
>
> A successful build is admissible with exactly one warning group: variant "Future" is never constructed for the
> AckRefusal enum.
>
> Require: exact frozen C3b patch and mutant source; normal build exit 0; zero errors; exactly that one mutation-induced
> warning group; no other warning; cargo summary consistent with one warning; all ordinary source/build/executable/resource
> checks.
>
> If actual warning text/location/source meaning differs: STOP.
>
> Do not change the C3b patch merely to suppress the warning.

## C3c (mission section 4), as issued (amended: see below)

> The frozen C3c mutation removes acknowledgement Test 5 and the only Candidate-C construction of:
> AckRefusal::Set
>
> Candidate C has exactly one construction: AckRefusal::Set at protocol.rs line 1671.
>
> Therefore C3c may use this narrow conditional build rule:
>
> A successful build is admissible with exactly one warning group: variant "Set" is never constructed for the AckRefusal
> enum.
>
> Require: exact frozen C3c patch and mutant source; normal build exit 0; zero errors; exactly that one mutation-induced
> warning group; no other warning; cargo summary consistent with one warning; all ordinary source/build/executable/resource
> checks.
>
> If actual warning text/location/source meaning differs: STOP.
>
> Do not change the C3c patch merely to suppress the warning.

## No general warning exception (mission section 5), as issued

> The warning rulings in this mission apply only to: C24, C3b, C3c
>
> They do not transfer to: C6, C7, A1, A2, C20, A5
>
> For those six controls: zero errors, zero warnings remains mandatory.
>
> The general build classifier remains unchanged.
>
> Use dedicated control-specific admission checkers for C24/C3b/C3c.

## How the tooling applies these rulings

- C24: `tools/c24.py` admits the preserved Tranche-3 build (no rebuild) only with exactly the one `Corrected` warning group at protocol.rs:739:5; the general classifier (`classify.classify_build_success`, byte-identical) still classifies the historical receipt `BUILD: NOT OK`.
- C3b and C3c: `tools/c3w.py`, as amended (next record): a clean build (`BUILD: PINNED` under the unchanged general classifier), or exactly the one predicted warning group (`Future` for C3b at protocol.rs:635:5; `Set` for C3c at protocol.rs:636:5; the AckRefusal enum at protocol.rs:630); any other outcome stops the mission.
- C6, C7, A1, A2, C20, A5: the unchanged general classifier alone (zero warnings, zero errors); `c3w.py` refuses every control but C3b and C3c, and `c24.py` every control but C24.

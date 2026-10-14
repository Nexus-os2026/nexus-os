C28 BUILD — ARCHITECT-ADMISSIBLE (NARROW C28-SPECIFIC RULING; NO GENERAL WARNING EXCEPTION)

# Architect ruling record — C28 build admissibility

Recorded by Claude Code (Claude Opus 5.5, `claude-opus-5-5`, effort High) for mission
P3-C11-DR01-D2A1-MUTATION-TRANCHE1-CONTINUE-02, from section 2 (and sections 8, 11 and 14) of that mission as issued
by the Sole Nexus OS Architect (ChatGPT GPT-6) via the Owner.

**This file is evidence of the Architect's ruling. A string is not authority.**

## The ruling

The existing C28 mutant build is ARCHITECT-ADMISSIBLE. The frozen C28 patch changes
`Read::Got(durable) if durable >= set.top` to `Read::Got(_)`; the next arm is also `Read::Got(_)`. The exact
warning `warning: unreachable pattern` at `crates/nexus-governed-control/src/ledger/protocol.rs:1834:17` is a direct
and expected consequence of the frozen C28 mutation.

For C28 only, the existing preserved build is admissible when all of the following hold:

- exit 0; no signal; no timeout; no OOM; no error diagnostic;
- exactly one warning; its class is `unreachable_patterns`; its location is `protocol.rs:1834:17`;
- the warning explains that the earlier `Read::Got(_)` matches all relevant values;
- source before and after the build is exactly Candidate C plus the frozen C28 patch;
- the expected executable exists and is byte-identical to the recorded artifact.

No other warning is permitted. This is a narrow C28-specific ruling. **It creates no general warning exception**:
for C12, C9, C11, C17, C18a and C18b the general rule (no error and no warning diagnostic) remains, and a warning
on one of them stops the mission for Architect review.

The historical frozen output `BUILD: NOT OK` remains unchanged as executor history; it is not edited or replaced.

## Identities stated by the mission (section 8)

| Item | Value |
|---|---|
| preserved worktree | `/home/nexus/NEXUS/nexus-os-p3-c11-mt1x-C28` |
| preserved executable | `/home/nexus/NEXUS/nexus-os-p3-c11-mt1x-C28/target/release/deps/nexus_governed_control-2faa0b9718d3e6a3` |
| executable sha256 | `116a6584ebe58c1fea17e2067c6f2af3f3c365cb64f595648040f5c797a5de36` |
| executable size | 8,983,776 bytes |
| mutant source blob used by the build | `446d0365e1472efc40321d259a6f1171d3fe716c` |
| mutant source sha256 | `1b59477e6c974f79c9c698916aa2aad350c4c27f1130c86481a15277572c77d8` |
| C28 patch sha256 | `519378de0e6b6a05019e6a2dc718441b2a03970ae0341d89d858f1168ee23868` |
| build stderr sha256 | `3e8336d9b25baf56610aad813202e9a3bea60b268de8e0a489b69d6b2caf7436` |

## The governing build evidence (resume packet, immutable)

| Receipt (`docs/evidence/p3-candidate11/d2a1-mutation-tranche1-resume-01/receipts/36-C28/`) | Blob | sha256 |
|---|---|---|
| `03-build/CLASSIFICATION.txt` (`BUILD: NOT OK`, executor history) | `42926e84b5eff10cdd5d70814faa3b0573ee83a2` | `a85c4dfa8bec87827ff7d47b47dcea30f2fd88511137e0d7d103c20592979ff4` |
| `03-build/build.stderr` | `e278f4dccfecbc1e0da38b35b6fa13bf48f5bda2` | `3e8336d9b25baf56610aad813202e9a3bea60b268de8e0a489b69d6b2caf7436` |
| `03-build/descriptor.json` | `a01a589b858d319b226feebd538aca80a78d0dee` | `a5b320965354093aafbc0924a9f4af88c939f6430d4c0c4557e1cd548f309433` |
| `02-apply/APPLY.json` | `a28476d4e542edd22e5f4d1d2cf7e7d879e9b5ee` | `1aad733717fae63ff685e0fefd27365d2dce4d9af7177155abea0403f397e3af` |
| `05-restore/RESTORE.json` | `8ad3c436d61117f06ea9c786f4777377947bab88` | `147e7561c7065c2aa6f78bbf57cff8894bc58c2293cf5794eef650f41654a963` |

## How this mission applies the ruling

- The C28 build is not repeated: no Cargo, no rustc, no target modification. A dedicated read-only admission
  checker (`tools/c28_admit.py`) authenticates the previous packet's C28 build evidence against these conditions
  and a read-only pin step authenticates the preserved executable. The general build classifier
  (`tools/classify.py`) is unchanged and still classifies the C28 build `BUILD: NOT OK`; the admission checker
  refuses every control other than C28.
- The C28 named test (T-RES) is run once, directly, on the pinned preserved executable; the classification is
  executor candidate evidence only (`KILL_EVIDENCE_CANDIDATE — TYPE (b) — ARCHITECT REVIEW PENDING`).

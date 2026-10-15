C5 BUILD — ARCHITECT-ADMISSIBLE (NARROW C5-SPECIFIC WARNING RULING; NO GENERAL WARNING EXCEPTION)

# Architect ruling record — C5 build admissibility

Recorded by Claude Code (Claude Opus 5.5, `claude-opus-5-5`, effort High) for mission P3-C11-DR01-D2A1-MUTATION-TRANCHE2-CONTINUE-02, from section 2 (C5 BUILD RULING) of that mission as issued by the Sole Nexus OS Architect (ChatGPT GPT-6) via the Owner.

This file is evidence of the Architect's ruling.
A string is not authority.

Claude Code does not independently accept any control.

## The ruling, as issued (mission section 2)

> C5's existing mutant build is: ARCHITECT-ADMISSIBLE — NARROW C5-SPECIFIC WARNING RULING
>
> The frozen C5 catalog explicitly anticipated warning-producing dead/unused code from this exact mutation.
>
> The build is admissible only if the preserved evidence confirms: normal exit 0; no signal; no timeout; no OOM; no compiler error; exact frozen C5 source before and after; exactly the five mutation-induced warning groups below; no additional warning or diagnostic.
>
> Permitted warnings, C5 only:
>
> 1. unused variable "nonce" at protocol.rs:1818:47;
> 2. unused variable "io" at protocol.rs:1818:65;
> 3. methods "acknowledgement" and "frontier" are never used;
> 4. variant "Resolve::Inconsistent" is never constructed;
> 5. variants "Verb::Acknowledgement" and "Verb::Frontier" are never constructed.
>
> The cargo summary stating that five warnings were generated is the summary of those warnings, not a sixth permitted diagnostic.
>
> The warning exception does NOT transfer to A3, A6, C8, C15 or any other control. Their builds still require zero warnings and zero errors.
>
> The historical frozen result: BUILD: NOT OK remains immutable executor history. Do not edit it.

## The admitted build (immutable; at the Tranche-2 evidence commit; not republished)

| Identity | Value |
|---|---|
| control | C5 |
| build receipt | `docs/evidence/p3-candidate11/d2a1-mutation-tranche2-exec-01/receipts/56-C5/03-build` at `2e06860f4b404d1a79a0d290116239b5545b931f` |
| patch sha256 | `a4776885b669bd912fd8710d5a3283f4c1863ab9213e8474d26707b5a6e5f3d8` |
| mutant protocol.rs blob | `e1a49f5ba952bc81cdac9f8f8ace9eeb2aa93495` |
| mutant protocol.rs sha256 | `31ca02cfaeb6058879af75091e2b1bdb21addf846d59ee3e5b4f5806fe30dedd` |
| executable path | `/home/nexus/NEXUS/nexus-os-p3-c11-mt2x-C5/target/release/deps/nexus_governed_control-2faa0b9718d3e6a3` |
| executable sha256 | `90a608834ad0227632ba26f095de5e16914942da86d2f05fdaab5a0d745c15f9` |
| executable size | 8,986,720 bytes |
| build stderr sha256 | `4b7bdea581849cc8a980ab637228c2e19afd7a8f12704e8a44a2dc922663e8dc` |
| build command | `cargo test -p nexus-governed-control --locked --release --lib --no-run -j 2` |

| Build receipt file | Blob | sha256 |
|---|---|---|
| `CHECKS.txt` | `a0da7e89db7620b208086e976b171a3289d4e51c` | `fcf617bd09528f9c78beed01f897d5ab6f5fa6d1317fd940c91b95eff9a4602a` |
| `CLASSIFICATION.txt` | `86e68a5edc72c381c0f3c57849c123fe36b868af` | `b69d2d68af6e61d710ac8fcfbdf1d4e71b1eb459567bbc712525a5a2e8751685` |
| `PARSED-RECEIPT.json` | `61af0401eb68ee48edc95acfa3c5e7fc7f58c877` | `fc213712a54dc866a9a3861400502e9f364cf8af1582c0109676cd26a3c42b1f` |
| `RECEIPT.txt` | `b2adf0554ed2c8b76b6c37dc85df2723d0295570` | `7b525e4697907f46fe6dc190a2d4d4dda035df872335bfe81a1e76d734c4a07f` |
| `build.stderr` | `4a3810aa7144f1bba024caccd951e821f3a8cfef` | `4b7bdea581849cc8a980ab637228c2e19afd7a8f12704e8a44a2dc922663e8dc` |
| `build.stdout` | `e69de29bb2d1d6434b8b29ae775ad8c2e48c5391` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `cgroup-samples.txt` | `31c1658b7324df27488f071f67d73c5fff2f1e3f` | `607a85b02805b50d8413b025cf4ed8a9aaf5af59910f66242d2ae1d5781b5117` |
| `descriptor.json` | `80d81f8252187c4f43355f9f20c0438c097a2ddb` | `b9028310bb865c84b349bb277dd5a362cbd43a8214ee728901a525a01e250d0a` |
| `scope-record.txt` | `e2a8d31d58babbc0abc864a79c4be8ada0e9d6e0` | `84eb32ab41bbeeaa826c13aff188a3d856effca9dabd271bef70f2940695b051` |
| `time-v.txt` | `09a5fab20b76085317d1b4702a96d83716a7a643` | `039f19e59991da6237127ad5fb539b84f6421cf03a0d433eaa640a8d4f81d24b` |

## How this mission applies it

- A dedicated checker (`tools/c5.py`, `admit`) authenticates the preserved build evidence against every condition above, refuses any control other than C5, and reads every quoted source line and marker of the five warning groups against the source the executable was built from. Its record is `receipts/61-C5/01-admission/`.
- The general build classifier (`classify.classify_build_success`) is byte-identical to Tranche 2's and is not given any warning exception; on a copy of the historical build receipt it still writes `BUILD: NOT OK` (`receipts/03-c2-controls/`).
- C5 is not rebuilt. Its preserved executable is pinned read-only where cargo wrote it (`receipts/61-C5/03-pin/`) and run once on F-5 (`receipts/61-C5/04-run/`).
- A3, A6, C8 and C15 are built fresh under the unchanged zero-warning, zero-error policy.

## What this record does not do

- It does not accept C5's kill: that is pending Architect review of this mission's C5 run.
- It does not edit the historical `BUILD: NOT OK` or C5's historical `CONTROL RESULT: MUTATION_CONTROL_BLOCKED`.
- It introduces no general warning exception.

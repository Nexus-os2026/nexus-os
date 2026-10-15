ARCHITECT AMENDMENT-01 — C3b AND C3c: A CLEAN BUILD OR EXACTLY THE ONE PREDICTED WARNING GROUP; THE WARNING IS PERMITTED, NOT REQUIRED

# Architect amendment record — P3-C11-DR01-D2A1-MUTATION-TRANCHE3-CONTINUE-02-AMENDMENT-01

Recorded by Claude Code (Claude Opus 5.5, `claude-opus-5-5`, effort High) for mission P3-C11-DR01-D2A1-MUTATION-TRANCHE3-CONTINUE-02. A string is not authority.

## How the amendment arose (executor record)

Before any build or run of this mission, and before the freeze, the executor found that the premise of mission sections 3 and 4 ("Candidate C has exactly one construction" of AckRefusal::Future, resp. AckRefusal::Set) does not hold for the library-test build: `crates/nexus-governed-control/src/ledger/protocol.rs` lines 32-33 compile `#[cfg(test)] mod unit_tests;`, and `crates/nexus-governed-control/src/ledger/protocol/unit_tests.rs`'s `#[test] fn t_ack_each_test_refuses_alone` constructs `AckRefusal::Future` at lines 1861, 1867 and 1903 and `AckRefusal::Set` at lines 1873 and 1879. The accepted Tranche-2 control C3a removed the only protocol.rs construction of `AckRefusal::Stale` (also constructed in unit_tests.rs) and built with no diagnostic (`docs/evidence/p3-candidate11/d2a1-mutation-tranche2-exec-01/receipts/55-C3a/03-build/CLASSIFICATION.txt` at `2e06860f4b404d1a79a0d290116239b5545b931f`: `BUILD: PINNED`). A clean C3b/C3c build was therefore the likely outcome, which sections 3 and 4 ("Require: ... exactly that one mutation-induced warning group") did not admit.

The executor put the question to the Owner. The Owner answered: admit a fully clean build (one that passes the unchanged general classifier: zero warnings, zero errors, BUILD: PINNED); exactly the ruled warning still admissible; any other warning stops. The Sole Architect then issued the amendment below, which governs.

## The amendment, as issued

> ARCHITECT AMENDMENT:
> P3-C11-DR01-D2A1-MUTATION-TRANCHE3-CONTINUE-02-AMENDMENT-01
>
> FROM: ChatGPT GPT-6 — Sole Nexus OS Architect
> RE: C3b / C3c pre-execution warning-rule correction
>
> The pre-execution conflict is resolved.
>
> My prior sections 3 and 4 were too strict in requiring the predicted dead_code warning to appear.
>
> The lib-test build also compiles unit_tests.rs, where the affected AckRefusal variants are constructed under
> #[cfg(test)].
>
> Therefore the governing rule is amended as follows.
>
> C3b BUILD ADMISSIBILITY
>
> A C3b build is admissible if EITHER:
>
> A. CLEAN BUILD
> - exit 0;
> - zero compiler errors;
> - zero compiler warnings;
> - all ordinary source/build/executable/resource checks hold.
> In this case: BUILD: PINNED under the unchanged general classifier is fully admissible.
>
> OR:
>
> B. EXACT NARROW WARNING BUILD
> - exit 0;
> - zero errors;
> - exactly one warning group: variant `Future` is never constructed for AckRefusal;
> - no other warning or diagnostic;
> - all ordinary source/build/executable/resource checks hold.
> In this case the narrow C3b Architect warning ruling may admit the build even though the unchanged general classifier
> rejects warnings.
>
> The warning is PERMITTED, not REQUIRED.
>
> C3c BUILD ADMISSIBILITY
>
> A C3c build is admissible if EITHER:
>
> A. CLEAN BUILD
> - exit 0;
> - zero compiler errors;
> - zero compiler warnings;
> - all ordinary source/build/executable/resource checks hold.
> In this case: BUILD: PINNED under the unchanged general classifier is fully admissible.
>
> OR:
>
> B. EXACT NARROW WARNING BUILD
> - exit 0;
> - zero errors;
> - exactly one warning group: variant `Set` is never constructed for AckRefusal;
> - no other warning or diagnostic;
> - all ordinary source/build/executable/resource checks hold.
> In this case the narrow C3c Architect warning ruling may admit the build even though the unchanged general classifier
> rejects warnings.
>
> The warning is PERMITTED, not REQUIRED.
>
> FAIL-CLOSED RULE
>
> For C3b/C3c:
> - zero-warning build -> ACCEPT BUILD;
> - exact predicted single warning only -> ACCEPT BUILD under the narrow Architect ruling;
> - any different warning -> STOP;
> - predicted warning plus any additional warning -> STOP;
> - any compiler error -> STOP;
> - source/build identity mismatch -> STOP.
>
> Do not weaken the general classifier.
>
> Do not alter either frozen patch merely to influence whether rustc emits the warning.
>
> C24's existing ruling is unchanged.
>
> C6, C7, A1, A2, C20 and A5 still require zero warnings and zero errors.
>
> Record this amendment in the evidence packet and bind the pre-execution tooling/negative controls to this corrected
> rule before the freeze.
>
> No build has yet run, so no execution evidence is superseded.
>
> Continue the mission after incorporating this amendment.
>
> END OF ARCHITECT AMENDMENT.

## Binding (executor record)

- `tools/c3w.py` implements exactly this rule; `tools/control.py` applies it to C3b and C3c only; the general classifier `classify.py` is byte-identical to the accepted tooling's.
- The negative and positive controls of the rule are in `receipts/03-t3-controls/` (section H), run before the freeze; this record, the build-ruling record and the tools are frozen before C24's test invocation.
- No frozen patch is changed.

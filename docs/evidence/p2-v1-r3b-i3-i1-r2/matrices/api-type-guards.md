# API/type guards (P2-V1-R3B-I3-I1-R2)

Compile-time probes from outside the store boundary. Each guard must fail to build with exactly its expected error and, once that one access is reopened in a scratch copy, build. They prove the safe API surface only; they are not behavioural detections and are not added to any behavioural figure.

- api-ownership-guard: 1 of 1 as required
- api-type-guard: 18 of 18 as required
- harness-check: 1 of 1 as required
- positive-control: 1 of 1 as required

| probe | category | boundary | base path it restates | attempt | expected error | guarded | self-check | as required |
|---|---|---|---|---|---|---|---|---|
| H-TYPE-ERROR | harness-check | - | - | - | error[E0308]: mismatched types | - | - | True |
| POSITIVE-CONTROL | positive-control | - | - | - | (none) | - | - | True |
| P-B1-CLOSURE-DATA | api-type-guard | closure to seal | B1: Recorder::request_seal(&header, &fabricated_closed, 0) sealed a journal whose custody refused closure | request a seal with fabricated closure data through the owner's recorder | error[E0616]: field `recorder` of struct `StoreOwner` is private | True | True | True |
| P-B2-CUSTODY-SWAP | api-type-guard | closure to seal | B2: another custody's genuine closure sealed this journal | exchange the custodies of two owners, so that one closure seals the other's journal | error[E0616]: field `custody` of struct `StoreOwner` is private | True | True | True |
| P-B2-HEADER-SWAP | api-type-guard | closure to seal | B2: the seal was requested with a caller's header | replace the claimed header an owner seals with | error[E0616]: field `header` of struct `StoreOwner` is private | True | True | True |
| P-SEAL-AGAIN | api-type-guard | one-way seal | (R1) a closed store requesting a second seal | request a seal again through a closed store's recorder | error[E0616]: field `recorder` of struct `ClosedStore` is private | True | True | True |
| P-B3-DECISION-EDIT | api-type-guard | opening immutability | B3: opened.decision.blocking.clear() then opened.claim(...) accepted | clear the opening's blocking incidents in place | error[E0616]: field `decision` of struct `Opened` is private | True | True | True |
| P-B3-SCAN-EDIT | api-type-guard | opening immutability | B3/B5: the opening's scan was a public field | remove the opening's verified incidents in place | error[E0616]: field `scan` of struct `Opened` is private | True | True | True |
| P-B3-CLAIM-CALL | api-type-guard | opening immutability | B3: Opened::claim(io, config, generation, applied) took the caller's applied list | claim through an opening from outside the store | error[E0624]: method `claim` is private | True | True | True |
| P-B4-IDENTITY-EDIT | api-type-guard | admission binding | B4: opened.opening and opened.selection.digest were overwritten with another opening's | overwrite the opening's identity and selection digest | error[E0616]: field `opening` of struct `Opened` is private; error[E0616]: field `selection` of struct `Opened` is private | True | True | True |
| P-B4-OPENING-MINT | api-type-guard | admission binding | B4: OpeningId::fresh() was public | mint a new opening identity outside the store | error[E0624]: associated function `fresh` is private | True | True | True |
| P-B4-ADMISSION-FORGE | api-type-guard | admission binding | (I3-I1 guard, re-verified) an admission built from parts | construct a StorageAdmission for another opening | error[E0451]: fields `opening`, `provision_digest` and `observation` of struct `StorageAdmission` are private | True | True | True |
| P-B5-VALIDATOR-FROM-COPY | api-type-guard | disposition provenance | B5: StoreValidator::new(&root, &edited_scan) validated a fabricated disposition | build a validator over an edited copy of a scan | error[E0624]: associated function `of` is private | True | True | True |
| P-B6-EXCHANGE-TYPE | api-type-guard | exchange containment | B6: Exchange::acquire().state.{fatal, durable_through, claim, seal} were writable | name the exchange to reach its state | error[E0603]: struct `Exchange` is private | True | True | True |
| P-B6-WORKER-TYPE | api-type-guard | exchange containment | B6: Worker::new pointed a worker at any file and exchange | name the worker to construct one | error[E0603]: struct `Worker` is private | True | True | True |
| P-SESSION-VERIFIED | api-type-guard | session verification | (R1) maintenance procedures took a caller's report, name or refusal | replace the session's retained verification | error[E0616]: field `verified` of struct `Session` is private | True | True | True |
| P-PROVISION-REWRITE | api-type-guard | session verification | (R1) a public PROVISION rewrite skipped retirement's and re-qualification's preconditions | rewrite PROVISION's retirement directly in a session | error[E0603]: function `rewrite_provision` is private | True | True | True |
| P-VERIFICATION-TRANSPLANT | api-type-guard | session verification | (R2, NC-SUCC-FOREIGN) a verification of one session or store authorizing another's succession | move one session's retained verification into another session | error[E0616]: field `verified` of struct `Session` is private | True | True | True |
| P-VERIFICATION-FORGE | api-type-guard | session verification | (R2) a caller-built verification standing in for the session's own | build a verification from a caller's assessment and an opening's identity | error[E0603]: struct `Verification` is private | True | True | True |
| P-TAKE-ASSESSMENT | api-type-guard | session verification | (R2) a caller consuming or reading the session's retained verify-before | take the session's retained verification from outside the store | error[E0624]: method `take_assessment` is private | True | True | True |
| P-CLOSE-TWICE | api-ownership-guard | one-way seal | (R1) one closure, one seal request | close one owner twice | error[E0382]: use of moved value: `owner` | True | True | True |

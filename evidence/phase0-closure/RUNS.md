# Closure candidate runs

## Candidate 1

- Branch `implement/p0-final-gate-closure`, commit
  `9ed607f8ab7f54ef0e00b510db5703f5f5d78207` (tree
  `f3778691bda389487640c003e64c9b60bb7a5424`), pushed once to GitHub at
  2026-09-29 (new branch; no force). Composition: six workstream components
  with their internal-review repairs, coordinator composition commits
  (registry, guard rows and completeness check, bound reconciliation, L6-first
  scheduled tick, autonomous-loop interval, messaging no-redirect/no-Referer,
  CI harness steps) and the security documentation.
- Local validation before the push (coordinator, isolated HOME, exclusive
  build slots), at `d1735577` (code identical to the candidate apart from one
  test-pin commit and the documentation): `cargo fmt --check` clean; clippy
  (workspace, all targets, all features, `-D warnings`) clean;
  `cargo test --workspace --locked` 7,674 passed, 1 failed (a guard pinning
  the pre-fix messaging client text; fixed in `fcc60e8a`, desktop suite then
  442 passed, 0 failed), 43 ignored; live webview harness dev and release
  profiles ok under Xvfb; frontend `npm ci`, `tsc`, node 21/21, vitest
  461/461, `vite build` ok.
- Advisories on the candidate's lockfiles: cargo-audit 7 vulnerabilities (13
  unmaintained, 12 unsound warnings); npm (app) 6; cargo-deny 10 advisory
  errors (measured after the openidconnect removal on the same package set).
- Fast-local: run `36557040915` (#7, push event): **success**. fast-linux:
  fmt clean, clippy clean, workspace 7,675 passed / 0 failed / 43 ignored
  (293 test binaries), all 241 Final Gate guard tests passed, live webview
  harness ok in the dev and release profiles; fast-python: voice 27 tests OK;
  fast-frontend: node and vitest 461/461, tsc and vite build ok.
- Hosted run 1 of 3: `gh workflow run ci.yml --ref implement/p0-final-gate-closure
  --raw-field candidate_sha=9ed607f8…`, dispatched once; run `36559976709`
  (#109): **failure** on Windows and macOS only; test-linux, test-frontend
  and test-python **success**. test-linux: fmt clean, clippy clean, workspace
  7,675 passed / 0 failed / 43 ignored (293 test binaries), 241/241 Final
  Gate guard tests, live webview harness ok in the dev and release profiles,
  packaged-toolchain steps ok; test-frontend vitest 461/461; test-python voice
  27 tests. Windows/macOS results: see PLATFORM-SCOPE.md (deferred).
- Owner decision after run #109: Linux-only Phase Zero target; no further
  hosted run. Hosted budget used: 1 of 3 (not continued).
- Linux validation gate: candidate `9ed607f8` -> Linux fast-local #7 success
  (above) -> Linux review.

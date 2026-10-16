# Stage 2 — ignored-test dispositions

## 1. The 34 `#[ignore]` tests of the two D2A-1 test files (AD-1-R4 E.3, row 15 as AD-1-R5 B replaces it)

Recomputed mechanically from Candidate C's `model_tests.rs` (blob `28085ce2`) by `tools/s2_configs.py`: **34** ignored
tests, every name and line equal to AD-1-R4 E.3's table; `unit_tests.rs` (blob `55dc6640`) has **none** (E.4 rule 1;
`receipts/23-S2-CHECKS.txt`). In VALIDATION-02 Stage B each of the 34 printed `ignored` and none was executed there; their
evidence is the exact-final receipt named below.

| # | Test (line at C) | Disposition (AD-1-R4 E.3, Owner adopted; row 15 per AD-1-R5) | Exact-final receipt | Raw outcome | Mutant runs (7.5, F) |
|---|---|---|---|---|---|
| 1 | `full_f1` (4169) | REQUIRED_FINAL_RUN | final-baseline-known-runs-01/receipts/45-full-F-1/raw-output.txt at f023cc3d031a (`c1455a1ba3e55ae1…`) | PASS | C1, C2, C10, C24, C25, A1, A2 |
| 2 | `full_f2` (4178) | REQUIRED_FINAL_RUN | final-baseline-known-runs-01/receipts/41-full-F-2/raw-output.txt at f023cc3d031a (`985a3976713bc77b…`) | PASS | C20, C23, A5 |
| 3 | `full_f3` (4184) | REPLACED_BY gc_symmetry_f3 | replacing run `gc_symmetry_f3`: final-baseline-known-runs-01/receipts/43-full-F-3/raw-output.txt at f023cc3d031a (`6dfc796a0b514418…`) | replacing run PASS | C7 without A4 (not required to kill) |
| 4 | `full_f4` (4190) | REQUIRED_FINAL_RUN | final-baseline-remaining-configs-01/receipts/62-full-F-4/raw-output.txt at 89326b859792 (`44b0023eb9106365…`) | PASS | C26 |
| 5 | `full_f5` (4196) | REQUIRED_FINAL_RUN | final-baseline-remaining-configs-01/receipts/65-full-F-5/raw-output.txt at 89326b859792 (`a6d17e7691c80b07…`) | PASS | C5, A3, A6 |
| 6 | `full_f6` (4202) | REQUIRED_FINAL_RUN | final-baseline-remaining-configs-01/receipts/66-full-F-6/raw-output.txt at 89326b859792 (`3660a0b2b88146f7…`) | PASS | C3b, C3c |
| 7 | `full_f7` (4208) | REQUIRED_FINAL_RUN | final-baseline-remaining-configs-01/receipts/67-full-F-7/raw-output.txt at 89326b859792 (`69e152df74318f49…`) | PASS | C3a |
| 8 | `full_f8` (4214) | REQUIRED_FINAL_RUN | final-baseline-remaining-configs-01/receipts/64-full-F-8/raw-output.txt at 89326b859792 (`2e0f15299b7ed66a…`) | PASS | C4 |
| 9 | `full_fpt` (4220) | REPLACED_BY gc_symmetry_fpt | replacing run `gc_symmetry_fpt`: final-baseline-known-runs-01/receipts/44-full-F-PT/raw-output.txt at f023cc3d031a (`0863794ef7500c25…`) | replacing run PASS | C6, C7 |
| 10 | `full_fb` (4226) | REPLACED_BY gc_symmetry_fb | replacing run `gc_symmetry_fb`: final-baseline-known-runs-01/receipts/42-full-F-B/raw-output.txt at f023cc3d031a (`c8b612e9ee7e2b84…`) | replacing run PASS | C8 |
| 11 | `full_fb2` (4232) | REQUIRED_FINAL_RUN | final-baseline-remaining-configs-01/receipts/61-full-F-B2/raw-output.txt at 89326b859792 (`516adc39418a5cc0…`) | PASS | C9, C11, C17 |
| 12 | `full_fg` (4238) | REQUIRED_FINAL_RUN | final-baseline-remaining-configs-01/receipts/60-full-F-G/raw-output.txt at 89326b859792 (`fabcbf13cfccec27…`) | PASS | C18a, C18b |
| 13 | `full_fw` (4244) | REQUIRED_FINAL_RUN | final-baseline-remaining-configs-01/receipts/63-full-F-W/raw-output.txt at 89326b859792 (`d309de48cde0febf…`) | PASS | none |
| 14 | `full_fd` (4250) | REQUIRED_FINAL_RUN | final-baseline-remaining-configs-01/receipts/68-full-F-D/raw-output.txt at 89326b859792 (`2f80e958de47a72a…`) | PASS | C16 |
| 15 | `full_fr` (4256) | REQUIRED_FINAL_RUN, receipt kept INCOMPLETE; unchanged-core 8.3 obligation DISCHARGED_BY_ACCEPTED_REFINEMENT_PROOF (AD-1-R5 B) | final-baseline-remaining-configs-01/receipts/69-full-F-R/raw-output.txt at 89326b859792 (`78e44c335b6ce65f…`) | INCOMPLETE | C15 |
| 16 | `quotient_full_f1` (5355) | EXPERIMENTAL_NOT_REQUIRED (admission observer quotient alone; E admits no configuration to it) | not run at C (no Architect mission named it) | — | none |
| 17 | `quotient_full_f2` (5361) | EXPERIMENTAL_NOT_REQUIRED (admission observer quotient alone; E admits no configuration to it) | not run at C (no Architect mission named it) | — | none |
| 18 | `quotient_full_f3` (5367) | EXPERIMENTAL_NOT_REQUIRED (admission observer quotient alone; E admits no configuration to it) | not run at C (no Architect mission named it) | — | none |
| 19 | `quotient_full_f4` (5373) | EXPERIMENTAL_NOT_REQUIRED (admission observer quotient alone; E admits no configuration to it) | not run at C (no Architect mission named it) | — | none |
| 20 | `quotient_full_f5` (5379) | EXPERIMENTAL_NOT_REQUIRED (admission observer quotient alone; E admits no configuration to it) | not run at C (no Architect mission named it) | — | none |
| 21 | `quotient_full_f6` (5385) | EXPERIMENTAL_NOT_REQUIRED (admission observer quotient alone; E admits no configuration to it) | not run at C (no Architect mission named it) | — | none |
| 22 | `quotient_full_f7` (5391) | EXPERIMENTAL_NOT_REQUIRED (admission observer quotient alone; E admits no configuration to it) | not run at C (no Architect mission named it) | — | none |
| 23 | `quotient_full_f8` (5397) | EXPERIMENTAL_NOT_REQUIRED (admission observer quotient alone; E admits no configuration to it) | not run at C (no Architect mission named it) | — | none |
| 24 | `quotient_full_fpt` (5403) | EXPERIMENTAL_NOT_REQUIRED (admission observer quotient alone; E admits no configuration to it) | not run at C (no Architect mission named it) | — | none |
| 25 | `quotient_full_fb` (5409) | EXPERIMENTAL_NOT_REQUIRED (admission observer quotient alone; E admits no configuration to it) | not run at C (no Architect mission named it) | — | none |
| 26 | `quotient_full_fb2` (5415) | EXPERIMENTAL_NOT_REQUIRED (admission observer quotient alone; E admits no configuration to it) | not run at C (no Architect mission named it) | — | none |
| 27 | `quotient_full_fg` (5421) | EXPERIMENTAL_NOT_REQUIRED (admission observer quotient alone; E admits no configuration to it) | not run at C (no Architect mission named it) | — | none |
| 28 | `quotient_full_fw` (5427) | EXPERIMENTAL_NOT_REQUIRED (admission observer quotient alone; E admits no configuration to it) | not run at C (no Architect mission named it) | — | none |
| 29 | `quotient_full_fd` (5433) | EXPERIMENTAL_NOT_REQUIRED (admission observer quotient alone; E admits no configuration to it) | not run at C (no Architect mission named it) | — | none |
| 30 | `quotient_full_fr` (5439) | EXPERIMENTAL_NOT_REQUIRED (admission observer quotient alone; E admits no configuration to it) | not run at C (no Architect mission named it) | — | none |
| 31 | `gc_symmetry_f3` (6933) | REQUIRED_FINAL_RUN (E.1 holds) | final-baseline-known-runs-01/receipts/43-full-F-3/raw-output.txt at f023cc3d031a (`6dfc796a0b514418…`) | PASS | none |
| 32 | `gc_symmetry_prefix_f3` (6951) | EXPERIMENTAL_NOT_REQUIRED | not run at C (INCOMPLETE by construction; establishes nothing) | — | none |
| 33 | `gc_symmetry_fpt` (8234) | REQUIRED_FINAL_RUN (E.1 holds) | final-baseline-known-runs-01/receipts/44-full-F-PT/raw-output.txt at f023cc3d031a (`0863794ef7500c25…`) | PASS | none |
| 34 | `gc_symmetry_fb` (8242) | REQUIRED_FINAL_RUN (E.1 holds) | final-baseline-known-runs-01/receipts/42-full-F-B/raw-output.txt at f023cc3d031a (`c8b612e9ee7e2b84…`) | PASS | none |

Totals: 11 REQUIRED_FINAL_RUN with an admissible original-explorer PASS (`full_f1`, `full_f2`, `full_f4`–`full_f8`,
`full_fb2`, `full_fg`, `full_fw`, `full_fd`); 1 REQUIRED_FINAL_RUN kept **INCOMPLETE** whose unchanged-core obligation is
discharged by the accepted refinement proof (`full_fr`); 3 REPLACED_BY (`full_f3`, `full_fpt`, `full_fb`) and their 3
replacing runs (`gc_symmetry_f3`, `gc_symmetry_fpt`, `gc_symmetry_fb`), each an admissible composed-quotient PASS; 16
EXPERIMENTAL_NOT_REQUIRED (15 `quotient_full_*` and `gc_symmetry_prefix_f3`), none run at C. 11 + 1 + 3 + 3 + 16 = 34.

## 2. The witness tests

| Test | Where | Ignored? | Disposition |
|---|---|---|---|
| `c1_witness_p1_at_a_successful_admission` | C `model_tests.rs` line 8512 | no (ordinary test) | runs in every `cargo test`; healthy PASS in Stage B and the Stage-1 suite; the G.3 healthy (e3) receipt is `final-baseline-remaining-configs-01/receipts/50-witness-C1`; the mutant (c) receipt is C1's |
| `c25_witness_p7_on_the_resulting_world` | C line 8519 | no | as above; healthy (e3) `51-witness-C25` |
| `c4_witness_p4_and_p8_on_f8` | C line 9197 | no | as above; healthy (e3) `52-witness-C4` |
| `c7_witness_p6_after_settlement_on_fpt` | **W-TREE only** (W-COMMIT `6c424c3a`, tree `5cc7ae59`, line 9712 of W-BLOB `c18a6235`); **not in Candidate C** (`git grep -w c7_witness` finds nothing at C) | no (the `--exact --ignored --list` listing selects 0 tests) | **witness-only test (AD-C7-W).** It is not part of Candidate C's test inventory and changes nothing in it. It ran exactly twice, in the AD-C7-W 13.3 order: (C) healthy on E-W (`e8b4e986…`), PASS_ADMISSIBLE; (B) mutant on E-W7 (`156cf0cc…`), ADMISSIBLE_TWO_CANONICAL. |

## 3. Ignored tests elsewhere in the workspace (unrelated to D2A-1)

VALIDATION-02 Stage B printed 79 `... ignored` lines: 78 are counted in the libtest result sums, and the custom-harness
`webview_boundary_live` printed one without a result line. 34 are the D2A-1 model tests above. The other 45, by test
binary (`raw/02-stageB-ignored-by-binary.tsv`, recomputed read-only from the raw output):

| Binary | Test | Reason printed |
|---|---|---|
| `ide_features-e2bd9ed56f48f37a` | `test_project_init_rust` | requires Ollama with a deployed model |
| `designer_agent-d80ccefd9f63989c` | `test_design_generation` | requires Ollama with a deployed model |
| `designer_agent-d80ccefd9f63989c` | `test_screenshot_analysis` | requires Ollama with a deployed model |
| `nexus_capability_measurement-1569eff64701d67a` | `evaluation::nim_client::tests::test_real_nim_adapter_single_agent` | — |
| `nexus_capability_measurement-1569eff64701d67a` | `evaluation::openrouter_client::tests::test_real_openrouter_query` | Requires OPENROUTER_API_KEY |
| `nexus_capability_measurement-1569eff64701d67a` | `evaluation::validation_run::tests::execute_run1_pre_bugfix_baseline` | — |
| `nexus_capability_measurement-1569eff64701d67a` | `evaluation::validation_run::tests::execute_run2_and_three_way_comparison` | — |
| `nexus_computer_use-02140f01b6a39f40` | `capture::screenshot::tests::test_backend_detection_real` | — |
| `nexus_computer_use-02140f01b6a39f40` | `capture::screenshot::tests::test_real_capture` | — |
| `nexus_computer_use-02140f01b6a39f40` | `capture::screenshot::tests::test_real_capture_with_region` | — |
| `nexus_computer_use-02140f01b6a39f40` | `governance::app_registry::tests::test_real_focused_app` | — |
| `nexus_computer_use-02140f01b6a39f40` | `governance::app_registry::tests::test_real_list_windows` | — |
| `nexus_computer_use-02140f01b6a39f40` | `governance::app_registry::tests::test_real_wm_class_detection` | — |
| `nexus_computer_use-02140f01b6a39f40` | `input::keyboard::tests::test_real_type_text` | — |
| `nexus_computer_use-02140f01b6a39f40` | `input::mouse::tests::test_real_get_position` | — |
| `nexus_computer_use-02140f01b6a39f40` | `input::mouse::tests::test_real_mouse_move` | — |
| `nexus_desktop_backend-6e974b81909e90e3` | `builder_workspace::process_lifecycle::tests::c4b_fixture_crash` | — |
| `nexus_desktop_backend-6e974b81909e90e3` | `builder_workspace::process_lifecycle::tests::c4b_fixture_delayed_exit` | — |
| `nexus_desktop_backend-6e974b81909e90e3` | `builder_workspace::process_lifecycle::tests::c4b_fixture_descendant` | — |
| `nexus_desktop_backend-6e974b81909e90e3` | `builder_workspace::process_lifecycle::tests::c4b_fixture_hold` | — |
| `nexus_desktop_backend-6e974b81909e90e3` | `builder_workspace::process_lifecycle::tests::c4b_fixture_root_exit` | — |
| `webview_boundary_live-d39bf6e05c108272` | `webview_boundary_live` | (pass `-- --ignored` to run) |
| `nexus_governed_control-a7b441620dedae4f` | `authority::tests::stop_measurement::rs01_t10_the_registry_stop_seen_from_outside_its_lock` | RS-01 T10: a diagnostic measurement, run only by its own command after T10a |
| `nexus_kernel-76743e5c6daefea0` | `secrets::backend_keyring::ak_commit4_tests::live_round_trip` | — |
| `nexus_kernel-76743e5c6daefea0` | `supervisor::tests::test_supervisor_config_from_env_with_invalid_value_uses_default` | — |
| `nexus_kernel-76743e5c6daefea0` | `supervisor::tests::test_supervisor_config_from_env_with_valid_values` | — |
| `resource_limiter_integration_tests-b947c10b3959a579` | `helper_attempt_spawn` | — |
| `resource_limiter_integration_tests-b947c10b3959a579` | `helper_cpu` | — |
| `resource_limiter_integration_tests-b947c10b3959a579` | `helper_direct` | — |
| `resource_limiter_integration_tests-b947c10b3959a579` | `helper_exit` | — |
| `resource_limiter_integration_tests-b947c10b3959a579` | `helper_linux_limits` | — |
| `resource_limiter_integration_tests-b947c10b3959a579` | `helper_root_exit` | — |
| `resource_limiter_integration_tests-b947c10b3959a579` | `helper_tree` | — |
| `sealed_spawn_tests-72dd1e4ccd0643ac` | `sealed_exit` | — |
| `sealed_spawn_tests-72dd1e4ccd0643ac` | `sealed_hold` | — |
| `sealed_spawn_tests-72dd1e4ccd0643ac` | `sealed_middle` | — |
| `sealed_spawn_tests-72dd1e4ccd0643ac` | `sealed_report` | — |
| `sealed_spawn_tests-72dd1e4ccd0643ac` | `sealed_tree` | — |
| `nexus_persistence-64df584b06e4d698` | `tests::perf_swarm_audit_1000_row_insert_then_query` | — |
| `phase1_teams-b72db6a88727a2e8` | `drive_builder_teams_end_to_end` | Phase 1.5 — requires specialists wired in Phase 1.2-1.4 |
| `vision_judge-bedbcec031649ae8` | `integration_real_codex_call` | requires real Codex CLI + ChatGPT Plus credentials; run manually |
| `xvfb_smoke-a717fa6f773d6b59` | `xvfb_session_input_calls_run_without_panic` | structural; requires Xvfb. Run with -- --ignored |
| `xvfb_smoke-a717fa6f773d6b59` | `xvfb_session_spawns_and_capture_runs_without_panic` | structural; requires Xvfb. Run with -- --ignored |
| `doc-tests nexus_integration` | `tests/integration/src/swarm_harness/prelude.rs - swarm_harness::prelude (line 4)` | — |
| `doc-tests nexus_migrate` | `crates/nexus-migrate/src/tauri_commands.rs - tauri_commands::_command_signatures (line 98)` | — |

None of these 45 is a D2A-1 test. `rs01_t10_the_registry_stop_seen_from_outside_its_lock` is in the governed-control
crate (RS-01, outside the ledger) and is the 35th ignored test of that binary. `webview_boundary_live` is the Fast Local
harness that is not discharged (`04-FAST-LOCAL-VERDICT.md`). No disposition of these 45 is proposed here.

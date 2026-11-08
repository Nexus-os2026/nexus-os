//! SB-3C R32 supplement (spec 8.2-8.4; SB3C-R6, R8): the 35 route-level sinks
//! of the session model bridge, verified from the TSV alone. No production
//! classifier is called: each row and cell must equal the literal audited
//! tables below (SB3C-R8 (d)), and each cited name must pass a source-text
//! typo lint (`is_test`), which does not show a test is compiled, enabled or
//! run. `c:` cites a retained test only where it executes that sink's own
//! boundary on that value (the SB3C-R8 audit); `x:` is a C2-new oracle. The
//! tables are the audit's record: equality proves a row was not changed since
//! the audit, not that the audit was right. `session_oracles.rs` requires
//! this and the retained 153,000-key matrix validator.
//!
//! A cell is (sink, dimension, value), the B1e side x value precedent. The
//! full cross product is not authored and not claimed (register 14.5).
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

const LIFECYCLE: &str = "Bootstrap Normal Emergency Failed ClosingNormal ClosingRestricted SealedFinalizing ClosedClean ClosedUnclean";
const INGRESS: &str = "CR LF CRLF original_cell_unscheduled repeated EOF partial_Ctrl_D master_close read_failure mission_stop stop_all door_fence budget grant_revocation grant_expiry quit fatal";
// SB3C-R6 (c): exactly coverage.rs FAILURES, which already contains `none`.
const FAULT: &str = "none decline record_failed record_unknown poison panic";
const SCHEDULE: &str = "before after";
const DIMENSIONS: [(&str, &str); 4] = [
    ("lifecycle", LIFECYCLE),
    ("ingress", INGRESS),
    ("fault", FAULT),
    ("schedule", SCHEDULE),
];
const HEADER: &str =
    "route\tsink\tgate\tboundary\tlifecycle\tingress\tfault\tschedule\tprimitive\toracle\tplant";

/// The frozen 35 identities, each bound whole (SB3C-R8 (d)):
/// route|sink|gate|boundary|primitive|oracle|plant.
const ROWS: &str = "\
R0|registration_reserve|-|model::reserve in HostSession::register_model_private|sb2_one_session_operation_active|R0|MP-SB-2
R0|registration_record|-|Authority::record_session_model_registered in register_model_private|sb2_registration_records_before_install_once|R0|MP-SB-2
R0|registration_install|-|slot installation in register_model_private (model.install.before)|sb2_install_completion_is_atomic_to_close_observers|R0|MP-SB-2
R0|registration_readiness|-|HostSession::finish_bootstrap readiness over the model slot|sb2_bootstrap_cannot_overtake_registration|R0|MP-SB-2
R1|identity_view|-|SessionDoor::model_identity (model_identity_private)|sb3c_identity_is_original_request_read_only|R1|MP-SB-4-Q
R2|inference_binding|-|SessionDoor::admit_inference to bind_inference_private original-door CAS|sb3c_public_binding_is_exact_and_idempotent|R2|MP-SB-3-CAS
R3|call_slot_reserve|-|model::Attempt::acquire slot reservation|sb3_concurrent_operation_refuses_before_ticket|A2|MP-SB-5-slot
R3|attempt_ticket|-|run table admit_session_attempt original-snapshot CAS|sb3_attempt_ceiling_is_eight_on_the_original_run|A1|MP-SB-5-count
R4|charge_append|-|Mother BorrowedMeter::charge ModelCallCharged append (coordinator/model_calls.rs)|sb3_caps_and_budget_keep_existing_goldens|C1|MP-SB-7-charge
R4|meter_settle|-|Mother BorrowedMeter::settle clears in_flight (coordinator/model_calls.rs)|sb3_caps_and_budget_keep_existing_goldens|C3|MP-SB-7-charge
R5|registration_version|reg-1|RegistrationGate::admit_request /api/version|session_override_required_at_each_request|G1|MP-SB-6-default
R5|registration_status|reg-2|RegistrationGate::admit_request /api/status|session_override_required_at_each_request|G1|MP-SB-6-default
R5|registration_tags|reg-3|RegistrationGate::admit_request /api/tags|session_override_required_at_each_request|G1|MP-SB-6-default
R5|registration_show|reg-4|RegistrationGate::admit_request /api/show|session_override_required_at_each_request|G1|MP-SB-6-default
R5|call_version|call-1|CallGate::admit_request /api/version|sb3_call_override_required_at_each_request|G3|MP-SB-6-CAS
R5|call_status|call-2|CallGate::admit_request /api/status|sb3_call_override_required_at_each_request|G3|MP-SB-6-CAS
R5|call_tags|call-3|CallGate::admit_request /api/tags|sb3_call_override_required_at_each_request|G3|MP-SB-6-CAS
R5|call_chat|call-4|CallGate::admit_request /api/chat|sb3_call_override_required_at_each_request|G3|MP-SB-6-CAS
R6|cancel_run|-|SessionDoor::cancel_run|sb3_held_request_cancels_and_joins|G5|MP-SB-6-once
R6|door_halt|-|SessionDoor::halt|sb3_held_request_cancels_and_joins|G5|MP-SB-6-once
R6|emergency_restrict|-|SessionIngress::emergency|sb3_held_request_cancels_and_joins|G5|MP-SB-6-once
R6|close_restrict|-|SessionIngress::accept(Quit)|sb3_held_request_cancels_and_joins|G5|MP-SB-6-once
R7|attempt_record|-|model::Attempt::record_return native model_call record|sb3_attempt_record_and_pin_are_exact|A3|MP-SB-9-record
R7|attempt_pin_release|-|run table settle_session_attempt exact model pin|sb3_attempt_record_and_pin_are_exact|A3|MP-SB-5-pin
R7|operation_complete|-|Attempt::complete to Owner::complete slot release|sb2_owner_completion_is_bounded_and_poison_fails_closed|A2|MP-SB-5-slot
R8|record_first_classify|-|Mother adapter::classify, record before reply (adapter/mod.rs)|sb3_registered_session_executes_one_recorded_call|O1|MP-SB-9-record
R8|post_record_recheck|-|Mother Caller::call C1 recheck after the record (coordinator/model_calls.rs)|sb3_revocation_after_chat_entry_is_entered_only|O2|MP-SB-10
R8|lane_result_admission|-|Mother Coordinator LaneReturned append (coordinator.rs)|sb3_registered_session_executes_one_recorded_call|O2|MP-SB-10
R9|close_obligations|-|HostSession::close_obligations (host_session/close.rs)|sb3_open_model_request_at_deadline_blocks_seal|H2-held|MP-SB-11
R9|close_report|-|Runtime::close_report (mother/host/src/hands.rs)|sb2_close_report_has_one_model_line|H1|MP-SB-11
R9|producer_join|-|Producer::joined for the close operation (settle_close_resources)|sb3_completed_call_allows_existing_clean_close|H1|MP-SB-11
R9|seal|-|HostSession::seal_close (host_session/close.rs)|sb3_unjoined_or_owed_prevents_clean_close|H2|MP-SB-11
R9|mother_end|-|CloseSeal::begin_mother_end / observe_mother_end|sb3_post_seal_model_events_are_absent|H5|MP-SB-9-meter
R9|primary_end|-|HostSession::close_primary session_closed record|sb3_post_seal_model_events_are_absent|H5|MP-SB-9-meter
R9|delivery|-|CloseSeal::finish_delivery|sb3_post_seal_model_events_are_absent|H5|MP-SB-9-meter
";
const NA_REASONS: &str = "BOOTSTRAP_NO_MISSION PREBIND_NO_RUN_OR_CONSENT NO_INVOCATION_AT_REGISTRATION METADATA_HAS_NO_CHARGE SETTLEMENT_HAS_NO_NEW_WORK_ADMISSION LIFECYCLE_DENIES_BEFORE_THIS_SINK LEGACY_FINALIZATION_ONLY";
/// The earlier denial oracles a lifecycle N/A may name (spec 8.4).
const DENIAL_ORACLES: &str =
    "sb3c_registration_is_bootstrap_only_once sb2_unjoined_or_owed_prevents_clean_close";
const OWNERS: &str = "D1 D2 F1";
/// SB3C-R8 (d): every cell's audited disposition: dimension|values|disposition|sinks.
/// A false citation, N/A or OPEN cannot match; the audit decided which holds
/// (register 14.6). R18 precedent for OPEN.
const CELLS: &str = "\
lifecycle|Bootstrap|c:sb2_one_session_operation_active|registration_reserve
lifecycle|Normal,Emergency,Failed,ClosingNormal,ClosingRestricted,SealedFinalizing,ClosedClean,ClosedUnclean|open:D2|registration_reserve,call_slot_reserve,cancel_run,door_halt
ingress|CR,LF,CRLF,original_cell_unscheduled,repeated,EOF,partial_Ctrl_D,master_close,read_failure,stop_all,quit,fatal|open:D2|registration_reserve,registration_record
ingress|mission_stop,door_fence,budget,grant_revocation,grant_expiry|na:BOOTSTRAP_NO_MISSION|registration_reserve,registration_record,registration_install,registration_readiness,registration_version,registration_status,registration_tags,registration_show
fault|none|c:sb2_one_session_operation_active|registration_reserve
fault|decline|na:BOOTSTRAP_NO_MISSION|registration_reserve,registration_record,registration_install,registration_readiness,registration_version,registration_status,registration_tags,registration_show
fault|record_failed,record_unknown,poison,panic|open:D2|registration_reserve,registration_install,producer_join
schedule|before|open:D2|registration_reserve,registration_record,call_slot_reserve,mother_end,primary_end,delivery
schedule|after|c:sb2_bootstrap_cannot_overtake_registration|registration_reserve
lifecycle|Bootstrap|c:sb2_registration_records_before_install_once|registration_record
lifecycle|Normal|na:LIFECYCLE_DENIES_BEFORE_THIS_SINK(sb3c_registration_is_bootstrap_only_once)|registration_record,registration_install,registration_version,registration_status,registration_tags,registration_show
lifecycle|Emergency,Failed,ClosingNormal,ClosingRestricted|open:D2|registration_record
lifecycle|SealedFinalizing,ClosedClean,ClosedUnclean|na:LIFECYCLE_DENIES_BEFORE_THIS_SINK(sb2_unjoined_or_owed_prevents_clean_close)|registration_record,registration_install,attempt_ticket,charge_append,meter_settle,registration_version,registration_status,registration_tags,registration_show,call_version,call_status,call_tags,call_chat,attempt_record,attempt_pin_release,operation_complete
fault|none|c:sb2_registration_records_before_install_once|registration_record
fault|record_failed|c:sb2_unrecorded_failed_or_late_registration_is_absent|registration_record,registration_readiness
fault|record_unknown,poison,panic|open:D2|registration_record,registration_readiness
schedule|after|c:sb2_unrecorded_failed_or_late_registration_is_absent|registration_record
lifecycle|Bootstrap|c:sb2_install_completion_is_atomic_to_close_observers|registration_install,close_obligations
lifecycle|Emergency|c:sb2_unrecorded_failed_or_late_registration_is_absent|registration_install,registration_readiness
lifecycle|Failed,ClosingNormal,ClosingRestricted|open:D2|registration_install,registration_version,registration_status,registration_tags,registration_show
ingress|CR,LF,CRLF|c:sb2_unrecorded_failed_or_late_registration_is_absent|registration_install,registration_readiness
ingress|original_cell_unscheduled,repeated,EOF,partial_Ctrl_D,master_close,read_failure,stop_all,quit,fatal|open:D2|registration_install,registration_readiness,registration_version,registration_status,registration_tags,registration_show
fault|none|c:sb2_install_completion_is_atomic_to_close_observers|registration_install
schedule|before|c:sb2_unrecorded_failed_or_late_registration_is_absent|registration_install
schedule|after|c:sb2_install_completion_is_atomic_to_close_observers|registration_install
lifecycle|Bootstrap|c:sb2_bootstrap_cannot_overtake_registration|registration_readiness
lifecycle|Normal,Failed,ClosingNormal,ClosingRestricted,SealedFinalizing,ClosedClean,ClosedUnclean|open:D2|registration_readiness
fault|none|c:sb3c_registration_is_bootstrap_only_once|registration_readiness
schedule|before|c:sb2_bootstrap_cannot_overtake_registration|registration_readiness
schedule|after|c:sb3c_registration_is_bootstrap_only_once|registration_readiness
lifecycle|Bootstrap|na:BOOTSTRAP_NO_MISSION|identity_view,call_slot_reserve,attempt_ticket,charge_append,meter_settle,call_version,call_status,call_tags,call_chat,cancel_run,door_halt,attempt_record,attempt_pin_release,operation_complete,record_first_classify,post_record_recheck,lane_result_admission
lifecycle|Normal,Emergency|c:sb3c_identity_is_original_request_read_only|identity_view
lifecycle|Failed,ClosingNormal,ClosingRestricted,SealedFinalizing,ClosedClean,ClosedUnclean|open:D2|identity_view,inference_binding
ingress|CR,LF,CRLF|c:sb3c_identity_is_original_request_read_only|identity_view
ingress|original_cell_unscheduled,repeated,EOF,partial_Ctrl_D,master_close,read_failure,stop_all,door_fence,quit,fatal|open:D2|identity_view
ingress|mission_stop,budget,grant_revocation,grant_expiry|na:PREBIND_NO_RUN_OR_CONSENT|identity_view
fault|none,poison|c:sb3c_identity_is_original_request_read_only|identity_view
fault|decline,record_failed,record_unknown,panic|open:D2|identity_view,call_slot_reserve,call_chat
schedule|before|c:sb3c_identity_is_original_request_read_only|identity_view
schedule|after|open:D2|identity_view,inference_binding,attempt_pin_release
lifecycle|Bootstrap|na:PREBIND_NO_RUN_OR_CONSENT|inference_binding
lifecycle|Normal|c:sb3c_public_binding_is_exact_and_idempotent|inference_binding
lifecycle|Emergency|c:sb3c_stop_before_public_binding_installs_nothing|inference_binding
ingress|CR,LF,CRLF|c:sb3c_stop_before_public_binding_installs_nothing|inference_binding
ingress|original_cell_unscheduled,repeated,EOF,partial_Ctrl_D,master_close,read_failure,mission_stop,stop_all,door_fence,budget,quit,fatal|open:D2|inference_binding
ingress|grant_revocation,grant_expiry|c:sb3_original_file_grant_remains_required|inference_binding
fault|none|c:sb3c_public_binding_is_exact_and_idempotent|inference_binding
fault|decline,record_failed,record_unknown,poison,panic|open:D2|inference_binding,attempt_ticket,emergency_restrict,close_restrict
schedule|before|c:sb3c_stop_before_public_binding_installs_nothing|inference_binding
ingress|CR,LF,CRLF,original_cell_unscheduled,repeated,EOF,partial_Ctrl_D,master_close,read_failure,mission_stop,stop_all,door_fence,budget,grant_revocation,grant_expiry,quit,fatal|open:D2|call_slot_reserve,charge_append,meter_settle,cancel_run,door_halt,operation_complete,close_report
fault|none,poison|c:sb3_matrix_r3_reserve_before|call_slot_reserve
schedule|after|c:sb3_matrix_r3_reserve_after|call_slot_reserve
lifecycle|Normal,Emergency,Failed,ClosingNormal,ClosingRestricted|c:sb3_matrix_r3_cas_before|attempt_ticket
ingress|CR,LF,CRLF,door_fence,budget,quit,fatal|c:sb3_matrix_r3_cas_before|attempt_ticket
ingress|original_cell_unscheduled,repeated,EOF,partial_Ctrl_D,master_close,read_failure,mission_stop,stop_all,grant_revocation,grant_expiry|open:D2|attempt_ticket
fault|none|c:sb3_matrix_r3_cas_before|attempt_ticket
schedule|before|c:sb3_matrix_r3_cas_before|attempt_ticket
schedule|after|c:sb3_matrix_r3_cas_after|attempt_ticket
lifecycle|Normal,Emergency,Failed,ClosingNormal,ClosingRestricted|open:D2|charge_append,meter_settle,operation_complete
fault|none,decline,record_unknown,poison,panic|open:D2|charge_append
fault|record_failed|x:sb3_durable_charge_precedes_chat|charge_append
schedule|before,after|open:D2|charge_append,meter_settle,cancel_run,door_halt,emergency_restrict,close_restrict,operation_complete,close_obligations,close_report,producer_join
fault|none,record_failed,record_unknown,poison,panic|open:D2|meter_settle,close_report
fault|decline|na:SETTLEMENT_HAS_NO_NEW_WORK_ADMISSION|meter_settle,attempt_record,attempt_pin_release,operation_complete,record_first_classify,post_record_recheck,close_obligations,close_report,producer_join,seal
lifecycle|Bootstrap|c:session_override_required_at_each_request|registration_version,registration_status,registration_tags,registration_show
lifecycle|Emergency|c:pre_gate_ingress_request_count_eq_zero|registration_version,registration_status,registration_tags,registration_show
ingress|CR,LF,CRLF|c:pre_gate_ingress_request_count_eq_zero|registration_version,registration_status,registration_tags,registration_show
fault|none|c:session_override_required_at_each_request|registration_version,registration_status,registration_tags,registration_show
fault|record_failed,record_unknown|na:METADATA_HAS_NO_CHARGE|registration_version,registration_status,registration_tags,registration_show,call_version,call_status,call_tags
fault|poison,panic|open:D2|registration_version,registration_status,registration_tags,registration_show
schedule|before|c:sb2_paused_before_cas_ingress_wins_dispatches_nothing|registration_version,registration_status,registration_tags,registration_show
schedule|after|c:sb2_held_registration_request_cancels_and_joins|registration_version,registration_status,registration_tags,registration_show
lifecycle|Normal,Emergency,Failed,ClosingNormal,ClosingRestricted|c:sb3_matrix_r5_observer_before|call_version,call_status,call_tags,call_chat
ingress|CR,LF,CRLF,mission_stop,door_fence,budget,grant_revocation,grant_expiry,quit,fatal|c:sb3_matrix_r5_observer_before|call_version,call_status,call_tags,call_chat
ingress|original_cell_unscheduled,repeated,EOF,partial_Ctrl_D,master_close,read_failure,stop_all|open:D2|call_version,call_status,call_tags,call_chat,attempt_record,attempt_pin_release
fault|none,poison|c:sb3_matrix_r5_observer_before|call_version,call_status,call_tags,call_chat
fault|decline,panic|open:D2|call_version,call_status,call_tags
schedule|before|c:sb3_matrix_r5_observer_before|call_version,call_status,call_tags,call_chat
schedule|after|c:sb3_matrix_r5_cas_after|call_version,call_status,call_tags,call_chat
fault|none,decline,record_failed,record_unknown,poison,panic|open:D2|cancel_run,door_halt
lifecycle|Bootstrap,Emergency,Failed,ClosingNormal,ClosingRestricted,SealedFinalizing,ClosedClean,ClosedUnclean|open:D2|emergency_restrict,close_restrict
lifecycle|Normal|c:sb3_held_request_cancels_and_joins|emergency_restrict,close_restrict
ingress|CR,LF,CRLF|c:sb3_held_request_cancels_and_joins|emergency_restrict
ingress|original_cell_unscheduled,repeated,EOF,partial_Ctrl_D,master_close,read_failure,mission_stop,stop_all,door_fence,budget,grant_revocation,grant_expiry,quit,fatal|open:D2|emergency_restrict
fault|none|c:sb3_held_request_cancels_and_joins|emergency_restrict,close_restrict
ingress|CR,LF,CRLF,original_cell_unscheduled,repeated,EOF,partial_Ctrl_D,master_close,read_failure,mission_stop,stop_all,door_fence,budget,grant_revocation,grant_expiry,fatal|open:D2|close_restrict,close_obligations,producer_join,seal
ingress|quit|c:sb3_held_request_cancels_and_joins|close_restrict
lifecycle|Normal,Emergency,Failed,ClosingNormal,ClosingRestricted|c:sb3_matrix_record_before|attempt_record
ingress|CR,LF,CRLF,mission_stop,door_fence,budget,grant_revocation,grant_expiry,quit,fatal|c:sb3_matrix_record_before|attempt_record
fault|none|c:sb3_matrix_record_before|attempt_record
fault|record_failed|c:sb2_record_debt_blocks_seal_and_reuse|attempt_record
fault|record_unknown,poison|open:D2|attempt_record
fault|panic|c:sb2_attempt_drop_panic_and_poison_retain_ownership|attempt_record
schedule|before|c:sb3_matrix_record_before|attempt_record
schedule|after|c:sb3_matrix_settle_before|attempt_record
lifecycle|Normal,Emergency,Failed,ClosingNormal,ClosingRestricted|c:sb3_matrix_settle_before|attempt_pin_release
ingress|CR,LF,CRLF,mission_stop,door_fence,budget,grant_revocation,grant_expiry,quit,fatal|c:sb3_matrix_settle_before|attempt_pin_release
fault|none,poison|c:sb3_matrix_settle_before|attempt_pin_release
fault|record_failed,record_unknown,panic|open:D2|attempt_pin_release
schedule|before|c:sb3_matrix_settle_before|attempt_pin_release
fault|none,record_failed,record_unknown,panic|open:D2|operation_complete
fault|poison|c:sb2_owner_completion_is_bounded_and_poison_fails_closed|operation_complete
lifecycle|Normal,Emergency,Failed,ClosingNormal,ClosingRestricted,SealedFinalizing,ClosedClean,ClosedUnclean|open:D1|record_first_classify,post_record_recheck
ingress|CR,LF,CRLF,original_cell_unscheduled,repeated,EOF,partial_Ctrl_D,master_close,read_failure,mission_stop,stop_all,door_fence,budget,grant_revocation,grant_expiry,quit,fatal|open:D1|record_first_classify,post_record_recheck,lane_result_admission
fault|none,record_failed,record_unknown,poison,panic|open:D1|record_first_classify,post_record_recheck
schedule|before,after|open:D1|record_first_classify,post_record_recheck
lifecycle|Normal|c:sb3_registered_session_executes_one_recorded_call|lane_result_admission
lifecycle|Emergency,Failed,ClosingNormal,ClosingRestricted,SealedFinalizing,ClosedClean,ClosedUnclean|open:D1|lane_result_admission
fault|none|c:sb3_registered_session_executes_one_recorded_call|lane_result_admission
fault|decline,record_failed,record_unknown,poison,panic|open:D1|lane_result_admission
schedule|before|x:sb3_revocation_after_chat_entry_is_entered_only|lane_result_admission
schedule|after|open:D1|lane_result_admission
lifecycle|Normal|c:sb3_open_model_request_at_deadline_blocks_seal|close_obligations
lifecycle|Emergency,Failed,ClosingNormal,ClosingRestricted,SealedFinalizing,ClosedClean,ClosedUnclean|open:D2|close_obligations
ingress|quit|c:sb3_open_model_request_at_deadline_blocks_seal|close_obligations
fault|none|c:sb3_open_model_request_at_deadline_blocks_seal|close_obligations
fault|record_failed,poison|c:sb3_unjoined_or_owed_prevents_clean_close|close_obligations
fault|record_unknown,panic|open:D2|close_obligations
lifecycle|Bootstrap,Normal,Emergency,Failed,ClosingNormal,ClosingRestricted,SealedFinalizing,ClosedClean,ClosedUnclean|open:D2|close_report
lifecycle|Bootstrap,Normal,Emergency,Failed,ClosingRestricted,SealedFinalizing,ClosedClean,ClosedUnclean|open:D2|producer_join,seal
lifecycle|ClosingNormal|c:sb3_completed_call_allows_existing_clean_close|producer_join,seal
ingress|quit|c:sb3_completed_call_allows_existing_clean_close|producer_join
fault|none|c:sb3_completed_call_allows_existing_clean_close|producer_join,seal
ingress|quit|c:sb3_unjoined_or_owed_prevents_clean_close|seal
fault|record_failed,poison,panic|c:sb3_unjoined_or_owed_prevents_clean_close|seal
fault|record_unknown|open:D2|seal
schedule|before|c:sb3_unjoined_or_owed_prevents_clean_close|seal
schedule|after|c:sb3_post_seal_model_events_are_absent|seal
lifecycle|Bootstrap,Normal,Emergency,Failed,ClosingNormal,ClosingRestricted,ClosedClean,ClosedUnclean|na:LEGACY_FINALIZATION_ONLY|mother_end,primary_end,delivery
lifecycle|SealedFinalizing|c:sb3_post_seal_model_events_are_absent|mother_end,primary_end,delivery
ingress|CR,LF,CRLF,original_cell_unscheduled,repeated,EOF,partial_Ctrl_D,master_close,read_failure,mission_stop,stop_all,door_fence,grant_revocation,grant_expiry,quit,fatal|na:LEGACY_FINALIZATION_ONLY|mother_end
ingress|budget|open:F1|mother_end
fault|none|c:sb3_post_seal_model_events_are_absent|mother_end,primary_end,delivery
fault|decline,record_failed,record_unknown,poison,panic|na:LEGACY_FINALIZATION_ONLY|mother_end,primary_end,delivery
schedule|after|na:LEGACY_FINALIZATION_ONLY|mother_end,primary_end,delivery
ingress|CR,LF,CRLF,original_cell_unscheduled,repeated,EOF,partial_Ctrl_D,master_close,read_failure,mission_stop,stop_all,door_fence,budget,grant_revocation,grant_expiry,quit,fatal|na:LEGACY_FINALIZATION_ONLY|primary_end,delivery
";
/// SB3C-R6 (a): class|session-level primitive|retained evidence for the
/// reduction|session-level test. SB3C-R8 (b): covered only by a test that
/// performs the reduction, order and repetition included: else `OPEN:<owner>`;
/// no retained evidence at all: `none`.
const INGRESS_MAP: &str = "\
CR|SessionIngress::emergency|review1_real_cr_lf_crlf_stop_during_publish_hold|sb3_matrix_r5_observer_before
LF|SessionIngress::emergency|review1_real_cr_lf_crlf_stop_during_publish_hold|sb3_matrix_r5_observer_before
CRLF|SessionIngress::emergency|review1_real_cr_lf_crlf_stop_during_publish_hold|sb3_matrix_r5_observer_before
original_cell_unscheduled|HaltCell::engage_emergency on the session word|session_setup_each_final_boundary_has_its_own_unscheduled_ingress_control|OPEN:D2
repeated|SessionIngress::emergency twice|m2dd_original_ingress_invalidates_queued_work_and_repeated_commands|OPEN:D2
EOF|StopRoute::press then SessionIngress::fail|d_o20_stop_ctrl_d_and_master_close_before_publication|OPEN:D2
partial_Ctrl_D|partial line then Ctrl-D, no retained test performs it|none|OPEN:D2
master_close|StopRoute::press then SessionIngress::fail|d_o20_stop_ctrl_d_and_master_close_before_publication|OPEN:D2
read_failure|StopRoute::press then SessionIngress::fail|d_o20_read_error_during_held_execution_engages_stop_before_exit|OPEN:D2
mission_stop|Coordinator::stop to SessionDoor::cancel_run|session_literal_thirteen_states_against_real_terminal_paths|sb3_matrix_r5_observer_before
stop_all|Coordinator::stop_all, no retained session-route evidence|none|OPEN:D2
door_fence|SessionPort::fence to SessionDoor::halt|session_literal_thirteen_states_against_real_terminal_paths|sb3_matrix_r5_observer_before
budget|mission wall and monotonic deadline|fix1_original_budget_is_rechecked_under_final_run_hold|sb3_matrix_r5_observer_before
grant_revocation|original grant revocation|session_each_native_work_step_rechecks_its_actual_prerequisites|sb3_matrix_r5_observer_before
grant_expiry|original grant expiry|session_each_native_work_step_rechecks_its_actual_prerequisites|sb3_matrix_r5_observer_before
quit|SessionIngress::accept(Quit)|quit_during_dialogue_cancels_without_waiting_for_an_answer|sb3_matrix_r5_observer_before
fatal|SessionIngress::fail|fix1_b_first_failure_deadline_cannot_be_refreshed_by_concurrent_quit|sb3_matrix_r5_observer_before
";
const RECORDED: [&str; 3] = [
    "SB3C/record/foreign_r\tN/A:reachable-limited\tone File port per session (file_session.rs:529)\tSB3C-R6 (e): public foreign-R row not constructible; second-file-port refusal plus retained private citations stand\tSB-3C",
    "SB3C/record/poison_session_port\tOPEN:D2\tno existing test seam reaches a broker hold from Mother\tSB3C-R6 (f): poisoned-broker SessionPort case needs a production seam; not claimed\tSB-3C",
    "SB3C/guard/lifecycle_states\tsb3c_lifecycle_list_matches_the_session_states\tSessionState (9 states; the forecast \"10\" was an estimate)\tSB3C-R6 (b): guard test fails on divergence\tSB-3C",
];
const C2_ORACLES: &str = include_str!("../../src/adapter/session_model_composition_tests.rs");

macro_rules! sources {
    ($($file:expr),+ $(,)?) => { [$(include_str!($file)),+].concat() };
}
fn evidence_sources() -> String {
    sources![
        "../../../crates/nexus-governed-control/src/host_session/model_tests.rs",
        "../../../crates/nexus-governed-control/src/host_session/model_call_tests.rs",
        "../../../crates/nexus-governed-control/src/host_session/model_exposure_tests.rs",
        "../../src/adapter/session_model_bridge_tests.rs",
        "../src/hands/sb2_close_tests.rs",
    ]
}
fn host_evidence_sources() -> String {
    sources![
        "faults.rs",
        "pty.rs",
        "../src/stop_tests.rs",
        "../../src/adapter/session_terminal_oracles_tests.rs",
        "../../../crates/nexus-governed-control/src/governed/session_setup_tests.rs",
        "../../../crates/nexus-governed-control/src/governed/session_budget_fix1_tests.rs",
        "../../../crates/nexus-governed-control/src/governed/session_lifecycle_fix1_tests.rs",
        "../../../crates/nexus-governed-control/src/governed/session_routes_tests.rs",
    ]
}
/// A typo lint on a cited name, and nothing more (SB3C-R10 (b)): a column-0
/// `fn` under a plain `#[test]`, read as source text. It does not establish
/// that a test is compiled, enabled or run. Forms known to pass it although
/// the test would not run: `#[ignore]` or `#[cfg(any())]` above a comment line
/// ending in `;` (`/* note */ // x;`); a whole test inside a nested block
/// comment after an item-like line; a file-level `#! [cfg(any())]` with the
/// space; a `cfg` on an enclosing module's `mod` declaration in a parent file.
/// What ran is the receipt check outside this file (register 14.10).
fn is_test(name: &str, source: &str) -> bool {
    let definition = format!("\nfn {name}() {{");
    let Some(at) = source.find(&definition) else {
        return false;
    };
    if source.matches(&definition).count() != 1 {
        return false;
    }
    let before = &source[..at];
    let mut tests = 0;
    for line in before.lines().rev() {
        let text = line.trim();
        if text.is_empty() {
            continue;
        }
        if text.starts_with("//") {
            let disabled = ["#[ignore", "#[test]", &format!("fn {name}(")];
            if disabled.iter().any(|d| text.contains(d)) {
                return false;
            }
        } else if text.starts_with("#[") {
            if line != "#[test]" {
                return false;
            }
            tests += 1;
        } else {
            let item_level = text.ends_with('}') || text.ends_with(';');
            return tests == 1 && item_level && !in_block_comment(before);
        }
    }
    false
}
/// A line heuristic for the lint above: nesting is not tracked.
fn in_block_comment(before: &str) -> bool {
    let delimiter = |l: &&str| l.trim_start().starts_with("/*") || l.contains("*/");
    let nearest = before.lines().rev().find(delimiter);
    nearest.is_some_and(|l| !l.contains("*/"))
}
fn words(s: &'static str) -> Vec<&'static str> {
    s.split(' ').collect()
}

/// Cell counts per disposition: `cited`, `executed`, `na:R`, `open:O`.
#[derive(Debug, Default, PartialEq)]
pub struct Summary {
    pub rows: usize,
    pub cells: usize,
    pub by_disposition: BTreeMap<String, usize>,
}

/// The frozen sink: [route, gate, boundary, primitive, oracle, plant].
type Frozen = [&'static str; 6];
fn frozen_sinks() -> BTreeMap<&'static str, Frozen> {
    let row = |line: &'static str| {
        let f: Vec<_> = line.split('|').collect();
        (f[1], [f[0], f[2], f[3], f[4], f[5], f[6]])
    };
    ROWS.lines().map(row).collect()
}
struct Context {
    frozen: BTreeMap<&'static str, Frozen>,
    cells: BTreeMap<(String, String, String), &'static str>,
    sources: String,
    receipts: BTreeSet<&'static str>,
    named_tests: RefCell<BTreeMap<String, bool>>,
}
impl Context {
    /// `is_test` once per name: the block walk re-reads the sources.
    fn names_test(&self, name: &str) -> bool {
        let mut seen = self.named_tests.borrow_mut();
        let entry = seen.entry(name.to_owned());
        *entry.or_insert_with(|| is_test(name, &self.sources))
    }
}
fn context(sources: String) -> Context {
    let mut cells = BTreeMap::new();
    for line in CELLS.lines() {
        let f: Vec<_> = line.split('|').collect();
        for (sink, value) in f[3]
            .split(',')
            .flat_map(|s| f[1].split(',').map(move |v| (s, v)))
        {
            let cell = (sink.to_owned(), f[0].to_owned(), value.to_owned());
            assert!(cells.insert(cell, f[2]).is_none(), "duplicate cell: {line}");
        }
    }
    assert_eq!(cells.len(), 35 * 34, "the audited table lists every cell");
    let receipts = include_str!("session/combination-receipts.tsv")
        .lines()
        .skip(1)
        .filter_map(|l| l.split('\t').next())
        .collect();
    Context {
        frozen: frozen_sinks(),
        cells,
        sources,
        receipts,
        named_tests: RefCell::default(),
    }
}

fn check_disposition(
    c: &Context,
    (sink, dimension, value): (&str, &str, &str),
    disposition: &str,
) -> Result<String, String> {
    let at = format!("{sink}/{dimension}/{value}");
    let (kind, rest) = disposition
        .split_once(':')
        .ok_or_else(|| format!("{at}: no disposition in {disposition:?}"))?;
    match kind {
        "c" | "x" => {
            if c.receipts.contains(rest) {
                return Err(format!("{at}: historical receipt {rest} is not a test"));
            }
            if kind == "x" && !is_test(rest, C2_ORACLES) {
                return Err(format!("{at}: x: only a C2-new oracle may claim execution"));
            }
            if kind == "c" && !c.names_test(rest) {
                return Err(format!("{at}: no such retained test {rest}"));
            }
            listed(c, (sink, dimension, value), disposition)?;
            Ok(if kind == "x" { "executed" } else { "cited" }.to_owned())
        }
        "na" => {
            let (reason, oracle) = match rest.strip_suffix(')') {
                Some(inner) => inner
                    .split_once('(')
                    .map(|(r, o)| (r, Some(o)))
                    .ok_or_else(|| format!("{at}: malformed N/A"))?,
                None => (rest, None),
            };
            if !words(NA_REASONS).contains(&reason) {
                return Err(format!(
                    "{at}: N/A reason {reason} is not on the closed list"
                ));
            }
            let denies = reason == "LIFECYCLE_DENIES_BEFORE_THIS_SINK";
            match oracle {
                Some(o) if denies => {
                    let named = words(DENIAL_ORACLES).contains(&o);
                    if !named || !c.names_test(o) {
                        return Err(format!("{at}: {o} is not this sink's denial oracle"));
                    }
                }
                None if !denies => {}
                _ => return Err(format!("{at}: {reason} and its oracle do not agree")),
            }
            listed(c, (sink, dimension, value), disposition)?;
            Ok(format!("na:{reason}"))
        }
        "open" => {
            if !words(OWNERS).contains(&rest) {
                return Err(format!("{at}: OPEN needs an owner, got {rest}"));
            }
            listed(c, (sink, dimension, value), disposition)?;
            Ok(format!("open:{rest}"))
        }
        _ => Err(format!("{at}: unknown disposition {disposition:?}")),
    }
}

/// SB3C-R8 (d): the disposition must be the one the audit recorded.
fn listed(
    c: &Context,
    (sink, dimension, value): (&str, &str, &str),
    disposition: &str,
) -> Result<(), String> {
    let cell = (sink.to_owned(), dimension.to_owned(), value.to_owned());
    match c.cells.get(&cell) {
        Some(audited) if *audited == disposition => Ok(()),
        _ => Err(format!(
            "{sink}/{dimension}/{value}: {disposition} is not the audited disposition"
        )),
    }
}

fn check_row(c: &Context, f: &[&str], summary: &mut Summary) -> Result<(), String> {
    let (route, sink) = (f[0], f[1]);
    let Some([owner_route, gate, boundary, primitive, oracle, plant]) = c.frozen.get(sink) else {
        return Err(format!("foreign sink {sink}"));
    };
    if route != *owner_route || f[2] != *gate {
        return Err(format!("{sink}: wrong route or gate ({route}, {})", f[2]));
    }
    if f[3] != *boundary {
        return Err(format!("{sink}: boundary {:?} is not its boundary", f[3]));
    }
    let known = c.names_test(f[8]) || is_test(f[8], C2_ORACLES);
    if f[8] != *primitive || !known {
        return Err(format!("{sink}: primitive {} is not its evidence", f[8]));
    }
    if f[9] != *oracle || f[10] != *plant {
        return Err(format!("{sink}: oracle/plant {}/{} is wrong", f[9], f[10]));
    }
    for (column, (dimension, values)) in DIMENSIONS.iter().enumerate() {
        let values = words(values);
        let mut covered = BTreeSet::new();
        for clause in f[4 + column].split(';') {
            let (mask, disposition) = clause
                .split_once('=')
                .ok_or_else(|| format!("{sink}/{dimension}: malformed clause {clause}"))?;
            for value in mask.split(',') {
                if !values.contains(&value) {
                    return Err(format!("{sink}/{dimension}: unknown value {value}"));
                }
                if !covered.insert(value) {
                    return Err(format!("{sink}/{dimension}: duplicate value {value}"));
                }
                let key = check_disposition(c, (sink, dimension, value), disposition)?;
                *summary.by_disposition.entry(key).or_default() += 1;
                summary.cells += 1;
            }
        }
        if covered.len() != values.len() {
            return Err(format!("{sink}/{dimension}: omitted a value"));
        }
    }
    Ok(())
}

/// Verifies a sink file's text.
pub fn verify(tsv: &str) -> Result<Summary, String> {
    verify_in(tsv, evidence_sources())
}
/// `verify` against `sources`, so a control can plant an ignored test.
fn verify_in(tsv: &str, sources: String) -> Result<Summary, String> {
    if sources.contains("#![cfg") {
        return Err("a file-level cfg in the evidence sources".into());
    }
    let c = context(sources);
    let mut lines = tsv.lines();
    if lines.next() != Some(HEADER) {
        return Err("missing or altered header".into());
    }
    let rows: Vec<&str> = lines.collect();
    if rows.len() != c.frozen.len() {
        return Err(format!("expected 35 data rows, found {}", rows.len()));
    }
    let mut seen = BTreeSet::new();
    let mut summary = Summary::default();
    for line in rows {
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() != 11 || f.iter().any(|field| field.is_empty()) {
            return Err(format!("malformed row: {line}"));
        }
        if !seen.insert(f[1]) {
            return Err(format!("duplicate sink {}", f[1]));
        }
        check_row(&c, &f, &mut summary)?;
        summary.rows += 1;
    }
    let gates = |p: &str| c.frozen.values().filter(|s| s[1].starts_with(p)).count();
    if (gates("reg-"), gates("call-")) != (4, 4) || summary.rows != 35 {
        return Err("35 sinks, four registration and four call gates required".into());
    }
    Ok(summary)
}

fn session_file(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/session")
        .join(name)
}
fn read(path: &Path) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))
}

/// Verifies the `SB-3C` block of `model-bridge-cases.tsv`: the ingress
/// reductions exactly as listed, and the recorded SB3C-R6 rows.
pub fn verify_cases(cases: &str) -> Result<usize, String> {
    verify_cases_in(cases, evidence_sources())
}
fn verify_cases_in(cases: &str, sources: String) -> Result<usize, String> {
    let host = host_evidence_sources();
    let mut block: Vec<Vec<&str>> = Vec::new();
    for line in cases.lines() {
        let row: Vec<&str> = line.split('\t').collect();
        if row[0].starts_with("SB3C/") || row.contains(&"SB-3C") {
            if row.len() != 5 || row[4] != "SB-3C" {
                return Err(format!("malformed SB-3C row: {line}"));
            }
            block.push(row);
        }
    }
    let mut remaining: BTreeSet<String> = words(INGRESS)
        .iter()
        .map(|i| format!("SB3C/ingress/{i}"))
        .chain(RECORDED.map(|l| l.split('\t').next().unwrap().to_owned()))
        .collect();
    for row in &block {
        if !remaining.remove(row[0]) {
            return Err(format!("unknown or duplicate SB-3C row {}", row[0]));
        }
        let Some(class) = row[0].strip_prefix("SB3C/ingress/") else {
            if !RECORDED.contains(&row.join("\t").as_str()) {
                return Err(format!("{}: recorded row altered", row[0]));
            }
            continue;
        };
        let line = INGRESS_MAP
            .lines()
            .find(|l| l.split('|').next() == Some(class))
            .ok_or_else(|| format!("{class}: no listed reduction"))?;
        let f: Vec<_> = line.split('|').collect();
        let (session_test, owned) = (f[3], f[3].starts_with("OPEN:"));
        if row[2] != f[1] || row[3] != format!("host:{}", f[2]) || row[1] != session_test {
            return Err(format!(
                "{class}: the reduction differs from the listed one"
            ));
        }
        if !owned && (f[2] == "none" || !is_test(session_test, &sources)) {
            return Err(format!("{class}: no session-level primitive named"));
        }
        if f[2] != "none" && !is_test(f[2], &host) {
            return Err(format!("{class}: no such retained evidence {}", f[2]));
        }
    }
    if !remaining.is_empty() {
        return Err(format!("missing SB-3C rows: {remaining:?}"));
    }
    Ok(block.len())
}

/// Both supplement checks, on the checked-in files (`session_oracles.rs`).
pub fn verify_supplement() -> Result<Summary, String> {
    verify_cases(&read(&session_file("model-bridge-cases.tsv"))?)?;
    verify(&read(&session_file("model-bridge-sinks.tsv"))?)
}

/// `pub enum SessionState { .. }`'s variants, from source text.
fn session_states(source: &str) -> Vec<String> {
    let body = source
        .split_once("pub enum SessionState {")
        .and_then(|(_, rest)| rest.split_once('}'))
        .map_or("", |(body, _)| body);
    let states = body.split(',').map(|v| v.trim().to_owned());
    states.filter(|v| !v.is_empty()).collect()
}

#[test]
fn sb3c_supplement_is_complete() {
    let summary = verify_supplement().unwrap();
    assert_eq!((summary.rows, summary.cells), (35, 35 * 34));
    println!("R32 supplement: {summary:?}");
}

/// SB3C-R6 (b): a guard, separate from the TSV-only verifier.
#[test]
fn sb3c_lifecycle_list_matches_the_session_states() {
    let code = sources!["../../../crates/nexus-governed-control/src/host_session.rs"];
    assert_eq!(session_states(&code), words(LIFECYCLE));
    let added = code.replace(
        "    ClosedUnclean,\n}",
        "    ClosedUnclean,\n    Paused,\n}",
    );
    assert_ne!(
        session_states(&added),
        words(LIFECYCLE),
        "divergence control"
    );
}

fn real() -> String {
    read(&session_file("model-bridge-sinks.tsv")).expect("the sink file")
}
fn must_fail(result: Result<Summary, String>, want: &str, spec: &str) {
    match result {
        Ok(_) => panic!("{spec}: tamper accepted"),
        Err(err) => assert!(err.contains(want), "{spec}: wanted {want:?}, got {err}"),
    }
}
/// Applies `f(columns)` to sink's row of the real file, and verifies it.
fn tamper(sink: &str, f: impl FnOnce(&mut Vec<String>)) -> Result<Summary, String> {
    let text = real();
    let mut f = Some(f);
    let mut out = Vec::new();
    for line in text.lines() {
        let mut c: Vec<String> = line.split('\t').map(String::from).collect();
        if c[1] == sink {
            f.take().expect("one row")(&mut c);
        }
        out.push(c.join("\t"));
    }
    verify(&(out.join("\n") + "\n"))
}
/// Plants: sink|column|from|to|wanted error. `from` empty sets the column.
const PLANTS: &str = "\
seal|1||launch|foreign sink
call_chat|0||R3|wrong route
call_tags|2||-|wrong route or gate
registration_show|2||call-4|wrong route or gate
seal|8||sb3_matrix_r3_cas_before|primitive
seal|9||R0|oracle/plant
call_chat|10||MP-SB-2|oracle/plant
seal|6|||malformed row
call_chat|4|Failed,||omitted
call_chat|5|CR,||omitted
call_chat|6|none,||omitted
call_chat|7|;after=c:sb3_matrix_r5_cas_after||omitted
call_chat|5|quit,|quit,escape,|unknown value escape
call_chat|5|quit,|quit,quit,|duplicate value quit
call_chat|5|||malformed row
call_chat|5||CR|malformed clause
call_chat|5|c:sb3_matrix_r5_observer_before|maybe|no disposition
call_chat|5|c:sb3_matrix_r5_observer_before|z:sb3_matrix_r5_observer_before|unknown disposition
call_chat|5|c:sb3_matrix_r5_observer_before|c:no_such_test_exists|no such retained test
call_chat|5|c:sb3_matrix_r5_observer_before|c:sb3c_registration_is_bootstrap_only_once|not the audited
call_chat|5|c:sb3_matrix_r5_observer_before|x:sb3_matrix_r5_cas_after|only a C2-new oracle
call_chat|5|c:sb3_matrix_r5_observer_before|na:BECAUSE|not on the closed list
call_chat|5|c:sb3_matrix_r5_observer_before|na:METADATA_HAS_NO_CHARGE|not the audited
call_chat|5|c:sb3_matrix_r5_observer_before|na:LEGACY_FINALIZATION_ONLY|not the audited
call_chat|5|c:sb3_matrix_r5_observer_before|na:LIFECYCLE_DENIES_BEFORE_THIS_SINK|do not agree
call_chat|5|c:sb3_matrix_r5_observer_before|na:BOOTSTRAP_NO_MISSION(sb3_post_seal_model_events_are_absent)|do not agree
call_chat|5|c:sb3_matrix_r5_observer_before|na:LIFECYCLE_DENIES_BEFORE_THIS_SINK(sb3_matrix_r5_cas_after)|denial oracle
call_chat|5|c:sb3_matrix_r5_observer_before|na:LIFECYCLE_DENIES_BEFORE_THIS_SINK(sb2_unjoined_or_owed_prevents_clean_close)|not the audited
call_chat|4|c:sb3_matrix_r5_observer_before|na:LIFECYCLE_DENIES_BEFORE_THIS_SINK(sb2_unjoined_or_owed_prevents_clean_close)|not the audited
call_chat|5|c:sb3_matrix_r5_observer_before|open:D9|OPEN needs an owner
call_chat|5|c:sb3_matrix_r5_observer_before|open:D2|not the audited
close_report|5|open:D2|c:sb3_completed_call_allows_existing_clean_close|not the audited
";
#[test]
fn sb3c_structural_and_dimension_tampering_fails() {
    verify(&real()).unwrap();
    assert!(read(&session_file("no-such-file.tsv")).is_err(), "absent");
    must_fail(verify(""), "missing or altered header", "empty");
    must_fail(
        verify(&format!("{HEADER}\n")),
        "expected 35 data rows",
        "header only",
    );
    must_fail(
        verify(&real().replacen("route", "route2", 1)),
        "header",
        "header",
    );
    let text = real();
    let rows: Vec<&str> = text.lines().collect();
    let mut dropped = rows.clone();
    dropped.remove(5);
    must_fail(
        verify(&(dropped.join("\n") + "\n")),
        "expected 35",
        "omitted row",
    );
    let mut extra = rows.clone();
    extra.push(rows[3]);
    must_fail(
        verify(&(extra.join("\n") + "\n")),
        "expected 35",
        "extra row",
    );
    let mut duplicate = rows.clone();
    duplicate[2] = rows[1];
    must_fail(
        verify(&(duplicate.join("\n") + "\n")),
        "duplicate sink",
        "duplicate",
    );
    for spec in PLANTS.lines() {
        let f: Vec<_> = spec.split('|').collect();
        let (sink, column, from, to, want) =
            (f[0], f[1].parse::<usize>().unwrap(), f[2], f[3], f[4]);
        let result = tamper(sink, |c: &mut Vec<String>| {
            let edited = c[column].replacen(from, to, 1);
            assert!(
                from.is_empty() || edited != c[column],
                "{spec}: not applied"
            );
            c[column] = if from.is_empty() {
                to.to_owned()
            } else {
                edited
            };
        });
        must_fail(result, want, spec);
    }
    let receipt = include_str!("session/combination-receipts.tsv")
        .lines()
        .nth(1)
        .and_then(|l| l.split('\t').next())
        .unwrap();
    let cite = format!("c:{receipt}");
    let result = tamper("call_chat", |c: &mut Vec<String>| {
        c[5] = c[5].replacen("c:sb3_matrix_r5_observer_before", &cite, 1);
    });
    must_fail(result, "historical receipt", "receipt");
}
/// SB3C-R8 (d): `sink|dimension|value|disposition|wanted error`. The first
/// four are Codex finding 5; the rest would restore a claim the audit
/// dropped (record of the audit: combination-coverage.md).
const CONTROLS: &str = "\
call_chat|schedule|after|c:sb3_matrix_r5_observer_before|not the audited
call_status|fault|none|c:sb2_unjoined_or_owed_prevents_clean_close|not the audited
call_slot_reserve|lifecycle|Emergency|na:LIFECYCLE_DENIES_BEFORE_THIS_SINK(sb2_unjoined_or_owed_prevents_clean_close)|not the audited
registration_version|lifecycle|Bootstrap|na:NO_INVOCATION_AT_REGISTRATION|not the audited
charge_append|lifecycle|Normal|c:sb3_matrix_charge_before|not the audited
charge_append|fault|panic|c:sb3_unconfigured_session_retains_default_denial|not the audited
meter_settle|schedule|before|c:sb3_matrix_settle_before|not the audited
meter_settle|lifecycle|Failed|c:sb3_matrix_settle_before|not the audited
record_first_classify|lifecycle|Normal|c:sb3_matrix_reply_after|not the audited
post_record_recheck|fault|none|c:sb3_matrix_settle_before|not the audited
close_report|lifecycle|ClosingNormal|c:sb3_completed_call_allows_existing_clean_close|not the audited
call_tags|ingress|repeated|c:sb3_matrix_r5_observer_before|not the audited
registration_install|ingress|EOF|c:sb2_unrecorded_failed_or_late_registration_is_absent|not the audited
identity_view|ingress|original_cell_unscheduled|c:sb3c_identity_is_original_request_read_only|not the audited
registration_reserve|fault|panic|c:sb2_owner_drop_never_discharges_debt|not the audited
inference_binding|schedule|after|c:sb3_stop_before_binding_installs_nothing|not the audited
attempt_record|fault|record_unknown|c:sb2_missing_outcome_is_unknown_not_success|not the audited
operation_complete|lifecycle|Normal|c:sb2_owner_completion_is_bounded_and_poison_fails_closed|not the audited
seal|lifecycle|ClosedClean|c:sb3_completed_call_allows_existing_clean_close|not the audited
emergency_restrict|lifecycle|Bootstrap|na:BOOTSTRAP_NO_MISSION|not the audited
close_restrict|lifecycle|Bootstrap|na:BOOTSTRAP_NO_MISSION|not the audited
close_obligations|lifecycle|Bootstrap|na:BOOTSTRAP_NO_MISSION|not the audited
emergency_restrict|lifecycle|ClosedClean|na:LIFECYCLE_DENIES_BEFORE_THIS_SINK(sb3_post_seal_model_events_are_absent)|denial oracle
delivery|lifecycle|SealedFinalizing|na:LEGACY_FINALIZATION_ONLY|not the audited
registration_install|fault|record_failed|c:sb2_unrecorded_failed_or_late_registration_is_absent|not the audited
registration_record|schedule|before|c:sb2_unrecorded_failed_or_late_registration_is_absent|not the audited
attempt_ticket|fault|poison|c:sb3_matrix_r3_cas_before|not the audited
attempt_ticket|ingress|mission_stop|c:sb3_matrix_r3_cas_before|not the audited
attempt_ticket|ingress|grant_revocation|c:sb3_matrix_r3_cas_before|not the audited
attempt_ticket|ingress|grant_expiry|c:sb3_matrix_r3_cas_before|not the audited
attempt_record|fault|poison|c:sb3_matrix_record_before|not the audited
cancel_run|schedule|before|c:sb3_pre_gate_ingress_sends_no_request|not the audited
door_halt|schedule|after|c:sb3_held_request_cancels_and_joins|not the audited
emergency_restrict|schedule|before|c:sb3_pre_gate_ingress_sends_no_request|not the audited
close_restrict|schedule|after|c:sb3_held_request_cancels_and_joins|not the audited
registration_readiness|lifecycle|Normal|na:LIFECYCLE_DENIES_BEFORE_THIS_SINK(sb3c_registration_is_bootstrap_only_once)|not the audited
registration_readiness|lifecycle|ClosedClean|na:LIFECYCLE_DENIES_BEFORE_THIS_SINK(sb2_unjoined_or_owed_prevents_clean_close)|not the audited
registration_reserve|lifecycle|Normal|na:LIFECYCLE_DENIES_BEFORE_THIS_SINK(sb3c_registration_is_bootstrap_only_once)|not the audited
registration_reserve|lifecycle|SealedFinalizing|na:LIFECYCLE_DENIES_BEFORE_THIS_SINK(sb2_unjoined_or_owed_prevents_clean_close)|not the audited
record_first_classify|lifecycle|ClosedClean|na:LIFECYCLE_DENIES_BEFORE_THIS_SINK(sb2_unjoined_or_owed_prevents_clean_close)|not the audited
post_record_recheck|lifecycle|SealedFinalizing|na:LIFECYCLE_DENIES_BEFORE_THIS_SINK(sb2_unjoined_or_owed_prevents_clean_close)|not the audited
";
/// Spellings the `is_test` typo lint catches: label|replacement for `{sig}`.
/// Not a claim about other spellings of the same thing; see `is_test`.
const DISABLED: [(&str, &str); 12] = [
    ("ignore above a comment", "#[ignore]\n// Disabled.\n{sig}"),
    ("ignore past a blank line", "#[ignore]\n\n{sig}"),
    ("ignore inside a comment", "// #[ignore]\n{sig}"),
    (
        "foreign cfg above a comment",
        "#[cfg(windows)]\n// Note.\n{sig}",
    ),
    ("commented-out test attribute", "// {sig}"),
    (
        "block-commented definition",
        "/*\n{sig}\n}\n*/\nfn after() {",
    ),
    (
        "wide block comment",
        "/*\nfn earlier() {}\n\n{sig}\n}\n*/\nfn after() {",
    ),
    ("unindented cfg module", "#[cfg(windows)]\nmod off {\n{sig}"),
    (
        "inside a raw string",
        "const OFF: &str = r#\"\n{sig}\n}\n\"#;\nfn after() {",
    ),
    ("second definition", "{sig}\n}\n{sig}"),
    ("another attribute, no test", "#[allow(dead_code)]\n{def}"),
    ("plain function, no test", "{def}"),
];
/// Sets one cell, whichever clause holds it now.
fn set_cell(sink: &str, dimension: &str, value: &str, to: &str) -> Result<Summary, String> {
    let at = DIMENSIONS.iter().position(|d| d.0 == dimension).unwrap();
    tamper(sink, |c: &mut Vec<String>| {
        let text = c[4 + at].clone();
        let mut cells = BTreeMap::new();
        for clause in text.split(';') {
            let (mask, disposition) = clause.split_once('=').expect("clause");
            cells.extend(mask.split(',').map(|v| (v, disposition)));
        }
        cells.insert(value, to);
        let rows = words(DIMENSIONS[at].1).into_iter();
        let rows = rows.map(|v| format!("{v}={}", cells[v]));
        c[4 + at] = rows.collect::<Vec<_>>().join(";");
    })
}
/// Every control runs; a RED names each one that was not rejected.
#[test]
fn sb3c_r8_tamper_controls_are_rejected() {
    let mut red: Vec<String> = Vec::new();
    let mut check = |label: &str, accepted: Result<(), String>, want: &str| match accepted {
        Ok(()) => red.push(format!("{label}: tamper accepted")),
        Err(e) if e.contains(want) => {}
        Err(e) => red.push(format!("{label}: wanted {want:?}, got {e}")),
    };
    for spec in CONTROLS.lines() {
        let f: Vec<_> = spec.split('|').collect();
        let tampered = set_cell(f[0], f[1], f[2], f[3]).map(|_| ());
        check(spec, tampered, f[4]);
    }
    let boundary = tamper("seal", |c: &mut Vec<String>| {
        c[3] = "any nonempty text".into();
    });
    check("boundary", boundary.map(|_| ()), "boundary");
    let row = tamper("attempt_pin_release", |c: &mut Vec<String>| {
        c[8] = "sb2_missing_outcome_is_unknown_not_success".into();
        c[9] = "A2".into();
        c[10] = "MP-SB-9-record".into();
    });
    check("row binding", row.map(|_| ()), "primitive");
    let signature = "#[test]\nfn sb3_matrix_r5_cas_after() {";
    let ignored = format!("#[ignore]\n{signature}");
    let sources = evidence_sources().replacen(signature, &ignored, 1);
    let run = verify_in(&real(), sources).map(|_| ());
    check("#[ignore] on a cited test", run, "no such retained test");
    for (label, form) in DISABLED {
        let definition = signature.strip_prefix("#[test]\n").unwrap();
        let replaced = form
            .replace("{sig}", signature)
            .replace("{def}", definition);
        let sources = evidence_sources().replacen(signature, &replaced, 1);
        let run = verify_in(&real(), sources).map(|_| ());
        check(label, run, "no such retained test");
    }
    let inner = format!("#![cfg(windows)]\n{}", evidence_sources());
    let run = verify_in(&real(), inner).map(|_| ());
    check("file-level cfg in the sources", run, "file-level cfg");
    let primitive = "#[test]\nfn sb3_concurrent_operation_refuses_before_ticket() {";
    let ignored = format!("#[ignore]\n{primitive}");
    let sources = evidence_sources().replacen(primitive, &ignored, 1);
    let run = verify_in(&real(), sources).map(|_| ());
    check("#[ignore] on a primitive", run, "is not its evidence");
    let text = read(&session_file("model-bridge-cases.tsv")).unwrap();
    let cases = |t: String| verify_cases(&t).map(|_| ());
    let poison = "SB3C/record/poison_session_port\tOPEN:D2";
    let pass = "SB3C/record/poison_session_port\tPASS";
    check(
        "poison disclosure",
        cases(text.replacen(poison, pass, 1)),
        "recorded row altered",
    );
    let orphan = format!("{text}SB3C/ingress/orphan\tt\tb\te\tSB-3C\tx\n");
    check("malformed orphan", cases(orphan), "malformed SB-3C row");
    let session = "#[test]\nfn sb3_matrix_r5_observer_before() {";
    let ignored = format!("#[ignore]\n{session}");
    let sources = evidence_sources().replacen(session, &ignored, 1);
    let run = verify_cases_in(&text, sources).map(|_| ());
    check(
        "#[ignore] on an ingress test",
        run,
        "no session-level primitive named",
    );
    let eof = "SB3C/ingress/EOF\tOPEN:D2";
    let claimed = "SB3C/ingress/EOF\tsb3_matrix_r5_observer_before";
    check(
        "reduction not performed",
        cases(text.replacen(eof, claimed, 1)),
        "differs",
    );
    assert!(
        red.is_empty(),
        "{} controls RED:\n{}",
        red.len(),
        red.join("\n")
    );
}
#[test]
fn sb3c_ingress_block_tampering_fails() {
    let text = read(&session_file("model-bridge-cases.tsv")).unwrap();
    assert_eq!(verify_cases(&text).unwrap(), 20);
    let without = |needle: &str| -> String {
        let kept: Vec<_> = text.lines().filter(|l| !l.starts_with(needle)).collect();
        kept.join("\n")
    };
    for (cases, want) in [
        (without("SB3C/ingress/quit\t"), "missing"),
        (without("SB3C/record/poison"), "missing"),
        (
            format!("{text}SB3C/ingress/quit\tt\tb\te\tSB-3C\n"),
            "duplicate",
        ),
        (
            text.replacen("SessionIngress::accept(Quit)", "SessionIngress::fail", 1),
            "differs",
        ),
        (
            text.replacen(
                "host:quit_during_dialogue_cancels_without_waiting_for_an_answer",
                "host:no_such",
                1,
            ),
            "differs",
        ),
        (
            text.replacen(
                "\tOPEN:D2\tCoordinator::stop_all",
                "\tsb3_matrix_r5_observer_before\tCoordinator::stop_all",
                1,
            ),
            "differs",
        ),
    ] {
        let err = verify_cases(&cases).expect_err(want);
        assert!(err.contains(want), "{want}: got {err}");
    }
}

//! SB-3C R32 supplement (spec 8.2-8.4; SB3C-R6, R8): the 35 route-level sinks
//! of the session model bridge, verified from the TSV alone. No production
//! classifier is called: each row and cell must equal the literal audited
//! tables below (SB3C-R8 (d)), and each cited name must pass a source-text
//! typo lint (`is_test`), which does not show a test is compiled, enabled or
//! run. `c:` cites a retained test only where it executes that sink's own
//! boundary on that value (the SB3C-R8 audit); `x:` is a C2-new oracle. The
//! tables are the audit's record: equality proves a row was not changed since
//! the audit, not that the audit was right. `session_oracles.rs` requires
//! this and the retained 153,000-key matrix validator. `x:` at the three
//! SB3D-R3 sinks may also cite a D1 oracle (session_model_bridge_d_tests.rs).
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
const OWNERS: &str = "D2 F1 M2e-D";
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
ingress|CR,LF,CRLF,original_cell_unscheduled,repeated,EOF,partial_Ctrl_D,master_close,read_failure,mission_stop,stop_all,door_fence,budget,grant_revocation,grant_expiry,quit,fatal|open:D2|call_slot_reserve,charge_append,meter_settle,cancel_run,door_halt,operation_complete
fault|none,poison|c:sb3_matrix_r3_reserve_before|call_slot_reserve
schedule|after|c:sb3_matrix_r3_reserve_after|call_slot_reserve
lifecycle|Normal,Emergency,Failed,ClosingNormal,ClosingRestricted|c:sb3_matrix_r3_cas_before|attempt_ticket
ingress|CR,LF,CRLF,budget,quit,fatal|c:sb3_matrix_r3_cas_before|attempt_ticket
ingress|original_cell_unscheduled,repeated,EOF,partial_Ctrl_D,master_close,read_failure,mission_stop,stop_all,door_fence,grant_revocation,grant_expiry|open:D2|attempt_ticket
fault|none|c:sb3_matrix_r3_cas_before|attempt_ticket
schedule|before|c:sb3_matrix_r3_cas_before|attempt_ticket
schedule|after|c:sb3_matrix_r3_cas_after|attempt_ticket
lifecycle|Emergency,Failed,ClosingRestricted|open:D2|charge_append,meter_settle,operation_complete
lifecycle|Normal|open:D2|meter_settle,operation_complete
lifecycle|Normal|x:sb3_mother_completed_call_allows_clean_close|charge_append
lifecycle|ClosingNormal|open:D2|charge_append,meter_settle
lifecycle|ClosingNormal|x:sb3_mother_open_model_request_blocks_close|operation_complete
fault|decline,record_unknown,poison,panic|open:D2|charge_append
fault|none|x:sb3_mother_completed_call_allows_clean_close|charge_append
fault|record_failed|x:sb3_durable_charge_precedes_chat|charge_append
schedule|before,after|open:D2|charge_append,meter_settle,cancel_run,door_halt,emergency_restrict,close_restrict,operation_complete,close_obligations,producer_join
fault|none,record_failed,record_unknown,poison,panic|open:D2|meter_settle
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
ingress|CR,LF,CRLF,budget,grant_revocation,grant_expiry,quit,fatal|c:sb3_matrix_r5_observer_before|call_version,call_status,call_tags,call_chat
ingress|original_cell_unscheduled,repeated,EOF,partial_Ctrl_D,master_close,read_failure,mission_stop,stop_all,door_fence|open:D2|call_version,call_status,call_tags,call_chat,attempt_record,attempt_pin_release
fault|none,poison|c:sb3_matrix_r5_observer_before|call_version,call_status,call_tags,call_chat
fault|decline,panic|open:D2|call_version,call_status,call_tags
schedule|before|c:sb3_matrix_r5_observer_before|call_version,call_status,call_tags,call_chat
schedule|after|c:sb3_matrix_r5_cas_after|call_version,call_status,call_tags,call_chat
fault|none,decline,record_failed,record_unknown,poison,panic|open:D2|cancel_run,door_halt
lifecycle|Emergency,Failed,SealedFinalizing,ClosedClean,ClosedUnclean|open:D2|emergency_restrict,close_restrict
lifecycle|ClosingRestricted|x:sb3_shutdown_keeps_first_deadline|emergency_restrict
lifecycle|ClosingRestricted|x:sb3_shutdown_keeps_first_deadline|close_restrict
lifecycle|ClosingNormal|x:sb3_shutdown_keeps_first_deadline|emergency_restrict
lifecycle|ClosingNormal|x:sb3_shutdown_keeps_first_deadline|close_restrict
lifecycle|Bootstrap|open:D2|emergency_restrict
lifecycle|Bootstrap|x:sb3_shutdown_keeps_first_deadline|close_restrict
lifecycle|Normal|c:sb3_held_request_cancels_and_joins|emergency_restrict,close_restrict
ingress|CR,LF,CRLF|c:sb3_held_request_cancels_and_joins|emergency_restrict
ingress|original_cell_unscheduled,repeated,EOF,partial_Ctrl_D,master_close,read_failure,mission_stop,stop_all,door_fence,budget,grant_revocation,grant_expiry,quit,fatal|open:D2|emergency_restrict
fault|none|c:sb3_held_request_cancels_and_joins|emergency_restrict,close_restrict
ingress|CR,LF,CRLF,original_cell_unscheduled,repeated,EOF,partial_Ctrl_D,master_close,read_failure,mission_stop,stop_all,door_fence,budget,grant_revocation,grant_expiry,fatal|open:D2|close_restrict,close_obligations,producer_join,seal
ingress|quit|c:sb3_held_request_cancels_and_joins|close_restrict
lifecycle|Normal,Emergency,Failed,ClosingNormal,ClosingRestricted|c:sb3_matrix_record_before|attempt_record
ingress|CR,LF,CRLF,budget,grant_revocation,grant_expiry,quit,fatal|c:sb3_matrix_record_before|attempt_record
fault|none|c:sb3_matrix_record_before|attempt_record
fault|record_failed|c:sb2_record_debt_blocks_seal_and_reuse|attempt_record
fault|record_unknown,poison|open:D2|attempt_record
fault|panic|c:sb2_attempt_drop_panic_and_poison_retain_ownership|attempt_record
schedule|before|c:sb3_matrix_record_before|attempt_record
schedule|after|c:sb3_matrix_settle_before|attempt_record
lifecycle|Normal,Emergency,Failed,ClosingNormal,ClosingRestricted|c:sb3_matrix_settle_before|attempt_pin_release
ingress|CR,LF,CRLF,budget,grant_revocation,grant_expiry,quit,fatal|c:sb3_matrix_settle_before|attempt_pin_release
fault|none,poison|c:sb3_matrix_settle_before|attempt_pin_release
fault|record_failed,record_unknown,panic|open:D2|attempt_pin_release
schedule|before|c:sb3_matrix_settle_before|attempt_pin_release
fault|record_failed,record_unknown,panic|open:D2|operation_complete
fault|none|x:sb3_mother_completed_call_allows_clean_close|operation_complete
fault|poison|c:sb2_owner_completion_is_bounded_and_poison_fails_closed|operation_complete
lifecycle|Normal|x:sb3_unrecorded_reply_is_unusable|record_first_classify
lifecycle|Emergency,Failed,ClosingNormal,ClosingRestricted|x:sb3_restriction_before_the_record_is_classified|record_first_classify
lifecycle|Normal,Emergency,Failed,ClosingNormal,ClosingRestricted|x:sb3_restriction_at_the_post_record_recheck|post_record_recheck
lifecycle|SealedFinalizing,ClosedClean,ClosedUnclean|open:D2|record_first_classify,post_record_recheck
ingress|CR,LF,CRLF,budget,quit,fatal|x:sb3_restriction_before_the_record_is_classified|record_first_classify
ingress|CR,LF,CRLF,budget,quit,fatal|x:sb3_restriction_at_the_post_record_recheck|post_record_recheck
ingress|original_cell_unscheduled,repeated,EOF,partial_Ctrl_D,master_close,read_failure,mission_stop,stop_all,door_fence,grant_revocation,grant_expiry|open:D2|record_first_classify,post_record_recheck
ingress|CR,LF,CRLF,original_cell_unscheduled,repeated,EOF,partial_Ctrl_D,master_close,read_failure,mission_stop,stop_all,door_fence,budget,grant_revocation,grant_expiry,quit,fatal|open:D2|lane_result_admission
fault|none,record_failed,record_unknown|x:sb3_unrecorded_reply_is_unusable|record_first_classify
fault|poison,panic|open:D2|record_first_classify
fault|none|x:sb3_restriction_at_the_post_record_recheck|post_record_recheck
fault|record_failed,record_unknown,poison,panic|open:D2|post_record_recheck
schedule|before|x:sb3_restriction_before_the_record_is_classified|record_first_classify
schedule|after|x:sb3_restriction_at_the_post_record_recheck|record_first_classify
schedule|before|x:sb3_restriction_at_the_post_record_recheck|post_record_recheck
schedule|after|x:sb3_restriction_after_the_recheck_or_the_append|post_record_recheck,lane_result_admission
lifecycle|Normal|c:sb3_registered_session_executes_one_recorded_call|lane_result_admission
lifecycle|Emergency,Failed,ClosingNormal,ClosingRestricted,SealedFinalizing,ClosedClean,ClosedUnclean|open:D2|lane_result_admission
fault|none|c:sb3_registered_session_executes_one_recorded_call|lane_result_admission
fault|decline,record_failed,record_unknown,poison,panic|open:D2|lane_result_admission
schedule|before|x:sb3_revocation_after_chat_entry_is_entered_only|lane_result_admission
lifecycle|Normal|c:sb3_open_model_request_at_deadline_blocks_seal|close_obligations
lifecycle|Emergency,Failed,ClosingRestricted,ClosedUnclean|open:D2|close_obligations
lifecycle|ClosedClean|x:sb3_mother_post_seal_model_events_are_absent|close_obligations
lifecycle|SealedFinalizing|x:sb3_mother_post_seal_model_events_are_absent|close_obligations
lifecycle|ClosingNormal|x:sb3_mother_open_model_request_blocks_close|close_obligations
ingress|quit|c:sb3_open_model_request_at_deadline_blocks_seal|close_obligations
fault|none|c:sb3_open_model_request_at_deadline_blocks_seal|close_obligations
fault|record_failed,poison|c:sb3_unjoined_or_owed_prevents_clean_close|close_obligations
fault|record_unknown,panic|open:D2|close_obligations
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
lifecycle|Bootstrap,Normal,Emergency,Failed,ClosingNormal,ClosingRestricted,SealedFinalizing,ClosedClean,ClosedUnclean|open:M2e-D|close_report
ingress|CR,LF,CRLF,original_cell_unscheduled,repeated,EOF,partial_Ctrl_D,master_close,read_failure,mission_stop,stop_all,door_fence,budget,grant_revocation,grant_expiry,quit,fatal|open:M2e-D|close_report
fault|none,record_failed,record_unknown,poison,panic|open:M2e-D|close_report
schedule|before,after|open:M2e-D|close_report
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
mission_stop|Coordinator::stop to SessionDoor::cancel_run|session_literal_thirteen_states_against_real_terminal_paths|OPEN:D2
stop_all|Coordinator::stop_all, no retained session-route evidence|none|OPEN:D2
door_fence|SessionPort::fence to SessionDoor::halt|session_literal_thirteen_states_against_real_terminal_paths|OPEN:D2
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
/// SB3D-R3 (c): the D1 oracle file; its `x:` citations are accepted only at
/// these sinks, and only on the cells the CELLS table lists.
const D1_ORACLES: &str = include_str!("../../src/adapter/session_model_bridge_d_tests.rs");
const D1_SINKS: &str = "record_first_classify post_record_recheck lane_result_admission";
/// SB3D-R5: the D2 oracle file, and the sinks where its `x:` is accepted, only
/// on the cells the CELLS table lists.
const D2_ORACLES: &str = include_str!("../../src/adapter/session_model_bridge_d2_tests.rs");
const D2_SINKS: &str =
    "charge_append operation_complete emergency_restrict close_restrict close_obligations";
/// Spec 8.4: every R4/R7/R8/R9 sink's Mother-level D oracle, or its open owner.
const D_ROWS: &str = "\
charge_append|sb3_mother_call_death_restarts_report_only
meter_settle|sb3_mother_unjoined_or_owed_prevents_clean_close
attempt_record|sb3_mother_completed_call_allows_clean_close
attempt_pin_release|OPEN:D2
operation_complete|sb3_mother_completed_call_allows_clean_close
record_first_classify|sb3_unrecorded_reply_is_unusable
post_record_recheck|sb3_restriction_at_the_post_record_recheck
lane_result_admission|sb3_restriction_after_the_recheck_or_the_append
close_obligations|sb3_mother_open_model_request_blocks_close
close_report|OPEN:M2e-D
producer_join|sb3_mother_completed_call_allows_clean_close
seal|sb3_mother_unjoined_or_owed_prevents_clean_close
mother_end|sb3_mother_post_seal_model_events_are_absent
primary_end|sb3_mother_post_seal_model_events_are_absent
delivery|sb3_mother_post_seal_model_events_are_absent
";

/// SB-3E U2a (design 2.5): the expectation-table verifier. The table is
/// authored separately from the manifest; neither is derived from the other,
/// the verifier compares them. It shows consistency, not that a test ran.
const CELLS_RS: &str = include_str!("route_cells/cells.rs");
const EXPECT: &str = include_str!("session/model-bridge-expect.tsv");
const EXPECT_SHA256: &str = "e81203c86168fe4fbb63ff24986decf2171d9d4b1486bbc1d184851ae51e73bf";
const UNIVERSE_SHA256: &str = "a6a5337538a92d273dd2efdcaedcf76843bbf2f3ddc6be1f6f44f3c241985fd2";
const EXPECT_HEADER: &str = "route\tsink\tdim\tvalues\tmech\tsetup\tdriver\tentry\tcheck\texit\texec\tapply\texpect\twitness\tcontrol\tplant\trefs\tstatus";
/// R4: `reader:` is these literal bytes, or one of the three closed paths.
const BYTE_APPLY: [(&str, &str); 7] = [
    ("CR", "0d"),
    ("LF", "0a"),
    ("CRLF", "0d0a"),
    ("EOF", "04"),
    ("partial_Ctrl_D", "partial+04"),
    ("master_close", "master_close"),
    ("read_failure", "read_failure"),
];
/// R5.3: which identity keys a driver's trace entries carry (design 2.3).
const DRIVER_KEYS: &str = "d_register=s,q d_bootstrap=s d_identity=s,run,inv d_bind=s,run,inv \
    d_call=s,q,run,inv,att d_lane=s,run,inv d_cancel=s,run d_halt=s d_stop=s d_quit=s,q \
    d_close=s host=s";
/// Real S2/shim record selectors (design 4.2.1); `none` is not one.
const RECORDS: &str = "model_registered model_call";
/// R6.3: `record=keys` a delta must carry, of those the driver's identity has.
/// Mother's `SessionClosed` is not native `session_closed`; `+0:all` is empty.
const LEDGER_RECORDS: &str = "ModelCallCharged=inv LaneReturned=inv model_call=run,att \
    model_registered=s,q session_closed=s SessionClosed=s all=s";
const LOCKS: &str = "ModelState EvidenceInner RunTable Coordinator Pending Producers Actions";
const AMEND_REASON: &str = "NO_SUCH_EVENT_AT_THIS_BOUNDARY";
/// Composed applicability oracles (6.2). None exists yet: U11 lists them.
const AMEND_ORACLES: &str = "";
const DENIAL_ROW_ORACLE: &str = "sb3e_unresolved_lane_refuses_seal";
/// Design 6.1, literal: `dimension|values|sinks` of the 25 candidates, which
/// stay OPEN as `status=amend:` until a reviewed unit (U11) applies them.
const AMEND_CELLS: &str = "\
fault|decline|identity_view,inference_binding,call_slot_reserve,attempt_ticket,charge_append,call_version,call_status,call_tags,call_chat,door_halt,emergency_restrict,close_restrict,lane_result_admission
fault|record_failed,record_unknown|registration_install,attempt_pin_release,operation_complete,post_record_recheck
fault|poison|record_first_classify,emergency_restrict,close_restrict,post_record_recheck
";
/// Design 6.3, literal: the nine R8 lifecycle cells, OPEN until their oracle.
const DENIAL_CELLS: &str = "lifecycle|SealedFinalizing,ClosedClean,ClosedUnclean|record_first_classify,post_record_recheck,lane_result_admission";
/// Design 4.4, literal: sinks|driver|entry|check|exit.
const SINK_POINTS: &str = "\
registration_reserve|d_register|reg.reserve|m:126-141|model.reserved
registration_record|d_register|model.record.before|m:292-299|m:300
registration_install|d_register|model.install.before|m:304|model.install.after
registration_readiness|d_bootstrap|reg.ready|h:838|h:841
identity_view|d_identity|identity|m:571,575-579|m:580
inference_binding|d_bind|bind.entry|model.bind.checked,CAS:r:2301|bind.done
call_slot_reserve|d_call|attempt.slot|m:371-381|model.attempt.reserved
attempt_ticket|d_call|model.attempt.reserved|model.attempt.granted,model.attempt.checked,CAS:r:2125|r:2127
charge_append|d_call|mother.charge.entry|mc:205-250,mother.charge|mother.charge.ok
meter_settle|d_call|mother.settle|mc:296-300|clear
registration_version,registration_status,registration_tags,registration_show|d_register|model.gate.before|model.gate.checked,CAS:m:238|m:258
call_version,call_status,call_tags,call_chat|d_call|call.gate|model.call.sampled,model.call.observed,CAS:m:774|model.call.admitted
cancel_run|d_cancel|door.cancel_run|h:1382-1390|return
door_halt|d_halt|door.halt|h:1409-1411|return
emergency_restrict|d_stop|ingress.emergency|CAS:h:1630-1633|event
close_restrict|d_quit|ingress.accept|h:1690-1712|request
attempt_record|d_call|model.attempt.record.before|m:470|m:475
attempt_pin_release|d_call|model.settle.before|r:2146-2153|return
operation_complete|d_call|complete|m:478,settle:m:482|m:488
record_first_classify|d_call|mother.classify|ad:549-558|ad:849
post_record_recheck|d_call|mother.recheck|mc:471,mc:481|mc:482
lane_result_admission|d_lane|mother.lane.prelock|mother.admit,co:712|mother.admit.ok,mother.lane.returned
close_obligations|d_close|close.obligations|c:265-300|return
close_report|host|host.close_report|note|string
producer_join|d_close|producer.join|c:48-54|c:54
seal|d_close|close.seal|c:372-429,CAS:c:446-452|c:456
mother_end|d_close|close.mother_end|CAS:c:208,c:462-480|return
primary_end|d_close|close.primary|c:490-505|return
delivery|d_close|close.delivery|c:227-234|return
";

/// The parsed table: its rows, each cell's row, and the literal manifest.
#[derive(Default)]
struct Expect {
    rows: Vec<Vec<String>>,
    cells: BTreeMap<(String, String, String), usize>,
    manifest: BTreeSet<String>,
}
/// The literal `cells!` manifest, strictly: a whole-line comment prologue, one
/// standalone invocation, literal tokens, mandatory semicolons, nothing else.
fn parse_manifest(text: &str) -> Result<BTreeSet<String>, String> {
    let mut lines = text.split_inclusive('\n');
    let open = lines
        .by_ref()
        .find(|l| !l.trim().is_empty() && !l.trim_start().starts_with("//"))
        .ok_or("manifest: no cells! block")?;
    let standalone = open
        .trim()
        .strip_prefix("cells!")
        .is_some_and(|rest| rest.trim_start() == "{");
    if !standalone {
        return Err("manifest: only a whole-line comment prologue may precede cells!".into());
    }
    let rest: String = lines.collect();
    let (body, tail) = rest
        .split_once('}')
        .ok_or("manifest: cells! block is not closed")?;
    if !tail.trim().is_empty() {
        return Err("manifest: text after the cells! block".into());
    }
    let ident = |t: &str, upper: bool| {
        !t.is_empty()
            && t.bytes().all(|b| {
                b.is_ascii_lowercase()
                    || b == b'_'
                    || (upper && (b.is_ascii_digit() || b.is_ascii_uppercase()))
            })
    };
    let mut entries: Vec<&str> = body.split(';').collect();
    if !entries.pop().is_some_and(|e| e.trim().is_empty()) {
        return Err("manifest: an entry has no final semicolon".into());
    }
    let (mut cells, mut slugs) = (BTreeSet::new(), BTreeSet::new());
    for entry in entries {
        let f: Vec<_> = entry.split_whitespace().collect();
        let route_ok = |r: &str| {
            r.strip_prefix('r')
                .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
        };
        let literal = f.len() == 4
            && route_ok(f[0])
            && ident(f[1], false)
            && ident(f[2], false)
            && ident(f[3], true);
        if !literal {
            return Err(format!("manifest: not a literal entry {:?}", entry.trim()));
        }
        if !slugs.insert(f.join("_").to_lowercase()) {
            return Err(format!("manifest: repeated entry {:?}", entry.trim()));
        }
        cells.insert(f.join("/"));
    }
    Ok(cells)
}
fn parse_expect(text: &str, manifest: &str) -> Result<Expect, String> {
    let mut lines = text.lines();
    if lines.next() != Some(EXPECT_HEADER) {
        return Err("expect: missing or altered header".into());
    }
    let mut expect = Expect {
        manifest: parse_manifest(manifest)?,
        ..Expect::default()
    };
    for line in lines {
        let row: Vec<String> = line.split('\t').map(String::from).collect();
        if row.len() != 18 || row.iter().any(String::is_empty) {
            return Err(format!("expect: malformed row: {line}"));
        }
        for value in row[3].split(',') {
            let cell = (row[1].clone(), row[2].clone(), value.to_owned());
            if expect.cells.insert(cell, expect.rows.len()).is_some() {
                let (sink, dim) = (&row[1], &row[2]);
                return Err(format!("expect: duplicate cell {sink}/{dim}/{value}"));
            }
        }
        expect.rows.push(row);
    }
    Ok(expect)
}
fn sha256_hex(text: &str) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(text.as_bytes()))
}
/// A changed table needs a reviewed pin; run after the semantic checks so it
/// cannot mask a semantic failure.
fn verify_expect_hash(text: &str, pin: &str) -> Result<(), String> {
    let actual = sha256_hex(text);
    if actual == pin {
        Ok(())
    } else {
        Err(format!(
            "EXPECT_SHA256 does not match the table: {actual} (changed without review?)"
        ))
    }
}
/// The mechanism a coordinate belongs to (design 2.2, 7).
fn mech_of(sink: &str, dim: &str, value: &str) -> &'static str {
    match (sink, dim, value) {
        ("close_report", ..) => "close_report",
        (_, "lifecycle", _) => "lifecycle",
        (_, "schedule", _) => "schedule",
        (_, "fault", "none") => "fault_none",
        (_, "fault", "decline") => "decline",
        (_, "fault", "poison") => "fault_lock",
        (_, "fault", "panic") => "fault_panic",
        (_, "fault", _) => "fault_record",
        ("mother_end", _, "budget") => "finalize",
        (_, _, "grant_revocation" | "grant_expiry") => "grant",
        _ if BYTE_APPLY.iter().any(|(v, _)| *v == value) => "byte",
        _ => "ingress",
    }
}
/// Design 4.4 for a sink: [driver, entry, check, exit].
fn sink_points(sink: &str) -> Option<[&'static str; 4]> {
    let names = |l: &&str| {
        l.split('|')
            .next()
            .is_some_and(|n| n.split(',').any(|m| m == sink))
    };
    let line = SINK_POINTS.lines().find(names)?;
    let f: Vec<_> = line.split('|').collect();
    Some([f[1], f[2], f[3], f[4]])
}
/// The row's driver and points are exactly the sink's in design 4.4.
fn points_agree(row: &[String]) -> Result<(), String> {
    let [driver, entry, check, exit] =
        sink_points(&row[1]).ok_or_else(|| format!("expect: foreign sink {}", row[1]))?;
    if row[6] != driver {
        return Err(format!("{}: wrong driver {}", row[1], row[6]));
    }
    if (row[7].as_str(), row[8].as_str(), row[9].as_str()) != (entry, check, exit) {
        let (e, c, x) = (&row[7], &row[8], &row[9]);
        return Err(format!("{}: mislabelled point {e}|{c}|{x}", row[1]));
    }
    Ok(())
}
/// The fake peer's requests, in order, for a gate sink's exchange (design 2.3).
fn peer_order(sink: &str) -> [&'static str; 4] {
    let last = if sink.starts_with("registration_") {
        "/api/show"
    } else {
        "/api/chat"
    };
    ["/api/version", "/api/status", "/api/tags", last]
}
/// A gate sink's request kind and ordinal against the fake peer (design 2.3).
fn gate_ordinal(sink: &str) -> Option<(&str, usize)> {
    let (route, kind) = sink.split_once('_')?;
    let ordinal = match kind {
        "version" => 1,
        "status" => 2,
        "tags" => 3,
        "show" | "chat" => 4,
        _ => return None,
    };
    matches!(route, "registration" | "call").then_some((kind, ordinal))
}
/// The exact identity the row's driver must show: symbolic variables, bound at
/// run time, the same in every field; a `g:` execution has its own binding.
fn expected_identity(row: &[String]) -> Result<String, String> {
    let (_, keys) = words(DRIVER_KEYS)
        .into_iter()
        .filter_map(|e| e.split_once('='))
        .find(|(d, _)| *d == row[6])
        .ok_or_else(|| format!("{}: wrong driver {}", row[1], row[6]))?;
    let group = row[10].strip_prefix("g:");
    let mut parts: Vec<String> = keys
        .split(',')
        .map(|k| match group {
            Some(g) => format!("{k}={}.{g}", k.to_uppercase()),
            None => format!("{k}={}", k.to_uppercase()),
        })
        .collect();
    if let Some((kind, ordinal)) = gate_ordinal(&row[1]) {
        parts.push(format!("kind={kind} ord={ordinal}"));
    }
    Ok(parts.join(" "))
}
/// Splits on `,` and drops nothing: a 4.4 column's literal point tokens.
fn tokens(column: &str) -> Vec<&str> {
    column.split(',').collect()
}
/// The parsed fields a `g:` execution is compared on (design 2.3, R5.3).
struct Witness {
    point: String,
    identity: String,
    ordinal: String,
    ledger: Vec<Delta>,
}
/// One canonical ledger delta: (operation, record, sorted identity tokens).
type Delta = (String, String, Vec<String>);
/// One `point[identity]` observation, or None for `absent`.
fn observation<'a>(
    text: &'a str,
    identity: &str,
    point_ok: impl Fn(&str) -> bool,
) -> Result<Option<&'a str>, String> {
    if text == "absent" {
        return Ok(None);
    }
    let (point, id) = text
        .strip_suffix(']')
        .and_then(|t| t.split_once('['))
        .ok_or("malformed")?;
    if id != identity {
        return Err("uncorrelated identity".into());
    }
    if !point_ok(point) {
        return Err("wrong point".into());
    }
    Ok(Some(point))
}
/// R6.3: the canonical parsed ledger, or None. Each delta names exactly its
/// record's identity keys from the driver's own identity.
fn parse_ledger(ledger: &str, identity: &str) -> Option<Vec<Delta>> {
    let mut deltas: Vec<Delta> = Vec::new();
    for delta in ledger.split(',') {
        let (op, rest) = delta.split_once(':')?;
        let (record, id) = rest.strip_suffix(']')?.split_once('[')?;
        let (_, keys) = words(LEDGER_RECORDS)
            .into_iter()
            .filter_map(|e| e.split_once('='))
            .find(|(r, _)| *r == record)?;
        let wanted = |t: &&str| {
            t.split_once('=')
                .is_some_and(|(k, _)| keys.split(',').any(|w| w == k))
        };
        let mut want: Vec<String> = identity
            .split(' ')
            .filter(wanted)
            .map(String::from)
            .collect();
        let mut got: Vec<String> = id.split(' ').map(String::from).collect();
        want.sort();
        got.sort();
        let op_ok = matches!(op, "+0" | "+1" | "owed" | "uncertain");
        let repeated = deltas.iter().any(|d| d.1 == record && d.2 == got);
        if !op_ok || want.is_empty() || got != want || repeated || (record == "all" && op != "+0") {
            return None;
        }
        deltas.push((op.to_owned(), record.to_owned(), got));
    }
    if deltas.len() > 1 && deltas.iter().any(|d| d.1 == "all") {
        return None;
    }
    deltas.sort();
    Some(deltas)
}
/// R5.3: the witness is exact. Every mandatory field, exact values, no prefix.
fn check_witness(row: &[String], at: &str) -> Result<Witness, String> {
    let bad = |what: String| format!("{at}: bad witness: {what}");
    let keys =
        "entry check value out branch peer ledger window next store deadline_fired grant locks";
    let mut seen = BTreeMap::new();
    for part in row[13].split(';') {
        let (key, value) = part.split_once('=').unwrap_or((part, ""));
        if !words(keys).contains(&key) || value.is_empty() || seen.insert(key, value).is_some() {
            return Err(format!("{at}: bad witness field {part:?}"));
        }
    }
    for key in [
        "entry",
        "check",
        "value",
        "out",
        "branch",
        "ledger",
        "window",
        "deadline_fired",
    ] {
        if !seen.contains_key(key) {
            return Err(bad(format!("missing {key}")));
        }
    }
    let (value, apply, expect, status) = (&row[3], &row[11], row[12].as_str(), &row[17]);
    let identity = expected_identity(row)?;
    let entry = observation(seen["entry"], &identity, |p| p == row[7])
        .map_err(|e| bad(format!("entry {e}")))?;
    let check = observation(seen["check"], &identity, |p| tokens(&row[8]).contains(&p))
        .map_err(|e| bad(format!("check {e}")))?;
    let unreached = status.starts_with("amend:") || status.starts_with("denial:");
    if entry.is_none() && !unreached {
        return Err(bad("entry observation is absent".into()));
    }
    if entry.is_none() && check.is_some() {
        return Err(bad("check without an entry".into()));
    }
    let absent_check_allowed = value == "panic" || (unreached && expect == "not_entered");
    if (check.is_none() || expect == "not_entered") && !absent_check_allowed {
        return Err(bad(
            "check observation is absent or improperly waived".into()
        ));
    }
    let (applied, ack, won) = {
        let mut parts = seen["value"].split(' ');
        match (parts.next(), parts.next(), parts.next(), parts.next()) {
            (Some(a), Some(b), Some(c), None) => (
                a.split_once('@')
                    .ok_or_else(|| bad("value has no point".into()))?,
                b.strip_prefix("ack=")
                    .ok_or_else(|| bad("value has no ack".into()))?,
                c.strip_prefix("won=")
                    .ok_or_else(|| bad("value has no won".into()))?,
            ),
            _ => return Err(bad("malformed value".into())),
        }
    };
    if applied.0 != value {
        return Err(bad(format!("value {} is not the cell's", applied.0)));
    }
    let ack_want = if value == "none" {
        "none"
    } else if apply.starts_with("paused@") {
        "held"
    } else if apply.starts_with("reader:") {
        "read"
    } else {
        "armed"
    };
    if ack != ack_want {
        return Err(bad(format!("ack {ack}, wanted {ack_want}")));
    }
    let point_known = [&row[7], &row[8], &row[9]]
        .iter()
        .any(|c| tokens(c).contains(&applied.1));
    let named = apply.split_once('@').map(|(_, p)| p);
    // Design 4.2.1: S2 is armed at the record point; the shim holds at the commit.
    let record_point = match apply.strip_prefix("s2:refuse:") {
        Some("model_registered") => Some("model.record.before"),
        Some(_) => Some("model.attempt.record.before"),
        None => apply.starts_with("shim:").then_some("store.commit"),
    };
    let point_ok = match record_point {
        Some(p) => applied.1 == p,
        None => point_known && named.is_none_or(|p| p == applied.1),
    };
    if !point_ok {
        return Err(bad(format!("value point {}", applied.1)));
    }
    let won_want = if matches!(expect, "passed" | "recorded") {
        "route"
    } else {
        "value"
    };
    if won != won_want {
        return Err(bad(format!("won {won}, wanted {won_want}")));
    }
    let at_point = check.or(entry).unwrap_or("absent");
    let refs = tokens(&row[16]);
    // An owed or uncertain end is observed at its record, which the row cites.
    let out_ok = match expect {
        "owed" | "uncertain" => seen["out"]
            .strip_prefix(&format!("{expect}@"))
            .is_some_and(|loc| refs.contains(&loc)),
        _ => seen["out"] == format!("{expect}@{at_point}"),
    };
    if !out_ok {
        return Err(bad(format!(
            "out {} is not {expect}@{at_point}",
            seen["out"]
        )));
    }
    let refusing = expect
        .strip_prefix("refused:")
        .or((expect == "gate_refused").then_some(expect));
    let branch_ok = match refusing {
        Some(p) => seen["branch"]
            .strip_prefix(&format!("note:{p}@"))
            .is_some_and(|loc| refs.contains(&loc)),
        None => seen["branch"] == "none",
    };
    if !branch_ok {
        return Err(bad(format!("branch {}", seen["branch"])));
    }
    let ledger = parse_ledger(seen["ledger"], &identity)
        .ok_or_else(|| bad(format!("ledger {}", seen["ledger"])))?;
    if !matches!(seen["window"], "boundary" | "completed_attempt") {
        return Err(bad(format!("window {}", seen["window"])));
    }
    let ordinal = gate_ordinal(&row[1]).map(|(_, n)| n);
    match (ordinal, seen.get("peer")) {
        (None, None) => {}
        (Some(n), Some(peer)) => {
            let (paths, chat) = peer
                .split_once(" chat=")
                .ok_or_else(|| bad("peer".into()))?;
            let order = peer_order(&row[1]);
            let sent: Vec<_> = if paths == "none" {
                Vec::new()
            } else {
                paths.split(',').collect()
            };
            let count = sent.len();
            let expected_chat = if sent.contains(&"/api/chat") {
                "1"
            } else {
                "0"
            };
            // Over a completed attempt a passing gate sees the whole exchange.
            let count_ok = if seen["window"] == "completed_attempt" && won == "route" {
                count == order.len()
            } else {
                count == n || count + 1 == n
            };
            if !count_ok || sent.as_slice() != &order[..count] || chat != expected_chat {
                return Err(bad(format!("peer {peer}")));
            }
        }
        _ => return Err(bad("peer is exactly for gate sinks".into())),
    }
    let grant_want = match value.as_str() {
        "grant_revocation" => Some("original[revoked]"),
        "grant_expiry" => Some("original[expired]"),
        _ => None,
    };
    if grant_want != seen.get("grant").copied() {
        return Err(bad(
            "grant does not identify the original grant's event".into()
        ));
    }
    // Design 4.5: each `+` entry is its own isolated execution poisoning only that lock.
    match (value == "poison" && status == "open", seen.get("locks")) {
        (false, None) => {}
        (true, Some(locks))
            if locks.split('+').all(|l| {
                l.split_once('@')
                    .is_some_and(|(lock, loc)| words(LOCKS).contains(&lock) && refs.contains(&loc))
            }) => {}
        _ => return Err(bad("locks".into())),
    }
    if expect == "retained" {
        let next = seen.get("next").copied().unwrap_or("");
        let named = |prefix: &str| {
            next.split(',')
                .filter(|t| t.strip_prefix(prefix).is_some_and(|x| !x.is_empty()))
                .count()
                == 1
        };
        if !named("retained:") || !named("prohibited:") {
            return Err(bad("retained names no obligation and prohibition".into()));
        }
    }
    if seen
        .get("store")
        .is_some_and(|s| !matches!(*s, "rolled_back" | "latched"))
    {
        return Err(bad(format!("store {}", seen["store"])));
    }
    let deadline_ok = match value.as_str() {
        "budget" => seen["deadline_fired"] == "budget",
        "grant_expiry" => matches!(seen["deadline_fired"], "none" | "grant_expiry"),
        _ => seen["deadline_fired"] == "none",
    };
    if !deadline_ok {
        return Err(bad(format!("deadline_fired {}", seen["deadline_fired"])));
    }
    Ok(Witness {
        point: entry.unwrap_or("absent").to_owned(),
        identity,
        ordinal: ordinal.map_or("-".to_owned(), |n| n.to_string()),
        ledger,
    })
}
fn apply_ok(row: &[String]) -> bool {
    let (value, apply) = (row[3].as_str(), row[11].as_str());
    if let Some((_, bytes)) = BYTE_APPLY.iter().find(|(v, _)| *v == value) {
        return apply == format!("reader:{bytes}");
    }
    let selector = |prefix: &str| {
        apply
            .strip_prefix(prefix)
            .is_some_and(|r| words(RECORDS).contains(&r))
    };
    match value {
        "record_failed" => selector("s2:refuse:"),
        "record_unknown" => selector("shim:wal_fsync:"),
        _ => apply
            .strip_prefix("paused@")
            .or_else(|| apply.strip_prefix("before@"))
            .is_some_and(|point| {
                [row[7].as_str(), row[8].as_str(), row[9].as_str()]
                    .iter()
                    .any(|c| tokens(c).contains(&point))
            }),
    }
}
/// One row's closed vocabulary, once its driver and points agree with 4.4.
/// Returns the parsed witness, or None for a `hold` row, which claims none.
fn check_expect_row(row: &[String], dim_values: &[&str]) -> Result<Option<Witness>, String> {
    let (sink, dim) = (&row[1], row[2].as_str());
    let at = format!("{sink}/{dim}/{}", row[3]);
    points_agree(row)?;
    let value = row[3].as_str();
    if !dim_values.contains(&value) {
        return Err(format!("{at}: unknown value {value}"));
    }
    if row[4] != mech_of(sink, dim, value) {
        return Err(format!("{at}: wrong mech {} for {value}", row[4]));
    }
    if row[14] != format!("ctl_{sink}") {
        return Err(format!("{at}: bad control {}", row[14]));
    }
    let plant = row[15].strip_prefix("shared:").unwrap_or(&row[15]);
    if !plant.starts_with("pl_") || row[16].contains(' ') {
        return Err(format!("{at}: bad plant or refs"));
    }
    let slug_ok = |s: &str| {
        !s.is_empty()
            && s.bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
    };
    let status_ok = match row[17].split_once(':') {
        None => row[17] == "open",
        Some(("amend", reason)) => reason == AMEND_REASON,
        Some(("denial", oracle)) => oracle == DENIAL_ROW_ORACLE,
        Some(("hold", slug)) => slug_ok(slug),
        _ => false,
    };
    if !status_ok {
        return Err(format!("{at}: bad status {}", row[17]));
    }
    // R5.2: a hold row states no recipe, so none can be mistaken for proof.
    if row[17].starts_with("hold:") {
        let claims = [&row[5], &row[11], &row[12], &row[13]];
        if claims.iter().any(|c| c.as_str() != "hold") || row[10] != "own" {
            return Err(format!("{at}: a hold row carries no recipe"));
        }
        return Ok(None);
    }
    let lifecycle = dim == "lifecycle";
    let setup_ok = match row[5].strip_prefix("state:") {
        Some(state) => words(LIFECYCLE).contains(&state) && (!lifecycle || value == state),
        None => !lifecycle && matches!(row[5].as_str(), "canonical" | "awaiting"),
    };
    if !setup_ok {
        return Err(format!("{at}: bad setup {}", row[5]));
    }
    let exec_ok = row[10] == "own"
        || row[10].strip_prefix("g:").is_some_and(|g| {
            !g.is_empty()
                && matches!(row[12].as_str(), "passed" | "recorded")
                && matches!((dim, value), ("lifecycle", "Normal") | ("fault", "none"))
                && g.bytes().all(|b| b.is_ascii_lowercase() || b == b'_')
        });
    if !exec_ok {
        return Err(format!("{at}: bad exec {}", row[10]));
    }
    if !apply_ok(row) {
        return Err(format!("{at}: bad apply {}", row[11]));
    }
    let expect = row[12].as_str();
    let predicate = expect
        .strip_prefix("refused:")
        .or(expect.strip_prefix("closed:"));
    let expect_ok = predicate.map_or(
        matches!(
            expect,
            "passed"
                | "recorded"
                | "gate_refused"
                | "owed"
                | "uncertain"
                | "retained"
                | "not_entered"
        ),
        |p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_'),
    );
    if !expect_ok {
        return Err(format!("{at}: bad expect {expect}"));
    }
    check_witness(row, &at).map(Some)
}
/// Expands one `dimension|values|sinks` literal.
fn listed_cells(table: &str) -> BTreeSet<(String, String, String)> {
    let mut cells = BTreeSet::new();
    for line in table.lines() {
        let f: Vec<_> = line.split('|').collect();
        for sink in f[2].split(',') {
            for value in f[1].split(',') {
                cells.insert((sink.to_owned(), f[0].to_owned(), value.to_owned()));
            }
        }
    }
    cells
}
/// Design 2.5: the universe literal, and the table expands to exactly it.
fn check_expect_table(c: &Context) -> Result<(), String> {
    let universe: BTreeSet<_> = c
        .cells
        .keys()
        .filter(|k| c.in_universe(k))
        .cloned()
        .collect();
    let mut lines: Vec<String> = universe
        .iter()
        .map(|(s, d, v)| format!("{}/{s}/{d}/{v}", c.frozen[s.as_str()][0]))
        .collect();
    lines.sort();
    let actual = sha256_hex(&(lines.join("\n") + "\n"));
    if actual != UNIVERSE_SHA256 {
        return Err(format!("universe: {actual} is not the frozen universe"));
    }
    if let Some((s, d, v)) = universe.iter().find(|k| !c.expect.cells.contains_key(*k)) {
        return Err(format!("expect: missing cell {s}/{d}/{v}"));
    }
    if let Some((s, d, v)) = c.expect.cells.keys().find(|k| !universe.contains(*k)) {
        return Err(format!("expect: extra cell {s}/{d}/{v}"));
    }
    let (amend, denial) = (listed_cells(AMEND_CELLS), listed_cells(DENIAL_CELLS));
    type Key = (String, String, String, Vec<Delta>);
    let mut groups: BTreeMap<&str, BTreeSet<Key>> = BTreeMap::new();
    for row in &c.expect.rows {
        let frozen = c.frozen.get(row[1].as_str());
        if frozen.is_none_or(|s| s[0] != row[0]) {
            return Err(format!("expect: wrong route {} for {}", row[0], row[1]));
        }
        let values = DIMENSIONS
            .iter()
            .find(|(d, _)| *d == row[2])
            .ok_or_else(|| format!("expect: unknown dimension {}", row[2]))?;
        for value in row[3].split(',') {
            let cell = (row[1].clone(), row[2].clone(), value.to_owned());
            let want = match (amend.contains(&cell), denial.contains(&cell)) {
                (true, _) => format!("amend:{AMEND_REASON}"),
                (_, true) => format!("denial:{DENIAL_ROW_ORACLE}"),
                // A hold names no design list: it may sit only on an open cell.
                _ if row[17].starts_with("hold:")
                    && c.cells.get(&cell).is_some_and(|d| d.starts_with("open:")) =>
                {
                    row[17].clone()
                }
                _ => "open".to_owned(),
            };
            if row[17] != want {
                return Err(format!(
                    "{}/{}/{value}: status is not {want}",
                    row[1], row[2]
                ));
            }
        }
        let witness = check_expect_row(row, &words(values.1))?;
        if let (Some(group), Some(w)) = (row[10].strip_prefix("g:"), witness) {
            let key = (w.point, w.identity, w.ordinal, w.ledger);
            if !groups.entry(group).or_default().insert(key) {
                return Err(format!(
                    "{}: exec group {group} repeats a point, identity, ordinal and ledger",
                    row[1]
                ));
            }
        }
    }
    // Neither list is derived from the other: every manifest entry needs an
    // open row of the same route.
    for entry in &c.expect.manifest {
        let f: Vec<_> = entry.split('/').collect();
        let cell = (f[1].to_owned(), f[2].to_owned(), f[3].to_owned());
        let row = c.expect.cells.get(&cell).map(|i| &c.expect.rows[*i]);
        if !row.is_some_and(|r| r[0].to_lowercase() == f[0] && r[17] == "open") {
            return Err(format!("manifest: {entry} has no open row"));
        }
    }
    Ok(())
}
/// `x:rc_<slug>` (design 2.5): the row exists and is open, the slug is the
/// computed one, the literal manifest lists the cell, and 4.4 holds.
fn check_rc(
    c: &Context,
    (sink, dimension, value): (&str, &str, &str),
    slug: &str,
) -> Result<String, String> {
    let at = format!("{sink}/{dimension}/{value}");
    let row = c
        .expect_row(sink, dimension, value)
        .ok_or(format!("{at}: x:rc_ has no expectation row"))?;
    if row[17] != "open" {
        // R5.2: `hold` is not open, and neither are `amend:` or `denial:`.
        return Err(format!("{at}: x:rc_ on a non-open row ({})", row[17]));
    }
    let route = row[0].to_lowercase();
    let computed = format!("rc_{route}_{sink}_{dimension}_{}", value.to_lowercase());
    if slug != computed {
        return Err(format!(
            "{at}: x:{slug} is not the computed slug {computed}"
        ));
    }
    if !c
        .expect
        .manifest
        .contains(&format!("{route}/{sink}/{dimension}/{value}"))
    {
        return Err(format!("{at}: x:rc_ has no cell!() in the manifest"));
    }
    points_agree(row).map_err(|e| format!("x:rc_ row fails design 4.4: {e}"))?;
    Ok("executed".to_owned())
}
/// A reviewed `na:` on a universe cell: only from its row's `amend:` or
/// `denial:` status, with the oracle that row's list names. Anything else is
/// `None` and falls to the audited path, which refuses it.
fn reviewed_universe_na(
    c: &Context,
    (sink, dimension, value): (&str, &str, &str),
    rest: &str,
) -> Option<String> {
    let row = c.expect_row(sink, dimension, value)?;
    let (reason, oracle) = rest.strip_suffix(')')?.split_once('(')?;
    let reviewed = match row[17].split_once(':')? {
        ("amend", wanted) => {
            !oracle.is_empty()
                && reason == wanted
                && words(AMEND_ORACLES).contains(&oracle)
                && c.names_test(oracle)
        }
        ("denial", wanted) => {
            reason == "LIFECYCLE_DENIES_BEFORE_THIS_SINK"
                && oracle == wanted
                && words(DENIAL_ORACLES).contains(&oracle)
                && c.names_test(oracle)
        }
        _ => false,
    };
    reviewed.then(|| format!("na:{reason}"))
}

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
    s.split_whitespace().collect()
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
    d1_oracles: String,
    receipts: BTreeSet<&'static str>,
    named_tests: RefCell<BTreeMap<String, bool>>,
    expect: Expect,
}
impl Context {
    /// The frozen 905 (design 2.1): audited `c:` or `open:` in the CELLS literal.
    fn in_universe(&self, cell: &(String, String, String)) -> bool {
        let audited = self.cells.get(cell);
        audited.is_some_and(|d| d.starts_with("c:") || d.starts_with("open:"))
    }
    fn expect_row(&self, sink: &str, dim: &str, value: &str) -> Option<&Vec<String>> {
        let cell = (sink.to_owned(), dim.to_owned(), value.to_owned());
        self.expect.cells.get(&cell).map(|i| &self.expect.rows[*i])
    }
    /// `is_test` once per name: the block walk re-reads the sources.
    fn names_test(&self, name: &str) -> bool {
        let mut seen = self.named_tests.borrow_mut();
        let entry = seen.entry(name.to_owned());
        *entry.or_insert_with(|| is_test(name, &self.sources))
    }
}
fn context(sources: String, d1_oracles: String, expect: Expect) -> Context {
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
        d1_oracles,
        receipts,
        named_tests: RefCell::default(),
        expect,
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
    // SB-3E (design 2.5): a universe cell is `c:`, `open:`, `x:rc_` or a reviewed
    // `na:`. Any other `x:` or `na:` on one still fails `listed` below.
    if kind == "x" && rest.starts_with("rc_") {
        return check_rc(c, (sink, dimension, value), rest);
    }
    if kind == "na" {
        // R5.2: only an amend or denial row is reviewed; a hold row falls to `listed`.
        if let Some(key) = reviewed_universe_na(c, (sink, dimension, value), rest) {
            return Ok(key);
        }
    }
    match kind {
        "c" | "x" => {
            if c.receipts.contains(rest) {
                return Err(format!("{at}: historical receipt {rest} is not a test"));
            }
            let d1_oracle = words(D1_SINKS).contains(&sink) && is_test(rest, &c.d1_oracles);
            let d2_oracle = words(D2_SINKS).contains(&sink) && is_test(rest, D2_ORACLES);
            if kind == "x" && !d1_oracle && !d2_oracle && !is_test(rest, C2_ORACLES) {
                return Err(format!(
                    "{at}: x: only a C2-new oracle, or a D1 oracle at a D1 sink, or a D2 oracle at a D2 sink, may claim execution"
                ));
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
    verify_with(tsv, sources, D1_ORACLES.to_owned())
}
/// `verify_in` against a D1 oracle file other than the checked-in one.
fn verify_with(tsv: &str, sources: String, d1: String) -> Result<Summary, String> {
    let tables = Tables {
        expect: EXPECT,
        manifest: CELLS_RS,
        sha256: EXPECT_SHA256,
    };
    verify_all(tsv, sources, d1, &tables)
}
/// The whole check against an expectation table, a manifest and a pin. The
/// register's cells are checked first, so `x:rc_` is judged on its row as it
/// stands, then the table, then the pin: a pin cannot mask a semantic failure.
fn verify_all(tsv: &str, sources: String, d1: String, tables: &Tables) -> Result<Summary, String> {
    if sources.contains("#![cfg") {
        return Err("a file-level cfg in the evidence sources".into());
    }
    let c = context(sources, d1, parse_expect(tables.expect, tables.manifest)?);
    let summary = verify_register(tsv, &c)?;
    check_expect_table(&c)?;
    verify_expect_hash(tables.expect, tables.sha256)?;
    Ok(summary)
}
/// What `verify_all` checks a register against.
struct Tables<'a> {
    expect: &'a str,
    manifest: &'a str,
    sha256: &'a str,
}
fn verify_register(tsv: &str, c: &Context) -> Result<Summary, String> {
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
        check_row(c, &f, &mut summary)?;
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

/// Spec 8.4 composition: the disjoint `SB-3D` block has one row per R4/R7/R8/R9
/// sink, on that sink's frozen boundary, naming exactly its listed D oracle.
pub fn verify_d_block(cases: &str) -> Result<usize, String> {
    let frozen = frozen_sinks();
    let receipts = include_str!("session/combination-receipts.tsv");
    let is_receipt = |id: &str| receipts.lines().any(|l| l.split('\t').next() == Some(id));
    let listed: BTreeMap<_, _> = D_ROWS.lines().filter_map(|l| l.split_once('|')).collect();
    for (sink, row) in &frozen {
        if ["R4", "R7", "R8", "R9"].contains(&row[0]) != listed.contains_key(sink) {
            return Err(format!("{sink}: the D table and the routes disagree"));
        }
    }
    let mut seen = BTreeSet::new();
    for line in cases.lines() {
        let row: Vec<&str> = line.split('\t').collect();
        if !row[0].starts_with("SB3D/") && !row.contains(&"SB-3D") {
            continue;
        }
        if row.len() != 5 || row[4] != "SB-3D" {
            return Err(format!("malformed SB-3D row: {line}"));
        }
        let sink = row[0].strip_prefix("SB3D/").unwrap_or(row[0]);
        let want = listed
            .get(sink)
            .ok_or_else(|| format!("unknown SB-3D row {sink}"))?;
        if !seen.insert(sink) {
            return Err(format!("duplicate SB-3D row {sink}"));
        }
        let test = row[1];
        if test.starts_with("N/A") {
            return Err(format!("{sink}: N/A escape in the SB-3D block"));
        }
        if is_receipt(test) {
            return Err(format!(
                "{sink}: historical receipt {test} is not a D oracle"
            ));
        }
        if row[2] != frozen[sink][2] {
            return Err(format!("{sink}: not this sink's boundary"));
        }
        if test != *want {
            return Err(format!("{sink}: unrelated oracle {test}"));
        }
        let defined = is_test(test, D1_ORACLES) || is_test(test, D2_ORACLES);
        if !test.starts_with("OPEN:") && !defined {
            return Err(format!("{sink}: no such D oracle {test}"));
        }
    }
    if seen.len() != listed.len() {
        return Err(format!("missing SB-3D rows: {}", listed.len() - seen.len()));
    }
    Ok(seen.len())
}

/// Both supplement checks, on the checked-in files (`session_oracles.rs`).
pub fn verify_supplement() -> Result<Summary, String> {
    let cases = read(&session_file("model-bridge-cases.tsv"))?;
    verify_cases(&cases)?;
    verify_d_block(&cases)?;
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
    verify(&tampered(sink, f))
}
/// The register text with `f(columns)` applied to sink's row.
fn tampered(sink: &str, f: impl FnOnce(&mut Vec<String>)) -> String {
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
    out.join("\n") + "\n"
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
close_report|5|open:M2e-D|c:sb3_completed_call_allows_existing_clean_close|not the audited
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
    verify(&cell_text(sink, dimension, value, to))
}
/// The register text with one cell set, whichever clause holds it now.
fn cell_text(sink: &str, dimension: &str, value: &str, to: &str) -> String {
    let at = DIMENSIONS.iter().position(|d| d.0 == dimension).unwrap();
    tampered(sink, |c: &mut Vec<String>| {
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
/// SB3D-R3 (c): `sink|dimension|value|disposition|wanted error`. Each is a
/// D1 claim the audit refused (sb3-d1-oracles.md) or an `x:` outside its cell.
const D1_CONTROLS: &str = "\
record_first_classify|fault|poison|x:sb3_restriction_before_the_record_is_classified|not the audited
post_record_recheck|fault|record_failed|x:sb3_unrecorded_reply_is_unusable|not the audited
post_record_recheck|lifecycle|SealedFinalizing|x:sb3_restriction_at_the_post_record_recheck|not the audited
record_first_classify|ingress|mission_stop|x:sb3_restriction_before_the_record_is_classified|not the audited
record_first_classify|ingress|door_fence|x:sb3_restriction_before_the_record_is_classified|not the audited
lane_result_admission|lifecycle|Emergency|x:sb3_restriction_after_the_recheck_or_the_append|not the audited
lane_result_admission|ingress|CR|x:sb3_restriction_after_the_recheck_or_the_append|not the audited
lane_result_admission|fault|record_failed|x:sb3_unrecorded_reply_is_unusable|not the audited
record_first_classify|lifecycle|Emergency|x:sb3_late_content_never_projects|not the audited
record_first_classify|schedule|after|x:sb3_restriction_before_the_record_is_classified|not the audited
post_record_recheck|schedule|after|x:sb3_restriction_at_the_post_record_recheck|not the audited
post_record_recheck|ingress|budget|x:sb3_restriction_before_the_record_is_classified|not the audited
call_chat|lifecycle|Normal|x:sb3_restriction_at_the_post_record_recheck|or a D1 oracle at a D1 sink
post_record_recheck|schedule|after|x:sb3_no_such_d1_oracle|or a D1 oracle at a D1 sink
record_first_classify|lifecycle|Normal|open:D1|OPEN needs an owner
post_record_recheck|lifecycle|Normal|open:D1|OPEN needs an owner
lane_result_admission|schedule|after|open:D1|OPEN needs an owner
";
/// SB3D-R3 (c): D1's `x:` cells are bound cell by cell, and only to a test
/// the D1 file really defines.
#[test]
fn sb3d_r3_d1_citation_controls_are_rejected() {
    let mut red: Vec<String> = Vec::new();
    let mut check = |label: &str, accepted: Result<(), String>, want: &str| match accepted {
        Ok(()) => red.push(format!("{label}: tamper accepted")),
        Err(e) if e.contains(want) => {}
        Err(e) => red.push(format!("{label}: wanted {want:?}, got {e}")),
    };
    for spec in D1_CONTROLS.lines() {
        let f: Vec<_> = spec.split('|').collect();
        check(spec, set_cell(f[0], f[1], f[2], f[3]).map(|_| ()), f[4]);
    }
    let want = "or a D1 oracle at a D1 sink";
    let signature = "#[test]\nfn sb3_restriction_at_the_post_record_recheck() {";
    assert!(D1_ORACLES.contains(signature), "the D1 oracle file");
    let forms = [
        (
            "#[ignore] on a D1 oracle",
            format!("#[ignore]\n{signature}"),
        ),
        (
            "D1 oracle not a test",
            signature.strip_prefix("#[test]\n").unwrap().to_owned(),
        ),
    ];
    for (label, form) in forms {
        let d1 = D1_ORACLES.replacen(signature, &form, 1);
        let run = verify_with(&real(), evidence_sources(), d1).map(|_| ());
        check(label, run, want);
    }
    let session = real().replace(
        "x:sb3_restriction_at_the_post_record_recheck",
        "c:sb3_restriction_at_the_post_record_recheck",
    );
    check(
        "a D1 oracle is not a retained `c:`",
        verify(&session).map(|_| ()),
        "no such retained test",
    );
    assert!(
        red.is_empty(),
        "{} controls RED:\n{}",
        red.len(),
        red.join("\n")
    );
}
/// MP-SB-R32-D (x4), and D2's `x:` bound cell by cell.
#[test]
fn sb3d_r32_composition_controls_are_rejected() {
    let text = read(&session_file("model-bridge-cases.tsv")).unwrap();
    assert_eq!(verify_d_block(&text).unwrap(), 15);
    let receipts = include_str!("session/combination-receipts.tsv");
    let receipt = receipts.lines().nth(1).unwrap().split('\t').next().unwrap();
    let row = "SB3D/close_obligations\tsb3_mother_open_model_request_blocks_close";
    assert!(text.contains(row));
    let swap = |to: &str| text.replacen(row, &format!("SB3D/close_obligations\t{to}"), 1);
    let kept: Vec<_> = text
        .lines()
        .filter(|l| !l.starts_with("SB3D/seal\t"))
        .collect();
    for (cases, want) in [
        (kept.join("\n"), "missing SB-3D rows"),
        (
            swap("sb3_mother_completed_call_allows_clean_close"),
            "unrelated oracle",
        ),
        (swap("N/A:reachable-limited"), "N/A escape"),
        (swap(receipt), "historical receipt"),
    ] {
        let err = verify_d_block(&cases).expect_err(want);
        assert!(err.contains(want), "{want}: got {err}");
    }
    let oracle = "x:sb3_mother_completed_call_allows_clean_close";
    for (sink, dimension, value, to, want) in [
        (
            "charge_append",
            "fault",
            "poison",
            oracle,
            "not the audited",
        ),
        (
            "close_obligations",
            "lifecycle",
            "Emergency",
            oracle,
            "not the audited",
        ),
        (
            "call_chat",
            "lifecycle",
            "Normal",
            oracle,
            "or a D2 oracle at a D2 sink",
        ),
    ] {
        let err = set_cell(sink, dimension, value, to).expect_err(want);
        assert!(err.contains(want), "{sink}: {want}: got {err}");
    }
}
/// SB3D-R3 (c), SB3D-R5: the audited disposition counts after D2.
#[test]
fn sb3d_r3_d1_cells_are_executed_or_owned() {
    let summary = verify_supplement().unwrap();
    let count = |key: &str| summary.by_disposition.get(key).copied().unwrap_or(0);
    assert_eq!(
        (
            count("executed"),
            count("open:D1"),
            count("open:D2"),
            count("open:F1"),
            count("open:M2e-D")
        ),
        (45, 0, 645, 1, 33),
        "after D2: {summary:?}"
    );
    assert_eq!(count("cited"), 226, "no retained citation moved");
}

/// SB-3E U2a: the manifest file is literal and, until a unit credits a cell, empty.
#[test]
fn sb3e_manifest_is_literal_and_empty() {
    assert_eq!(parse_manifest(CELLS_RS), Ok(BTreeSet::new()));
}
#[test]
fn sb3e_no_amend_oracle_is_listed_yet() {
    assert!(words("").is_empty() && words(AMEND_ORACLES).is_empty());
}
/// One synthetic row, valid by construction, for the fixture table below.
fn fixture_row(route: &str, sink: &str, dim: &str, value: &str, status: &str) -> String {
    let [driver, entry, check, exit] = sink_points(sink).unwrap();
    let first = tokens(check)[0];
    let open = status == "open";
    let apply = match (BYTE_APPLY.iter().find(|(v, _)| *v == value), value) {
        (Some((_, b)), _) => format!("reader:{b}"),
        (_, "record_failed") => "s2:refuse:model_call".to_owned(),
        (_, "record_unknown") => "shim:wal_fsync:model_call".to_owned(),
        _ => format!("before@{entry}"),
    };
    let setup = match dim {
        "lifecycle" => format!("state:{value}"),
        _ => "canonical".to_owned(),
    };
    let expect = if open { "passed" } else { "not_entered" };
    let mut row = vec![String::new(); 18];
    let plain = [route, sink, dim, value, mech_of(sink, dim, value)];
    for (i, text) in plain.into_iter().enumerate() {
        row[i] = text.to_owned();
    }
    row[5] = setup;
    row[6] = driver.to_owned();
    (row[7], row[8], row[9]) = (entry.to_owned(), check.to_owned(), exit.to_owned());
    (row[10], row[11], row[12]) = ("own".to_owned(), apply.clone(), expect.to_owned());
    let id = expected_identity(&row).unwrap();
    let session = id.split(' ').next().unwrap();
    let ack = match (value, apply.starts_with("reader:")) {
        ("none", _) => "none",
        (_, true) => "read",
        _ => "armed",
    };
    let (seen, reached, out, won) = if open {
        (
            format!("{entry}[{id}]"),
            format!("{first}[{id}]"),
            first,
            "route",
        )
    } else {
        ("absent".to_owned(), "absent".to_owned(), "absent", "value")
    };
    let deadline = if value == "budget" { "budget" } else { "none" };
    let applied_at = match apply.as_str() {
        "s2:refuse:model_call" => "model.attempt.record.before",
        "shim:wal_fsync:model_call" => "store.commit",
        _ => entry,
    };
    let mut w = format!(
        "entry={seen};check={reached};value={value}@{applied_at} ack={ack} won={won};\
         out={expect}@{out};branch=none;ledger=+0:all[{session}];window=boundary;\
         deadline_fired={deadline}"
    );
    if let Some((_, n)) = gate_ordinal(sink) {
        let paths = &peer_order(sink)[..n];
        let chat = usize::from(paths.contains(&"/api/chat"));
        w += &format!(";peer={} chat={chat}", paths.join(","));
    }
    match value {
        "grant_revocation" => w += ";grant=original[revoked]",
        "grant_expiry" => w += ";grant=original[expired]",
        "poison" if open => w += ";locks=ModelState@fixture",
        _ => {}
    }
    row[13] = w;
    row[14] = format!("ctl_{sink}");
    row[15] = format!("pl_fixture_{sink}_{dim}");
    row[16] = "fixture".to_owned();
    row[17] = status.to_owned();
    row.join("\t")
}
/// A synthetic table for every universe cell: real register and universe, no
/// recorded expectation. It proves the verifier's rules, never a route.
fn fixture_table() -> String {
    let c = context(evidence_sources(), D1_ORACLES.to_owned(), Expect::default());
    let (amend, denial) = (listed_cells(AMEND_CELLS), listed_cells(DENIAL_CELLS));
    let mut out = format!("{EXPECT_HEADER}\n");
    for key in c.cells.keys().filter(|k| c.in_universe(k)) {
        let status = match (amend.contains(key), denial.contains(key)) {
            (true, _) => format!("amend:{AMEND_REASON}"),
            (_, true) => format!("denial:{DENIAL_ROW_ORACLE}"),
            _ => "open".to_owned(),
        };
        let route = &c.frozen[key.0.as_str()][0];
        out += &fixture_row(route, &key.0, &key.1, &key.2, &status);
        out.push('\n');
    }
    out
}
/// `table` with `f(columns)` applied to the row holding this `sink/dim/value`.
fn fixture_edit(table: &str, cell: &str, f: impl FnOnce(&mut Vec<String>)) -> String {
    let mut f = Some(f);
    let mut out = String::new();
    for line in table.lines() {
        let mut c: Vec<String> = line.split('\t').map(String::from).collect();
        if format!("{}/{}/{}", c[1], c[2], c[3]) == cell {
            f.take().expect("one row")(&mut c);
        }
        out += &c.join("\t");
        out.push('\n');
    }
    assert!(f.is_none(), "{cell}: no row");
    out
}
/// `column|from|to|wanted` on call_chat/lifecycle/Normal: `from` empty sets the
/// column, otherwise the first `from` is replaced.
const NORMAL_EDITS: &str = "\
7||model.call.observed|mislabelled point
8||m:738|mislabelled point
9||m:786|mislabelled point
4||fault_lock|wrong mech
5||state:Failed|bad setup
10||g:Bad-Name|bad exec
11||paused@m:1|bad apply
11||reader:Line::Eof|bad apply
12||fine|bad expect
13||entry=call.gate|bad witness: missing check
14||ctl_seal|bad control
15||mutant_1|bad plant
17||closed|status is not open
13|entry=call.gate[s=S |entry=call.gate[s=OTHER |entry uncorrelated identity
13|entry=call.gate[|entry=model.call.sampled[|entry wrong point
13|;check=model.call.sampled[s=S |;check=model.call.sampled[s=OTHER |check uncorrelated identity
13|;check=model.call.sampled[|;check=m:738[|check wrong point
13|entry=call.gate[s=S q=Q run=RUN inv=INV att=ATT kind=chat ord=4]|entry=absent|entry observation is absent
13|value=Normal@|value=Emergency@|is not the cell's
13|ack=armed|ack=held|ack held, wanted armed
13|won=route|won=value|won value, wanted route
13|Normal@call.gate|Normal@m:999|value point
13|out=passed@model.call.sampled|out=passed@call.gate|bad witness: out
13|out=passed@model.call.sampled|out=passed@model.call.sampledX|bad witness: out
13|branch=none|branch=note:x@fixture|bad witness: branch
13|ledger=+0:all[s=S]|ledger=none|bad witness: ledger
13|ledger=+0:all[s=S]|ledger=+1:model_call|bad witness: ledger
13|ledger=+0:all[s=S]|ledger=+1:model_call[run=OTHER att=ATT]|bad witness: ledger
13|ledger=+0:all[s=S]|ledger=+1:nope[run=RUN att=ATT]|bad witness: ledger
13|ledger=+0:all[s=S]|ledger=+9:model_call[run=RUN att=ATT]|bad witness: ledger
13|ledger=+0:all[s=S]|ledger=+1:model_call[run=RUN]|bad witness: ledger
13|ledger=+0:all[s=S]|ledger=+1:model_call[s=S]|bad witness: ledger
13|ledger=+0:all[s=S]|ledger=+1:ModelCallCharged[s=S]|bad witness: ledger
13|ledger=+0:all[s=S]|ledger=+1:model_call[run=RUN run=RUN att=ATT]|bad witness: ledger
13|ledger=+0:all[s=S]|ledger=+1:model_call[run=RUN att=ATT],+0:model_call[run=RUN att=ATT]|bad witness: ledger
13|ledger=+0:all[s=S]|ledger=+1:all[s=S]|bad witness: ledger
13|ledger=+0:all[s=S]|ledger=+0:all[run=RUN]|bad witness: ledger
13|ledger=+0:all[s=S]|ledger=+0:all[s=S],+1:model_call[run=RUN att=ATT]|bad witness: ledger
13|;window=boundary|;window=someday|bad witness: window
13|/api/tags,/api/chat chat=1|/api/chat,/api/tags chat=1|bad witness: peer
13|/api/tags,/api/chat chat=1|/api/tags,/api/tags chat=1|bad witness: peer
13|/api/tags,/api/chat chat=1|/api/tags,/api/show chat=1|bad witness: peer
13|chat=1|chat=0|bad witness: peer
13|;peer=/api/version,/api/status,/api/tags,/api/chat chat=1|;peer=/api/version chat=0|bad witness: peer
13|;deadline_fired=none|;deadline_fired=budget|bad witness: deadline_fired
13|;deadline_fired=none|;deadline_fired=grant_expiry|bad witness: deadline_fired
13|;deadline_fired=none|;deadline_fired=none;store=zap|bad witness: store
13|;deadline_fired=none|;deadline_fired=none;grant=original[revoked]|bad witness: grant
13|;deadline_fired=none|;deadline_fired=none;locks=ModelState@fixture|bad witness: locks
";
/// `sink/dim/value|column|from|to|wanted` on other rows.
const CELL_EDITS: &str = "\
emergency_restrict/lifecycle/Emergency|6||d_halt|wrong driver
call_version/lifecycle/Normal|13|peer=/api/version chat=0|peer=/api/version,/api/status,/api/tags,/api/chat chat=1|bad witness: peer
call_chat/ingress/CR|11||reader:0a|bad apply
call_chat/ingress/CR|11||reader:Line::Eof|bad apply
call_chat/ingress/master_close|11||reader:eof|bad apply
call_chat/fault/record_failed|11||s2:refuse:none|bad apply
call_chat/fault/record_failed|13|@model.attempt.record.before|@call.gate|bad witness: value point
call_chat/fault/record_unknown|13|@store.commit|@model.attempt.record.before|bad witness: value point
call_chat/fault/record_failed|11||shim:wal_fsync:model_call|bad apply
call_chat/fault/record_unknown|11||shim:wal_fsync:nope|bad apply
call_chat/ingress/grant_revocation|13|;grant=original[revoked]||bad witness: grant
call_chat/ingress/grant_revocation|13|[revoked]|[expired]|bad witness: grant
call_chat/ingress/grant_expiry|13|[expired]|[revoked]|bad witness: grant
call_chat/fault/poison|13|;locks=ModelState@fixture||bad witness: locks
call_chat/fault/poison|13|locks=ModelState@|locks=Nope@|bad witness: locks
call_chat/fault/poison|13|locks=ModelState@fixture|locks=ModelState@elsewhere|bad witness: locks
close_report/fault/panic|13|;deadline_fired=none|;deadline_fired=none;peer=none chat=0|bad witness: peer
call_chat/fault/decline|17||open|status is not
seal/lifecycle/Normal|17||denial:sb3e_unresolved_lane_refuses_seal|status is not
call_chat/fault/decline|17||hold:no_recipe|status is not
call_chat/ingress/mission_stop|17||hold:Bad-Slug|bad status
";
/// `manifest|wanted`, `~` a newline: refused through the verifier, for that reason.
const BAD_MANIFESTS: &str = "\
// cells! { r5 call_chat lifecycle Normal; }~|no cells! block
#[cfg(any())] cells! {~ r5 call_chat lifecycle Normal;~}~|comment prologue
use x::y;~cells! {~}~|comment prologue
cells! { r5 call_chat lifecycle Normal; }~|comment prologue
cells! {~    r5 call_chat lifecycle Normal~}~|no final semicolon
cells! {~    ;~}~|not a literal entry
cells! {~    // r5 call_chat lifecycle Normal;~}~|not a literal entry
cells! {~    r5 call_chat lifecycle \"Normal\";~}~|not a literal entry
cells! {~    include!(\"more.rs\");~}~|not a literal entry
cells! {~    r5 call_chat lifecycle $value;~}~|not a literal entry
cells! {~}~fn extra() {}~|text after
cells! {~}~cells! {~}~|text after
cells! {~    r5 call_chat lifecycle Normal;~    r5 call_chat lifecycle Normal;~}~|repeated entry
cells! {~    r5 call_chat lifecycle Normal extra;~}~|not a literal entry
cells! {~|not closed
cells! {~    r5 call_chat fault nope;~}~|has no open row
cells! {~    r5 call_chat fault decline;~}~|has no open row
cells! {~    r6 call_chat fault panic;~}~|has no open row
";
/// `sink|dim|value|disposition|manifest entry or -|wanted`: register tampers.
const RC_CONTROLS: &str = "\
call_chat|lifecycle|Normal|x:rc_r5_call_chat_lifecycle_normal|-|no cell!() in the manifest
call_chat|lifecycle|Normal|x:rc_r5_call_chat_lifecycle_wrong|r5 call_chat lifecycle Normal|computed slug
call_chat|fault|decline|x:rc_r5_call_chat_fault_decline|r5 call_chat fault decline|x:rc_ on a non-open row
record_first_classify|lifecycle|ClosedClean|x:rc_r8_record_first_classify_lifecycle_closedclean|r8 record_first_classify lifecycle ClosedClean|x:rc_ on a non-open row
emergency_restrict|lifecycle|Emergency|x:sb3_shutdown_keeps_first_deadline|-|not the audited
seal|lifecycle|Normal|na:BOOTSTRAP_NO_MISSION|-|not the audited
call_chat|fault|decline|na:BOOTSTRAP_NO_MISSION|-|not the audited
call_chat|fault|decline|na:NO_SUCH_EVENT_AT_THIS_BOUNDARY(no_such_oracle)|-|not on the closed list
call_chat|fault|decline|na:NO_SUCH_EVENT_AT_THIS_BOUNDARY()|-|not on the closed list
record_first_classify|lifecycle|ClosedClean|na:LIFECYCLE_DENIES_BEFORE_THIS_SINK(sb2_unjoined_or_owed_prevents_clean_close)|-|not the audited
";
/// Rejected through `verify_all` on a synthetic table, each for its own reason.
#[test]
fn sb3e_expect_verifier_controls_are_rejected() {
    let base = fixture_table();
    let pin = sha256_hex(&base);
    let tsv = real();
    let run = |register: &str, expect: &str, manifest: &str, sha256: &str| {
        let tables = Tables {
            expect,
            manifest,
            sha256,
        };
        verify_all(register, evidence_sources(), D1_ORACLES.to_owned(), &tables).map(|_| ())
    };
    // Each tamper is pinned, so a missing rule shows as accepted, not as a hash.
    let table = |text: &str| run(&tsv, text, CELLS_RS, &sha256_hex(text));
    assert_eq!(table(&base), Ok(()), "the synthetic table is accepted");
    let mut red: Vec<String> = Vec::new();
    let mut check = |label: &str, accepted: Result<(), String>, want: &str| match accepted {
        Ok(()) => red.push(format!("{label}: tamper accepted")),
        Err(e) if e.contains(want) => {}
        Err(e) => red.push(format!("{label}: wanted {want:?}, got {e}")),
    };
    let accepts = |label: &str, text: &str| {
        assert_eq!(
            run(&tsv, text, CELLS_RS, &sha256_hex(text)),
            Ok(()),
            "{label}"
        );
    };
    const NORMAL: &str = "call_chat/lifecycle/Normal";
    let apply_spec = |cell: &str, col: usize, from: &str, to: &str| {
        fixture_edit(&base, cell, |c: &mut Vec<String>| {
            c[col] = if from.is_empty() {
                to.to_owned()
            } else {
                assert!(c[col].contains(from), "{cell}: no {from}");
                c[col].replacen(from, to, 1)
            };
        })
    };
    for spec in NORMAL_EDITS.lines() {
        let f: Vec<_> = spec.split('|').collect();
        check(
            spec,
            table(&apply_spec(NORMAL, f[0].parse().unwrap(), f[1], f[2])),
            f[3],
        );
    }
    for spec in CELL_EDITS.lines() {
        let f: Vec<_> = spec.split('|').collect();
        check(
            spec,
            table(&apply_spec(f[0], f[1].parse().unwrap(), f[2], f[3])),
            f[4],
        );
    }
    for key in [
        "entry",
        "check",
        "value",
        "out",
        "branch",
        "ledger",
        "window",
        "deadline_fired",
    ] {
        let edited = fixture_edit(&base, NORMAL, |c: &mut Vec<String>| {
            let kept: Vec<_> = c[13]
                .split(';')
                .filter(|p| !p.starts_with(&format!("{key}=")))
                .collect();
            c[13] = kept.join(";");
        });
        check(key, table(&edited), &format!("bad witness: missing {key}"));
    }
    // Universe and expansion.
    let row = base
        .lines()
        .find(|l| l.contains("call_chat\tfault\tpanic\t"))
        .unwrap();
    check(
        "row removed",
        table(&base.replacen(&format!("{row}\n"), "", 1)),
        "expect: missing cell",
    );
    let extra = row.replacen("call_chat\tfault\tpanic", "call_chat\tschedule\tmiddle", 1);
    check(
        "row added",
        table(&format!("{base}{extra}\n")),
        "expect: extra cell",
    );
    check(
        "row repeated",
        table(&format!("{base}{row}\n")),
        "expect: duplicate cell",
    );
    let wrong = fixture_edit(&base, "call_chat/fault/panic", |c: &mut Vec<String>| {
        c[3] = "poison".into()
    });
    check("expands wrong", table(&wrong), "expect: duplicate cell");
    let poison = base
        .lines()
        .find(|l| l.contains("call_chat\tfault\tpoison\t"))
        .unwrap();
    let merged = fixture_edit(
        &base.replacen(&format!("{poison}\n"), "", 1),
        "call_chat/fault/panic",
        |c: &mut Vec<String>| c[3] = "panic,poison".into(),
    );
    check("two values, one row", table(&merged), "unknown value");
    // The pin is part of the verifier, after the semantic checks.
    let witness = fixture_edit(&base, NORMAL, |c: &mut Vec<String>| {
        c[13] = c[13].replace("model.call.sampled", "model.call.observed");
    });
    check(
        "witness without hash",
        run(&tsv, &witness, CELLS_RS, &pin),
        "EXPECT_SHA256",
    );
    accepts("the same change with its pin", &witness);
    // A refused row: the exact branch, in the row's own references.
    let refused = |branch: &'static str| {
        fixture_edit(&base, NORMAL, move |c: &mut Vec<String>| {
            c[12] = "refused:8".into();
            c[13] = c[13]
                .replace("out=passed@", "out=refused:8@")
                .replace("won=route", "won=value")
                .replace("branch=none", branch);
        })
    };
    accepts(
        "a refused row with its branch",
        &refused("branch=note:8@fixture"),
    );
    check(
        "refusal without a branch",
        table(&refused("branch=none")),
        "bad witness: branch",
    );
    check(
        "another predicate",
        table(&refused("branch=note:7@fixture")),
        "bad witness: branch",
    );
    check(
        "a branch not in refs",
        table(&refused("branch=note:8@m:1")),
        "bad witness: branch",
    );
    // An owed end is observed at the record the row cites.
    let owed = |at: &'static str| {
        fixture_edit(&base, NORMAL, move |c: &mut Vec<String>| {
            c[12] = "owed".into();
            c[13] = c[13]
                .replace("out=passed@model.call.sampled", at)
                .replace("won=route", "won=value");
        })
    };
    accepts("owed at its record", &owed("out=owed@fixture"));
    check(
        "owed elsewhere",
        table(&owed("out=owed@call.gate")),
        "bad witness: out",
    );
    // `retained` names what stays owed and what stays prohibited.
    let retained = |next: &'static str| {
        fixture_edit(&base, NORMAL, move |c: &mut Vec<String>| {
            c[12] = "retained".into();
            c[13] = c[13]
                .replace("out=passed@", "out=retained@")
                .replace("won=route", "won=value")
                + next;
        })
    };
    accepts(
        "retained, named",
        &retained(";next=retained:pin,prohibited:seal"),
    );
    check("retained, unnamed", table(&retained("")), "retained names");
    check(
        "retained, half named",
        table(&retained(";next=retained:pin")),
        "retained names",
    );
    // A `g:` execution: its own binding, compared on the parsed key.
    let group = |text: &str, cell: &str, ledger: &'static str| {
        fixture_edit(text, cell, move |c: &mut Vec<String>| {
            let plain = expected_identity(c).unwrap();
            c[10] = "g:x".into();
            let bound = expected_identity(c).unwrap();
            c[13] = c[13]
                .replace(&plain, &bound)
                .replace("ledger=+0:all[s=S]", ledger);
        })
    };
    let pair = |first: &'static str, second: &'static str| {
        group(&group(&base, NORMAL, first), "call_chat/fault/none", second)
    };
    const EMPTY: &str = "ledger=+0:all[s=S.x]";
    check(
        "one execution, same key",
        table(&pair(EMPTY, EMPTY)),
        "repeats a point",
    );
    accepts(
        "one execution, distinct ledger",
        &pair(EMPTY, "ledger=+1:model_call[run=RUN.x att=ATT.x]"),
    );
    const TWO: &str = "ledger=+0:ModelCallCharged[inv=INV.x],+1:model_call[run=RUN.x att=ATT.x]";
    for (label, second) in [
        (
            "reordered deltas",
            "ledger=+1:model_call[run=RUN.x att=ATT.x],+0:ModelCallCharged[inv=INV.x]",
        ),
        (
            "reordered identity fields",
            "ledger=+0:ModelCallCharged[inv=INV.x],+1:model_call[att=ATT.x run=RUN.x]",
        ),
    ] {
        check(label, table(&pair(TWO, second)), "repeats a point");
    }
    check(
        "a value-applied row in a group",
        table(&group(&base, "call_chat/ingress/CR", EMPTY)),
        "bad exec",
    );
    for ledger in ["+1:SessionClosed[s=S]", "+1:session_closed[s=S]"] {
        accepts(ledger, &apply_spec(NORMAL, 13, "+0:all[s=S]", ledger));
    }
    let unbound = fixture_edit(&base, NORMAL, |c: &mut Vec<String>| c[10] = "g:x".into());
    check(
        "group with no binding",
        table(&unbound),
        "uncorrelated identity",
    );
    let refused_group = group(&refused("branch=note:8@fixture"), NORMAL, EMPTY);
    check(
        "a refused row in a group",
        table(&refused_group),
        "bad exec",
    );
    // Manifests, through the same entry point.
    for spec in BAD_MANIFESTS.lines() {
        let (manifest, want) = spec.split_once('|').unwrap();
        let manifest = manifest.replace('~', "\n");
        check(spec, run(&tsv, &base, &manifest, &pin), want);
    }
    // `x:rc_` needs its row, slug, manifest entry and 4.4; `na:` a reviewed row.
    let credited = cell_text(
        "call_chat",
        "lifecycle",
        "Normal",
        "x:rc_r5_call_chat_lifecycle_normal",
    );
    let listed = "cells! {\n    r5 call_chat lifecycle Normal;\n}\n";
    assert_eq!(
        run(&credited, &base, listed, &pin),
        Ok(()),
        "an honest x:rc_ is accepted"
    );
    const CHECKED: &str =
        ";check=model.call.sampled[s=S q=Q run=RUN inv=INV att=ATT kind=chat ord=4]";
    let waive = |expect: &'static str| {
        fixture_edit(&base, NORMAL, move |c: &mut Vec<String>| {
            let won = if expect == "passed" { "route" } else { "value" };
            c[12] = expect.into();
            c[13] = c[13]
                .replace(CHECKED, ";check=absent")
                .replace(
                    "out=passed@model.call.sampled",
                    &format!("out={expect}@call.gate"),
                )
                .replace("won=route", &format!("won={won}"));
        })
    };
    check(
        "check absent, out moved",
        table(&waive("passed")),
        "check observation",
    );
    let not_entered = waive("not_entered");
    check(
        "not_entered, ordinary cell",
        table(&not_entered),
        "check observation",
    );
    let checked = fixture_edit(&base, NORMAL, |c: &mut Vec<String>| {
        c[12] = "not_entered".into();
        c[13] = c[13]
            .replace("out=passed@", "out=not_entered@")
            .replace("won=route", "won=value");
    });
    check("not_entered, checked", table(&checked), "improperly waived");
    check(
        "not_entered, credited",
        run(&credited, &not_entered, listed, &sha256_hex(&not_entered)),
        "check observation",
    );
    for (col, to) in [(8, "model.call.sampled"), (6, "d_register")] {
        let moved = apply_spec(NORMAL, col, "", to);
        check(
            "x:rc_ fails 4.4",
            run(&credited, &moved, listed, &sha256_hex(&moved)),
            "x:rc_ row fails design 4.4",
        );
    }
    for spec in RC_CONTROLS.lines() {
        let f: Vec<_> = spec.split('|').collect();
        let register = cell_text(f[0], f[1], f[2], f[3]);
        let entry = format!("cells! {{\n    {};\n}}\n", f[4]);
        let manifest = if f[4] == "-" { CELLS_RS } else { &entry };
        check(spec, run(&register, &base, manifest, &pin), f[5]);
    }
    // R5.2: a hold row states no recipe and is never credited or excused.
    const STOP: &str = "call_chat/ingress/mission_stop";
    let hold = |text: &str, cell: &str, slug: &'static str| {
        fixture_edit(text, cell, move |c: &mut Vec<String>| {
            for col in [5, 11, 12, 13] {
                c[col] = "hold".into();
            }
            c[17] = format!("hold:{slug}");
        })
    };
    let held = hold(&base, STOP, "no_recipe");
    accepts("a hold row", &held);
    check(
        "a hold row on an audited citation",
        table(&hold(&base, NORMAL, "no_recipe")),
        "status is not open",
    );
    let half = fixture_edit(&held, STOP, |c: &mut Vec<String>| {
        c[11] = "before@call.gate".into()
    });
    check(
        "a hold row with a recipe",
        table(&half),
        "carries no recipe",
    );
    let grouped = fixture_edit(&held, STOP, |c: &mut Vec<String>| c[10] = "g:x".into());
    check(
        "a hold row in a group",
        table(&grouped),
        "carries no recipe",
    );
    let on_stop = "x:rc_r5_call_chat_ingress_mission_stop";
    let listed = "cells! {\n    r5 call_chat ingress mission_stop;\n}\n";
    let credited = cell_text("call_chat", "ingress", "mission_stop", on_stop);
    check(
        "x:rc_ on hold",
        run(&credited, &held, listed, &sha256_hex(&held)),
        "x:rc_ on a non-open row (hold:",
    );
    let excused = cell_text(
        "call_chat",
        "ingress",
        "mission_stop",
        "na:BOOTSTRAP_NO_MISSION",
    );
    check(
        "na: on hold",
        run(&excused, &held, CELLS_RS, &sha256_hex(&held)),
        "not the audited disposition",
    );
    check(
        "a manifest entry on hold",
        run(&tsv, &held, listed, &sha256_hex(&held)),
        "has no open row",
    );
    let completed = |peer: &'static str| {
        fixture_edit(
            &base,
            "call_version/lifecycle/Normal",
            move |c: &mut Vec<String>| {
                c[13] = c[13]
                    .replace("window=boundary", "window=completed_attempt")
                    .replace("peer=/api/version chat=0", peer);
            },
        )
    };
    check(
        "completed attempt, partial exchange",
        table(&completed("peer=/api/version chat=0")),
        "bad witness: peer",
    );
    assert!(
        red.is_empty(),
        "{} controls RED:\n{}",
        red.len(),
        red.join("\n")
    );
    accepts(
        "completed attempt, whole exchange",
        &completed("peer=/api/version,/api/status,/api/tags,/api/chat chat=1"),
    );
}

/// SB-3E U2b (design 2.1, 2.2, R5): the real table expands to the frozen universe.
#[test]
fn sb3e_expect_table_is_complete() {
    verify(&real()).unwrap();
    let expect = parse_expect(EXPECT, CELLS_RS).unwrap();
    assert!(expect.manifest.is_empty(), "no cell is credited");
    let mut by_status: BTreeMap<&str, usize> = BTreeMap::new();
    for row in &expect.rows {
        *by_status
            .entry(row[17].split(':').next().unwrap())
            .or_default() += 1;
    }
    assert_eq!(expect.rows.len(), 905);
    assert_eq!(
        (
            by_status["open"],
            by_status["amend"],
            by_status["denial"],
            by_status["hold"]
        ),
        (780, 25, 9, 91)
    );
}
/// Tampers with the checked-in table, through the verifier on the real files.
#[test]
fn sb3e_real_table_tampers_are_rejected() {
    let run = |expect: &str, manifest: &str, register: &str| {
        let tables = Tables {
            expect,
            manifest,
            sha256: EXPECT_SHA256,
        };
        verify_all(register, evidence_sources(), D1_ORACLES.to_owned(), &tables).map(|_| ())
    };
    let tsv = real();
    let table = |text: &str| run(text, CELLS_RS, &tsv);
    let edit = |cell: &str, col: usize, to: &'static str| {
        fixture_edit(EXPECT, cell, move |c: &mut Vec<String>| c[col] = to.into())
    };
    let panic = "call_chat/fault/panic";
    let row = EXPECT
        .lines()
        .find(|l| l.contains("call_chat\tfault\tpanic\t"))
        .unwrap();
    let mut red: Vec<String> = Vec::new();
    let mut check = |label: &str, got: Result<(), String>, want: &str| match got {
        Ok(()) => red.push(format!("{label}: tamper accepted")),
        Err(e) if e.contains(want) => {}
        Err(e) => red.push(format!("{label}: wanted {want:?}, got {e}")),
    };
    check(
        "row removed",
        table(&EXPECT.replacen(&format!("{row}\n"), "", 1)),
        "expect: missing cell",
    );
    let extra = row.replacen("call_chat\tfault\tpanic", "call_chat\tschedule\tmiddle", 1);
    check(
        "row added",
        table(&format!("{EXPECT}{extra}\n")),
        "expect: extra cell",
    );
    check(
        "wrong driver",
        table(&edit(panic, 6, "d_register")),
        "wrong driver",
    );
    check(
        "mislabelled point",
        table(&edit(panic, 8, "m:738")),
        "mislabelled point",
    );
    let held = edit("call_chat/ingress/mission_stop", 17, "hold:x");
    check(
        "a hold row keeping a recipe",
        table(&held),
        "carries no recipe",
    );
    check(
        "expands wrong",
        table(&edit(panic, 3, "poison")),
        "expect: duplicate cell",
    );
    let witness = fixture_edit(
        EXPECT,
        "call_chat/lifecycle/Normal",
        |c: &mut Vec<String>| {
            c[13] = c[13].replace("model.call.sampled", "model.call.observed");
        },
    );
    check(
        "witness changed without hash",
        table(&witness),
        "EXPECT_SHA256",
    );
    let on_amend = cell_text(
        "call_chat",
        "fault",
        "decline",
        "x:rc_r5_call_chat_fault_decline",
    );
    check(
        "x: on amend",
        run(
            EXPECT,
            "cells! {\n    r5 call_chat fault decline;\n}\n",
            &on_amend,
        ),
        "x:rc_ on a non-open row",
    );
    let cell = "call_chat/lifecycle/Normal";
    let listed = "cells! {\n    r5 call_chat lifecycle Normal;\n}\n";
    let credited = cell_text(
        "call_chat",
        "lifecycle",
        "Normal",
        "x:rc_r5_call_chat_lifecycle_normal",
    );
    check(
        "x:rc_ with a mislabelled point",
        run(&edit(cell, 8, "model.call.sampled"), listed, &credited),
        "x:rc_ row fails design 4.4",
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

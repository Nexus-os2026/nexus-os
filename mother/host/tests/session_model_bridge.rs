//! SB-3C R32 supplement (spec 8.2-8.4; SB3C-R6): the 35 route-level sinks of
//! the session model bridge, verified from the TSV alone. No production
//! classifier is called: rows are joined only against the literal tables
//! below, plus an exact-name check that each cited test exists in the test
//! sources. `c:` cites a retained test as primitive evidence and never claims
//! the new route executed it; `x:` is a C2-new oracle. `session_oracles.rs`
//! requires this and the retained 153,000-key matrix validator.
//!
//! A cell is (sink, dimension, value), the B1e side x value precedent. The
//! full cross product is not authored and not claimed (register 14.5).
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

/// The frozen 35 identities (spec 8.3): route|sinks|oracles|plants. The R5
/// sinks are the four registration then four call gates, in request order.
const ROUTES: &str = "\
R0|registration_reserve registration_record registration_install registration_readiness|R0|MP-SB-2
R1|identity_view|R1|MP-SB-4-Q
R2|inference_binding|R2|MP-SB-3-CAS
R3|call_slot_reserve attempt_ticket|A1 A2|MP-SB-5-count MP-SB-5-slot
R4|charge_append meter_settle|C1 C3|MP-SB-7-charge
R5|registration_version registration_status registration_tags registration_show call_version call_status call_tags call_chat|G1 G3|MP-SB-6-default MP-SB-6-CAS
R6|cancel_run door_halt emergency_restrict close_restrict|G5|MP-SB-6-once
R7|attempt_record attempt_pin_release operation_complete|A2 A3|MP-SB-5-pin MP-SB-5-slot MP-SB-9-record
R8|record_first_classify post_record_recheck lane_result_admission|O1 O2|MP-SB-9-record MP-SB-10
R9|close_obligations close_report producer_join seal mother_end primary_end delivery|H1 H2 H2-held H5|MP-SB-11 MP-SB-9-meter
";
const NA_REASONS: &str = "BOOTSTRAP_NO_MISSION PREBIND_NO_RUN_OR_CONSENT NO_INVOCATION_AT_REGISTRATION METADATA_HAS_NO_CHARGE SETTLEMENT_HAS_NO_NEW_WORK_ADMISSION LIFECYCLE_DENIES_BEFORE_THIS_SINK LEGACY_FINALIZATION_ONLY";
/// The earlier denial oracles a lifecycle N/A may name (spec 8.4).
const DENIAL_ORACLES: &str = "sb3c_registration_is_bootstrap_only_once sb2_unrecorded_failed_or_late_registration_is_absent sb2_unjoined_or_owed_prevents_clean_close sb3_post_seal_model_events_are_absent";
const OWNERS: &str = "D1 D2 F1";
/// The tests each route may cite (route or R5reg/R5call|tests): an unrelated
/// test fails.
const EVIDENCE: &str = "\
R0|sb2_bootstrap_cannot_overtake_registration,sb2_install_completion_is_atomic_to_close_observers,sb2_one_session_operation_active,sb2_owner_drop_never_discharges_debt,sb2_registration_records_before_install_once,sb2_unjoined_or_owed_prevents_clean_close,sb2_unrecorded_failed_or_late_registration_is_absent,sb3c_registration_is_bootstrap_only_once
R1|sb3c_identity_is_original_request_read_only
R2|sb3_original_file_grant_remains_required,sb3_stop_before_binding_installs_nothing,sb3c_public_binding_is_exact_and_idempotent,sb3c_stop_before_public_binding_installs_nothing
R3|sb2_attempt_drop_panic_and_poison_retain_ownership,sb2_unjoined_or_owed_prevents_clean_close,sb3_attempt_ceiling_is_eight_on_the_original_run,sb3_concurrent_operation_refuses_before_ticket,sb3_matrix_r3_cas_after,sb3_matrix_r3_cas_before,sb3_matrix_r3_reserve_after,sb3_matrix_r3_reserve_before
R4|sb2_unjoined_or_owed_prevents_clean_close,sb3_caps_and_budget_keep_existing_goldens,sb3_durable_charge_precedes_chat,sb3_matrix_charge_after,sb3_matrix_charge_before,sb3_matrix_settle_before,sb3_unconfigured_session_retains_default_denial,sb3_unsettled_recorded_call_is_uncertain
R5reg|pre_gate_ingress_request_count_eq_zero,sb2_held_registration_request_cancels_and_joins,sb2_paused_before_cas_ingress_wins_dispatches_nothing,sb2_unjoined_or_owed_prevents_clean_close,sb2_unrecorded_failed_or_late_registration_is_absent,sb3c_registration_is_bootstrap_only_once,session_override_required_at_each_request
R5call|sb2_unjoined_or_owed_prevents_clean_close,sb3_call_override_required_at_each_request,sb3_matrix_r5_cas_after,sb3_matrix_r5_observer_before
R6|sb3_held_request_cancels_and_joins,sb3_post_seal_model_events_are_absent,sb3_pre_gate_ingress_sends_no_request
R7|sb2_attempt_drop_panic_and_poison_retain_ownership,sb2_missing_outcome_is_unknown_not_success,sb2_owner_completion_is_bounded_and_poison_fails_closed,sb2_owner_completion_retries_benign_observer_contention,sb2_record_debt_blocks_seal_and_reuse,sb2_unjoined_or_owed_prevents_clean_close,sb3_attempt_record_and_pin_are_exact,sb3_matrix_record_before,sb3_matrix_settle_before
R8|sb2_unjoined_or_owed_prevents_clean_close,sb3_matrix_record_before,sb3_matrix_reply_after,sb3_matrix_settle_before,sb3_registered_session_executes_one_recorded_call,sb3_revocation_after_chat_entry_is_entered_only
R9|sb3_completed_call_allows_existing_clean_close,sb3_open_model_request_at_deadline_blocks_seal,sb3_post_seal_model_events_are_absent,sb3_unjoined_or_owed_prevents_clean_close
";
/// SB3C-R6 (d): the only OPEN cells, dimension|values|owner|sinks. OPEN is
/// validator-bound to exactly this list (R18 precedent).
const OPEN_CELLS: &str = "\
lifecycle|Emergency,Failed|D2|registration_reserve,registration_record
ingress|CR,LF,CRLF,original_cell_unscheduled,repeated,EOF,partial_Ctrl_D,master_close,read_failure,stop_all,fatal|D2|registration_reserve,registration_record
fault|record_failed,record_unknown,poison|D2|registration_reserve,producer_join
schedule|before|D2|registration_reserve
fault|record_unknown,poison,panic|D2|registration_record,registration_install,registration_readiness
lifecycle|Failed|D2|registration_install,registration_readiness,registration_version,registration_status,registration_tags,registration_show
ingress|stop_all,fatal|D2|registration_install,registration_readiness,registration_version,registration_status,registration_tags,registration_show
lifecycle|ClosingNormal,ClosingRestricted,SealedFinalizing,ClosedClean,ClosedUnclean|D2|identity_view
ingress|stop_all,door_fence,quit,fatal|D2|identity_view
fault|decline,record_failed,record_unknown,panic|D2|identity_view,call_slot_reserve,call_chat
lifecycle|Failed,ClosingNormal,ClosingRestricted,SealedFinalizing,ClosedClean,ClosedUnclean|D2|inference_binding
ingress|mission_stop,stop_all,door_fence,budget,quit,fatal|D2|inference_binding
fault|decline,record_failed,record_unknown,poison,panic|D2|inference_binding,cancel_run,door_halt,emergency_restrict,close_restrict
ingress|stop_all|D2|call_slot_reserve,attempt_ticket,charge_append,meter_settle,call_version,call_status,call_tags,call_chat,attempt_record,attempt_pin_release,record_first_classify
fault|decline,record_failed,record_unknown|D2|attempt_ticket
fault|decline,record_unknown|D2|charge_append
fault|record_failed,record_unknown,panic|D2|meter_settle,record_first_classify
schedule|after|D2|meter_settle,attempt_pin_release,close_report
fault|poison,panic|D2|registration_version,registration_status,registration_tags,registration_show
fault|decline,panic|D2|call_version,call_status,call_tags
lifecycle|Emergency,Failed,ClosingNormal,ClosingRestricted|D2|cancel_run,door_halt,emergency_restrict,close_restrict,operation_complete
ingress|CR,LF,CRLF,original_cell_unscheduled,repeated,EOF,partial_Ctrl_D,master_close,read_failure,stop_all,door_fence,budget,grant_revocation,grant_expiry,quit,fatal|D2|cancel_run
ingress|CR,LF,CRLF,original_cell_unscheduled,repeated,EOF,partial_Ctrl_D,master_close,read_failure,mission_stop,stop_all,budget,grant_revocation,grant_expiry,quit,fatal|D2|door_halt
ingress|mission_stop,stop_all,door_fence,budget,grant_revocation,grant_expiry,quit,fatal|D2|emergency_restrict
ingress|CR,LF,CRLF,original_cell_unscheduled,repeated,EOF,partial_Ctrl_D,master_close,read_failure,mission_stop,stop_all,door_fence,budget,grant_revocation,grant_expiry,fatal|D2|close_restrict,close_obligations,close_report,producer_join,seal
fault|record_failed,record_unknown|D2|attempt_pin_release,operation_complete
ingress|CR,LF,CRLF,original_cell_unscheduled,repeated,EOF,partial_Ctrl_D,master_close,read_failure,mission_stop,stop_all,door_fence,budget,grant_revocation,grant_expiry,quit,fatal|D2|operation_complete
ingress|stop_all|D1|post_record_recheck
fault|record_failed,record_unknown,poison,panic|D1|post_record_recheck
schedule|after|D1|post_record_recheck,lane_result_admission
lifecycle|Emergency,Failed,ClosingNormal,ClosingRestricted,SealedFinalizing,ClosedClean,ClosedUnclean|D1|lane_result_admission
ingress|CR,LF,CRLF,original_cell_unscheduled,repeated,EOF,partial_Ctrl_D,master_close,read_failure,mission_stop,stop_all,door_fence,budget,grant_revocation,grant_expiry,quit,fatal|D1|lane_result_admission
fault|decline,record_failed,record_unknown,poison,panic|D1|lane_result_admission
lifecycle|Emergency,Failed,SealedFinalizing,ClosedClean,ClosedUnclean|D2|close_obligations
fault|record_unknown|D2|close_obligations,seal
lifecycle|Normal,Emergency,Failed,SealedFinalizing,ClosedClean,ClosedUnclean|D2|close_report,producer_join
fault|record_failed,record_unknown,poison,panic|D2|close_report
lifecycle|Normal,Emergency,Failed,ClosedUnclean|D2|seal
ingress|budget|F1|mother_end
";
/// SB3C-R6 (a): class|session-level primitive|retained evidence for the
/// reduction|session-level test. No real reduction: `OPEN:<owner>`, `none`.
const INGRESS_MAP: &str = "\
CR|SessionIngress::emergency|review1_real_cr_lf_crlf_stop_during_publish_hold|sb3_matrix_r5_observer_before
LF|SessionIngress::emergency|review1_real_cr_lf_crlf_stop_during_publish_hold|sb3_matrix_r5_observer_before
CRLF|SessionIngress::emergency|review1_real_cr_lf_crlf_stop_during_publish_hold|sb3_matrix_r5_observer_before
original_cell_unscheduled|HaltCell::engage_emergency on the session word|session_setup_each_final_boundary_has_its_own_unscheduled_ingress_control|sb3_matrix_r5_observer_before
repeated|SessionIngress::emergency twice|m2dd_original_ingress_invalidates_queued_work_and_repeated_commands|sb3_matrix_r5_observer_before
EOF|StopRoute::press then SessionIngress::fail|d_o20_stop_ctrl_d_and_master_close_before_publication|sb3_matrix_r5_observer_before
partial_Ctrl_D|StopRoute::press then SessionIngress::fail|d_o20_stop_ctrl_d_and_master_close_before_publication|sb3_matrix_r5_observer_before
master_close|StopRoute::press then SessionIngress::fail|d_o20_stop_ctrl_d_and_master_close_before_publication|sb3_matrix_r5_observer_before
read_failure|StopRoute::press then SessionIngress::fail|d_o20_read_error_during_held_execution_engages_stop_before_exit|sb3_matrix_r5_observer_before
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
    "SB3C/record/foreign_r",
    "SB3C/record/poison_session_port",
    "SB3C/guard/lifecycle_states",
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
fn is_test(name: &str, source: &str) -> bool {
    source.contains(&format!("#[test]\nfn {name}() {{"))
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

/// The frozen sink: (route, gate, oracles, plants).
type Frozen = (&'static str, String, Vec<&'static str>, Vec<&'static str>);
fn frozen_sinks() -> BTreeMap<&'static str, Frozen> {
    let mut out = BTreeMap::new();
    for line in ROUTES.lines() {
        let f: Vec<_> = line.split('|').collect();
        let (mut reg, mut call) = (0, 0);
        for sink in f[1].split(' ') {
            let gate = if f[0] != "R5" {
                "-".to_owned()
            } else if sink.starts_with("registration_") {
                reg += 1;
                format!("reg-{reg}")
            } else {
                call += 1;
                format!("call-{call}")
            };
            out.insert(sink, (f[0], gate, words(f[2]), words(f[3])));
        }
    }
    out
}
struct Context {
    frozen: BTreeMap<&'static str, Frozen>,
    evidence: BTreeMap<&'static str, BTreeSet<&'static str>>,
    open: BTreeMap<(String, String, String), &'static str>,
    sources: String,
    receipts: BTreeSet<&'static str>,
}
fn context() -> Context {
    let evidence = EVIDENCE
        .lines()
        .map(|l| {
            let (route, tests) = l.split_once('|').expect("route|tests");
            (route, tests.split(',').collect())
        })
        .collect();
    let mut open = BTreeMap::new();
    for line in OPEN_CELLS.lines() {
        let f: Vec<_> = line.split('|').collect();
        for (sink, value) in f[3]
            .split(',')
            .flat_map(|s| f[1].split(',').map(move |v| (s, v)))
        {
            let cell = (sink.to_owned(), f[0].to_owned(), value.to_owned());
            assert!(open.insert(cell, f[2]).is_none(), "duplicate OPEN: {line}");
        }
    }
    let receipts = include_str!("session/combination-receipts.tsv")
        .lines()
        .skip(1)
        .filter_map(|l| l.split('\t').next())
        .collect();
    Context {
        frozen: frozen_sinks(),
        evidence,
        open,
        sources: evidence_sources(),
        receipts,
    }
}

/// R5's registration and call gates have separate evidence.
fn evidence_key(route: &str, sink: &str) -> String {
    match route {
        "R5" if sink.starts_with("registration_") => "R5reg".to_owned(),
        "R5" => "R5call".to_owned(),
        _ => route.to_owned(),
    }
}
/// Which N/A reason may close which cell (spec 8.4): anything else is an
/// N/A escape, including a closed reason on the wrong dimension or value.
fn na_allowed(reason: &str, route: &str, sink: &str, dimension: &str, value: &str) -> bool {
    let registration = route == "R0" || sink.starts_with("registration_");
    let mission_cause = matches!(
        value,
        "mission_stop" | "door_fence" | "budget" | "grant_revocation" | "grant_expiry"
    );
    match reason {
        "BOOTSTRAP_NO_MISSION" => {
            value == "Bootstrap"
                || registration && (value == "decline" || dimension == "ingress" && mission_cause)
        }
        "PREBIND_NO_RUN_OR_CONSENT" => {
            matches!(route, "R1" | "R2")
                && (value == "Bootstrap"
                    || dimension == "ingress" && mission_cause && value != "door_fence")
        }
        "NO_INVOCATION_AT_REGISTRATION" => registration,
        "METADATA_HAS_NO_CHARGE" => {
            value.starts_with("record_") && (route == "R5" && sink != "call_chat")
        }
        "SETTLEMENT_HAS_NO_NEW_WORK_ADMISSION" => {
            value == "decline" && matches!(route, "R4" | "R7" | "R8" | "R9")
        }
        // Normal is denied earlier only for Bootstrap-only registration.
        "LIFECYCLE_DENIES_BEFORE_THIS_SINK" => {
            dimension == "lifecycle" && (value != "Normal" || registration)
                || registration && value == "quit"
        }
        "LEGACY_FINALIZATION_ONLY" => matches!(sink, "mother_end" | "primary_end" | "delivery"),
        _ => false,
    }
}

fn check_disposition(
    c: &Context,
    (route, sink, dimension, value): (&str, &str, &str, &str),
    disposition: &str,
) -> Result<String, String> {
    let at = format!("{sink}/{dimension}/{value}");
    let (kind, rest) = disposition
        .split_once(':')
        .ok_or_else(|| format!("{at}: no disposition in {disposition:?}"))?;
    let key = evidence_key(route, sink);
    let allowed = |test: &str| {
        c.evidence
            .get(key.as_str())
            .is_some_and(|set| set.contains(test))
    };
    match kind {
        "c" | "x" => {
            if c.receipts.contains(rest) {
                return Err(format!("{at}: historical receipt {rest} is not a test"));
            }
            if kind == "x" && !is_test(rest, C2_ORACLES) {
                return Err(format!("{at}: x: only a C2-new oracle may claim execution"));
            }
            if kind == "c" && !is_test(rest, &c.sources) {
                return Err(format!("{at}: no such retained test {rest}"));
            }
            if !allowed(rest) {
                return Err(format!("{at}: {rest} is unrelated to {route}"));
            }
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
                    if !named || !is_test(o, &c.sources) || !allowed(o) {
                        return Err(format!("{at}: {o} is not this sink's denial oracle"));
                    }
                }
                None if !denies => {}
                _ => return Err(format!("{at}: {reason} and its oracle do not agree")),
            }
            if !na_allowed(reason, route, sink, dimension, value) {
                return Err(format!("{at}: N/A escape: {reason} does not apply here"));
            }
            Ok(format!("na:{reason}"))
        }
        "open" => {
            if !words(OWNERS).contains(&rest) {
                return Err(format!("{at}: OPEN needs an owner, got {rest}"));
            }
            let cell = (sink.to_owned(), dimension.to_owned(), value.to_owned());
            if c.open.get(&cell).copied() != Some(rest) {
                return Err(format!("{at}: OPEN is not listed for this cell"));
            }
            Ok(format!("open:{rest}"))
        }
        _ => Err(format!("{at}: unknown disposition {disposition:?}")),
    }
}

fn check_row(
    c: &Context,
    f: &[&str],
    opened: &mut BTreeSet<(String, String, String)>,
    summary: &mut Summary,
) -> Result<(), String> {
    let (route, sink) = (f[0], f[1]);
    let Some((owner_route, gate, oracles, plants)) = c.frozen.get(sink) else {
        return Err(format!("foreign sink {sink}"));
    };
    if route != *owner_route || f[2] != gate {
        return Err(format!("{sink}: wrong route or gate ({route}, {})", f[2]));
    }
    let evidence = &c.evidence[evidence_key(route, sink).as_str()];
    let known = is_test(f[8], &c.sources) || is_test(f[8], C2_ORACLES);
    if !evidence.contains(f[8]) || !known {
        return Err(format!("{sink}: primitive {} is not its evidence", f[8]));
    }
    if !oracles.contains(&f[9]) || !plants.contains(&f[10]) {
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
                let key = check_disposition(c, (route, sink, dimension, value), disposition)?;
                if key.starts_with("open:") {
                    opened.insert((sink.to_owned(), dimension.to_string(), value.to_owned()));
                }
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
    let c = context();
    let mut lines = tsv.lines();
    if lines.next() != Some(HEADER) {
        return Err("missing or altered header".into());
    }
    let rows: Vec<&str> = lines.collect();
    if rows.len() != c.frozen.len() {
        return Err(format!("expected 35 data rows, found {}", rows.len()));
    }
    let (mut seen, mut opened) = (BTreeSet::new(), BTreeSet::new());
    let mut summary = Summary::default();
    for line in rows {
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() != 11 || f.iter().any(|field| field.is_empty()) {
            return Err(format!("malformed row: {line}"));
        }
        if !seen.insert(f[1]) {
            return Err(format!("duplicate sink {}", f[1]));
        }
        check_row(&c, &f, &mut opened, &mut summary)?;
        summary.rows += 1;
    }
    let gates = |p: &str| c.frozen.values().filter(|s| s.1.starts_with(p)).count();
    if (gates("reg-"), gates("call-")) != (4, 4) || summary.rows != 35 {
        return Err("35 sinks, four registration and four call gates required".into());
    }
    let listed: BTreeSet<_> = c.open.keys().cloned().collect();
    if opened != listed {
        return Err(format!(
            "listed OPEN cells are no longer OPEN: {:?}",
            listed.difference(&opened).take(5).collect::<Vec<_>>()
        ));
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
    let (sources, host) = (evidence_sources(), host_evidence_sources());
    let block: Vec<Vec<&str>> = cases
        .lines()
        .map(|l| l.split('\t').collect::<Vec<_>>())
        .filter(|r| r.len() == 5 && r[4] == "SB-3C")
        .collect();
    let mut remaining: BTreeSet<String> = words(INGRESS)
        .iter()
        .map(|i| format!("SB3C/ingress/{i}"))
        .chain(RECORDED.map(String::from))
        .collect();
    for row in &block {
        if !remaining.remove(row[0]) {
            return Err(format!("unknown or duplicate SB-3C row {}", row[0]));
        }
        let Some(class) = row[0].strip_prefix("SB3C/ingress/") else {
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
        if owned != (f[2] == "none") || !owned && !is_test(session_test, &sources) {
            return Err(format!("{class}: no executed session-level primitive"));
        }
        if !owned && !is_test(f[2], &host) {
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
call_chat|5|c:sb3_matrix_r5_observer_before|c:sb3c_registration_is_bootstrap_only_once|unrelated
call_chat|5|c:sb3_matrix_r5_observer_before|x:sb3_matrix_r5_cas_after|only a C2-new oracle
call_chat|5|c:sb3_matrix_r5_observer_before|na:BECAUSE|not on the closed list
call_chat|5|c:sb3_matrix_r5_observer_before|na:METADATA_HAS_NO_CHARGE|N/A escape
call_chat|5|c:sb3_matrix_r5_observer_before|na:LEGACY_FINALIZATION_ONLY|N/A escape
call_chat|5|c:sb3_matrix_r5_observer_before|na:LIFECYCLE_DENIES_BEFORE_THIS_SINK|do not agree
call_chat|5|c:sb3_matrix_r5_observer_before|na:BOOTSTRAP_NO_MISSION(sb3_post_seal_model_events_are_absent)|do not agree
call_chat|5|c:sb3_matrix_r5_observer_before|na:LIFECYCLE_DENIES_BEFORE_THIS_SINK(sb3_matrix_r5_cas_after)|denial oracle
call_chat|5|c:sb3_matrix_r5_observer_before|na:LIFECYCLE_DENIES_BEFORE_THIS_SINK(sb2_unjoined_or_owed_prevents_clean_close)|N/A escape
call_chat|4|c:sb3_matrix_r5_observer_before|na:LIFECYCLE_DENIES_BEFORE_THIS_SINK(sb2_unjoined_or_owed_prevents_clean_close)|N/A escape
call_chat|5|c:sb3_matrix_r5_observer_before|open:D9|OPEN needs an owner
call_chat|5|c:sb3_matrix_r5_observer_before|open:D2|OPEN is not listed
close_report|5|open:D2|c:sb3_completed_call_allows_existing_clean_close|no longer OPEN
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

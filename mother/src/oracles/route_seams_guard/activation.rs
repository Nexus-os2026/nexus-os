//! SB-3E U22: the set-B activation check (SB3E-R196.1, R255.2, R260).
//!
//! SOURCE-FACT GUARD. Everything here reads source text and data files. It
//! proves that the cited code facts hold in the text of this tree; it is NOT
//! runtime proof that a sink is unreachable.
//!
//! One check joins, by key, the expect table (905 keys), the literal credit
//! manifest and the generated activation register (activation_register.py).
//! Every set-B key must be credited, carry a reviewed resolution, or be
//! owed-guarded. It fails on a duplicate, missing, unknown, expired or
//! reopened entry, and no list excuses an unsettled key: an `uncredited` row
//! is itself a finding. W1 keys rest on a per-event caller guard (R214.3 D1)
//! over Mother and governed-control. W2 and W3 keys are reported by key.
use super::*;
use std::collections::{BTreeMap, BTreeSet};

const EXPECT_TSV: &str = "host/tests/session/model-bridge-expect.tsv";
const MANIFEST_RS: &str = "host/tests/route_cells/cells.rs";
const REGISTER_TSV: &str = "host/tests/session/model-bridge-activation.tsv";
const TR1: &str = "hold:unreachable_by_production_order";
const BYTES: [&str; 6] = ["CR", "LF", "CRLF", "EOF", "partial_Ctrl_D", "master_close"];
const OWNER_LIFE: [&str; 4] = ["Emergency", "Failed", "ClosingNormal", "ClosingRestricted"];
const NO_CALLER: [&str; 3] = ["mission_stop", "stop_all", "grant_revocation"];
/// R255.2: the sinks only a model call reaches. `lane_result_admission`, the
/// registration sinks and `cancel_run` are reached by the shipped host today.
const MODEL_PATH: [&str; 15] = [
    "identity_view",
    "inference_binding",
    "call_slot_reserve",
    "attempt_ticket",
    "charge_append",
    "meter_settle",
    "call_version",
    "call_status",
    "call_tags",
    "call_chat",
    "attempt_record",
    "attempt_pin_release",
    "operation_complete",
    "record_first_classify",
    "post_record_recheck",
];
/// The owed-set pin: the number of the register's owed rows and the sha256 of
/// their sorted keys (one per line), as activation_register.py prints it.
const OWED: (usize, &str) = (
    27,
    "12f42f81b3df5be1dc47cc850ab5d271783a66c7418c7cc0e34451971e871f99",
);

// --- the three code facts of R255.2 --------------------------------------------
/// `fact|file|enclosing fn|source text`: the tokens of the text occur exactly
/// once among the file's production tokens, inside that `fn`. A missing,
/// repeated or moved anchor is a finding (fail closed).
const OWED_FORMS: [(&str, &str, &str, &str); 12] = [
    (
        "F1 refusal",
        "host/src/hands.rs",
        "run",
        r#"} else if model
                && command
                    .request
                    .as_ref()
                    .is_some_and(|r| r.kind() == CommandKind::Run)
            {
                let refusal = page_preview(&screen, &session, page)
                    .err()
                    .unwrap_or("page_mission_unavailable");
                screen.emit("RESULT: request refused; no new operation was admitted.\n");
                screen.emit(&format!("REFUSED {refusal}\n"));
            } else if let Some(request) = command.request {"#,
    ),
    (
        "F1 model flag",
        "host/src/hands.rs",
        "run",
        "let model = page_args.is_some();",
    ),
    (
        "F2 trait default",
        "src/lane.rs",
        "-",
        "fn max_model_calls(&self) -> u32 { 0 }",
    ),
    (
        "F2 scripted lanes",
        "host/src/hands.rs",
        "run",
        "let lanes: Vec<Arc<dyn Lane>> = LaneRole::ALL.into_iter().map(|role| { let mut lane = ScriptedLane::new(",
    ),
    (
        "F2 observed wrapper",
        "host/src/hands.rs",
        "run",
        "Arc::new(ObservedLane { route: self.screen.stop.clone(), lane, }) as Arc<dyn Lane> }).collect();",
    ),
    (
        "F2 lanes passed",
        "host/src/hands.rs",
        "run",
        "let lanes = Lanes::new(lanes).map_err(",
    ),
    (
        "F2 ObservedLane impl",
        "host/src/hands.rs",
        "-",
        "impl Lane for ObservedLane { fn identity(&self) -> LaneIdentity { self.lane.identity() } fn invoke(&self, input: &LaneInput) -> LaneOutput { let output = self.lane.invoke(input);",
    ),
    (
        "F2 ScriptedLane impl",
        "src/lane.rs",
        "-",
        "impl Lane for ScriptedLane { fn identity(&self) -> LaneIdentity { self.identity.clone() } fn invoke(&self, _: &LaneInput) -> LaneOutput { self.output.clone() } }",
    ),
    (
        "F3 identity under a positive cap",
        "src/coordinator.rs",
        "run_lanes",
        "let cap = lane.max_model_calls(); let model = (cap > 0).then(|| self.port.model_identity()).flatten();",
    ),
    (
        "F3 submit",
        "src/coordinator.rs",
        "submit",
        ".any(|lane| lane.max_model_calls() > 0) && (self.file.is_none() || self.port.model_identity().is_none())",
    ),
    (
        "F3 caller under a positive cap",
        "src/coordinator.rs",
        "run_lanes",
        "Then::Go(input) if cap > 0 => Some(model_calls::Caller::reserved(",
    ),
    (
        "F3 admission only with a caller",
        "src/coordinator.rs",
        "run_lanes",
        "if let Some(caller) = &caller { if caller.admit().is_err() {",
    ),
];
/// R214.3 D1 over the facts' identifiers: every production occurrence but
/// its own `fn`, `file|enclosing fn|name[*count]`. A new caller of the
/// identity view or of the inference admission, or a new cap, is unlisted.
const OWED_NAMES: [&str; 5] = [
    "model_identity",
    "admit_inference",
    "max_model_calls",
    "PageBuilderLane",
    "page_preview",
];
const OWED_SITES: &str = "\
host/src/hands.rs|run|max_model_calls
host/src/hands.rs|run|page_preview
src/adapter/lite_review4.rs|approved|max_model_calls
src/adapter/lite_review5.rs|approved|max_model_calls
src/adapter/lite_review6.rs|lite_failure_after_recognised_file|max_model_calls
src/adapter/mod.rs|-|admit_inference
src/adapter/mod.rs|model_identity|model_identity
src/adapter/session_port.rs|-|admit_inference*3
src/adapter/session_port.rs|model_identity|model_identity*3
src/consent.rs|-|max_model_calls
src/consent.rs|lines|max_model_calls
src/consent.rs|of|max_model_calls*2
src/contract.rs|-|max_model_calls
src/contract.rs|validate|max_model_calls
src/coordinator.rs|load_content|max_model_calls*2
src/coordinator.rs|run_lanes|max_model_calls
src/coordinator.rs|run_lanes|model_identity
src/coordinator.rs|submit|max_model_calls
src/coordinator.rs|submit|model_identity
src/coordinator/model_calls.rs|admit|admit_inference
src/coordinator/model_calls.rs|reserve_lane|max_model_calls
src/page_builder/lane.rs|-|PageBuilderLane*2
src/page_mission.rs|-|max_model_calls
src/page_mission.rs|preview|max_model_calls
src/teaching.rs|explain|max_model_calls
";
/// Every production `fn max_model_calls` (a definition is not a D1 row).
const CAP_DEFINITIONS: [&str; 2] = ["src/lane.rs", "src/page_builder/lane.rs"];
/// `page_preview` only previews: none of these names in its body.
const PREVIEW_FORBIDDEN: [&str; 8] = [
    "spawn",
    "producer",
    "runtime",
    "Runtime",
    "command",
    "Coordinator",
    "open_confirmed_file_door",
    "Lanes",
];

// --- W1: no production caller (R196.1) ---------------------------------------
/// Mother's own tree: `Coordinator::stop` / `FileSession::stop` (mission_stop),
/// `stop_all`, and the grant revocation, by name.
const W1_NAMES: [(&str, &str); 5] = [
    ("stop", "mission_stop"),
    ("stop_all", "stop_all"),
    ("revoke_grant", "grant_revocation"),
    ("revoke", "grant_revocation"),
    ("route_revoke_original_grant", "grant_revocation"),
];
const W1_SITES: &str = "\
desktop/src/app.rs|-|stop
desktop/src/lib.rs|-|stop
desktop/src/status.rs|spawn_feed|stop*2
desktop/src/window.rs|-|stop
host/src/hands.rs|-|stop
host/src/hands.rs|approve|stop
host/src/hands.rs|failed_startup|stop*3
host/src/hands.rs|inventory|stop
host/src/hands.rs|operation_result|stop
host/src/hands.rs|progress|stop
host/src/hands.rs|run|stop*6
host/src/lib.rs|-|stop
host/src/stop.rs|-|stop
host/src/stop.rs|report_receipts|stop*6
host/src/terminal.rs|-|stop*2
host/src/terminal.rs|ask|stop
host/src/terminal.rs|drop|stop*2
host/src/terminal.rs|fail|stop
host/src/terminal.rs|goal_valid|stop
host/src/terminal.rs|input|stop
host/src/terminal.rs|next|stop
host/src/terminal.rs|open|stop*3
host/src/terminal.rs|protocol_valid|stop
host/src/terminal.rs|quiesce|stop
host/src/terminal.rs|read_goal|stop
host/src/terminal.rs|read_input|stop*2
host/src/terminal.rs|receive|stop*6
host/src/terminal.rs|route_bind|stop
host/src/terminal.rs|valid|stop
src/adapter/lite_recognition.rs|lite_recognition_child|stop
src/adapter/lite_review1_dispatch.rs|lite_review1_dispatch_after_stop_replays|stop
src/adapter/lite_review1_sequential.rs|lite_review1_b_before_perform_never_settles_as_a|stop
src/adapter/lite_review4.rs|lite_first_cause_competing_touch|stop
src/adapter/lite_review4.rs|terminal_pair|stop*4
src/adapter/lite_review4.rs|terminal_pair|stop_all
src/adapter/lite_review5.rs|run_case|stop*2
src/adapter/lite_review5.rs|run_case|stop_all
src/adapter/lite_review6.rs|lite_failure_after_recognised_file|stop
src/adapter/lite_settlement.rs|lite_settlement_child|stop*10
src/adapter/lite_settlement.rs|lite_settlement_child|stop_all*2
src/adapter/session_port.rs|route_revoke_original_grant|route_revoke_original_grant
src/adapter/session_terminal_oracles_tests.rs|child|stop*3
src/coordinator.rs|invalidate_calls|revoke
src/coordinator.rs|stop_all|stop
src/coordinator/lite.rs|stop_all|stop
src/coordinator/lite.rs|stop|stop
";
/// governed-control, walked from its own root (`gc:` rows; SB3E-R260.1 F6):
/// the revocation only. `close_session_authority`'s loop is the one
/// production revoker; `revoke_grant`'s one caller is the S3 seam.
const W1_GC_NAMES: [(&str, &str); 2] = [
    ("revoke_grant", "grant_revocation"),
    ("revoke", "grant_revocation"),
];
const W1_GC_SITES: &str = "\
gc:src/browser/tests.rs|a_revoked_grant_ends_the_session_before_its_next_step|revoke
gc:src/evidence_store/sink_tests.rs|el1_m6_c1_an_executing_commitment_is_ended_only_by_its_finish|revoke
gc:src/evidence_store/sink_tests.rs|el1_m6_mb8_a_revocation_with_a_refusing_store_takes_effect_and_is_owed|revoke
gc:src/files/mod.rs|close|revoke
gc:src/files/mod.rs|closure|revoke
gc:src/files/mod.rs|narrow_grants|revoke
gc:src/files/mod.rs|revoke_write_grant_for_test|revoke
gc:src/governed.rs|close_session_authority|revoke
gc:src/governed.rs|revoke_grant|revoke
gc:src/governed/files_exec_tests.rs|arm|revoke_grant
gc:src/governed/files_exec_tests.rs|m2d_b2_d_o16_the_bridge_through_the_composed_route|revoke_grant
gc:src/governed/files_tests.rs|m2d_b1_d_o12_prepare_refusals_leave_nothing|revoke_grant
gc:src/governed/session_routes_tests.rs|session_each_native_work_step_rechecks_its_actual_prerequisites|revoke_grant
gc:src/host_session/route_s3.rs|route_revoke_original_grant|revoke_grant
gc:src/inference.rs|drop|revoke
";

type Lexed<'a> = Vec<(&'a str, Vec<&'a str>)>;
fn lexed(sources: &[(String, String)]) -> Lexed<'_> {
    sources
        .iter()
        .filter(|(_, s)| tokens(s).is_ok())
        .map(|(f, s)| (f.as_str(), lex(s)))
        .collect()
}
/// governed-control's non-production files: its integration tests and the
/// modules it declares under `#[cfg(test)]` at their default path.
fn gc_excluded(file: &str, mods: &[String]) -> bool {
    ["tests/", "benches/", "examples/"]
        .iter()
        .any(|d| file.starts_with(d))
        || mods
            .iter()
            .any(|m| file == format!("{m}.rs") || file.starts_with(&format!("{m}/")))
}
/// D1: each production occurrence of a name in `names` but its own `fn`.
fn name_sites(
    l: &Lexed,
    excluded: impl Fn(&str) -> bool,
    names: &[&str],
    tag: &str,
) -> Vec<String> {
    let mut rows = Vec::new();
    for (file, t) in l.iter().filter(|(f, _)| !excluded(f)) {
        let pick = |i: usize| names.contains(&t[i]) && (i == 0 || t[i - 1] != "fn");
        for (within, i) in production_sites(t, pick) {
            rows.push(format!("{tag}{file}|{within}|{}", t[i]));
        }
    }
    rows
}
/// `rows` against a pinned `row[*count]` list, as a multiset.
fn pin_findings(what: &str, rows: &[String], pins: &str) -> Vec<String> {
    let count = |l: &str| -> (String, usize) {
        l.split_once('*').map_or((l.to_string(), 1), |(r, n)| {
            (r.to_string(), n.parse().unwrap_or(0))
        })
    };
    let mut listed: Vec<String> = pins
        .lines()
        .map(count)
        .flat_map(|(r, n)| std::iter::repeat_n(r, n))
        .collect();
    let mut found = Vec::new();
    for row in rows {
        match listed.iter().position(|l| l == row) {
            Some(at) => drop(listed.remove(at)),
            None => found.push(format!("{what}: unlisted {row}")),
        }
    }
    found.extend(listed.iter().map(|l| format!("{what}: missing {l}")));
    found
}
/// R255.2's facts, read from Mother's sources: the page `run` refusal, cap 0
/// on every shipped lane, and no model identity without a positive cap at
/// either site (`run_lanes` and `submit`, SB3E-R260.1 F1).
fn owed_fact_findings(sources: &[(String, String)]) -> Vec<String> {
    let l = lexed(sources);
    let mods = test_mods(&l);
    let mut found = Vec::new();
    for (fact, file, within, text) in OWED_FORMS {
        let Some((_, t)) = l.iter().find(|(f, _)| *f == file) else {
            found.push(format!("owed facts: {fact}: no {file}"));
            continue;
        };
        let form = piece(text);
        let at = |i: usize| t[i..].starts_with(&form);
        let sites = production_sites(t, at);
        let all = (0..t.len()).filter(|&i| at(i)).count();
        match sites[..] {
            [(inside, _)] if all == 1 && inside == within => {}
            [(inside, _)] if all == 1 => found.push(format!(
                "owed facts: {fact}: {file}: the anchor moved to fn {inside}, not {within}"
            )),
            _ => found.push(format!(
                "owed facts: {fact}: {file}: the anchor occurs {} times in production text ({all} in all), not once",
                sites.len()
            )),
        }
    }
    // F1: the refusal branch is ahead of the dispatch in one `run`, and the preview only previews.
    if let Some((_, hands)) = sources.iter().find(|(f, _)| f == "host/src/hands.rs") {
        match item(hands, "page_preview") {
            None => found.push("owed facts: F1: no single fn page_preview".into()),
            Some(body) => {
                let t = lex(body);
                for name in PREVIEW_FORBIDDEN.iter().filter(|n| t.contains(n)) {
                    found.push(format!("owed facts: F1: page_preview names {name}"));
                }
            }
        }
    }
    // F2: no cap is defined outside the two reviewed files.
    let mut defined: Vec<&str> = l
        .iter()
        .filter(|(f, _)| !caller_excluded(f, &mods))
        .filter(|(_, t)| {
            !production_sites(t, |i| {
                i > 0 && t[i - 1] == "fn" && t[i] == "max_model_calls"
            })
            .is_empty()
        })
        .map(|(f, _)| *f)
        .collect();
    defined.sort();
    if defined != CAP_DEFINITIONS {
        found.push(format!(
            "owed facts: F2: fn max_model_calls is defined in {defined:?}, not {CAP_DEFINITIONS:?}"
        ));
    }
    // F2: the shipped host implements `Lane` once, for `ObservedLane`.
    for (file, t) in l.iter().filter(|(f, _)| !caller_excluded(f, &mods)) {
        if !(file.starts_with("host/src/") || file.starts_with("desktop/src/")) {
            continue;
        }
        let lane_impl = |i: usize| t[i..].starts_with(&["impl", "Lane", "for"]);
        for (_, i) in production_sites(t, lane_impl) {
            if (*file, t[i + 3]) != ("host/src/hands.rs", "ObservedLane") {
                found.push(format!(
                    "owed facts: F2: {file} implements Lane for {}",
                    t[i + 3]
                ));
            }
        }
    }
    let rows = name_sites(&l, |f| caller_excluded(f, &mods), &OWED_NAMES, "");
    found.extend(pin_findings("owed facts: sites", &rows, OWED_SITES));
    found
}
/// R196.1 W1: per event (by its guard name), every production occurrence of
/// its primitive's name (D1: calls, method values, `use .. as`, raw identifiers).
const W1_GUARDS: [&str; 3] = ["w1_mission_stop", "w1_stop_all", "w1_grant_revocation"];
fn w1_findings(sources: &[(String, String)], gc: &[(String, String)]) -> Facts {
    let (l, g) = (lexed(sources), lexed(gc));
    let (mods, gc_mods) = (test_mods(&l), test_mods(&g));
    let mut by_event: Facts = W1_GUARDS.iter().map(|g| (*g, Vec::new())).collect();
    for event in NO_CALLER {
        let of = |names: &[(&'static str, &str)]| -> Vec<&'static str> {
            names.iter().filter(|n| n.1 == event).map(|n| n.0).collect()
        };
        let pinned = |pins: &str, names: &[&str]| -> String {
            let named = |l: &&str| {
                names.contains(
                    &l.rsplit('|')
                        .next()
                        .unwrap_or("")
                        .split('*')
                        .next()
                        .unwrap_or(""),
                )
            };
            pins.lines()
                .filter(named)
                .map(|l| format!("{l}\n"))
                .collect()
        };
        let (names, gc_names) = (of(&W1_NAMES), of(&W1_GC_NAMES));
        let rows = name_sites(&l, |f| caller_excluded(f, &mods), &names, "");
        let gc_rows = name_sites(&g, |f| gc_excluded(f, &gc_mods), &gc_names, "gc:");
        let what = format!("W1 {event}");
        let found = by_event
            .get_mut(W1_GUARDS[NO_CALLER.iter().position(|e| *e == event).unwrap()])
            .unwrap();
        found.extend(pin_findings(&what, &rows, &pinned(W1_SITES, &names)));
        found.extend(pin_findings(
            &what,
            &gc_rows,
            &pinned(W1_GC_SITES, &gc_names),
        ));
    }
    by_event
}

// --- the join ----------------------------------------------------------------
/// What each source guard found, by the guard name a register row carries.
type Facts = BTreeMap<&'static str, Vec<String>>;
fn facts(sources: &[(String, String)], gc: &[(String, String)]) -> Facts {
    let hands = sources.iter().find(|(f, _)| f == "host/src/hands.rs");
    let order = hands.map_or_else(|| vec!["no hands.rs".into()], |h| order_findings(&h.1));
    let mut by_guard = Facts::from([
        ("model_path", owed_fact_findings(sources)),
        ("order", order),
        ("close_callers", close_caller_findings(sources)),
        ("door_callers", door_caller_findings(sources)),
        ("t7", t7_findings(sources)),
        ("none", Vec::new()),
    ]);
    by_guard.extend(w1_findings(sources, gc));
    by_guard
}
fn sha256(text: &str) -> String {
    use sha2::Digest;
    let digest = sha2::Sha256::digest(text.as_bytes());
    digest.iter().map(|b| format!("{b:02x}")).collect()
}
/// The literal `cells!` manifest as `rN/sink/dim/value` keys.
fn manifest_keys(cells: &str) -> BTreeSet<String> {
    let t = lex(cells);
    let Some(start) = (0..t.len()).find(|&i| t[i..].starts_with(&["cells", "!", "{"])) else {
        return BTreeSet::new();
    };
    let body = &t[start + 3..close_of(&t, start + 2)];
    let entries = body.split(|w| *w == ";").filter(|e| !e.is_empty());
    entries.map(|e| e.join("/")).collect()
}
/// SB3E-R260.2: set B is a B-valued row (R196.1) that is `open` or held by
/// TR-1. A B-valued row under an earlier `hold:` word is W2.
fn set_b(f: &[&str]) -> bool {
    let valued = matches!(f[4], "byte" | "close_report")
        || (f[2] == "ingress" && matches!(f[3], "quit" | "repeated"))
        || (f[2] == "lifecycle" && OWNER_LIFE.contains(&f[3]));
    valued && (f[17] == "open" || f[17] == TR1)
}
/// The reviewed resolutions (docs 14.18, 14.19, 14.22), closed: a set-B
/// row's `(reason, guard)`, or None when no review covers the key.
fn reviewed(f: &[&str]) -> Option<(&'static str, &'static str)> {
    const CLOSE: [&str; 5] = [
        "close_obligations",
        "producer_join",
        "seal",
        "close_restrict",
        "emergency_restrict",
    ];
    const REPORT_W1: [&str; 7] = [
        "original_cell_unscheduled",
        "repeated",
        "mission_stop",
        "stop_all",
        "door_fence",
        "fatal",
        "quit",
    ];
    let (sink, dim, value) = (f[1], f[2], f[3]);
    let report = sink == "close_report";
    let guard = if report { "order" } else { "close_callers" };
    let again = matches!(value, "repeated" | "quit");
    let door = sink == "door_halt"
        && match dim {
            "ingress" => BYTES.contains(&value) || again || value == "read_failure",
            _ => dim == "lifecycle" && OWNER_LIFE.contains(&value),
        };
    let by_fault = matches!(
        (dim, value),
        ("lifecycle", "ClosingNormal" | "ClosedUnclean") | ("fault", "poison" | "panic")
    );
    match dim {
        _ if f[17] == TR1 => Some(("tr1_hold", guard)),
        "ingress" if report && REPORT_W1.contains(&value) => Some(("w1_type", guard)),
        "ingress" if CLOSE.contains(&sink) && again => Some(("w1_type", guard)),
        _ if door => Some(("w3_event", "door_callers")),
        _ if report && by_fault => Some(("w3_event", "none")),
        _ => None,
    }
}
/// The owed set (R255.2; race B stays owed, R265.4): the reason of a key
/// that may be owed-guarded, or None.
fn owed_reason(f: &[&str]) -> Option<&'static str> {
    let (sink, dim, value) = (f[1], f[2], f[3]);
    let s2 = sink == "identity_view"
        && match dim {
            "ingress" => BYTES.contains(&value) || value == "repeated",
            _ => dim == "lifecycle" && matches!(value, "Emergency" | "ClosingRestricted"),
        };
    let race_b = (sink, dim) == ("call_slot_reserve", "ingress") && BYTES[..3].contains(&value);
    match (dim, value) {
        _ if !MODEL_PATH.contains(&sink) || f[17] != "open" || reviewed(f).is_some() => None,
        ("ingress", "read_failure") => Some("read_failure"),
        _ if s2 => Some("s2"),
        _ if race_b => Some("race_b"),
        _ => None,
    }
}
/// The activation check: `(findings, report)`. The report lists the owed,
/// W2 and W3 keys; it is information, never a pass.
fn activation_findings(
    expect: &str,
    manifest: &str,
    register: &str,
    facts: &Facts,
) -> (Vec<String>, Vec<String>) {
    let (mut found, mut report) = (Vec::new(), Vec::new());
    found.extend(facts.get("t7").into_iter().flatten().cloned());
    let credited = manifest_keys(manifest);
    let mut entries: BTreeMap<&str, Vec<Vec<&str>>> = BTreeMap::new();
    for line in register.lines().skip(1).filter(|l| !l.is_empty()) {
        let f: Vec<&str> = line.split('\t').collect();
        match f.len() == 5 && f.iter().all(|x| !x.is_empty()) {
            true => entries.entry(f[0]).or_default().push(f),
            false => found.push(format!("malformed entry: {line}")),
        }
    }
    let broken = |guard: &str| match facts.get(guard) {
        None => Some(format!("unknown guard {guard}")),
        Some(found) => found.first().cloned(),
    };
    let (mut seen, mut owed) = (BTreeSet::new(), Vec::new());
    for (n, line) in expect.lines().enumerate().skip(1) {
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() != 18 {
            found.push(format!("malformed table row x:{}", n + 1));
            continue;
        }
        let key = format!("{}/{}/{}/{}", f[0].to_lowercase(), f[1], f[2], f[3]);
        let at = format!("{key} (x:{})", n + 1);
        if !seen.insert(key.clone()) {
            found.push(format!("duplicate key: {at}"));
        }
        let (credit, b) = (credited.contains(&key), set_b(&f));
        let w1 = !b && !credit && f[17] == "open" && NO_CALLER.contains(&f[3]);
        let rows = entries.get(key.as_str()).map_or(&[][..], |r| &r[..]);
        if rows.len() > 1 {
            found.push(format!("duplicate entry: {at}"));
        }
        for e in rows {
            let (state, reason, guard) = (e[1], e[2], e[3]);
            if state == "owed" && !MODEL_PATH.contains(&f[1]) {
                let sink = f[1];
                found.push(format!(
                    "owed refused: {at}: the shipped host reaches {sink} today"
                ));
            }
            let finding = match state {
                _ if credit => Some(format!("duplicate: {at} is credited and has an entry")),
                "w1" if w1 && (reason, guard) == (f[3], format!("w1_{}", f[3]).as_str()) => {
                    broken(guard).map(|why| format!("reopened: {at} becomes set B: {why}"))
                }
                _ if w1 => Some(format!(
                    "malformed entry: {at}: a W1 row names its own event's guard"
                )),
                _ if !b => Some(format!(
                    "unknown: {at} is outside set B and W1 but has an entry"
                )),
                "resolved" if reason == "tr1_hold" && f[17] != TR1 => Some(format!(
                    "reopened: {at}: status {} is not a tr1_hold row's",
                    f[17]
                )),
                "resolved" => match reviewed(&f) {
                    None => Some(format!(
                        "unknown resolution: {at}: {reason} is not reviewed for this key"
                    )),
                    Some(want) if want != (reason, guard) => Some(format!(
                        "unknown guard: {at}: {reason} by {guard}, not {} by {}",
                        want.0, want.1
                    )),
                    Some(_) => broken(guard)
                        .map(|why| format!("reopened: {at}: its guard {guard} fails: {why}")),
                },
                "owed" if !MODEL_PATH.contains(&f[1]) => None,
                "owed" => match owed_reason(&f) {
                    None => Some(format!("owed refused: {at} is outside the owed set")),
                    Some(want) if (want, "model_path") != (reason, guard) => Some(format!(
                        "malformed entry: {at}: an owed {want} row rests on model_path"
                    )),
                    Some(_) => {
                        owed.push(key.clone());
                        report.push(format!("OWED\t{at}\t{reason}"));
                        broken("model_path").map(|why| format!("expired: {at}: {why}"))
                    }
                },
                "uncredited" => Some(format!(
                    "missing: set-B key {at} is not credited, resolved or owed (uncredited: {reason})"
                )),
                _ => Some(format!("unknown state {state} at {at}")),
            };
            found.extend(finding);
        }
        match rows.is_empty() {
            _ if credit => {}
            true if b => found.push(format!("missing: set-B key {at} has no register row")),
            true if w1 => found.push(format!("missing: W1 key {at} has no guard row")),
            true => report.push(match f[17].split_once(':') {
                None => format!("W3\t{at}\topen"),
                Some(("denial", oracle)) => format!("W2\t{at}\tdenial: oracle missing ({oracle})"),
                Some(_) => format!("W2\t{at}\t{}", f[17]),
            }),
            false => {}
        }
    }
    for key in entries.keys().filter(|k| !seen.contains(**k)) {
        found.push(format!("unknown key: {key} is not in the expect table"));
    }
    if seen.len() != 905 {
        found.push(format!("the table has {} keys, not 905", seen.len()));
    }
    owed.sort();
    let digest = sha256(&owed.iter().map(|k| format!("{k}\n")).collect::<String>());
    if (owed.len(), digest.as_str()) != OWED {
        found.push(format!(
            "owed set: {} keys, sha256 {digest}; not the pinned {} ({})",
            owed.len(),
            OWED.0,
            OWED.1
        ));
    }
    (found, report)
}

// --- the real tree, and the controls -------------------------------------------
fn gc_sources() -> Vec<(String, String)> {
    let root = crate::adapter::scan::crate_root().join("../crates/nexus-governed-control");
    crate::adapter::scan::route_guard_sources(&root).0
}
fn data(file: &str) -> String {
    let path = crate::adapter::scan::crate_root().join(file);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}
/// Sources and data are read at run time, so a built guard sees a planted file.
struct Tree {
    sources: Vec<(String, String)>,
    gc: Vec<(String, String)>,
    expect: String,
    manifest: String,
    register: String,
    facts: Facts,
}
impl Tree {
    fn read() -> Self {
        let (sources, gc) = (walked().0, gc_sources());
        let facts = facts(&sources, &gc);
        let [expect, manifest, register] = [EXPECT_TSV, MANIFEST_RS, REGISTER_TSV].map(data);
        Self {
            sources,
            gc,
            expect,
            manifest,
            register,
            facts,
        }
    }
    fn found(&self) -> Vec<String> {
        activation_findings(&self.expect, &self.manifest, &self.register, &self.facts).0
    }
    /// The findings with one guard's result replaced (a planted source).
    fn with(&self, changed: Facts) -> Vec<String> {
        let mut facts = self.facts.clone();
        facts.extend(changed);
        activation_findings(&self.expect, &self.manifest, &self.register, &facts).0
    }
    fn on(&self, register: &str) -> Vec<String> {
        activation_findings(&self.expect, &self.manifest, register, &self.facts).0
    }
    fn owed(&self, sources: &[(String, String)]) -> Vec<String> {
        self.with(Facts::from([("model_path", owed_fact_findings(sources))]))
    }
}
/// THE CHECK (SB3E-R255.2): every set-B key is credited, resolved or
/// owed-guarded, and every W1 key is guarded. No key is excused.
#[test]
#[rustfmt::skip]
fn sb3e_u22_every_set_b_key_is_credited_resolved_or_owed() {
    let tree = Tree::read();
    let (found, report) = activation_findings(&tree.expect, &tree.manifest, &tree.register, &tree.facts);
    println!("{}", report.join("\n"));
    fail_on(found);
}
/// Every source-fact guard a register row can rest on holds on this tree.
#[test]
#[rustfmt::skip]
fn sb3e_u22_source_facts_hold() {
    let facts = Tree::read().facts;
    fail_on(facts.into_iter().flat_map(|(g, found)| found.into_iter().map(move |f| format!("{g}: {f}"))).collect());
}
/// The check is red on exactly the register's `uncredited` rows (the keys a
/// cell must still credit), and green on the same tree once they are credited.
#[test]
#[rustfmt::skip]
fn sb3e_u22_red_is_the_uncredited_rows_and_green_once_credited() {
    let tree = Tree::read();
    let rows: Vec<Vec<&str>> = tree.register.lines().map(|l| l.split('\t').collect()).collect();
    let uncredited: Vec<&Vec<&str>> = rows.iter().filter(|r| r.get(1) == Some(&"uncredited")).collect();
    let mut misses = Vec::new();
    let red = tree.found();
    println!("RED {} findings\n{}", red.len(), red.join("\n"));
    for row in &uncredited {
        let want = format!("missing: set-B key {} (x:", row[0]);
        let own = red.iter().filter(|f| f.starts_with(&want) && f.ends_with(&format!("(uncredited: {})", row[2])));
        if own.count() != 1 || !matches!(row[2], "race_a" | "reachable_read_failure") {
            misses.push(format!("(a) {}: not red once as {}", row[0], row[2]));
        }
    }
    let own = |f: &&String| f.starts_with("missing: set-B key ") && f.contains(" (uncredited: ");
    misses.extend(red.iter().filter(|f| !own(f)).map(|f| format!("(a) not an uncredited row's: {f}")));
    if red.len() != uncredited.len() {
        misses.push(format!("(a) {} findings for {} uncredited rows", red.len(), uncredited.len()));
    }
    // (b) the same tree with those keys in the manifest and their rows gone.
    let cells: String = uncredited.iter().map(|r| format!("    {};\n", r[0].replace('/', " "))).collect();
    let manifest = tree.manifest.replacen("cells! {\n", &format!("cells! {{\n{cells}"), 1);
    let kept = rows.iter().filter(|r| r.get(1) != Some(&"uncredited"));
    let register: String = kept.map(|r| r.join("\t") + "\n").collect();
    let green = activation_findings(&tree.expect, &manifest, &register, &tree.facts).0;
    if !green.is_empty() || manifest_keys(&manifest).len() != manifest_keys(&tree.manifest).len() + uncredited.len() {
        misses.push(format!("(b) not green once credited: {green:?}"));
    }
    // (b) is no blanket pass: a credit alone, its row kept, is a duplicate.
    let stale = activation_findings(&tree.expect, &manifest, &tree.register, &tree.facts).0;
    if stale.iter().filter(|f| f.starts_with("duplicate: ")).count() != uncredited.len() {
        misses.push("(b) a credited key's kept row is not a duplicate".into());
    }
    fail_on(misses);
}
/// The owed state is illegal under each mutation of its facts (in memory;
/// route_plants.json plants the same on the files), and for a key outside
/// the owed set or on a sink the shipped host reaches.
#[test]
#[rustfmt::skip]
fn sb3e_u22_owed_guard_controls() {
    let tree = Tree::read();
    let (base, s) = (tree.found(), &tree.sources);
    let (h, la, co) = ("host/src/hands.rs", "src/lane.rs", "src/coordinator.rs");
    let refusal = "                let refusal = page_preview(&screen, &session, page)\n                    .err()\n                    .unwrap_or(\"page_mission_unavailable\");\n";
    let observed = "impl Lane for ObservedLane {\n";
    let scripted = "impl Lane for ScriptedLane {\n";
    let cap = "    fn max_model_calls(&self) -> u32 {\n        1\n    }\n";
    let tail = "        output\n    }\n}\n\nstruct Mission {";
    let forward = "        output\n    }\n    fn max_model_calls(&self) -> u32 {\n        self.lane.max_model_calls()\n    }\n}\n\nstruct Mission {";
    let then = "let model = (cap > 0).then(|| self.port.model_identity()).flatten();";
    let any = "            .any(|lane| lane.max_model_calls() > 0)\n            && (self.file.is_none() || self.port.model_identity().is_none())\n";
    let guard = "Then::Go(input) if cap > 0 => Some(model_calls::Caller::reserved(";
    let flag = "    let model = page_args.is_some();\n";
    let moved = planted(&planted(s, h, flag, ""), h, "fn failed_startup(\n", &format!("fn moved(page_args: Option<()>) {{\n{flag}}}\nfn failed_startup(\n"));
    let quit = "    let request = screen.input().quit.take();\n";
    let mut misses = Vec::new();
    for (plant, planted, want) in [
        ("the refusal replaced by a dispatch", planted(s, h, refusal, "                runtime.lock().unwrap().page(page);\n"), "owed facts: F1 refusal"),
        ("ObservedLane gets cap 1", planted(s, h, observed, &format!("{observed}{cap}")), "owed facts: F2 ObservedLane impl"),
        ("ObservedLane forwards the cap", planted(s, h, tail, forward), "owed facts: F2: fn max_model_calls is defined in [\"host/src/hands.rs\""),
        ("ScriptedLane gets cap 1", planted(s, la, scripted, &format!("{scripted}{cap}")), "owed facts: F2 ScriptedLane impl"),
        ("the trait default becomes 1", planted(s, la, "    fn max_model_calls(&self) -> u32 {\n        0\n    }", "    fn max_model_calls(&self) -> u32 {\n        1\n    }"), "owed facts: F2 trait default"),
        ("run_lanes: the positive-cap check removed", planted(s, co, then, "let model = self.port.model_identity();"), "owed facts: F3 identity under a positive cap"),
        ("submit: the positive-cap check removed", planted(s, co, any, "            .port\n            .model_identity()\n            .is_none()\n"), "owed facts: F3 submit"),
        ("the caller guard removed", planted(s, co, guard, "Then::Go(input) => Some(model_calls::Caller::reserved("), "owed facts: F3 caller under a positive cap"),
        ("an anchor moved to another fn", moved, "owed facts: F1 model flag: host/src/hands.rs: the anchor moved to fn moved, not run"),
        ("an anchor missing", planted(s, h, flag, "    let model = false;\n"), "owed facts: F1 model flag: host/src/hands.rs: the anchor occurs 0 times"),
        ("a method-value identity caller", planted(s, h, quit, &format!("    let _ = Some(&port).map(SessionPort::model_identity);\n{quit}")), "owed facts: sites: unlisted host/src/hands.rs|run|model_identity"),
    ] {
        let found = tree.owed(&planted);
        let expired = found.iter().filter(|f| f.starts_with("expired: ")).count();
        killed(&mut misses, plant, &found, &[want]);
        if expired != OWED.0 {
            misses.push(format!("{plant}: {expired} owed keys expired, not {}", OWED.0));
        }
    }
    let r = &tree.register;
    let row = |key: &str| format!("{r}{key}\towed\tread_failure\tmodel_path\tR255.2\n");
    let w1 = "r1/identity_view/ingress/stop_all\tw1\tstop_all\tw1_stop_all\tR196.1";
    let done = "r9/close_report/ingress/quit\tresolved\tw1_type\torder\tR199";
    let s2 = "r1/identity_view/ingress/CR\towed\ts2\tmodel_path\tR255.2\n";
    for (plant, register, want) in [
        ("owed x:177 (cancel_run, reachable)", row("r6/cancel_run/ingress/read_failure"), "owed refused: r6/cancel_run/ingress/read_failure (x:177): the shipped host reaches cancel_run today"),
        ("owed x:322 (lane admission, reachable)", row("r8/lane_result_admission/ingress/read_failure"), "owed refused: r8/lane_result_admission/ingress/read_failure (x:322): the shipped host reaches lane_result_admission today"),
        ("owed on a W3 key", row("r5/call_chat/lifecycle/Normal"), "unknown: r5/call_chat/lifecycle/Normal (x:533) is outside set B and W1 but has an entry"),
        ("owed on a model-path set-B key outside the set", row("r4/meter_settle/ingress/quit"), "is credited and has an entry"),
        ("owed on a credited key", row("r5/call_chat/ingress/quit"), "duplicate: r5/call_chat/ingress/quit (x:99) is credited and has an entry"),
        ("a W1 key turned owed", r.replacen(w1, "r1/identity_view/ingress/stop_all\towed\tread_failure\tmodel_path\tR255.2", 1), "malformed entry: r1/identity_view/ingress/stop_all (x:295): a W1 row names its own event's guard"),
        ("a resolved key turned owed", r.replacen(done, "r9/close_report/ingress/quit\towed\ts2\tmodel_path\tR255.2", 1), "owed refused: r9/close_report/ingress/quit (x:231): the shipped host reaches close_report today"),
        ("an owed key dropped", r.replacen(s2, "", 1), "missing: set-B key r1/identity_view/ingress/CR (x:286) has no register row"),
        ("an owed key dropped: the pin", r.replacen(s2, "", 1), "owed set: 26 keys"),
        ("an owed row with another reason", r.replacen(s2, &s2.replace("\ts2\t", "\tread_failure\t"), 1), "malformed entry: r1/identity_view/ingress/CR (x:286): an owed s2 row rests on model_path"),
    ] {
        let found = tree.on(&register);
        killed(&mut misses, plant, &found, &[want]);
        if found.len() <= base.len() {
            misses.push(format!("{plant}: no new finding"));
        }
    }
    // A race A key (uncredited today) can never be written as owed.
    if let Some(race) = r.lines().find(|l| l.ends_with("\tuncredited\trace_a\t-\tR265")) {
        let key = race.split('\t').next().unwrap_or("");
        let found = tree.on(&r.replacen(race, &format!("{key}\towed\trace_a\tmodel_path\tR255.2"), 1));
        killed(&mut misses, "a race A key written as owed", &found, &["is outside the owed set"]);
    }
    fail_on(misses);
}
/// A production caller of a W1 event reopens that event's rows only (D1: a
/// call, a method value, a `use .. as` alias, a raw identifier, UFCS; and a
/// caller inside governed-control).
#[test]
#[rustfmt::skip]
fn sb3e_u22_w1_caller_controls() {
    let tree = Tree::read();
    let (s, h) = (&tree.sources, "host/src/hands.rs");
    let quit = "    let request = screen.input().quit.take();\n";
    let at = |line: &str| planted(s, h, quit, &format!("    {line}\n{quit}"));
    let import = "use nexus_mother::journal::MissionId;\n";
    let top = |text: &str| planted(s, h, import, &format!("{import}{text}"));
    let call = "fn f(s: &S) {\n    s.stop_all();\n}\n";
    let mut gc_caller = tree.gc.clone();
    let governed = gc_caller.iter().position(|(f, _)| f == "src/governed.rs").expect("governed.rs");
    gc_caller[governed].1.push_str("\nfn u22(c: &GovernedControl, id: GrantId) {\n    let _ = c.revoke_grant(id);\n}\n");
    let rows = |event: &str| tree.register.lines().filter(|l| l.contains(&format!("\tw1\t{event}\t"))).count();
    let mut misses = Vec::new();
    for (plant, sources, gc, guard, want) in [
        ("a call", at("let _ = mission.session.stop_all();"), &tree.gc, "w1_stop_all", "W1 stop_all: unlisted host/src/hands.rs|run|stop_all"),
        ("a method value", at("let _ = missions.iter().map(FileSession::stop_all);"), &tree.gc, "w1_stop_all", "W1 stop_all: unlisted host/src/hands.rs|run|stop_all"),
        ("a use alias", top("use nexus_mother::coordinator::{stop_all as halt_everything};\n"), &tree.gc, "w1_stop_all", "W1 stop_all: unlisted host/src/hands.rs|-|stop_all"),
        ("a raw identifier", at("let _ = mission.session.r#stop_all();"), &tree.gc, "w1_stop_all", "W1 stop_all: unlisted host/src/hands.rs|run|stop_all"),
        ("a UFCS mission stop", at("let _ = FileSession::stop(&mission.session, id);"), &tree.gc, "w1_mission_stop", "W1 mission_stop: unlisted host/src/hands.rs|run|stop"),
        ("a method-value mission stop", at("let halt = FileSession::stop;"), &tree.gc, "w1_mission_stop", "W1 mission_stop: unlisted host/src/hands.rs|run|stop"),
        ("a host grant revocation", at("let _ = control.revoke_grant(grant);"), &tree.gc, "w1_grant_revocation", "W1 grant_revocation: unlisted host/src/hands.rs|run|revoke_grant"),
        ("a governed-control caller", s.clone(), &gc_caller, "w1_grant_revocation", "W1 grant_revocation: unlisted gc:src/governed.rs|u22|revoke_grant"),
    ] {
        let by_guard = w1_findings(&sources, gc);
        killed(&mut misses, plant, &by_guard[guard], &[want]);
        if by_guard.iter().any(|(g, found)| *g != guard && !found.is_empty()) {
            misses.push(format!("{plant}: another event's guard fired: {by_guard:?}"));
        }
        let event = guard.trim_start_matches("w1_");
        let joined = tree.with(by_guard);
        let reopened = joined.iter().filter(|f| f.contains("becomes set B")).count();
        if reopened != rows(event) || reopened == 0 {
            misses.push(format!("{plant}: {reopened} W1 rows reopened, not {}", rows(event)));
        }
    }
    let mut test_file = s.clone();
    test_file.push(("host/tests/u22.rs".to_string(), call.to_string()));
    for (plant, sources) in [
        ("a comment", at("// mission.session.stop_all();")),
        ("a string", at("let _ = \"mission.session.stop_all()\";")),
        ("a cfg(test) fn", top(&format!("#[cfg(test)]\n{call}"))),
        ("a cfg(test) module", top(&format!("#[cfg(test)]\nmod u22 {{\n{call}}}\n"))),
        ("an integration test", test_file),
    ] {
        let found: Vec<String> = w1_findings(&sources, &tree.gc).into_values().flatten().collect();
        if !found.is_empty() {
            misses.push(format!("{plant} counted as a production caller: {found:?}"));
        }
    }
    fail_on(misses);
}
/// The join fails on a duplicate, missing, unknown, expired or reopened
/// entry, each by its own finding.
#[test]
#[rustfmt::skip]
fn sb3e_u22_join_controls() {
    let tree = Tree::read();
    let (base, r, s) = (tree.found(), &tree.register, &tree.sources);
    let s2 = "r1/identity_view/ingress/CR\towed\ts2\tmodel_path\tR255.2\n";
    let hold = "r9/close_report/ingress/CR\tresolved\ttr1_hold\torder\tR197\n";
    let w1 = "r1/identity_view/ingress/stop_all\tw1\tstop_all\tw1_stop_all\tR196.1\n";
    let swap = |from: &str, to: &str| tree.on(&r.replacen(from, to, 1));
    let then = "let model = (cap > 0).then(|| self.port.model_identity()).flatten();";
    let dispatching = planted(s, "src/coordinator.rs", then, "let model = self.port.model_identity();");
    let stop = "    let reader_done = terminal.stop_reader(producers_until);\n";
    let late = planted(s, "host/src/hands.rs", stop, "    let reader_done = true;\n");
    let reordered = Facts::from([
        ("order", order_findings(text(&late, "host/src/hands.rs"))),
        ("close_callers", close_caller_findings(&late)),
    ]);
    let row = tree.expect.lines().find(|l| l.starts_with("R9\tclose_report\tingress\tCR\t")).expect("x:222");
    let unheld = tree.expect.replacen(row, &row.replace(TR1, "open"), 1);
    let on_table = |expect: &str, manifest: &str| activation_findings(expect, manifest, r, &tree.facts).0;
    let uncredited = tree.manifest.replacen("    r5 call_chat ingress quit;\n", "", 1);
    let mut misses = Vec::new();
    for (plant, found, want) in [
        ("duplicate entry", tree.on(&format!("{r}{s2}")), "duplicate entry: r1/identity_view/ingress/CR (x:286)"),
        ("duplicate: credited and entered", tree.on(&format!("{r}r5/call_chat/ingress/quit\tresolved\ttr1_hold\torder\tR197\n")), "duplicate: r5/call_chat/ingress/quit (x:99) is credited and has an entry"),
        ("missing set-B entry", swap(hold, ""), "missing: set-B key r9/close_report/ingress/CR (x:222) has no register row"),
        ("missing W1 entry", swap(w1, ""), "missing: W1 key r1/identity_view/ingress/stop_all (x:295) has no guard row"),
        ("missing: a withdrawn credit", on_table(&tree.expect, &uncredited), "missing: set-B key r5/call_chat/ingress/quit (x:99) has no register row"),
        ("missing: an uncredited row excuses nothing", swap(s2, &s2.replace("\towed\ts2\tmodel_path\t", "\tuncredited\ts2\t-\t")), "missing: set-B key r1/identity_view/ingress/CR (x:286) is not credited, resolved or owed (uncredited: s2)"),
        ("unknown key", tree.on(&format!("{r}r9/close_report/ingress/sigint\tresolved\ttr1_hold\torder\tR197\n")), "unknown key: r9/close_report/ingress/sigint is not in the expect table"),
        ("unknown state", swap(hold, &hold.replace("\tresolved\t", "\twaived\t")), "unknown state waived at r9/close_report/ingress/CR (x:222)"),
        ("unknown guard", swap(hold, &hold.replace("\torder\t", "\tnone\t")), "unknown guard: r9/close_report/ingress/CR (x:222): tr1_hold by none, not tr1_hold by order"),
        ("unknown resolution", swap(s2, "r1/identity_view/ingress/CR\tresolved\tw3_event\tnone\tR197\n"), "unknown resolution: r1/identity_view/ingress/CR (x:286): w3_event is not reviewed for this key"),
        ("malformed entry", swap(hold, "r9/close_report/ingress/CR\tresolved\ttr1_hold\n"), "malformed entry: r9/close_report/ingress/CR\tresolved\ttr1_hold"),
        ("expired", tree.owed(&dispatching), "expired: r1/identity_view/ingress/CR (x:286): owed facts: F3 identity under a positive cap"),
        ("reopened by its guard", tree.with(reordered), "reopened: r9/close_report/ingress/CR (x:222): its guard order fails: order: the reader is not joined before hands-close spawns"),
        ("reopened in the table", on_table(&unheld, &tree.manifest), "reopened: r9/close_report/ingress/CR (x:222): status open is not a tr1_hold row's"),
        ("a failed T7 guard", tree.with(Facts::from([("t7", vec!["T7: planted".into()])])), "T7: planted"),
    ] {
        killed(&mut misses, plant, &found, &[want]);
        if found.len() <= base.len() {
            misses.push(format!("{plant}: no new finding"));
        }
    }
    fail_on(misses);
}

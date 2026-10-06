#!/usr/bin/env python3
"""Phase Three Candidate 9 mutation controls.

Each control restores one wrong behaviour that a Candidate 9 repair removed
(or adds the one input a repaired guard must refuse) and must:

- apply: every edit's anchor occurs exactly once in its file, in order (an
  edit without an anchor appends to the file);
- compile: the control's crate builds its library's unit-test target;
- run its one intended test alone (--exact) and fail it ("0 passed; 1
  failed", "<test> ... FAILED");
- fail at the intended assertion: the test's output carries the control's
  marker;
- restore: the mutated files are written back byte for byte in `finally`;
  every edited file is verified by SHA-256 against its starting content
  after each restoration, and the checkout's Git status must equal its
  status at the start.

No control edits a test file (a file whose name ends in `tests.rs`): the
tests and guards are the fixed judge. Every intended test must pass
unmutated first (the baseline), or nothing is counted.

Usage:
  c9_controls.py <checkout> <log directory> --target-dir <dir> [--only ID,...]
  c9_controls.py <checkout> --check-anchors

<checkout> must be an isolated Git checkout of the candidate (never the
worktree under review), used by nothing else while the controls run. Run
it with CI=1 and no DISPLAY (the live display tests then fail rather than
skip, and the confirmation-window test runs itself under xvfb-run); Xvfb
and xvfb-run must be installed. The target directory is dedicated to these
runs; it may be warm: every tracked file's modification time is set to now
before the baseline, so nothing in it built from other content is taken as
current. Run it alone: it mutates and restores files in the checkout.

This harness holds the Candidate 9 controls only. It is not the original
Phase Three mutation harness and does not stand in for it.
"""
import hashlib
import json
import os
import pathlib
import subprocess
import sys
import time

GC = "nexus-governed-control"
DESKTOP = "nexus-desktop-backend"
KERNEL = "nexus-kernel"

GCS = "crates/nexus-governed-control/src"
APP = "app/src-tauri/src"
EVIDENCE = f"{GCS}/authority/evidence.rs"
COMMITMENT = f"{GCS}/authority/commitment.rs"
POLICY = f"{GCS}/authority/policy.rs"
BROKER = f"{GCS}/broker.rs"
CONTROL = f"{GCS}/control.rs"
GOVERNED = f"{GCS}/governed.rs"
DISPLAY = f"{GCS}/display/mod.rs"
BROWSER = f"{GCS}/browser/mod.rs"
PROXY = f"{GCS}/browser/proxy.rs"
DESTINATION = f"{GCS}/egress/destination.rs"
CONNECTOR = f"{GCS}/connector/mod.rs"
WORLD = f"{APP}/governed_real_world.rs"
COGNITIVE = f"{APP}/commands/cognitive.rs"
AGENTS = f"{APP}/commands/agents.rs"
HIVE = f"{APP}/commands/cognitive/hive.rs"
LIB = f"{APP}/lib.rs"
ADVANCED = f"{APP}/commands/advanced.rs"
LOOP_RUNTIME = "kernel/src/cognitive/loop_runtime.rs"

ROOT = None
LOG = None
TARGET_DIR = None


def control(cid, item, required, what, crate, test, marker, edits):
    """`required` names the mission's required control it answers (None for
    an additional control); `edits` are (file, anchor or None, replacement)."""
    return dict(id=cid, item=item, required=required, what=what, crate=crate, test=test,
                marker=marker,
                edits=[dict(file=f, anchor=a, replacement=r) for (f, a, r) in edits])


# A desktop function the guard controls append (never called; it only has to
# compile and be read by the guards).
TOOLKIT_GATE = '\n#[cfg(all(target_os = "linux", feature = "tauri-runtime"))]\n#[allow(dead_code)]\n'
SYNC_COMMAND = "\n#[tauri::command]\n#[allow(dead_code)]\n"

CONTROLS = [
    # ── P-1: the R2 approval window ───────────────────────────────────────
    control(
        "NC-C9-P1-WIDE-SPACE-SHOWN", "P-1", "R2 wide-space/fake-header spoof",
        "U+3000 IDEOGRAPHIC SPACE is shown as itself again in security-display text "
        "(the padding of the demonstrated fake-header spoof)",
        GC, "authority::tests::no_wide_or_unusual_blank_is_shown_as_itself", "'\\u{3000}'",
        [(EVIDENCE, "        && (c == ' ' || !c.is_whitespace())\n",
          "        && (c == ' ' || c == '\\u{3000}' || !c.is_whitespace())\n")],
    ),
    control(
        "NC-C9-P1-HEADER-SCROLLS-AWAY", "P-1", "scroll-away of genuine target",
        "the security header is laid out inside the scrolled details (it scrolls away "
        "with them, as in Candidate 8)",
        DESKTOP,
        "governed_real_world::tests::the_confirmation_window_keeps_its_header_fixed_and_the_requests_content_marked",
        "the header",
        [(WORLD, "    scroll.add(&details);\n",
          "    let scrolled = gtk::Box::new(gtk::Orientation::Vertical, 0);\n"
          "    scrolled.pack_start(&header, false, false, 0);\n"
          "    scrolled.pack_start(&details, true, true, 0);\n"
          "    scroll.add(&scrolled);\n"),
         (WORLD, "    content.pack_start(&header, false, false, 0);\n", "")],
    ),
    # ── C8-1 / C8-2: the interrupted drag ─────────────────────────────────
    control(
        "NC-C9-C81-SAME-CLIENT", "C8-1", "same-X-client drag relaxation",
        "an interrupted drag is let go at its origin over any window of the source's X "
        "client (its resource-id base), not only the exact source window",
        GC, "display::tests::an_interrupted_drag_is_let_go_where_it_began_or_on_nothing",
        "over Own(",
        [(DISPLAY, "            let back = on_top == window && to((x, y));\n",
          "            let back = (on_top == window || (on_top != 0 && on_top >> 21 == window >> 21))\n"
          "                && to((x, y));\n")],
    ),
    control(
        "NC-C9-C82-ESCAPE-BEFORE-CORNER", "C8-2", None,
        "a focused source gets an Escape and a release over the covering window even "
        "when a bare corner is available",
        GC, "display::tests::an_interrupted_drag_is_let_go_where_it_began_or_on_nothing",
        "focused true, grabbed false",
        [(DISPLAY, "                let bare = corners\n",
          "                let bare = !server.focus_on(window)\n                    && corners\n")],
    ),
    control(
        "NC-C9-C82-ESCAPE-UNDER-GRAB", "C8-2", None,
        "the Escape is sent while another client holds a keyboard grab (it would take it)",
        GC, "display::tests::an_interrupted_drag_is_let_go_where_it_began_or_on_nothing",
        "grabbed true",
        [(DISPLAY, "                    if server.focus_on(window) && server.keyboard_free() {\n",
          "                    if server.focus_on(window) {\n")],
    ),
    # ── P-2: no authority lock across evidence ────────────────────────────
    control(
        "NC-C9-P2-START-RECORDED-UNDER-LOCK", "P-2",
        "audit/evidence call while authority lock is held",
        "a commitment's start is recorded while the commitment registry's lock is held",
        GC, "authority::tests::no_authority_lock_is_held_while_any_record_is_written",
        "an authority lock was held while a record was written",
        [(COMMITMENT, "        let started_wall_ms = now.wall_ms;\n        self.record(&record)?;\n",
          "        let started_wall_ms = now.wall_ms;\n"
          "        let held = self.entries();\n"
          "        let recorded = self.record(&record);\n"
          "        drop(held);\n"
          "        recorded?;\n")],
    ),
    control(
        "NC-C9-P2-GRANT-RECORDED-UNDER-LOCK", "P-2", None,
        "a grant's issue is recorded while the grant store's lock is held",
        GC, "authority::tests::no_authority_lock_is_held_while_any_record_is_written",
        "an authority lock was held while a record was written",
        [(POLICY, "        self.record(EvidencePhase::GrantIssued, Some(id), &scope)?;\n",
          "        let held = self.grants.lock().expect(\"grant store\");\n"
          "        let recorded = self.record(EvidencePhase::GrantIssued, Some(id), &scope);\n"
          "        drop(held);\n"
          "        recorded?;\n")],
    ),
    control(
        "NC-C9-P2-LEASE-RECORDED-UNDER-LOCK", "P-2", None,
        "a lease's issue is recorded while the lease table's lock is held",
        GC, "broker::tests::leases_are_issued_released_and_ended_with_no_lock_held_across_a_record",
        "the operation waited on a lock held across external work",
        [(BROKER, "        // Evidence first: an unrecorded lease is never issued.\n        self.record(record)?;\n",
          "        // Evidence first: an unrecorded lease is never issued.\n"
          "        let held = self.table.leases.lock().expect(\"leases\");\n"
          "        let recorded = self.record(record);\n"
          "        drop(held);\n"
          "        recorded?;\n")],
    ),
    control(
        "NC-C9-P2-DISPLAY-START-UNDER-LOCK", "P-2", None,
        "the display's start is recorded while the display's session lock is held",
        GC, "governed::tests::a_display_start_is_recorded_with_no_lock_held_and_a_stop_overtakes_it",
        "the operation waited on a lock held across external work",
        [(DISPLAY, "        admit()?;\n",
          "        let held = self.shared.session.lock().expect(\"display\");\n"
          "        let admitted = admit();\n"
          "        drop(held);\n"
          "        admitted?;\n")],
    ),
    control(
        "NC-C9-P2-STALE-RESERVATION-COMPLETES", "P-2",
        "stale evidence reservation becoming executable",
        "a start completes on a reservation that no longer holds the commitment (the "
        "token is not compared), and a start is reserved over a pending one",
        GC, "authority::tests::a_commitment_starts_exactly_once_while_its_start_is_recorded",
        "right: Err(NotAuthorized)",
        [(COMMITMENT, "                Some(entry) if !entry.reserved_by(token) || entry.state != expected => {\n",
          "                Some(entry) if entry.state != expected => {\n"),
         (COMMITMENT, "        if entry.state != CommitmentState::Authorized || entry.reserved.is_some() {\n",
          "        if entry.state != CommitmentState::Authorized {\n")],
    ),
    # ── P-3: the browser under a machine policy ───────────────────────────
    control(
        "NC-C9-P3-POLICY-FILES-IGNORED", "P-3", "Chrome managed-policy direct mode",
        "a configured machine policy (a direct-mode proxy policy) is not seen",
        GC, "browser::tests::a_machine_browser_policy_anywhere_the_browser_reads_one_refuses_it",
        "managed/proxy.json",
        [(BROWSER, "            let mut entries = std::fs::read_dir(&dir).map_err(|_| configured.clone())?;\n"
                   "            if entries.next().is_some() {\n"
                   "                return Err(configured);\n"
                   "            }\n",
          "            let _ = std::fs::read_dir(&dir);\n")],
    ),
    control(
        "NC-C9-P3-PREPARE-UNCHECKED", "P-3", None,
        "a browser session is prepared without looking for a machine policy",
        DESKTOP, "phase3_tests::p3_g9_03_the_browser_goes_out_only_while_its_session_may",
        "`no_machine_policy(&self.policies)?;` missing",
        [(BROWSER, "        no_machine_policy(&self.policies)?;\n", "")],
    ),
    control(
        "NC-C9-P3-LAUNCH-UNCHECKED", "P-3", None,
        "a browser is launched without looking for a machine policy that appeared since",
        DESKTOP, "phase3_tests::p3_g9_03_the_browser_goes_out_only_while_its_session_may",
        "`no_machine_policy(&session.policies).map_err(` missing",
        [(BROWSER, "        no_machine_policy(&session.policies).map_err(|_| {\n"
                   "            (\n"
                   "                FailureClass::Refused,\n"
                   "                format!(\"{POLICY_CONFIGURED}: the browser was not started\"),\n"
                   "            )\n"
                   "        })?;\n", "")],
    ),
    # ── P-4: this machine's network boundary ──────────────────────────────
    control(
        "NC-C9-P4-LOCAL-BOUNDARY-SKIPPED", "P-4",
        "local-interface/on-link global address acceptance",
        "public-only egress no longer checks this machine's addresses and directly "
        "connected networks",
        GC, "egress::tests::this_machines_addresses_and_link_networks_are_refused_however_public",
        '["93.184.216.34"]',
        [(DESTINATION, "    if !allow_private {\n        let local = resolver\n",
          "    if false && !allow_private {\n        let local = resolver\n")],
    ),
    control(
        "NC-C9-P4-UNREADABLE-BOUNDARY-ADMITS", "P-4", None,
        "a boundary that cannot be read counts as an empty one (public-only egress admits)",
        GC, "egress::tests::this_machines_addresses_and_link_networks_are_refused_however_public",
        "unwrap_err()` on an `Ok` value",
        [(DESTINATION, "            .map_err(|_| DestinationError::NoLocalBoundary)?;\n",
          "            .unwrap_or(LocalNetworks(Vec::new()));\n")],
    ),
    # ── P-5: connector destination identity ───────────────────────────────
    control(
        "NC-C9-P5-DESTINATION-NOT-RECHECKED", "P-5", "connector destination metadata mismatch",
        "a post is sent although its destination's identity changed since approval",
        GC, "connector::tests::a_destination_that_changed_after_approval_fails_the_post_and_sends_nothing",
        "execute(view.id, &h.agent, h.run).is_err()",
        [(CONNECTOR, "        if now != identity {\n", "        if now != identity && now.display.is_empty() {\n")],
    ),
    # ── P-6: the toolkit's mechanisms ─────────────────────────────────────
    control(
        "NC-C9-P6-GLIB-SPAWN", "P-6", "gtk/glib spawn",
        "a desktop function launches a process through the toolkit's glib",
        DESKTOP, "phase3_tests::p3_g6_08_every_real_world_mechanism_in_the_workspace_is_classified",
        "SITE app/src-tauri/src/commands/advanced.rs process",
        [(ADVANCED, None, TOOLKIT_GATE +
          'pub(crate) fn c9_control() {\n'
          '    let _ = gtk::glib::spawn_command_line_async("xdg-open https://example.invalid");\n'
          '}\n')],
    ),
    control(
        "NC-C9-P6-GIO-SUBPROCESS", "P-6", "gio subprocess/opener",
        "a desktop function launches a process through gio's Subprocess",
        DESKTOP, "phase3_tests::p3_g6_08_every_real_world_mechanism_in_the_workspace_is_classified",
        "SITE app/src-tauri/src/commands/advanced.rs process",
        [(ADVANCED, None, TOOLKIT_GATE +
          'pub(crate) fn c9_control() {\n'
          '    let _ = gtk::gio::Subprocess::newv(\n'
          '        &[std::ffi::OsStr::new("true")],\n'
          '        gtk::gio::SubprocessFlags::NONE,\n'
          '    );\n'
          '}\n')],
    ),
    control(
        "NC-C9-P6-GIO-OPENER", "P-6", "gio subprocess/opener",
        "a desktop function opens a URI with the system's default handler through gio",
        DESKTOP, "phase3_tests::p3_g6_08_every_real_world_mechanism_in_the_workspace_is_classified",
        "SITE app/src-tauri/src/commands/advanced.rs process",
        [(ADVANCED, None, TOOLKIT_GATE +
          'pub(crate) fn c9_control() {\n'
          '    let _ = gtk::gio::AppInfo::launch_default_for_uri(\n'
          '        "https://example.invalid",\n'
          '        None::<&gtk::gio::AppLaunchContext>,\n'
          '    );\n'
          '}\n')],
    ),
    control(
        "NC-C9-P6-SHOW-URI", "P-6", None,
        "a desktop function opens a URI through gtk's show_uri_on_window",
        DESKTOP, "phase3_tests::p3_g6_08_every_real_world_mechanism_in_the_workspace_is_classified",
        "SITE app/src-tauri/src/commands/advanced.rs process",
        [(ADVANCED, None, TOOLKIT_GATE +
          'pub(crate) fn c9_control() {\n'
          '    let _ = gtk::show_uri_on_window(None::<&gtk::Window>, "https://example.invalid", 0);\n'
          '}\n')],
    ),
    control(
        "NC-C9-P6-ROOT-WINDOW", "P-6", None,
        "a desktop function takes the owner's root window (screen capture)",
        DESKTOP, "phase3_tests::p3_g6_08_every_real_world_mechanism_in_the_workspace_is_classified",
        "SITE app/src-tauri/src/commands/advanced.rs device",
        [(ADVANCED, None, TOOLKIT_GATE +
          'pub(crate) fn c9_control() {\n'
          '    use gtk::prelude::*;\n'
          '    let _ = gtk::gdk::Window::default_root_window();\n'
          '}\n')],
    ),
    control(
        "NC-C9-P6-UNCLASSIFIED-WIDGET", "P-6", None,
        "a desktop function uses a toolkit API outside the approval window's own",
        DESKTOP, "phase3_tests::p3_g6_08_every_real_world_mechanism_in_the_workspace_is_classified",
        "SITE app/src-tauri/src/commands/advanced.rs toolkit",
        [(ADVANCED, None, TOOLKIT_GATE +
          'pub(crate) fn c9_control() {\n'
          '    let _ = gtk::Entry::new();\n'
          '}\n')],
    ),
    # ── P-6b: the interface-thread lock guard ─────────────────────────────
    control(
        "NC-C9-P6B-TYPE-PATH", "P-6b", "lock call by type path",
        "a sync command reaches the loops' lock through a type-path call",
        DESKTOP, "phase3_tests::p3_g10_01_no_interface_thread_command_waits_on_the_loops_lock",
        '"c9_control"',
        [(ADVANCED, None, SYNC_COMMAND +
          "pub(crate) fn c9_control(state: tauri::State<'_, crate::AppState>) -> bool {\n"
          '    nexus_kernel::cognitive::CognitiveRuntime::get_agent_status(&state.cognitive_runtime, "x")\n'
          "        .is_some()\n"
          "}\n")],
    ),
    control(
        "NC-C9-P6B-RECEIVER-ALIAS", "P-6b", "lock call through receiver alias",
        "a sync command reaches the loops' lock through a renamed receiver",
        DESKTOP, "phase3_tests::p3_g10_01_no_interface_thread_command_waits_on_the_loops_lock",
        '"c9_control"',
        [(ADVANCED, None, SYNC_COMMAND +
          "pub(crate) fn c9_control(state: tauri::State<'_, crate::AppState>) -> bool {\n"
          "    let rt = &state.cognitive_runtime;\n"
          '    rt.get_agent_status("x").is_some()\n'
          "}\n")],
    ),
    control(
        "NC-C9-P6B-TYPE-ALIAS", "P-6b", None,
        "a sync command reaches the loops' lock through an imported alias of the type",
        DESKTOP, "phase3_tests::p3_g10_01_no_interface_thread_command_waits_on_the_loops_lock",
        '"c9_control"',
        [(ADVANCED, None,
          "\nuse nexus_kernel::cognitive::CognitiveRuntime as C9Runtime;\n" + SYNC_COMMAND +
          "pub(crate) fn c9_control(state: tauri::State<'_, crate::AppState>) -> bool {\n"
          '    C9Runtime::get_agent_status(&state.cognitive_runtime, "x").is_some()\n'
          "}\n")],
    ),
    control(
        "NC-C9-P6B-METHOD-VALUE", "P-6b", None,
        "a sync command reaches the loops' lock through the method as a value",
        DESKTOP, "phase3_tests::p3_g10_01_no_interface_thread_command_waits_on_the_loops_lock",
        '"c9_control"',
        [(ADVANCED, None, SYNC_COMMAND +
          "pub(crate) fn c9_control(state: tauri::State<'_, crate::AppState>) -> bool {\n"
          "    let status = nexus_kernel::cognitive::CognitiveRuntime::get_agent_status;\n"
          '    status(&state.cognitive_runtime, "x").is_some()\n'
          "}\n")],
    ),
    # ── C8-3: a missing agent; clearing all agents ────────────────────────
    control(
        "NC-C9-C83-MISSING-AGENT-RUNS", "C8-3", "missing-agent fail-open",
        "an agent the supervisor holds no record of counts as running again",
        DESKTOP,
        "commands::cognitive::stop_tests::an_agent_the_supervisor_does_not_hold_has_no_authority_to_run",
        "a missing agent has authority to run",
        [(COGNITIVE, "    Uuid::parse_str(agent_id).ok().is_none_or(|id| {\n",
          "    Uuid::parse_str(agent_id).is_ok_and(|id| {\n"),
         (COGNITIVE, "            .get_agent(id)\n            .is_none_or(|handle| {\n",
          "            .get_agent(id)\n            .is_some_and(|handle| {\n")],
    ),
    control(
        "NC-C9-C83-CLEARED-LOOP-CYCLES", "C8-3", None,
        "the loop of a cleared agent runs a cycle (a missing agent counts as running)",
        DESKTOP,
        "commands::cognitive::stop_tests::a_cleared_agents_loop_runs_no_cycle_and_it_takes_no_subtask",
        "the loop of an agent with no authority to run ran a cycle",
        [(COGNITIVE, "    Uuid::parse_str(agent_id).ok().is_none_or(|id| {\n",
          "    Uuid::parse_str(agent_id).is_ok_and(|id| {\n"),
         (COGNITIVE, "            .get_agent(id)\n            .is_none_or(|handle| {\n",
          "            .get_agent(id)\n            .is_some_and(|handle| {\n")],
    ),
    control(
        "NC-C9-C83-CLEAR-ORPHANS", "C8-3", "clear_all_agents orphaning a loop",
        "clearing all agents removes their records without stopping them first",
        DESKTOP, "governed_real_world::tests::clearing_all_agents_stops_each_one_everywhere_first",
        "a cleared agent's",
        [(AGENTS, "    let _ = stop_agents(state, &known_agents(state));\n",
          "    let _ = known_agents(state);\n")],
    ),
    control(
        "NC-C9-C83-UNION-MISSES-LOOPS", "C8-3", None,
        "the agents cleared are not looked for among the runtime's loops",
        DESKTOP, "governed_real_world::tests::clearing_all_agents_stops_each_one_everywhere_first",
        "a cleared agent's loop was left",
        [(AGENTS, "    agents.extend(state.cognitive_runtime.loop_agents());\n", "")],
    ),
    control(
        "NC-C9-C83-SPELLINGS-MISS-LOOP-KEY", "C8-3", None,
        "a loop kept under another spelling of the agent's id is not found by the stop",
        DESKTOP, "governed_real_world::tests::clearing_all_agents_stops_each_one_everywhere_first",
        "a cleared agent's loop was left",
        [(AGENTS, "    for key in scheduled.chain(looping).chain(loops) {\n",
          "    let _ = loops;\n    for key in scheduled.chain(looping) {\n")],
    ),
    control(
        "NC-C9-C83-UNION-MISSES-SCHEDULES", "C8-3", None,
        "the agents cleared are not looked for among the schedules",
        DESKTOP, "governed_real_world::tests::clearing_all_agents_stops_each_one_everywhere_first",
        "a cleared agent's schedule was left",
        [(AGENTS, "    agents.extend(state.agent_scheduler.list().into_iter().map(|s| s.agent_id));\n", "")],
    ),
    control(
        "NC-C9-C83-UNION-MISSES-DRIVERS", "C8-3", None,
        "the agents cleared are not looked for among the loops' drivers",
        DESKTOP, "governed_real_world::tests::clearing_all_agents_stops_each_one_everywhere_first",
        "loop driver was not told to stop",
        [(AGENTS, "    agents.extend(drivers);\n", "    let _ = drivers;\n")],
    ),
    control(
        "NC-C9-C83-UNION-MISSES-PHASE-THREE", "C8-3", None,
        "the agents cleared are not looked for among Phase Three's runs",
        DESKTOP, "governed_real_world::tests::clearing_all_agents_stops_each_one_everywhere_first",
        "a cleared agent's waiting action stayed open",
        [(AGENTS, "        agents.extend(world.agents_with_runs());\n", "        let _ = world;\n")],
    ),
    # ── C8-4: a goal's own cleanup ────────────────────────────────────────
    control(
        "NC-C9-C84-COMPARED-BEFORE-THE-GUARD", "C8-4", "end_goal_loop TOCTOU",
        "the kernel compares the goal from the lock-free snapshot before it waits for the "
        "loops' guard (Candidate 8's check-then-act)",
        KERNEL, "cognitive::loop_runtime::tests::an_older_goals_cleanup_never_ends_a_newer_goal",
        "an older goal's cleanup ended a newer goal",
        [(LOOP_RUNTIME,
          "        waiting();\n"
          "        let mut loops = self.loops.lock().unwrap_or_else(|p| p.into_inner());\n"
          "        // No loop, or one for another goal: nothing is ended.\n"
          "        if loops\n"
          "            .get(agent_id)\n"
          "            .is_none_or(|state| state.goal.id != goal_id)\n"
          "        {\n",
          "        let ours = self\n"
          "            .get_agent_status_fast(agent_id)\n"
          "            .and_then(|status| status.active_goal)\n"
          "            .is_some_and(|goal| goal.id == goal_id);\n"
          "        waiting();\n"
          "        let mut loops = self.loops.lock().unwrap_or_else(|p| p.into_inner());\n"
          "        if !ours {\n")],
    ),
    control(
        "NC-C9-C84-DESKTOP-CHECK-THEN-ACT", "C8-4", None,
        "end_goal_loop goes back to Candidate 8's snapshot check then a removal by agent",
        DESKTOP, "phase3_tests::p3_g6_11_the_owners_stops_reach_phase_three",
        "end_goal_loop",
        [(COGNITIVE,
          "    if state\n"
          "        .cognitive_runtime\n"
          "        .stop_agent_loop_if(agent_id, goal_id)\n"
          "    {\n",
          "    let ours = state\n"
          "        .cognitive_runtime\n"
          "        .get_agent_status_fast(agent_id)\n"
          "        .and_then(|status| status.active_goal)\n"
          "        .is_some_and(|goal| goal.id == goal_id);\n"
          "    if ours && state.cognitive_runtime.stop_agent_loop(agent_id).is_ok() {\n")],
    ),
    control(
        "NC-C9-C84-AGENT-WIDE-WAKE", "C8-4", None,
        "a goal's cleanup wakes whichever consent wait the agent has, not only its goal's",
        DESKTOP,
        "commands::cognitive::stop_tests::a_goals_cleanup_ends_only_that_goal_and_wakes_only_its_wait",
        "woke another goal's consent wait",
        [(COGNITIVE, "        state.wake_and_clear_goal_consent_wait(agent_id, goal_id);\n",
          "        state.wake_and_clear_blocked_consent_wait(agent_id);\n")],
    ),
    # ── C8-5: HiveMind sessions ───────────────────────────────────────────
    control(
        "NC-C9-C85-UNLIMITED-STARTS", "C8-5", "unlimited HiveMind starts",
        "HiveMind sessions are admitted without a cap",
        DESKTOP,
        "commands::cognitive::hive_tests::at_most_the_cap_runs_at_once_and_an_ended_session_gives_its_place_back",
        "beyond the cap",
        [(HIVE, "        if table.live.len() >= MAX_SESSIONS {\n",
          "        if table.live.len() >= usize::MAX {\n")],
    ),
    control(
        "NC-C9-C85-RACING-STARTS", "C8-5", None,
        "racing HiveMind starts are admitted without a cap",
        DESKTOP, "commands::cognitive::hive_tests::racing_starts_never_exceed_the_cap",
        "racing starts ran more sessions than the cap",
        [(HIVE, "        if table.live.len() >= MAX_SESSIONS {\n",
          "        if table.live.len() >= usize::MAX {\n")],
    ),
    control(
        "NC-C9-C85-PLACE-NOT-GIVEN-BACK", "C8-5", None,
        "a session that ends keeps its place (no RAII release)",
        DESKTOP,
        "commands::cognitive::hive_tests::at_most_the_cap_runs_at_once_and_an_ended_session_gives_its_place_back",
        "left: 2",
        [(HIVE, "        self.sessions.table().live.remove(&self.id);\n",
          "        let _ = &self.sessions;\n")],
    ),
    control(
        "NC-C9-C85-NOT-RATE-LIMITED", "C8-5", None,
        "a HiveMind start is admitted past the exhausted agent-execution rate limit",
        DESKTOP,
        "commands::cognitive::hive_tests::a_start_is_rate_limited_and_a_refused_one_holds_no_place",
        "passed the exhausted agent-execution rate limit",
        [(COGNITIVE, "    state.check_rate(nexus_kernel::rate_limit::RateCategory::AgentExecute)?;\n"
                     "    state.log_event(\n        SYSTEM_UUID,\n        EventType::StateChange,\n"
                     "        json!({\"action\": \"hivemind_session_admitted\"",
          "    state.log_event(\n        SYSTEM_UUID,\n        EventType::StateChange,\n"
          "        json!({\"action\": \"hivemind_session_admitted\"")],
    ),
    control(
        "NC-C9-C85-CANCELLED-SESSION-ASSIGNS", "C8-5", None,
        "a cancelled session assigns a further sub-task",
        DESKTOP, "commands::cognitive::hive_tests::a_cancelled_session_assigns_no_further_subtask",
        "a cancelled session assigned a sub-task",
        [(COGNITIVE, "    if session.cancelled() {\n        return Err(hive::SESSION_CANCELLED.to_string());\n"
                     "    }\n    // A stopped agent takes no sub-task",
          "    // A stopped agent takes no sub-task")],
    ),
    control(
        "NC-C9-C85-WAIT-IGNORES-CANCEL", "C8-5", None,
        "a sub-task a cancelled session waits on runs on",
        DESKTOP,
        "commands::cognitive::hive_tests::a_cancelled_sessions_waiting_subtask_ends_only_its_own_goal",
        "sub-task timed out",
        [(COGNITIVE, "    loop {\n        if session.cancelled() {\n            end_goal_loop(state, agent_id, goal_id);\n"
                     "            return Err(hive::SESSION_CANCELLED.to_string());\n        }\n",
          "    loop {\n")],
    ),
    control(
        "NC-C9-C85-OWNER-CANCEL-UNWIRED", "C8-5", None,
        "the owner's cancel_hivemind does not reach a session under way",
        DESKTOP,
        "commands::cognitive::hive_tests::the_owner_the_emergency_stop_and_quitting_signal_sessions",
        "the owner's cancellation did not reach it",
        [(COGNITIVE, "    if state.hive_sessions.cancel(&session_id) {\n",
          "    if session_id.is_empty() && state.hive_sessions.cancel(&session_id) {\n")],
    ),
    control(
        "NC-C9-C85-QUIT-ADMITS", "C8-5", None,
        "a session starts while the desktop quits",
        DESKTOP,
        "commands::cognitive::hive_tests::the_owner_the_emergency_stop_and_quitting_signal_sessions",
        "a session started while the desktop quits",
        [(HIVE, "        table.quitting = true;\n", "")],
    ),
    control(
        "NC-C9-C85-KILL-SWITCH-UNWIRED", "C8-5", None,
        "the emergency key does not signal HiveMind sessions",
        DESKTOP, "phase3_tests::p3_c9_hivemind_sessions_are_bounded_and_signalled",
        "the emergency key does not stop HiveMind sessions",
        [(LIB, "                        state.hive_sessions.cancel_all();\n", "")],
    ),
    control(
        "NC-C9-C85-P3-STOP-UNWIRED", "C8-5", None,
        "Phase Three's emergency stop command does not signal HiveMind sessions",
        DESKTOP, "phase3_tests::p3_c9_hivemind_sessions_are_bounded_and_signalled",
        "the Phase Three emergency stop does not stop HiveMind sessions",
        [(WORLD, "        state.inner().hive_sessions.cancel_all();\n", "")],
    ),
    control(
        "NC-C9-C85-QUIT-UNWIRED", "C8-5", None,
        "quitting does not close HiveMind sessions",
        DESKTOP, "phase3_tests::p3_c9_hivemind_sessions_are_bounded_and_signalled",
        "quitting does not close HiveMind sessions",
        [(LIB, "                    app.state::<AppState>().hive_sessions.close();\n", "")],
    ),
    # ── P-7a: the owner's reserved places ─────────────────────────────────
    control(
        "NC-C9-P7A-NAME-GETS-OWNER-PLACES", "P-7a", "owner-session string capacity escalation",
        "an agent named like the owner gets the owner's reserved places",
        GC, "control::tests::agents_cannot_fill_the_places_kept_for_the_owner",
        "unwrap_err()` on an `Ok` value",
        [(CONTROL, "            Some(RunClass::Agent) => MAX_PENDING - OWNER_RESERVE,\n",
          "            Some(RunClass::Agent) if *agent == AgentId::owner_session() => MAX_PENDING,\n"
          "            Some(RunClass::Agent) => MAX_PENDING - OWNER_RESERVE,\n")],
    ),
    control(
        "NC-C9-P7A-AGENT-IN-OWNER-RUN", "P-7a", None,
        "an agent's action runs in an owner run",
        GC, "governed::tests::an_agent_action_never_runs_in_an_owner_run",
        "unwrap_err()` on an `Ok` value",
        [(GOVERNED, "        if self.authority().runs().class(run) != Some(RunClass::Agent) {\n",
          "        if self.authority().runs().class(run).is_none() {\n")],
    ),
    # ── P-7k / P-7l: the production browser's address policy and stop ─────
    control(
        "NC-C9-P7K-PRIVATE-FLIP", "P-7k", "private-browser-policy flip",
        "the production browser admits private destinations",
        GC, "browser::tests::a_production_browser_session_reaches_no_private_address",
        "CONNECT 127.0.0.1",
        [(BROWSER, "            allow_private: false,\n            policies: POLICY_ROOTS",
          "            allow_private: true,\n            policies: POLICY_ROOTS")],
    ),
    control(
        "NC-C9-P7L-STOP-FLAG-IGNORED", "P-7l", "proxy stop-flag removal",
        "the proxy's connections ignore the proxy's own stop (only the grant's liveness)",
        GC, "browser::tests::an_open_tunnel_ends_with_its_session_within_the_poll_bound",
        "the tunnel outlived its session",
        [(PROXY, "    !stop.load(Ordering::SeqCst) && (policy.live)()\n",
          "    let _ = stop;\n    (policy.live)()\n")],
    ),
]


def sha256(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def mutable_files():
    return sorted({e["file"] for c in CONTROLS for e in c["edits"]})


def state():
    return {path: sha256(ROOT / path) for path in mutable_files()}


def status():
    out = subprocess.run(["git", "status", "--porcelain=v1", "--untracked-files=all"],
                         cwd=ROOT, capture_output=True, text=True, check=True).stdout
    return sorted(line for line in out.splitlines() if line)


def refresh_mtimes():
    """Set every tracked file's modification time to now (content
    untouched), so that no artifact a warm target directory holds, built
    from other content, is taken as current: every workspace crate is
    built from this checkout at least once."""
    out = subprocess.run(["git", "ls-files", "-z"], cwd=ROOT, capture_output=True,
                         check=True).stdout
    now = time.time()
    for name in out.split(b"\0"):
        path = ROOT / os.fsdecode(name)
        if name and path.is_file() and not path.is_symlink():
            os.utime(path, (now, now))


def mutate(control_, originals):
    texts = {}
    for edit in control_["edits"]:
        if edit["file"].endswith("tests.rs"):
            raise SystemExit(f"{control_['id']}: edits a test file: {edit['file']}")
        text = texts.get(edit["file"], originals[edit["file"]].decode())
        if edit["anchor"] is None:
            if not text.endswith("\n"):
                raise SystemExit(f"{control_['id']}: {edit['file']} does not end with a newline")
            texts[edit["file"]] = text + edit["replacement"]
            continue
        count = text.count(edit["anchor"])
        if count != 1:
            raise SystemExit(f"{control_['id']}: anchor occurs {count} times in {edit['file']}: "
                             f"{edit['anchor'][:80]!r}")
        texts[edit["file"]] = text.replace(edit["anchor"], edit["replacement"], 1)
    return texts


def run_test(crate, name):
    env = dict(os.environ, CARGO_TARGET_DIR=str(TARGET_DIR))
    return subprocess.run(
        ["cargo", "test", "-p", crate, "--locked", "--lib", "--", name, "--exact"],
        cwd=ROOT, capture_output=True, text=True, timeout=3600, env=env)


def compiled(output):
    return ("could not compile" not in output and "error[E" not in output
            and "Running unittests src/lib.rs" in output)


def run_one(control_, originals, original_state, expected_status):
    texts = mutate(control_, originals)
    if state() != original_state:
        raise SystemExit(f"{control_['id']}: files changed before the control")
    try:
        for path, text in texts.items():
            (ROOT / path).write_bytes(text.encode())
        proc = run_test(control_["crate"], control_["test"])
    finally:
        for path in texts:
            (ROOT / path).write_bytes(originals[path])
    restored = state() == original_state
    if status() != expected_status:
        raise SystemExit(f"{control_['id']}: unexpected checkout state after restoring: {status()}")
    output = proc.stdout + proc.stderr
    name = control_["test"]
    failed = (proc.returncode != 0 and "test result: FAILED. 0 passed; 1 failed" in output
              and f"{name} ... FAILED" in output)
    return output, compiled(output), failed, restored, proc.returncode


def baseline(original_state):
    results = {}
    for crate, name in sorted({(c["crate"], c["test"]) for c in CONTROLS}):
        proc = run_test(crate, name)
        output = proc.stdout + proc.stderr
        results[f"{crate} {name}"] = (proc.returncode == 0
                                      and "test result: ok. 1 passed; 0 failed" in output)
        (LOG / f"baseline--{crate}--{name.replace('::', '__')}.log").write_text(output)
    if state() != original_state:
        raise SystemExit("files changed during the baseline")
    return results


def check_anchors():
    originals = {path: (ROOT / path).read_bytes() for path in mutable_files()}
    ids = [c["id"] for c in CONTROLS]
    if len(ids) != len(set(ids)):
        raise SystemExit("duplicate control ids")
    for c in CONTROLS:
        mutate(c, originals)
    return len(CONTROLS)


def main():
    global ROOT, LOG, TARGET_DIR
    args = sys.argv[1:]
    if len(args) == 2 and args[1] == "--check-anchors":
        ROOT = pathlib.Path(args[0]).resolve()
        print(json.dumps({"controls": check_anchors(), "anchors": "ok"}))
        return 0
    if "--target-dir" not in args:
        raise SystemExit(__doc__)
    at = args.index("--target-dir")
    TARGET_DIR = pathlib.Path(args[at + 1]).resolve()
    del args[at:at + 2]
    only = None
    if "--only" in args:
        at = args.index("--only")
        only = set(args[at + 1].split(","))
        del args[at:at + 2]
    if len(args) != 2:
        raise SystemExit(__doc__)
    ROOT = pathlib.Path(args[0]).resolve()
    LOG = pathlib.Path(args[1]).resolve()
    LOG.mkdir(parents=True, exist_ok=True)
    TARGET_DIR.mkdir(parents=True, exist_ok=True)
    if os.environ.get("CI") is None or os.environ.get("DISPLAY") is not None:
        raise SystemExit("run with CI set and DISPLAY unset (see the usage)")
    check_anchors()
    head = subprocess.run(["git", "rev-parse", "HEAD", "HEAD^{tree}"], cwd=ROOT,
                          capture_output=True, text=True, check=True).stdout.split()
    script = sha256(pathlib.Path(__file__).resolve())
    refresh_mtimes()
    original_state = state()
    originals = {path: (ROOT / path).read_bytes() for path in mutable_files()}
    expected_status = status()
    passing = baseline(original_state)
    results = []
    for c in CONTROLS:
        if only is not None and c["id"] not in only:
            continue
        output, built, failed, restored, code = run_one(c, originals, original_state,
                                                       expected_status)
        (LOG / f"{c['id']}.log").write_text(output)
        marked = c["marker"] in output
        unmutated = passing[f"{c['crate']} {c['test']}"]
        killed = unmutated and built and failed and marked
        result = dict(id=c["id"], item=c["item"], required=c["required"], what=c["what"],
                      files=sorted({e["file"] for e in c["edits"]}), crate=c["crate"],
                      test=c["test"], marker=c["marker"], test_passes_unmutated=unmutated,
                      compiled=built, failed_intended_test=failed, marker_seen=marked,
                      files_restored=restored, exit=code, killed=killed,
                      ok=killed and restored)
        results.append(result)
        print(json.dumps(result), flush=True)
    final_state = state()
    required = sorted({c["required"] for c in CONTROLS if c["required"]})
    summary = dict(
        head=head[0], tree=head[1], script_sha256=script,
        complete=only is None,
        controls=len(results),
        killed=sum(1 for r in results if r["killed"]),
        survived_or_invalid=[r["id"] for r in results if not r["killed"]],
        required_controls_answered={req: [r["id"] for r in results if r["required"] == req]
                                    for req in required},
        by_item={item: sum(1 for r in results if r["item"] == item)
                 for item in sorted({r["item"] for r in results})},
        baseline_all_pass=all(passing.values()),
        files_identical=final_state == original_state,
        status_unchanged=status() == expected_status,
        original=original_state, final=final_state,
        all_ok=all(r["ok"] for r in results),
    )
    print(json.dumps(summary), flush=True)
    (LOG / "summary.json").write_text(
        json.dumps(dict(summary=summary, baseline=passing, counted=results), indent=2) + "\n")
    ok = (summary["complete"] and summary["baseline_all_pass"] and summary["files_identical"]
          and summary["status_unchanged"] and summary["all_ok"]
          and all(summary["required_controls_answered"].values()))
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())

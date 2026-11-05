"""Expand reviewed structural dispositions; no production output decides expectations.

Run from any directory after updating the checked-in successful execution receipts.
The Rust checker independently generates the required key set. Never infer success
from a missing receipt: both the generator and checker reject missing references.
"""
from pathlib import Path
import csv
import itertools
import re
from functools import cache
from collections import Counter

ROOT = Path(__file__).resolve().parent

def read(name):
    return list(csv.DictReader((ROOT / name).open(), delimiter="\t"))

receipts = read("combination-receipts.tsv")
@cache
def proof(fragment):
    found = [r for r in receipts if fragment in r["observed"]]
    if not found:
        raise ValueError("missing successful execution: " + fragment)
    return found[0]["receipt"]

pauses = "Accepted Captured BeforeDialogue AfterDialogue BeforeSlow AfterSlow BeforeHold Reserved BeforeAdmission AfterAdmission KernelEntry PublicationObservation PhysicalEffect Successor Transfer CloseAssessment Quiescence Seal EndRecord Unwind".split()
ingresses = "CR LF CRLF original_cell_unscheduled repeated EOF partial_Ctrl_D master_close read_failure mission_stop stop_all door_fence budget grant_revocation grant_expiry quit fatal".split()
failures = "none decline record_failed record_unknown poison panic".split()
after = set("AfterAdmission KernelEntry PublicationObservation PhysicalEffect Successor Transfer Unwind".split())
dialogues = set("grant_confirm door_confirm mission_confirm action_confirm undo_confirm".split())
native = set("project_select read_grant run_construct staging_grant snapshot edit structural_verify page_verify review revalidation_review request_approval page_recheck write_grant apply undo_request undo_grant restore retry_cleanup discard_staging housekeeping_sync".split())
private = {"private_" + p for p in "mkdir temp_create write sync rename parent_sync error_remove".split()}
removed = {"obsolete_undo_cleanup", "undo_prior_cleanup"}
# These sinks have no own durable append. A failing predecessor record is tested
# at its own named sink and cannot be re-labelled as a failure of this mutation.
in_memory = set("hold_configure workspace_install grant_install port_issue attachment_claim file_select consent_reserve consent_cache run_create run_project lane_reserve file_binding file_binding_project file_effect_select preparation_reserve preparation_validate pending_install token_issue authorize commitment_start effect_dispatch command_accept dequeue target_resolve".split())
aliases = dict(contract_draft="draft", contract_revise="revise", verify_done="verify_pass", paused_rest="paused_store", approval_pending="pending_install", undo_request="request_restore")

rules = {
 "close": ("equivalence", "The original Quit capability may finalize its fixed inventory after input quiescence and before seal, within the first shutdown deadline. It is distinct from an ordinary cleanup request. The linked B producer/retained-close oracles and E failed/held-finalizer oracle test this exact lifecycle; emergency/failure still deny new restorative handoffs and the deadline path reports an unknown in-flight snapshot."),
 "removed": ("exclusion", "No filesystem sink: the old implicit cleanup route was removed. The non-owning index transfers no cleanup authority; the linked real ownership runner and existing route bypass plants prove conservation. Explicit cleanup has its own R29 dispositions."),
 "lifecycle": ("exclusion", "Unreachable lifecycle for a new W/R sink: after seal/end-record only pre-reserved final obligations may run. These rows do not exclude resources that already exist: B retained-close and in-flight-finalizer oracles exercise refusal and sealing debt, with independent pre-seal controls."),
 "dialogue": ("exclusion", "No such dialogue at this named sink. Dialogues are separately enumerated grant_confirm, door_confirm, mission_confirm, action_confirm and undo_confirm; an earlier dialogue is preparation, followed by this sink's independently checked final boundary and its positive/negative controls."),
 "decline": ("exclusion", "No such Owner-answer primitive at this sink. Explicit decline belongs to the separately named confirmation sink. Its downstream operation is not entered after decline; the linked boundary runner proves it cannot acquire authority from a non-answer."),
 "kernel": ("exclusion", "No kernel handoff at this sink: this is a broker/Mother/terminal metadata or private-filesystem operation. Its own final hold or filesystem API boundary is independently executed. No claimed kernel receipt is substituted for that boundary."),
 "publication": ("exclusion", "No Owner publication observation at this sink. Only the entered apply or restore call can establish that physical outcome. Staging, private content, metadata and dialogue operations are independently covered at their own mutation and successor boundaries."),
 "record": ("exclusion", "No durable record primitive at this named in-memory mutation. An earlier gating record has its own sink and failure coverage; a refused/unknown append cannot confer permission on this later mutation. The linked final-boundary runner tests the actual in-memory mutation, not a fictitious append."),
 "before": ("equivalence", "Frozen-boundary equivalence: acceptance, capture, slow preparation, reservation and the before-hold/before-admission placements all precede this sink's final original-lifetime check. The same immutable scope reaches that check; no new request is minted. The linked exact sink runner includes successful controls and an original-cell ingress with no worker, including compare/owning-hold races where applicable. Separate linked ingress receipts establish the terminal-to-cell restriction."),
 "after": ("equivalence", "Frozen R-1 handoff equivalence: after this sink's admission its entered operation may finish only its documented internal work. The runner observes the actual native/filesystem/journal result and tests the next separately admitted step; it never treats a later ingress as revoking an earlier handoff. Transfer/unwind preserves inventory ownership and is separately witnessed. This is composition of executed boundaries, not a claim that every Cartesian tuple ran."),
 "record-after": ("equivalence", "Frozen-boundary record-outcome equivalence: an already-entered primitive may return an error or leave its return unknown. Outcome recording and physical effect are separate facts; an error cannot discharge the reserved resource. Linked real native/journal fault runners exercise missing/failed recording, and the exact sink runner exercises the admission/successor boundary. Unknown means no conclusive returned proof and is additionally covered by unwind/death ownership controls; it is never treated as Recorded."),
 "unwind": ("equivalence", "Frozen ownership-boundary equivalence: panic/poison is not permission or cleanup. The primitive's reserved owner stays in the inventory when the call or owning hold cannot return normally. The linked poison/unwind runner checks real resources and last-owner destruction; the exact sink runner checks its own handoff. Earlier ingress cannot discharge these obligations, and later ingress cannot introduce a successor call."),
 "prerequisite": ("exclusion", "No such grant or mission authority yet at this setup boundary: workspace/grant/door construction precedes the mission/run being created. Its own original session lifetime is still checked. Existing grants and missions are exercised only at sinks they actually constrain, with their original clocks/identity and no ambient fallback."),
}

ingress_fragments = {
 "CR":"review1_real_cr_lf_crlf_stop_during_publish_hold ... ok",
 "LF":"review1_real_cr_lf_crlf_stop_during_publish_hold ... ok",
 "CRLF":"review1_real_cr_lf_crlf_stop_during_publish_hold ... ok",
 "original_cell_unscheduled":"session_setup_each_final_boundary_has_its_own_unscheduled_ingress_control ... ok",
 "repeated":"m2dd_original_ingress_invalidates_queued_work_and_repeated_commands ... ok",
 "EOF":"d_o20_stop_ctrl_d_and_master_close_before_publication ... ok",
 "partial_Ctrl_D":"d_o20_stop_ctrl_d_and_master_close_before_publication ... ok",
 "master_close":"d_o20_stop_ctrl_d_and_master_close_before_publication ... ok",
 "read_failure":"d_o20_read_error_during_held_execution_engages_stop_before_exit ... ok",
 "quit":"quit_during_dialogue_cancels_without_waiting_for_an_answer ... ok",
 "fatal":"fix1_b_first_failure_deadline_cannot_be_refreshed_by_concurrent_quit",
 "mission_stop":"session_each_native_work_step_rechecks_its_actual_prerequisites",
 "stop_all":"session_each_native_work_step_rechecks_its_actual_prerequisites",
 "door_fence":"session_each_native_work_step_rechecks_its_actual_prerequisites",
 "budget":"fix1_original_budget_is_rechecked_under_final_run_hold",
 "grant_revocation":"PREREQUISITE/staging_grant/revoke:",
 "grant_expiry":"PREREQUISITE/staging_grant/grant_expiry:",
}
# Exact method names are resolved below against checked-in successful outputs;
# no missing candidate is silently substituted.
def matching_test(fragment):
    return proof(fragment)

mapping = read("sink-verification.tsv")
sink_rows = [s for s in read("sinks.tsv") if s["class"] in {"W", "R"}]
assert len(sink_rows) == 75
witness_rows=[]
out=[]
for s in sink_rows:
    route, sink = s["route"], s["sink"]
    m = next(m for m in mapping if m["route"] == route and m["sink"] == sink)
    tests = re.findall(r"session_[a-zA-Z0-9_]+", m["real_route_oracle"])
    test_receipts = [proof(t + " ... ok") for t in tests]
    specific = {
        "read_grant": "session_read_constructor_and_write_grant_require_individual_handoffs",
        "run_construct": "session_read_constructor_and_write_grant_require_individual_handoffs",
        "write_grant": "session_read_constructor_and_write_grant_require_individual_handoffs",
        "page_verify": "session_inert_page_check_and_recheck_are_separate_mutating_calls",
        "page_recheck": "session_inert_page_check_and_recheck_are_separate_mutating_calls",
        "revalidation_review": "session_r21_r22_call_admissions_check_native_ledger_independently",
        "request_approval": "session_r21_r22_call_admissions_check_native_ledger_independently",
        "apply": "session_r21_r22_call_admissions_check_native_ledger_independently",
        "restore": "session_r27_normal_restore_admission_and_post_return_no_discard",
        "discard_staging": "session_discard_after_normal_cleanup_reservation_rechecks_original_lifetime",
    }
    boundary = proof(specific[sink]+" ... ok") if sink in specific else test_receipts[0]
    alias=aliases.get(sink,sink)
    candidates=[r for r in receipts if ("/"+alias+"/" in r["observed"] or "."+alias+"/" in r["observed"]) and "conforms=true" in r["observed"]]
    witness_rows.append([route+":"+sink, m["final_boundary"], ",".join(dict.fromkeys(test_receipts+[r["receipt"] for r in candidates])), m["plant_receipt"]])
    for pause, ingress, failure in itertools.product(pauses,ingresses,failures):
        key=":".join([route,sink,pause,ingress,failure])
        refs=[boundary]
        rule="after" if pause in after else "before"
        disposition="equivalence"
        if sink in removed: rule="removed"
        elif pause in {"Seal","EndRecord"}: rule="lifecycle"
        elif pause in {"Quiescence","CloseAssessment"} and route == "R29":
            rule="close"
            refs += [proof("fix1_b_each_close_operation_remains_accounted_until_native_return ... ok"), proof("fix1_e_unclean_close_lists_failed_or_inflight_finalization_separately ... ok")]
        elif pause == "Quiescence": rule="lifecycle"
        elif pause in {"BeforeDialogue","AfterDialogue"} and sink not in dialogues: rule="dialogue"
        elif failure=="decline" and sink not in dialogues: rule="decline"
        elif pause=="KernelEntry" and sink not in native: rule="kernel"
        elif pause=="PublicationObservation" and sink not in {"apply","restore"}: rule="publication"
        elif failure.startswith("record_") and sink in in_memory: rule="record"
        elif ingress in {"mission_stop","stop_all","door_fence","budget","grant_revocation","grant_expiry"} and route in {"R03","R04","R05","R06","R07"}: rule="prerequisite"
        else:
            refs.append(proof(ingress_fragments[ingress]))
            if pause in after and failure.startswith("record_"):
                rule="record-after"
                # Native exact after-handoff record faults are preferred.
                exact=[r for r in candidates if "F11-NATIVE/" in r["observed"] and "record_failed=true" in r["observed"]]
                record_oracle = {
                    "grant_issue": "el1_n2_a_failed_commit_is_uncertain_and_latches",
                    "door_confirm": "m2c_a7_b_an_unrecorded_door_is_no_door",
                    "door_activate": "m2c_a7_b_an_unrecorded_door_is_no_door",
                    "mission_confirm": "m2a_a_yes_the_broker_cannot_record_gives_no_consent",
                    "action_confirm": "el1_n2_a_failed_commit_is_uncertain_and_latches",
                    "commitment_create": "el1_n2_a_failed_commit_is_uncertain_and_latches",
                    "restore": "fix1_f08_restore_completion_record_failure_keeps_physical_completion",
                    "discard_staging": "fix1_a_discard_record_failure_keeps_exact_acked_resource_debt",
                    "retry_cleanup": "fix1_f11_retry_cleanup_record_failure_retains_debt",
                    "housekeeping_sync": "session_discharge_requires_separate_sync_and_keeps_the_owner_file",
                }.get(sink, "session_r37_each_private_call_and_error_removal_has_its_own_boundary" if sink in private else "fix1_terminal_record_is_not_a_readable_prefix")
                refs.append(exact[0]["receipt"] if exact else proof(record_oracle+" ... ok"))
            if failure in {"poison","panic","record_unknown"}:
                refs.append(proof("session_poisoned_ownership_holds_preserve_resources_through_last_owner_drop ... ok"))
                refs.append(proof("session_execution_guard_unwind_retains_the_unrecorded_outcome ... ok"))
                rule="unwind" if not failure.startswith("record_") else rule
            if failure=="decline":
                refs.append(proof("f10_grant_explicit_no ... ok"))
            if candidates:
                selected=[r for r in candidates if ("/after/" in r["observed"] or "/returned/" in r["observed"]) == (pause in after)]
                refs.append((selected or candidates)[0]["receipt"])
        if rule == "lifecycle":
            refs.append(proof("fix1_b_retained_close_resources_refuse_before_native_entry_after_seal ... ok"))
        if rules[rule][0]=="exclusion": disposition="exclusion"
        # The newly registered omitted schedule is executed for both private
        # content kinds at both exact API boundaries, not replaced by a model.
        if sink in private and pause in {"BeforeHold","PhysicalEffect"} and ingress in {"EOF","quit","fatal"} and failure=="none":
            when="before" if pause=="BeforeHold" else "after"
            refs=[proof("R37-CLOSE/"+kind+"/"+sink+"/"+when+"/"+ingress+":") for kind in ["Contract","PausedPhase"]]
            disposition,rule="execution","exact"
        out.append([key,disposition,rule,",".join(dict.fromkeys(refs))])
assert len(out)==153000 and len({r[0] for r in out})==153000

def write(name,header,rows):
    (ROOT/name).write_text("\t".join(header)+"\n"+"".join("\t".join(row)+"\n" for row in rows))
write("combination-dispositions.tsv",["key","disposition","rule","receipts"],out)
write("combination-rules.tsv",["rule","disposition","justification"],[[k,*v] for k,v in rules.items()])
write("combination-boundaries.tsv",["sink","sealed_boundary","executed_runners","existing_bypass_plants"],witness_rows)
print(dict(Counter(r[1] for r in out)))

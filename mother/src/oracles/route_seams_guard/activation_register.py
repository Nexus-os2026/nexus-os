#!/usr/bin/python3 -I
"""SB-3E U22: generate the activation register (SB3E-R255.2, R260.6 F8).

  python3 -I mother/src/oracles/route_seams_guard/activation_register.py [--check]

Derives mother/host/tests/session/model-bridge-activation.tsv from the expect
table, CREDITED (session_model_bridge.rs) and the reviewed resolutions below.
One row per uncredited set-B key and per uncredited W1 key, in table order:

  key  state  reason  guard  review

  resolved    a reviewed resolution; `guard` is the source guard it rests on
  owed        owed-guarded (R255.2): legal only while guard `model_path` holds
  uncredited  neither: the activation check is red on this key until a cell
              credits it (no row can excuse it)
  w1          no production caller of its event (R196.1); guard `w1_<event>`

Deterministic: no clock, no environment, table order. `--check` writes nothing
and exits 1 when the file differs. The last line printed is the owed-set pin
(count and sha256 of the sorted owed keys, one per line) that activation.rs
carries as `OWED`. Regenerate on every head whose credits changed; the check
(activation.rs) re-derives every row's class on its own and refuses a stale row.
"""
import hashlib
import os
import re
import sys

ROOT = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "../../../.."))
TESTS = os.path.join(ROOT, "mother/host/tests")
OUT = os.path.join(TESTS, "session/model-bridge-activation.tsv")
TR1 = "hold:unreachable_by_production_order"
BYTES = ("CR", "LF", "CRLF", "EOF", "partial_Ctrl_D", "master_close")
OWNER_LIFE = ("Emergency", "Failed", "ClosingNormal", "ClosingRestricted")
EVENTS = ("mission_stop", "stop_all", "grant_revocation")
CLOSE = ("close_obligations", "producer_join", "seal", "close_restrict", "emergency_restrict")
REPORT_W1 = ("original_cell_unscheduled", "repeated", "mission_stop", "stop_all", "door_fence", "fatal", "quit")
MODEL_PATH = ("identity_view inference_binding call_slot_reserve attempt_ticket charge_append meter_settle "
              "call_version call_status call_tags call_chat attempt_record attempt_pin_release "
              "operation_complete record_first_classify post_record_recheck").split()


def resolution(sink, dim, value, status):
    """(reason, guard, review) of a reviewed set-B resolution (docs 14.18, 14.19, 14.22), or None."""
    report = sink == "close_report"
    guard = "order" if report else "close_callers"
    if status == TR1:
        return "tr1_hold", guard, "R197" if report else "R214"
    if dim == "ingress" and (report and value in REPORT_W1 or sink in CLOSE and value in ("repeated", "quit")):
        return "w1_type", guard, "R199" if report else "R214"
    if sink == "door_halt" and (dim == "ingress" and value in BYTES + ("repeated", "quit", "read_failure")
                                or dim == "lifecycle" and value in OWNER_LIFE):
        return "w3_event", "door_callers", "R226"
    if report and (dim, value) in (("lifecycle", "ClosingNormal"), ("lifecycle", "ClosedUnclean"),
                                   ("fault", "poison"), ("fault", "panic")):
        return "w3_event", "none", "R197"
    return None


def owed(sink, dim, value):
    """(reason, review) of an owed-guarded key (R255.2; race B stays owed, R265.4), or None."""
    if sink not in MODEL_PATH:
        return None
    if (dim, value) == ("ingress", "read_failure"):
        return "read_failure", "R255.2"
    if sink == "identity_view" and (dim == "ingress" and value in BYTES + ("repeated",)
                                    or dim == "lifecycle" and value in ("Emergency", "ClosingRestricted")):
        return "s2", "R255.2"
    if (sink, dim) == ("call_slot_reserve", "ingress") and value in ("CR", "LF", "CRLF"):
        return "race_b", "R265.4"
    return None


def rows():
    source = open(os.path.join(TESTS, "session_model_bridge.rs"), encoding="utf-8").read()
    credited = set(re.findall(r'"([^"]+)"', source.split("const CREDITED:", 1)[1].split("];", 1)[0]))
    table = open(os.path.join(TESTS, "session/model-bridge-expect.tsv"), encoding="utf-8").read().split("\n")
    for line in table[1:]:
        if not line:
            continue
        f = line.split("\t")
        sink, dim, value, mech, status = f[1], f[2], f[3], f[4], f[17]
        key = f"{f[0].lower()}/{sink}/{dim}/{value}"
        if key in credited:
            continue
        valued = (mech in ("byte", "close_report") or dim == "ingress" and value in ("quit", "repeated")
                  or dim == "lifecycle" and value in OWNER_LIFE)
        if valued and status in ("open", TR1):  # set B (R260.2)
            done, later = resolution(sink, dim, value, status), owed(sink, dim, value)
            if done:
                yield (key, "resolved") + done
            elif later and status == "open":
                yield key, "owed", later[0], "model_path", later[1]
            elif sink in MODEL_PATH:
                yield key, "uncredited", "race_a", "-", "R265"
            elif value == "read_failure":
                yield key, "uncredited", "reachable_read_failure", "-", "R267"
            else:
                yield key, "uncredited", "unclassified", "-", "-"
        elif status == "open" and value in EVENTS:
            yield key, "w1", value, "w1_" + value, "R196.1"


def main():
    out = list(rows())
    text = "key\tstate\treason\tguard\treview\n" + "".join("\t".join(r) + "\n" for r in out)
    stale = not os.path.exists(OUT) or open(OUT, encoding="utf-8").read() != text
    if "--check" not in sys.argv:
        with open(OUT, "w", encoding="utf-8") as handle:
            handle.write(text)
    counts = {}
    for r in out:
        counts[(r[1], r[2])] = counts.get((r[1], r[2]), 0) + 1
    for (state, reason), n in sorted(counts.items()):
        print(f"{state}\t{reason}\t{n}")
    owed_keys = sorted(r[0] for r in out if r[1] == "owed")
    print(f"rows {len(out)}; set B {sum(r[1] != 'w1' for r in out)}; w1 {sum(r[1] == 'w1' for r in out)}")
    print(f"OWED {len(owed_keys)} sha256 {hashlib.sha256(''.join(k + chr(10) for k in owed_keys).encode()).hexdigest()}")
    return 1 if stale and "--check" in sys.argv else 0


if __name__ == "__main__":
    sys.exit(main())

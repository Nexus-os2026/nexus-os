#!/usr/bin/env python3
"""Independent golden vectors for the custody codec, version 1 (I2A, extended
by I2-R1 with the request receipt vectors; the I2A vectors are unchanged).

Stdlib only. Each frame is assembled field by field from the version-1
table (written out below from the format specification, not from the Rust
code), and its digest is hashlib.sha256 of every byte before the digest.
Prints the Rust literals pasted into tests/phase2_custody_codec.rs, and a
plain listing for review. Session storage only; not a repository file.
"""
import hashlib
import struct
import sys

MAGIC = b"NXCD"
RECORD, REQUEST = 0x52, 0x51
VERSION = 0x01

G0 = bytes(16)
G1 = bytes([0x11] * 16)
G2 = bytes(range(16))
GF = bytes([0xFF] * 16)
G3C = bytes([0x3C] * 16)
GA5 = bytes([0xA5] * 16)
GENS = {G0: "G0", G1: "G1", G2: "G2", GF: "GF", G3C: "G3C", GA5: "GA5"}

U32_MAX = 0xFFFF_FFFF
U64_MAX = 0xFFFF_FFFF_FFFF_FFFF

RECORD_KIND = {
    "RunStarted": 0x01, "CaseStarted": 0x02, "ActionStarted": 0x03,
    "ActionFailed": 0x04, "ActionSettled": 0x05, "CaseEnded": 0x06,
    "Control": 0x07, "RecoveryRequired": 0x08, "RecoveryAttempt": 0x09,
    "RunEnded": 0x0A, "IncidentOpened": 0x0B, "IncidentSettled": 0x0C,
}
EXPECTATION = {"Clean": 0x01, "RetainedBoundary": 0x02, "OutputDetached": 0x03}
SLOT = {"Process": 0x01, "Workspace": 0x02, "Fixture": 0x03}
SETTLEMENT = {"Confirmed": 0x01, "OutputLost": 0x02, "NothingCreated": 0x03, "NotAdmitted": 0x04}
VERDICT = {"Pending": 0x01, "Passed": 0x02, "Failed": 0x03}
CONTROL_FACT = {"AdmissionClosed": 0x01, "ShutdownRefused": 0x02}
CLOSURE = {"Cancelled": 0x01, "Failed": 0x02}
CANCEL = {"Requested": 0x01, "LeaseLost": 0x02, "Shutdown": 0x03, "Stop": 0x04, "RunBudget": 0x05}
FAILURE = {
    "Assertion": 0x01, "UnexpectedRetained": 0x02, "UnexpectedCleanup": 0x03,
    "ExpectedConditionUnmet": 0x04, "UnexpectedOwner": 0x05, "LateOwner": 0x06,
    "UnknownOutcome": 0x07, "OutputLost": 0x08, "AuthorityLost": 0x09,
    "RecordFailed": 0x0A, "RecorderFault": 0x0B, "Cancelled": 0x0C,
}
REQUEST_OP = {"Retry": 0x01, "Shutdown": 0x02}


def u8(v):
    return bytes([v])


def u16(v):
    return struct.pack(">H", v)


def u32(v):
    return struct.pack(">I", v)


def u64(v):
    return struct.pack(">Q", v)


def boolean(v):
    return b"\x01" if v else b"\x00"


def ident(pair):
    gen, seq = pair
    return [gen, u64(seq)]


def frame(domain, fields):
    payload = b"".join(fields)
    header = [MAGIC, u8(domain), u8(VERSION), u16(len(payload))]
    covered = b"".join(header) + payload
    digest = hashlib.sha256(covered).hexdigest()
    text = " ".join(part.hex() for part in header + fields)
    return text, digest, len(payload)


def rust_u64(v):
    if v == U64_MAX:
        return "u64::MAX"
    if v < 1_000_000:
        return str(v)
    h = f"{v:016x}"
    return "0x" + "_".join(h[i:i + 4] for i in range(0, 16, 4))


def rust_u32(v):
    if v == U32_MAX:
        return "u32::MAX"
    if v < 1_000_000:
        return str(v)
    h = f"{v:08x}"
    return "0x" + h[:4] + "_" + h[4:]


def rust_id(pair):
    return f"({GENS[pair[0]]}, {rust_u64(pair[1])})"


def record(rid, at, kind, args):
    fields = [rid[0], u64(rid[1]), u64(at), u8(RECORD_KIND[kind])]
    if kind == "RunStarted":
        (n,) = args
        fields += [u32(n)]
        want = f"Want::Plain(RecordKind::RunStarted {{ dispositioned: {rust_u32(n)} }})"
    elif kind == "CaseStarted":
        case, exp = args
        fields += [u32(case), u8(EXPECTATION[exp])]
        want = (f"Want::Plain(RecordKind::CaseStarted {{ case: CaseId({rust_u32(case)}), "
                f"expectation: Expectation::{exp} }})")
    elif kind == "ActionStarted":
        action, slot = args
        fields += ident(action) + [u8(SLOT[slot])]
        want = f"Want::ActionStarted({rust_id(action)}, SlotKind::{slot})"
    elif kind == "ActionFailed":
        (action,) = args
        fields += ident(action)
        want = f"Want::ActionFailed({rust_id(action)})"
    elif kind == "ActionSettled":
        action, how = args
        fields += ident(action) + [u8(SETTLEMENT[how])]
        want = f"Want::ActionSettled({rust_id(action)}, Settlement::{how})"
    elif kind == "CaseEnded":
        case, passed = args
        fields += [u32(case), boolean(passed)]
        want = (f"Want::Plain(RecordKind::CaseEnded {{ case: CaseId({rust_u32(case)}), "
                f"passed: {'true' if passed else 'false'} }})")
    elif kind == "Control":
        fact = args[0]
        fields += [u8(CONTROL_FACT[fact])]
        if fact == "AdmissionClosed":
            closure, inner, after = args[1:]
            table = CANCEL if closure == "Cancelled" else FAILURE
            enum = "CancelReason" if closure == "Cancelled" else "FailureClass"
            fields += [u8(CLOSURE[closure]), u8(table[inner]), u64(after)]
            want = (f"Want::Plain(RecordKind::Control {{ fact: ControlFact::AdmissionClosed {{ "
                    f"reason: ClosureReason::{closure}({enum}::{inner}), after: {rust_u64(after)} }} }})")
        else:
            want = "Want::Plain(RecordKind::Control { fact: ControlFact::ShutdownRefused })"
    elif kind == "RecoveryRequired":
        want = "Want::Plain(RecordKind::RecoveryRequired)"
    elif kind == "RecoveryAttempt":
        attempt, resolved = args
        fields += [u32(attempt), boolean(resolved)]
        want = (f"Want::Plain(RecordKind::RecoveryAttempt {{ attempt: {rust_u32(attempt)}, "
                f"resolved: {'true' if resolved else 'false'} }})")
    elif kind == "RunEnded":
        verdict, by = args
        fields += [u8(VERDICT[verdict])]
        if by is None:
            fields += [u8(0)]
            rust_by = "None"
        else:
            fields += [u8(1), u32(by)]
            rust_by = f"Some({rust_u32(by)})"
        want = f"Want::Plain(RecordKind::RunEnded {{ verdict: Verdict::{verdict}, resolved_by: {rust_by} }})"
    elif kind == "IncidentOpened":
        incident, action, slot = args
        fields += ident(incident) + ident(action)
        if slot is None:
            fields += [u8(0)]
            rust_slot = "None"
        else:
            fields += [u8(1), u8(SLOT[slot])]
            rust_slot = f"Some(SlotKind::{slot})"
        want = f"Want::IncidentOpened({rust_id(incident)}, {rust_id(action)}, {rust_slot})"
    elif kind == "IncidentSettled":
        incident, how = args
        fields += ident(incident) + [u8(SETTLEMENT[how])]
        want = f"Want::IncidentSettled({rust_id(incident)}, Settlement::{how})"
    else:
        raise SystemExit(kind)
    text, digest, length = frame(RECORD, fields)
    return dict(text=text, digest=digest, length=length,
                rust=(f"    RecordVector {{\n        frame: \"{text}\",\n        digest: \"{digest}\",\n"
                      f"        id: {rust_id(rid)},\n        at: {rust_u64(at)},\n        kind: {want},\n    }},"))


def request(gen, seq, op, args):
    fields = [gen, u64(seq), u8(REQUEST_OP[op])]
    if op == "Retry":
        (epoch,) = args
        fields += [u64(epoch)]
        rust_op = f"RequestOp::Retry {{ epoch: {rust_u64(epoch)} }}"
    else:
        rust_op = "RequestOp::Shutdown"
    text, digest, length = frame(REQUEST, fields)
    return dict(text=text, digest=digest, length=length,
                rust=(f"    RequestVector {{\n        frame: \"{text}\",\n        digest: \"{digest}\",\n"
                      f"        request: Request {{ generation: Generation::new({GENS[gen]}), "
                      f"seq: {rust_u64(seq)}, op: {rust_op} }},\n    }},"))


PATTERN64 = 0x0102_0304_0506_0708
PATTERN_AT = 0x1122_3344_5566_7788
PATTERN32 = 0x0102_0304

RECORDS = [
    ((G1, 1), 0, "RunStarted", (0,)),
    ((GF, U64_MAX), U64_MAX, "RunStarted", (U32_MAX,)),
    ((G1, 2), 1, "CaseStarted", (0, "Clean")),
    ((G0, 0), PATTERN_AT, "CaseStarted", (U32_MAX, "RetainedBoundary")),
    ((G2, PATTERN64), 3, "CaseStarted", (PATTERN32, "OutputDetached")),
    ((G1, 3), 4, "ActionStarted", ((G2, 0), "Process")),
    ((G1, 4), 5, "ActionStarted", ((GF, U64_MAX), "Workspace")),
    ((G1, 5), 6, "ActionStarted", ((G1, 7), "Fixture")),
    ((G1, 6), 7, "ActionFailed", ((G1, 1),)),
    ((G1, 7), 8, "ActionSettled", ((G1, 1), "Confirmed")),
    ((G1, 8), 9, "ActionSettled", ((G1, 1), "OutputLost")),
    ((G1, 9), 10, "ActionSettled", ((G0, 0), "NothingCreated")),
    ((G1, 10), 11, "ActionSettled", ((G2, PATTERN64), "NotAdmitted")),
    ((G1, 11), 12, "CaseEnded", (1, False)),
    ((G1, 12), 13, "CaseEnded", (1, True)),
]
for index, reason in enumerate(CANCEL):
    after = [0, 1, U64_MAX, PATTERN64, 2][index]
    RECORDS.append(((G1, 20 + index), 30 + index, "Control", ("AdmissionClosed", "Cancelled", reason, after)))
for index, klass in enumerate(FAILURE):
    after = [0, 1, 2, 3, U64_MAX, 5, 6, 7, 8, 9, 10, PATTERN64][index]
    RECORDS.append(((G1, 40 + index), 60 + index, "Control", ("AdmissionClosed", "Failed", klass, after)))
RECORDS += [
    ((G1, 60), 80, "Control", ("ShutdownRefused",)),
    ((G1, 61), 81, "RecoveryRequired", ()),
    ((G1, 62), 82, "RecoveryAttempt", (1, False)),
    ((G1, 63), 83, "RecoveryAttempt", (U32_MAX, True)),
    ((G1, 64), 84, "RunEnded", ("Pending", None)),
    ((G1, 65), 85, "RunEnded", ("Passed", None)),
    ((G1, 66), 86, "RunEnded", ("Failed", 0)),
    ((G1, 67), 87, "RunEnded", ("Passed", U32_MAX)),
    ((G1, 70), 90, "IncidentOpened", ((G1, 1), (G1, 3), None)),
    ((G1, 71), 91, "IncidentOpened", ((G1, 2), (G1, 3), "Process")),
    ((G1, 72), 92, "IncidentOpened", ((GF, U64_MAX), (G0, 0), "Workspace")),
    ((G1, 73), 93, "IncidentOpened", ((G2, PATTERN64), (G2, 1), "Fixture")),
    ((G1, 74), 94, "IncidentSettled", ((G1, 1), "Confirmed")),
    ((G1, 75), 95, "IncidentSettled", ((G1, 2), "OutputLost")),
    ((G1, 76), 96, "IncidentSettled", ((G0, 0), "NothingCreated")),
    ((G1, 77), 97, "IncidentSettled", ((GF, U64_MAX), "NotAdmitted")),
    ((G1, 78), 98, "IncidentSettled", ((G2, 0), "Confirmed")),
    ((G2, 0), 2, "RunStarted", (1,)),
]
REQUESTS = [
    (G1, 1, "Retry", (0,)),
    (GF, U64_MAX, "Retry", (U64_MAX,)),
    (G0, 0, "Shutdown", ()),
    (G2, 2, "Retry", (PATTERN64,)),
]
# The requests the I2-R1 receipt control serves through the real core:
# custody A (generation 3c, requiring recovery), custody B (generation a5)
# and custody C (generation 3c), in the order each serves them.
RECEIPTS = [
    (G3C, 1, "Retry", (0,)),
    (G3C, 2, "Shutdown", ()),
    (G3C, 3, "Retry", (1,)),
    (G3C, 4, "Retry", (U64_MAX,)),
    (G3C, 5, "Retry", (0,)),
    (GA5, 1, "Retry", (0,)),
    (G3C, 1, "Shutdown", ()),
    (G3C, 2, "Retry", (0,)),
    (G3C, 3, "Retry", (2,)),
]


def main():
    records = [record(*spec) for spec in RECORDS]
    requests = [request(*spec) for spec in REQUESTS]
    if "--rust-receipts" in sys.argv:
        print("const RECEIPT_VECTORS: &[RequestVector] = &[")
        for spec in RECEIPTS:
            print(request(*spec)["rust"])
        print("];")
        return
    if "--rust" in sys.argv:
        print("const RECORD_VECTORS: &[RecordVector] = &[")
        for item in records:
            print(item["rust"])
        print("];")
        print()
        print("const REQUEST_VECTORS: &[RequestVector] = &[")
        for item in requests:
            print(item["rust"])
        print("];")
        return
    for spec, item in zip(RECORDS, records):
        print(f"R {spec[2]:16} L={item['length']:3} {item['digest']}  {item['text']}")
    for spec, item in zip(REQUESTS, requests):
        print(f"Q {spec[2]:16} L={item['length']:3} {item['digest']}  {item['text']}")
    if "--with-receipts" in sys.argv:
        for spec in RECEIPTS:
            item = request(*spec)
            print(f"T {spec[2]:16} L={item['length']:3} {item['digest']}  {item['text']}")
    lengths = [item["length"] for item in records]
    print(f"records {len(records)}, payload lengths {min(lengths)}..{max(lengths)}; requests {len(requests)}")


if __name__ == "__main__":
    main()

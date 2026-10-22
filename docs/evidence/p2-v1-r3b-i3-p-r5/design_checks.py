#!/usr/bin/env python3
"""Design model for the P2 custody durable recorder and refusal store.

Mission P2-V1-R3B-I3-P-R5. The specification under test is
docs/architecture/p2-custody-durable-recorder-design.md (revision R5).

What this is: a standard-library model of that design, carried forward from
the R4 model (docs/evidence/p2-v1-r3b-i3-p-r4/design_checks.py, which stays
unchanged, as do the R1, R2 and R3 models). It exercises the storage state
model (kernel-visible, durable and pending bytes, writeback errors with
per-description cursors, page reclaim apart from inode eviction, metadata made
durable under permitted schedules before or after explicit directory syncs,
and the journal behaviour a new descriptor's sync depends on), the journal
formats, open-file-description locks, the recorder's total fatal-failure
protocol and store admission gate, provision selection with post-lock
revalidation and durable activation, the maintenance session, bounded
enumeration, classification, bindings and the administrative procedures,
alone and composed with dependent work and repeated crashes, all on in-memory
byte images and simulated state. R4 added the effective filesystem profile and
a small transaction model of the jbd2 and ext4 paths activation depends on.
R5 adds the stable-completion storage contract: the storage below the
filesystem as the store uid observes it (the device's form, the kernel's
registration of its cache, its identity), the trusted construction of a
verified storage admission, devices whose completed writes are stable or
still in a volatile cache, reported write failures, writes still in flight,
and the superblock's tail write with native or emulated forced unit access.
It first re-runs the Architect's counterexamples against the unchanged R1 and
R2 models, the R4 findings against the unchanged R3 model and the R5 findings,
R4's checkpoint witness among them, against the unchanged R4 model, and
requires each to reproduce. Model-level negative controls restore incorrect
behaviours (several are the first candidate's or an earlier revision's) and
must fail the intended marked assertions.

What this is not: the store, the recorder, or a test of the Rust code, the
kernel, ext4 or any device. It opens no store, calls no native operation,
performs no privileged I/O and causes no real crash or power loss. Its record
sequences and its core model (CoreSim) are derived by reading the source at
45898e05178a56efaadeb1f8f9ee7a0a521c72c9; they are not Rust execution results.
Its journal and block-layer behaviour is derived by reading the Linux v6.17
sources the design cites; it is not a kernel. Its storage values (the sysfs
attributes, the identities, the controller's report) are fixtures in the form
v6.17 prints them, not observations of any host or device, and they qualify
nothing. Its version-1 record encoder is checked against the golden vectors
copied from crates/nexus-verifier-sandbox/tests/phase2_custody_codec.rs at
that commit.

Usage: python3 -B design_checks.py [--json PATH]

Exit status 0 only if every R1 and R2 counterexample, every R4 finding and
every R5 finding reproduces on its own unchanged model, every baseline check
passes, every negative control is caught by its intended assertion (and a
composed-only control leaves the earlier snapshot checks passing), the
restored baseline passes again and no tool failure occurred.
"""

import argparse
import collections
import copy
import datetime
import hashlib
import importlib.util
import itertools
import json
import pathlib
import re
import struct
import sys
import traceback

SOURCE_BASELINE = "45898e05178a56efaadeb1f8f9ee7a0a521c72c9"


# ===========================================================================
# 0. Mutants: the model-level negative controls switch these on, one at a time
# ===========================================================================


class _Mutants:
    def __init__(self):
        self.active = frozenset()

    def on(self, name):
        return name in self.active


MUT = _Mutants()


# ===========================================================================
# 1. Primitives
# ===========================================================================

BLOCK = 4096
MAX_CLAIM = 1 << 63
U64_MAX = (1 << 64) - 1
U32_MAX = (1 << 32) - 1


def sha(data):
    return hashlib.sha256(bytes(data)).digest()


def u8(v):
    return bytes([v])


def u16(v):
    return struct.pack(">H", v)


def u32(v):
    return struct.pack(">I", v)


def u64(v):
    return struct.pack(">Q", v)


def be(data):
    return int.from_bytes(bytes(data), "big")


# ===========================================================================
# 2. Version-1 record codec (codec.rs:22-79; codec.rs:238-324)
# ===========================================================================

NXCD = b"NXCD"
RECORD_DOMAIN = 0x52
CODEC_VERSION = 0x01
KIND_TAG = {
    "RunStarted": 0x01, "CaseStarted": 0x02, "ActionStarted": 0x03,
    "ActionFailed": 0x04, "ActionSettled": 0x05, "CaseEnded": 0x06,
    "Control": 0x07, "RecoveryRequired": 0x08, "RecoveryAttempt": 0x09,
    "RunEnded": 0x0A, "IncidentOpened": 0x0B, "IncidentSettled": 0x0C,
}
EXPECTATION = {"Clean": 1, "RetainedBoundary": 2, "OutputDetached": 3}
SLOT = {"Process": 1, "Workspace": 2, "Fixture": 3}
SETTLEMENT = {"Confirmed": 1, "OutputLost": 2, "NothingCreated": 3, "NotAdmitted": 4}
VERDICT = {"Pending": 1, "Passed": 2, "Failed": 3}
CONTROL_FACT = {"AdmissionClosed": 1, "ShutdownRefused": 2}
CLOSURE = {"Cancelled": 1, "Failed": 2}
CANCEL = {"Requested": 1, "LeaseLost": 2, "Shutdown": 3, "Stop": 4, "RunBudget": 5}
FAILURE = {
    "Assertion": 1, "UnexpectedRetained": 2, "UnexpectedCleanup": 3,
    "ExpectedConditionUnmet": 4, "UnexpectedOwner": 5, "LateOwner": 6,
    "UnknownOutcome": 7, "OutputLost": 8, "AuthorityLost": 9,
    "RecordFailed": 10, "RecorderFault": 11, "Cancelled": 12,
}
MIN_RECORD_PAYLOAD = 33
MAX_RECORD_PAYLOAD = 83

Record = collections.namedtuple("Record", "gen seq at kind digest")


class CodecError(Exception):
    pass


def _ident(pair):
    return pair[0] + u64(pair[1])


def record_payload(gen, seq, at, kind):
    out = [gen, u64(seq), u64(at)]
    name = kind[0]
    if name == "AdmissionClosed":
        (closure, inner), after = kind[1], kind[2]
        table = CANCEL if closure == "Cancelled" else FAILURE
        out += [u8(KIND_TAG["Control"]), u8(CONTROL_FACT["AdmissionClosed"]),
                u8(CLOSURE[closure]), u8(table[inner]), u64(after)]
    elif name == "ShutdownRefused":
        out += [u8(KIND_TAG["Control"]), u8(CONTROL_FACT["ShutdownRefused"])]
    else:
        out.append(u8(KIND_TAG[name]))
        if name == "RunStarted":
            out.append(u32(kind[1]))
        elif name == "CaseStarted":
            out += [u32(kind[1]), u8(EXPECTATION[kind[2]])]
        elif name == "ActionStarted":
            out += [_ident(kind[1]), u8(SLOT[kind[2]])]
        elif name == "ActionFailed":
            out.append(_ident(kind[1]))
        elif name == "ActionSettled":
            out += [_ident(kind[1]), u8(SETTLEMENT[kind[2]])]
        elif name == "CaseEnded":
            out += [u32(kind[1]), u8(1 if kind[2] else 0)]
        elif name == "RecoveryRequired":
            pass
        elif name == "RecoveryAttempt":
            out += [u32(kind[1]), u8(1 if kind[2] else 0)]
        elif name == "RunEnded":
            out.append(u8(VERDICT[kind[1]]))
            out += [u8(0)] if kind[2] is None else [u8(1), u32(kind[2])]
        elif name == "IncidentOpened":
            out += [_ident(kind[1]), _ident(kind[2])]
            out += [u8(0)] if kind[3] is None else [u8(1), u8(SLOT[kind[3]])]
        elif name == "IncidentSettled":
            out += [_ident(kind[1]), u8(SETTLEMENT[kind[2]])]
        else:
            raise ValueError(name)
    return b"".join(out)


def encode_record(gen, seq, at, kind):
    payload = record_payload(gen, seq, at, kind)
    covered = NXCD + u8(RECORD_DOMAIN) + u8(CODEC_VERSION) + u16(len(payload)) + payload
    return covered + sha(covered)


class _Reader:
    def __init__(self, data):
        self.data = data
        self.at = 0

    def take(self, n):
        if self.at + n > len(self.data):
            raise CodecError("truncated payload")
        out = self.data[self.at:self.at + n]
        self.at += n
        return out

    def u8(self):
        return self.take(1)[0]

    def u32(self):
        return be(self.take(4))

    def u64(self):
        return be(self.take(8))

    def ident(self):
        return (bytes(self.take(16)), self.u64())

    def boolean(self):
        value = self.u8()
        if value not in (0, 1):
            raise CodecError("non-canonical boolean or presence byte")
        return value == 1

    def tag(self, table):
        value = self.u8()
        for key, code in table.items():
            if code == value:
                return key
        raise CodecError("invalid tag")

    def finish(self):
        if self.at != len(self.data):
            raise CodecError("trailing payload bytes")


def decode_record(frame):
    frame = bytes(frame)
    if len(frame) > 4096:
        raise CodecError("frame too long")
    if len(frame) < 8:
        raise CodecError("truncated")
    if frame[0:4] != NXCD or frame[4] != RECORD_DOMAIN or frame[5] != CODEC_VERSION:
        raise CodecError("bad magic, domain or version")
    length = be(frame[6:8])
    if length == 0 or length > 4096 - 40:
        raise CodecError("bad length")
    if len(frame) != 8 + length + 32:
        raise CodecError("frame size")
    if sha(frame[:8 + length]) != frame[8 + length:]:
        raise CodecError("digest mismatch")
    r = _Reader(frame[8:8 + length])
    gen = bytes(r.take(16))
    seq = r.u64()
    at = r.u64()
    tag = r.u8()
    if tag == 0x01:
        kind = ("RunStarted", r.u32())
    elif tag == 0x02:
        kind = ("CaseStarted", r.u32(), r.tag(EXPECTATION))
    elif tag == 0x03:
        kind = ("ActionStarted", r.ident(), r.tag(SLOT))
    elif tag == 0x04:
        kind = ("ActionFailed", r.ident())
    elif tag == 0x05:
        kind = ("ActionSettled", r.ident(), r.tag(SETTLEMENT))
    elif tag == 0x06:
        kind = ("CaseEnded", r.u32(), r.boolean())
    elif tag == 0x07:
        fact = r.tag(CONTROL_FACT)
        if fact == "AdmissionClosed":
            closure = r.tag(CLOSURE)
            inner = r.tag(CANCEL if closure == "Cancelled" else FAILURE)
            kind = ("AdmissionClosed", (closure, inner), r.u64())
        else:
            kind = ("ShutdownRefused",)
    elif tag == 0x08:
        kind = ("RecoveryRequired",)
    elif tag == 0x09:
        kind = ("RecoveryAttempt", r.u32(), r.boolean())
    elif tag == 0x0A:
        verdict = r.tag(VERDICT)
        by = r.u32() if r.boolean() else None
        kind = ("RunEnded", verdict, by)
    elif tag == 0x0B:
        incident = r.ident()
        action = r.ident()
        slot = r.tag(SLOT) if r.boolean() else None
        kind = ("IncidentOpened", incident, action, slot)
    elif tag == 0x0C:
        kind = ("IncidentSettled", r.ident(), r.tag(SETTLEMENT))
    else:
        raise CodecError("invalid record kind")
    r.finish()
    return Record(gen, seq, at, kind, frame[8 + length:])


# Golden record vectors copied from RECORD_VECTORS in
# crates/nexus-verifier-sandbox/tests/phase2_custody_codec.rs at the source
# baseline: (line of the frame literal, covered bytes as written there, digest,
# record id, instant, kind). reference_check.py verifies every literal against
# that file. The kind values were transcribed from the Rust literals.
GOLDEN_GENERATIONS = {
    "G0": bytes(16), "G1": bytes([0x11]) * 16, "G2": bytes(range(16)),
    "GF": bytes([0xFF]) * 16, "G3C": bytes([0x3C]) * 16, "GA5": bytes([0xA5]) * 16,
}
GOLDEN_RECORD_VECTORS = (
    (123, '4e584344 52 01 0025 11111111111111111111111111111111 0000000000000001 0000000000000000 01 00000000', '1df88a9cfe3b6dd027815af0cfb6a92c412c6de2d94cb8e84f80d3cf0ee516e3', ('G1', 1), 0, ('RunStarted', 0)),
    (130, '4e584344 52 01 0025 ffffffffffffffffffffffffffffffff ffffffffffffffff ffffffffffffffff 01 ffffffff', '75a67f6f239fad206b35209fcaa2bca3bf52e63def87aa12efac2ab777b12754', ('GF', 18446744073709551615), 18446744073709551615, ('RunStarted', 4294967295)),
    (137, '4e584344 52 01 0026 11111111111111111111111111111111 0000000000000002 0000000000000001 02 00000000 01', '6266f85645c857776ba08de4ea3f2c945b52efffdbed9ad1011412673b80afb8', ('G1', 2), 1, ('CaseStarted', 0, 'Clean')),
    (144, '4e584344 52 01 0026 00000000000000000000000000000000 0000000000000000 1122334455667788 02 ffffffff 02', '4d95799bca2b31230db6f44a4769fb7c0c4e6c0bc458b27d8369bdeac203f43c', ('G0', 0), 1234605616436508552, ('CaseStarted', 4294967295, 'RetainedBoundary')),
    (151, '4e584344 52 01 0026 000102030405060708090a0b0c0d0e0f 0102030405060708 0000000000000003 02 01020304 03', 'ee05dad50310a8fff8cbcc33a09ac7514244a452365d091a386dfd3a30611ed6', ('G2', 72623859790382856), 3, ('CaseStarted', 16909060, 'OutputDetached')),
    (158, '4e584344 52 01 003a 11111111111111111111111111111111 0000000000000003 0000000000000004 03 000102030405060708090a0b0c0d0e0f 0000000000000000 01', '8587df47a167e211ee9b2d086d2edf14fc779e4496e1547856c40f1aa4f90da8', ('G1', 3), 4, ('ActionStarted', ('G2', 0), 'Process')),
    (165, '4e584344 52 01 003a 11111111111111111111111111111111 0000000000000004 0000000000000005 03 ffffffffffffffffffffffffffffffff ffffffffffffffff 02', '35f55ec7504f80c3d1ff32562dc80fa44973511257aa883260efa3a7dd3721b6', ('G1', 4), 5, ('ActionStarted', ('GF', 18446744073709551615), 'Workspace')),
    (172, '4e584344 52 01 003a 11111111111111111111111111111111 0000000000000005 0000000000000006 03 11111111111111111111111111111111 0000000000000007 03', '8cec803c4c603d85e97b58d85865e6e071cb50593c507cf6c666a685538b374f', ('G1', 5), 6, ('ActionStarted', ('G1', 7), 'Fixture')),
    (179, '4e584344 52 01 0039 11111111111111111111111111111111 0000000000000006 0000000000000007 04 11111111111111111111111111111111 0000000000000001', '2c6b71ae14a5bee04123ba167cc03a6f0174a6bfb35d21ade9df45ad4237454e', ('G1', 6), 7, ('ActionFailed', ('G1', 1))),
    (186, '4e584344 52 01 003a 11111111111111111111111111111111 0000000000000007 0000000000000008 05 11111111111111111111111111111111 0000000000000001 01', '4d912098c05741a18bfc5c13339d26d1cedccfd2488e991fffd2ed36c09b3af5', ('G1', 7), 8, ('ActionSettled', ('G1', 1), 'Confirmed')),
    (193, '4e584344 52 01 003a 11111111111111111111111111111111 0000000000000008 0000000000000009 05 11111111111111111111111111111111 0000000000000001 02', 'a67e2c8d8f9c96692387cdf23c73344cfe621f98d42aef5f59b515d3a25a6a87', ('G1', 8), 9, ('ActionSettled', ('G1', 1), 'OutputLost')),
    (200, '4e584344 52 01 003a 11111111111111111111111111111111 0000000000000009 000000000000000a 05 00000000000000000000000000000000 0000000000000000 03', '7922ac776b03addca545e429b3171e28b1f441dde8b660a6434aed588580feb8', ('G1', 9), 10, ('ActionSettled', ('G0', 0), 'NothingCreated')),
    (207, '4e584344 52 01 003a 11111111111111111111111111111111 000000000000000a 000000000000000b 05 000102030405060708090a0b0c0d0e0f 0102030405060708 04', '68cecad189f89fcf86ee152b4a1113298ea9b6a44a1a513aba3be2e0f213db8c', ('G1', 10), 11, ('ActionSettled', ('G2', 72623859790382856), 'NotAdmitted')),
    (214, '4e584344 52 01 0026 11111111111111111111111111111111 000000000000000b 000000000000000c 06 00000001 00', '036cce17c3a88ed633fb3f1c807e9a994d8b06de68b963a8400acd5445f55e0c', ('G1', 11), 12, ('CaseEnded', 1, False)),
    (221, '4e584344 52 01 0026 11111111111111111111111111111111 000000000000000c 000000000000000d 06 00000001 01', '0be591dbb25a40f0651a2991f483bbed188c4f5cd96f697607c794fa9518fd69', ('G1', 12), 13, ('CaseEnded', 1, True)),
    (228, '4e584344 52 01 002c 11111111111111111111111111111111 0000000000000014 000000000000001e 07 01 01 01 0000000000000000', 'dbb39737cc969c9d9467ef2fff7f50d5a903669c9e6368334f533e95a205e465', ('G1', 20), 30, ('AdmissionClosed', ('Cancelled', 'Requested'), 0)),
    (235, '4e584344 52 01 002c 11111111111111111111111111111111 0000000000000015 000000000000001f 07 01 01 02 0000000000000001', '4e300222b14664e3acb2709138018c56c740a3b092f0196f966fa40f346d4223', ('G1', 21), 31, ('AdmissionClosed', ('Cancelled', 'LeaseLost'), 1)),
    (242, '4e584344 52 01 002c 11111111111111111111111111111111 0000000000000016 0000000000000020 07 01 01 03 ffffffffffffffff', 'eebf9acedc715992f8d1ef20cea08ad2dd541e4e6973f80e93f721e09ae2ee00', ('G1', 22), 32, ('AdmissionClosed', ('Cancelled', 'Shutdown'), 18446744073709551615)),
    (249, '4e584344 52 01 002c 11111111111111111111111111111111 0000000000000017 0000000000000021 07 01 01 04 0102030405060708', '4c8b83ea960bc819cd6a50b96c39b9c4728a476b3ce50f3173a30b5441b4fdd3', ('G1', 23), 33, ('AdmissionClosed', ('Cancelled', 'Stop'), 72623859790382856)),
    (256, '4e584344 52 01 002c 11111111111111111111111111111111 0000000000000018 0000000000000022 07 01 01 05 0000000000000002', 'd02e309dd2c7acff5802f97ac0a8b94cb5deb606c4144a3ef5544fb95147159a', ('G1', 24), 34, ('AdmissionClosed', ('Cancelled', 'RunBudget'), 2)),
    (263, '4e584344 52 01 002c 11111111111111111111111111111111 0000000000000028 000000000000003c 07 01 02 01 0000000000000000', '5c926c40256a4161178c2c8ace241886a017cb28e02ae649ea894093a1bbe2af', ('G1', 40), 60, ('AdmissionClosed', ('Failed', 'Assertion'), 0)),
    (270, '4e584344 52 01 002c 11111111111111111111111111111111 0000000000000029 000000000000003d 07 01 02 02 0000000000000001', '0f7a9b1ae736416ca465c2b4a875ad8abcffea76e1784689549049160baf6fa1', ('G1', 41), 61, ('AdmissionClosed', ('Failed', 'UnexpectedRetained'), 1)),
    (277, '4e584344 52 01 002c 11111111111111111111111111111111 000000000000002a 000000000000003e 07 01 02 03 0000000000000002', 'a99b37298be48dcc6f9ab47bc6e7cf90021b3269ea65b7cc47bc9c7b706fe48b', ('G1', 42), 62, ('AdmissionClosed', ('Failed', 'UnexpectedCleanup'), 2)),
    (284, '4e584344 52 01 002c 11111111111111111111111111111111 000000000000002b 000000000000003f 07 01 02 04 0000000000000003', 'f577db8f3108a08415e2088c73ceb906059de0180f580313fd11a290211b1c6a', ('G1', 43), 63, ('AdmissionClosed', ('Failed', 'ExpectedConditionUnmet'), 3)),
    (291, '4e584344 52 01 002c 11111111111111111111111111111111 000000000000002c 0000000000000040 07 01 02 05 ffffffffffffffff', 'f1548c8e50f27d9b9e4ec1f6337cc1632a8f97db7c406d259f5706b1f276d5a1', ('G1', 44), 64, ('AdmissionClosed', ('Failed', 'UnexpectedOwner'), 18446744073709551615)),
    (298, '4e584344 52 01 002c 11111111111111111111111111111111 000000000000002d 0000000000000041 07 01 02 06 0000000000000005', '0d0e03162de32a12d65d9b2752052df62179b62606255e6fefa0dc655b0021f8', ('G1', 45), 65, ('AdmissionClosed', ('Failed', 'LateOwner'), 5)),
    (305, '4e584344 52 01 002c 11111111111111111111111111111111 000000000000002e 0000000000000042 07 01 02 07 0000000000000006', '5eb24ae75a71aed445b94b4bb94b572a6d66de686616b0725030f2fb3dc1e08c', ('G1', 46), 66, ('AdmissionClosed', ('Failed', 'UnknownOutcome'), 6)),
    (312, '4e584344 52 01 002c 11111111111111111111111111111111 000000000000002f 0000000000000043 07 01 02 08 0000000000000007', '12bb5b920746116c031b01dcfb4e7899927ffda2b9ad4665801013ece379db92', ('G1', 47), 67, ('AdmissionClosed', ('Failed', 'OutputLost'), 7)),
    (319, '4e584344 52 01 002c 11111111111111111111111111111111 0000000000000030 0000000000000044 07 01 02 09 0000000000000008', '811014db56bd3d849b46d59cbf063bb6b7aeaa367e1baa2fffdb723bc2c6f71e', ('G1', 48), 68, ('AdmissionClosed', ('Failed', 'AuthorityLost'), 8)),
    (326, '4e584344 52 01 002c 11111111111111111111111111111111 0000000000000031 0000000000000045 07 01 02 0a 0000000000000009', 'd237e9fff6c494921dc24300263235fb09f64e68bc7e474691a8077c076c654b', ('G1', 49), 69, ('AdmissionClosed', ('Failed', 'RecordFailed'), 9)),
    (333, '4e584344 52 01 002c 11111111111111111111111111111111 0000000000000032 0000000000000046 07 01 02 0b 000000000000000a', 'a3011ca0fd78d22af9b7b2cc82cf843ff1a9c251fdd01f7f31033f075fdfaeb5', ('G1', 50), 70, ('AdmissionClosed', ('Failed', 'RecorderFault'), 10)),
    (340, '4e584344 52 01 002c 11111111111111111111111111111111 0000000000000033 0000000000000047 07 01 02 0c 0102030405060708', '62fc6d6e1767a20b779a3125770830ae573914366b1939ac6dd2a279232f9011', ('G1', 51), 71, ('AdmissionClosed', ('Failed', 'Cancelled'), 72623859790382856)),
    (347, '4e584344 52 01 0022 11111111111111111111111111111111 000000000000003c 0000000000000050 07 02', 'ccb5d38fd66e5fcbf971eb089fd78614eac039cbf87f29b1a5cf83d766d0ea57', ('G1', 60), 80, ('ShutdownRefused',)),
    (354, '4e584344 52 01 0021 11111111111111111111111111111111 000000000000003d 0000000000000051 08', '25cd018c756c393ab5b79706f2d08821308bcc9113b16bb649dfb8b7d667a248', ('G1', 61), 81, ('RecoveryRequired',)),
    (361, '4e584344 52 01 0026 11111111111111111111111111111111 000000000000003e 0000000000000052 09 00000001 00', '3abf32d0e8cbbf9c6de478ca93c4d83b44a2e279354daf5e8a68738bb5949361', ('G1', 62), 82, ('RecoveryAttempt', 1, False)),
    (368, '4e584344 52 01 0026 11111111111111111111111111111111 000000000000003f 0000000000000053 09 ffffffff 01', 'f60d502edc0bdb8f646ce56b484a8a665568a759f383d529b6201ddf91fcbdd5', ('G1', 63), 83, ('RecoveryAttempt', 4294967295, True)),
    (375, '4e584344 52 01 0023 11111111111111111111111111111111 0000000000000040 0000000000000054 0a 01 00', '8a07b4e7cb23e30f58054986216699a1117cd870fbfdc1172d8c909d2c823ce5', ('G1', 64), 84, ('RunEnded', 'Pending', None)),
    (382, '4e584344 52 01 0023 11111111111111111111111111111111 0000000000000041 0000000000000055 0a 02 00', 'a2d26f768e6ecfaff56605339726c8cf6500a4a7113738bde27cd6191803ff31', ('G1', 65), 85, ('RunEnded', 'Passed', None)),
    (389, '4e584344 52 01 0027 11111111111111111111111111111111 0000000000000042 0000000000000056 0a 03 01 00000000', '95c6ffa53dc4e4acd7a448f8ebdb293694a41b5ae43e7f6ec191b71ec69c2d59', ('G1', 66), 86, ('RunEnded', 'Failed', 0)),
    (396, '4e584344 52 01 0027 11111111111111111111111111111111 0000000000000043 0000000000000057 0a 02 01 ffffffff', '05d1c57038aee796c78fdfa194da81a14ab774690bace971bc83e2ab46ea8b1d', ('G1', 67), 87, ('RunEnded', 'Passed', 4294967295)),
    (403, '4e584344 52 01 0052 11111111111111111111111111111111 0000000000000046 000000000000005a 0b 11111111111111111111111111111111 0000000000000001 11111111111111111111111111111111 0000000000000003 00', 'd1e2d6a959b71689485a3418447c5fb232e2a8de05ef51ad9c54b03f4959de24', ('G1', 70), 90, ('IncidentOpened', ('G1', 1), ('G1', 3), None)),
    (410, '4e584344 52 01 0053 11111111111111111111111111111111 0000000000000047 000000000000005b 0b 11111111111111111111111111111111 0000000000000002 11111111111111111111111111111111 0000000000000003 01 01', 'a4814e6bc5633b7c1424ba93f971040012c830db627fcb8d2d0bd1ce288f789e', ('G1', 71), 91, ('IncidentOpened', ('G1', 2), ('G1', 3), 'Process')),
    (417, '4e584344 52 01 0053 11111111111111111111111111111111 0000000000000048 000000000000005c 0b ffffffffffffffffffffffffffffffff ffffffffffffffff 00000000000000000000000000000000 0000000000000000 01 02', '54400e4f3d95134fd68699b912e7f559caa1e4776007bc31e5acbffa616b7a80', ('G1', 72), 92, ('IncidentOpened', ('GF', 18446744073709551615), ('G0', 0), 'Workspace')),
    (424, '4e584344 52 01 0053 11111111111111111111111111111111 0000000000000049 000000000000005d 0b 000102030405060708090a0b0c0d0e0f 0102030405060708 000102030405060708090a0b0c0d0e0f 0000000000000001 01 03', 'c38beb76b437acf545c4b6e2b118846be46921b8a6bdee14be89e38c09c4ec2c', ('G1', 73), 93, ('IncidentOpened', ('G2', 72623859790382856), ('G2', 1), 'Fixture')),
    (431, '4e584344 52 01 003a 11111111111111111111111111111111 000000000000004a 000000000000005e 0c 11111111111111111111111111111111 0000000000000001 01', '014ef751cf6ec4548a32e95663f8a68cb70bb5b8fbc437193d329fd46c9bc79f', ('G1', 74), 94, ('IncidentSettled', ('G1', 1), 'Confirmed')),
    (438, '4e584344 52 01 003a 11111111111111111111111111111111 000000000000004b 000000000000005f 0c 11111111111111111111111111111111 0000000000000002 02', '388313d7da15b1dff00effbd8408a6e36d8bba34beb7789e6a57245acedca8ba', ('G1', 75), 95, ('IncidentSettled', ('G1', 2), 'OutputLost')),
    (445, '4e584344 52 01 003a 11111111111111111111111111111111 000000000000004c 0000000000000060 0c 00000000000000000000000000000000 0000000000000000 03', '0d61b1f16567bb350574b725a08f479007a3d49da0dda093bc48903685d0ee00', ('G1', 76), 96, ('IncidentSettled', ('G0', 0), 'NothingCreated')),
    (452, '4e584344 52 01 003a 11111111111111111111111111111111 000000000000004d 0000000000000061 0c ffffffffffffffffffffffffffffffff ffffffffffffffff 04', '8fe5ff3aecf7b914573fc78452072762681430a2ed9bb71318c118e2ba225229', ('G1', 77), 97, ('IncidentSettled', ('GF', 18446744073709551615), 'NotAdmitted')),
    (459, '4e584344 52 01 003a 11111111111111111111111111111111 000000000000004e 0000000000000062 0c 000102030405060708090a0b0c0d0e0f 0000000000000000 01', '2d0b890abbe20d9d28a09eda6cbb350c8e7d38775ae6dbcf83c732a7a661f9b5', ('G1', 78), 98, ('IncidentSettled', ('G2', 0), 'Confirmed')),
    (466, '4e584344 52 01 0025 000102030405060708090a0b0c0d0e0f 0000000000000000 0000000000000002 01 00000001', 'c73769a961f5df85874608a6d6d3092339969fb180100a19981dc7e4f4ac8f56', ('G2', 0), 2, ('RunStarted', 1)),
)


def _resolve_golden(value):
    if isinstance(value, tuple):
        if len(value) == 2 and value[0] in GOLDEN_GENERATIONS and isinstance(value[1], int):
            return (GOLDEN_GENERATIONS[value[0]], value[1])
        return tuple(_resolve_golden(v) for v in value)
    return value


# ===========================================================================
# 3. Container: header, seal, record blocks (design section 7)
# ===========================================================================

HEADER_MAGIC = b"NXCJ"
SEAL_MAGIC = b"NXCS"
REASON_TAG = {"owner-destroyed": 1, "host-rebooted": 2, "records-malformed": 3,
              "completion-not-recorded": 4, "other": 5}


def header_block(root_id, generation, claim, index, c_pool, capacity, applied=(),
                 created=0, boot=bytes(16)):
    b = bytearray(BLOCK)
    b[0:4] = HEADER_MAGIC
    b[4], b[5], b[6], b[7] = 1, 0x52, 1, 12
    b[8:24] = root_id
    b[24:40] = generation
    b[40:48] = u64(claim)
    b[48:52] = u32(index)
    b[52:56] = u32(c_pool)
    b[56:60] = u32(capacity)
    b[64:72] = u64(created)
    b[72:88] = boot
    b[88] = len(applied)
    off = 92
    for binding, tag in applied:
        b[off:off + 32] = binding
        b[off + 32] = tag
        off += 33
    b[off:off + 32] = sha(b[0:off])
    return bytes(b)


def redigest_header(block):
    """Recompute a header digest after an edit (to build checksum-valid variants)."""
    b = bytearray(block)
    n = b[88]
    end = 92 + 33 * n
    b[end:end + 32] = sha(b[0:end])
    return bytes(b)


def parse_header(block, root_id, c_pool=None, index=None):
    """Return ('zero' | 'checksum-invalid' | 'invalid' | 'valid', detail)."""
    block = bytes(block)
    if not any(block):
        return "zero", None
    n = block[88]
    if block[0:4] != HEADER_MAGIC or n > 32:
        return "checksum-invalid", "magic or count"
    end = 92 + 33 * n
    if sha(block[0:end]) != block[end:end + 32]:
        return "checksum-invalid", "digest"
    problems = []
    if block[4:8] != bytes([1, 0x52, 1, 12]):
        problems.append("version bytes")
    if block[8:24] != root_id:
        problems.append("root id")
    generation = block[24:40]
    if not any(generation):
        problems.append("zero generation")
    claim = be(block[40:48])
    if not 1 <= claim <= MAX_CLAIM:
        problems.append("claim range")
    idx = be(block[48:52])
    if (index is not None and idx != index) or (index is None and idx >= 1024):
        problems.append("pool index")
    cp = be(block[52:56])
    if not 1 <= cp <= 4096 or (c_pool is not None and cp != c_pool):
        problems.append("pool capacity")
    capacity = be(block[56:60])
    if not 1 <= capacity <= cp:
        problems.append("capacity")
    if not MUT.on("NC10a"):
        if any(block[60:64]) or any(block[89:92]):
            problems.append("reserved bytes")
    applied = []
    previous = None
    for k in range(n):
        off = 92 + 33 * k
        binding = block[off:off + 32]
        tag = block[off + 32]
        if tag not in REASON_TAG.values():
            problems.append("reason tag")
        if previous is not None and binding <= previous:
            problems.append("bindings not strictly increasing")
        previous = binding
        applied.append((binding, tag))
    if any(block[end + 32:]):
        problems.append("bytes after the digest")
    if problems:
        return "invalid", problems
    return "valid", {"generation": generation, "claim": claim, "index": idx, "c_pool": cp,
                     "capacity": capacity, "applied": applied, "digest": block[end:end + 32]}


def seal_block(generation, claim, header_digest, records, last_digest, terminal, verdict, by,
               closed=0):
    b = bytearray(BLOCK)
    b[0:4] = SEAL_MAGIC
    b[4] = 1
    b[8:24] = generation
    b[24:32] = u64(claim)
    b[32:64] = header_digest
    b[64:72] = u64(records)
    b[72:104] = last_digest
    b[104:112] = u64(terminal)
    b[112] = verdict
    b[113] = 0 if by is None else 1
    b[114:118] = u32(by or 0)
    b[118:126] = u64(closed)
    b[128:160] = sha(b[0:128])
    return bytes(b)


def redigest_seal(block):
    b = bytearray(block)
    b[128:160] = sha(b[0:128])
    return bytes(b)


def seal_for(header, prefix):
    """The seal the recorder writes once close() returned Ok (section 9.7)."""
    last = prefix[-1].digest if prefix else bytes(32)
    ended = next((r for r in prefix if r.kind[0] == "RunEnded"), None)
    terminal = ended.seq if ended else 0
    verdict = VERDICT[ended.kind[1]] if ended else 0
    by = ended.kind[2] if ended else None
    return seal_block(header["generation"], header["claim"], header["digest"], len(prefix), last,
                      terminal, verdict, by)


def parse_seal(block, header, prefix, summary):
    """Return 'unsealed' | 'unreadable' | 'malformed' | 'sealed'."""
    block = bytes(block)
    if not any(block):
        return "unsealed"
    if block[0:4] != SEAL_MAGIC or sha(block[0:128]) != block[128:160]:
        return "unreadable"
    problems = []
    if block[4] != 1:
        problems.append("version")
    if not MUT.on("NC10b"):
        if any(block[5:8]) or any(block[126:128]) or any(block[160:]):
            problems.append("reserved or tail bytes")
    if block[8:24] != header["generation"] or be(block[24:32]) != header["claim"]:
        problems.append("identity")
    if block[32:64] != header["digest"]:
        problems.append("header digest")
    records = be(block[64:72])
    if records != len(prefix) or records > header["capacity"]:
        problems.append("record count")
    last = prefix[-1].digest if prefix else bytes(32)
    if block[72:104] != last:
        problems.append("last record digest")
    ended = summary["re_record"]
    terminal = ended.seq if ended else 0
    if be(block[104:112]) != terminal:
        problems.append("terminal sequence")
    verdict = VERDICT[ended.kind[1]] if ended else 0
    if block[112] != verdict:
        problems.append("verdict")
    by = ended.kind[2] if ended else None
    if block[113] not in (0, 1) or (block[113] == 1) != (by is not None):
        problems.append("resolved_by presence")
    if be(block[114:118]) != (by or 0):
        problems.append("resolved_by")
    if summary["has_rs"] and not summary["has_re"]:
        problems.append("a started run without RunEnded")
    if summary["unsettled_actions"] or summary["unsettled_incidents"]:
        problems.append("recorded unsettled entries")
    return "malformed" if problems else "sealed"


def record_block(frame):
    return bytes(frame) + bytes(BLOCK - len(frame))


def parse_record_block(block):
    """Return ('zero' | 'invalid' | 'valid', detail)."""
    block = bytes(block)
    if not any(block):
        return "zero", None
    length = be(block[6:8])
    if not MIN_RECORD_PAYLOAD <= length <= MAX_RECORD_PAYLOAD:
        return "invalid", "payload length"
    end = 8 + length + 32
    try:
        record = decode_record(block[0:end])
    except CodecError as error:
        return "invalid", str(error)
    if not MUT.on("NC10c") and any(block[end:]):
        return "invalid", "padding"
    return "valid", record


# ===========================================================================
# 4. Record grammar (design section 11.4)
# ===========================================================================


def _new_grammar_state():
    return {"at": 0, "rs": False, "re": False, "re_record": None, "ac": False, "sr": False,
            "case": None, "case_failed": False, "case_actions": [], "actions": {},
            "next_action": 1, "incidents": {}, "next_incident": 1, "rr": 0, "ra": 0,
            "ra_true": 0, "last_ra_true": None, "io_after_re": False}


def _all_settled(s):
    return (all(a["settled"] is not None for a in s["actions"].values())
            and all(x["settled"] is not None for x in s["incidents"].values()))


def _grammar_step(s, r, generation, header_n):
    kind = r.kind
    name = kind[0]
    if r.at < s["at"]:
        return "G1"
    s["at"] = r.at
    if s["re"] and name not in ("IncidentOpened", "IncidentSettled", "RecoveryAttempt",
                                "ShutdownRefused"):
        return "G17"
    if not s["rs"] and name not in ("RunStarted", "AdmissionClosed", "ShutdownRefused"):
        return "G3"

    def own(identity):
        return identity[0] == generation

    if name == "RunStarted":
        if s["rs"] or s["ac"] or kind[1] != header_n:
            return "G2"
        s["rs"] = True
    elif name == "CaseStarted":
        if s["case"] is not None or s["ac"] or s["rr"] > 0 or not _all_settled(s):
            return "G4"
        s["case"], s["case_failed"], s["case_actions"] = kind[1], False, []
    elif name == "CaseEnded":
        if s["case"] is None or kind[1] != s["case"]:
            return "G5"
        if kind[2] and (s["case_failed"] or any(s["actions"][a]["settled"] is None
                                                for a in s["case_actions"])):
            return "G5"
        s["case"] = None
    elif name == "ActionStarted":
        if not own(kind[1]):
            return "G-id"
        if s["case"] is None or s["ac"] or kind[1][1] != s["next_action"]:
            return "G6"
        s["actions"][kind[1][1]] = {"failed": False, "settled": None}
        s["case_actions"].append(kind[1][1])
        s["next_action"] += 1
    elif name == "ActionFailed":
        if not own(kind[1]):
            return "G-id"
        a = s["actions"].get(kind[1][1])
        if a is None or a["failed"] or a["settled"] is not None:
            return "G7"
        a["failed"] = True
        if s["case"] is not None:
            s["case_failed"] = True
    elif name == "ActionSettled":
        if not own(kind[1]):
            return "G-id"
        a = s["actions"].get(kind[1][1])
        if a is None or a["settled"] is not None:
            return "G8"
        if kind[2] == "NotAdmitted" and a["failed"]:
            return "G8"
        a["settled"] = kind[2]
    elif name == "AdmissionClosed":
        if s["ac"] or kind[2] > s["next_action"] - 1:
            return "G9"
        s["ac"] = True
        if kind[1][0] == "Failed" and s["case"] is not None:
            s["case_failed"] = True
    elif name == "ShutdownRefused":
        if s["sr"]:
            return "G10"
        if not s["rs"] and not s["ac"]:
            return "G3"
        s["sr"] = True
    elif name == "RecoveryRequired":
        if s["case"] is not None:
            return "G11"
        s["rr"] += 1
    elif name == "RecoveryAttempt":
        if kind[1] != s["ra"] + 1:
            return "G12"
        if not s["re"] and s["rr"] <= s["ra_true"]:
            return "G12"
        if s["re"] and not s["io_after_re"]:
            return "G12"
        s["ra"] += 1
        if kind[2] and not s["re"]:
            s["ra_true"] += 1
            s["last_ra_true"] = kind[1]
    elif name == "RunEnded":
        if s["case"] is not None or not _all_settled(s) or kind[1] not in ("Passed", "Failed"):
            return "G15"
        expected_by = None if s["rr"] == 0 else s["last_ra_true"]
        if kind[2] != expected_by:
            return "G15"
        if not MUT.on("NC13") and (kind[1] == "Failed") != s["ac"]:
            return "G16"
        s["re"], s["re_record"] = True, r
    elif name == "IncidentOpened":
        if not own(kind[1]) or not own(kind[2]):
            return "G-id"
        if kind[1][1] != s["next_incident"] or kind[2][1] not in s["actions"]:
            return "G13"
        s["incidents"][kind[1][1]] = {"settled": None}
        s["next_incident"] += 1
        if s["re"]:
            s["io_after_re"] = True
    elif name == "IncidentSettled":
        if not own(kind[1]):
            return "G-id"
        x = s["incidents"].get(kind[1][1])
        if x is None or x["settled"] is not None or kind[2] not in ("Confirmed", "OutputLost"):
            return "G14"
        x["settled"] = kind[2]
    return None


def check_grammar(records, generation, header_n):
    """Return (None | (rule, index), summary)."""
    s = _new_grammar_state()
    violation = None
    for i, record in enumerate(records):
        rule = _grammar_step(s, record, generation, header_n)
        if rule:
            violation = (rule, i)
            break
    summary = {
        "has_rs": s["rs"], "has_re": s["re"], "re_record": s["re_record"],
        "has_as": s["next_action"] > 1,
        "unsettled_actions": sorted(k for k, v in s["actions"].items() if v["settled"] is None),
        "unsettled_incidents": sorted(k for k, v in s["incidents"].items()
                                      if v["settled"] is None),
    }
    return violation, summary


# ===========================================================================
# 5. Classification and the restart report (design section 11)
# ===========================================================================

MALFORMED = ("MalformedJournal", "MalformedPoolFile")


def refusal(report):
    cls = report["class"]
    if MUT.on("NC09"):
        # First-candidate-era shortcut: refuse only on recorded outstanding work.
        return cls in MALFORMED or bool(report["unsettled"])
    if MUT.on("NC16") and cls == "UnsealedAction" and report["summary"]["has_re"]:
        # Wrong heuristic: a visible RunEnded taken as resolution.
        return False
    return cls in MALFORMED or cls == "UnsealedAction"


def _finish_report(rep):
    cls = rep["class"]
    summary = rep.get("summary") or {}
    rep["unsettled"] = [("action", a) for a in summary.get("unsettled_actions", [])] + \
        [("incident", i) for i in summary.get("unsettled_incidents", [])]
    rep["evidence_complete"] = cls == "Sealed"
    rep["native_work_possible"] = cls in MALFORMED or bool(summary.get("has_as"))
    rep["refused"] = refusal(rep)
    notes = []
    if cls == "UnsealedAction":
        notes.append("late owners or other native effects may exist with no durable record")
        notes.append("an owner surfacing after close() is outside custody")
    if cls == "AbandonedClaim":
        notes.append("interrupted claim; this does not prove nothing happened in domains S or A")
    rep["notes"] = notes
    return rep


def classify(data, root_id, c_pool, index):
    """Classify one pool file's bytes (design section 11.1)."""
    data = bytes(data)
    rep = {"class": None, "reason": "", "header": None, "prefix": [], "seal": None,
           "summary": None, "content": sha(data)}
    if len(data) != (c_pool + 2) * BLOCK:
        rep["class"], rep["reason"] = "SizeInvalid", "size"
        return _finish_report(rep)
    if not any(data):
        rep["class"] = "Unused"
        return _finish_report(rep)
    status, detail = parse_header(data[0:BLOCK], root_id, c_pool, index)
    rest_zero = not any(data[BLOCK:])
    if status != "valid":
        if status == "checksum-invalid" and rest_zero:
            rep["class"] = "AbandonedClaim"
        elif status == "invalid" and rest_zero and MUT.on("NC10d"):
            rep["class"] = "AbandonedClaim"
        else:
            rep["class"], rep["reason"] = "MalformedPoolFile", f"header {status}"
        return _finish_report(rep)
    header = detail
    rep["header"] = header
    prefix = []
    ended = False
    for n in range(1, c_pool + 1):
        blk = data[(n + 1) * BLOCK:(n + 2) * BLOCK]
        if n > header["capacity"]:
            if any(blk):
                rep["class"], rep["reason"] = "MalformedJournal", "block beyond capacity"
                return _finish_report(rep)
            continue
        kind, record = parse_record_block(blk)
        if kind == "zero":
            ended = True
            continue
        if ended:
            rep["class"], rep["reason"] = "MalformedJournal", "hole"
            return _finish_report(rep)
        if kind == "invalid":
            rep["class"], rep["reason"] = "MalformedJournal", f"record block {n}: {record}"
            return _finish_report(rep)
        if record.gen != header["generation"] or record.seq != n:
            rep["class"], rep["reason"] = "MalformedJournal", f"record identity {n}"
            return _finish_report(rep)
        prefix.append(record)
    rep["prefix"] = prefix
    violation, summary = check_grammar(prefix, header["generation"], len(header["applied"]))
    rep["summary"] = summary
    if violation:
        rep["class"], rep["reason"] = "MalformedJournal", f"grammar {violation[0]}"
        return _finish_report(rep)
    seal = parse_seal(data[BLOCK:2 * BLOCK], header, prefix, summary)
    rep["seal"] = seal
    if seal == "malformed":
        rep["class"], rep["reason"] = "MalformedJournal", "seal"
    elif seal == "sealed":
        rep["class"] = "Sealed"
    elif summary["has_as"]:
        rep["class"] = "UnsealedAction"
    else:
        rep["class"] = "UnsealedNoAction"
    return _finish_report(rep)


_CLASSIFY_CACHE = {}


def classify_cached(data, root_id, c_pool, index):
    key = (sha(data), root_id, c_pool, index, MUT.active)
    if key not in _CLASSIFY_CACHE:
        _CLASSIFY_CACHE[key] = classify(data, root_id, c_pool, index)
    return _CLASSIFY_CACHE[key]


# ===========================================================================
# 6. Bindings (design section 12.3)
# ===========================================================================

BINDING_PREFIX = b"nexus-phase2-custody-incident\x00\x01"


def bind_journal(root_id, claim, generation, content, cls, location=None):
    data = BINDING_PREFIX + b"\x01" + root_id + u64(claim) + generation
    if not MUT.on("NC11b"):
        data += content
    data += bytes([1 if cls == "unresolved" else 2])
    if MUT.on("NC11a"):
        data += repr(location).encode()
    return sha(data)


def bind_gap(root_id, claim):
    return sha(BINDING_PREFIX + b"\x02" + root_id + u64(claim))


def bind_pool(root_id, index, content):
    return sha(BINDING_PREFIX + b"\x03" + root_id + u32(index) + content)


# ===========================================================================
# 7. Text grammars: PROVISION and dispositions (design section 7.5)
# ===========================================================================


class ParseError(Exception):
    pass


_DEC = re.compile(r"0|[1-9][0-9]{0,19}")
_HEX32 = re.compile(r"[0-9a-f]{32}")
_HEX64 = re.compile(r"[0-9a-f]{64}")
_OPTS = re.compile(r"[\x21-\x7e]{1,1024}")
_TEXT = re.compile(r"[\x20-\x7e]+")
_TS = re.compile(r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z")
_COMPONENT = re.compile(r"[A-Za-z0-9._-]{1,64}")


def _dec(value, lo=0, hi=U64_MAX):
    if MUT.on("NC10e"):
        if not value.isdigit():
            raise ParseError("not a decimal")
    elif not _DEC.fullmatch(value):
        raise ParseError("non-canonical decimal")
    number = int(value)
    if not lo <= number <= hi:
        raise ParseError("decimal out of range")
    return number


def _statement(value, limit=512):
    if not 1 <= len(value) <= limit or not _TEXT.fullmatch(value) or value != value.strip(" "):
        raise ParseError("bad statement")
    return value


def _timestamp(value):
    if not _TS.fullmatch(value):
        raise ParseError("bad timestamp")
    try:
        moment = datetime.datetime.strptime(value, "%Y-%m-%dT%H:%M:%SZ")
    except ValueError as error:
        raise ParseError("bad timestamp") from error
    if not 1970 <= moment.year <= 9999:
        raise ParseError("timestamp range")
    return value


def _path(value):
    parts = value.split("/")
    if len(value) > 255 or parts[0] != "" or len(parts) < 2:
        raise ParseError("bad path")
    for part in parts[1:]:
        if part in (".", "..") or not _COMPONENT.fullmatch(part):
            raise ParseError("bad path component")
    return value


def _lines(data, limit):
    if len(data) > limit:
        raise ParseError("too large")
    try:
        text = bytes(data).decode("ascii")
    except UnicodeDecodeError as error:
        raise ParseError("not ASCII") from error
    if not text.endswith("\n") or "\r" in text or "\x00" in text:
        raise ParseError("line endings")
    return text, text[:-1].split("\n")


def _fields(lines, keys):
    out = {}
    if len(lines) != len(keys):
        raise ParseError("field count")
    for line, key in zip(lines, keys):
        if not line.startswith(key + "="):
            raise ParseError(f"expected {key}")
        out[key] = line[len(key) + 1:]
    return out


PROV_HEAD = ["uid", "gid", "root-id", "state-root", "root-inode", "lock-inode", "journals-inode",
             "dispositions-inode", "revoked-inode", "archive-inode", "mount-fstype",
             "mount-options", "super-options", "fs-block-size", "page-size",
             "device-logical-block-size", "device-physical-block-size", "kernel",
             "storage-class", "storage-pci-function", "storage-partition", "storage-identity",
             "storage-attestation", "pool", "pool-capacity"]
PROV_TAIL = ["retired-through", "predecessor", "predecessor-statement", "operator", "created",
             "revision"]


def provision_text(p):
    lines = ["nexus-phase2-custody-provision 1"]
    head = {
        "uid": p["uid"], "gid": p["gid"], "root-id": p["root_id"].hex(),
        "state-root": p["state_root"], "root-inode": p["root_inode"],
        "lock-inode": p["lock_inode"], "journals-inode": p["journals_inode"],
        "dispositions-inode": p["dispositions_inode"], "revoked-inode": p["revoked_inode"],
        "archive-inode": p["archive_inode"], "mount-fstype": "ext4",
        "mount-options": p["mount_options"], "super-options": p["super_options"],
        "fs-block-size": 4096, "page-size": 4096, "device-logical-block-size": 512,
        "device-physical-block-size": 4096, "kernel": p["kernel"],
        "storage-class": p["storage_class"], "storage-pci-function": p["storage_pci"],
        "storage-partition": "none" if p["storage_partition"] is None else p["storage_partition"],
        "storage-identity": p["storage_identity"],
        "storage-attestation": p["attestation"],
        "pool": len(p["pool_inodes"]), "pool-capacity": p["c_pool"],
    }
    lines += [f"{key}={head[key]}" for key in PROV_HEAD]
    lines += [f"pool-{i:05d}-inode={ino}" for i, ino in enumerate(p["pool_inodes"])]
    tail = {"retired-through": p["retired"],
            "predecessor": p["predecessor"].hex() if p["predecessor"] else "none",
            "predecessor-statement": p["predecessor_statement"] or "none",
            "operator": p["operator"], "created": p["created"], "revision": p["revision"]}
    lines += [f"{key}={tail[key]}" for key in PROV_TAIL]
    body = ("\n".join(lines) + "\n").encode("ascii")
    return body + f"digest={hashlib.sha256(body).hexdigest()}\n".encode("ascii")


def parse_provision(data):
    text, lines = _lines(data, 65536)
    if not lines or lines[0] != "nexus-phase2-custody-provision 1":
        raise ParseError("version line")
    head = _fields(lines[1:1 + len(PROV_HEAD)], PROV_HEAD)
    pool = _dec(head["pool"], 1, 1024)
    c_pool = _dec(head["pool-capacity"], 1, 4096)
    start = 1 + len(PROV_HEAD)
    pool_keys = [f"pool-{i:05d}-inode" for i in range(pool)]
    pools = _fields(lines[start:start + pool], pool_keys)
    tail = _fields(lines[start + pool:start + pool + len(PROV_TAIL)], PROV_TAIL)
    rest = lines[start + pool + len(PROV_TAIL):]
    if len(rest) != 1 or not rest[0].startswith("digest="):
        raise ParseError("digest line")
    body = text[:text.rindex("digest=")]
    if rest[0][len("digest="):] != hashlib.sha256(body.encode("ascii")).hexdigest():
        raise ParseError("digest mismatch")
    if not _HEX32.fullmatch(head["root-id"]):
        raise ParseError("root id")
    if head["mount-fstype"] != "ext4" or head["fs-block-size"] != "4096" or \
            head["page-size"] != "4096":
        raise ParseError("filesystem profile")
    for key in ("mount-options", "super-options"):
        if not _OPTS.fullmatch(head[key]):
            raise ParseError("option string")
    for key in ("device-logical-block-size", "device-physical-block-size"):
        size = _dec(head[key], 1, 4096)
        if 4096 % size:
            raise ParseError("device block size")
    predecessor = tail["predecessor"]
    statement = tail["predecessor-statement"]
    if predecessor == "none":
        if statement != "none":
            raise ParseError("predecessor statement without predecessor")
    else:
        if not _HEX32.fullmatch(predecessor):
            raise ParseError("predecessor")
        _statement(statement)
    _statement(head["kernel"])
    if head["storage-class"] != STORAGE_CLASS:
        raise ParseError("storage class")
    if not _PCI_FUNCTION.fullmatch(head["storage-pci-function"]):
        raise ParseError("storage PCI function")
    partition = None if head["storage-partition"] == "none" else \
        _dec(head["storage-partition"], 1, 255)
    if not _HEX64.fullmatch(head["storage-identity"]):
        raise ParseError("storage identity")
    _statement(head["storage-attestation"])
    _statement(tail["operator"], 64)
    _timestamp(tail["created"])
    return {
        "uid": _dec(head["uid"], 0, U32_MAX), "gid": _dec(head["gid"], 0, U32_MAX),
        "root_id": bytes.fromhex(head["root-id"]), "state_root": _path(head["state-root"]),
        "root_inode": _dec(head["root-inode"]), "lock_inode": _dec(head["lock-inode"]),
        "journals_inode": _dec(head["journals-inode"]),
        "dispositions_inode": _dec(head["dispositions-inode"]),
        "revoked_inode": _dec(head["revoked-inode"]),
        "archive_inode": _dec(head["archive-inode"]),
        "mount_options": head["mount-options"], "super_options": head["super-options"],
        "kernel": head["kernel"], "storage_class": head["storage-class"],
        "storage_pci": head["storage-pci-function"], "storage_partition": partition,
        "storage_identity": head["storage-identity"], "pool": pool, "c_pool": c_pool,
        "pool_inodes": [_dec(pools[k]) for k in pool_keys],
        "retired": _dec(tail["retired-through"], 0, MAX_CLAIM),
        "revision": _dec(tail["revision"], 1),
        "predecessor": None if predecessor == "none" else bytes.fromhex(predecessor),
    }


DISP_KEYS = ["root", "binding", "kind", "claim", "generation", "pool-index", "content", "class",
             "recorded-unsettled", "reason", "statement", "operator", "at"]


def disposition_text(d):
    values = {
        "root": d["root"].hex(), "binding": d["binding"].hex(), "kind": d["kind"],
        "claim": "none" if d["claim"] is None else d["claim"],
        "generation": "none" if d["generation"] is None else d["generation"].hex(),
        "pool-index": "none" if d["pool_index"] is None else d["pool_index"],
        "content": "none" if d["content"] is None else d["content"].hex(),
        "class": d["class"], "recorded-unsettled": d["unsettled"], "reason": d["reason"],
        "statement": d["statement"], "operator": d["operator"], "at": d["at"],
    }
    lines = ["nexus-phase2-custody-disposition 1"] + [f"{k}={values[k]}" for k in DISP_KEYS]
    return ("\n".join(lines) + "\n").encode("ascii")


def disposition_binding(d):
    if d["kind"] == "journal":
        return bind_journal(d["root"], d["claim"], d["generation"], d["content"], d["class"],
                            location="disposition")
    if d["kind"] == "claim-gap":
        return bind_gap(d["root"], d["claim"])
    return bind_pool(d["root"], d["pool_index"], d["content"])


def parse_disposition(data):
    _, lines = _lines(data, 4096)
    if not lines or lines[0] != "nexus-phase2-custody-disposition 1":
        raise ParseError("version line")
    f = _fields(lines[1:], DISP_KEYS)
    if not _HEX32.fullmatch(f["root"]) or not _HEX64.fullmatch(f["binding"]):
        raise ParseError("root or binding")
    kind = f["kind"]
    if kind not in ("journal", "claim-gap", "pool-file"):
        raise ParseError("kind")
    shape = {"journal": ("dec", "hex", "none", "hex"), "claim-gap": ("dec", "none", "none", "none"),
             "pool-file": ("none", "none", "dec", "hex")}[kind]
    values = []
    for key, form in zip(("claim", "generation", "pool-index", "content"), shape):
        raw = f[key]
        if form == "none":
            if raw != "none":
                raise ParseError(f"{key} must be none")
            values.append(None)
        elif form == "dec":
            values.append(_dec(raw, 0, MAX_CLAIM if key == "claim" else 1023))
        else:
            pattern = _HEX32 if key == "generation" else _HEX64
            if not pattern.fullmatch(raw):
                raise ParseError(key)
            values.append(bytes.fromhex(raw))
    if f["class"] not in ("unresolved", "malformed") or \
            (kind != "journal" and f["class"] != "malformed"):
        raise ParseError("class")
    unsettled = _dec(f["recorded-unsettled"])
    if kind != "journal" and unsettled != 0:
        raise ParseError("recorded-unsettled")
    if f["reason"] not in REASON_TAG:
        raise ParseError("reason")
    _statement(f["statement"])
    _statement(f["operator"], 64)
    _timestamp(f["at"])
    d = {"root": bytes.fromhex(f["root"]), "binding": bytes.fromhex(f["binding"]), "kind": kind,
         "claim": values[0], "generation": values[1], "pool_index": values[2],
         "content": values[3], "class": f["class"], "unsettled": unsettled,
         "reason": f["reason"]}
    if disposition_binding(d) != d["binding"]:
        raise ParseError("binding does not recompute")
    return d


# ===========================================================================
# 8. Simulated storage (design sections 4 and 4.8)
# ===========================================================================


class SimError(Exception):
    def __init__(self, code):
        super().__init__(code)
        self.code = code


class Blocked(Exception):
    """A blocking open of a FIFO would wait for a writer forever."""


class Inode:
    def __init__(self, ino, kind, uid, gid, mode):
        self.ino, self.kind, self.uid, self.gid, self.mode = ino, kind, uid, gid, mode
        self.K = bytearray()        # kernel-visible content
        self.D = bytearray()        # durable content
        self.pending = set()        # 4096-byte blocks not yet written back
        self.err = 0                # writeback-error sequence: count ...
        self.seen = True            # ... and whether the newest error was reported
        self.ents_K = {}            # directories: visible entries
        self.ents_D = {}            # directories: durable entries
        self.dev = FIXTURE_DEV      # st_dev: the filesystem holding the inode


class OFD:
    def __init__(self, inode, writable, cursor, serial):
        self.inode, self.writable, self.cursor = inode, writable, cursor
        self.serial = serial        # stable identity: the lock table survives a world clone
        self.refs = 0
        self.open = True


class FD:
    def __init__(self, ofd, proc, owner):
        self.ofd, self.proc, self.owner = ofd, proc, owner
        self.closed = False


class Proc:
    def __init__(self, name):
        self.name = name
        self.fds = []


Stat = collections.namedtuple("Stat", "kind uid gid mode ino nlink size")

# One directory effect of one metadata operation: an entry added (name -> ino)
# or removed. A rename contributes two halves with the same operation index.
Half = collections.namedtuple("Half", "op dir name ino add")


class SimFS:
    """Files: K (kernel-visible), D (durable), pending blocks, errseq.

    Metadata (section 4.8): create, link, unlink and rename change the visible
    entries at once and append halves, in issue order, to a pending log.
    fsync_dir(d) guarantees the halves of d issued so far. At power loss a
    schedule decides which pending halves became durable:

    ("ordered", c): every half of the first c operations, c at least one past
    the last guaranteed operation. Atomic operations made durable in issue
    order: the supported ext4 profile's journal order (assumption A-M1). It
    is a superset of ext4's outcomes (it also lets a create stay non-durable
    after an fsync of the file itself, which POSIX permits).

    ("perdir", {dir: k}): an independent prefix of each directory's halves,
    at least its guaranteed prefix. A conservative over-approximation that
    also splits a cross-directory rename; it is not ext4 behaviour.

    R3 adds the journal behaviour a new descriptor's sync depends on (design
    section 10.6, Linux v6.17): sync_dir() is fsync(2) of a directory through a
    new O_RDONLY descriptor; touch() is futimens(2) of a file the caller owns;
    fsync_file() is fsync(2) of a regular file. abort_journal() is a commit that
    failed in the commit thread: every metadata operation not yet guaranteed is
    lost, and nothing reports it until a journal handle is started.

    R4 corrects fsync_file() (design section 5.1, A-M2): the probe's
    transaction is running or completed (probe_txn), and only a running one is
    waited for and followed by the abort test. It adds an abort that lands
    inside the probe's handle start, after ext4's test and before jbd2's
    (abort_in_probe_handle), and the background commit of the probe's
    transaction (commit_probe()).

    R5 adds the storage below the filesystem (design sections 5.5 to 5.8):
    `device` is "stable" (admitted storage: a completed write is stable),
    "volatile" (a volatile write-back cache that receives flushes) or
    "unflushed" (a volatile cache the kernel was told is absent, so no flush is
    sent). checkpoint() writes the committed metadata home and moves the log
    tail, following JournalSim's transitions; on a cache, power_loss() returns a
    released home write to the older content still on the medium.
    """

    def __init__(self):
        self.inodes = {}
        self.next_ino = 2
        self.locks = {}
        self.log = []
        self.pwrite_plan = []
        self.sync_plan = []
        self.wb_fail = set()
        self._procs = {}
        self.op_count = 0
        self.pending_meta = []
        self.guaranteed = set()     # indexes into pending_meta
        self.trace = None           # a list while parity is being traced
        self.step = None            # the protocol step being executed
        self.journal_aborted = False    # jbd2 abort: permanent until the next mount
        self.emergency_ro = False       # EXT4_FLAGS_EMERGENCY_RO: ext4 noticed the abort
        self.dir_sync_plan = []         # injected results of sync_dir(): "EINTR", "EIO"
        self.next_ofd = 0               # serials of open file descriptions
        self.probe_txn = None           # the probe's transaction: None, "running" or "completed"
        self.abort_in_probe_handle = False  # injected: the abort lands between ext4's and jbd2's test
        self.device = "stable"          # R5: "stable", "volatile" or "unflushed" (design section 5.5)
        self.home_stable = None         # R5: directory entries the medium's home blocks hold, once tracked
        self.home_cached = None         # R5: home writes completed into a volatile cache, not yet stable
        self.released = False           # R5: the tail moved past metadata whose home writes are not stable
        self.root = self.mkinode("dir", 0, 0, 0o755)

    # -- processes -------------------------------------------------------
    def register(self, proc):
        self._procs[proc.name] = proc
        return proc

    def procs(self):
        return list(self._procs.values())

    # -- namespace -------------------------------------------------------
    def mkinode(self, kind, uid, gid, mode):
        inode = Inode(self.next_ino, kind, uid, gid, mode)
        self.inodes[inode.ino] = inode
        self.next_ino += 1
        return inode

    def nlink(self, ino):
        return sum(list(d.ents_K.values()).count(ino) for d in self.inodes.values()
                   if d.kind == "dir")

    def stat_inode(self, inode):
        return Stat(inode.kind, inode.uid, inode.gid, inode.mode, inode.ino,
                    self.nlink(inode.ino), len(inode.K))

    def stat(self, directory, name, proc=None):
        if proc is not None:
            self.log.append((proc.name, "fstatat", name))
        ino = directory.ents_K.get(name)
        return None if ino is None else self.stat_inode(self.inodes[ino])

    def _record(self, op, directory, name):
        if self.trace is not None:
            self.trace.append((self.step, op, directory.ino if directory else None, name))

    def _meta(self, halves):
        op = self.op_count
        self.op_count += 1
        for directory, name, ino, add in halves:
            self.pending_meta.append(Half(op, directory.ino, name, ino, add))

    def _check_journal(self):
        """A metadata change needs a journal handle (ext4_journal_check_start)."""
        if self.emergency_ro:
            raise SimError("EROFS")
        if self.journal_aborted:
            self.emergency_ro = True        # ext4_abort: "Detected aborted journal"
            raise SimError("EROFS")

    def create(self, directory, name, kind, uid, gid, mode, size=0):
        self._check_journal()
        if name in directory.ents_K:
            raise SimError("EEXIST")
        inode = self.mkinode(kind, uid, gid, mode)
        inode.dev = directory.dev
        if size:
            # fallocate: visible zeros; nothing durable until a sync.
            inode.K = bytearray(size)
            inode.pending = set(range((size + BLOCK - 1) // BLOCK))
        directory.ents_K[name] = inode.ino
        self._meta([(directory, name, inode.ino, True)])
        self._record("mkdir" if kind == "dir" else "create", directory, name)
        return inode

    def link(self, directory, name, ino):
        self._check_journal()
        if name in directory.ents_K:
            raise SimError("EEXIST")
        directory.ents_K[name] = ino
        self._meta([(directory, name, ino, True)])
        self._record("link", directory, name)

    def unlink(self, directory, name):
        self._check_journal()
        ino = directory.ents_K.pop(name)
        self._meta([(directory, name, ino, False)])
        self._record("unlink", directory, name)

    def rename(self, src, sname, dst, dname):
        self._check_journal()
        ino = src.ents_K.pop(sname)
        dst.ents_K[dname] = ino
        self._meta([(src, sname, ino, False), (dst, dname, ino, True)])
        self._record("rename", dst, dname)

    def fsync_dir(self, directory):
        for i, half in enumerate(self.pending_meta):
            if half.dir == directory.ino:
                self.guaranteed.add(i)
        self._record("fsync_dir", directory, None)
        self._compact()
        if self.device == "volatile":
            self.flush_ok()             # R5: the commit record's PREFLUSH (commit.c:152-154)

    def _compact(self):
        """Every pending half guaranteed: durable under every schedule."""
        if self.pending_meta and len(self.guaranteed) == len(self.pending_meta):
            self._apply_halves(self.pending_meta)
            self.pending_meta = []
            self.guaranteed = set()

    def _apply_halves(self, halves):
        for half in sorted(halves, key=lambda h: h.op):
            directory = self.inodes[half.dir]
            if half.add:
                directory.ents_D[half.name] = half.ino
            elif directory.ents_D.get(half.name) == half.ino:
                del directory.ents_D[half.name]

    # -- syncs through a new descriptor (design section 10.6) -------------
    def sync_dir(self, proc, directory, read_only=False):
        """fsync(2) of a directory through a new O_RDONLY descriptor, as ext4
        does it at Linux v6.17: an emergency state fails at once; a read-only
        superblock returns 0 having committed nothing; otherwise a full commit,
        which returns 0 with nothing committed when an earlier commit already
        failed silently (nothing is pending), and EIO when it waited for a
        transaction of an aborted journal."""
        self.log.append((proc.name, "fsync_dir", self._name_of(directory)))
        if self.trace is not None:
            self.trace.append((self.step, "sync_dir", directory.ino, None))
        if self.dir_sync_plan:
            planned = self.dir_sync_plan.pop(0)
            if planned is not None:
                return planned
        if self.emergency_ro:
            return "EROFS"
        if read_only:
            return 0
        if self.journal_aborted:
            return 0
        self.fsync_dir(directory)
        return 0

    def touch(self, fd, read_only=False):
        """futimens(fd, NULL) by the file's owner: notify_change -> ext4_setattr
        (which returns an emergency state first) -> mark_inode_dirty ->
        ext4_dirty_inode, which starts a journal handle and swallows its error.
        ext4_journal_check_start notices an aborted journal and forces emergency
        read-only. R4: an abort landing after that test fails only jbd2's
        start_this_handle, which sets nothing; the inode keeps its older sync tid,
        so probe_txn is left as it was."""
        self.log.append((fd.proc.name, "futimens", self._name_of(fd.ofd.inode)))
        if self.emergency_ro or read_only:
            return "EROFS"
        if self.journal_aborted:
            self.emergency_ro = True        # ext4_abort; futimens itself still returns 0
            return 0
        if self.abort_in_probe_handle:
            self.abort_journal()            # between the two tests: no emergency state
            return 0
        self.probe_txn = "running"
        return 0

    def fsync_file(self, fd, read_only=False):
        """fsync(2) of a regular file (R4, design section 5.1). ext4_sync_file
        returns an emergency state first and nothing for a read-only superblock.
        Otherwise jbd2_complete_transaction for the inode's sync tid: a running
        transaction is committed and waited for, and the abort flag is tested
        after the wait; a completed one returns 0 without that test, and ext4
        then issues and checks its own cache flush, which succeeds (A-S5)."""
        self.log.append((fd.proc.name, "fsync", self._name_of(fd.ofd.inode)))
        if self.emergency_ro:
            return "EROFS"
        if read_only:
            return 0
        if MUT.on("NC-FSYNC-COMPLETED"):
            # R3's model: every aborted, unnoticed journal reported, whatever the
            # state of the inode's transaction.
            if self.journal_aborted:
                return "EIO"
            self.probe_txn = "completed" if self.probe_txn else None
            return 0
        if self.probe_txn == "running":
            self.probe_txn = "completed"
            return "EIO" if self.journal_aborted else 0
        return 0

    def commit_probe(self, fail=False):
        """The probe's running transaction completes in the background (kjournald2
        on its timer, or another task's commit). A failing commit aborts the
        journal before the transaction completes; the probe's own update is lost."""
        if self.probe_txn == "running":
            if fail:
                self.abort_journal()
            self.probe_txn = "completed"

    def abort_journal(self):
        """A commit fails in the commit thread (EIO): the guaranteed operations
        were committed; every other pending operation is lost and stays visible
        only until power loss; the journal is aborted; ext4 has not noticed."""
        kept = [h for i, h in enumerate(self.pending_meta) if i in self.guaranteed]
        self._apply_halves(kept)
        self.pending_meta = []
        self.guaranteed = set()
        self.journal_aborted = True

    # -- R5: the storage below the journal (design sections 5.5 to 5.8) ----
    def mark_home_stable(self):
        """The medium's home blocks now hold every durable directory entry."""
        self.home_stable = {ino: dict(d.ents_D) for ino, d in self.inodes.items() if d.kind == "dir"}
        self.home_cached = None
        self.released = False

    def flush_ok(self):
        """A cache flush the device completed: every home write completed before it is
        stable (writeback_cache_control.rst:23-27). Only a volatile cache receives one."""
        if self.device == "volatile" and self.home_cached is not None:
            self.home_stable = self.home_cached
            self.home_cached = None
            self.released = False

    def checkpoint(self, home="ok", flush="ok", sb="ok", rewrite="ok", ambiguous="new"):
        """A jbd2 checkpoint of the committed metadata, in the whole model; JournalSim
        models each step in detail and check C31 compares the two. The home writes
        complete with success, with a reported error, or not yet ("inflight"); the
        flush before the tail moves is discarded (checkpoint.c:338-339) and reaches only
        a volatile cache; the superblock's tail write and the abort's rewrite of it may
        fail (journal.c:1827-1837, 2592-2595). Returns 0 or "EIO"."""
        if self.journal_aborted:
            return "EIO"                        # checkpoint.c:323-324
        if home == "inflight":
            return 0                            # checkpoint.c:235-248: the tail waits
        if home == "fail":
            self.abort_journal()                # journal.c:1861-1864: before any new tail
            return "EIO"
        snapshot = {ino: dict(d.ents_D) for ino, d in self.inodes.items() if d.kind == "dir"}
        if self.device == "stable":
            self.home_stable = snapshot         # stable when the writes completed (A-S1)
        else:
            assert self.home_stable is not None, "fixture: the medium's home blocks are not tracked"
            self.home_cached = snapshot         # completed into the device's cache
            if self.device == "volatile" and flush == "ok":
                self.flush_ok()
        if sb == "ok":
            released = True
        else:
            self.abort_journal()                # the abort rewrites the in-memory tail
            released = rewrite == "ok" or ambiguous == "new"
        if released and self.home_stable != snapshot:
            self.released = True
        return 0 if sb == "ok" else "EIO"

    # -- descriptors -----------------------------------------------------
    def _sample(self, inode):
        return inode.err - 1 if inode.err and not inode.seen else inode.err

    def open(self, proc, owner, directory, name, writable=False, nonblock=True):
        self.log.append((proc.name, "open", name))
        ino = directory.ents_K.get(name)
        if ino is None:
            raise SimError("ENOENT")
        inode = self.inodes[ino]
        if inode.kind == "symlink":
            raise SimError("ELOOP")
        if inode.kind == "fifo" and not nonblock:
            raise Blocked(name)
        self.next_ofd += 1
        ofd = OFD(inode, writable, self._sample(inode), self.next_ofd)
        return self._fd(ofd, proc, owner)

    def _fd(self, ofd, proc, owner):
        fd = FD(ofd, proc, owner)
        ofd.refs += 1
        proc.fds.append(fd)
        return fd

    def dup(self, fd, proc, owner):
        return self._fd(fd.ofd, proc, owner)

    def close(self, fd):
        if fd.closed:
            return
        fd.closed = True
        fd.proc.fds.remove(fd)
        ofd = fd.ofd
        ofd.refs -= 1
        if ofd.refs == 0:
            ofd.open = False
            self._unlock(ofd)

    def _unlock(self, ofd):
        held = self.locks.get(ofd.inode.ino)
        if held and ofd.serial in held[1]:
            held[1].discard(ofd.serial)
            if not held[1]:
                del self.locks[ofd.inode.ino]

    def flock(self, fd, op):
        """Non-blocking flock; True when granted (flock(2): per open file description)."""
        ofd = fd.ofd
        self.log.append((fd.proc.name, f"flock-{op}", self._name_of(ofd.inode)))
        ino = ofd.inode.ino
        if op == "UN":
            self._unlock(ofd)
            return True
        held = self.locks.get(ino)
        others = set(held[1]) - {ofd.serial} if held else set()
        if op == "EX":
            if others:
                return False
            self.locks[ino] = ["EX", {ofd.serial}]
            return True
        if held and held[0] == "EX" and others:
            return False
        if held and held[0] == "SH":
            held[1].add(ofd.serial)
        else:
            self.locks[ino] = ["SH", {ofd.serial}]
        return True

    def holds(self, fd, mode):
        """Whether this open file description holds the lock in `mode`."""
        held = self.locks.get(fd.ofd.inode.ino)
        return bool(held and not fd.closed and held[0] == mode and fd.ofd.serial in held[1])

    def _where(self, inode):
        for d in self.inodes.values():
            if d.kind == "dir":
                for name, ino in d.ents_K.items():
                    if ino == inode.ino:
                        return d.ino, name
        return None, f"<inode {inode.ino}>"

    def _name_of(self, inode):
        for d in self.inodes.values():
            if d.kind == "dir":
                for name, ino in d.ents_K.items():
                    if ino == inode.ino:
                        return name
        return f"<inode {inode.ino}>"

    # -- data ------------------------------------------------------------
    def pread(self, fd, offset, count):
        self.log.append((fd.proc.name, "pread", self._name_of(fd.ofd.inode)))
        return bytes(fd.ofd.inode.K[offset:offset + count])

    def pwrite(self, fd, offset, data):
        """Returns the count written, or 'EINTR'. The plan injects faults."""
        assert fd.ofd.writable, "pwrite on a read-only description"
        plan = self.pwrite_plan.pop(0) if self.pwrite_plan else ("ok",)
        if plan[0] == "eintr":
            return "EINTR"
        if plan[0] == "zero":
            return 0
        count = len(data) if plan[0] == "ok" else min(plan[1], len(data))
        inode = fd.ofd.inode
        end = offset + count
        if end > len(inode.K):
            inode.K.extend(bytes(end - len(inode.K)))
        inode.K[offset:end] = data[:count]
        if count:
            for b in range(offset // BLOCK, (end - 1) // BLOCK + 1):
                inode.pending.add(b)
        if self.trace is not None and fd.proc.name == "root":
            self.trace.append((self.step, "write") + self._where(inode))
        return count

    def writeback(self, inode, block, ok=True):
        """Background or sync writeback of one pending block."""
        inode.pending.discard(block)
        if ok:
            lo, hi = block * BLOCK, min((block + 1) * BLOCK, len(inode.K))
            if len(inode.D) < hi:
                inode.D.extend(bytes(hi - len(inode.D)))
            inode.D[lo:hi] = inode.K[lo:hi]
        else:
            inode.err += 1
            inode.seen = False

    def fdatasync(self, fd):
        """Returns 0, 'EIO', 'EINTR' or 'EROFS' (design sections 4.2 and 10.6).
        Overwrites of written, preallocated extents need no metadata, so an
        unnoticed journal abort does not stop the data from becoming durable."""
        inode = fd.ofd.inode
        self.log.append((fd.proc.name, "fdatasync", self._name_of(inode)))
        if self.trace is not None and fd.proc.name == "root":
            self.trace.append((self.step, "fsync") + self._where(inode))
        if self.emergency_ro:
            return "EROFS"
        plan = self.sync_plan.pop(0) if self.sync_plan else ("ok",)
        if plan[0] == "eintr":
            return "EINTR"
        for block in sorted(inode.pending):
            self.writeback(inode, block, (inode.ino, block) not in self.wb_fail)
        if len(inode.D) != len(inode.K):
            inode.D = inode.D[:len(inode.K)] + bytes(max(0, len(inode.K) - len(inode.D)))
        if self.device == "volatile":
            self.flush_ok()             # R5: ext4's own cache flush (fsync.c:166-170)
        if inode.err != fd.ofd.cursor:
            fd.ofd.cursor = inode.err
            inode.seen = True
            if MUT.on("NC-STORAGE-ERROR"):
                return 0                # Wrong: a write the device reported failed taken as success
            return "EIO"
        return 0

    def open_descriptions(self, inode):
        return any(fd.ofd.inode is inode and not fd.closed for p in self.procs() for fd in p.fds)

    def evict_pages(self, inode):
        """Page reclaim: clean pages revert to the medium's content. Allowed while
        descriptors are open (an open descriptor does not pin clean pages)."""
        if MUT.on("NC22b") and self.open_descriptions(inode):
            return False            # the R1 coupling: no reclaim while open
        for b in range((len(inode.K) + BLOCK - 1) // BLOCK):
            if b in inode.pending:
                continue
            lo, hi = b * BLOCK, min((b + 1) * BLOCK, len(inode.K))
            durable = bytes(inode.D[lo:hi])
            inode.K[lo:hi] = durable + bytes((hi - lo) - len(durable))
        return True

    def evict_inode(self, inode):
        """Inode eviction drops the pages and the error sequence; only possible
        with no open description and nothing pending."""
        if inode.pending or self.open_descriptions(inode):
            return False
        inode.K = bytearray(inode.D)
        inode.err, inode.seen = 0, True
        return True

    # -- faults ----------------------------------------------------------
    def kill(self, proc):
        """F1: process death. Descriptors close; nothing becomes durable."""
        for fd in list(proc.fds):
            self.close(fd)
        if MUT.on("NC01"):
            # First candidate's model: process death made current bytes durable.
            for inode in self.inodes.values():
                if inode.kind == "file":
                    inode.D = bytearray(inode.K)
                    inode.pending.clear()

    def kill_owner(self, proc, owner):
        """A thread's own descriptors close when it ends (Rust File drop)."""
        for fd in [f for f in proc.fds if f.owner == owner]:
            self.close(fd)

    def min_cut(self):
        ops = [self.pending_meta[i].op for i in self.guaranteed]
        first = self.pending_meta[0].op if self.pending_meta else self.op_count
        return (max(ops) + 1) if ops else first

    def schedules(self, family):
        """Every permitted metadata schedule of `family` for the pending log."""
        if not self.pending_meta:
            return [("ordered", self.op_count)]
        if family == "ordered":
            last = self.pending_meta[-1].op + 1
            return [("ordered", c) for c in range(self.min_cut(), last + 1)]
        dirs = {}
        for i, half in enumerate(self.pending_meta):
            dirs.setdefault(half.dir, []).append(i)
        choices = []
        for d, idx in sorted(dirs.items()):
            floor = sum(1 for i in idx if i in self.guaranteed)
            choices.append([(d, k) for k in range(floor, len(idx) + 1)])
        return [("perdir", dict(combo)) for combo in itertools.product(*choices)]

    def power_loss(self, schedule=None, outcomes=None, data="old"):
        """F2: durable state becomes visible. `schedule` picks the durable
        metadata (default: the guaranteed minimum); pending data blocks resolve
        per `outcomes`, otherwise `data` ('old' or 'new'), within their own
        4096-byte block."""
        if MUT.on("NC22a"):
            # The R1 model: an entry is durable only after fsync_dir of its directory.
            schedule = ("guaranteed-only", None)
        schedule = schedule or ("ordered", self.min_cut())
        if self.released and self.home_stable is not None:
            # R5 (design section 5.8): the tail moved past committed metadata whose home
            # writes were still only in a volatile cache; the medium holds the older blocks.
            for ino, entries in self.home_stable.items():
                if ino in self.inodes:
                    self.inodes[ino].ents_D = dict(entries)
        self.released = False
        self.home_cached = None
        if schedule[0] == "ordered":
            assert schedule[1] >= self.min_cut(), "schedule drops a guaranteed operation"
            durable = [h for h in self.pending_meta if h.op < schedule[1]]
        elif schedule[0] == "perdir":
            durable = []
            per = {}
            for i, half in enumerate(self.pending_meta):
                per.setdefault(half.dir, []).append((i, half))
            for d, items in per.items():
                k = schedule[1].get(d, 0)
                assert k >= sum(1 for i, _ in items if i in self.guaranteed)
                durable += [h for _, h in items[:k]]
        else:
            durable = [h for i, h in enumerate(self.pending_meta) if i in self.guaranteed]
        self._apply_halves(durable)
        self.pending_meta = []
        self.guaranteed = set()
        outcomes = outcomes or {}
        for inode in self.inodes.values():
            if inode.kind == "file":
                for block in sorted(inode.pending):
                    tear(inode, block, outcomes.get((inode.ino, block), data))
                inode.pending.clear()
                inode.K = bytearray(inode.D)
                inode.err, inode.seen = 0, True
            elif inode.kind == "dir":
                inode.ents_K = dict(inode.ents_D)
        for proc in self.procs():
            for fd in list(proc.fds):
                fd.closed = True
            proc.fds.clear()
        self.locks.clear()
        self.journal_aborted = self.emergency_ro = False
        self.probe_txn = None
        self.abort_in_probe_handle = False
        if self.home_stable is not None:
            self.mark_home_stable()     # after the power loss the medium is all there is


def tear(inode, block, outcome):
    """Resolve one pending block at power loss: 'old', 'new', ('prefix', k) or 'garbage'."""
    lo = block * BLOCK
    hi = min(lo + BLOCK, len(inode.K))
    if lo >= hi:
        return
    if len(inode.D) < hi and outcome != "old":
        inode.D.extend(bytes(hi - len(inode.D)))
    new = bytes(inode.K[lo:hi])
    if outcome == "old":
        return
    if outcome == "new":
        inode.D[lo:hi] = new
    elif outcome == "garbage":
        mixed = bytearray(new)
        for i in range(0, len(mixed), 97):
            mixed[i] ^= 0x5A
        inode.D[lo:hi] = mixed
    else:
        k = outcome[1]
        inode.D[lo:lo + k] = new[:k]


TEAR_OUTCOMES = ("old", "new", ("prefix", 8), ("prefix", 64), ("prefix", 2048), "garbage")


# ===========================================================================
# 9. The host model: provisioning, selection, startup, maintenance
#    (design sections 6, 10, 11.2, 13)
# ===========================================================================

Config = collections.namedtuple("Config", "record_capacity incident_limit")

# R4, the effective profile (design sections 5.3, 6.4 steps 9-11 and 13.2). The
# listing has the form Linux v6.17 prints in /proc/fs/ext4/<name>/options
# (super.c:3049-3058, _ext4_show_options with nodefs = 1): every option of ext4's
# table in its effective form, one per line, defaults included. Its option set is
# a fixture, not a host observation; so is the kernel identity.
QUALIFIED_LISTING = ("rw", "journal_checksum", "barrier", "user_xattr", "acl", "errors=remount-ro",
                     "commit=5", "data=ordered")
QUALIFIED_KERNEL = "6.17.0-fixture #1 SMP PREEMPT_DYNAMIC"
REQUIRED_OPTIONS = ("data=ordered", "barrier")
EXCLUDED_OPTIONS = ("nobarrier", "data=journal", "data=writeback", "journal_async_commit",
                    "norecovery", "noload", "fc_debug_force", "emergency_ro", "shutdown")
JOURNAL_INODE = 8               # the internal journal's inode; jbd2 names it "<device name>-8"
QUALIFIED_FEATURES = frozenset({"has_journal", "ext_attr", "dir_index", "filetype", "extent",
                                "flex_bg", "sparse_super", "large_file", "metadata_csum"})

# R5, the storage (design sections 5.5, 5.9 and 6.4 step 12). The fixture filesystem lies on
# one partition of an NVMe namespace behind a PCI Express controller. R4's fixture was a
# device-mapper device (253:1, dm-1), which R5 refuses as stacked storage. Every value is a
# fixture in the form Linux v6.17 prints it, not an observation of a host or a device.
FIXTURE_DEV = "259:3"           # st_dev of the fixture filesystem
FIXTURE_PCI = "0000:3d:00.0"    # the controller's PCI function
FIXTURE_NAME = "nvme0n1p2"      # the block device's name: the link's last component
FIXTURE_LINK = ("../../devices/pci0000:00/0000:00:1d.0/" + FIXTURE_PCI + "/nvme/nvme0/nvme0n1/"
                + FIXTURE_NAME)
FIXTURE_ATTRS = {
    "transport": "pcie\n",                                  # nvme/sysfs.c:406-414, nvme/pci.c:3240-3241
    "model": "Fixture NVMe controller" + " " * 17 + "\n",   # full field width (nvme/sysfs.c:362-374)
    "serial": "FIXTURE-0000000001  \n",
    "firmware_rev": "FW-0001 \n",
    "wwid": "eui.0025380000000001\n",                       # nvme/sysfs.c:103-132
    "write_cache": "write through\n",                       # blk-sysfs.c:452-457
    "fua": "0\n",                                           # blk-sysfs.c:284
}


def storage_files(link, attrs):
    """The sysfs files step 12 reads for a block device at `link`, as a host would have
    them: the controller's attributes, the namespace's wwid and its disk's queue."""
    form = parse_storage_link(link)
    if form is None:
        return {}
    ctrl_dir, disk_dir = form[0], form[1]
    files = {f"{ctrl_dir}/{name}": attrs[name] for name in ("transport", "model", "serial",
                                                            "firmware_rev")}
    files[f"{disk_dir}/wwid"] = attrs["wwid"]
    files[f"{disk_dir}/queue/write_cache"] = attrs["write_cache"]
    files[f"{disk_dir}/queue/fua"] = attrs["fua"]
    return files


class MountEnv:
    def __init__(self):
        self.mnt_id = 31
        self.st_dev = FIXTURE_DEV
        self.lines = [(31, FIXTURE_DEV, "ext4", "rw,relatime", "rw,errors=remount-ro")]
        self.magic = 0xEF53
        self.bsize = self.frsize = self.page = 4096
        self.protected = (1, 1)
        # R4: what the store uid observes of the effective profile (design section 6.4); R5: the
        # whole target of /sys/dev/block/<major>:<minor>, whose last component is the name
        self.links = {FIXTURE_DEV: FIXTURE_LINK}
        self.ext4_options = {FIXTURE_NAME: list(QUALIFIED_LISTING)}     # /proc/fs/ext4/<name>/options
        self.jbd2 = {f"{FIXTURE_NAME}-{JOURNAL_INODE}"}                 # entries of /proc/fs/jbd2
        self.kernel = QUALIFIED_KERNEL       # uname(2): release and version
        # R5: the files below /sys/devices that step 12 reads, all world-readable
        self.sysfs_files = storage_files(FIXTURE_LINK, FIXTURE_ATTRS)
        # R4: what only root reads, at qualification (design section 13.2)
        self.features = set(QUALIFIED_FEATURES)
        self.journal = {"inode": JOURNAL_INODE, "device": None}
        # R5: what only root establishes, at qualification (design sections 5.9 and 13.2): the
        # controller's Identify data, which an admin command returns, and the host itself
        self.identify = {"vwc_present": False, "ns_vwc_not_present": False}
        self.guest = False


def device_name(env):
    """Step 9: the last component of readlink("/sys/dev/block/<major>:<minor>")."""
    link = env.links.get(env.st_dev)
    return link.rsplit("/", 1)[-1] if link else None


def check_profile(env, prov, pinned=True):
    """R4 (design section 6.4, steps 9-11): the effective profile, read from the
    listing the kernel prints with every option in its effective form, never
    inferred from an absent string in mountinfo, which omits defaults; the
    journal's location; and the kernel the Owner qualified. Unknown or
    contradictory information refuses. `pinned=False` (re-qualification) skips
    only the kernel comparison: the pinned identity is being replaced."""
    if MUT.on("NC-ACT-UNQUALIFIED"):
        # Wrong: the profile taken from claims, the pinned mountinfo strings and
        # the attestation, as R3 did; the kernel never compared.
        return None
    name = device_name(env)
    if name is None:
        return "device name unresolved"
    listing = env.ext4_options.get(name)
    if not listing:
        return "effective options unreadable"
    if listing[0] != "rw":
        return "read-only"
    super_opts = [line for line in env.lines if line[0] == env.mnt_id][0][4]
    relevant = set(REQUIRED_OPTIONS + EXCLUDED_OPTIONS)
    if not {o for o in super_opts.split(",") if o in relevant} <= set(listing):
        return "contradictory profile information"
    if not MUT.on("NC-PROF-MODE"):
        if any(o not in listing for o in REQUIRED_OPTIONS) or \
                any(o in listing for o in EXCLUDED_OPTIONS):
            return "effective profile"
    if not MUT.on("NC-PROF-JOURNAL") and f"{name}-{JOURNAL_INODE}" not in env.jbd2:
        return "journal not internal"
    if pinned and env.kernel != prov["kernel"]:
        return "kernel not qualified"
    return None


def qualify(env):
    """Section 13.2 step 1, the Owner's qualification as root: the superblock
    facts the store uid cannot read, then everything every opening checks.
    Returns the problems; provisioning proceeds only with none."""
    problems = []
    if "has_journal" not in env.features:
        problems.append("no journal")
    if "fast_commit" in env.features:
        problems.append("fast_commit feature")
    if env.journal["device"] is not None or env.journal["inode"] != JOURNAL_INODE:
        problems.append("journal not internal at inode 8")
    problem = check_profile(env, {"kernel": env.kernel})
    if problem:
        problems.append(problem)
    return problems


# -- R5: the storage below the filesystem (design sections 5.5, 5.9, 6.4 step 12, 10.8) --

STORAGE_CLASS = "nvme-pcie"     # the only admitted storage class
STORAGE_READ_LIMIT = 4096       # each attribute is read with a limit (section 6.6)
STORAGE_IDENTITY_PREFIX = b"nexus-phase2-custody-storage\x00\x01"
STORAGE_IDENTITY_ATTRS = ("model", "serial", "firmware_rev", "wwid")
STORAGE_CACHE_ADMITTED = ("write through\n", "0\n")     # queue/write_cache, queue/fua
_PCI_FUNCTION = re.compile(r"[0-9a-f]{4}:[0-9a-f]{2}:[0-9a-f]{2}\.[0-7]")
_NVME_CONTROLLER = re.compile(r"nvme(?:0|[1-9][0-9]{0,4})")
_NVME_DISK = re.compile(r"nvme(?:0|[1-9][0-9]{0,4})n[1-9][0-9]{0,9}")
_NVME_PARTITION = re.compile(r"(nvme(?:0|[1-9][0-9]{0,4})n[1-9][0-9]{0,9})p([1-9][0-9]{0,2})")


def parse_storage_link(link):
    """Step 12's form. The target of /sys/dev/block/<major>:<minor> must be
    ../../devices/pci.../<PCI function>/nvme/nvme<k>/<disk>[/<disk>p<i>]: a namespace's disk
    whose parent is an NVMe controller (nvme/core.c:4175), itself in its class directory
    under a PCI function (nvme/core.c:5121-5126, base/core.c:3257-3264). Returns the
    controller's and the disk's directories below /sys, the PCI function and the partition
    (or None for the whole namespace); or None for any other form."""
    if not link or not link.startswith("../../devices/"):
        return None
    parts = link[len("../../devices/"):].split("/")
    partition = None
    last = _NVME_PARTITION.fullmatch(parts[-1])
    if last is not None:
        if len(parts) < 2 or parts[-2] != last.group(1) or int(last.group(2)) > 255:
            return None
        partition = int(last.group(2))
        parts = parts[:-1]
    if len(parts) < 5 or not _NVME_DISK.fullmatch(parts[-1]) or \
            not _NVME_CONTROLLER.fullmatch(parts[-2]) or parts[-3] != "nvme" or \
            not _PCI_FUNCTION.fullmatch(parts[-4]) or not parts[0].startswith("pci"):
        return None
    disk_dir = "/sys/devices/" + "/".join(parts)
    return disk_dir.rsplit("/", 1)[0], disk_dir, parts[-4], partition


def read_sysfs(env, path):
    """One bounded read of a world-readable sysfs attribute; None if absent or oversized."""
    data = env.sysfs_files.get(path)
    if data is None or len(data) > STORAGE_READ_LIMIT:
        return None
    return data


def storage_identity(attrs):
    """The identity record's SHA-256 (section 6.4 step 12): the prefix, then each of the
    controller's model, serial and firmware_rev and the namespace's wwid, as read, final
    newline included, each after its length as a big-endian u16."""
    record = bytearray(STORAGE_IDENTITY_PREFIX)
    for name in STORAGE_IDENTITY_ATTRS:
        raw = attrs[name].encode("ascii")
        record += u16(len(raw)) + raw
    return sha(bytes(record)).hex()


def observe_storage(env):
    """Step 12's reads, as the store uid performs them: (observation, problem)."""
    form = parse_storage_link(env.links.get(env.st_dev))
    if form is None:
        return None, "storage not a direct NVMe namespace on PCI Express"
    ctrl_dir, disk_dir, pci, partition = form
    attrs = {name: read_sysfs(env, f"{ctrl_dir}/{name}")
             for name in ("transport", "model", "serial", "firmware_rev")}
    attrs["wwid"] = read_sysfs(env, f"{disk_dir}/wwid")
    attrs["write_cache"] = read_sysfs(env, f"{disk_dir}/queue/write_cache")
    attrs["fua"] = read_sysfs(env, f"{disk_dir}/queue/fua")
    if any(value is None for value in attrs.values()):
        return None, "storage unreadable"
    return {"pci": pci, "partition": partition, "attrs": attrs,
            "identity": storage_identity(attrs)}, None


def check_storage(env, prov, pinned=True):
    """R5, design section 6.4 step 12: the storage below the filesystem, observed afresh.
    Admitted only: a direct NVMe namespace (or one partition of it) behind a PCI Express
    controller, which the kernel registered without a volatile write cache, matching the
    qualified identity in PROVISION. queue/write_cache alone is not enough: a write to it
    changes the kernel's view, not the device (blk-sysfs.c:459-478); queue/fua tells the
    two apart, because the NVMe driver sets both features together (nvme/core.c:2395-2398)
    and the block layer clears FUA without a write cache (blk-settings.c:468-469).
    `pinned=False` (re-qualification) skips only the identity comparison, never a rule.
    Returns (observation, problem)."""
    if MUT.on("NC-STORAGE-COMPOSED"):
        # Wrong (R4's domain): storage admitted on the Owner's attestation, a volatile
        # write-back cache included (A-S5); nothing below the filesystem is checked.
        return {"pci": prov.get("storage_pci"), "partition": prov.get("storage_partition"),
                "identity": prov.get("storage_identity"), "attrs": None}, None
    obs, problem = observe_storage(env)
    if problem:
        return None, problem
    attrs = obs["attrs"]
    if attrs["transport"] != "pcie\n":
        return None, "storage transport"
    cache = (attrs["write_cache"], attrs["fua"])
    if MUT.on("NC-STORAGE-VOLATILE") and attrs["write_cache"] == "write through\n":
        cache = STORAGE_CACHE_ADMITTED  # Wrong: the kernel's write_cache value alone decides
    if cache == ("write back\n", "1\n"):
        return None, "volatile write cache"
    if cache == ("write through\n", "1\n"):
        return None, "volatile write cache, flushes disabled in the kernel's view only"
    if cache != STORAGE_CACHE_ADMITTED:
        return None, "contradictory storage information"
    if prov.get("storage_class") != STORAGE_CLASS:
        return None, "storage not qualified"
    if MUT.on("NC-STORAGE-CLAIM"):
        # Wrong: the identity PROVISION claims taken in place of the one observed.
        obs = dict(obs, pci=prov.get("storage_pci"), partition=prov.get("storage_partition"),
                   identity=prov.get("storage_identity"))
    if pinned and (obs["pci"], obs["partition"], obs["identity"]) != \
            (prov.get("storage_pci"), prov.get("storage_partition"), prov.get("storage_identity")):
        return None, "storage not qualified"
    return obs, None


_ADMISSION_KEY = object()       # held only by admit_storage(): the private constructor
_ADMISSIONS = {}                # id -> every admission admit_storage() made
_OPENINGS = itertools.count(1)  # serials of openings: owners, verifiers and sessions


class StorageAdmission:
    """R5, design section 10.8: a verified storage qualification. Only admit_storage()
    makes one, from a fresh observation; decoding PROVISION never yields one. It names the
    opening that observed the storage, the PROVISION digest of that opening's selection and
    the identity observed; nothing stores it beyond its opening."""

    __slots__ = ("opening", "prov_digest", "identity", "pci", "partition")

    def __init__(self, key, opening, prov_digest, obs):
        if key is not _ADMISSION_KEY:
            raise TypeError("a storage admission is made only by the opening's storage check")
        self.opening, self.prov_digest = opening, prov_digest
        self.identity, self.pci, self.partition = obs["identity"], obs["pci"], obs["partition"]


def admit_storage(env, sel, opening, pinned=True):
    """The only constructor of a StorageAdmission: step 12 afresh, then the admission."""
    obs, problem = check_storage(env, sel.prov, pinned)
    if problem:
        return None, problem
    admission = StorageAdmission(_ADMISSION_KEY, opening, sel.prov_digest, obs)
    _ADMISSIONS[id(admission)] = admission
    return admission, None


def verify_admission(admission, opening, prov_digest):
    """The claim's test (section 10.8): an admission made by the storage check, at this
    opening, for this selection."""
    if MUT.on("NC-STORAGE-CLAIM"):
        return admission is not None    # Wrong: any admission-like object, from any opening
    return (admission is not None and _ADMISSIONS.get(id(admission)) is admission
            and admission.opening == opening and admission.prov_digest == prov_digest)


def qualify_storage(env):
    """R5, section 13.2 step 1 (storage): what the Owner establishes as root before
    provisioning. The controller's Identify data and the host are root-only facts the store
    uid cannot observe; step 12's rules are applied as the store uid will read them. Passing
    qualifies nothing by itself: stable completion (A-S1) remains an assumption."""
    problems = []
    if env.guest:
        problems.append("virtual machine guest")
    if env.identify["vwc_present"] and not env.identify["ns_vwc_not_present"]:
        problems.append("the controller reports a volatile write cache")
    _, problem = check_storage(env, {"storage_class": STORAGE_CLASS}, pinned=False)
    if problem:
        problems.append(problem)
    return problems


def owner_storage_record(env):
    """The PROVISION storage fields the Owner records at qualification (section 7.5)."""
    obs, problem = observe_storage(env)
    if problem and MUT.on("NC-STORAGE-COMPOSED"):
        # R4's domain: the Owner attests storage that nothing observes; nobody checks the fields.
        return {"storage_class": STORAGE_CLASS, "storage_pci": "0000:00:00.0",
                "storage_partition": None, "storage_identity": "0" * 64}
    assert problem is None, f"fixture: provisioning on storage that cannot be observed ({problem})"
    return {"storage_class": STORAGE_CLASS, "storage_pci": obs["pci"],
            "storage_partition": obs["partition"], "storage_identity": obs["identity"]}


def check_mount(env, prov, pinned=True, profile=True):
    """Mount identity (section 6.4). `pinned=False` is re-qualification only: the
    pinned option strings are being replaced, every other check still applies.
    R4: `profile` adds the effective-profile checks of steps 9-11."""
    if MUT.on("NC14b"):
        # Wrong: EXT2/EXT3/EXT4 share 0xef53 (statfs(2)).
        return None if env.magic == 0xEF53 else "magic"
    matches = [line for line in env.lines if line[0] == env.mnt_id]
    if len(matches) != 1:
        return "mount id not unique"
    _, dev, fstype, mount_opts, super_opts = matches[0]
    if dev != env.st_dev:
        return "device mismatch"
    if fstype != "ext4":
        return f"filesystem {fstype}"
    if pinned and (mount_opts != prov["mount_options"] or super_opts != prov["super_options"]):
        return "options changed"
    for bad in ("ro", "nobarrier", "barrier=0", "data=writeback"):
        if bad in mount_opts.split(",") or bad in super_opts.split(","):
            return "unsupported option"
    if (env.bsize, env.frsize, env.page) != (4096, 4096, 4096):
        return "block or page size"
    if env.protected != (1, 1):
        return "link protection"
    if profile:
        return check_profile(env, prov, pinned)
    return None


ROOT_ID = bytes(range(0x10, 0x20))
SUCCESSOR_ID = bytes(range(0x20, 0x30))
UID = GID = 1001
STORE_NAMES = ("LOCK", "journals", "dispositions", "archive")
ENTRY_LIMIT = 4096          # entries in dispositions/, revoked/ and archive/ (section 6.6)
REPORT_LIMIT = 64           # incidents listed in detail by a report (section 11.2)
SELECTION_ATTEMPTS = 3      # bounded re-selection after a failed revalidation (section 10.5)
ATTESTATION = "fixture: stable completion and 4096-byte containment stated by the Owner"


def write_file(world, directory, name, data, proc=None):
    """Write `data` at offset 0 through a fresh writable description (no sync)."""
    fs = world.fs
    fd = fs.open(proc or world.admin, "main", directory, name, writable=True)
    fs.pwrite(fd, 0, data)
    fs.close(fd)


def sync_file(world, directory, name, proc=None):
    fs = world.fs
    fd = fs.open(proc or world.admin, "main", directory, name)
    result = fs.fdatasync(fd)
    fs.close(fd)
    assert result == 0, f"sync of {name} failed"


def tagged(doc, fn):
    fn.doc = doc
    return fn


def run_steps(world, steps):
    for step in steps:
        world.fs.step = step.doc
        step()
    world.fs.step = None


class World:
    """One simulated host: a provisioned store, a PROVISION directory and the
    parent directory of state roots."""

    def __init__(self, n=2, c_pool=16, provision=True, root_id=ROOT_ID, state="store-a"):
        self.fs = SimFS()
        self.mount = MountEnv()
        self.N, self.C = n, c_pool
        self.root_id = root_id
        fs = self.fs
        self.admin = fs.register(Proc("root"))
        self.provdir = fs.create(fs.root, "etc-provision", "dir", 0, 0, 0o755)
        self.parent = fs.create(fs.root, "var-state", "dir", 0, 0, 0o755)
        fs.fsync_dir(fs.root)
        self.state = state
        self.root_ids = {}          # the Owner's record: state root name -> root id
        if provision:
            run_steps(self, provision_steps(self, root_id, state))

    def store_root(self, state=None):
        return self.fs.inodes[self.parent.ents_K[state or self.state]]

    def dir(self, *names, state=None):
        node = self.store_root(state)
        for name in names:
            node = self.fs.inodes[node.ents_K[name]]
        return node

    def pool_inode(self, index, state=None):
        return self.fs.inodes[self.dir("journals", state=state).ents_K[f"j{index:05d}.journal"]]

    def write_pool(self, index, data):
        """Install durable bytes in a pool file (fixture set-up, outside any process)."""
        inode = self.pool_inode(index)
        assert len(data) == len(inode.K)
        inode.K = bytearray(data)
        inode.D = bytearray(data)
        inode.pending.clear()

    def provision_record(self):
        fd = self.fs.open(self.admin, "main", self.provdir, "PROVISION")
        data = self.fs.pread(fd, 0, 65537)
        self.fs.close(fd)
        return parse_provision(data)


def provision_record_for(world, ctx, root_id, state, predecessor, statement, revision):
    return {
        "uid": UID, "gid": GID, "root_id": root_id,
        "state_root": f"/var/lib/nexus-os/phase2-custody/{state}",
        "root_inode": ctx["root"].ino, "lock_inode": ctx["lock"].ino,
        "journals_inode": ctx["journals"].ino, "dispositions_inode": ctx["dispositions"].ino,
        "revoked_inode": ctx["revoked"].ino, "archive_inode": ctx["archive"].ino,
        "mount_options": world.mount.lines[0][3], "super_options": world.mount.lines[0][4],
        "kernel": world.mount.kernel, **owner_storage_record(world.mount),
        "attestation": ATTESTATION, "pool_inodes": list(ctx["pool"]), "c_pool": world.C,
        "retired": 0, "predecessor": predecessor, "predecessor_statement": statement,
        "operator": "owner", "created": "2026-10-01T00:00:00Z", "revision": revision}


def store_steps(world, state, ctx):
    """Section 13.2 steps 2-4: directories, LOCK and the pool, each step synced."""
    fs = world.fs
    steps = []

    def add(doc, fn):
        steps.append(tagged(doc, fn))

    add("13.2/2", lambda: ctx.__setitem__("root", fs.create(world.parent, state, "dir", 0, 0, 0o755)))
    for name in ("journals", "dispositions", "archive"):
        add("13.2/2", lambda name=name: ctx.__setitem__(name, fs.create(ctx["root"], name, "dir", 0, 0, 0o755)))
    add("13.2/2", lambda: ctx.__setitem__("revoked", fs.create(ctx["dispositions"], "revoked", "dir", 0, 0, 0o755)))
    for key in ("revoked", "dispositions", "journals", "archive", "root"):
        add("13.2/2", lambda key=key: fs.fsync_dir(ctx[key]))
    add("13.2/2", lambda: fs.fsync_dir(world.parent))
    add("13.2/3", lambda: ctx.__setitem__("lock", fs.create(ctx["root"], "LOCK", "file", UID, GID, 0o600)))
    add("13.2/3", lambda: sync_file(world, ctx["root"], "LOCK"))
    add("13.2/3", lambda: fs.fsync_dir(ctx["root"]))
    ctx["pool"] = []
    size = (world.C + 2) * BLOCK
    for i in range(world.N):
        name = f"j{i:05d}.journal"
        add("13.2/4", lambda name=name: ctx["pool"].append(
            fs.create(ctx["journals"], name, "file", UID, GID, 0o600, size).ino))
        add("13.2/4", lambda name=name: write_file(world, ctx["journals"], name, bytes(size)))
        add("13.2/4", lambda name=name: sync_file(world, ctx["journals"], name))
    add("13.2/4", lambda: fs.fsync_dir(ctx["journals"]))
    return steps


def publish_provision_steps(world, make_record, write_doc, publish_doc, before=None):
    """Write PROVISION to a temporary name, sync it, rename it into place and
    sync the directory (sections 13.2 step 5, 13.3 and 13.6)."""
    fs = world.fs

    def create():
        if before is not None:
            before()
        fs.create(world.provdir, "PROVISION.tmp", "file", 0, 0, 0o444)

    return [
        tagged(write_doc, create),
        tagged(write_doc, lambda: write_file(world, world.provdir, "PROVISION.tmp",
                                             provision_text(make_record()))),
        tagged(write_doc, lambda: sync_file(world, world.provdir, "PROVISION.tmp")),
        tagged(publish_doc, lambda: fs.rename(world.provdir, "PROVISION.tmp", world.provdir, "PROVISION")),
        tagged(publish_doc, lambda: fs.fsync_dir(world.provdir)),
    ]


def provision_steps(world, root_id, state, predecessor=None, statement=None, revision=1):
    """P-PROV (design section 13.2) as crash-injectable steps."""
    ctx = {}
    world.root_ids[state] = root_id
    steps = store_steps(world, state, ctx)
    steps += publish_provision_steps(
        world, lambda: provision_record_for(world, ctx, root_id, state, predecessor, statement,
                                            revision), "13.2/5", "13.2/5")
    return steps


def rewrite_provision_steps(world, change, session=None):
    """A PROVISION rewrite (design section 13.3), revision + 1."""
    holder = {}

    def make():
        if "record" not in holder:
            prov = world.provision_record()
            record = {
                "uid": prov["uid"], "gid": prov["gid"], "root_id": prov["root_id"],
                "state_root": prov["state_root"], "root_inode": prov["root_inode"],
                "lock_inode": prov["lock_inode"], "journals_inode": prov["journals_inode"],
                "dispositions_inode": prov["dispositions_inode"],
                "revoked_inode": prov["revoked_inode"], "archive_inode": prov["archive_inode"],
                "mount_options": prov["mount_options"], "super_options": prov["super_options"],
                "kernel": prov["kernel"], "storage_class": prov["storage_class"],
                "storage_pci": prov["storage_pci"], "storage_partition": prov["storage_partition"],
                "storage_identity": prov["storage_identity"],
                "attestation": ATTESTATION, "pool_inodes": list(prov["pool_inodes"]),
                "c_pool": prov["c_pool"], "retired": prov["retired"],
                "predecessor": prov["predecessor"],
                "predecessor_statement": None if prov["predecessor"] is None else "successor",
                "operator": "owner", "created": "2026-10-01T00:00:00Z",
                "revision": prov["revision"] + 1}
            change(record)
            holder["record"] = record
        return holder["record"]

    steps = publish_provision_steps(world, make, "13.3/2", "13.3/3")
    if session is not None:
        last = steps[-1]
        steps[-1] = tagged(last.doc, lambda: (last(), session.reselect()))
    return steps


# -- selection and revalidation (design section 10.5) ----------------------

Selection = collections.namedtuple(
    "Selection", "prov prov_ino prov_digest state root_ino lock_ino fd provdir_ino parent_ino")


def select_provision(world, proc):
    """Read the active PROVISION through a fresh lookup of its entry. Under
    NC21b every selection keeps its descriptor for the wrong revalidation."""
    fs = world.fs
    keep_fd = MUT.on("NC21b")
    st = fs.stat(world.provdir, "PROVISION", proc)
    if st is None:
        return None, ("Unprovisioned", "")
    if st.kind != "file" or (st.uid, st.mode, st.nlink) != (0, 0o444, 1):
        return None, ("Invalid", "PROVISION type")
    fd = fs.open(proc, "main", world.provdir, "PROVISION")
    data = fs.pread(fd, 0, 65537)
    if not keep_fd:
        fs.close(fd)
        fd = None
    try:
        prov = parse_provision(data)
    except ParseError as error:
        if fd is not None:
            fs.close(fd)
        return None, ("Invalid", f"PROVISION: {error}")
    state = prov["state_root"].rsplit("/", 1)[1]
    return Selection(prov, st.ino, sha(data), state, prov["root_inode"], prov["lock_inode"],
                     fd, world.provdir.ino, world.parent.ino), None


def revalidate(world, proc, sel, lock_fd):
    """Post-lock revalidation against the authoritative entries: PROVISION by a
    fresh lookup (inode and full content), the state root and the LOCK entry,
    which must be the inode this description locked."""
    fs = world.fs
    if MUT.on("NC21a"):
        return True                 # R1: no revalidation after locking
    if MUT.on("NC21b"):
        # Wrong: re-read through the descriptor kept from selection. It still
        # names the original inode after a replacement.
        return sha(fs.pread(sel.fd, 0, 65537)) == sel.prov_digest
    st = fs.stat(world.provdir, "PROVISION", proc)
    if st is None or st.ino != sel.prov_ino:
        return False
    fd = fs.open(proc, "main", world.provdir, "PROVISION")
    data = fs.pread(fd, 0, 65537)
    fs.close(fd)
    if sha(data) != sel.prov_digest:
        return False
    rst = fs.stat(world.parent, sel.state, proc)
    if rst is None or rst.ino != sel.root_ino:
        return False
    root = fs.inodes[rst.ino]
    return root.ents_K.get("LOCK") == sel.lock_ino == lock_fd.ofd.inode.ino


def open_root(world, proc, sel, pinned=True, opening=None, holder=None):
    """R5: after the mount, profile and one-filesystem checks, the storage
    (section 6.4 step 12); the admission it makes is kept in `holder`."""
    fs = world.fs
    st = fs.stat(world.parent, sel.state, proc)
    if st is None or st.kind != "dir" or st.ino != sel.prov["root_inode"]:
        return None, ("Lost", "state root")
    if (st.uid, st.gid, st.mode) != (0, 0, 0o755):
        return None, ("Invalid", "state root ownership")
    problem = check_mount(world.mount, sel.prov, pinned)
    if problem:
        return None, ("Unsupported", problem)
    if len({world.provdir.dev, world.parent.dev, fs.inodes[st.ino].dev}) != 1:
        return None, ("Unsupported", "PROVISION directory and state root on different filesystems")
    admission, problem = admit_storage(world.mount, sel, opening, pinned)
    if problem:
        return None, ("Unsupported", problem)
    if holder is not None:
        holder["storage"] = admission
    return fs.inodes[st.ino], None


def lock_entry(world, proc, root, prov):
    st = world.fs.stat(root, "LOCK", proc)
    if st is None or st.kind != "file" or st.ino != prov["lock_inode"]:
        return ("Lost", "LOCK")
    if (st.uid, st.gid, st.mode, st.nlink) != (prov["uid"], prov["gid"], 0o600, 1):
        return ("Invalid", "LOCK type")
    return None


# -- durable activation (design section 10.6) ------------------------------

# Every directory whose entries the selection and the decision depend on, in
# sync order: the store's directories first, the PROVISION directory last.
ACTIVATION_DIRECTORIES = ("journals", "dispositions/revoked", "dispositions", "archive", "root",
                          "parent", "provdir")
ACTIVATION_EINTR_RETRIES = 3
# The durability operations of an activation, as section 10.6 lists them.
ACTIVATION_PROTOCOL = ([("10.6/A2", "fsync_dir", label) for label in ACTIVATION_DIRECTORIES]
                       + [("10.6/A2", "fdatasync", "PROVISION"), ("10.6/A3", "futimens", "LOCK"),
                          ("10.6/A3", "fsync", "LOCK")])


def _super_read_only(world):
    lines = [line for line in world.mount.lines if line[0] == world.mount.mnt_id]
    return any("ro" in line[4].split(",") for line in lines)


def resolve_activation_dir(world, sel, root, label):
    """Fresh resolution of one activation directory: (directory, expected inode),
    the store's directories as PROVISION records them, the state root's parent
    and the PROVISION directory as the selection saw them."""
    fs = world.fs
    prov = sel.prov
    expected = {"journals": prov["journals_inode"], "dispositions": prov["dispositions_inode"],
                "dispositions/revoked": prov["revoked_inode"], "archive": prov["archive_inode"],
                "root": prov["root_inode"], "parent": sel.parent_ino, "provdir": sel.provdir_ino}
    if label == "root":
        node = root
    elif label == "parent":
        node = world.parent
    elif label == "provdir":
        node = world.provdir
    else:
        node = root
        for name in label.split("/"):
            ino = node.ents_K.get(name)
            if ino is None:
                return None, expected[label]
            node = fs.inodes[ino]
    return node, expected[label]


def activate(world, proc, sel, root, lock_fd, pinned=True, hook=None, opening=None, holder=None):
    """Durable activation (design section 10.6), under the store lock held
    through `lock_fd`. Returns None once the selection is activated, otherwise
    the refusal (state, reason). It changes no byte of the store: it makes the
    namespace the selection and the decision depend on durable. R4: each A2
    sync that waits tests the abort flag itself; the probe's handle start tests
    it for the transactions that had already completed, and the probe's fsync
    reports that test through the emergency state (design section 10.7). The
    hook also sees "probe" and "probe-fsync", the two probe windows. R5: A4
    checks the storage again and makes this opening's admission afresh
    (section 10.8)."""
    fs = world.fs
    if MUT.on("NC-ACT-COMPOSE"):
        return None                     # R2: no activation at all
    if MUT.on("NC-ACT-VISIBLE"):
        # Wrong: the freshly revalidated, visible selection taken as activated.
        return None if revalidate(world, proc, sel, lock_fd) else ("SelectionChanged", "visible")
    fs.log.append((proc.name, "activate", "begin"))
    if len({world.provdir.dev, world.parent.dev, root.dev}) != 1:
        return ("Unsupported", "PROVISION directory and state root on different filesystems")
    for label in ACTIVATION_DIRECTORIES:
        if hook is not None:
            hook(world, label)
        directory, expected = resolve_activation_dir(world, sel, root, label)
        if directory is None:
            return ("Lost", f"activation: {label}")
        if directory.ino != expected and not MUT.on("NC-ACT-REVALIDATE"):
            return ("Invalid", f"activation: {label} is not the selected directory")
        for attempt in range(1 + ACTIVATION_EINTR_RETRIES):
            result = fs.sync_dir(proc, directory, _super_read_only(world))
            if result != "EINTR":
                break
        if result != 0 and not MUT.on("NC-ACT-ERROR"):
            return ("Unreliable", f"activation sync of {label}: {result}")
    fd = fs.open(proc, "main", world.provdir, "PROVISION")
    result = fs.fdatasync(fd)           # defence in depth; never a certificate of content
    fs.close(fd)
    if result != 0 and not MUT.on("NC-ACT-ERROR"):
        return ("Unreliable", f"activation sync of PROVISION: {result}")
    if not MUT.on("NC-ACT-PROBE"):
        if hook is not None:
            hook(world, "probe")
        result = fs.touch(lock_fd, _super_read_only(world))
        if result == 0:
            if hook is not None:
                hook(world, "probe-fsync")
            result = fs.fsync_file(lock_fd, _super_read_only(world))
        if result != 0 and not MUT.on("NC-ACT-ERROR"):
            return ("Unreliable", f"activation probe: {result}")
    if not MUT.on("NC-ACT-REVALIDATE"):
        for label in ACTIVATION_DIRECTORIES:
            directory, expected = resolve_activation_dir(world, sel, root, label)
            if directory is None or directory.ino != expected:
                return ("SelectionChanged", f"after activation: {label}")
        if not revalidate(world, proc, sel, lock_fd):
            return ("SelectionChanged", "after activation")
        # R4: the effective profile is checked again after the syncs (A4)
        problem = check_mount(world.mount, sel.prov, pinned, profile=not MUT.on("NC-PROF-GUARD"))
        if problem:
            return ("Unsupported", problem)
        if len({world.provdir.dev, world.parent.dev, root.dev}) != 1:
            return ("Unsupported", "PROVISION directory and state root on different filesystems")
        # R5: the storage again (section 6.4 step 12), and this opening's admission afresh
        again, problem = admit_storage(world.mount, sel, opening, pinned)
        if problem:
            return ("Unsupported", problem)
        before = holder.get("storage") if holder is not None else None
        if before is not None and again.identity != before.identity:
            return ("Unsupported", "storage changed during activation")
        if holder is not None:
            holder["storage"] = again
    fs.log.append((proc.name, "activate", "certified"))
    return None


def late_activation(world, proc, rep):
    """NC-ACT-ORDER only: the activation performed after dependent work."""
    sel = rep["selection"]
    result = activate(world, proc, sel, world.store_root(sel.state), rep["guard"][-1])
    rep["activated"] = result is None
    return result


Claim = collections.namedtuple("Claim", "index claim generation header lock_fd io_fd applied")


def new_report():
    return {"state": None, "reason": "", "files": {}, "incidents": {}, "blocking": set(),
            "dispositioned": set(), "history": set(), "preserved": False,
            "durability_certified": False, "claim": None, "guard": [], "attempts": 0,
            "scanned": False, "report": None, "selection": None, "activated": False,
            "opening": next(_OPENINGS), "storage": None}


def startup(world, proc, cfg, *, sync=True, claim=False, verifier=False, seed=b"g",
            generation=None, hook=None, activation_hook=None, storage_hook=None):
    """Startup (section 10.2) for an owner, or the standalone verifier. An owner
    activates the selection (section 10.6) before it reads the store."""
    fs = world.fs
    rep = new_report()
    opened = []

    def finish(state, reason=""):
        rep["state"], rep["reason"] = state, reason
        keep = set()
        if rep["claim"] is not None:
            keep = set(id(fd) for fd in rep["guard"]) | {id(rep["claim"].io_fd)}
        for fd in opened:
            if id(fd) not in keep:
                fs.close(fd)
        return rep

    def sopen(directory, name, writable=False, owner="main"):
        fd = fs.open(proc, owner, directory, name, writable=writable)
        opened.append(fd)
        return fd

    if cfg.record_capacity > world.C or cfg.incident_limit > 32:
        return finish("Unsupported", "configuration")
    for attempt in range(SELECTION_ATTEMPTS):
        rep["attempts"] = attempt + 1
        sel, err = select_provision(world, proc)
        if err:
            return finish(*err)
        if sel.fd is not None:
            opened.append(sel.fd)
        rep["provision"], rep["selection"] = sel.prov, sel
        rep["storage"] = None
        root, err = open_root(world, proc, sel, opening=rep["opening"], holder=rep)
        if err:
            return finish(*err)
        if MUT.on("NC06"):
            journals = fs.inodes[root.ents_K["journals"]]
            for i in range(sel.prov["pool"]):
                fd = sopen(journals, f"j{i:05d}.journal")
                fs.fdatasync(fd)
                fs.pread(fd, 0, BLOCK)
                fs.close(fd)
        err = lock_entry(world, proc, root, sel.prov)
        if err:
            return finish(*err)
        lock_fd = sopen(root, "LOCK")
        if hook is not None:
            hook(world, attempt)
        if not fs.flock(lock_fd, "SH" if verifier else "EX"):
            return finish("Busy")
        if revalidate(world, proc, sel, lock_fd):
            rep["guard"].append(lock_fd)
            break
        for fd in list(opened):
            fs.close(fd)
        opened.clear()
    else:
        return finish("SelectionChanged", f"after {SELECTION_ATTEMPTS} attempts")
    if not verifier and not MUT.on("NC-ACT-ORDER"):
        outcome = activate(world, proc, sel, root, rep["guard"][-1], hook=activation_hook,
                           opening=rep["opening"], holder=rep)
        if outcome:
            rep["storage"] = None       # R5: an unsupported configuration keeps no admission
            return finish(*outcome)
        rep["activated"] = True
    if storage_hook is not None:
        storage_hook(world, rep)        # R5: a test's injection point (C30)
    outcome = scan(world, proc, sel, root, rep, cfg, "verifier" if verifier else "owner",
                   sync=sync, opened=opened)
    if outcome:
        return finish(*outcome)
    outcome = decide(rep, cfg)
    if outcome:
        return finish(*outcome)
    if not claim:
        return finish("Ready")
    return claim_handoff(world, proc, cfg, sel, root, rep, opened, finish, seed, generation)


def decide(rep, cfg):
    """Authorization from the complete incident set, never from a report."""
    source = rep["incidents"]
    if MUT.on("NC24b"):
        source = {b: rep["incidents"][b] for b in rep["report"]["detail"]}
    current = [b for b in source if b not in rep["history"]]
    if len(current) > cfg.incident_limit:
        return ("Capacity", f"{len(current)} current incidents")
    for b in current:
        if disposition_matches(rep["dispositions"].get(b), rep["incidents"][b]):
            rep["dispositioned"].add(b)
        else:
            rep["blocking"].add(b)
    if rep["blocking"]:
        return ("PriorUnresolved", "")
    return None


def claim_handoff(world, proc, cfg, sel, root, rep, opened, finish, seed, generation):
    fs = world.fs
    # R5 (section 10.8): the claim needs this opening's own admission of the storage.
    if not verify_admission(rep.get("storage"), rep["opening"], sel.prov_digest):
        return finish("Unsupported", "storage admission not verified")
    journals = fs.inodes[root.ents_K["journals"]]
    pool_names = [f"j{i:05d}.journal" for i in range(sel.prov["pool"])]
    current = [b for b in rep["incidents"] if b not in rep["history"]]
    unused = [i for i, r in sorted(rep["files"].items()) if r["class"] == "Unused"]
    if not unused:
        return finish("PoolExhausted")
    new_claim = rep["max_claim"] + 1
    if new_claim > MAX_CLAIM and not MUT.on("NC17"):
        return finish("ClaimExhausted")
    index = unused[0]
    n = pool_names[index]
    lock_owner = "worker" if MUT.on("NC05a") else "main"
    jfd = fs.open(proc, lock_owner, journals, n)
    opened.append(jfd)
    if not fs.flock(jfd, "EX"):
        return finish("ClaimFailed", "journal lock")
    io_fd = fs.open(proc, "worker", journals, n, writable=True)
    opened.append(io_fd)
    if io_fd.ofd.inode is not jfd.ofd.inode or any(fs.pread(io_fd, 0, len(io_fd.ofd.inode.K))):
        return finish("ClaimFailed", "identity or content")
    if MUT.on("NC05a"):
        rep["guard"][0].owner = "worker"
    rep["guard"].append(jfd)
    seen = {f["header"]["generation"] for f in rep["files"].values() if f["header"]} | \
        rep.get("generations", set())
    draws = 0
    while generation is None or not any(generation) or generation in seen:
        draws += 1
        if draws > 8:
            return finish("ClaimFailed", "generation")
        generation = sha(seed + u64(new_claim) + u8(draws))[:16]
    applied = sorted((b, REASON_TAG[rep["dispositions"][b]["reason"]]) for b in current)
    header = header_block(sel.prov["root_id"], generation, new_claim & U64_MAX, index,
                          sel.prov["c_pool"], cfg.record_capacity, applied)
    rep["claim"] = Claim(index, new_claim, generation, header, jfd, io_fd, applied)
    return finish("Ready")


def scan(world, proc, sel, root, rep, cfg, mode, sync=False, opened=None, session=None):
    """Steps 4-8 of section 10.2. `mode` is 'owner', 'verifier' or 'session'.
    Returns None or (state, reason)."""
    fs = world.fs
    prov = sel.prov
    rep["provision"], rep["selection"] = prov, sel
    opened = opened if opened is not None else []

    def sopen(directory, name):
        fd = fs.open(proc, "main", directory, name)
        opened.append(fd)
        return fd

    # Step 4: names only, counted against the bounds before any entry is examined.
    if set(root.ents_K) != set(STORE_NAMES):
        extra = set(root.ents_K) - set(STORE_NAMES)
        if any(n.startswith(".tmp-") for n in extra):
            return ("MaintenanceIncomplete", "root")
        return ("Invalid", "root entries")
    dirs = {}
    for key, ino_key in (("journals", "journals_inode"), ("dispositions", "dispositions_inode"),
                         ("archive", "archive_inode")):
        st = fs.stat(root, key, proc)
        if st.kind != "dir" or st.ino != prov[ino_key] or (st.uid, st.mode) != (0, 0o755):
            return ("Invalid", f"{key} directory")
        dirs[key] = fs.inodes[st.ino]
    st = fs.stat(dirs["dispositions"], "revoked", proc)
    if st is None or st.kind != "dir" or st.ino != prov["revoked_inode"]:
        return ("Invalid", "revoked directory")
    dirs["revoked"] = fs.inodes[st.ino]
    journals = dirs["journals"]
    pool_names = [f"j{i:05d}.journal" for i in range(prov["pool"])]
    bounds = ((dirs["dispositions"], ENTRY_LIMIT + 1), (dirs["revoked"], ENTRY_LIMIT),
              (dirs["archive"], ENTRY_LIMIT))
    if not MUT.on("NC24a"):
        for directory, limit in bounds:
            if len(directory.ents_K) > limit:
                return ("Invalid", f"enumeration bound: {fs._name_of(directory)}")
    if MUT.on("NC14a"):
        for n in pool_names:
            if n in journals.ents_K:
                fs.close(fs.open(proc, "main", journals, n, nonblock=False))
    accepts = ((journals, lambda n: n in pool_names),
               (dirs["dispositions"],
                lambda n: n == "revoked" or re.fullmatch(r"[0-9a-f]{64}\.disposition", n)
                or (MUT.on("NC12a") and n.startswith(".tmp-"))),
               (dirs["revoked"], lambda n: re.fullmatch(r"[0-9a-f]{64}-\d{8}T\d{6}Z\.disposition", n)),
               (dirs["archive"], lambda n: parse_archive_name(n) is not None))
    for directory, accept in accepts:
        names = list(directory.ents_K)
        for n in names:
            if n.startswith(".tmp-") and not accept(n):
                return ("MaintenanceIncomplete", n)
        for n in names:
            if not accept(n):
                return ("Invalid", f"unexpected entry {n}")
    for i, n in enumerate(pool_names):
        st = fs.stat(journals, n, proc)
        if st is None:
            return ("Lost", n)
        if st.kind != "file":
            return ("Invalid", f"{n} is a {st.kind}")
        if (st.uid, st.gid, st.mode, st.nlink) != (prov["uid"], prov["gid"], 0o600, 1):
            return ("Invalid", f"{n} ownership")
        if st.ino != prov["pool_inodes"][i] and not MUT.on("NC12b"):
            return ("Lost", f"{n} replaced")
    # Step 5: each pool file.
    for i, n in enumerate(pool_names):
        held = session.journal_locks.get((sel.state, i)) if session else None
        if held is not None:
            fd = held
        else:
            fd = sopen(journals, n)
            if not fs.flock(fd, "EX" if mode == "session" else "SH"):
                return ("Live", n)
        if mode == "owner" and sync and not MUT.on("NC02"):
            if fs.fdatasync(fd) != 0:
                return ("Unreliable", n)
        data = fs.pread(fd, 0, (prov["c_pool"] + 2) * BLOCK + 1)
        if held is None:
            fs.close(fd)
        if len(data) != (prov["c_pool"] + 2) * BLOCK:
            return ("Invalid", f"{n} size")
        rep["files"][i] = classify_cached(data, prov["root_id"], prov["c_pool"], i)
    rep["scanned"] = True
    rep["preserved"] = mode == "owner" and sync
    rep["durability_certified"] = bool(MUT.on("NC03") and mode == "owner" and sync)
    # Step 6: archive entries (names, blocks 0 and 1).
    archive = []
    for n in sorted(dirs["archive"].ents_K):
        parsed = parse_archive_name(n)
        st = fs.stat(dirs["archive"], n, proc)
        if st.kind != "file" or (st.uid, st.gid, st.mode, st.nlink) != (0, 0, 0o444, 1):
            return ("Invalid", f"archive {n} type")
        fd = sopen(dirs["archive"], n)
        blocks = fs.pread(fd, 0, 2 * BLOCK)
        fs.close(fd)
        archive.append((n, parsed, blocks, st.size))
    # Step 7: dispositions; revoked entries are type-checked only.
    dispositions = {}
    for n in sorted(dirs["dispositions"].ents_K):
        if n == "revoked":
            continue
        st = fs.stat(dirs["dispositions"], n, proc)
        if st.kind != "file" or (st.uid, st.gid, st.mode, st.nlink) != (0, 0, 0o444, 1):
            return ("Invalid", f"disposition {n} type")
        fd = sopen(dirs["dispositions"], n)
        data = fs.pread(fd, 0, 4097)
        fs.close(fd)
        try:
            d = parse_disposition(data)
        except ParseError as error:
            return ("Invalid", f"disposition {n}: {error}")
        if n != d["binding"].hex() + ".disposition" and not \
                (MUT.on("NC12a") and n.startswith(".tmp-")):
            return ("Invalid", f"disposition {n} name")
        if d["root"] != prov["root_id"]:
            return ("Invalid", f"disposition {n} root")
        dispositions[d["binding"]] = d
    for n in dirs["revoked"].ents_K:
        st = fs.stat(dirs["revoked"], n, proc)
        if st.kind != "file" or (st.uid, st.gid, st.mode, st.nlink) != (0, 0, 0o444, 1):
            return ("Invalid", f"revoked {n} type")
    if MUT.on("NC24a"):
        for directory, limit in bounds:
            if len(directory.ents_K) > limit:
                return ("Invalid", f"enumeration bound: {fs._name_of(directory)}")
    rep["dispositions"] = dispositions
    # Step 8: pool-level checks, incidents, history, report.
    outcome = pool_level(world, prov, rep, archive, dispositions, cfg)
    rep["report"] = build_report(rep)
    return outcome


def build_report(rep):
    """A bounded report: at most REPORT_LIMIT incidents in detail, in claim
    order, with exact totals and an explicit partial flag."""
    def order(b):
        inc = rep["incidents"][b]
        return (inc["claim"] if inc["claim"] is not None else -1,
                inc["pool_index"] if inc["pool_index"] is not None else -1, b)
    everything = sorted(rep["incidents"], key=order)
    return {"detail": everything[:REPORT_LIMIT], "total": len(everything),
            "partial": len(everything) > REPORT_LIMIT,
            "current": sum(1 for b in everything if b not in rep["history"])}


ARCHIVE_J = re.compile(r"j-(0|[1-9][0-9]{0,19})-([0-9a-f]{32})-([0-9a-f]{64})-([snum])\.journal")
ARCHIVE_P = re.compile(r"p-([0-9]{5})-([0-9a-f]{64})\.journal")


def parse_archive_name(name):
    m = ARCHIVE_J.fullmatch(name)
    if m:
        claim = int(m.group(1))
        if not 1 <= claim <= MAX_CLAIM:
            return None
        return ("j", claim, bytes.fromhex(m.group(2)), bytes.fromhex(m.group(3)), m.group(4))
    m = ARCHIVE_P.fullmatch(name)
    if m and int(m.group(1)) < 1024:
        return ("p", int(m.group(1)), bytes.fromhex(m.group(2)))
    return None


def disposition_matches(d, incident):
    if d is None:
        return False
    return all(d[key] == incident[key] for key in
               ("kind", "claim", "generation", "pool_index", "content", "class", "unsettled"))


def pool_level(world, prov, rep, archive, dispositions, cfg):
    root_id = prov["root_id"]
    retired = prov["retired"]
    claims = {}
    generations = {}
    applied = set()
    pending = set()
    archived_j = {}
    archived_p = {}
    for n, parsed, blocks, size in archive:
        if parsed[0] == "j":
            _, claim, gen, content, cls = parsed
            status, header = parse_header(blocks[0:BLOCK], root_id, None, None)
            if status != "valid" or header["claim"] != claim or header["generation"] != gen:
                return ("Invalid", f"archive {n} header")
            if size != (header["c_pool"] + 2) * BLOCK:
                return ("Invalid", f"archive {n} size")
            applied.update(b for b, _ in header["applied"])
            if claim <= retired:
                continue
            if cls == "s":
                seal = bytes(blocks[BLOCK:2 * BLOCK])
                if seal[0:4] != SEAL_MAGIC or sha(seal[0:128]) != seal[128:160] or \
                        seal[8:24] != gen or be(seal[24:32]) != claim or \
                        seal[32:64] != header["digest"]:
                    return ("Invalid", f"archive {n} seal")
            if cls in "um":
                b = bind_journal(root_id, claim, gen, content,
                                 "unresolved" if cls == "u" else "malformed", location="archive")
                if b not in dispositions:
                    return ("Invalid", f"archive {n} inconsistent")
                rep["history"].add(b)
            claims.setdefault(claim, []).append(("archive", gen, content))
            generations.setdefault(gen, []).append(("archive", claim, content))
            archived_j[(claim, gen, content)] = n
        else:
            _, index, content = parsed
            if size != (prov["c_pool"] + 2) * BLOCK:
                return ("Invalid", f"archive {n} size")
            b = bind_pool(root_id, index, content)
            if b not in dispositions:
                return ("Invalid", f"archive {n} inconsistent")
            rep["history"].add(b)
            archived_p[(index, content)] = n
    for i, r in sorted(rep["files"].items()):
        h = r["header"]
        if r["class"] == "SizeInvalid":
            return ("Invalid", f"pool {i} size")
        if h is None:
            if r["class"] == "MalformedPoolFile":
                b = bind_pool(root_id, i, r["content"])
                if (i, r["content"]) in archived_p:
                    pending.add(i)
                    continue
                rep["incidents"][b] = {"kind": "pool-file", "claim": None, "generation": None,
                                       "pool_index": i, "content": r["content"],
                                       "class": "malformed", "unsettled": 0,
                                       "outcome": "Malformed"}
            continue
        applied.update(b for b, _ in h["applied"])
        if h["claim"] <= retired:
            if MUT.on("NC12c"):
                continue
            return ("Invalid", f"pool {i} claim retired")
        key = (h["claim"], h["generation"], r["content"])
        if key in archived_j:
            pending.add(i)
            continue
        claims.setdefault(h["claim"], []).append(("pool", h["generation"], r["content"]))
        generations.setdefault(h["generation"], []).append(("pool", h["claim"], r["content"]))
        if r["class"] in ("UnsealedAction", "MalformedJournal"):
            cls = "unresolved" if r["class"] == "UnsealedAction" else "malformed"
            b = bind_journal(root_id, h["claim"], h["generation"], r["content"], cls,
                             location=("pool", i))
            rep["incidents"][b] = {"kind": "journal", "claim": h["claim"],
                                   "generation": h["generation"], "pool_index": None,
                                   "content": r["content"], "class": cls,
                                   "unsettled": len(r["unsettled"]),
                                   "outcome": "Unresolved" if cls == "unresolved" else "Malformed"}
    for claim, entries in claims.items():
        if len(entries) > 1:
            return ("Invalid", f"duplicate claim {claim}")
    for gen, entries in generations.items():
        if len(entries) > 1:
            return ("Invalid", "duplicate generation")
    live_claims = sorted(c for c in claims if c > retired)
    top = max([retired] + list(claims) + [c for (c, _, _) in archived_j])
    rep["max_claim"] = top
    rep["generations"] = set(generations)
    rep["pending_recycle"] = pending
    rep["applied"] = applied
    gap_count = (top - retired) - len(live_claims)
    if gap_count > cfg.incident_limit + len(applied):
        return ("Capacity", f"{gap_count} claim gaps")
    present = set(live_claims)
    for c in range(retired + 1, top + 1):
        if c not in present:
            b = bind_gap(root_id, c)
            rep["incidents"][b] = {"kind": "claim-gap", "claim": c, "generation": None,
                                   "pool_index": None, "content": None, "class": "malformed",
                                   "unsettled": 0, "outcome": "Malformed"}
    for b in rep["incidents"]:
        if b in applied and disposition_matches(dispositions.get(b), rep["incidents"][b]):
            rep["history"].add(b)
    return None


# -- the maintenance session (design section 13.1) -------------------------


class MaintenanceSession:
    """The maintenance capability: one process holds the store lock for the
    whole session. Verification inside it uses this object's retained lock
    descriptions, never a new lock, a descriptor number, a PID or a flag."""

    def __init__(self, world, proc, requalify=False):
        self.world, self.proc, self.fs = world, proc, world.fs
        self.requalify = requalify
        self.locks = {}             # state root name -> description holding LOCK_EX
        self.journal_locks = {}     # (state, pool index) -> description holding LOCK_EX
        self.selection = None
        self.active = False
        self.events = []
        self.interleave = None      # a test hook (NC20c only)
        self.storage = None         # R5: the admission this session's activation made
        self.last_report = None     # kept only for the NC20f mutant

    def begin(self, hook=None):
        fs = self.fs
        for attempt in range(SELECTION_ATTEMPTS):
            sel, err = select_provision(self.world, self.proc)
            if err:
                return err[0]
            opening, holder = next(_OPENINGS), {}
            root, err = open_root(self.world, self.proc, sel, pinned=not self.requalify,
                                  opening=opening, holder=holder)
            if err:
                return err[0]
            err = lock_entry(self.world, self.proc, root, sel.prov)
            if err:
                return err[0]
            fd = fs.open(self.proc, "main", root, "LOCK")
            if hook is not None:
                hook(self.world, attempt)
            if not fs.flock(fd, "EX"):
                fs.close(fd)
                return "Busy"
            if not revalidate(self.world, self.proc, sel, fd):
                fs.close(fd)
                continue
            outcome = activate(self.world, self.proc, sel, root, fd, pinned=not self.requalify,
                               opening=opening, holder=holder)
            if outcome:
                fs.close(fd)
                return outcome[0]
            self.storage = holder.get("storage")
            self.locks[sel.state] = fd
            self.selection = sel
            self.active = True
            self.events.append(("begin", sel.state))
            return "Active"
        return "SelectionChanged"

    def authorized(self, state=None):
        state = state or self.selection.state
        if MUT.on("NC20e"):
            return self.active      # Wrong: a flag stands in for the retained lock.
        fd = self.locks.get(state)
        if not self.active or fd is None or fd.closed or not self.fs.holds(fd, "EX"):
            return False
        return self.world.store_root(state).ents_K.get("LOCK") == fd.ofd.inode.ino

    def verify(self, cfg=None, state=None):
        """In-session verification (section 13.11): same checks as the standalone
        verifier, under the session's own exclusion, with no lock request."""
        cfg = cfg or CFG
        state = state or self.selection.state
        if MUT.on("NC20f") and self.last_report is not None:
            return self.last_report         # Wrong: verification skipped, a cached report reused
        if MUT.on("NC20a") or MUT.on("NC20d"):
            # Wrong: an independent shared-lock request on a new description.
            rep = startup(self.world, self.proc, cfg, sync=False, verifier=True)
            if MUT.on("NC20d") and rep["state"] == "Busy":
                rep = new_report()
                rep["state"] = "Ready"      # Wrong: Busy taken as a successful verification
            self.events.append(("verify", rep["state"]))
            return rep
        fd = self.locks.get(state)
        if MUT.on("NC20b") and fd is not None:
            self.fs.flock(fd, "SH")         # Wrong: converts the session's lock
        if MUT.on("NC20c") and fd is not None:
            self.fs.flock(fd, "UN")         # Wrong: releases the lock around verification
            if self.interleave:
                self.interleave()
            self.fs.flock(fd, "EX")
        rep = new_report()
        if not self.authorized(state):
            rep["state"] = "NotAuthorized"
            self.events.append(("verify", rep["state"]))
            return rep
        sel = self.selection if state == self.selection.state else self.select_for(state)
        lock_fd = self.locks.get(state)
        if lock_fd is not None and not revalidate(self.world, self.proc, sel, lock_fd):
            rep["state"] = "SelectionChanged"
            self.events.append(("verify", rep["state"]))
            return rep
        root = self.world.store_root(state)
        outcome = scan(self.world, self.proc, sel, root, rep, cfg, "session", session=self)
        if not outcome:
            outcome = decide(rep, cfg)
        rep["state"], rep["reason"] = outcome or ("Ready", "")
        self.events.append(("verify", rep["state"]))
        self.last_report = rep
        return rep

    def select_for(self, state):
        sel, err = select_provision(self.world, self.proc)
        assert err is None and sel.state == state
        return sel

    def reselect(self):
        """After the session itself published PROVISION, under its own lock."""
        sel, err = select_provision(self.world, self.proc)
        assert err is None, err
        self.selection = sel
        self.events.append(("reselect", sel.prov["revision"]))

    def hold_journal(self, index, state=None):
        state = state or self.selection.state
        journals = self.world.dir("journals", state=state)
        fd = self.fs.open(self.proc, "main", journals, f"j{index:05d}.journal")
        if not self.fs.flock(fd, "EX"):
            self.fs.close(fd)
            return False
        self.journal_locks[(state, index)] = fd
        return True

    def adopt(self, state):
        """Take the store lock of a state root the session has just created."""
        root = self.world.store_root(state)
        fd = self.fs.open(self.proc, "main", root, "LOCK")
        if not self.fs.flock(fd, "EX"):
            self.fs.close(fd)
            return False
        self.locks[state] = fd
        self.events.append(("adopt", state))
        return True

    def end(self):
        for fd in list(self.journal_locks.values()) + list(self.locks.values()):
            self.fs.close(fd)
        self.journal_locks.clear()
        self.locks.clear()
        self.active = False
        self.events.append(("end", None))


# -- administrative procedures, run inside a session (design section 13) ---


def disposition_steps(world, fields):
    """P-DISP (section 13.4, publication)."""
    fs = world.fs
    text = disposition_text(fields)
    tmp = ".tmp-" + fields["binding"].hex()
    final = fields["binding"].hex() + ".disposition"
    d = world.dir("dispositions")
    return [
        tagged("13.4/3", lambda: fs.create(d, tmp, "file", 0, 0, 0o444)),
        tagged("13.4/3", lambda: write_file(world, d, tmp, text)),
        tagged("13.4/3", lambda: sync_file(world, d, tmp)),
        tagged("13.4/4", lambda: fs.link(d, final, d.ents_K[tmp])),
        tagged("13.4/5", lambda: fs.unlink(d, tmp)),
        tagged("13.4/6", lambda: fs.fsync_dir(d)),
    ]


def revoke_steps(world, binding):
    """P-REVOKE (section 13.4, revocation)."""
    fs = world.fs
    d = world.dir("dispositions")
    rv = world.dir("dispositions", "revoked")
    name = binding.hex() + ".disposition"
    return [
        tagged("13.4-revoke/2", lambda: fs.rename(d, name, rv, binding.hex() + "-20261001T120000Z.disposition")),
        tagged("13.4-revoke/3", lambda: fs.fsync_dir(rv)),
        tagged("13.4-revoke/3", lambda: fs.fsync_dir(d)),
    ]


def archive_steps(world, index, rep):
    """P-ARCH (section 13.7) for a pool journal with a valid header."""
    fs = world.fs
    report = rep["files"][index]
    h = report["header"]
    letter = {"Sealed": "s", "UnsealedNoAction": "n", "UnsealedAction": "u",
              "MalformedJournal": "m"}[report["class"]]
    final = f"j-{h['claim']}-{h['generation'].hex()}-{report['content'].hex()}-{letter}.journal"
    tmp = ".tmp-" + final
    a = world.dir("archive")
    data = bytes(world.pool_inode(index).K)

    def verify_copy():
        sync_file(world, a, tmp)
        assert sha(bytes(fs.inodes[a.ents_K[tmp]].K)) == report["content"]

    steps = [
        tagged("13.7/3", lambda: fs.create(a, tmp, "file", 0, 0, 0o444)),
        tagged("13.7/3", lambda: write_file(world, a, tmp, data)),
        tagged("13.7/3", verify_copy),
        tagged("13.7/4", lambda: fs.link(a, final, a.ents_K[tmp])),
        tagged("13.7/5", lambda: fs.unlink(a, tmp)),
        tagged("13.7/5", lambda: fs.fsync_dir(a)),
    ]
    return steps, final


def recycle_steps(world, index, session=None):
    """P-RECYCLE (section 13.8), then the PROVISION rewrite (section 13.3)."""
    fs = world.fs
    j = world.dir("journals")
    tmp = f".tmp-{index:05d}"
    size = (world.C + 2) * BLOCK
    state = {}

    def create():
        state["inode"] = fs.create(j, tmp, "file", UID, GID, 0o600, size)

    def change(record):
        record["pool_inodes"][index] = state["inode"].ino

    if MUT.on("NC23"):
        hidden = [tagged("13.8/3", lambda: fs.fsync_dir(world.provdir))]   # an undocumented sync
    else:
        hidden = []
    return [
        tagged("13.8/2", create),
        tagged("13.8/2", lambda: write_file(world, j, tmp, bytes(size))),
        tagged("13.8/2", lambda: sync_file(world, j, tmp)),
        tagged("13.8/3", lambda: fs.rename(j, tmp, j, f"j{index:05d}.journal")),
        tagged("13.8/3", lambda: fs.fsync_dir(j)),
    ] + hidden + rewrite_provision_steps(world, change, session)


def leftover_steps(world, session):
    """Section 13.1 recovery: inside the session, remove every leftover temporary
    entry of an interrupted procedure, then sync each directory changed."""
    fs = world.fs
    state = session.selection.state
    dirs = [world.dir("journals", state=state), world.dir("dispositions", state=state),
            world.dir("dispositions", "revoked", state=state), world.dir("archive", state=state),
            world.provdir]
    steps = []
    for d in dirs:
        if d is world.provdir:
            names = sorted(n for n in d.ents_K if n in ("PROVISION.tmp", "PROVISION.predecessor.tmp"))
        else:
            names = sorted(n for n in d.ents_K if n.startswith(".tmp-"))
        for n in names:
            steps.append(tagged("13.1/recover", lambda d=d, n=n: fs.unlink(d, n)))
        if names:
            steps.append(tagged("13.1/recover", lambda d=d: fs.fsync_dir(d)))
    return steps


def resume_recycle_steps(world, index, rep, session):
    """Section 13.8 resumption: only when the session's verification refuses
    exactly because the pool name holds a different inode, and that inode,
    read under the session's journal lock, is a zero-filled pool file."""
    name = f"j{index:05d}.journal"
    if rep["state"] != "Lost" or rep["reason"] != f"{name} replaced":
        return None
    j = world.dir("journals", state=session.selection.state)
    st = world.fs.stat(j, name, session.proc)
    if st is None or st.kind != "file" or (st.uid, st.gid, st.mode, st.nlink) != (UID, GID, 0o600, 1):
        return None
    if not session.hold_journal(index):
        return None
    fd = session.journal_locks[(session.selection.state, index)]
    size = (session.selection.prov["c_pool"] + 2) * BLOCK
    data = world.fs.pread(fd, 0, size + 1)
    if len(data) != size or any(data):
        return None

    def change(record):
        record["pool_inodes"][index] = st.ino

    return rewrite_provision_steps(world, change, session)


def roots_with_history(world):
    """The Owner's inspection after an Unprovisioned refusal (section 13.2): every
    state root under the parent whose pool holds a non-zero byte."""
    fs = world.fs
    found = []
    for name in sorted(world.parent.ents_K):
        root = world.store_root(name)
        journals = root.ents_K.get("journals")
        if journals is None:
            continue
        if any(any(fs.inodes[ino].K) for ino in fs.inodes[journals].ents_K.values()):
            found.append(name)
    return found


def republish_steps(world, state):
    """Recovery of an interrupted provisioning (section 13.2): publish PROVISION
    for the existing root, with its recorded root id and its actual inodes."""
    fs = world.fs
    root = world.store_root(state)
    journals = fs.inodes[root.ents_K["journals"]]
    dispositions = fs.inodes[root.ents_K["dispositions"]]
    ctx = {"root": root, "lock": fs.inodes[root.ents_K["LOCK"]], "journals": journals,
           "dispositions": dispositions, "archive": fs.inodes[root.ents_K["archive"]],
           "revoked": fs.inodes[dispositions.ents_K["revoked"]],
           "pool": [journals.ents_K[f"j{i:05d}.journal"] for i in range(world.N)]}
    leftover = []
    if "PROVISION.tmp" in world.provdir.ents_K:
        leftover = [tagged("13.2/recover", lambda: fs.unlink(world.provdir, "PROVISION.tmp"))]
    return leftover + publish_provision_steps(
        world, lambda: provision_record_for(world, ctx, world.root_ids[state], state, None, None, 1),
        "13.2/recover", "13.2/recover")


def recover_unprovisioned(world):
    """Section 13.2 recovery after Unprovisioned: never a fresh replacement while
    a root may hold history. Returns what was done."""
    if MUT.on("NC-ACT-COMPOSE"):
        # R2 section 15.2: remove the incomplete root, "which holds no history",
        # and provision again: a fresh root.
        run_steps(world, provision_steps(world, bytes(range(0x40, 0x50)), "store-fresh"))
        return "provisioned afresh (R2)"
    found = roots_with_history(world)
    if len(found) == 1:
        run_steps(world, republish_steps(world, found[0]))
        return f"re-published {found[0]}"
    return "no root with history" if not found else "several roots with history: disposition"


def retirement_allowed(rep, through):
    """Section 13.9 preconditions over the complete incident set."""
    files, incidents = rep["files"], rep["incidents"]
    if MUT.on("NC24c"):
        incidents = {b: rep["incidents"][b] for b in rep["report"]["detail"]}
    for i, f in files.items():
        if f["header"] is not None and f["header"]["claim"] <= through \
                and i not in rep.get("pending_recycle", set()):
            return False
    for b, inc in incidents.items():
        if inc["claim"] is not None and inc["claim"] <= through and b not in rep["history"]:
            return False
    return True


def retire_steps(world, through, rep, session=None):
    """P-RETIRE (section 13.9): preconditions verified in the session first."""
    if not MUT.on("NC12c") and not retirement_allowed(rep, through):
        return None

    def change(record):
        record["retired"] = through

    return rewrite_provision_steps(world, change, session)


def requalify_steps(world, session, mount_options, kernel=None, storage=None):
    """Re-qualification (section 13.3): new pinned options and, R4, a newly
    qualified kernel identity; R5, a newly qualified storage identity; revision + 1."""
    def change(record):
        record["mount_options"] = mount_options
        if kernel is not None:
            record["kernel"] = kernel
        if storage is not None:
            record.update(storage)
    return rewrite_provision_steps(world, change, session)


def successor_steps(world, session, new_root_id, new_state):
    """P-SUCCESSOR (section 13.6). The session holds the predecessor's lock
    throughout and takes the successor's lock before publishing it."""
    fs = world.fs
    old = {}
    ctx = {}

    def copy_create():
        old["data"] = bytes(fs.inodes[world.provdir.ents_K["PROVISION"]].K)
        old["revision"] = world.provision_record()["revision"]
        fs.create(world.provdir, "PROVISION.predecessor.tmp", "file", 0, 0, 0o444)

    pred = "PROVISION.predecessor-" + world.root_id.hex()
    steps = [
        tagged("13.6/2a", copy_create),
        tagged("13.6/2a", lambda: write_file(world, world.provdir, "PROVISION.predecessor.tmp", old["data"])),
        tagged("13.6/2a", lambda: sync_file(world, world.provdir, "PROVISION.predecessor.tmp")),
        tagged("13.6/2a", lambda: fs.link(world.provdir, pred, world.provdir.ents_K["PROVISION.predecessor.tmp"])),
        tagged("13.6/2a", lambda: fs.unlink(world.provdir, "PROVISION.predecessor.tmp")),
        tagged("13.6/2a", lambda: fs.fsync_dir(world.provdir)),
    ]
    world.root_ids[new_state] = new_root_id
    steps += [tagged("13.6/2b", s) for s in store_steps(world, new_state, ctx)]

    def adopt():
        assert session.adopt(new_state), "the successor's lock was not free"

    publish = publish_provision_steps(
        world, lambda: provision_record_for(world, ctx, new_root_id, new_state, world.root_id,
                                            "predecessor fully dispositioned",
                                            old["revision"] + 1), "13.6/2c", "13.6/2c",
        before=adopt)
    last = publish[-1]
    publish[-1] = tagged(last.doc, lambda: (last(), session.reselect()))
    return steps + publish


def _clone_world(world):
    return copy.deepcopy(world)


# ===========================================================================
# 10. The recorder: exchange, worker, owner, admission gate (design section 9)
# ===========================================================================


class LedgerModel:
    """Source-derived model of Ledger::acknowledge and Ledger::fail (core.rs:1042-1090)."""

    def __init__(self, generation):
        self.generation = generation
        self.digests = []
        self.next_ack = 1
        self.failed = None

    def acknowledge(self, gen, seq, digest):
        if gen != self.generation:
            return "Foreign"
        if seq == 0 or seq > len(self.digests):
            return "NotIssued"
        expected = self.digests[seq - 1]
        if seq < self.next_ack:
            return "Duplicate" if digest == expected else "Conflict"
        if self.failed is not None:
            return "LedgerFailed"
        if seq > self.next_ack:
            return "OutOfOrder"
        if digest != expected:
            return "Conflict"
        self.next_ack += 1
        return "Acknowledged"

    def fail(self, gen, seq):
        if gen != self.generation:
            return "Foreign"
        if self.failed is not None:
            return "LedgerFailed"
        if seq == 0 or seq > len(self.digests):
            return "NotIssued"
        if seq < self.next_ack:
            return "Conflict"
        if seq > self.next_ack:
            return "OutOfOrder"
        self.failed = seq
        return "FailureRecorded"


class Exchange:
    """The state-based exchange (section 9.2). One mutex (modelled as atomic
    methods) guards the slots, the latched fatal state and publication; the
    store admission gate takes the same mutex (section 9.8)."""

    def __init__(self, generation, capacity, events=None):
        self.generation, self.capacity = generation, capacity
        self.slots = [None] * capacity
        self.submitted_through = 0
        self.claim = "None"
        self.seal = "None"
        self.durable_through = 0
        self.fatal = None           # (first cause, P = durable_through at the latch)
        self.later = 0              # causes after the first, counted
        self.pending = None
        self.poisoned = False       # a panic while the mutex was held
        self.waits = 0
        self.dropped = 0
        self.events = events if events is not None else []

    def footprint(self):
        return len(self.slots)

    def _poison_check(self):
        if self.poisoned and self.fatal is None:
            self._latch(("Poisoned",))
        return self.poisoned

    def _latch(self, cause):
        if self.fatal is None:
            self.fatal = (cause, self.durable_through)
            if self.claim == "Requested":
                self.claim = "Failed"
            if self.seal == "Requested":
                self.seal = "Failed"
            self.events.append(("latch", self.durable_through, cause[0]))
        else:
            self.later += 1

    def latch(self, cause):
        self._poison_check()
        self._latch(cause)

    def healthy(self):
        return not self._poison_check() and self.fatal is None

    def submit(self, intent):
        """RecordSink::submit (section 9.3): contiguous, idempotent, never blocking."""
        if self._poison_check() or self.fatal is not None:
            return
        seq = intent.seq
        if self.claim != "Claimed":
            return self._latch(("InvalidSubmission", seq, "before the claim"))
        if intent.gen != self.generation:
            return self._latch(("InvalidSubmission", seq, "foreign generation"))
        if seq == 0 or seq > self.capacity:
            return self._latch(("InvalidSubmission", seq, "out of range"))
        if seq <= self.submitted_through:
            if self.slots[seq - 1].digest != intent.digest:
                self._latch(("Conflict", seq))
            return
        if seq == self.submitted_through + 1:
            self.slots[seq - 1] = intent
            self.submitted_through = seq
            return
        if MUT.on("NC18c"):
            self.slots[seq - 1] = intent        # R1: a future slot accepted silently
            return
        self._latch(("InvalidSubmission", seq, "future sequence"))

    def next_for_worker(self):
        if self._poison_check() or self.fatal is not None:
            return None
        nxt = self.durable_through + 1
        if nxt > self.capacity:
            return None
        return self.slots[nxt - 1]

    def publish(self, seq):
        """W6, under the mutex: only while healthy."""
        if not MUT.on("NC18d") and (self._poison_check() or self.fatal is not None):
            return False
        self.durable_through = seq
        self.pending = None
        self.events.append(("publish", seq))
        return True

    def claimed(self):
        self.claim = "Claimed" if self.healthy() else "Failed"

    def sealed(self):
        self.seal = "Sealed" if self.healthy() else "Failed"

    def owner_view(self):
        self._poison_check()
        return {"durable_through": self.durable_through, "fatal": self.fatal,
                "claim": self.claim, "seal": self.seal}


class LegacyQueueExchange(Exchange):
    """The first candidate's shape (negative controls NC07 and NC08): bounded
    message queues of capacity + 2, re-acknowledgement of duplicates, failure
    delivered as a message."""

    def __init__(self, generation, capacity, events=None):
        super().__init__(generation, capacity, events)
        self.inbox = collections.deque()
        self.queue = collections.deque()
        self.limit = capacity + 2
        self.written = 0
        self.view_durable = 0
        self.view_fatal = None

    def footprint(self):
        return len(self.slots) + len(self.inbox) + len(self.queue)

    def submit(self, intent):
        if len(self.inbox) >= self.limit:
            self.waits += 1          # the owner would block on a full queue
            return
        self.inbox.append(intent)

    def _emit(self, event):
        if len(self.queue) >= self.limit:
            self.dropped += 1        # a full queue: the message is lost
            return
        self.queue.append(event)

    def next_for_worker(self):
        while self.inbox:
            intent = self.inbox.popleft()
            if intent.seq <= self.written:
                self._emit(("ack", intent.seq))     # re-acknowledgement
                continue
            if self.fatal is None and intent.seq == self.written + 1:
                return intent
        return None

    def publish(self, seq):
        self.written = seq
        self.durable_through = seq
        self._emit(("ack", seq))
        return True

    def latch(self, cause):
        if self.fatal is None:
            self.fatal = (cause, self.written)
        self._emit(("failed", cause))

    def owner_view(self):
        while self.queue:
            event = self.queue.popleft()
            if event[0] == "ack":
                self.view_durable = max(self.view_durable, event[1])
            elif self.view_fatal is None:
                self.view_fatal = (event[1], self.view_durable)
        return {"durable_through": self.view_durable, "fatal": self.view_fatal,
                "claim": self.claim, "seal": self.seal}


class CoreModel:
    """The core's issuing side, as far as the recorder sees it (fixture runs)."""

    def __init__(self, generation):
        self.generation = generation
        self.ledger = LedgerModel(generation)
        self.intents = []
        self.unsent = collections.deque()
        self.at = 0

    def issue(self, kind):
        seq = len(self.intents) + 1
        frame = encode_record(self.generation, seq, self.at, kind)
        record = Record(self.generation, seq, self.at, kind, frame[-32:])
        self.at += 1
        self.intents.append(record)
        self.ledger.digests.append(record.digest)
        self.unsent.append(record)
        return record

    def flush(self, sink, panic_after_store=False, duplicates=0):
        """flush_records (core.rs:2958-2991): a sink panic keeps the record unsent."""
        while self.unsent:
            record = self.unsent[0]
            sink.submit(record)
            for _ in range(duplicates):
                sink.submit(record)
            if panic_after_store:
                return False
            self.unsent.popleft()
        return True

    def acknowledge(self, seq, digest):
        return self.ledger.acknowledge(self.generation, seq, digest)

    def record_failed(self, seq):
        return self.ledger.fail(self.generation, seq)

    def evidence(self):
        """EvidenceView (model.rs:837-845): issued, acknowledged, failed."""
        return len(self.ledger.digests), self.ledger.next_ack - 1, self.ledger.failed


class CoreSim(CoreModel):
    """A source-derived model of the core paths the fatal protocol depends on:
    start (core.rs:1459-1488), case begin (core.rs:1519-1566), reservation
    (core.rs:1579-1620), admission (core.rs:1626-1697), settlement
    (core.rs:1700-1704), case end (core.rs:2157-2212), stop and commitment
    (core.rs:2624-2687), acknowledgement and record failure
    (core.rs:2996-3031), fail-stop (core.rs:3046-3069) and close
    (core.rs:2929-2954). Not the Rust core."""

    def __init__(self, generation, events=None):
        super().__init__(generation)
        self.phase = "NotStarted"
        self.case = None
        self.op = None
        self.held = set()
        self.closure = None
        self.closure_recorded = False
        self.committed = False
        self.terminal = None
        self.admitted = 0
        self.next_action = 1
        self.failures = []
        self.faults = []
        self.native = []
        self.events = events if events is not None else []

    def acknowledge(self, seq, digest):
        out = self.ledger.acknowledge(self.generation, seq, digest)
        if out == "Acknowledged":
            self.try_finalize()
            if self.phase == "Finalizing" and self.ledger.next_ack > self.terminal:
                self.phase = "Finalized"
        return out

    def record_failed(self, seq):
        out = self.ledger.fail(self.generation, seq)
        if out == "FailureRecorded":
            self.fail("RecordFailed")
        return out

    def observe_closure(self):
        if self.closure is not None and not self.closure_recorded:
            self.closure_recorded = True
            reason, cls, after = self.closure
            self.issue(AC(reason, cls, after))

    def fail(self, cls):
        if self.committed:
            self.faults.append(cls)
            return
        self.failures.append(cls)
        if self.closure is None:
            self.closure = ("Failed", cls, self.admitted)
        self.observe_closure()

    def start_run(self):
        if self.phase != "NotStarted":
            return "Phase"
        if self.ledger.failed is not None:
            return "EvidenceFailed"
        if self.closure is not None:
            self.observe_closure()
            return "AdmissionClosed"
        self.issue(RS())
        self.phase = "Running"
        return "Started"

    def begin_case(self):
        if self.closure is not None:
            self.observe_closure()
            return "AdmissionClosed"
        if self.phase != "Running" or self.case is not None:
            return "Phase"
        if self.ledger.failed is not None:
            return "EvidenceFailed"
        if self.held or self.op is not None:
            return "GroupUnresolved"
        if self.ledger.next_ack <= len(self.ledger.digests):
            return "EvidencePending"
        self.issue(CS())
        self.case = 1
        return "Begun"

    def reserve(self):
        if self.closure is not None:
            self.observe_closure()
            return "AdmissionClosed"
        if self.phase != "Running" or self.case is None:
            return "Phase"
        if self.ledger.failed is not None:
            return "EvidenceFailed"
        if self.op is not None:
            return "OperationPending"
        action = self.next_action
        self.next_action += 1
        record = self.issue(AS(action))
        self.op = {"action": action, "start": record.seq, "phase": "Reserved"}
        return action

    def admit(self, action):
        self.observe_closure()
        op = self.op
        if op is None or op["action"] != action or op["phase"] != "Reserved":
            return "NotReserved"
        if self.ledger.failed is not None:
            self.settle_unadmitted()
            return "EvidenceFailed"
        if self.ledger.next_ack <= op["start"]:
            return "EvidencePending"
        self.events.append(("admit", action))          # the gate's linearization point
        if self.closure is not None:
            self.settle_unadmitted()
            return "AdmissionClosed"
        self.admitted += 1
        op["phase"] = "InFlight"
        return "Admitted"

    def start_native(self, action):
        """The integration starts a native operation only with an admission."""
        assert self.op is not None and self.op["action"] == action and self.op["phase"] == "InFlight"
        self.native.append(action)
        self.events.append(("native", action))

    def complete_created(self, action):
        if self.op is not None and self.op["action"] == action and self.op["phase"] == "InFlight":
            self.held.add(action)
            self.op = None

    def settle_unadmitted(self):
        if self.op is not None and self.op["phase"] == "Reserved":
            self.issue(ASET(self.op["action"], "NotAdmitted"))
            self.op = None

    def end_case(self, cleanup=True):
        if self.phase != "Running" or self.case is None:
            return "Phase"
        self.observe_closure()
        failed_before = len(self.failures)
        if self.op is not None and self.op["phase"] == "Reserved":
            self.settle_unadmitted()
        elif self.op is not None:
            self.issue(AF(self.op["action"]))
            self.fail("UnknownOutcome")
        for action in sorted(a for a in self.held if isinstance(a, int)):
            if cleanup:
                self.held.discard(action)
                self.issue(ASET(action, "Confirmed"))
        passed = len(self.failures) == failed_before and not self.held and self.op is None
        self.issue(CE(1, passed))
        self.case = None
        if self.held or self.op is not None or self.closure is not None or \
                self.ledger.failed is not None:
            self.stop()
        return "Ended"

    def finish_run(self):
        if self.phase != "Running" or self.case is not None:
            return "Phase"
        self.stop()
        return self.phase

    def stop(self):
        if self.phase != "Running":
            return
        self.observe_closure()
        if self.held or self.op is not None:
            self.phase = "RecoveryRequired"
            self.issue(RR())
        else:
            self.phase = "Candidate"
            self.try_finalize()

    def try_finalize(self):
        if self.phase != "Candidate" or self.ledger.failed is not None:
            return
        if self.ledger.next_ack <= len(self.ledger.digests) or self.held or self.op is not None:
            return
        self.committed = True
        verdict = "Failed" if (self.failures or self.closure is not None) else "Passed"
        self.terminal = self.issue(RE(verdict)).seq
        self.phase = "Finalizing"

    def late_owner(self, action):
        """An owner delivered after the operation retired: held as an incident."""
        incident = 1 + sum(1 for a in self.held if not isinstance(a, int))
        self.held.add(("late", incident))
        self.issue(IO(incident, action))
        self.fail("LateOwner")

    def close(self):
        untouched = self.phase == "NotStarted" and not self.intents
        if not (untouched or self.phase == "Finalized") or self.held or self.op is not None:
            return "Refused"
        if self.ledger.failed is not None or self.ledger.next_ack <= len(self.ledger.digests):
            return "Refused"
        self.phase = "Closed"
        return "Closed"


class Owner:
    """The owner's apply step and failure delivery (sections 9.4, 9.5, 9.8)."""

    def __init__(self, core, exchange, on_ack=None, worker=None):
        self.core, self.exchange, self.worker = core, exchange, worker
        self.retained = {}          # the owner's own copies of submitted intents
        self.applied = 0
        self.delivered = 0
        self.stopped = False
        self.failure_outcome = None
        self.failure_target = None
        self.faults = []
        self.ack_calls = collections.Counter()
        self.outcomes = []
        self.on_ack = on_ack

    def submit(self, intent):
        """The exchange-backed RecordSink."""
        self.retained.setdefault(intent.seq, intent)
        self.exchange.submit(intent)

    def watch(self):
        """JoinHandle::is_finished without a latched normal exit latches loss."""
        if self.worker is not None and self.worker.finished and not self.worker.normal_exit:
            self.exchange.latch(("WorkerVanished",))

    def apply(self):
        self.watch()
        view = self.exchange.owner_view()
        limit = view["durable_through"]
        self.delivered = max(self.delivered, limit)
        while self.applied < limit and not self.stopped:
            seq = self.applied + 1
            intent = self.retained.get(seq) or self.core.intents[seq - 1]
            outcome = self.core.acknowledge(seq, intent.digest)
            self.ack_calls[seq] += 1
            self.outcomes.append(outcome)
            if self.on_ack:
                self.on_ack(seq, outcome)
            if outcome != "Acknowledged":
                if MUT.on("NC18e"):
                    self.applied = seq      # Wrong: an unexpected outcome ignored
                    continue
                self.exchange.latch(("UnexpectedAck", seq, outcome))
                self.stopped = True
                break
            self.applied = seq
        view = self.exchange.owner_view()
        if view["fatal"] is not None:
            self.deliver_failure(view)

    def deliver_failure(self, view):
        """record_failed only for the core's next unacknowledged record, and only
        once that record has been issued (section 9.5)."""
        if self.failure_outcome is not None:
            return
        issued, acknowledged, failed = self.core.evidence()
        if failed is not None:
            self.failure_outcome = "LedgerFailed"
            return
        if MUT.on("NC18b"):
            # R1: the cause's own sequence (or durable + 1), only at that exact position.
            cause = view["fatal"][0]
            first_bad = cause[1] if len(cause) > 1 and isinstance(cause[1], int) \
                else view["durable_through"] + 1
            if self.applied != first_bad - 1 or first_bad > issued:
                return
            self.failure_target = first_bad
            self.failure_outcome = self.core.record_failed(first_bad)
            return
        target = acknowledged + 1
        if MUT.on("NC18f"):
            # Wrong: report even when no record exists there, and count it as delivered.
            self.failure_target = target
            self.core.record_failed(target)
            self.failure_outcome = "FailureRecorded"
            return
        if target > issued:
            return                      # no real record yet; try again later
        self.failure_target = target
        outcome = self.core.record_failed(target)
        self.failure_outcome = outcome
        if outcome not in ("FailureRecorded", "LedgerFailed"):
            self.faults.append(("record_failed", target, outcome))


class AdmissionGate:
    """The store admission gate (section 9.8): the integration calls
    Custody::admit only here. One critical section of the exchange mutex
    covers the health check and the core's admission."""

    def __init__(self, exchange):
        self.exchange = exchange

    def admit(self, core, action):
        if MUT.on("NC18a"):
            return core.admit(action)           # R1: no store gate
        if not self.exchange.healthy():
            return "RecorderFatal"
        return core.admit(action)

    def admit_steps(self, core, action, result):
        """The admission as atomic scheduling steps for the interleaving checks."""
        if MUT.on("NC19"):
            # Wrong: a check, then an unprotected admission call.
            def check():
                result["healthy"] = self.exchange.fatal is None

            def call():
                result["admit"] = core.admit(action) if result["healthy"] else "RecorderFatal"
            return [check, call]

        def both():
            result["admit"] = self.admit(core, action)
        return [both]


class Worker:
    """The recorder worker (section 9.4). `steps()` yields after each step so
    that crash points and interleavings can be explored."""

    def __init__(self, world, io_fd, exchange, index, state=None):
        self.world, self.fs, self.fd, self.ex, self.index = world, world.fs, io_fd, exchange, index
        self.state = state or world.state     # W5 rechecks through the opened root's journals
        self.normal_exit = False
        self.finished = False

    def _write_block(self, offset, block):
        written = 0
        stalls = 0
        while written < len(block):
            result = self.fs.pwrite(self.fd, offset + written, block[written:])
            if result == "EINTR" or result == 0:
                stalls += 1
                if stalls > 3:
                    return False
                continue
            written += result
        return True

    def _sync(self):
        tries = 0
        while True:
            result = self.fs.fdatasync(self.fd)
            if result == "EINTR":
                tries += 1
                if tries > 3:
                    return "EINTR"
                continue
            return result

    def _identity_ok(self):
        inode = self.fd.ofd.inode
        journals = self.world.dir("journals", state=self.state)
        return journals.ents_K.get(f"j{self.index:05d}.journal") == inode.ino and \
            self.fs.nlink(inode.ino) == 1

    def claim(self, header):
        self.ex.claim = "Requested"
        if not self._write_block(0, header):
            self.ex.latch(("ClaimWrite",))
            return
        yield "claim-written"
        if self._sync() != 0 or not self._identity_ok():
            self.ex.latch(("ClaimSync",))
            return
        self.ex.claimed()
        yield "claimed"

    def seal(self, block):
        self.ex.seal = "Requested"
        if not self._write_block(BLOCK, block):
            self.ex.latch(("SealWrite",))
            return
        yield "seal-written"
        if self._sync() != 0:
            self.ex.latch(("SealSync",))
            return
        self.ex.sealed()
        yield "sealed"

    def steps(self):
        while True:
            intent = self.ex.next_for_worker()
            if intent is None:
                return
            seq = intent.seq
            self.ex.pending = seq
            frame = encode_record(intent.gen, intent.seq, intent.at, intent.kind)
            if frame[-32:] != intent.digest or not 73 <= len(frame) <= 123:
                self.ex.latch(("Encode", seq))
                return
            if not self._write_block(BLOCK * (seq + 1), record_block(frame)):
                self.ex.latch(("Write", seq))
                return
            yield ("written", seq)
            if MUT.on("NC15a"):
                # Wrong: durable published before the sync.
                self.ex.publish(seq)
                yield ("published", seq)
            result = self._sync()
            if result == "EIO" and MUT.on("NC15b"):
                # Wrong: retry after EIO; errseq reports an error once per description.
                result = self._sync()
            if result != 0:
                self.ex.latch(("Sync", seq, result))
                return
            yield ("synced", seq)
            if not self._identity_ok():
                self.ex.latch(("Identity", seq))
                return
            if not MUT.on("NC15a"):
                if not self.ex.publish(seq):
                    return              # a fatal condition latched meanwhile
                yield ("published", seq)

    def abort(self, drop_guard=True):
        """The worker unwinds (panic) or dies: its own descriptor closes. Without the
        drop guard (an abort path) only the owner's join-handle check notices."""
        self.fs.kill_owner(self.fd.proc, "worker")
        self.finished = True
        if drop_guard:
            self.ex.latch(("WorkerLost",))


def interleavings(lengths):
    """Every merge of threads with the given numbers of atomic steps."""
    if not any(lengths):
        yield []
        return
    for t, n in enumerate(lengths):
        if n:
            rest = list(lengths)
            rest[t] -= 1
            for tail in interleavings(rest):
                yield [t] + tail


# ===========================================================================
# 11. Source-derived design fixtures (design section 11.5)
# ===========================================================================

G_RUN = bytes([0x3C]) * 16


def RS(n=0):
    return ("RunStarted", n)


def CS(case=1, expectation="Clean"):
    return ("CaseStarted", case, expectation)


def AS(n=1, slot="Process"):
    return ("ActionStarted", (G_RUN, n), slot)


def AF(n=1):
    return ("ActionFailed", (G_RUN, n))


def ASET(n=1, how="Confirmed"):
    return ("ActionSettled", (G_RUN, n), how)


def CE(case=1, passed=True):
    return ("CaseEnded", case, passed)


def AC(closure, inner, after):
    return ("AdmissionClosed", (closure, inner), after)


def SR():
    return ("ShutdownRefused",)


def RR():
    return ("RecoveryRequired",)


def RA(n, resolved):
    return ("RecoveryAttempt", n, resolved)


def RE(verdict, by=None):
    return ("RunEnded", verdict, by)


def IO(i=1, a=1, slot="Process"):
    return ("IncidentOpened", (G_RUN, i), (G_RUN, a), slot)


def IS(i=1, how="Confirmed"):
    return ("IncidentSettled", (G_RUN, i), how)


def r(kind):
    return ("rec", kind)


def nat(sign, name):
    return ("nat" + sign, name)


T1 = [r(RS()), r(CS()), r(AS()), nat("+", "a1"), nat("-", "a1"), r(ASET()), r(CE()),
      r(RE("Passed"))]
TRACES = {
    "T1": T1,
    "T2": [r(AC("Cancelled", "Requested", 0)), r(SR())],
    "T3": [r(RS()), r(CS()), r(AS()), nat("+", "a1"), r(AC("Cancelled", "Requested", 1)),
           nat("-", "a1"), r(ASET()), r(CE()), r(RE("Failed"))],
    "T4": [r(RS()), r(CS()), r(AS()), nat("+", "a1"), r(AF()),
           r(AC("Failed", "UnexpectedCleanup", 1)), r(CE(1, False)), r(RR()), nat("-", "a1"),
           r(ASET()), r(RA(1, True)), r(RE("Failed", 1))],
    "T5": [r(RS()), r(CS()), r(AS()), nat("+", "a1"), r(AF()),
           r(AC("Failed", "UnknownOutcome", 1)), r(CE(1, False)), r(RR()), nat("-", "a1"),
           r(ASET(1, "NothingCreated")), r(RA(1, True)), r(RE("Failed", 1))],
    "T6": [r(RS()), r(CS()), r(AS()), nat("+", "a1"), r(AF()),
           r(AC("Failed", "AuthorityLost", 1)), r(CE(1, False)), r(RR())],
    "T7": T1 + [nat("+", "i1"), r(IO()), nat("-", "i1"), r(IS()), r(RA(1, True))],
    "T9": [r(RS()), r(CS(1, "RetainedBoundary")), r(AS()), nat("+", "a1"), nat("-", "a1"),
           r(ASET()), r(CE()), r(RE("Passed"))],
    "T10": [r(RS()), r(CS(1, "OutputDetached")), r(AS()), nat("+", "a1"), nat("-", "a1"),
            r(ASET(1, "OutputLost")), r(CE()), r(RE("Passed"))],
    "T11": [r(RS()), r(CS()), r(AS()), nat("+", "a1"), r(AC("Cancelled", "Shutdown", 1)),
            r(SR()), nat("-", "a1"), r(ASET()), r(CE()), r(RE("Failed"))],
    "T13": [r(RS()), r(CS()), r(AS()), nat("+", "a1"), nat("-", "a1"), r(ASET()), r(CE()),
            nat("+", "i1"), r(IO()), r(AC("Failed", "LateOwner", 1)), r(RR()),
            nat("-", "i1"), r(IS()), r(RA(1, True)), r(RE("Failed", 1))],
    "T14": [r(RS()), r(CS()), r(AS()), r(AC("Cancelled", "Requested", 0)),
            r(ASET(1, "NotAdmitted")), r(CE()), r(RE("Failed"))],
    "T15": [r(RS()), r(CS()), r(AS(1, "Workspace")), nat("+", "a1"), r(AF()),
            r(AC("Failed", "UnexpectedOwner", 1)), nat("-", "a1"), r(ASET()),
            r(CE(1, False)), r(RE("Failed"))],
}


def trace_kinds(name):
    return [event[1] for event in TRACES[name] if event[0] == "rec"]


def records_of(kinds, generation=G_RUN, ats=None):
    out = []
    for i, kind in enumerate(kinds):
        at = i if ats is None else ats[i]
        frame = encode_record(generation, i + 1, at, kind)
        out.append(Record(generation, i + 1, at, kind, frame[-32:]))
    return out


# ===========================================================================
# 12. Running a fixture through the recorder with crash-point capture
# ===========================================================================

CFG = Config(record_capacity=16, incident_limit=16)


class Run:
    """One generation: startup and claim, then the fixture's events through the
    exchange, worker and owner; a snapshot of the journal at every step."""

    def __init__(self, name, lazy=False, fail_sync_at=None, exchange_cls=Exchange,
                 on_ack=None, interleave=False):
        self.name = name
        self.interleave = interleave
        self.world = World(n=2, c_pool=16)
        self.proc = self.world.fs.register(Proc("owner"))
        rep = startup(self.world, self.proc, CFG, claim=True, generation=G_RUN)
        assert rep["state"] == "Ready", f"fixture start-up failed: {rep['state']} {rep['reason']}"
        self.claim = rep["claim"]
        self.guard = rep["guard"]
        self.inode = self.claim.io_fd.ofd.inode
        self.core = CoreModel(G_RUN)
        self.exchange = exchange_cls(G_RUN, CFG.record_capacity)
        self.worker = Worker(self.world, self.claim.io_fd, self.exchange, self.claim.index)
        self.owner = Owner(self.core, self.exchange, on_ack, worker=self.worker)
        self.native = set()
        self.snapshots = []
        self.lazy = lazy
        self.fail_sync_at = fail_sync_at
        self.header = parse_header(self.claim.header, ROOT_ID, 16, self.claim.index)[1]
        for label in self.worker.claim(self.claim.header):
            self.snap(label)
        assert self.exchange.claim == "Claimed"

    def snap(self, label):
        i = self.inode
        self.snapshots.append({"label": label, "K": bytes(i.K), "D": bytes(i.D),
                               "pending": frozenset(i.pending), "native": frozenset(self.native),
                               "applied": self.owner.applied})

    def fatal(self):
        return self.exchange.owner_view()["fatal"] is not None

    def drain(self, until=None):
        """Run the worker and the owner's apply step until `until` is acknowledged
        (or everything submitted is durable), capturing every step."""
        while True:
            self.core.flush(self.owner)
            progressed = False
            for label in self.worker.steps():
                progressed = True
                self.snap(label)
                if self.interleave:
                    self.owner.apply()
            self.owner.apply()
            self.snap("applied")
            if self.fatal():
                return False
            if until is not None and self.core.ledger.next_ack > until:
                return True
            if not progressed:
                return until is None or self.core.ledger.next_ack > until

    def start_seq(self, action_name):
        n = int(action_name[1:])
        for record in self.core.intents:
            if record.kind[0] == "ActionStarted" and record.kind[1][1] == n:
                return record.seq
        return None

    def play(self):
        for event in TRACES[self.name]:
            if self.fatal():
                break
            if event[0] == "rec":
                kind = event[1]
                if kind[0] in ("CaseStarted", "RunEnded") and self.core.intents:
                    # core.rs:1542-1544 and core.rs:2663-2669: everything acknowledged first.
                    if not self.drain(until=len(self.core.intents)):
                        break
                record = self.core.issue(kind)
                if self.fail_sync_at == record.seq:
                    self.world.fs.wb_fail.add((self.inode.ino, record.seq + 1))
                if not self.lazy:
                    self.drain(until=record.seq)
            elif event[0] == "nat+":
                if event[1].startswith("a"):
                    seq = self.start_seq(event[1])
                    # Admission needs the acknowledged start record (core.rs:1658-1664).
                    if not self.drain(until=seq):
                        break
                    assert self.core.ledger.next_ack > seq, "write-ahead violated by the driver"
                self.native.add(event[1])
                self.snap(f"native+{event[1]}")
            else:
                self.native.discard(event[1])
                self.snap(f"native-{event[1]}")
        self.drain()
        self.snap("end")
        return self

    def closable(self):
        kinds = [x.kind for x in self.core.intents]
        names = [k[0] for k in kinds]
        _, summary = check_grammar(self.core.intents, G_RUN, 0)
        return (not self.native and not self.fatal()
                and self.core.ledger.next_ack == len(self.core.intents) + 1
                and ("RunStarted" not in names or "RunEnded" in names)
                and not summary["unsettled_actions"] and not summary["unsettled_incidents"])

    def seal(self):
        block = seal_for(self.header, self.core.intents)
        for label in self.worker.seal(block):
            self.snap(label)
        return self


def restart_images(snapshot):
    """Byte images a restart can read after a crash at this snapshot (section 4)."""
    k, d, pending = snapshot["K"], snapshot["D"], snapshot["pending"]
    images = {"F1": k}
    reclaimed = bytearray(k)
    for block in range((len(k) + BLOCK - 1) // BLOCK):
        if block not in pending:
            lo, hi = block * BLOCK, min((block + 1) * BLOCK, len(k))
            durable = d[lo:hi]
            reclaimed[lo:hi] = durable + bytes((hi - lo) - len(durable))
    if bytes(reclaimed) != k:
        images["F1+page-reclaim"] = bytes(reclaimed)
    if k != d and not pending:
        images["F1+inode-evict"] = d
    for outcome in TEAR_OUTCOMES:
        image = bytearray(d)
        for block in pending:
            lo = block * BLOCK
            new = k[lo:lo + BLOCK]
            if outcome == "new":
                image[lo:lo + BLOCK] = new
            elif outcome == "garbage":
                mixed = bytearray(new)
                for i in range(0, len(mixed), 97):
                    mixed[i] ^= 0x5A
                image[lo:lo + BLOCK] = mixed
            elif outcome != "old":
                image[lo:lo + outcome[1]] = new[:outcome[1]]
        images[f"F2:{outcome}"] = bytes(image)
    synced = bytearray(d)
    for block in pending:
        synced[block * BLOCK:(block + 1) * BLOCK] = k[block * BLOCK:(block + 1) * BLOCK]
    images["F1>sync>F2"] = bytes(synced)
    return images


class Rig:
    """One generation with the source-derived core model (CoreSim), the store
    admission gate, the exchange, the worker and the owner (section 9.8)."""

    def __init__(self):
        self.events = []
        self.world = World(n=2, c_pool=16)
        self.proc = self.world.fs.register(Proc("owner"))
        rep = startup(self.world, self.proc, CFG, claim=True, generation=G_RUN)
        assert rep["state"] == "Ready", rep
        self.claim = rep["claim"]
        self.inode = self.claim.io_fd.ofd.inode
        self.core = CoreSim(G_RUN, self.events)
        self.exchange = Exchange(G_RUN, CFG.record_capacity, self.events)
        self.worker = Worker(self.world, self.claim.io_fd, self.exchange, self.claim.index)
        self.owner = Owner(self.core, self.exchange, worker=self.worker)
        self.gate = AdmissionGate(self.exchange)
        self.header = parse_header(self.claim.header, ROOT_ID, 16, self.claim.index)[1]
        for _ in self.worker.claim(self.claim.header):
            pass

    def pump(self):
        """Flush, let the worker write everything it can, and apply."""
        self.core.flush(self.owner)
        for _ in self.worker.steps():
            pass
        self.owner.apply()

    def do(self, fn, *args):
        result = fn(*args)
        self.pump()
        return result

    def reserved(self):
        """RunStarted, CaseStarted and ActionStarted durable and acknowledged;
        the reservation not yet admitted."""
        assert self.do(self.core.start_run) == "Started"
        assert self.do(self.core.begin_case) == "Begun"
        action = self.do(self.core.reserve)
        assert self.exchange.durable_through == self.owner.applied == 3
        return action

    def latch_index(self):
        return next((i for i, e in enumerate(self.events) if e[0] == "latch"), None)


# ===========================================================================
# 13. Checks
# ===========================================================================


def check_c00_golden():
    """[golden-codec] The model's frames are version-1 frames."""
    assert len(GOLDEN_RECORD_VECTORS) == 50, "[golden-codec] expected the 50 RECORD_VECTORS entries"
    for line, covered, digest, rid, at, kind in GOLDEN_RECORD_VECTORS:
        gen, seq = _resolve_golden(rid)
        k = _resolve_golden(kind)
        frame = encode_record(gen, seq, at, k)
        assert frame[:-32].hex() == covered.replace(" ", ""), f"[golden-codec] frame at line {line}"
        assert frame[-32:].hex() == digest, f"[golden-codec] digest at line {line}"
        decoded = decode_record(frame)
        assert (decoded.gen, decoded.seq, decoded.at, decoded.kind) == (gen, seq, at, k), \
            f"[golden-codec] decode at line {line}"
        assert MIN_RECORD_PAYLOAD <= be(frame[6:8]) <= MAX_RECORD_PAYLOAD, \
            f"[golden-codec] payload range at line {line}"
    return "50 golden vectors reproduced and decoded"


def check_c01_f1_volatile():
    """[F1-volatile] Process death leaves the durable state unchanged."""
    run = Run("T1")
    run.core.issue(RS())
    run.core.flush(run.owner)
    steps = run.worker.steps()
    label = next(steps)
    assert label == ("written", 1)
    block = run.inode.K[2 * BLOCK:3 * BLOCK]
    assert any(block), "[F1-volatile] the record should be kernel-visible"
    run.world.fs.kill(run.proc)
    durable = bytes(run.inode.D[2 * BLOCK:3 * BLOCK])
    assert not any(durable), "[F1-volatile] process death made a pending record durable"
    assert bytes(run.inode.K[2 * BLOCK:3 * BLOCK]) == bytes(block), \
        "[F1-volatile] process death changed kernel-visible bytes"
    assert run.world.fs.locks == {}, "[F1-volatile] process death must release its locks"
    return "after F1: K shows the record, D does not, locks released"


def _f1_record_world():
    run = Run("T1")
    run.core.issue(RS())
    run.core.flush(run.owner)
    next(run.worker.steps())
    run.world.fs.kill(run.proc)
    return run


def check_c02_f1_f2_sync():
    """[F1-F2-sync] F1, restart, F2 differs with and without an actual sync."""
    # (a) F1, then F2 before the restart's sync.
    a = _f1_record_world()
    a.world.fs.power_loss()
    lost = not any(a.inode.D[2 * BLOCK:3 * BLOCK])
    # (b) F1, restart (preservation sync), then F2.
    b = _f1_record_world()
    proc = b.world.fs.register(Proc("restart"))
    rep = startup(b.world, proc, CFG)
    claimed_preserved = rep["preserved"]
    b.world.fs.kill(proc)
    b.world.fs.power_loss()
    kept = any(b.inode.D[2 * BLOCK:3 * BLOCK])
    assert lost, "[F1-F2-sync] F2 before any sync must lose the visible-only record"
    assert not claimed_preserved or kept, \
        "[F1-F2-sync] a report of preserved evidence did not survive F2"
    assert kept, "[F1-F2-sync] the startup preservation sync must make the record durable"
    assert rep["state"] in ("PriorUnresolved", "Ready") and not rep["durability_certified"], \
        "[F1-F2-sync] the restart must classify and must not certify history"
    return "without a sync the record is lost; after the startup sync it survives F2"


def check_c03_errseq_reopen():
    """[errseq-reopen] A sync failure is not repaired by reopening."""
    run = Run("T1")
    run.core.issue(RS())
    run.core.flush(run.owner)
    steps = run.worker.steps()
    next(steps)
    fs = run.world.fs
    fs.wb_fail.add((run.inode.ino, 2))
    for _ in steps:
        pass
    fatal = run.exchange.fatal
    assert fatal is not None and fatal[0][0] == "Sync", "[errseq-reopen] the writer must observe EIO"
    fs.wb_fail.clear()
    fs.kill(run.proc)
    proc = fs.register(Proc("restart"))
    probe = fs.open(proc, "main", run.world.dir("journals"), "j00000.journal")
    result = fs.fdatasync(probe)
    fs.close(probe)
    visible = any(run.inode.K[2 * BLOCK:3 * BLOCK])
    durable = any(run.inode.D[2 * BLOCK:3 * BLOCK])
    assert result == 0 and visible and not durable, \
        "[errseq-reopen] expected: new-description sync returns 0 while D lacks the record"
    rep = startup(run.world, proc, CFG)
    if rep["durability_certified"]:
        for record_seq in range(1, 2):
            assert any(run.inode.D[(record_seq + 1) * BLOCK:(record_seq + 2) * BLOCK]), \
                "[errseq-reopen] the report certified a record that is not durable"
    assert not rep["durability_certified"], "[errseq-reopen] durability certified by a new description"
    fs.kill(proc)
    assert fs.evict_inode(run.inode), "[errseq-reopen] inode eviction expected with no open description"
    assert not any(run.inode.K[2 * BLOCK:3 * BLOCK]), "[errseq-reopen] eviction drops the record"
    # The writer dies before observing the error: a new description still sees it.
    run2 = Run("T1")
    run2.core.issue(RS())
    run2.core.flush(run2.owner)
    next(run2.worker.steps())
    fs2 = run2.world.fs
    fs2.writeback(run2.inode, 2, ok=False)
    fs2.kill(run2.proc)
    proc2 = fs2.register(Proc("restart"))
    rep2 = startup(run2.world, proc2, CFG)
    assert rep2["state"] == "Unreliable", "[errseq-reopen] an unseen error must refuse the restart"
    return "new-description sync 0 is not a certificate; an unseen error refuses"


class OldLayout:
    """First candidate's geometry (negative control NC04): 128-byte slots in shared
    pages from 4096; header and seal (offset 2048) share block 0."""

    @staticmethod
    def record_range(seq):
        start = 4096 + (seq - 1) * 128
        return start, start + 128

    @staticmethod
    def seal_range():
        return 2048, 2048 + 118


class NewLayout:
    @staticmethod
    def record_range(seq):
        start = BLOCK * (seq + 1)
        return start, start + BLOCK

    @staticmethod
    def seal_range():
        return BLOCK, 2 * BLOCK


def _containment_case(layout, durable_records, write_range):
    """Durable bytes for records 1..n, one write in flight; resolve power loss
    within the 4096-byte unit and return the bytes changed outside the write."""
    size = 18 * BLOCK
    durable = bytearray(size)
    durable[0:200] = bytes(range(200))           # a header
    for seq in range(1, durable_records + 1):
        lo, hi = layout.record_range(seq)
        durable[lo:lo + 100] = sha(u64(seq)) * 3 + bytes(4)
    lo, hi = write_range
    visible = bytearray(durable)
    visible[lo:lo + 100] = sha(b"in flight") * 3 + bytes(4)
    first = lo // BLOCK
    last = (hi - 1) // BLOCK
    changed_outside = []
    for outcome in TEAR_OUTCOMES:
        after = bytearray(durable)
        for block in range(first, last + 1):
            b_lo, b_hi = block * BLOCK, (block + 1) * BLOCK
            new = bytes(visible[b_lo:b_hi])
            if outcome == "new":
                after[b_lo:b_hi] = new
            elif outcome == "garbage":
                mixed = bytearray(new)
                for i in range(0, len(mixed), 97):
                    mixed[i] ^= 0x5A
                after[b_lo:b_hi] = mixed
            elif outcome != "old":
                after[b_lo:b_lo + outcome[1]] = new[:outcome[1]]
        for i in range(size):
            if not lo <= i < hi and after[i] != durable[i]:
                changed_outside.append((outcome, i))
                break
    return changed_outside


def check_c04_tear_containment():
    """[tear-containment] Shared-region tear hazards and the supported profile."""
    layout = OldLayout if MUT.on("NC04") else NewLayout
    for n in range(1, 6):
        changed = _containment_case(layout, n, layout.record_range(n + 1))
        assert not changed, f"[tear-containment] writing record {n + 1} altered durable data: {changed[0]}"
    changed = _containment_case(layout, 5, layout.seal_range())
    assert not changed, f"[tear-containment] writing the seal altered durable data: {changed[0]}"
    # Classification of every tear of the block in flight (supported profile).
    run = Run("T1")
    run.play()
    outcomes = set()
    for snap in run.snapshots:
        if not snap["pending"]:
            continue
        for label, image in restart_images(snap).items():
            rep = classify_cached(image, ROOT_ID, 16, 0)
            outcomes.add(rep["class"])
            if rep["class"] not in MALFORMED and rep["class"] != "Unused":
                visible = [x.kind for x in rep["prefix"]]
                issued = [x.kind for x in run.core.intents]
                assert visible == issued[:len(visible)], \
                    "[tear-containment] a torn block produced a record that was never issued"
    assert "MalformedJournal" in outcomes, "[tear-containment] torn blocks must classify as Malformed"
    # Unsupported profile (A-S2 violated: a 16 KiB unit): show the assumption is
    # load-bearing by finding a silent loss of a durable start record.
    rec = records_of([RS(), CS(), AS()])
    hdr = header_block(ROOT_ID, G_RUN, 1, 0, 16, 16)
    image = bytearray(18 * BLOCK)
    image[0:BLOCK] = hdr
    for x in rec:
        frame = encode_record(x.gen, x.seq, x.at, x.kind)
        image[(x.seq + 1) * BLOCK:(x.seq + 2) * BLOCK] = record_block(frame)
    before = classify(bytes(image), ROOT_ID, 16, 0)
    damaged = bytearray(image)
    damaged[4 * BLOCK:5 * BLOCK] = bytes(BLOCK)   # the 16 KiB unit tore record 3 back to zero
    after = classify(bytes(damaged), ROOT_ID, 16, 0)
    assert before["class"] == "UnsealedAction" and after["class"] == "UnsealedNoAction", \
        "[tear-containment] expected the unsupported profile to hide a start record"
    ASSUMPTION_EVIDENCE.append(
        "A-S2 is load-bearing: with a 16 KiB failure unit, losing a durable ActionStarted block "
        "while the next block stays zero yields a valid shorter prefix (UnsealedAction -> "
        "UnsealedNoAction). Outside every guarantee; attested by the Owner, qualified empirically.")
    return f"containment holds for records and seal; tear classes {sorted(outcomes)}"


ASSUMPTION_EVIDENCE = []


def _owner_world():
    """An owner process that claimed a journal and holds its StoreGuard."""
    world = World(n=2, c_pool=16)
    owner = world.fs.register(Proc("owner"))
    rep = startup(world, owner, CFG, claim=True)
    assert rep["state"] == "Ready"
    exchange = Exchange(rep["claim"].generation, CFG.record_capacity)
    worker = Worker(world, rep["claim"].io_fd, exchange, rep["claim"].index)
    for _ in worker.claim(rep["claim"].header):
        pass
    return world, owner, rep, worker


def check_c05_lock_retained():
    """[lock-retained] Worker failure keeps exclusion while custody is unresolved."""
    world, owner, rep, worker = _owner_world()
    fs = world.fs
    if MUT.on("NC05b"):
        # Wrong: the store lock description is duplicated into the worker, which unlocks.
        dup = fs.dup(rep["guard"][0], owner, "worker")
        fs.flock(dup, "UN")
    fs.flock(rep["claim"].io_fd, "UN")  # a stray unlock on the worker's own description
    worker.abort()                      # recorder panic: its descriptors close
    rival = fs.register(Proc("rival"))
    probe = fs.open(rival, "main", world.store_root(), "LOCK")
    store_lock_free = fs.flock(probe, "EX")
    fs.close(probe)
    jprobe = fs.open(rival, "main", world.dir("journals"), f"j{rep['claim'].index:05d}.journal")
    journal_lock_free = fs.flock(jprobe, "EX")
    fs.close(jprobe)
    assert not store_lock_free, "[lock-retained] the store lock was released by recorder failure"
    assert not journal_lock_free, "[lock-retained] the journal lock was released by recorder failure"
    second = startup(world, rival, CFG, claim=True)
    assert second["state"] == "Busy", f"[lock-retained] a second owner got {second['state']}"
    assert worker.ex.fatal is not None and worker.ex.fatal[0][0] == "WorkerLost", \
        "[lock-retained] recorder loss must be latched"
    # Descriptors that survive the scan-to-claim handoff (directories are not modelled).
    w2, owner2, rep2, _ = _owner_world()
    kept = sorted((w2.fs._name_of(fd.ofd.inode), fd.owner, fd.ofd.writable) for fd in owner2.fds)
    name = f"j{rep2['claim'].index:05d}.journal"
    lock_owner = "worker" if MUT.on("NC05a") else "main"
    assert kept == sorted([("LOCK", lock_owner, False), (name, lock_owner, False), (name, "worker", True)]), \
        f"[lock-retained] descriptors after the claim handoff: {kept}"
    # Lifecycle: normal closure, failed seal, failed claim.
    outcomes = {}
    for case in ("closed", "seal failed", "claim failed"):
        run = None
        if case == "claim failed":
            world = World(n=2, c_pool=16)
            proc = world.fs.register(Proc("owner"))
            rep3 = startup(world, proc, CFG, claim=True, generation=G_RUN)
            ex = Exchange(G_RUN, CFG.record_capacity)
            wk = Worker(world, rep3["claim"].io_fd, ex, rep3["claim"].index)
            world.fs.wb_fail.add((rep3["claim"].io_fd.ofd.inode.ino, 0))
            for _ in wk.claim(rep3["claim"].header):
                pass
            assert ex.claim == "Failed", "[lock-retained] the claim should have failed"
        else:
            run = Run("T1").play()
            world, proc = run.world, run.proc
            if case == "seal failed":
                world.fs.wb_fail.add((run.inode.ino, 1))
            run.seal()
        rival = world.fs.register(Proc("rival"))
        busy = startup(world, rival, CFG)["state"]
        assert busy == "Busy", f"[lock-retained] {case}: a rival got {busy} while the owner lives"
        world.fs.kill(proc)
        visible = startup(world, rival, CFG)["state"]
        world.fs.kill(rival)
        pool = world.pool_inode(0)
        assert world.fs.evict_inode(pool), "[lock-retained] inode eviction expected after exit"
        outcomes[case] = (visible, startup(world, rival, CFG)["state"])
        world.fs.kill(rival)
    # A seal whose write failed stays visible (and true: close() returned Ok) until
    # eviction or power loss; then the generation is unsealed and refused.
    assert outcomes == {"closed": ("Ready", "Ready"), "seal failed": ("Ready", "PriorUnresolved"),
                        "claim failed": ("Ready", "Ready")}, \
        f"[lock-retained] lifecycle outcomes after exit, then after eviction: {outcomes}"
    return "locks outlive recorder failure; 3 descriptors survive the claim; after exit/eviction: " + \
        ", ".join(f"{k} {v[0]}/{v[1]}" for k, v in outcomes.items())


def check_c06_busy_before_sync():
    """[busy-before-sync] A competing writer is refused before any journal sync or read."""
    world, owner, rep, worker = _owner_world()
    fs = world.fs
    rival = fs.register(Proc("rival"))
    fs.log.clear()
    second = startup(world, rival, CFG, claim=True)
    touched = [e for e in fs.log if e[0] == "rival" and e[2].endswith(".journal")]
    assert second["state"] == "Busy", f"[busy-before-sync] expected Busy, got {second['state']}"
    assert not touched, f"[busy-before-sync] journal touched before the lock: {touched[:3]}"
    # Defense in depth: store lock free but the journal lock held -> Live, untouched.
    fs.close(rep["guard"][0])
    fs.log.clear()
    third = startup(world, rival, CFG)
    live = f"j{rep['claim'].index:05d}.journal"
    synced = [e for e in fs.log if e[0] == "rival" and e[1] in ("fdatasync", "pread") and e[2] == live]
    assert third["state"] == "Live", f"[busy-before-sync] expected Live, got {third['state']}"
    assert not synced, "[busy-before-sync] the live journal was synced or read"
    return "Busy before any journal operation; a held journal lock refuses as Live untouched"


def check_c07_dup_bounded():
    """[dup-bounded] Duplicate delivery cannot block the owner or grow state."""
    exchange_cls = LegacyQueueExchange if MUT.on("NC07") else Exchange
    run = Run("T1", exchange_cls=exchange_cls)
    footprint0 = run.exchange.footprint()
    for kind in trace_kinds("T1"):
        run.core.issue(kind)
        run.core.flush(run.owner, duplicates=200)
        # A sink panic after storing: the record stays unsent and is resubmitted.
        run.core.unsent.append(run.core.intents[-1])
        run.core.flush(run.owner, panic_after_store=True)
        run.core.unsent.clear()
        assert run.exchange.waits == 0, "[dup-bounded] the owner would block on submission"
        assert run.exchange.footprint() == footprint0, "[dup-bounded] exchange state grew"
        run.drain()
    assert run.exchange.waits == 0, "[dup-bounded] the owner would block on submission"
    assert all(v == 1 for v in run.owner.ack_calls.values()), "[dup-bounded] a record was acknowledged twice"
    assert "Duplicate" not in run.owner.outcomes, "[dup-bounded] a duplicate acknowledgement reached the core"
    assert run.core.ledger.next_ack == len(run.core.intents) + 1, "[dup-bounded] records not acknowledged"
    return f"{len(run.core.intents)} records x 201 submissions: one acknowledgement each, no growth"


def check_c08_fatal_latched():
    """[fatal-latched] A stalled or full consumer cannot hide a fatal recorder condition."""
    exchange_cls = LegacyQueueExchange if MUT.on("NC08") else Exchange
    run = Run("T1", exchange_cls=exchange_cls)
    kinds = [RS(), CS(), AS(), ASET(), CE(), RE("Passed")]
    # The owner stalls (applies nothing) while the worker records and re-records.
    for i, kind in enumerate(kinds):
        record = run.core.issue(kind)
        if i == len(kinds) - 1:
            run.world.fs.wb_fail.add((run.inode.ino, record.seq + 1))
        run.core.flush(run.owner, duplicates=4)
        for _ in run.worker.steps():
            pass
    first_bad = len(kinds)
    run.owner.apply()
    assert run.owner.applied <= run.owner.delivered <= run.exchange.durable_through, \
        "[fatal-latched] applied <= delivered <= durable must hold"
    assert run.core.ledger.failed == first_bad, \
        f"[fatal-latched] the core never learned of the failure at record {first_bad}"
    assert run.core.ledger.next_ack == first_bad, "[fatal-latched] acknowledgements before the failure missing"
    # Recorder loss: the worker dies after writing, before syncing.
    loss = Run("T1", exchange_cls=exchange_cls)
    loss.core.issue(RS())
    loss.core.flush(loss.owner)
    steps = loss.worker.steps()
    next(steps)
    loss.worker.abort()
    loss.owner.apply()
    assert loss.core.ledger.failed == 1, "[fatal-latched] recorder loss was not reported"
    # An abort that skips the drop guard: the owner's join-handle check latches loss.
    gone = Run("T1", exchange_cls=exchange_cls)
    gone.core.issue(RS())
    gone.core.flush(gone.owner)
    next(gone.worker.steps())
    gone.worker.abort(drop_guard=False)
    gone.owner.apply()          # every apply step includes the join-handle check
    fatal = gone.exchange.owner_view()["fatal"]
    assert fatal is not None and fatal[0][0] == "WorkerVanished", \
        "[fatal-latched] the join-handle check did not latch loss"
    assert gone.core.ledger.failed == 1, "[fatal-latched] the join-handle loss was not delivered"
    return "failure and loss (drop guard or join handle) reach the core after a stall; no ack at or beyond"


def check_c09_late_uncertain():
    """[late-uncertain] A non-durable late incident yields uncertainty and refusal."""
    run = Run("T7").play()
    t8 = [s for s in run.snapshots if s["label"] == "native+i1"]
    assert t8, "[late-uncertain] the fixture lacks the late-owner point"
    checked = 0
    for label, image in restart_images(t8[0]).items():
        rep = classify_cached(image, ROOT_ID, 16, 0)
        assert rep["class"] == "UnsealedAction", f"[late-uncertain] {label}: {rep['class']}"
        assert not rep["unsettled"], "[late-uncertain] recorded unsettled must be exact (empty)"
        assert rep["native_work_possible"], "[late-uncertain] native work must be possible"
        assert all(x.kind[0] != "IncidentOpened" for x in rep["prefix"]), \
            "[late-uncertain] the report named an incident that no visible record holds"
        assert rep["notes"], "[late-uncertain] the uncertainty notes are missing"
        assert rep["refused"], f"[late-uncertain] {label}: restart permitted with a late owner held"
        checked += 1
    # T12: recording fails at CaseEnded; recorded unsettled 0, still refused.
    t12 = Run("T1", fail_sync_at=5).play()
    for label, image in restart_images(t12.snapshots[-1]).items():
        rep = classify_cached(image, ROOT_ID, 16, 0)
        assert rep["refused"], f"[late-uncertain] T12 {label}: refusal lost"
        checked += 1
    return f"{checked} restart images refused, no identity invented"


def _header_variants():
    applied = sorted([(sha(b"x"), 1), (sha(b"y"), 5)])
    base = header_block(ROOT_ID, G_RUN, 7, 0, 16, 16, applied)
    out = []
    for offset in (60, 61, 62, 63, 89, 90, 91):
        b = bytearray(base)
        b[offset] = 1
        out.append((f"reserved byte {offset}", redigest_header(b)))
    for label, offset, value in (("claim 0", 40, u64(0)), ("claim 2^63+1", 40, u64(MAX_CLAIM + 1)),
                                 ("capacity 0", 56, u32(0)), ("capacity > C_pool", 56, u32(17)),
                                 ("C_pool mismatch", 52, u32(15)), ("pool index", 48, u32(1)),
                                 ("generation zero", 24, bytes(16)), ("root id", 8, bytes(16)),
                                 ("block size", 7, u8(9)), ("version", 4, u8(2))):
        b = bytearray(base)
        b[offset:offset + len(value)] = value
        out.append((label, redigest_header(b)))
    b = bytearray(base)
    b[92:92 + 33], b[125:125 + 33] = base[125:158], base[92:125]
    out.append(("bindings out of order", redigest_header(b)))
    b = bytearray(base)
    b[125:157] = base[92:124]
    out.append(("duplicate binding", redigest_header(b)))
    b = bytearray(base)
    b[124] = 0
    out.append(("reason tag 0", redigest_header(b)))
    b = bytearray(base)
    b[124] = 6
    out.append(("reason tag 6", redigest_header(b)))
    return base, out


def check_c10_exact_bytes():
    """[exact-bytes] Exact header, seal and record-block consumption."""
    base, variants = _header_variants()
    assert parse_header(base, ROOT_ID, 16, 0)[0] == "valid", "[exact-bytes] the base header must be valid"
    for pos in range(BLOCK):
        b = bytearray(base)
        b[pos] ^= 0x01
        status = parse_header(bytes(b), ROOT_ID, 16, 0)[0]
        assert status != "valid", f"[exact-bytes] header byte {pos} unchecked"
    for label, block in variants:
        status = parse_header(block, ROOT_ID, 16, 0)[0]
        assert status == "invalid", f"[exact-bytes] header {label}: {status}"
        file = block + bytes(17 * BLOCK)
        cls = classify(file, ROOT_ID, 16, 0)["class"]
        assert cls == "MalformedPoolFile", f"[exact-bytes] header {label} classified {cls}"
    n33 = bytearray(base)
    n33[88] = 33
    assert parse_header(bytes(n33), ROOT_ID, 16, 0)[0] == "checksum-invalid", "[exact-bytes] n > 32"
    # Seal block.
    recs = records_of([RS(), CS(), AS(), ASET(), CE(), RE("Passed")])
    header = parse_header(header_block(ROOT_ID, G_RUN, 1, 0, 16, 16), ROOT_ID, 16, 0)[1]
    _, summary = check_grammar(recs, G_RUN, 0)
    seal = seal_for(header, recs)
    assert parse_seal(seal, header, recs, summary) == "sealed", "[exact-bytes] base seal"
    for pos in range(BLOCK):
        b = bytearray(seal)
        b[pos] ^= 0x01
        assert parse_seal(bytes(b), header, recs, summary) != "sealed", f"[exact-bytes] seal byte {pos} unchecked"
    for offset in (5, 6, 7, 126, 127):
        b = bytearray(seal)
        b[offset] = 1
        assert parse_seal(redigest_seal(b), header, recs, summary) == "malformed", \
            f"[exact-bytes] seal reserved byte {offset} accepted"
    for offset in (160, 2000, 4095):
        b = bytearray(seal)
        b[offset] = 1
        assert parse_seal(bytes(b), header, recs, summary) == "malformed", \
            f"[exact-bytes] seal tail byte {offset} accepted"
    # Record blocks.
    frame = encode_record(G_RUN, 1, 0, RS())
    block = record_block(frame)
    assert parse_record_block(block)[0] == "valid", "[exact-bytes] base record block"
    for pos in range(BLOCK):
        b = bytearray(block)
        b[pos] ^= 0x01
        assert parse_record_block(bytes(b))[0] != "valid", f"[exact-bytes] record byte {pos} unchecked"
    for length in (32, 84):
        b = bytearray(block)
        b[6:8] = u16(length)
        assert parse_record_block(bytes(b))[0] == "invalid", f"[exact-bytes] payload length {length}"
    # A checksum-valid but invalid header is never an abandoned claim.
    capacity0 = bytearray(base)
    capacity0[56:60] = u32(0)
    file = redigest_header(capacity0) + bytes(17 * BLOCK)
    cls = classify(file, ROOT_ID, 16, 0)["class"]
    assert cls == "MalformedPoolFile", f"[exact-bytes] checksum-valid invalid header became {cls}"
    torn = bytearray(base)
    torn[100] ^= 0xFF
    assert classify(bytes(torn) + bytes(17 * BLOCK), ROOT_ID, 16, 0)["class"] == "AbandonedClaim", \
        "[exact-bytes] a torn header with nothing else written is an abandoned claim"
    # Text grammars: canonical forms only.
    binding = bind_gap(ROOT_ID, 3)
    good = {"root": ROOT_ID, "binding": binding, "kind": "claim-gap", "claim": 3, "generation": None,
            "pool_index": None, "content": None, "class": "malformed", "unsettled": 0,
            "reason": "other", "statement": "claim 3 was never durable", "operator": "owner",
            "at": "2026-10-01T12:00:00Z"}
    text = disposition_text(good).decode()
    assert parse_disposition(text.encode())["claim"] == 3, "[exact-bytes] base disposition"
    bad_texts = {
        "leading zero": text.replace("claim=3", "claim=03"),
        "uppercase hex": text.replace(binding.hex(), binding.hex().upper()),
        "CRLF": text.replace("\n", "\r\n"),
        "no final newline": text[:-1],
        "trailing line": text + "extra=1\n",
        "leading space": text.replace("statement=claim", "statement= claim"),
        "pool-index for a gap": text.replace("pool-index=none", "pool-index=1"),
        "binding mismatch": text.replace("claim=3", "claim=4"),
        "bad timestamp": text.replace("2026-10-01T12:00:00Z", "2026-02-30T12:00:00Z"),
        "fields reordered": text.replace("reason=other\nstatement=claim 3 was never durable",
                                         "statement=claim 3 was never durable\nreason=other"),
    }
    for label, variant in bad_texts.items():
        try:
            parse_disposition(variant.encode())
        except ParseError:
            continue
        raise AssertionError(f"[exact-bytes] disposition text accepted with {label}")
    world = World(n=2, c_pool=16)
    prov = bytes(world.fs.inodes[world.provdir.ents_K["PROVISION"]].K).decode()
    bad_prov = {
        "digest": prov[:-3] + ("0" if prov[-3] != "0" else "1") + prov[-2:],
        "pool count": prov.replace("pool=2\n", "pool=3\n"),
        "device block size": prov.replace("device-logical-block-size=512", "device-logical-block-size=3000"),
        "predecessor statement": prov.replace("predecessor-statement=none", "predecessor-statement=x"),
        "leading zero": prov.replace("pool-capacity=16", "pool-capacity=016"),
        "revision zero": prov.replace("revision=1\n", "revision=0\n"),
        "revision missing": prov.replace("revision=1\n", ""),
    }
    for label, variant in bad_prov.items():
        try:
            parse_provision(variant.encode())
        except ParseError:
            continue
        raise AssertionError(f"[exact-bytes] PROVISION accepted with {label}")
    return f"every byte of header, seal and record block checked; {len(variants)} checksum-valid " \
        f"header variants and {len(bad_texts) + len(bad_prov)} non-canonical texts refused"


def _unresolved_world(n=3):
    """A store whose pool file 0 holds T6 (unresolved, claim 1)."""
    world = World(n=n, c_pool=16)
    recs = records_of(trace_kinds("T6"))
    data = bytearray((16 + 2) * BLOCK)
    data[0:BLOCK] = header_block(ROOT_ID, G_RUN, 1, 0, 16, 16)
    for x in recs:
        data[(x.seq + 1) * BLOCK:(x.seq + 2) * BLOCK] = record_block(
            encode_record(x.gen, x.seq, x.at, x.kind))
    world.write_pool(0, bytes(data))
    return world


def _verify(world, name="verifier"):
    proc = world.fs.register(Proc(name))
    rep = startup(world, proc, CFG, sync=False, verifier=True)
    world.fs.kill(proc)
    return rep


def _incident_fields(rep, binding, reason="owner-destroyed"):
    inc = rep["incidents"][binding]
    return {"root": ROOT_ID, "binding": binding, "kind": inc["kind"], "claim": inc["claim"],
            "generation": inc["generation"], "pool_index": inc["pool_index"],
            "content": inc["content"], "class": inc["class"], "unsettled": inc["unsettled"],
            "reason": reason, "statement": "owner destroyed the process tree",
            "operator": "owner", "at": "2026-10-01T12:00:00Z"}


def in_session(world, steps_fn, cfg=None):
    """A procedure as section 13 specifies it: begin a maintenance session,
    verify before, the steps, verify after, end. Returns (before, after)."""
    session = MaintenanceSession(world, world.admin)
    assert session.begin() == "Active", "maintenance session did not begin"
    before = session.verify(cfg)
    steps = steps_fn(world, session)
    if isinstance(steps, tuple):
        steps = steps[0]
    after = None
    if steps is not None:
        run_steps(world, steps)
        after = session.verify(cfg)
    session.end()
    return before, after


def check_c11_archive_binding():
    """[archive-binding] Archival keeps bindings; byte changes change them explicitly."""
    content = sha(b"journal bytes")
    places = [("pool", 0), ("pool", 7), "archive", "disposition"]
    values = {bind_journal(ROOT_ID, 1, G_RUN, content, "unresolved", location=p) for p in places}
    assert len(values) == 1, "[archive-binding] the binding depends on the file location"
    world = _unresolved_world()
    rep = _verify(world)
    assert rep["state"] == "PriorUnresolved" and len(rep["blocking"]) == 1, \
        f"[archive-binding] expected one blocking incident, got {rep['state']}"
    binding = next(iter(rep["blocking"]))
    in_session(world, lambda w, s: disposition_steps(w, _incident_fields(rep, binding)))
    rep = _verify(world)
    assert binding in rep["dispositioned"], "[archive-binding] disposition not applied"
    holder = {}

    def archive(w, s):
        steps, holder["final"] = archive_steps(w, 0, s.verify())
        return steps

    in_session(world, archive)
    rep2 = _verify(world)
    assert rep2["state"] == "Ready", f"[archive-binding] after archival: {rep2['state']} {rep2['reason']}"
    parsed = parse_archive_name(holder["final"])
    archived = bind_journal(ROOT_ID, parsed[1], parsed[2], parsed[3], "unresolved", location="archive")
    assert archived == binding, "[archive-binding] archival changed the binding"
    assert binding in rep2["history"] and 0 in rep2["pending_recycle"], \
        "[archive-binding] the archived journal must be history pending recycling"
    data = bytes(world.pool_inode(0).K)
    assert classify(data, ROOT_ID, 16, 0) == classify(data, ROOT_ID, 16, 0), "[archive-binding] determinism"
    w2 = _unresolved_world()
    rep = _verify(w2)
    old = next(iter(rep["blocking"]))
    in_session(w2, lambda w, s: disposition_steps(w, _incident_fields(rep, old)))
    inode = w2.pool_inode(0)
    changed = bytearray(inode.K)
    changed[64:72] = u64(123456)                  # header 'created at': data only
    changed[0:BLOCK] = redigest_header(changed[0:BLOCK])
    w2.write_pool(0, bytes(changed))
    rep = _verify(w2)
    assert rep["state"] == "PriorUnresolved", \
        f"[archive-binding] a changed journal was covered by the old disposition ({rep['state']})"
    assert old not in rep["incidents"] and old in rep["dispositions"], \
        "[archive-binding] the old disposition must be visible as stale"
    return "binding preserved across archival; a changed byte made the disposition stale"


# -- the administrative crash matrix (design section 15.2) ------------------


def _steps_for(world, factory, use_session):
    session = None
    if use_session:
        session = MaintenanceSession(world, world.admin, requalify=use_session == "requalify")
        assert session.begin() == "Active", "maintenance session did not begin"
    steps = factory(world, session)
    return steps[0] if isinstance(steps, tuple) else steps


def crash_series(world, factory, check, label, use_session=True):
    """Every crash point of a procedure. F1: the session's process dies and a
    restart reads the visible state. F2: power loss under every permitted
    ordered schedule and every per-directory over-approximation schedule, with
    pending administrative data old or new. Returns (count, cells)."""
    probe = _clone_world(world)
    total = len(_steps_for(probe, factory, use_session))
    cells = []
    count = 0
    for crash_after in range(total + 1):
        base = _clone_world(world)
        steps = _steps_for(base, factory, use_session)
        for step in steps[:crash_after]:
            base.fs.step = step.doc
            step()
        base.fs.step = None
        base.fs.kill(base.admin)
        cell = {"F1": set(), "ordered": set(), "perdir": set()}
        w = _clone_world(base)
        after = _verify(w)
        check(w, after, crash_after, "F1")
        cell["F1"].add(label(w, after))
        count += 1
        for family in ("ordered", "perdir"):
            for schedule in base.fs.schedules(family):
                for data in ("old", "new"):
                    w = _clone_world(base)
                    w.fs.power_loss(schedule, data=data)
                    after = _verify(w)
                    check(w, after, crash_after, family)
                    cell[family].add(label(w, after))
                    count += 1
        cells.append(cell)
    return count, cells


def compress(cells):
    """Maximal runs of crash points with identical outcome sets, as rows
    (first, last, F1, ordered, added by the per-directory over-approximation)."""
    rows = []
    for k, cell in enumerate(cells):
        key = (tuple(sorted(cell["F1"])), tuple(sorted(cell["ordered"])),
               tuple(sorted(cell["perdir"] - cell["ordered"])))
        if rows and rows[-1][2:] == key:
            rows[-1] = (rows[-1][0], k) + key
        else:
            rows.append((k, k) + key)
    return rows


def _state_label(after):
    return {"MaintenanceIncomplete": "maintenance", "Invalid": "invalid", "Lost": "lost",
            "Unprovisioned": "unprovisioned", "Unsupported": "unsupported"}.get(after["state"])


def _label_incident(binding):
    def label(w, after):
        fixed = _state_label(after)
        if fixed:
            return fixed
        if binding in after["history"]:
            return "archived" if after["files"][0]["class"] != "Unused" else "recycled"
        if binding in after["dispositioned"]:
            return "published"
        if binding in after["blocking"]:
            return "blocks"
        return after["state"]
    return label


def _label_provision(w, after):
    fixed = _state_label(after)
    if fixed:
        return fixed
    return "fresh" if after["state"] == "Ready" else after["state"]


def _label_retire(w, after):
    fixed = _state_label(after)
    if fixed:
        return fixed
    return f"bound {after['provision']['retired']}"


def _label_selected(w, after):
    fixed = _state_label(after)
    if fixed:
        return fixed
    return "successor" if after["provision"]["root_id"] == SUCCESSOR_ID else "predecessor"


def _label_requalify(w, after):
    fixed = _state_label(after)
    if fixed:
        return fixed
    return f"revision {after['provision']['revision']}"


def admin_scenarios(n=1):
    """The worlds and procedures of the crash matrix (one pool file each; the
    composed checks use two, so that dependent work can follow every procedure)."""
    base = _unresolved_world(n=n)
    before = _verify(base)
    binding = next(iter(before["blocking"]))
    fields = _incident_fields(before, binding)
    disposed = _clone_world(base)
    in_session(disposed, lambda w, s: disposition_steps(w, fields))
    archived = _clone_world(disposed)
    in_session(archived, lambda w, s: archive_steps(w, 0, s.verify()))
    recycled = _clone_world(archived)
    in_session(recycled, lambda w, s: recycle_steps(w, 0, s))
    requal = World(n=n, c_pool=16)
    requal.mount.lines[0] = (31, FIXTURE_DEV, "ext4", "rw,noatime", "rw,errors=remount-ro")
    blank = World(n=n, c_pool=16, provision=False)
    return {
        "binding": binding,
        "P-PROV": (blank, lambda w, s: provision_steps(w, ROOT_ID, "store-a"), _label_provision, False),
        "P-DISP": (base, lambda w, s: disposition_steps(w, fields), _label_incident(binding), True),
        "P-REVOKE": (disposed, lambda w, s: revoke_steps(w, binding), _label_incident(binding), True),
        "P-ARCH": (disposed, lambda w, s: archive_steps(w, 0, s.verify()), _label_incident(binding), True),
        "P-RECYCLE": (archived, lambda w, s: recycle_steps(w, 0, s), _label_incident(binding), True),
        "P-RETIRE": (recycled, lambda w, s: retire_steps(w, 1, s.verify(), s), _label_retire, True),
        "P-REQUALIFY": (requal, lambda w, s: requalify_steps(w, s, "rw,noatime"), _label_requalify,
                        "requalify"),
        "P-SUCCESSOR": (recycled, lambda w, s: successor_steps(w, s, SUCCESSOR_ID, "store-b"),
                        _label_selected, True),
    }


def check_c12_admin_crash():
    """[admin-crash] No crash point installs partial state or retires unresolved history."""
    scenarios = admin_scenarios()
    binding = scenarios["binding"]
    base = scenarios["P-DISP"][0]
    blocking = set(_verify(_clone_world(base))["blocking"])
    fields = _incident_fields(_verify(_clone_world(base)), binding)
    counts = collections.Counter()

    def no_silent_loss(w, after, crash_after, kind):
        if after["state"] in ("Ready", "PriorUnresolved"):
            for b in blocking:
                ok = b in after["blocking"] or b in after["history"] or b in after["dispositioned"]
                assert ok, f"[admin-crash] incident lost silently at step {crash_after} ({kind})"
            d = w.dir("dispositions", state=after["selection"].state)
            for b in after["dispositioned"]:
                assert b.hex() + ".disposition" in d.ents_K, \
                    f"[admin-crash] a disposition took effect without its final name ({kind} {crash_after})"
                inode = w.fs.inodes[d.ents_K[b.hex() + ".disposition"]]
                assert bytes(inode.K) == disposition_text(fields) or b != binding, \
                    "[admin-crash] a partial disposition took effect"
        else:
            assert after["state"] in ("MaintenanceIncomplete", "Invalid", "Lost"), \
                f"[admin-crash] unexpected state {after['state']} ({kind} after {crash_after})"

    def recycle_ok(w, after, crash_after, kind):
        no_silent_loss(w, after, crash_after, kind)
        prov = w.provision_record()
        inode = w.dir("journals").ents_K.get("j00000.journal")
        if inode != prov["pool_inodes"][0]:
            assert after["state"] in ("Lost", "MaintenanceIncomplete"), \
                f"[admin-crash] a replaced pool file was accepted ({kind} after step {crash_after})"
        if after["state"] == "Ready" and inode == prov["pool_inodes"][0]:
            assert binding in after["history"], "[admin-crash] recycling lost the archived history"

    def prov_ok(w, after, crash_after, kind):
        if after["state"] == "Ready":
            for i in range(w.N):
                assert w.pool_inode(i) is not None, "[admin-crash] a provisioned pool file is missing"
        else:
            assert after["state"] == "Unprovisioned", \
                f"[admin-crash] provisioning crash left {after['state']} ({kind} after {crash_after})"

    def retire_ok(w, after, crash_after, kind):
        prov = w.provision_record()
        assert after["state"] == "Ready", \
            f"[admin-crash] retirement crash left {after['state']} ({kind} after {crash_after})"
        assert prov["retired"] in (0, 1), "[admin-crash] retirement installed an unexpected bound"
        assert any(n.startswith("j-1-") for n in w.dir("archive").ents_K), \
            "[admin-crash] retirement removed archived history"
        if prov["retired"] == 0:
            assert binding in after["history"], "[admin-crash] archived history lost before retirement"

    def requal_ok(w, after, crash_after, kind):
        assert after["state"] in ("Ready", "Unsupported"), \
            f"[admin-crash] re-qualification crash left {after['state']} ({kind} after {crash_after})"

    successor_world = scenarios["P-SUCCESSOR"][0]
    old_pool = bytes(successor_world.pool_inode(0).K)

    def successor_ok(w, after, crash_after, kind):
        current = bytes(w.fs.inodes[w.dir("journals", state="store-a").ents_K["j00000.journal"]].K)
        assert current == old_pool, "[admin-crash] succession changed the predecessor's journal"
        assert after["state"] == "Ready", \
            f"[admin-crash] succession crash left {after['state']} ({kind} after {crash_after})"
        if after["provision"]["root_id"] == SUCCESSOR_ID:
            assert "PROVISION.predecessor-" + ROOT_ID.hex() in w.provdir.ents_K, \
                "[admin-crash] successor in force without the predecessor's PROVISION"

    checks = {"P-PROV": prov_ok, "P-DISP": no_silent_loss, "P-REVOKE": no_silent_loss,
              "P-ARCH": no_silent_loss, "P-RECYCLE": recycle_ok, "P-RETIRE": retire_ok,
              "P-REQUALIFY": requal_ok, "P-SUCCESSOR": successor_ok}
    # Recovery (sections 13.1 and 13.8): every MaintenanceIncomplete or Lost
    # outcome of these procedures is recovered inside a session, without force.
    finals = {"P-DISP": {"blocks", "published"}, "P-ARCH": {"published", "archived"},
              "P-RECYCLE": {"archived", "recycled"}}

    def with_recovery(name, check, label):
        def run(w, after, crash_after, kind):
            check(w, after, crash_after, kind)
            if after["state"] not in ("MaintenanceIncomplete", "Lost"):
                return
            r = _clone_world(w)
            session = MaintenanceSession(r, r.admin)
            assert session.begin() == "Active", f"[admin-crash] {name}: no session for recovery"
            run_steps(r, leftover_steps(r, session))
            rep = session.verify()
            if rep["state"] == "Lost":
                steps = resume_recycle_steps(r, 0, rep, session)
                assert steps is not None, \
                    f"[admin-crash] {name}: {rep['reason']} has no resumption ({kind} after {crash_after})"
                run_steps(r, steps)
            session.end()
            outcome = label(r, _verify(r))
            assert outcome in finals[name], \
                f"[admin-crash] {name}: recovery after {crash_after} ({kind}) left {outcome}"
            counts[f"{name} recovered"] += 1
        return run

    matrix = {}
    for name, check in checks.items():
        world, factory, label, use_session = scenarios[name]
        if name in finals:
            check = with_recovery(name, check, label)
        n, cells = crash_series(world, factory, check, label, use_session)
        counts[name] = n
        matrix[name] = compress(cells)
    # Retirement is refused while an unresolved, undispositioned generation exists.
    pending = _unresolved_world(n=1)
    session = MaintenanceSession(pending, pending.admin)
    assert session.begin() == "Active"
    steps = retire_steps(pending, 1, session.verify(), session)
    if steps is not None:
        run_steps(pending, steps)
    session.end()
    after = _verify(pending)
    assert after["state"] == "PriorUnresolved" and binding in after["blocking"], \
        f"[admin-crash] retirement hid an unresolved generation ({after['state']})"
    counts["P-RETIRE refused"] = 1
    MATRIX_RESULT.clear()
    MATRIX_RESULT.update(matrix)
    return "outcomes checked: " + ", ".join(f"{k} {v}" for k, v in sorted(counts.items()))


MATRIX_RESULT = {}


GRAMMAR_MUTATIONS = [
    ("RunEnded before CaseEnded", "T1", lambda k: k[:4] + [k[5], k[4]], "G15"),
    ("ActionStarted removed", "T1", lambda k: k[:2] + k[3:], "G8"),
    ("second RunStarted", "T1", lambda k: k[:1] + [RS()] + k[1:], "G2"),
    ("ShutdownRefused before AdmissionClosed", "T2", lambda k: [k[1], k[0]], "G3"),
    ("CaseStarted before RunStarted", "T1", lambda k: [k[1], k[0]] + k[2:], "G3"),
    ("ActionStarted after AdmissionClosed", "T3", lambda k: k[:2] + [AC("Cancelled", "Requested", 0), AS()] + k[5:], "G6"),
    ("CaseEnded for another case", "T1", lambda k: k[:4] + [CE(2)] + k[5:], "G5"),
    ("RunEnded Pending", "T1", lambda k: k[:5] + [RE("Pending")], "G15"),
    ("Failed verdict without a closure", "T1", lambda k: k[:5] + [RE("Failed")], "G16"),
    ("Passed verdict after a closure", "T3", lambda k: k[:6] + [RE("Passed")], "G16"),
    ("recovery attempt numbering", "T4", lambda k: k[:8] + [RA(2, True), RE("Failed", 2)], "G12"),
    ("recovery attempt without RecoveryRequired", "T4", lambda k: k[:6] + k[7:], "G12"),
    ("attempt after RunEnded without an incident", "T7", lambda k: k[:6] + [RA(1, True)], "G12"),
    ("IncidentSettled NotAdmitted", "T7", lambda k: k[:7] + [IS(1, "NotAdmitted")] + k[8:], "G14"),
    ("IncidentOpened for an unstarted action", "T7", lambda k: k[:6] + [IO(1, 2)] + k[7:], "G13"),
    ("incident numbering", "T7", lambda k: k[:6] + [IO(2, 1)] + k[7:], "G13"),
    ("NotAdmitted after ActionFailed", "T5", lambda k: k[:7] + [ASET(1, "NotAdmitted")] + k[8:], "G8"),
    ("ActionFailed after ActionSettled", "T4", lambda k: k[:3] + [ASET(), AF()] + k[5:], "G7"),
    ("ActionFailed after RunEnded", "T1", lambda k: k + [AF()], "G17"),
    ("second AdmissionClosed", "T2", lambda k: [k[0], AC("Cancelled", "Requested", 0), k[1]], "G9"),
    ("second ShutdownRefused", "T11", lambda k: k[:5] + [SR()] + k[5:], "G10"),
    ("RecoveryRequired with a case open", "T4", lambda k: k[:5] + [RR(), k[5]] + k[7:], "G11"),
    ("dispositioned count differs from the header", "T1", lambda k: [RS(1)] + k[1:], "G2"),
    ("closure counts more admissions than starts", "T3", lambda k: k[:3] + [AC("Cancelled", "Requested", 2)] + k[4:], "G9"),
    ("CaseStarted after RecoveryRequired", "T4", lambda k: k[:7] + [CS(2)] + k[7:], "G4"),
    ("passed case with a failed action", "T15", lambda k: k[:6] + [CE(1, True)] + k[7:], "G5"),
    ("resolved_by missing after recovery", "T4", lambda k: k[:9] + [RE("Failed")], "G15"),
    ("resolved_by without recovery", "T1", lambda k: k[:5] + [RE("Passed", 1)], "G15"),
    ("CaseStarted while a case is open", "T1", lambda k: k[:2] + [CS(2)] + k[2:], "G4"),
    ("action numbering gap", "T1", lambda k: k[:2] + [AS(2)] + k[3:], "G6"),
    ("identity of another generation", "T13", lambda k: k[:5] + [("IncidentOpened", (G_RUN, 1), (bytes(16), 1), "Process")] + k[6:], "G-id"),
]


def check_c13_grammar_conformance():
    """[grammar-conformance] Fixtures, prefixes and single-rule mutations."""
    fixtures = 0
    prefixes = 0
    for name in sorted(TRACES):
        kinds = trace_kinds(name)
        recs = records_of(kinds)
        violation, _ = check_grammar(recs, G_RUN, 0)
        assert violation is None, f"[grammar-conformance] fixture {name} rejected: {violation}"
        fixtures += 1
        for n in range(len(recs) + 1):
            violation, _ = check_grammar(recs[:n], G_RUN, 0)
            assert violation is None, f"[grammar-conformance] prefix {n} of {name} rejected"
            prefixes += 1
    decreasing = records_of(trace_kinds("T1"), ats=[0, 1, 2, 3, 2, 5])
    assert check_grammar(decreasing, G_RUN, 0)[0][0] == "G1", "[grammar-conformance] instants"
    for label, base, mutate, rule in GRAMMAR_MUTATIONS:
        recs = records_of(mutate(trace_kinds(base)))
        violation, _ = check_grammar(recs, G_RUN, 0)
        assert violation is not None and violation[0] == rule, \
            f"[grammar-conformance] mutation '{label}' gave {violation}, expected {rule}"
    total = len(GRAMMAR_MUTATIONS) + 1
    return f"{fixtures} fixtures, {prefixes} prefixes accepted; {total} mutations rejected by their rules"


def check_c14_safe_open():
    """[safe-open] Special files refused unopened; ext4 only through the mount."""
    world = World(n=2, c_pool=16)
    fs = world.fs
    j = world.dir("journals")
    fs.unlink(j, "j00001.journal")
    fifo = fs.create(j, "j00001.journal", "fifo", UID, GID, 0o600)
    proc = fs.register(Proc("starter"))
    fs.log.clear()
    try:
        rep = startup(world, proc, CFG)
    except Blocked:
        raise AssertionError("[safe-open] startup blocked opening a FIFO")
    opened = [e for e in fs.log if e[1] == "open" and e[2] == "j00001.journal"]
    assert rep["state"] == "Invalid" and not opened, \
        f"[safe-open] a FIFO must refuse without being opened ({rep['state']}, {opened})"
    fs.kill(proc)
    fs.unlink(j, "j00001.journal")
    fs.create(j, "j00001.journal", "symlink", UID, GID, 0o777)
    rep = startup(world, proc, CFG)
    assert rep["state"] == "Invalid", "[safe-open] a symbolic link must refuse"
    fs.kill(proc)
    cases = []
    for label, edit in (
            ("ext3 with the shared magic", lambda m: m.lines.__setitem__(0, (31, FIXTURE_DEV, "ext3", "rw,relatime", "rw,errors=remount-ro"))),
            ("ext2 with the shared magic", lambda m: m.lines.__setitem__(0, (31, FIXTURE_DEV, "ext2", "rw,relatime", "rw,errors=remount-ro"))),
            ("two records with the mount ID", lambda m: m.lines.append((31, FIXTURE_DEV, "ext4", "rw,relatime", "rw,errors=remount-ro"))),
            ("no record with the mount ID", lambda m: setattr(m, "mnt_id", 99)),
            ("device mismatch", lambda m: setattr(m, "st_dev", "8:1")),
            ("options changed", lambda m: m.lines.__setitem__(0, (31, FIXTURE_DEV, "ext4", "rw,relatime,nobarrier", "rw,errors=remount-ro"))),
            ("block size", lambda m: setattr(m, "bsize", 1024)),
            ("link protection off", lambda m: setattr(m, "protected", (0, 1)))):
        w = World(n=2, c_pool=16)
        edit(w.mount)
        p = w.fs.register(Proc("starter"))
        rep = startup(w, p, CFG)
        assert rep["state"] == "Unsupported", f"[safe-open] {label}: {rep['state']}"
        cases.append(label)
    return f"FIFO and symlink refused unopened; {len(cases)} mount cases refused"


def check_c15_ack_durable():
    """[ack-durable] INV-1 and INV-4 under write and sync faults."""
    results = []

    def scenario(label, pwrite_plan=(), sync_plan=(), fail_at=None, expect_fail=None):
        state = {}

        def on_ack(seq, outcome):
            if outcome == "Acknowledged":
                lo = (seq + 1) * BLOCK
                inode = state["run"].inode
                assert bytes(inode.D[lo:lo + BLOCK]) == bytes(inode.K[lo:lo + BLOCK]) and any(inode.D[lo:lo + BLOCK]), \
                    f"[ack-durable] {label}: record {seq} acknowledged before it was durable"
                fatal = state["run"].exchange.fatal
                assert fatal is None or seq <= fatal[1], \
                    f"[ack-durable] {label}: acknowledged beyond the durable position at the latch"
        run = Run("T1", on_ack=on_ack, fail_sync_at=fail_at, interleave=True)
        state["run"] = run
        run.world.fs.pwrite_plan = list(pwrite_plan)
        run.world.fs.sync_plan = list(sync_plan)
        run.play()
        if expect_fail is None:
            assert run.core.ledger.next_ack == len(run.core.intents) + 1, f"[ack-durable] {label}: incomplete"
        else:
            assert run.core.ledger.failed == expect_fail, \
                f"[ack-durable] {label}: failure expected at {expect_fail}, got {run.core.ledger.failed}"
            assert run.core.ledger.next_ack <= expect_fail, f"[ack-durable] {label}: INV-4"
        results.append(label)

    scenario("clean")
    scenario("short writes", pwrite_plan=[("short", 1000), ("short", 7), ("ok",)])
    scenario("EINTR twice", pwrite_plan=[("eintr",), ("eintr",), ("ok",)])
    scenario("zero progress four times", pwrite_plan=[("zero",)] * 4, expect_fail=1)
    scenario("EINTR four times", pwrite_plan=[("eintr",)] * 4, expect_fail=1)
    scenario("sync EINTR twice", sync_plan=[("ok",), ("eintr",), ("eintr",), ("ok",)])
    for k in range(1, 7):
        scenario(f"sync EIO at record {k}", fail_at=k, expect_fail=k)
    return f"{len(results)} fault scenarios: every acknowledgement durable; none at or after a failure"


def check_c16_no_false_resolution():
    """[no-false-resolution] INV-3 at every crash point of every fixture."""
    images = 0
    false_positive = 0
    for name in sorted(TRACES):
        variants = [dict(lazy=False), dict(lazy=True)]
        n_records = len(trace_kinds(name))
        variants += [dict(lazy=False, fail_sync_at=k) for k in range(1, n_records + 1)]
        for v in variants:
            run = Run(name, **v).play()
            if not v.get("fail_sync_at") and run.closable():
                run.seal()
            for snap in run.snapshots:
                for label, image in restart_images(snap).items():
                    rep = classify_cached(image, ROOT_ID, 16, run.claim.index)
                    images += 1
                    if snap["native"]:
                        assert rep["refused"], \
                            f"[no-false-resolution] {name} {v} at {snap['label']} ({label}): " \
                            f"restart permitted with {sorted(snap['native'])} outstanding as {rep['class']}"
                    elif rep["refused"]:
                        false_positive += 1
                    if rep["class"] == "Sealed":
                        assert not snap["native"], "[no-false-resolution] sealed while native state outstanding"
    return f"{images} restart images over {len(TRACES)} fixtures; refusals without native work: {false_positive} (disclosed)"


def check_c17_arith_bounds():
    """[arith-bounds] Claim exhaustion and numeric bounds."""
    # Claim exhaustion: a sealed journal holds the largest claim.
    world = World(n=2, c_pool=16)

    def through(record):
        record["retired"] = MAX_CLAIM - 1
    for step in rewrite_provision_steps(world, through):
        step()
    data = bytearray((16 + 2) * BLOCK)
    data[0:BLOCK] = header_block(ROOT_ID, G_RUN, MAX_CLAIM, 0, 16, 16)
    header = parse_header(data[0:BLOCK], ROOT_ID, 16, 0)[1]
    data[BLOCK:2 * BLOCK] = seal_for(header, [])
    world.write_pool(0, bytes(data))
    proc = world.fs.register(Proc("owner"))
    rep = startup(world, proc, CFG, claim=True)
    assert rep["state"] == "ClaimExhausted", f"[arith-bounds] claim after 2^63 gave {rep['state']}"
    # Configuration bounds.
    for cfg in (Config(17, 16), Config(16, 33)):
        p = world.fs.register(Proc("cfg"))
        assert startup(world, p, cfg)["state"] == "Unsupported", f"[arith-bounds] {cfg}"
    # Size bound.
    w = World(n=2, c_pool=16)
    inode = w.pool_inode(1)
    inode.K.extend(bytes(BLOCK))
    inode.D.extend(bytes(BLOCK))
    assert startup(w, w.fs.register(Proc("s")), CFG)["state"] == "Invalid", "[arith-bounds] size"
    # PROVISION bounds.
    for bad in (b"x" * 65537, b"nexus-phase2-custody-provision 1\npool=0\n"):
        try:
            parse_provision(bad)
            raise AssertionError("[arith-bounds] PROVISION bound not enforced")
        except ParseError:
            pass
    # Incident count above the limit refuses without truncation.
    w = World(n=3, c_pool=16)
    for idx in range(3):
        d = bytearray((16 + 2) * BLOCK)
        d[0:8] = b"garbage!"
        d[5 * BLOCK] = 1
        w.write_pool(idx, bytes(d))
    rep = startup(w, w.fs.register(Proc("c")), Config(16, 2))
    assert rep["state"] == "Capacity" and len(rep["incidents"]) == 3, \
        f"[arith-bounds] 3 incidents over a limit of 2 gave {rep['state']}"
    w = World(n=3, c_pool=16)
    for idx, claim in ((0, 1), (1, 5)):
        d = bytearray((16 + 2) * BLOCK)
        d[0:BLOCK] = header_block(ROOT_ID, sha(u64(claim))[:16], claim, idx, 16, 16)
        w.write_pool(idx, bytes(d))
    rep = startup(w, w.fs.register(Proc("c")), Config(16, 2))
    assert rep["state"] == "Capacity", f"[arith-bounds] 3 gaps over a limit of 2 gave {rep['state']}"
    # A huge claim gap is counted, never enumerated.
    w = World(n=2, c_pool=16)
    d = bytearray((16 + 2) * BLOCK)
    d[0:BLOCK] = header_block(ROOT_ID, G_RUN, MAX_CLAIM, 0, 16, 16)
    w.write_pool(0, bytes(d))
    rep = startup(w, w.fs.register(Proc("g")), CFG)
    assert rep["state"] == "Capacity", f"[arith-bounds] 2^63 - 1 gaps gave {rep['state']}"
    # Archive entry count.
    w = World(n=2, c_pool=16)
    a = w.dir("archive")
    for i in range(4097):
        a.ents_K[f"p-00000-{i:064x}.journal"] = 0
    assert startup(w, w.fs.register(Proc("a")), CFG)["state"] == "Invalid", "[arith-bounds] archive count"
    return "claim exhaustion, configuration, size, PROVISION, incident and gap bounds refused"


def check_c18_fatal_total():
    """[fatal-total] Every fatal condition: admission refused at once, true
    acknowledgements kept, failure delivered only at a real record."""
    done = []
    forged = Record(G_RUN, 1, 99, RS(), encode_record(G_RUN, 1, 99, RS())[-32:])

    def admit_attempt(rig, action):
        result = rig.gate.admit(rig.core, action)
        if result == "Admitted":
            rig.core.start_native(action)
        return result

    def nothing_after_latch(rig, kind):
        i = rig.latch_index()
        assert i is not None, f"[fatal-total] {kind}: no fatal state was latched"
        late = [e[0] for e in rig.events[i:] if e[0] in ("admit", "native", "publish")]
        assert not late, f"[fatal-total] {kind}: {late[0]} after the latch"

    def settle_and_deliver(rig, kind, target):
        rig.core.end_case()         # settles the unadmitted reservation: a real record
        rig.pump()
        assert rig.owner.failure_target == target and rig.owner.failure_outcome == "FailureRecorded", \
            f"[fatal-total] {kind}: failure delivered at {rig.owner.failure_target} " \
            f"({rig.owner.failure_outcome}), expected {target}"
        assert rig.core.ledger.failed == target and rig.core.ledger.next_ack == target, \
            f"[fatal-total] {kind}: acknowledgements retracted or added around the failure"
        assert not rig.owner.faults, f"[fatal-total] {kind}: {rig.owner.faults}"
        done.append(kind)

    def loss_scenario(kind, fail):
        rig = Rig()
        action = rig.reserved()
        fail(rig)
        rig.owner.apply()
        assert rig.owner.failure_outcome is None and rig.core.ledger.failed is None, \
            f"[fatal-total] {kind}: a failure was reported without a real record"
        assert rig.core.ledger.next_ack == 4, f"[fatal-total] {kind}: acknowledgements 1-3 must stay"
        assert admit_attempt(rig, action) == "RecorderFatal", \
            f"[fatal-total] {kind}: admission after the fatal condition was not refused"
        assert not rig.core.native, f"[fatal-total] {kind}: native work started after the fatal condition"
        nothing_after_latch(rig, kind)
        settle_and_deliver(rig, kind, 4)
        assert rig.core.intents[3].kind == ASET(action, "NotAdmitted"), \
            f"[fatal-total] {kind}: the reservation was not settled as never admitted"
        return rig

    # A and D: issued = durable = applied = 3, the start record acknowledged and
    # its reservation not admitted; the worker dies; no fourth record exists.
    loss_scenario("A/D worker loss", lambda rig: rig.worker.abort())
    loss_scenario("worker vanished", lambda rig: rig.worker.abort(drop_guard=False))
    # B: a conflicting duplicate of acknowledged sequence 1.
    rig = loss_scenario("B conflict on an acknowledged record", lambda rig: rig.owner.submit(forged))
    assert rig.exchange.fatal == (("Conflict", 1), 3), f"[fatal-total] B: latched {rig.exchange.fatal}"
    # C: invalid submissions.
    for label, intent in (
            ("C zero", Record(G_RUN, 0, 5, RS(), bytes(32))),
            ("C out of range", Record(G_RUN, 17, 5, RS(), bytes(32))),
            ("C foreign", Record(bytes(16), 4, 5, RS(), bytes(32))),
            ("C future", Record(G_RUN, 7, 7, ASET(), encode_record(G_RUN, 7, 7, ASET())[-32:]))):
        rig = loss_scenario(label, lambda rig, intent=intent: rig.owner.submit(intent))
        fatal = rig.exchange.fatal
        assert fatal is not None and fatal[0][0] == "InvalidSubmission", f"[fatal-total] {label}: {fatal}"
    rig = loss_scenario("poisoned exchange", lambda rig: setattr(rig.exchange, "poisoned", True))
    # C: a conflict beyond the next unapplied record (records 4 and 5 submitted, unwritten).
    rig = Rig()
    action = rig.reserved()
    assert admit_attempt(rig, action) == "Admitted"
    rig.core.complete_created(action)
    rig.core.end_case()
    rig.core.flush(rig.owner)
    bad = Record(G_RUN, 5, 50, CE(1, False), encode_record(G_RUN, 5, 50, CE(1, False))[-32:])
    rig.owner.submit(bad)
    for _ in rig.worker.steps():
        pass
    rig.owner.apply()
    assert rig.exchange.fatal == (("Conflict", 5), 3), f"[fatal-total] C beyond: {rig.exchange.fatal}"
    assert rig.owner.failure_target == 4 and rig.core.ledger.failed == 4, \
        "[fatal-total] C beyond: the failure was not delivered at the next unacknowledged record"
    nothing_after_latch(rig, "C beyond")
    done.append("C beyond")
    # E: an append in flight when another fatal condition latches.
    rig = Rig()
    assert rig.do(rig.core.start_run) == "Started"
    rig.core.begin_case()
    rig.core.flush(rig.owner)
    steps = rig.worker.steps()
    assert next(steps) == ("written", 2)
    rig.owner.submit(forged)
    for _ in steps:
        pass
    rig.owner.apply()
    assert rig.exchange.durable_through == 1, "[fatal-total] E: published after the latch"
    assert rig.core.ledger.next_ack == 2 and rig.owner.failure_target == 2 and \
        rig.core.ledger.failed == 2, "[fatal-total] E: the record in flight must be the one failed"
    nothing_after_latch(rig, "E")
    done.append("E")
    # Unexpected acknowledgement outcome: the owner's copy of record 2 differs.
    rig = Rig()
    assert rig.do(rig.core.start_run) == "Started"
    rig.core.begin_case()
    rig.core.flush(rig.owner)
    real = rig.owner.retained[2]
    rig.owner.retained[2] = Record(real.gen, real.seq, real.at, real.kind, bytes(32))
    for _ in rig.worker.steps():
        pass
    rig.owner.apply()
    fatal = rig.exchange.fatal
    assert fatal is not None and fatal[0][0] == "UnexpectedAck", f"[fatal-total] unexpected ack: {fatal}"
    assert rig.owner.ack_calls[2] == 1 and rig.owner.applied == 1 and rig.core.ledger.failed == 2, \
        "[fatal-total] unexpected ack: application must stop and the failure land at that record"
    done.append("unexpected acknowledgement")
    # F: before the claim completes.
    w = World(n=2, c_pool=16)
    p = w.fs.register(Proc("owner"))
    rep = startup(w, p, CFG, claim=True, generation=G_RUN)
    ex = Exchange(G_RUN, 16)
    wk = Worker(w, rep["claim"].io_fd, ex, rep["claim"].index)
    claim = wk.claim(rep["claim"].header)
    next(claim)
    wk.abort()
    assert ex.claim == "Failed", "[fatal-total] F: a claim interrupted by recorder loss must fail"
    assert CoreSim(G_RUN).close() == "Closed", "[fatal-total] F: the untouched custody must close"
    done.append("F before the claim")

    def finalized():
        rig = Rig()
        action = rig.reserved()
        assert admit_attempt(rig, action) == "Admitted"
        rig.core.complete_created(action)
        rig.do(rig.core.end_case)
        rig.do(rig.core.finish_run)
        assert rig.core.phase == "Finalized", f"[fatal-total] fixture: {rig.core.phase}"
        return rig, action

    # F: after the terminal commitment: a late owner is held; nothing is admitted.
    rig, action = finalized()
    verdict = rig.core.intents[rig.core.terminal - 1].kind
    rig.worker.abort()
    rig.owner.apply()
    rig.core.late_owner(action)
    rig.pump()
    assert rig.owner.failure_outcome == "FailureRecorded" and \
        rig.core.intents[rig.core.terminal - 1].kind == verdict, \
        "[fatal-total] F: after the commitment the failure lands at the late incident; the verdict stays"
    assert rig.core.close() == "Refused" and rig.core.held, \
        "[fatal-total] F: the late owner must stay held and closure refused"
    nothing_after_latch(rig, "F after the commitment")
    done.append("F after the commitment")
    # F: during the seal.
    rig, _ = finalized()
    assert rig.core.close() == "Closed"
    seal = rig.worker.seal(seal_for(rig.header, rig.core.intents))
    next(seal)
    rig.worker.abort()
    assert rig.exchange.seal == "Failed", "[fatal-total] F: a seal interrupted by loss must fail"
    done.append("F during the seal")
    # No native ownership left: the custody closes, the seal is withheld, restart refuses.
    rig, _ = finalized()
    rig.worker.abort()
    rig.owner.apply()
    assert rig.owner.failure_outcome is None, "[fatal-total] nothing issued: nothing may be reported"
    assert rig.core.close() == "Closed", "[fatal-total] with nothing held and everything acknowledged it closes"
    assert not rig.exchange.healthy(), "[fatal-total] the store outcome must stay fatal"
    report = classify(bytes(rig.inode.K), ROOT_ID, 16, rig.claim.index)
    assert report["class"] == "UnsealedAction" and report["refused"], \
        "[fatal-total] an unsealed generation with an action start must be refused"
    done.append("no native ownership left")
    return f"{len(done)} fatal scenarios: admission refused at once, acknowledgements kept, " \
           "failures only at issued records"


def check_c19_admission_fence():
    """[admission-fence] Fatal versus admission and publication, in every interleaving."""
    count = 0
    probe = Rig()
    probe_action = probe.reserved()
    width = len(probe.gate.admit_steps(probe.core, probe_action, {}))
    for schedule in interleavings([width + 1, 1]):
        rig = Rig()
        action = rig.reserved()
        result = {}
        owner = rig.gate.admit_steps(rig.core, action, result)
        owner.append(lambda: rig.core.start_native(action) if result.get("admit") == "Admitted" else None)
        threads = [owner, [lambda: rig.exchange.latch(("Sync", 4, "EIO"))]]
        for t in schedule:
            threads[t].pop(0)()
        latch = rig.latch_index()
        admits = [k for k, e in enumerate(rig.events) if e[0] == "admit"]
        assert all(k < latch for k in admits), \
            f"[admission-fence] an admission linearized after the fatal latch (schedule {schedule})"
        if rig.core.native:
            assert admits and result["admit"] == "Admitted", "[admission-fence] native work without admission"
        count += 1
    forged = Record(G_RUN, 1, 99, RS(), encode_record(G_RUN, 1, 99, RS())[-32:])
    for schedule in interleavings([3, 1]):
        rig = Rig()
        rig.core.start_run()
        rig.core.flush(rig.owner)
        worker = rig.worker.steps()
        threads = [[lambda: next(worker, None)] * 3, [lambda: rig.owner.submit(forged)]]
        for t in schedule:
            threads[t].pop(0)()
        latch = rig.latch_index()
        late = [e for e in rig.events[latch:] if e[0] == "publish"]
        assert not late, f"[admission-fence] publication after the latch (schedule {schedule})"
        rig.owner.apply()
        assert rig.exchange.fatal is not None and rig.owner.applied <= rig.exchange.fatal[1], \
            "[admission-fence] an acknowledgement beyond P"
        count += 1
    # Three parties: the worker appending record 2 (W1-W3, W4-W5, W6), the
    # owner's apply step, and a conflicting submission that latches.
    for schedule in interleavings([3, 1, 1]):
        rig = Rig()
        assert rig.do(rig.core.start_run) == "Started"
        rig.core.begin_case()
        rig.core.flush(rig.owner)
        worker = rig.worker.steps()
        threads = [[lambda: next(worker, None)] * 3, [rig.owner.apply],
                   [lambda: rig.owner.submit(forged)]]
        for t in schedule:
            threads[t].pop(0)()
        rig.owner.apply()
        latch = rig.latch_index()
        p = rig.exchange.fatal[1]
        late = [e for e in rig.events[latch:] if e[0] == "publish"]
        assert not late, f"[admission-fence] publication after the latch (schedule {schedule})"
        acknowledged = rig.core.ledger.next_ack - 1
        assert acknowledged == rig.owner.applied <= p, \
            f"[admission-fence] acknowledged {acknowledged} beyond P = {p} (schedule {schedule})"
        if acknowledged + 1 > len(rig.core.intents):
            # No record exists to fail yet: nothing may be reported.
            assert rig.core.ledger.failed is None and rig.owner.failure_outcome is None, \
                f"[admission-fence] a failure reported without an issued record (schedule {schedule})"
            rig.core.end_case()
            rig.pump()
        assert rig.core.ledger.failed == acknowledged + 1 and \
            rig.owner.failure_outcome == "FailureRecorded", \
            f"[admission-fence] failure at {rig.core.ledger.failed}, expected {acknowledged + 1} " \
            f"(schedule {schedule})"
        count += 1
    return f"{count} interleavings: no admission or publication after the latch"


def check_c20_session_verify():
    """[session-verify] Verification inside maintenance uses the session's own exclusion."""
    world = World(n=2, c_pool=16)
    session = MaintenanceSession(world, world.admin)
    assert session.begin() == "Active", "[session-verify] the session did not begin"
    rival = world.fs.register(Proc("rival"))
    seen = {}
    session.interleave = lambda: seen.update(rival=startup(world, rival, CFG, claim=True)["state"])
    mark = len(world.fs.log)
    rep = session.verify()
    assert seen.get("rival", "Busy") == "Busy", \
        "[session-verify] an owner started while the session released its lock"
    for label, competitor in (("standalone verifier", dict(sync=False, verifier=True)),
                              ("owner", dict(claim=True))):
        state = startup(world, rival, CFG, **competitor)["state"]
        assert state == "Busy", f"[session-verify] a {label} was not excluded during maintenance ({state})"
    assert rep["state"] == "Ready" and rep["scanned"], \
        f"[session-verify] in-session verification did not run ({rep['state']})"
    lock_ops = [e for e in world.fs.log[mark:] if e[0] == "root" and e[1].startswith("flock")
                and e[2] == "LOCK"]
    assert not lock_ops, f"[session-verify] the session touched its store lock during verification: {lock_ops}"
    forged = MaintenanceSession(world, rival)
    forged.active, forged.selection = True, session.selection
    assert forged.verify()["state"] == "NotAuthorized", \
        "[session-verify] verification without the retained lock was not refused"
    session.end()
    assert startup(world, rival, CFG, sync=False, verifier=True)["state"] == "Ready"
    # Nested: recycling an abandoned claim, then the PROVISION rewrite, in one session.
    w = World(n=2, c_pool=16)
    torn = bytearray((16 + 2) * BLOCK)
    torn[0:BLOCK] = header_block(ROOT_ID, G_RUN, 1, 0, 16, 16)
    torn[100] ^= 0xFF
    w.write_pool(0, bytes(torn))
    session = MaintenanceSession(w, w.admin)
    assert session.begin() == "Active"
    mark = len(w.fs.log)
    before = session.verify()
    assert before["files"][0]["class"] == "AbandonedClaim", "[session-verify] fixture"
    run_steps(w, recycle_steps(w, 0, session))
    after = session.verify()
    lock_ops = [e for e in w.fs.log[mark:] if e[0] == "root" and e[1].startswith("flock")
                and e[2] == "LOCK"]
    assert not lock_ops, "[session-verify] the nested procedure took another store lock"
    assert after["state"] == "Ready" and after["files"][0]["class"] == "Unused" and \
        after["provision"]["revision"] == 2, \
        f"[session-verify] verify-after inside the same session: {after['state']}"
    assert startup(w, rival, CFG, sync=False, verifier=True)["state"] == "Busy", \
        "[session-verify] a competitor entered between the nested steps"
    session.end()
    events = [e[0] for e in session.events]
    assert events[0] == "begin" and events[-1] == "end" and events.count("begin") == 1, \
        "[session-verify] verify-before, mutation and verify-after must share one lock lifetime"
    return "verification inside the session: no lock request, competitors excluded, nested rewrite covered"


def check_c21_provision_selection():
    """[provision-selection] A superseded PROVISION never regains authority."""
    done = []
    # The R1 interleaving: startup reads A, pauses before locking; succession
    # publishes B and releases A; the startup resumes.
    world = World(n=1, c_pool=16)
    stale = world.fs.register(Proc("stale"))

    def succession(w, attempt):
        if attempt == 0:
            s = MaintenanceSession(w, w.admin)
            assert s.begin() == "Active"
            run_steps(w, successor_steps(w, s, SUCCESSOR_ID, "store-b"))
            assert s.verify(state="store-b")["state"] == "Ready"
            s.end()

    rep = startup(world, stale, CFG, claim=True, hook=succession)
    assert rep["state"] == "Ready" and rep["provision"]["root_id"] == SUCCESSOR_ID, \
        f"[provision-selection] a stale selection regained authority: {rep['state']} " \
        f"{rep['provision']['root_id'].hex()[:8]}"
    fresh = world.fs.register(Proc("fresh"))
    other = startup(world, fresh, CFG, claim=True)
    assert other["state"] == "Busy", \
        f"[provision-selection] two cooperating owners: a second startup got {other['state']}"
    done.append("succession")
    # A PROVISION rewrite between selection and lock, for each rewriting procedure.
    for kind in ("retire", "requalify", "recycle"):
        w = World(n=2, c_pool=16)
        if kind == "recycle":
            torn = bytearray((16 + 2) * BLOCK)
            torn[0:BLOCK] = header_block(ROOT_ID, G_RUN, 1, 0, 16, 16)
            torn[100] ^= 0xFF
            w.write_pool(0, bytes(torn))
        proc = w.fs.register(Proc("stale"))

        def rewrite(w, attempt, kind=kind):
            if attempt:
                return
            if kind == "requalify":
                w.mount.lines[0] = (31, FIXTURE_DEV, "ext4", "rw,noatime", "rw,errors=remount-ro")
            s = MaintenanceSession(w, w.admin, requalify=kind == "requalify")
            assert s.begin() == "Active"
            if kind == "retire":
                steps = retire_steps(w, 0, s.verify(), s)
            elif kind == "requalify":
                steps = requalify_steps(w, s, "rw,noatime")
            else:
                steps = recycle_steps(w, 0, s)
            run_steps(w, steps)
            s.end()

        rep = startup(w, proc, CFG, hook=rewrite)
        assert rep["state"] == "Ready" and rep["provision"]["revision"] == 2 and rep["attempts"] == 2, \
            f"[provision-selection] {kind}: {rep['state']} revision {rep.get('provision', {}).get('revision')}"
        done.append(kind)
    # Bounded: the selection changes before every lock.
    w = World(n=2, c_pool=16)
    proc = w.fs.register(Proc("churn"))

    def churn(w, attempt):
        s = MaintenanceSession(w, w.admin)
        assert s.begin() == "Active"
        run_steps(w, rewrite_provision_steps(w, lambda record: None, s))
        s.end()

    rep = startup(w, proc, CFG, hook=churn)
    assert rep["state"] == "SelectionChanged" and rep["attempts"] == SELECTION_ATTEMPTS, \
        f"[provision-selection] unbounded re-selection: {rep['state']} after {rep['attempts']}"
    done.append("bounded")
    return "post-lock revalidation: " + ", ".join(done)


def check_c22_metadata_schedules():
    """[metadata-schedules] Metadata durable before an explicit directory sync,
    page reclaim with open descriptors, and the crash matrix against section 15.2."""
    scenarios = admin_scenarios()
    world, factory, label, use_session = scenarios["P-DISP"]
    n, cells = crash_series(world, factory, lambda *a: None, label, use_session)
    after_unlink = cells[5]
    assert {"published", "blocks"} <= after_unlink["ordered"], \
        f"[metadata-schedules] a link durable before the directory sync is not represented: {after_unlink}"
    assert "maintenance" in cells[1]["ordered"], \
        "[metadata-schedules] a temporary entry durable before any directory sync is not represented"
    world, factory, label, use_session = scenarios["P-REVOKE"]
    n, cells = crash_series(world, factory, lambda *a: None, label, use_session)
    assert all("invalid" not in c["ordered"] for c in cells), \
        "[metadata-schedules] the ordered schedules split an atomic rename"
    assert any("invalid" in c["perdir"] for c in cells), \
        "[metadata-schedules] the per-directory over-approximation is not exercised"
    # Page reclaim while the writer's descriptor is open.
    run = Run("T1", fail_sync_at=4)
    run.play()
    lo = 5 * BLOCK
    assert any(run.inode.K[lo:lo + BLOCK]) and not any(run.inode.D[lo:lo + BLOCK])
    assert run.world.fs.open_descriptions(run.inode)
    assert run.world.fs.evict_pages(run.inode), \
        "[metadata-schedules] clean pages must be reclaimable while a descriptor is open"
    assert not any(run.inode.K[lo:lo + BLOCK]), "[metadata-schedules] page reclaim did not drop the page"
    assert not run.world.fs.evict_inode(run.inode), \
        "[metadata-schedules] the inode (and its error sequence) must stay while open"
    report = classify(bytes(run.inode.K), ROOT_ID, 16, run.claim.index)
    assert report["refused"], "[metadata-schedules] reclaim made an unsealed generation look resolved"
    # The crash matrix of section 15.2.
    if not MATRIX_RESULT:
        check_c12_admin_crash()
    computed = {name: [list(r) for r in rows] for name, rows in MATRIX_RESULT.items()}
    expected = {name: [list(r) for r in rows] for name, rows in DOC_CRASH_MATRIX.items()}
    for name in sorted(set(computed) | set(expected)):
        assert computed.get(name) == expected.get(name), \
            f"[metadata-schedules] crash matrix differs from section 15.2 for {name}: {computed.get(name)}"
    return f"{len(computed)} procedures: computed crash matrix equals section 15.2; " \
           "link durable before sync, reclaim with open descriptors represented"


# The durability steps of every procedure, as section 13 specifies them (one
# pool file). "each" marks the operations repeated for every pool file.
REWRITE_STEPS = [("13.3/2", "create", "provdir/tmp"), ("13.3/2", "write", "provdir/tmp"),
                 ("13.3/2", "fsync", "provdir/tmp"), ("13.3/3", "rename", "provdir/PROVISION"),
                 ("13.3/3", "fsync_dir", "provdir")]
STORE_STEPS = [
    ("13.2/2", "mkdir", "parent/root"), ("13.2/2", "mkdir", "root/journals"),
    ("13.2/2", "mkdir", "root/dispositions"), ("13.2/2", "mkdir", "root/archive"),
    ("13.2/2", "mkdir", "dispositions/revoked"), ("13.2/2", "fsync_dir", "revoked"),
    ("13.2/2", "fsync_dir", "dispositions"), ("13.2/2", "fsync_dir", "journals"),
    ("13.2/2", "fsync_dir", "archive"), ("13.2/2", "fsync_dir", "root"),
    ("13.2/2", "fsync_dir", "parent"), ("13.2/3", "create", "root/LOCK"),
    ("13.2/3", "fsync", "root/LOCK"), ("13.2/3", "fsync_dir", "root"),
    ("13.2/4", "create", "journals/pool"), ("13.2/4", "write", "journals/pool"),
    ("13.2/4", "fsync", "journals/pool"), ("13.2/4", "fsync_dir", "journals")]
PROTOCOL = {
    "P-PROV": STORE_STEPS + [
        ("13.2/5", "create", "provdir/tmp"), ("13.2/5", "write", "provdir/tmp"),
        ("13.2/5", "fsync", "provdir/tmp"), ("13.2/5", "rename", "provdir/PROVISION"),
        ("13.2/5", "fsync_dir", "provdir")],
    "P-DISP": [("13.4/3", "create", "dispositions/tmp"), ("13.4/3", "write", "dispositions/tmp"),
               ("13.4/3", "fsync", "dispositions/tmp"), ("13.4/4", "link", "dispositions/final"),
               ("13.4/5", "unlink", "dispositions/tmp"), ("13.4/6", "fsync_dir", "dispositions")],
    "P-REVOKE": [("13.4-revoke/2", "rename", "revoked/entry"),
                 ("13.4-revoke/3", "fsync_dir", "revoked"),
                 ("13.4-revoke/3", "fsync_dir", "dispositions")],
    "P-ARCH": [("13.7/3", "create", "archive/tmp"), ("13.7/3", "write", "archive/tmp"),
               ("13.7/3", "fsync", "archive/tmp"), ("13.7/4", "link", "archive/final"),
               ("13.7/5", "unlink", "archive/tmp"), ("13.7/5", "fsync_dir", "archive")],
    "P-RECYCLE": [("13.8/2", "create", "journals/tmp"), ("13.8/2", "write", "journals/tmp"),
                  ("13.8/2", "fsync", "journals/tmp"), ("13.8/3", "rename", "journals/pool"),
                  ("13.8/3", "fsync_dir", "journals")] + REWRITE_STEPS,
    "P-RETIRE": REWRITE_STEPS,
    "P-REQUALIFY": REWRITE_STEPS,
    "P-SUCCESSOR": [("13.6/2a", "create", "provdir/tmp"), ("13.6/2a", "write", "provdir/tmp"),
                    ("13.6/2a", "fsync", "provdir/tmp"), ("13.6/2a", "link", "provdir/predecessor"),
                    ("13.6/2a", "unlink", "provdir/tmp"), ("13.6/2a", "fsync_dir", "provdir")]
                   + [("13.6/2b", op, role) for _, op, role in STORE_STEPS]
                   + [("13.6/2c", "create", "provdir/tmp"), ("13.6/2c", "write", "provdir/tmp"),
                      ("13.6/2c", "fsync", "provdir/tmp"), ("13.6/2c", "rename", "provdir/PROVISION"),
                      ("13.6/2c", "fsync_dir", "provdir")],
}
POOL_FILE_STEPS = 3     # create, write and fsync repeat for every pool file
# Recovery (sections 13.1 and 13.8): a leftover temporary disposition removed,
# and the resumption of a recycling interrupted after its rename.
RECOVERY_PROTOCOL = {
    "R-LEFTOVER": [("13.1/recover", "unlink", "dispositions/tmp"),
                   ("13.1/recover", "fsync_dir", "dispositions")],
    "R-RESUME": REWRITE_STEPS,
    # R3: re-publication of an unpublished root with history, with a leftover temporary.
    "R-REPUBLISH": [("13.2/recover", "unlink", "provdir/tmp"), ("13.2/recover", "create", "provdir/tmp"),
                    ("13.2/recover", "write", "provdir/tmp"), ("13.2/recover", "fsync", "provdir/tmp"),
                    ("13.2/recover", "rename", "provdir/PROVISION"),
                    ("13.2/recover", "fsync_dir", "provdir")],
}


def role_of(world, state, dir_ino, name):
    """Map a traced operation to the protocol's names for directories and files."""
    fs = world.fs
    names = {world.parent.ino: "parent", world.provdir.ino: "provdir"}
    if state in world.parent.ents_K:
        root = world.store_root(state)
        names[root.ino] = "root"
        for key in ("journals", "dispositions", "archive"):
            if key in root.ents_K:
                names[root.ents_K[key]] = key
        disp = root.ents_K.get("dispositions")
        if disp is not None and "revoked" in fs.inodes[disp].ents_K:
            names[fs.inodes[disp].ents_K["revoked"]] = "revoked"
    d = names.get(dir_ino, "other")
    if name is None:
        return d
    if d == "parent":
        return "parent/root"
    if d == "root":
        return f"root/{name}"
    if d == "dispositions":
        if name == "revoked":
            return "dispositions/revoked"
        return "dispositions/tmp" if name.startswith(".tmp-") else "dispositions/final"
    if d == "revoked":
        return "revoked/entry"
    if d in ("archive", "journals"):
        return f"{d}/tmp" if name.startswith(".tmp-") else (f"{d}/final" if d == "archive" else "journals/pool")
    if d == "provdir":
        if name == "PROVISION":
            return "provdir/PROVISION"
        return "provdir/predecessor" if name.startswith("PROVISION.predecessor-") else "provdir/tmp"
    return f"{d}/{name}"


def traced_protocol(world, steps, state):
    world.fs.trace = []
    run_steps(world, steps)
    trace, world.fs.trace = world.fs.trace, None
    mapped = []
    for step, op, dir_ino, name in trace:
        entry = (step, op, role_of(world, state, dir_ino, name if op != "fsync_dir" else None))
        if not mapped or mapped[-1] != entry:
            mapped.append(entry)
    return mapped


def check_c23_step_parity():
    """[step-parity] Every modelled durability operation is a written protocol step."""
    scenarios = admin_scenarios()
    checked = 0
    for name in PROTOCOL:
        world, factory, _, use_session = scenarios[name]
        w = _clone_world(world)
        steps = _steps_for(w, factory, use_session)
        state = "store-b" if name == "P-SUCCESSOR" else "store-a"
        mapped = traced_protocol(w, steps, state)
        assert mapped == PROTOCOL[name], \
            f"[step-parity] {name}: modelled operations differ from the protocol: " \
            f"{[m for m in mapped if m not in PROTOCOL[name]] or mapped}"
        assert len(steps) == len(PROTOCOL[name]), \
            f"[step-parity] {name}: crash points do not align with protocol operations"
        checked += 1
    # Provisioning with more pool files repeats exactly the per-file operations.
    w = World(n=3, c_pool=16, provision=False)
    mapped = traced_protocol(w, provision_steps(w, ROOT_ID, "store-a"), "store-a")
    pool = STORE_STEPS[14:17]
    expected = STORE_STEPS[:14] + pool * 3 + STORE_STEPS[17:] + PROTOCOL["P-PROV"][len(STORE_STEPS):]
    collapsed = []
    for entry in expected:
        if not collapsed or collapsed[-1] != entry:
            collapsed.append(entry)
    assert mapped == collapsed, "[step-parity] P-PROV with three pool files differs from the protocol"
    # Recovery operations are written protocol steps too.
    for name, factory, crash_after in (("R-LEFTOVER", scenarios["P-DISP"][1], 1),
                                       ("R-RESUME", scenarios["P-RECYCLE"][1], 5)):
        world = scenarios["P-DISP" if name == "R-LEFTOVER" else "P-RECYCLE"][0]
        w = _clone_world(world)
        run_steps(w, _steps_for(w, factory, True)[:crash_after])
        w.fs.kill(w.admin)
        session = MaintenanceSession(w, w.admin)
        assert session.begin() == "Active"
        if name == "R-LEFTOVER":
            steps = leftover_steps(w, session)
        else:
            steps = resume_recycle_steps(w, 0, session.verify(), session)
        mapped = traced_protocol(w, steps, "store-a")
        session.end()
        assert mapped == RECOVERY_PROTOCOL[name] and len(steps) == len(mapped), \
            f"[step-parity] {name}: recovery operations differ from the protocol: {mapped}"
        checked += 1
    # R3: the re-publication of an unpublished root, with a leftover temporary.
    w = World(n=1, c_pool=16, provision=False)
    run_steps(w, provision_steps(w, ROOT_ID, "store-a")[:21])     # PROVISION.tmp written, not renamed
    w.fs.kill(w.admin)
    steps = republish_steps(w, "store-a")
    mapped = traced_protocol(w, steps, "store-a")
    assert mapped == RECOVERY_PROTOCOL["R-REPUBLISH"] and len(steps) == len(mapped), \
        f"[step-parity] R-REPUBLISH: recovery operations differ from the protocol: {mapped}"
    checked += 1
    return f"{checked} procedures and recoveries: operations, order and crash points match section 13"


def check_c24_aggregate_bounds():
    """[aggregate-bounds] Bounded enumeration and reporting; never authorization
    from a truncated set."""
    done = []
    for label, path in (("dispositions", ("dispositions",)), ("revoked", ("dispositions", "revoked")),
                        ("archive", ("archive",))):
        w = World(n=2, c_pool=16)
        d = w.dir(*path)
        filler = w.fs.mkinode("file", 0, 0, 0o444)
        for i in range(ENTRY_LIMIT + (2 if label == "dispositions" else 1)):
            name = {"dispositions": f"{i:064x}.disposition",
                    "revoked": f"{i:064x}-20261001T120000Z.disposition",
                    "archive": f"p-00000-{i:064x}.journal"}[label]
            d.ents_K[name] = filler.ino
        mark = len(w.fs.log)
        rep = startup(w, w.fs.register(Proc("bounds")), CFG, sync=False, verifier=True)
        touched = [e for e in w.fs.log[mark:] if e[1] in ("fstatat", "open", "pread")
                   and (e[2].endswith(".disposition") or e[2].startswith("p-0"))]
        assert rep["state"] == "Invalid" and "enumeration bound" in rep["reason"], \
            f"[aggregate-bounds] {label}: {ENTRY_LIMIT + 1} entries gave {rep['state']} {rep['reason']}"
        assert not touched, f"[aggregate-bounds] {label}: entries examined before the bound refused"
        done.append(label)
    # Many incidents: the report is partial, authorization uses the complete set.
    w = World(n=80, c_pool=4)
    for i in range(80):
        garbage = bytearray((4 + 2) * BLOCK)
        garbage[0:8] = b"garbage!"
        garbage[5 * BLOCK] = 1
        w.write_pool(i, bytes(garbage))
    rep = startup(w, w.fs.register(Proc("many")), Config(4, 16), sync=False, verifier=True)
    assert rep["report"]["partial"] and rep["report"]["total"] == 80 and \
        len(rep["report"]["detail"]) == REPORT_LIMIT and rep["state"] == "Capacity", \
        f"[aggregate-bounds] a report beyond its bound must say so: {rep['state']} {rep['report']}"
    done.append("partial report")
    # Retirement with a partial report: 69 history gaps listed by three sealed
    # headers, one undispositioned gap beyond the reported detail.
    w = World(n=3, c_pool=16)
    gaps = [bind_gap(ROOT_ID, c) for c in range(1, 71)]
    d = w.dir("dispositions")
    for c in range(1, 70):
        fields = {"root": ROOT_ID, "binding": gaps[c - 1], "kind": "claim-gap", "claim": c,
                  "generation": None, "pool_index": None, "content": None, "class": "malformed",
                  "unsettled": 0, "reason": "other", "statement": "claim never durable",
                  "operator": "owner", "at": "2026-10-01T12:00:00Z"}
        inode = w.fs.create(d, gaps[c - 1].hex() + ".disposition", "file", 0, 0, 0o444)
        inode.K = bytearray(disposition_text(fields))
        inode.D = bytearray(inode.K)
    w.fs.fsync_dir(d)
    applied = sorted((g, 5) for g in gaps[:69])
    for index, claim, part in ((0, 71, applied[:32]), (1, 72, applied[32:64]), (2, 73, applied[64:])):
        data = bytearray((16 + 2) * BLOCK)
        data[0:BLOCK] = header_block(ROOT_ID, sha(u64(claim))[:16], claim, index, 16, 16, part)
        header = parse_header(data[0:BLOCK], ROOT_ID, 16, index)[1]
        data[BLOCK:2 * BLOCK] = seal_for(header, [])
        w.write_pool(index, bytes(data))
    cfg = Config(16, 32)
    before = _verify(w)
    assert before["state"] == "PriorUnresolved" and before["report"]["partial"] and \
        gaps[69] in before["blocking"], \
        f"[aggregate-bounds] a startup decided from the truncated report: {before['state']} " \
        f"partial={before['report']['partial']}"
    session = MaintenanceSession(w, w.admin)
    assert session.begin() == "Active"
    steps = retire_steps(w, 70, session.verify(cfg), session)
    if steps is not None:
        run_steps(w, steps)
    session.end()
    after = _verify(w)
    assert after["state"] == "PriorUnresolved" and gaps[69] in after["blocking"], \
        f"[aggregate-bounds] retirement from a partial report hid an incident ({after['state']})"
    done.append("retirement uses the complete set")
    return "bounded: " + ", ".join(done)


# Section 15.2 of the design, transcribed. reference_check.py checks that the
# document's table says exactly this; C22 checks that the model computes it.
DOC_CRASH_MATRIX = {
    'P-ARCH': [
        (0, 0, ('published',), ('published',), ()),
        (1, 4, ('maintenance',), ('maintenance', 'published'), ()),
        (5, 5, ('archived',), ('archived', 'maintenance', 'published'), ()),
        (6, 6, ('archived',), ('archived',), ()),
    ],
    'P-DISP': [
        (0, 0, ('blocks',), ('blocks',), ()),
        (1, 4, ('maintenance',), ('blocks', 'maintenance'), ()),
        (5, 5, ('published',), ('blocks', 'maintenance', 'published'), ()),
        (6, 6, ('published',), ('published',), ()),
    ],
    'P-PROV': [
        (0, 21, ('unprovisioned',), ('unprovisioned',), ()),
        (22, 22, ('fresh',), ('fresh', 'unprovisioned'), ()),
        (23, 23, ('fresh',), ('fresh',), ()),
    ],
    'P-RECYCLE': [
        (0, 0, ('archived',), ('archived',), ()),
        (1, 3, ('maintenance',), ('archived', 'maintenance'), ()),
        (4, 4, ('lost',), ('archived', 'lost', 'maintenance'), ()),
        (5, 8, ('lost',), ('lost',), ()),
        (9, 9, ('recycled',), ('lost', 'recycled'), ()),
        (10, 10, ('recycled',), ('recycled',), ()),
    ],
    'P-REQUALIFY': [
        (0, 3, ('unsupported',), ('unsupported',), ()),
        (4, 4, ('revision 2',), ('revision 2', 'unsupported'), ()),
        (5, 5, ('revision 2',), ('revision 2',), ()),
    ],
    'P-RETIRE': [
        (0, 3, ('bound 0',), ('bound 0',), ()),
        (4, 4, ('bound 1',), ('bound 0', 'bound 1'), ()),
        (5, 5, ('bound 1',), ('bound 1',), ()),
    ],
    'P-REVOKE': [
        (0, 0, ('published',), ('published',), ()),
        (1, 1, ('blocks',), ('blocks', 'published'), ('invalid',)),
        (2, 2, ('blocks',), ('blocks',), ('invalid',)),
        (3, 3, ('blocks',), ('blocks',), ()),
    ],
    'P-SUCCESSOR': [
        (0, 27, ('predecessor',), ('predecessor',), ()),
        (28, 28, ('successor',), ('predecessor', 'successor'), ()),
        (29, 29, ('successor',), ('successor',), ()),
    ],
}


# -- durable activation and composed recovery (design sections 10.6, 15.4) ----

class ProcessDied(Exception):
    """A test hook stopping a process in the middle of a call (F1 or F2 follows)."""


SELECTION_REFUSALS = ("Unprovisioned", "Lost", "Invalid", "MaintenanceIncomplete", "Unsupported",
                      "SelectionChanged", "Unreliable", "Busy")


def owner_work(world, name="owner", admit=True, activation_hook=None):
    """An ordinary owner (sections 9 and 10): startup with activation and claim,
    then RunStarted, CaseStarted and ActionStarted, each made durable by the
    worker and acknowledged by the owner, then an admission through the store
    gate and a native start. Returns the startup report and, once the
    ActionStarted is acknowledged, the generation the oracle must track."""
    proc = world.fs.register(Proc(name))
    rep = startup(world, proc, CFG, claim=True, seed=name.encode(), activation_hook=activation_hook)
    out = {"rep": rep, "proc": proc, "tracked": None, "admitted": None}
    if rep["state"] != "Ready":
        return out
    sel, claim = rep["selection"], rep["claim"]
    events = []
    core = CoreSim(claim.generation, events)
    exchange = Exchange(claim.generation, CFG.record_capacity, events)
    worker = Worker(world, claim.io_fd, exchange, claim.index, state=sel.state)
    owner = Owner(core, exchange, worker=worker)
    gate = AdmissionGate(exchange)
    out.update(core=core, exchange=exchange)
    for _ in worker.claim(claim.header):
        pass
    if exchange.claim != "Claimed":
        out["claim_failed"] = exchange.fatal
        return out

    def pump():
        core.flush(owner)
        for _ in worker.steps():
            pass
        owner.apply()

    core.start_run()
    pump()
    core.begin_case()
    pump()
    action = core.reserve()
    pump()
    if core.ledger.next_ack - 1 >= 3:
        out["tracked"] = {"state": sel.state, "index": claim.index, "generation": claim.generation}
    if admit:
        out["admitted"] = gate.admit(core, action)
        if out["admitted"] == "Admitted":
            core.start_native(action)
    if MUT.on("NC-ACT-ORDER"):
        # The wrong order: activation only after claim, acknowledgement and
        # admission. A crash can fall between the two.
        out["before_activation"] = _clone_world(world)
        late_activation(world, proc, rep)
    return out


def second_crashes(world, families=("ordered", "perdir")):
    """F1, then F2 under every distinct permitted schedule of each family, with
    pending data old and new. With nothing pending, every family has the same
    single schedule, which is run once."""
    yield ("F1", None, None)
    seen = set()
    for family in families:
        for schedule in world.fs.schedules(family):
            key = repr(schedule) if world.fs.pending_meta else "nothing pending"
            if key in seen:
                continue
            seen.add(key)
            for data in ("old", "new"):
                yield ("F2", schedule, data)


def apply_crash(world, crash, procs):
    kind, schedule, data = crash
    if kind == "F1":
        for proc in procs:
            world.fs.kill(proc)
    else:
        world.fs.power_loss(schedule, data=data)


def discoverable(rep, tracked):
    sel = rep.get("selection")
    return sel is not None and sel.state == tracked["state"] and any(
        inc.get("generation") == tracked["generation"] for inc in rep["incidents"].values())


def recover(world, rep):
    """The specified recovery for a refusing opening (sections 13.1, 13.2, 13.8),
    or None when the refusal is terminal (Owner disposition, history retained)."""
    if rep["state"] == "Unprovisioned":
        done = recover_unprovisioned(world)
        return done if done.startswith(("re-published", "provisioned")) else None
    if rep["state"] in ("MaintenanceIncomplete", "Lost"):
        session = MaintenanceSession(world, world.admin)
        if session.begin() != "Active":
            return None
        run_steps(world, leftover_steps(world, session))
        verified = session.verify()
        done = "leftovers removed"
        if verified["state"] == "Lost":
            steps = resume_recycle_steps(world, 0, verified, session)
            if steps is None:
                session.end()
                return None
            run_steps(world, steps)
            done = "recycling resumed"
        session.end()
        return done
    return None


def oracle(world, tracked, label, marker):
    """After a crash: a fresh owner either finds the tracked generation in the
    selected history or refuses explicitly; a refusal's specified recovery must
    then do the same. Returns (final report, recoveries applied)."""
    fresh = startup(world, world.fs.register(Proc("fresh")), CFG, claim=True)
    applied = []
    for attempt in range(3):
        if tracked is None or discoverable(fresh, tracked):
            return fresh, applied
        if fresh["state"] not in SELECTION_REFUSALS:
            sel = fresh.get("selection")
            root = sel.state if sel else None
            raise AssertionError(
                f"{marker} {label}: {fresh['state']} on {root} with the acknowledged, unsealed "
                f"generation of {tracked['state']} outside the selected history")
        done = recover(world, fresh)
        if done is None:
            inode = world.pool_inode(tracked["index"], state=tracked["state"])
            report = classify(bytes(inode.K), world.root_ids[tracked["state"]], world.C,
                              tracked["index"])
            assert report["refused"], f"{marker} {label}: terminal refusal lost the history"
            return fresh, applied + [f"terminal {fresh['state']}"]
        applied.append(done)
        fresh = startup(world, world.fs.register(Proc(f"fresh{attempt}")), CFG, claim=True)
    raise AssertionError(f"{marker} {label}: recovery did not converge ({applied})")


def witness_world(kind):
    """The section 3 witnesses, after F1: a successor B visible but not durable
    (operation 28 of P-SUCCESSOR), or a first PROVISION visible but not durable
    (operation 22 of P-PROV)."""
    if kind == "successor":
        world = World(n=1, c_pool=16)
        session = MaintenanceSession(world, world.admin)
        assert session.begin() == "Active", "fixture: session"
        run_steps(world, successor_steps(world, session, SUCCESSOR_ID, "store-b")[:28])
    else:
        world = World(n=1, c_pool=16, provision=False)
        run_steps(world, provision_steps(world, ROOT_ID, "store-a")[:22])
    world.fs.kill(world.admin)
    return world


def run_witness(kind, marker, admit=True, activation_hook=None, before=None, label=None):
    """Witness, ordinary owner, dependent work, every second crash, oracle."""
    label = label or f"{kind} witness"
    world = witness_world(kind)
    if before is not None:
        before(world)
    work = owner_work(world, admit=admit, activation_hook=activation_hook)
    checked = 0
    for snapshot in [w for w in (work.get("before_activation"), world) if w is not None]:
        for crash in second_crashes(snapshot):
            x = _clone_world(snapshot)
            apply_crash(x, crash, [x.fs._procs["owner"]] if crash[0] == "F1" else [])
            oracle(x, work["tracked"], f"{label}, {crash[0]} {crash[1] or ''} "
                   f"{crash[2] or ''}".strip(), marker)
            checked += 1
    return work, checked


def activation_ops(world, entries, proc_name, state):
    """The durability operations of one activation in a process's log, named
    as section 10.6 names them."""
    names = {"journals": "journals", "revoked": "dispositions/revoked", "dispositions": "dispositions",
             "archive": "archive", state: "root", world.fs._name_of(world.parent): "parent",
             world.fs._name_of(world.provdir): "provdir", "PROVISION": "PROVISION", "LOCK": "LOCK"}
    ops = []
    active = False
    for who, op, name in entries:
        if who != proc_name:
            continue
        if (op, name) == ("activate", "begin"):
            active = True
        elif (op, name) == ("activate", "certified"):
            break
        elif active and op in ("fsync_dir", "fdatasync", "futimens", "fsync"):
            ops.append((op, names.get(name, name)))
    return ops


def check_c25_durable_activation():
    """[durable-activation] No claim on a selection that is not durable; the
    activation's failures refuse; the read-only verifier never activates."""
    done = []
    marker = "[durable-activation]"
    # The section 3 witnesses: dependent work, then every second crash.
    for kind in ("successor", "provisioning"):
        work, checked = run_witness(kind, marker)
        assert work["rep"]["state"] == "Ready" and work["tracked"] is not None, \
            f"{marker} {kind} witness: the owner did not run ({work['rep']['state']})"
        done.append(f"{kind} witness: {checked} second crashes")
    # Failures before any claim: no generation, and nothing for a crash to omit.
    cases = [
        ("EIO at the PROVISION directory", lambda w: w.fs.dir_sync_plan.extend([None] * 6 + ["EIO"]),
         "Unreliable"),
        ("EINTR four times", lambda w: w.fs.dir_sync_plan.extend(["EINTR"] * 4), "Unreliable"),
        ("silent journal abort", lambda w: w.fs.abort_journal(), "Unreliable"),
        ("emergency read-only", lambda w: setattr(w.fs, "emergency_ro", True), "Unreliable"),
    ]
    for label, inject, expected in cases:
        work, checked = run_witness("successor", marker, before=inject, label=label)
        rep = work["rep"]
        assert rep["state"] == expected and rep["claim"] is None, \
            f"{marker} {label}: activation gave {rep['state']} {rep['reason']}"
        done.append(label)
    # Positive paths: an established store, and a completed succession.
    for label, make in (("established store", lambda: World(n=1, c_pool=16)),
                        ("completed succession", lambda: _completed_succession())):
        world = make()
        mark = len(world.fs.log)
        work = owner_work(world)
        assert work["rep"]["state"] == "Ready" and work["rep"]["activated"], \
            f"{marker} {label}: {work['rep']['state']} {work['rep']['reason']}"
        log = [e for e in world.fs.log[mark:] if e[0] == "owner"]
        ops = [e[1] for e in log]
        certified = log.index(("owner", "activate", "certified")) if \
            ("owner", "activate", "certified") in log else None
        syncs = [i for i, e in enumerate(log) if e[1] == "fsync_dir"]
        claim_lock = next(i for i, e in enumerate(log) if e[1] == "flock-EX" and e[2].endswith(".journal"))
        assert certified is not None and len(syncs) == len(ACTIVATION_DIRECTORIES) and \
            max(syncs) < certified < claim_lock, f"{marker} {label}: activation is not complete before the claim"
        assert "futimens" in ops and "fsync" in ops and \
            ops.index("futimens") < ops.index("fsync") < certified, \
            f"{marker} {label}: the certification probe is missing or late"
        first_scan = next(i for i, e in enumerate(log) if e[1] == "fstatat" and e[2] == "journals")
        assert certified < first_scan, f"{marker} {label}: the store was read before activation"
        performed = activation_ops(world, world.fs.log[mark:], "owner", work["rep"]["selection"].state)
        assert performed == [(op, obj) for _, op, obj in ACTIVATION_PROTOCOL], \
            f"{marker} {label}: activation operations differ from section 10.6: {performed}"
        for crash in second_crashes(world):
            x = _clone_world(world)
            apply_crash(x, crash, [x.fs._procs["owner"]] if crash[0] == "F1" else [])
            oracle(x, work["tracked"], f"{label} {crash[0]}", marker)
        done.append(label)
    world = witness_world("successor")
    world.fs.dir_sync_plan.extend(["EINTR"] * 2)
    work = owner_work(world)
    assert work["rep"]["activated"] and work["tracked"] is not None, \
        f"{marker} EINTR twice must be retried ({work['rep']['state']})"
    done.append("EINTR twice retried")

    def to_read_only(w, label):
        if label == "parent":
            line = w.mount.lines[0]
            w.mount.lines[0] = line[:4] + ("ro,errors=remount-ro",)
    world = witness_world("successor")
    work = owner_work(world, activation_hook=to_read_only)
    assert work["rep"]["state"] in ("Unreliable", "Unsupported") and work["rep"]["claim"] is None, \
        f"{marker} read-only during activation: {work['rep']['state']}"
    done.append("read-only superblock")

    def replace_archive(w, label):
        if label == "journals":
            root = w.store_root("store-b")
            w.fs.rename(root, "archive", root, "archive.old")
            w.fs.create(root, "archive", "dir", 0, 0, 0o755)
    world = witness_world("successor")
    work = owner_work(world, activation_hook=replace_archive)
    assert work["rep"]["state"] == "Invalid" and work["rep"]["claim"] is None, \
        f"{marker} a replaced directory: {work['rep']['state']}"
    done.append("identity")
    for kind in ("F1", "F2"):
        world = witness_world("successor")

        def die(w, label, kind=kind):
            if label == "root":
                raise ProcessDied(kind)
        try:
            owner_work(world, activation_hook=die)
            raise AssertionError(f"{marker} fixture: the activation hook did not run")
        except ProcessDied:
            pass
        if kind == "F1":
            world.fs.kill(world.fs._procs["owner"])
        else:
            world.fs.power_loss(world.fs.schedules("ordered")[0], data="old")
        assert not any(world.pool_inode(0, state="store-b").K), \
            f"{marker} {kind} during activation: a claim was written"
        oracle(world, None, f"{kind} during activation", marker)
        done.append(f"{kind} during activation")
    # Read-only inspection never activates.
    world = witness_world("successor")
    mark = len(world.fs.log)
    rep = startup(world, world.fs.register(Proc("verifier")), CFG, sync=False, verifier=True)
    ops = {e[1] for e in world.fs.log[mark:]}
    assert not rep["activated"] and not ops & {"fsync_dir", "futimens", "fsync", "activate"}, \
        f"{marker} the verifier synced or activated: {sorted(ops)}"
    done.append("verifier read-only")
    # A maintenance session activates before it verifies, and refuses on failure.
    world = World(n=1, c_pool=16)
    session = MaintenanceSession(world, world.admin)
    mark = len(world.fs.log)
    assert session.begin() == "Active", f"{marker} session begin"
    assert ("root", "activate", "certified") in world.fs.log[mark:], f"{marker} the session did not activate"
    session.end()
    world.fs.dir_sync_plan.extend(["EIO"])
    session = MaintenanceSession(world, world.admin)
    state = session.begin()
    assert state == "Unreliable" and not session.locks, f"{marker} a failed session activation: {state}"
    done.append("session activation")
    # One filesystem: the PROVISION directory or the state root elsewhere refuses.
    for label, move in (("PROVISION directory", lambda w: setattr(w.provdir, "dev", "253:2")),
                        ("state root a mount point", lambda w: setattr(w.store_root(), "dev", "253:2"))):
        world = World(n=1, c_pool=16)
        move(world)
        work = owner_work(world)
        assert work["rep"]["state"] == "Unsupported" and work["rep"]["claim"] is None, \
            f"{marker} {label} on another filesystem: {work['rep']['state']}"
        verified = startup(world, world.fs.register(Proc("verifier")), CFG, sync=False, verifier=True)
        assert verified["state"] == "Unsupported", \
            f"{marker} {label}: the verifier did not refuse ({verified['state']})"
        done.append(label)
    # A stale reader re-selects; a competing owner is Busy.
    world = World(n=1, c_pool=16)

    def succession(w, attempt):
        if attempt == 0:
            s = MaintenanceSession(w, w.admin)
            assert s.begin() == "Active"
            run_steps(w, successor_steps(w, s, SUCCESSOR_ID, "store-b"))
            s.end()
    stale = world.fs.register(Proc("stale"))
    rep = startup(world, stale, CFG, claim=True, generation=G_RUN, hook=succession)
    assert rep["state"] == "Ready" and rep["activated"] and rep["provision"]["root_id"] == SUCCESSOR_ID, \
        f"{marker} stale reader: {rep['state']}"
    rival = startup(world, world.fs.register(Proc("rival")), CFG, claim=True)
    assert rival["state"] == "Busy", f"{marker} competing owner: {rival['state']}"
    done.append("stale reader and competing owner")
    # An unsupported PROVISION replacement racing activation is never certified.

    def replace(w, label):
        if label == "parent" and "store-c" not in w.parent.ents_K:
            run_steps(w, provision_steps(w, bytes(range(0x30, 0x40)), "store-c"))
    work, checked = run_witness("successor", marker, activation_hook=replace,
                                label="replacement during activation")
    assert work["rep"]["state"] == "SelectionChanged" and work["rep"]["claim"] is None, \
        f"{marker} a replacement during activation: {work['rep']['state']}"
    done.append("replacement during activation")
    return "activation before every claim: " + "; ".join(done)


def _completed_succession():
    world = World(n=1, c_pool=16)
    session = MaintenanceSession(world, world.admin)
    assert session.begin() == "Active"
    run_steps(world, successor_steps(world, session, SUCCESSOR_ID, "store-b"))
    session.end()
    return world


def composed_scenarios():
    """The administrative procedures of the composed checks: (world, factory,
    use_session), each on a store with two pool files, so that an owner can claim
    after every procedure."""
    scenarios = admin_scenarios(n=2)
    out = {name: (scenarios[name][0], scenarios[name][1], scenarios[name][3])
           for name in ("P-PROV", "P-DISP", "P-REVOKE", "P-ARCH", "P-RECYCLE", "P-RETIRE",
                        "P-REQUALIFY", "P-SUCCESSOR")}
    return out


COMPOSED_STATS = {}
COMPOSED_TRACES = {}            # representative passing traces, one run per procedure


def check_c26_composed_recovery():
    """[composed-recovery] Administrative prefix, F1, ordinary startup and
    activation, claim, durable and acknowledged records, admission, a second
    F1 or F2, then a fresh startup and its recovery: an acknowledged, unsealed
    generation is always in the selected history or explicitly refused."""
    marker = "[composed-recovery]"
    stats = collections.Counter()
    COMPOSED_TRACES.clear()
    for name, (world, factory, use_session) in composed_scenarios().items():
        probe = _clone_world(world)
        total = len(_steps_for(probe, factory, use_session))
        for k in range(total + 1):
            prefix = _clone_world(world)
            steps = _steps_for(prefix, factory, use_session)
            run_steps(prefix, steps[:k])
            stats["prefixes"] += 1
            for first in second_crashes(prefix):
                base = _clone_world(prefix)
                apply_crash(base, first, [base.admin])
                work = owner_work(base)
                stats["first crashes"] += 1
                if work["tracked"] is None:
                    stats[f"refused before dependent work: {work['rep']['state']}"] += 1
                    continue
                stats["runs with dependent work"] += 1
                stats[f"runs with dependent work: {name}"] += 1
                label = f"{name} prefix {k}, first {first[0]} {first[1] or ''} {first[2] or ''}".strip()
                record = k == total - 1 and first[0] == "F1" and name not in COMPOSED_TRACES
                for snapshot in [w for w in (work.get("before_activation"), base) if w is not None]:
                    for crash in second_crashes(snapshot):
                        x = _clone_world(snapshot)
                        apply_crash(x, crash, [x.fs._procs["owner"]] if crash[0] == "F1" else [])
                        fresh, applied = oracle(
                            x, work["tracked"], f"{label}, second {crash[0]} {crash[1] or ''} "
                            f"{crash[2] or ''}".strip(), marker)
                        stats["second crashes"] += 1
                        stats[f"after the second crash: {fresh['state']}"] += 1
                        if record:
                            sel = fresh.get("selection")
                            COMPOSED_TRACES.setdefault(name, []).append(
                                f"{label}, second {crash[0]} {crash[1] or ''} {crash[2] or ''}".strip()
                                + f": {fresh['state']} on {sel.state if sel else None}, generation "
                                f"{'in' if discoverable(fresh, work['tracked']) else 'outside'} the decision")
                        for a in applied:
                            stats[f"recovery: {a}"] += 1
    # Chained administration: an archival crashed at every point (F1), then a
    # recycling and a retirement in later sessions, then F2 under every schedule.
    # A retired claim must keep its archive copy; an unretired one must stay in
    # the decision.
    scenarios = admin_scenarios(n=2)
    disposed, archive_factory = scenarios["P-ARCH"][0], scenarios["P-ARCH"][1]
    binding = scenarios["binding"]
    total = len(_steps_for(_clone_world(disposed), archive_factory, True))
    for k in range(total + 1):
        w = _clone_world(disposed)
        run_steps(w, _steps_for(w, archive_factory, True)[:k])
        w.fs.kill(w.admin)
        session = MaintenanceSession(w, w.admin)
        if session.begin() != "Active":
            continue
        verified = session.verify()
        recycled = 0 in verified.get("pending_recycle", set())
        if recycled:
            run_steps(w, recycle_steps(w, 0, session))
        session.end()
        session = MaintenanceSession(w, w.admin)
        assert session.begin() == "Active", f"{marker} chained administration: session"
        steps = retire_steps(w, 1, session.verify(), session) if recycled else None
        if steps is not None:
            run_steps(w, steps)
        session.end()
        for crash in second_crashes(w):
            x = _clone_world(w)
            apply_crash(x, crash, [])
            after = _verify(x)
            stats["chained administration: crashes"] += 1
            retired = after["provision"]["retired"] if after.get("provision") else 0
            archive = x.dir("archive").ents_K
            if retired >= 1:
                assert any(n.startswith("j-1-") for n in archive), \
                    f"{marker} chained administration, archival prefix {k}, {crash[0]} {crash[1] or ''}: " \
                    f"claim 1 retired with no archive copy (history dropped)"
                stats["chained administration: retired with the archive copy"] += 1
            else:
                assert after["state"] in SELECTION_REFUSALS or binding in after["blocking"] or \
                    binding in after["dispositioned"] or binding in after["history"], \
                    f"{marker} chained administration, archival prefix {k}, {crash[0]}: claim 1 dropped"
                stats["chained administration: claim 1 still decided"] += 1
    # A root left unpublished with history (domain S, or an R2-era store): the
    # recovery re-publishes it and never provisions a fresh replacement.
    world = World(n=1, c_pool=16)
    work = owner_work(world)
    world.fs.kill(work["proc"])
    world.fs.unlink(world.provdir, "PROVISION")
    world.fs.fsync_dir(world.provdir)
    fresh, applied = oracle(world, work["tracked"], "an unpublished root with history", marker)
    assert discoverable(fresh, work["tracked"]) and "store-fresh" not in world.parent.ents_K, \
        f"{marker} an unpublished root with history was replaced ({applied})"
    stats["unpublished root recovered"] += 1
    # An explicit disposition is the positive path: the generation is dispositioned.
    world = World(n=2, c_pool=16)
    work = owner_work(world)
    world.fs.kill(work["proc"])
    before = _verify(world)
    binding = next(b for b, inc in before["incidents"].items()
                   if inc.get("generation") == work["tracked"]["generation"])
    in_session(world, lambda w, s: disposition_steps(w, _incident_fields(before, binding)))
    for crash in second_crashes(world):
        x = _clone_world(world)
        apply_crash(x, crash, [])
        fresh, _ = oracle(x, work["tracked"], f"dispositioned, {crash[0]}", marker)
        assert fresh["state"] == "Ready" and binding in fresh["dispositioned"], \
            f"{marker} a durable disposition did not take effect ({fresh['state']})"
        stats["dispositioned generation: Ready"] += 1
    COMPOSED_STATS.clear()
    COMPOSED_STATS.update(stats)
    return (f"{stats['prefixes']} administrative prefixes, {stats['first crashes']} first crashes, "
            f"{stats['runs with dependent work']} runs with dependent work, "
            f"{stats['second crashes']} second crashes: every acknowledged, unsealed generation "
            f"discoverable or explicitly refused")


# ===========================================================================
# 13b. R4: the supported profile and the journal behind activation
#      (design sections 5.1, 6.4, 10.7 and 13.2)
# ===========================================================================


def _swap_option(old, new):
    def edit(m):
        listing = m.ext4_options[FIXTURE_NAME]
        listing[listing.index(old)] = new
    return edit


# Hosts that differ from the qualified profile in one fact, with the reason the
# opening must give. "absent from mountinfo": the superblock's default, which
# mountinfo's per-superblock options omit (super.c:2949-2951, 2985-2986).
PROFILE_CASES = [
    ("nobarrier, the superblock's default, absent from mountinfo and contradicting the attestation",
     _swap_option("barrier", "nobarrier"), "effective profile"),
    ("a kernel other than the qualified one, same release prefix",
     lambda m: setattr(m, "kernel", "6.17.0-fixture2 #1 SMP PREEMPT_DYNAMIC"),
     "kernel not qualified"),
    ("an external journal",
     lambda m: (m.jbd2.clear(), m.jbd2.add("sdb1"), m.journal.update(inode=0, device="8:17")),
     "journal not internal"),
    ("data=writeback, the superblock's default, absent from mountinfo",
     _swap_option("data=ordered", "data=writeback"), "effective profile"),
    ("data=journal", _swap_option("data=ordered", "data=journal"), "effective profile"),
    ("journal_async_commit", lambda m: m.ext4_options[FIXTURE_NAME].append("journal_async_commit"),
     "effective profile"),
    ("norecovery: no journal loaded",
     lambda m: (m.ext4_options[FIXTURE_NAME].remove("data=ordered"),
                m.ext4_options[FIXTURE_NAME].append("norecovery"), m.jbd2.clear()),
     "effective profile"),
    ("fc_debug_force", lambda m: m.ext4_options[FIXTURE_NAME].append("fc_debug_force"),
     "effective profile"),
    ("emergency read-only shown in the listing",
     lambda m: m.ext4_options[FIXTURE_NAME].append("emergency_ro"), "effective profile"),
    ("a read-only superblock in the listing", lambda m: m.ext4_options[FIXTURE_NAME].__setitem__(0, "ro"),
     "read-only"),
    ("the device name unresolved", lambda m: m.links.clear(), "device name unresolved"),
    ("the effective options unreadable", lambda m: m.ext4_options.clear(),
     "effective options unreadable"),
]


def check_c27_supported_profile():
    """[supported-profile] Only the qualified supported profile authorizes: an
    unsupported, unknown, contradictory or unqualified profile refuses before any
    claim, at every opening and again after activation's syncs; never decided
    from an absent string, a version prefix or an attestation; a qualified
    profile activates and claims."""
    marker = "[supported-profile]"
    done = []
    world = World(n=1, c_pool=16)
    assert not qualify(world.mount), f"{marker} the fixture's profile does not qualify"
    work = owner_work(world)
    assert work["rep"]["state"] == "Ready" and work["rep"]["activated"] and work["tracked"], \
        f"{marker} the qualified profile did not activate ({work['rep']['state']} {work['rep']['reason']})"
    done.append("the qualified profile activated and claimed")
    cases = [(label, None, edit, reason) for label, edit, reason in PROFILE_CASES]
    cases.append(("contradictory: mountinfo shows barrier, the listing nobarrier",
                  lambda m: m.lines.__setitem__(0, (31, FIXTURE_DEV, "ext4", "rw,relatime",
                                                    "rw,barrier,errors=remount-ro")),
                  _swap_option("barrier", "nobarrier"), "contradictory profile information"))
    for label, before, edit, reason in cases:
        world = World(n=1, c_pool=16, provision=False)
        if before is not None:
            before(world.mount)
        run_steps(world, provision_steps(world, ROOT_ID, "store-a"))
        edit(world.mount)
        work = owner_work(world)
        rep = work["rep"]
        assert rep["state"] == "Unsupported" and rep["reason"] == reason and rep["claim"] is None \
            and work["tracked"] is None and not any(world.pool_inode(0).K), \
            f"{marker} {label}: the owner got {rep['state']}{' (' + rep['reason'] + ')' if rep['reason'] else ''}, " \
            f"not a refusal before the claim"
        verified = startup(world, world.fs.register(Proc("verifier")), CFG, sync=False, verifier=True)
        assert verified["state"] == "Unsupported", f"{marker} {label}: the verifier got {verified['state']}"
        session = MaintenanceSession(world, world.admin)
        assert session.begin() == "Unsupported", f"{marker} {label}: a session began"
    done.append(f"{len(cases)} unsupported, unknown, contradictory or unqualified profiles refused "
                f"before any claim by owners, the verifier and sessions")

    def remount(w, label):
        if label == "parent":
            _swap_option("barrier", "nobarrier")(w.mount)
    world = World(n=1, c_pool=16)
    work = owner_work(world, activation_hook=remount)
    assert work["rep"]["state"] == "Unsupported" and work["rep"]["claim"] is None, \
        f"{marker} a profile changed during activation was certified ({work['rep']['state']})"
    done.append("a profile changed during activation refused at A4")
    for label, edit, problem in (
            ("a fast_commit feature", lambda m: m.features.add("fast_commit"), "fast_commit feature"),
            ("an external journal", lambda m: m.journal.update(inode=0, device="8:17"),
             "journal not internal at inode 8"),
            ("no journal", lambda m: m.features.discard("has_journal"), "no journal")):
        env = MountEnv()
        edit(env)
        assert problem in qualify(env), f"{marker} qualification accepted {label}"
    env = MountEnv()
    env.features.add("fast_commit")
    assert check_profile(env, {"kernel": env.kernel}) is None, \
        f"{marker} the model claims the store uid observes a superblock feature"
    done.append("qualification refuses fast_commit, an external journal and no journal; the store "
                "uid cannot observe the superblock features, which the Owner keeps (section 13.2)")
    # A kernel update: refused until re-qualification, which never accepts an unsupported profile.
    world = World(n=1, c_pool=16)
    world.mount.kernel = "6.17.0-fixture2 #1 SMP PREEMPT_DYNAMIC"
    rep = startup(world, world.fs.register(Proc("before")), CFG, claim=True)
    assert rep["state"] == "Unsupported" and rep["reason"] == "kernel not qualified", \
        f"{marker} an updated kernel was not refused ({rep['state']})"
    session = MaintenanceSession(world, world.admin, requalify=True)
    assert session.begin() == "Active", f"{marker} re-qualification could not begin"
    run_steps(world, requalify_steps(world, session, world.mount.lines[0][3], kernel=world.mount.kernel))
    session.end()
    work = owner_work(world)
    assert work["rep"]["state"] == "Ready" and work["tracked"], \
        f"{marker} after re-qualification: {work['rep']['state']} {work['rep']['reason']}"
    world.fs.kill(work["proc"])
    _swap_option("barrier", "nobarrier")(world.mount)
    session = MaintenanceSession(world, world.admin, requalify=True)
    assert session.begin() == "Unsupported", f"{marker} re-qualification accepted an unsupported profile"
    done.append("a kernel update refused until re-qualification, which never accepts an unsupported profile")
    return "; ".join(done)


class JournalSim:
    """The smallest state of the Linux v6.17 jbd2 and ext4 paths that the
    activation proof depends on (design sections 5.1 and 10.7). Each method is
    one source branch, cited; it is not a kernel. Profile: the journal is
    'internal' or 'external', commits 'sync' or 'async'. State: transactions
    running, committing or completed, then committed or failed; the abort flag;
    ext4's emergency read-only flag; the probe inode's sync tid; and, for each
    committed transaction, whether its log blocks are durable and still in the
    log and whether its home blocks are written or durable. Flushes the device
    fails are taken from `flushes`; `sites` counts every flush site executed
    with its outcome.

    R5 (design sections 5.5 to 5.8): `device` is "stable" (admitted storage: a
    write that completes successfully is stable, and the block layer sends no
    flush, blk-core.c:809-820), "volatile" (a volatile write-back cache that
    receives flushes; `fua` "native" or "emulated") or "unflushed" (a volatile
    cache the kernel registered as absent, so that no flush is sent: the
    configuration A-S1 excludes). Home writes complete with success, with a
    reported error (recorded on the device's errseq) or not yet; the log tail
    is the oldest transaction whose home writes have not all completed; the
    superblock's tail write, and the abort's rewrite of it, may fail, after
    which the medium may hold the old tail or the new one."""

    def __init__(self, journal="internal", commit="sync", device="stable", fua="native"):
        self.journal, self.commit = journal, commit
        self.device, self.fua = device, fua
        self.txns = {}
        self.next_tid = 1
        self.running = self.committing = None
        self.commit_sequence = 0
        self.aborted = self.emergency = False
        self.lock_tid = 0               # the probe inode's sync tid: an older, completed one
        self.outcomes = []              # outcomes of the commits a waiting sync forces
        self.flushes = []
        self.sites = collections.Counter()
        self.trace = []
        self.home_plan = []             # R5: outcomes of the next home writes: ok, fail, inflight
        self.sb_plan = []               # R5: outcomes of the next superblock writes: ok, fail
        self.dev_err = False            # R5: a write error recorded on the device since the journal's sample
        self.mem_tail = 1               # R5: j_tail_sequence (journal.c:1084-1086)
        self.sb_mem = 1                 # R5: the tail the in-memory superblock holds (journal.c:1871-1872)
        self.sb_disk = (1,)             # R5: the tails the superblock on the medium may hold

    def txn_of(self, name):
        return next((t for t in self.txns.values() if name in t["ops"]), None)

    def committed(self, name):
        t = self.txn_of(name)
        return t is not None and t["outcome"] == "committed"

    def survives(self, name):
        """After a power loss, recovery replays the committed transactions still in
        the log from the superblock's tail (recovery.c:611); a transaction behind
        the tail survives only through its durable home blocks. R5: the tail is the
        one on the medium; after a failed superblock write it may be either."""
        t = self.txn_of(name)
        if t is None or t["outcome"] != "committed":
            return False
        if t["home"] == "durable":
            return True
        return t["log_durable"] and all(t["tid"] >= tail for tail in self.sb_disk)

    def abort(self, why):
        """jbd2_journal_abort: permanent until the journal is closed (journal.c:2549-2597).
        R5: it writes the superblock with the error code (journal.c:2592-2595), carrying
        the tail the in-memory superblock holds."""
        if not self.aborted:
            self.aborted = True
            self.trace.append(("abort", why))
            self._write_sb()

    def _join(self):
        """start_this_handle joins the running transaction or starts one."""
        if self.running is None:
            self.running = self.next_tid
            self.txns[self.next_tid] = {"tid": self.next_tid, "ops": [], "state": "running",
                                        "outcome": None, "log_durable": False, "in_log": True,
                                        "home": None}
            self.next_tid += 1
        return self.running

    def op(self, name):
        """A metadata operation's handle start: ext4_journal_check_start returns an
        emergency state, or turns an aborted journal into emergency read-only
        (ext4_jbd2.c:65-91)."""
        if self.emergency:
            return "EROFS"
        if self.aborted:
            self.emergency = True
            return "EROFS"
        self.txns[self._join()]["ops"].append(name)
        return 0

    def start_commit(self):
        """kjournald2 takes the running transaction to commit (commit.c:532-534)."""
        if self.running is None or self.committing is not None:
            return False
        self.committing, self.running = self.running, None
        self.txns[self.committing]["state"] = "committing"
        return True

    def flush(self, site, checked):
        """A cache flush. R5: only a volatile cache that the kernel knows of receives it;
        otherwise blk-core.c:809-820 completes it without the device, and an injected
        failure is never consumed."""
        if self.device != "volatile":
            self.sites[(site, "not sent")] += 1
            return "not sent"
        outcome = self.flushes.pop(0) if self.flushes else "ok"
        self.sites[(site, outcome)] += 1
        if outcome != "ok" and not checked and MUT.on("NC-ERR-DETECTED"):
            self.abort(f"{site} (invented)")    # Wrong: a discarded status treated as detected
        return outcome

    def _home_flushed(self, ok):
        """A flush the device completed makes every completed home write stable."""
        if ok and self.device == "volatile":
            for t in self.txns.values():
                if t["home"] == "written":
                    t["home"] = "durable"

    def _home_write(self, t, outcome):
        """R5: a home write completes successfully (stable on stable storage, otherwise in
        the device's cache), completes with a reported error, recorded on the device's
        errseq before the buffer is unlocked (buffer.c:165-176, 387-424, 1214-1222), or is
        still in flight."""
        if outcome == "inflight":
            t["home"] = "inflight"
        elif outcome == "fail":
            t["home"] = "failed"
            self.dev_err = True
        else:
            t["home"] = "durable" if self.device == "stable" else "written"

    def _home_done(self, t):
        """checkpoint.c:235-258, 627-648: a buffer leaves the checkpoint list only once
        it is unlocked, so its write has completed, with success or an error."""
        if MUT.on("NC-STORAGE-COMPLETION") and t["home"] == "inflight":
            return True                 # Wrong: a write still in flight counted as complete
        return t["home"] in ("written", "durable", "failed")

    def _log_tail(self):
        """jbd2_journal_get_log_tail (journal.c:1017-1044): the oldest transaction still
        on the checkpoint list, else the committing or the running one."""
        waiting = [t["tid"] for t in self.txns.values() if t["in_log"] and (
            t["state"] != "completed" or (t["outcome"] == "committed" and not self._home_done(t)))]
        return min(waiting) if waiting else self.next_tid

    def _write_sb(self):
        """jbd2_write_superblock (journal.c:1784-1840) with REQ_FUA. Stable storage, and a
        cache with native FUA, complete it stably; with emulated FUA a flush follows the
        write (blk-flush.c:398-403), covering every write completed before it, and its
        failure fails the write (blk-flush.c:160-163); an unflushed cache only caches it
        (taken here as reaching the medium, the worst case). A failed write may have
        reached the medium or not, and aborts the journal (1827-1837), whose errno write
        rewrites the same in-memory superblock (2592-2595)."""
        outcome = self.sb_plan.pop(0) if self.sb_plan else "ok"
        if outcome == "ok" and self.device == "volatile" and self.fua == "emulated":
            if self.flush("blk-flush.c:398-403", checked=True) == "ok":
                self._home_flushed(True)
            else:
                outcome = "fail"
        if outcome == "ok":
            self.sb_disk = (self.sb_mem,)
            return True
        self.sb_disk = tuple(sorted(set(self.sb_disk) | {self.sb_mem}))
        self.trace.append(("superblock write failed", self.sb_mem))
        if not self.aborted:
            self.abort("the superblock write failed (journal.c:1827-1837)")
        return False

    def _tail_update(self):
        """__jbd2_update_log_tail (journal.c:1056-1091) and
        jbd2_journal_update_sb_log_tail (1852-1885): an aborted journal refuses
        (1859-1860); a write error recorded on the device aborts before the new tail is
        set (1861-1864); the superblock is written (1871-1874), and only then does the
        in-memory tail move (1084-1086)."""
        if self.aborted:
            return "EIO"
        new = self._log_tail()
        if new <= self.mem_tail:
            return 0
        if self.dev_err and not MUT.on("NC-STORAGE-TAIL"):
            self.abort("a home write failed (journal.c:1861-1864)")
            return "EIO"
        self.sb_mem = new
        if not self._write_sb():
            return "EIO"
        self.mem_tail = new
        for t in self.txns.values():
            if t["tid"] < new:
                t["in_log"] = False
        return 0

    def finish_commit(self, outcome="ok"):
        """The committing transaction completes (commit.c:348-1160); 'io-error' is a
        checked failure, a log or commit-record write (866, 878, 888-889). Every
        abort precedes the completion (1104-1105)."""
        tid = self.committing
        if tid is None:
            return False
        t = self.txns[tid]
        tail = any(u["in_log"] and u["outcome"] == "committed" and u["home"] for u in self.txns.values())
        if self.journal == "external" and not self.aborted:
            # 775-778: the filesystem device is flushed before the record; status discarded
            self._home_flushed(self.flush("commit.c:775-778", checked=False) == "ok")
        if not self.aborted and outcome == "io-error":
            self.abort(f"the commit of transaction {tid}")
        if self.aborted:
            t["outcome"] = "failed"             # no commit record is written (126-127)
        else:
            t["outcome"] = "committed"
            if self.commit == "async":
                # 781-786, 883-886: the record goes out early without PREFLUSH|FUA; the
                # final flush makes the log durable on a cache, and its status is discarded
                final = self.flush("commit.c:883-886", checked=False)
                t["log_durable"] = self.device == "stable" or final == "ok"
            else:
                # 803-866, 874-881, 888-889: the record goes out after the log blocks
                # completed, with PREFLUSH|FUA, its completion checked; on an internal
                # journal the PREFLUSH also flushes the home blocks written before it. R5:
                # on stable storage the flags are stripped and every completion is stable;
                # on an unflushed cache nothing is stable yet.
                t["log_durable"] = self.device != "unflushed"
                self.sites[("commit.c:152-154", "ok" if self.device == "volatile" else "not sent")] += 1
                if self.journal == "internal":
                    self._home_flushed(True)
            if tail:
                self._tail_update()             # 899-900
        t["state"] = "completed"
        self.commit_sequence = tid
        self.committing = None
        self.trace.append(("completed", tid, t["outcome"]))
        return True

    def _complete_through(self, tid):
        """jbd2_log_wait_commit waits until `tid` completed (journal.c:678-686);
        commits run one at a time, in order."""
        while self.commit_sequence < tid:
            if self.committing is None:
                self.start_commit()
            self.finish_commit(self.outcomes.pop(0) if self.outcomes else "ok")

    def dir_fsync(self):
        """fsync of a directory: an emergency state first (fsync.c:135-137);
        ext4_force_commit (108-109) -> __jbd2_journal_force_commit
        (journal.c:499-527): the running transaction, else the committing one, is
        waited for and the abort flag then tested (689-690); with neither,
        "Nothing to commit" returns 0 untested (513-517)."""
        if self.emergency:
            return "EROFS"
        tid = self.running if self.running is not None else self.committing
        if tid is None:
            return 0
        self._complete_through(tid)
        return "EIO" if self.aborted else 0

    def probe_touch(self, window=False):
        """futimens(LOCK) by its owner: ext4_setattr returns an emergency state first
        (inode.c:5854-5856); ext4_dirty_inode's handle start (6531-6540) turns an
        aborted journal into emergency read-only (ext4_jbd2.c:86-88), and its error
        is swallowed. `window`: the abort lands after that test and before
        start_this_handle's (transaction.c:366-371), which sets nothing; the sync
        tid is not updated (ext4_jbd2.h:354-365)."""
        if self.emergency:
            return "EROFS"
        if self.aborted:
            self.emergency = True
            self.trace.append(("probe handle", "abort seen"))
            return 0
        self.trace.append(("probe handle", "passed ext4's test"))
        if window:
            self.abort("between the probe's two handle tests")
            return 0
        tid = self._join()
        self.txns[tid]["ops"].append("probe")
        self.lock_tid = tid
        return 0

    def probe_fsync(self):
        """fsync(LOCK): an emergency state first (fsync.c:135-137); then
        jbd2_complete_transaction for i_sync_tid (fast_commit.c:1207-1208,
        journal.c:787-808): a running or committing transaction is waited for and
        the abort flag tested (689-690); a completed one returns 0 untested
        (800-805), after ext4's own cache flush, checked (fsync.c:111-113,
        166-170). R5: on stable storage that flush is not sent."""
        if self.emergency:
            return "EROFS"
        if MUT.on("NC-FSYNC-COMPLETED") and self.aborted:
            return "EIO"                        # R3's model
        tid = self.lock_tid
        if tid in (self.running, self.committing):
            self.trace.append(("fsync branch", "running" if tid == self.running else "committing"))
            self._complete_through(tid)
            return "EIO" if self.aborted else 0
        self.trace.append(("fsync branch", "completed"))
        self.trace.append(("completed branch", self.aborted))
        return "EIO" if self.flush("fsync.c:166-170", checked=True) == "fail" else 0

    def writeback(self):
        """Committed transactions' home blocks are written, by checkpointing
        (checkpoint.c:154-298) or ordinary writeback, without a flush yet. R5: each
        write's outcome is taken from `home_plan`, success by default."""
        for t in self.txns.values():
            if t["outcome"] == "committed" and t["in_log"] and t["home"] is None:
                self._home_write(t, self.home_plan.pop(0) if self.home_plan else "ok")

    def complete_inflight(self, outcome="ok"):
        """R5: home writes still in flight complete."""
        for t in self.txns.values():
            if t["home"] == "inflight":
                self._home_write(t, outcome)

    def checkpoint(self):
        """jbd2_log_do_checkpoint writes the home blocks; jbd2_cleanup_journal_tail
        (checkpoint.c:318-342): an aborted journal refuses (323-324); the flush
        before the tail moves is issued and its status discarded (338-339); the
        superblock update then moves the tail (journal.c:1852-1885)."""
        self.writeback()
        if self.aborted:
            return "EIO"
        self._home_flushed(self.flush("checkpoint.c:338-339", checked=False) == "ok")
        return self._tail_update()


JOURNAL_PRE = ("d1", "d2", "u", "start", "ok", "fail")
JOURNAL_GAPS = (None, "start", "bg-ok", "bg-fail", "u", "abort")
JOURNAL_AFTER_R4 = ((), ("u2", "bg-fail"), ("abort",), ("u2", "bg-ok", "ckpt"),
                    ("u2", "bg-fail", "ckpt"), ("ckpt",))
# R5: on admitted storage the later continuations also include every failure the storage reports
# and a write still in flight (design sections 5.6 and 5.7), and the R4 witness's failed flush,
# which such storage is never sent.
JOURNAL_AFTER = JOURNAL_AFTER_R4 + (
    ("flushfail", "ckpt"), ("wbfail", "ckpt"), ("u2", "bg-ok", "wbfail", "ckpt"),
    ("inflight", "ckpt"), ("sbfail", "ckpt"), ("sbfail2", "ckpt"))
JOURNAL_STATS = {}


def journal_event(j, ev, strict=False):
    """One event of the transaction model's environment: the dependency
    operations d1 and d2 (a procedure's metadata, before activation), unrelated
    operations u, a commit started or completed in the background, an ext4 error
    elsewhere that aborts the journal (super.c:691-692), and a checkpoint."""
    if ev in ("d1", "d2"):
        return j.op(ev) == 0
    if ev in ("u", "u2"):
        j.op(ev)
        return True
    if ev == "start":
        return j.start_commit() or not strict
    if ev in ("ok", "fail"):
        return j.finish_commit("ok" if ev == "ok" else "io-error")
    if ev in ("bg-ok", "bg-fail"):
        if j.committing is None and not j.start_commit():
            return True
        return j.finish_commit("ok" if ev == "bg-ok" else "io-error")
    if ev == "abort":
        j.abort("an ext4 error elsewhere")
        return True
    if ev == "wb":
        j.writeback()
        return True
    if ev == "ckpt":
        j.checkpoint()
        return True
    # R5 (design sections 5.6 to 5.8): the storage's own events
    if ev == "flushfail":
        j.flushes = ["fail"]            # the next flush the device receives fails
        return True
    if ev == "flush2fail":
        j.flushes = ["fail", "fail"]
        return True
    if ev == "flush3fail":
        j.flushes = ["fail", "fail", "fail"]
        return True
    if ev == "wbfail":
        j.home_plan = ["fail"]          # the next home write reports an error
        return True
    if ev == "inflight":
        j.home_plan = ["inflight"]      # the home writes are submitted and not completed
        j.writeback()
        return True
    if ev == "complete":
        j.complete_inflight("ok")
        return True
    if ev == "sbfail":
        j.sb_plan = ["fail", "ok"]      # the tail write fails, the abort's rewrite succeeds
        return True
    if ev == "sbfail2":
        j.sb_plan = ["fail", "fail"]    # both fail: the medium holds the old tail or the new
        return True
    raise ValueError(ev)


def journal_initial_states(max_len=4):
    """Every distinct journal state reachable by at most `max_len` events that
    issue d1 and d2 once each: the dependencies in running, committing, committed
    or failed transactions, before or after unrelated work and commits."""
    seen = {}
    for n in range(2, max_len + 1):
        for seq in itertools.product(JOURNAL_PRE, repeat=n):
            if seq.count("d1") != 1 or seq.count("d2") != 1:
                continue
            j = JournalSim()
            if not all(journal_event(j, ev, strict=True) for ev in seq):
                continue
            key = repr((sorted((tid, t["state"], t["outcome"], tuple(t["ops"]))
                               for tid, t in j.txns.items()), j.running, j.committing, j.aborted))
            seen.setdefault(key, (seq, j))
    return list(seen.values())


def journal_activation(j, gaps, window):
    """The journal side of section 10.6: A2's syncs of the two dependency
    directories, then A3's probe; gaps[i] is a background event before step i."""
    steps = ["sync", "sync", "touch", "fsync"]
    if MUT.on("NC-PROOF-ORDER"):
        steps = ["touch", "fsync", "sync", "sync"]   # Wrong: the probe before the syncs
    results = []
    for gap, step in zip(gaps, steps):
        if gap is not None:
            journal_event(j, gap)
        if step == "sync":
            result = j.dir_fsync()
        elif step == "touch":
            result = j.probe_touch(window)
        else:
            result = j.probe_fsync()
        results.append(result)
        if result != 0:
            return False, results
        if step == "sync" and steps[:len(results)].count("sync") == 2:
            j.trace.append(("A2 syncs returned",))
    return True, results


JOURNAL_WINDOWS = (
    ("an abort before the probe's handle test", lambda j: ("probe handle", "abort seen") in j.trace),
    ("an abort inside the probe's handle start",
     lambda j: ("abort", "between the probe's two handle tests") in j.trace),
    ("the probe's transaction running at its fsync", lambda j: ("fsync branch", "running") in j.trace),
    ("the probe's transaction committing at its fsync",
     lambda j: ("fsync branch", "committing") in j.trace),
    ("the probe's transaction completed before its fsync",
     lambda j: ("fsync branch", "completed") in j.trace),
    ("an error after the directory syncs returned", lambda j: _error_after_syncs(j)),
    ("emergency read-only at a sync or the probe", lambda j: j.emergency),
)


def _error_after_syncs(j):
    done = next((i for i, e in enumerate(j.trace) if e == ("A2 syncs returned",)), None)
    aborted = next((i for i, e in enumerate(j.trace) if e[0] == "abort"), None)
    return done is not None and aborted is not None and aborted > done


def journal_runs(stats):
    """Every initial state, every background event (or none) before each of the
    four steps, an abort inside the probe's handle start or not, and the commits
    a waiting sync forces succeeding or failing. R5: on admitted storage, where
    no flush is sent and failures are reported (design section 5.6). Yields one
    run at a time."""
    for seq, j0 in journal_initial_states():
        stats["initial states"] += 1
        for gaps in itertools.product(JOURNAL_GAPS, repeat=4):
            for window in (False, True):
                for wait in ("ok", "io-error"):
                    j = copy.deepcopy(j0)
                    j.outcomes = [wait] * 6
                    certified, results = journal_activation(j, gaps, window)
                    stats["runs"] += 1
                    yield seq, gaps, window, wait, certified, results, j


def _late_abort(j):
    """Whether the abort was set after the probe's handle passed ext4's test."""
    passed = next((i for i, e in enumerate(j.trace) if e == ("probe handle", "passed ext4's test")), None)
    aborted = next((i for i, e in enumerate(j.trace) if e[0] == "abort"), None)
    return passed is not None and aborted is not None and aborted > passed


def check_c28_activation_proof():
    """[activation-proof] Whatever the transaction states and the timing of
    aborts and commits, an activation that certifies has its dependencies in
    committed transactions that survive every later crash and every later error
    of domain H; a failed dependency persistence refuses, even behind later
    successes; a late, harmless error is certified with the history intact; and
    the protocol certifies whenever nothing failed. R5: on admitted storage, and
    the later continuations include reported home and superblock write failures,
    incomplete home writes and a flush failure the device is never sent."""
    marker = "[activation-proof]"
    stats = collections.Counter()
    for seq, gaps, window, wait, certified, results, j in journal_runs(stats):
        label = f"initial {seq}, gaps {gaps}, abort in the probe's handle {window}, waits {wait}"
        for name, applies in JOURNAL_WINDOWS:
            if applies(j):
                stats[f"window: {name}: {'certified' if certified else 'refused'}"] += 1
        if not certified:
            stats["refused"] += 1
            assert j.aborted, f"{marker} {label}: refused although nothing failed ({results})"
            if j.committed("d1") and j.committed("d2"):
                stats["refused, the dependencies committed (a conservative refusal)"] += 1
            continue
        stats["certified"] += 1
        assert j.committed("d1") and j.committed("d2"), \
            f"{marker} {label}: certified with a dependency in a failed transaction ({results})"
        if j.aborted:
            stats["certified, the journal aborted by then"] += 1
            assert _late_abort(j), f"{marker} {label}: certified over an abort before the probe's test"
        if ("completed branch", True) in j.trace:
            stats["certified through the completed branch with the journal aborted"] += 1
        for after in JOURNAL_AFTER:
            k = copy.deepcopy(j)
            for ev in after:
                journal_event(k, ev)
            stats["continuations, then power loss"] += 1
            assert k.survives("d1") and k.survives("d2"), \
                f"{marker} {label}, then {after}: a certified dependency lost at power loss"
    assert stats["certified"] and stats["certified, the journal aborted by then"], \
        f"{marker} no certified run, or no harmless late error certified"
    # The same windows through the whole model: the successor witness, an owner,
    # dependent work, every second crash and the oracle.
    for label, hook, certifies in (
            ("an abort between the probe's two handle tests",
             lambda w, l: setattr(w.fs, "abort_in_probe_handle", l == "probe"), True),
            ("the probe's transaction committed, then a later commit failed",
             lambda w, l: (w.fs.commit_probe(), w.fs.abort_journal()) if l == "probe-fsync" else None, True),
            ("the probe's own commit failed in the background",
             lambda w, l: w.fs.commit_probe(fail=True) if l == "probe-fsync" else None, True),
            ("a commit failed while the probe's transaction ran",
             lambda w, l: w.fs.abort_journal() if l == "probe-fsync" else None, False)):
        work, checked = run_witness("successor", marker, activation_hook=hook, label=label)
        rep = work["rep"]
        if certifies:
            assert rep["state"] == "Ready" and rep["activated"] and work["tracked"], \
                f"{marker} {label}: a harmless late error refused ({rep['state']} {rep['reason']})"
        else:
            assert rep["state"] == "Unreliable" and rep["claim"] is None, \
                f"{marker} {label}: {rep['state']} {rep['reason']}"
        stats[f"whole model: {label}: {rep['state']}, {checked} second crashes"] += 1
    JOURNAL_STATS.clear()
    JOURNAL_STATS.update(stats)
    return (f"{stats['initial states']} initial journal states, {stats['runs']} activation runs: "
            f"{stats['certified']} certified, {stats['refused']} refused; "
            f"{stats['certified, the journal aborted by then']} certified after a late abort; "
            f"{stats['continuations, then power loss']} later continuations and power losses kept "
            f"every certified dependency; 4 windows through the whole model")


def check_c29_journal_conformance():
    """[journal-conformance] The modelled journal transitions follow the cited
    v6.17 branches: fsync of a completed transaction returns 0 without the abort
    test; an emergency state is returned first; a discarded flush status is never
    turned into an abort; on a volatile write-back cache the checkpoint flush's
    discarded status can lose a certified dependency (R4's counterexample,
    retained: such storage is not admitted); R5, the superblock's tail write, the
    abort's rewrite and native or emulated FUA, each kept separate; on admitted
    storage no flush is sent, a failed home write aborts before a new tail is
    written, and a write in flight holds the tail; and the checked statuses
    abort."""
    marker = "[journal-conformance]"
    done = []
    # fsync of LOCK, the three branches, on the whole model.
    world = World(n=1, c_pool=16)
    proc = world.fs.register(Proc("prober"))
    fd = world.fs.open(proc, "main", world.store_root(), "LOCK")
    fs = world.fs
    assert fs.touch(fd) == 0 and fs.probe_txn == "running", f"{marker} the probe joined no transaction"
    fs.commit_probe()
    fs.abort_journal()
    result = fs.fsync_file(fd)
    assert result == 0, \
        f"{marker} fsync of a completed transaction after an abort returned {result}; " \
        f"jbd2_complete_transaction returns 0 untested (journal.c:800-805)"
    fs.journal_aborted = False
    assert fs.touch(fd) == 0 and fs.probe_txn == "running"
    fs.abort_journal()
    result = fs.fsync_file(fd)
    assert result == "EIO", f"{marker} fsync of a running transaction of an aborted journal returned {result}"
    fs.emergency_ro = True
    result = fs.fsync_file(fd)
    assert result == "EROFS", f"{marker} an emergency state was not returned first ({result})"
    done.append("fsync: completed 0, running EIO, emergency EROFS")
    # The enumeration's runs, classified by when the abort was set.
    stats = collections.Counter()
    sites = collections.Counter()
    early = late_zero = 0
    for seq, gaps, window, wait, certified, results, j in journal_runs(stats):
        sites.update(j.sites)
        if ("probe handle", "abort seen") in j.trace:
            early += 1
            assert results[-1] == "EROFS", \
                f"{marker} an abort before the probe's test was not returned as EROFS ({results})"
        if ("completed branch", True) in j.trace:
            late_zero += 1
            assert results[-1] == 0 and _late_abort(j), \
                f"{marker} the completed branch with an aborted journal ({results})"
    assert early and late_zero, \
        f"{marker} the completed-transaction return was never reached with the journal aborted " \
        f"({late_zero} runs), or no abort before the probe's test was seen ({early} runs)"
    assert not any(site in ("commit.c:775-778", "commit.c:883-886") for site, _ in sites), \
        f"{marker} a flush site outside the profile ran on the supported profile: {sites}"
    done.append(f"{early} runs with the abort before the probe's test returned EROFS; {late_zero} "
                f"reached the completed branch with the journal aborted and returned 0, each abort "
                f"after the probe's test; the commit.c:775-778 and 883-886 flushes never ran")
    # Discarded statuses on a volatile write-back cache, which R5 does not admit (design section
    # 5.8): on each profile that reaches the site, a failed flush leaves the abort flag clear, and
    # a committed operation is lost at power loss.
    for profile, site, events in (
            (("internal", "sync"), "checkpoint.c:338-339", ("d1", "bg-ok", "fail-next", "ckpt")),
            (("external", "sync"), "commit.c:775-778", ("d1", "bg-ok", "wb", "u", "fail-next", "bg-ok")),
            (("internal", "async"), "commit.c:883-886", ("d1", "fail-next", "bg-ok"))):
        j = JournalSim(*profile, device="volatile")
        for ev in events:
            if ev == "fail-next":
                j.flushes = ["fail"]
            else:
                journal_event(j, ev)
        assert (site, "fail") in j.sites, f"{marker} {site}: the site did not run"
        assert not j.aborted, f"{marker} {site}: a discarded flush status became an abort"
        assert j.committed("d1") and not j.survives("d1"), \
            f"{marker} {site}: a discarded flush failure lost nothing"
    done.append("on a volatile write-back cache, checkpoint.c:338-339, commit.c:775-778 and "
                "commit.c:883-886: a failed flush is discarded, the journal does not abort, and a "
                "committed operation is lost at power loss")
    # R4's counterexample, retained: a certified activation on a volatile write-back cache, then
    # a checkpoint whose flush fails; every result the store saw was 0.
    j = storage_path("volatile", "native", ("flushfail", "ckpt"))
    assert not j.aborted and not j.survives("d1"), \
        f"{marker} on a volatile cache a failed checkpoint flush after activation lost nothing"
    done.append("R4's counterexample, retained: on a volatile write-back cache, activation certified "
                "with every result 0, then a failed checkpoint flush; the dependency is lost at power "
                "loss and no process sees an error. Such storage is not admitted (C30)")
    # R5: every storage path of design section 5.8, kept separate, as its cited branches give it.
    for label, device, fua, events, aborts, kept in STORAGE_PATHS:
        j = storage_path(device, fua, events)
        assert j.aborted == aborts and j.survives("d1") == kept, \
            f"{marker} {label}: aborted {j.aborted}, kept {j.survives('d1')}; the cited branches give " \
            f"aborted {aborts}, kept {kept}"
    volatile = sum(1 for p in STORAGE_PATHS if p[1] == "volatile")
    done.append(f"{volatile} paths on a volatile cache, each with its own outcome: a discarded flush "
                f"failure loses history unaborted; a checked superblock failure aborts but its rewrite "
                f"can still carry the new tail; emulated FUA's later flush, or any later successful "
                f"flush, keeps it")
    # On admitted storage no flush is sent and no injected failure is consumed; a failed home
    # write aborts before the in-memory superblock takes the new tail, so the abort's errno write
    # carries the old one; after a failed tail write the errno write carries the new one; a write
    # in flight holds the tail.
    j = storage_path("stable", "native", ("flushfail", "ckpt"))
    assert j.flushes == ["fail"] and ("checkpoint.c:338-339", "not sent") in j.sites and \
        not any(outcome in ("ok", "fail") for (_, outcome) in j.sites), \
        f"{marker} a flush reached admitted storage: {dict(j.sites)}"
    j = storage_path("stable", "native", ("wbfail", "ckpt"))
    assert j.aborted and j.sb_mem == 1 and j.sb_disk == (1,) and j.mem_tail == 1, \
        f"{marker} a failed home write did not abort before the new tail (tails {j.sb_mem} {j.sb_disk})"
    j = storage_path("stable", "native", ("sbfail", "ckpt"))
    assert j.aborted and j.mem_tail == 1 and j.sb_mem > 1 and j.sb_disk == (j.sb_mem,), \
        f"{marker} after a failed tail write the abort's rewrite did not carry the new tail"
    j = storage_path("stable", "native", ("inflight", "ckpt"))
    assert not j.aborted and j.mem_tail == 1 and j.sb_disk == (1,), \
        f"{marker} the tail moved past a home write still in flight"
    done.append("on admitted storage: no flush sent and no injected failure consumed; a failed home "
                "write aborts before a new tail is set, so the abort writes the old one; a failed "
                "tail write aborts and the abort rewrites the new one; a write in flight holds the tail")
    # Checked statuses abort: a commit-record failure; the waiting sync returns
    # EIO; after the abort the tail does not move.
    j = JournalSim()
    assert journal_event(j, "d1")
    j.outcomes = ["io-error"]
    assert j.dir_fsync() == "EIO" and j.aborted and not j.committed("d1"), \
        f"{marker} a failed commit record did not abort the journal"
    assert journal_event(j, "d2") is False and j.emergency, \
        f"{marker} the next handle did not force emergency read-only"
    j = JournalSim()
    assert journal_event(j, "d1") and journal_event(j, "bg-ok") and j.checkpoint() == 0
    assert journal_event(j, "d2") and journal_event(j, "start")
    j.abort("a later failure")
    j.finish_commit()
    assert j.survives("d1"), f"{marker} an abort lost a committed, checkpointed operation"
    done.append("a failed commit record aborts and its waiting sync returns EIO; the next handle "
                "forces emergency read-only; an abort never moves the tail")
    return "; ".join(done)


# ===========================================================================
# 13c. R5: the stable-completion storage contract
#      (design sections 5.5 to 5.9, 6.4 step 12, 10.8, 13.2 and 13.3)
# ===========================================================================


def storage_path(device, fua, events):
    """A certified activation on the given storage (design section 10.7), then `events`."""
    j = JournalSim(device=device, fua=fua)
    assert journal_event(j, "d1") and journal_event(j, "d2"), "fixture: the dependencies"
    certified, results = journal_activation(j, (None, None, None, None), False)
    assert certified and results == [0, 0, 0, 0], f"fixture: no certified activation on {device}"
    for ev in events:
        journal_event(j, ev)
    return j


# Each path of design section 5.8, kept separate: (label, device, FUA, events after a certified
# activation, whether v6.17 aborts the journal, whether the dependency survives a power loss).
STORAGE_PATHS = [
    ("(a) volatile cache: the checkpoint flush fails, its status discarded; native FUA",
     "volatile", "native", ("flushfail", "ckpt"), False, False),
    ("(b) volatile cache: a failed flush, then the tail's superblock write fails; the abort "
     "rewrites the new tail", "volatile", "native", ("flushfail", "sbfail", "ckpt"), True, False),
    ("(b) volatile cache: a failed flush, then both superblock writes fail",
     "volatile", "native", ("flushfail", "sbfail2", "ckpt"), True, False),
    ("(c) volatile cache: native FUA after a flush that succeeded",
     "volatile", "native", ("ckpt",), False, True),
    ("(d) volatile cache, emulated FUA: the checkpoint flush fails, the flush after the superblock "
     "succeeds", "volatile", "emulated", ("flushfail", "ckpt"), False, True),
    ("(d) volatile cache, emulated FUA: that flush fails too; the abort's rewrite and its flush "
     "succeed", "volatile", "emulated", ("flush2fail", "ckpt"), True, True),
    ("(d) volatile cache, emulated FUA: every flush fails",
     "volatile", "emulated", ("flush3fail", "ckpt"), True, False),
    ("(e) volatile cache: a failed flush, then a later successful flush before the power loss",
     "volatile", "native", ("flushfail", "ckpt", "u2", "bg-ok"), False, True),
    ("admitted storage: R4's witness sequence; the flush is never sent",
     "stable", "native", ("flushfail", "ckpt"), False, True),
    ("admitted storage: a home write fails", "stable", "native", ("wbfail", "ckpt"), True, True),
    ("admitted storage: the tail's superblock write fails, the abort's rewrite succeeds",
     "stable", "native", ("sbfail", "ckpt"), True, True),
    ("admitted storage: both superblock writes fail", "stable", "native", ("sbfail2", "ckpt"), True,
     True),
    ("admitted storage: a home write still in flight", "stable", "native", ("inflight", "ckpt"),
     False, True),
    ("admitted storage: the write in flight completes, then a checkpoint",
     "stable", "native", ("inflight", "ckpt", "complete", "ckpt"), False, True),
]


def _storage_edit(**changes):
    """Edit the fixture host's storage attributes as sysfs would show them; None removes one."""
    def edit(m):
        ctrl_dir, disk_dir, _, _ = parse_storage_link(m.links[m.st_dev])
        for name, value in changes.items():
            if name in ("write_cache", "fua"):
                path = f"{disk_dir}/queue/{name}"
            elif name == "wwid":
                path = f"{disk_dir}/wwid"
            else:
                path = f"{ctrl_dir}/{name}"
            if value is None:
                m.sysfs_files.pop(path, None)
            else:
                m.sysfs_files[path] = value
    return edit


def _relink(link):
    """Put the fixture filesystem on another block device: /sys/dev/block's link, the
    effective-profile entries named after the device, and the attributes such a device has."""
    def edit(m):
        name = link.rsplit("/", 1)[-1]
        m.ext4_options = {name: list(m.ext4_options[device_name(m)])}
        m.jbd2 = {f"{name}-{JOURNAL_INODE}"}
        m.links[m.st_dev] = link
        m.sysfs_files = storage_files(link, FIXTURE_ATTRS)
    return edit


NOT_DIRECT = "storage not a direct NVMe namespace on PCI Express"

# Hosts that differ from the qualified storage in one fact, with the reason the opening must
# give. The first is the configuration the Architect's disposition names: queue/write_cache
# written to "write through" over a volatile cache.
STORAGE_CASES = [
    ("a volatile cache hidden by a write to queue/write_cache (write through, fua 1)",
     _storage_edit(write_cache="write through\n", fua="1\n"),
     "volatile write cache, flushes disabled in the kernel's view only"),
    ("a volatile write-back cache (write back, fua 1)",
     _storage_edit(write_cache="write back\n", fua="1\n"), "volatile write cache"),
    ("contradictory cache attributes (write back, fua 0)", _storage_edit(write_cache="write back\n"),
     "contradictory storage information"),
    ("the cache attributes unreadable", _storage_edit(fua=None), "storage unreadable"),
    ("a device-mapper device (LVM, dm-crypt, dm-cache)", _relink("../../devices/virtual/block/dm-1"),
     NOT_DIRECT),
    ("an NVMe multipath disk", _relink("../../devices/virtual/nvme-subsystem/nvme-subsys0/nvme0n1/"
                                       "nvme0n1p2"), NOT_DIRECT),
    ("NVMe over fabrics", _relink("../../devices/virtual/nvme-fabrics/ctl/nvme1/nvme1n1/nvme1n1p2"),
     NOT_DIRECT),
    ("a SATA or SAS disk", _relink("../../devices/pci0000:00/0000:00:17.0/ata1/host0/target0:0:0/"
                                   "0:0:0:0/block/sda/sda2"), NOT_DIRECT),
    ("a loop device", _relink("../../devices/virtual/block/loop0"), NOT_DIRECT),
    ("a virtual machine's virtio disk", _relink("../../devices/pci0000:00/0000:00:04.0/virtio1/block/"
                                                "vda/vda2"), NOT_DIRECT),
    ("a transport attribute other than pcie", _storage_edit(transport="tcp\n"), "storage transport"),
    ("another controller (serial)", _storage_edit(serial="FIXTURE-0000000002  \n"),
     "storage not qualified"),
    ("a firmware update", _storage_edit(firmware_rev="FW-0002 \n"), "storage not qualified"),
    ("another namespace (wwid)", _storage_edit(wwid="eui.0025380000000002\n"), "storage not qualified"),
    ("another PCI function", _relink(FIXTURE_LINK.replace(FIXTURE_PCI, "0000:3e:00.0")),
     "storage not qualified"),
    ("another partition", _relink(FIXTURE_LINK[:-1] + "3"), "storage not qualified"),
]


def _provision_without_storage(world, rename=None):
    """Replace PROVISION's content (fixture, root's act): R4's form, without the storage fields,
    or with one storage line rewritten. The digest is recomputed, so only the grammar decides."""
    inode = world.fs.inodes[world.provdir.ents_K["PROVISION"]]
    lines = bytes(inode.K).decode("ascii").splitlines()[:-1]
    if rename is None:
        lines = [line for line in lines if not line.startswith("storage-") or
                 line.startswith("storage-attestation=")]
    else:
        lines = [rename if line.startswith(rename.split("=")[0] + "=") else line for line in lines]
    body = ("\n".join(lines) + "\n").encode("ascii")
    data = body + f"digest={hashlib.sha256(body).hexdigest()}\n".encode("ascii")
    inode.K = bytearray(data)
    inode.D = bytearray(data)


def check_c30_storage_qualification():
    """[storage-qualification] Only admitted, qualified storage authorizes (design sections 5.5,
    5.9, 6.4 step 12 and 10.8). A qualified stable-completion fixture activates, claims,
    acknowledges and admits, completes a succession, keeps prior history discoverable and
    follows the existing disposition and retirement authority. Volatile, contradictory,
    unreadable, stacked, remote, virtual or changed storage, and a PROVISION without a storage
    qualification, refuse before any claim, with no admission. A stale or forged admission never
    authorizes, and decoding manufactures none. A loss of qualification refuses later openings
    and discards no custody or history."""
    marker = "[storage-qualification]"
    done = []
    world = World(n=2, c_pool=16)
    assert not qualify(world.mount) and not qualify_storage(world.mount), \
        f"{marker} the fixture's storage does not qualify"
    work = owner_work(world)
    rep = work["rep"]
    assert rep["state"] == "Ready" and rep["activated"] and work["tracked"] and \
        work["admitted"] == "Admitted" and \
        verify_admission(rep["storage"], rep["opening"], rep["selection"].prov_digest), \
        f"{marker} the qualified fixture did not activate, claim and admit ({rep['state']} {rep['reason']})"
    done.append("the qualified stable-completion fixture activated, claimed, acknowledged and admitted")
    world.fs.kill(work["proc"])
    again = startup(world, world.fs.register(Proc("next")), CFG, claim=True)
    assert again["state"] == "PriorUnresolved" and discoverable(again, work["tracked"]), \
        f"{marker} the prior generation was not in the decision ({again['state']})"
    before = _verify(world)
    binding = next(b for b, inc in before["incidents"].items()
                   if inc.get("generation") == work["tracked"]["generation"])
    in_session(world, lambda w, s: disposition_steps(w, _incident_fields(before, binding)))
    again = startup(world, world.fs.register(Proc("after")), CFG, claim=True)
    assert again["state"] == "Ready" and binding in again["dispositioned"], \
        f"{marker} a disposition did not take effect on admitted storage ({again['state']})"
    scenarios = admin_scenarios(n=1)
    retired = _clone_world(scenarios["P-RETIRE"][0])
    in_session(retired, scenarios["P-RETIRE"][1])
    after = _verify(retired)
    assert after["state"] == "Ready" and after["provision"]["retired"] == 1, \
        f"{marker} a retirement in a session failed on admitted storage ({after['state']})"
    succeeded = _completed_succession()
    work = owner_work(succeeded)
    assert work["rep"]["state"] == "Ready" and work["rep"]["selection"].state == "store-b" and \
        work["tracked"], f"{marker} no claim on a completed successor ({work['rep']['state']})"
    done.append("prior history discoverable; a disposition and a retirement took effect; an owner "
                "claimed on a completed successor")
    for label, edit, reason in STORAGE_CASES:
        world = World(n=1, c_pool=16)
        edit(world.mount)
        work = owner_work(world)
        rep = work["rep"]
        assert rep["state"] == "Unsupported" and rep["reason"] == reason and rep["claim"] is None \
            and rep["storage"] is None and work["tracked"] is None and not any(world.pool_inode(0).K), \
            f"{marker} {label}: the owner got {rep['state']}{' (' + rep['reason'] + ')' if rep['reason'] else ''}" \
            f", not a refusal before the claim with no admission"
        verified = startup(world, world.fs.register(Proc("verifier")), CFG, sync=False, verifier=True)
        assert verified["state"] == "Unsupported", f"{marker} {label}: the verifier got {verified['state']}"
        session = MaintenanceSession(world, world.admin)
        assert session.begin() == "Unsupported" and session.storage is None, \
            f"{marker} {label}: a session began"
    done.append(f"{len(STORAGE_CASES)} volatile, contradictory, unreadable, stacked, remote, virtual or "
                f"changed storage configurations refused before any claim by owners, the verifier and "
                f"sessions, with no admission made")
    for label, rename, problem in (
            ("no storage qualification (R4's PROVISION)", None, "PROVISION: expected storage-class"),
            ("a storage class other than nvme-pcie", "storage-class=sata", "PROVISION: storage class")):
        world = World(n=1, c_pool=16)
        _provision_without_storage(world, rename)
        work = owner_work(world)
        rep = work["rep"]
        assert rep["state"] == "Invalid" and rep["reason"] == problem and rep["claim"] is None, \
            f"{marker} {label}: {rep['state']} {rep['reason']}"
    done.append("a PROVISION without a storage qualification, or with another class, refused as Invalid")
    # Stale and forged admissions; decoding manufactures none.
    world = World(n=1, c_pool=16)
    first = startup(world, world.fs.register(Proc("first")), CFG)
    assert first["state"] == "Ready" and first["storage"] is not None, f"{marker} fixture: first opening"
    stale = first["storage"]
    forged = object.__new__(StorageAdmission)
    for name in StorageAdmission.__slots__:
        setattr(forged, name, getattr(stale, name))
    for label, token in (("an earlier opening's admission", stale), ("a forged admission", forged)):
        rep = startup(world, world.fs.register(Proc("claimant")), CFG, claim=True,
                      storage_hook=lambda w, r, token=token: r.__setitem__("storage", token))
        assert rep["state"] == "Unsupported" and rep["claim"] is None and not any(world.pool_inode(0).K), \
            f"{marker} {label} authorized a claim ({rep['state']})"
        world.fs.kill(world.fs._procs["claimant"])
    try:
        StorageAdmission(object(), 1, b"", {"identity": "", "pci": "", "partition": None})
        made = True
    except TypeError:
        made = False
    prov = world.provision_record()
    assert not made and not any(isinstance(v, StorageAdmission) for v in prov.values()), \
        f"{marker} an admission was made without the storage check, or decoded from PROVISION"
    done.append("an earlier opening's admission and a forged one refused at the claim; no admission "
                "from a constructor without the key or from decoded bytes")

    def flip(w, label):
        if label == "parent":
            _storage_edit(write_cache="write through\n", fua="1\n")(w.mount)
    world = World(n=1, c_pool=16)
    work = owner_work(world, activation_hook=flip)
    assert work["rep"]["state"] == "Unsupported" and work["rep"]["claim"] is None and \
        work["rep"]["storage"] is None, \
        f"{marker} storage changed during activation was certified ({work['rep']['state']})"
    world = World(n=1, c_pool=16)
    _storage_edit(write_cache="write back\n", fua="1\n")(world.mount)
    session = MaintenanceSession(world, world.admin, requalify=True)
    assert session.begin() == "Unsupported", f"{marker} re-qualification admitted a volatile cache"
    done.append("storage changed during activation refused at A4; re-qualification never admits a "
                "volatile cache")
    # The Owner's qualification: root-only facts the store uid cannot observe.
    for label, edit, problem in (
            ("a virtual machine guest", lambda m: setattr(m, "guest", True), "virtual machine guest"),
            ("a controller reporting a volatile write cache",
             lambda m: m.identify.update(vwc_present=True),
             "the controller reports a volatile write cache")):
        env = MountEnv()
        edit(env)
        assert problem in qualify_storage(env), f"{marker} qualification accepted {label}"
        assert check_storage(env, {"storage_class": STORAGE_CLASS}, pinned=False)[1] is None, \
            f"{marker} the model claims the store uid observes {label}"
    done.append("qualification refuses a virtual machine guest and a controller reporting a volatile "
                "cache; the store uid observes neither (design section 5.9)")
    # A loss of qualification: the running owner keeps its custody; later openings refuse; the
    # history stays; re-qualification brings the generation back into the decision.
    world = World(n=2, c_pool=16)
    work = owner_work(world)
    assert work["admitted"] == "Admitted", f"{marker} fixture: native work"
    guard = list(work["rep"]["guard"])
    _storage_edit(firmware_rev="FW-0002 \n")(world.mount)
    assert all(world.fs.holds(fd, "EX") for fd in guard) and work["exchange"].fatal is None, \
        f"{marker} a loss of qualification released the running owner's custody"
    index = work["tracked"]["index"]
    pool = bytes(world.pool_inode(index).K)
    world.fs.kill(work["proc"])
    rep = startup(world, world.fs.register(Proc("after-update")), CFG, claim=True)
    assert rep["state"] == "Unsupported" and rep["reason"] == "storage not qualified" and \
        rep["claim"] is None and bytes(world.pool_inode(index).K) == pool, \
        f"{marker} after a firmware update: {rep['state']} {rep['reason']}"
    assert MaintenanceSession(world, world.admin).begin() == "Unsupported", \
        f"{marker} a session began on storage that lost its qualification"
    session = MaintenanceSession(world, world.admin, requalify=True)
    assert session.begin() == "Active", f"{marker} re-qualification could not begin"
    run_steps(world, requalify_steps(world, session, world.mount.lines[0][3],
                                     storage=owner_storage_record(world.mount)))
    session.end()
    rep = startup(world, world.fs.register(Proc("requalified")), CFG, claim=True)
    assert rep["state"] == "PriorUnresolved" and discoverable(rep, work["tracked"]), \
        f"{marker} after re-qualification the generation is not in the decision ({rep['state']})"
    done.append("a firmware update outside any procedure: the running owner kept its custody, later "
                "openings and sessions refused with the history intact, and re-qualification brought "
                "the generation back into the decision")
    return "; ".join(done)


def _simfs_path(device, **checkpoint):
    """The whole model's checkpoint on one path (design section 5.8): a guaranteed entry, the
    checkpoint, a power loss. Returns whether the entry survived."""
    world = World(n=1, c_pool=16)
    fs = world.fs
    fs.device = device
    fs.mark_home_stable()
    fs.create(world.provdir, "dependency", "file", 0, 0, 0o444)
    fs.fsync_dir(world.provdir)
    later = checkpoint.pop("later_flush", False)
    fs.checkpoint(**checkpoint)
    if later:
        fs.create(world.provdir, "later", "file", 0, 0, 0o444)
        fs.fsync_dir(world.provdir)
    fs.power_loss()
    return "dependency" in world.provdir.ents_K


# The whole model's checkpoint paths and the JournalSim events they stand for.
PARITY_PATHS = [
    ("stable", {}, ("ckpt",)),
    ("stable", {"flush": "fail"}, ("flushfail", "ckpt")),
    ("stable", {"home": "fail"}, ("wbfail", "ckpt")),
    ("stable", {"home": "inflight"}, ("inflight", "ckpt")),
    ("stable", {"sb": "fail", "rewrite": "ok"}, ("sbfail", "ckpt")),
    ("stable", {"sb": "fail", "rewrite": "fail"}, ("sbfail2", "ckpt")),
    ("volatile", {}, ("ckpt",)),
    ("volatile", {"flush": "fail"}, ("flushfail", "ckpt")),
    ("volatile", {"flush": "fail", "sb": "fail", "rewrite": "ok"}, ("flushfail", "sbfail", "ckpt")),
    ("volatile", {"flush": "fail", "sb": "fail", "rewrite": "fail"}, ("flushfail", "sbfail2", "ckpt")),
    ("volatile", {"flush": "fail", "later_flush": True}, ("flushfail", "ckpt", "u2", "bg-ok")),
    ("unflushed", {}, ("ckpt",)),
]


def check_c31_stable_completion():
    """[stable-completion] On admitted storage no failure the storage reports and no write
    still in flight loses required history or becomes an acknowledgement (design sections 5.6
    and 5.7): a completed home write is stable, and a failed one stops the tail before its
    transaction; R4's witness sequence keeps the history; the whole model's checkpoint agrees
    with JournalSim on every path; and stable completion (A-S1) is shown necessary."""
    marker = "[stable-completion]"
    done = []
    admitted = [p for p in STORAGE_PATHS if p[1] == "stable"]
    for label, device, fua, events, _, _ in admitted:
        j = storage_path(device, fua, events)
        assert j.survives("d1") and j.survives("d2"), \
            f"{marker} {label}: a certified dependency lost at power loss on admitted storage"
    done.append(f"{len(admitted)} storage events after a certified activation on admitted storage, "
                f"R4's witness sequence included: every dependency kept at power loss")
    for k in range(1, 7):
        state = {}

        def on_ack(seq, outcome, k=k):
            if outcome == "Acknowledged":
                inode = state["run"].inode
                lo = (seq + 1) * BLOCK
                assert seq < k and bytes(inode.D[lo:lo + BLOCK]) == bytes(inode.K[lo:lo + BLOCK]), \
                    f"{marker} record {seq} acknowledged although the device reported the write of " \
                    f"record {k} failed"
        run = Run("T1", on_ack=on_ack, fail_sync_at=k, interleave=True)
        state["run"] = run
        run.play()
        fatal = run.exchange.fatal
        assert fatal is not None and fatal[0][0] == "Sync" and run.core.ledger.next_ack <= k, \
            f"{marker} a reported write failure of record {k} did not latch ({fatal})"
    done.append("a write the device reported failed, at each of six records: latched, never acknowledged")
    run = Run("T1", interleave=True)
    run.play()
    for snap in run.snapshots:
        for seq in range(1, snap["applied"] + 1):
            lo = (seq + 1) * BLOCK
            assert seq + 1 not in snap["pending"] and snap["D"][lo:lo + BLOCK] == snap["K"][lo:lo + BLOCK] \
                and any(snap["D"][lo:lo + BLOCK]), \
                f"{marker} at {snap['label']}: record {seq} acknowledged while its write was incomplete"
    done.append(f"{len(run.snapshots)} steps of a run: no record acknowledged before its write completed")
    for device, kwargs, events in PARITY_PATHS:
        whole = _simfs_path(device, **dict(kwargs))
        small = storage_path(device, "native", events).survives("d1")
        assert whole == small, \
            f"{marker} the whole model's checkpoint ({device}, {kwargs}) kept {whole}; JournalSim {small}"
    done.append(f"the whole model's checkpoint agrees with JournalSim on {len(PARITY_PATHS)} paths")
    # Stable completion is necessary (A-S1): a cache the kernel registered as absent receives no
    # flush, and an activated dependency is lost at a power loss with every result 0.
    j = storage_path("unflushed", "native", ("ckpt",))
    assert not j.aborted and not j.survives("d1"), \
        f"{marker} an unflushed cache lost nothing: stable completion would be unnecessary"
    world = witness_world("successor")
    world.fs.device = "unflushed"
    world.fs.mark_home_stable()
    work = owner_work(world)
    assert work["tracked"], f"{marker} fixture: the unflushed host's opening refused"
    world.fs.checkpoint()
    world.fs.power_loss(data="old")
    fresh = startup(world, world.fs.register(Proc("fresh")), CFG, claim=True)
    assert fresh["state"] == "Ready" and not discoverable(fresh, work["tracked"]), \
        f"{marker} on an unflushed cache the successor's generation survived ({fresh['state']})"
    done.append("necessity of A-S1: on a cache the kernel registered as absent, the opening admits "
                "what it observes, and after a checkpoint and a power loss the successor's "
                "acknowledged generation is lost with no error reported")
    return "; ".join(done)


def storage_prefixes():
    """C32's starting points: every operation prefix of first provisioning and of succession,
    each followed by process death (F1)."""
    scenarios = composed_scenarios()
    for name in ("P-PROV", "P-SUCCESSOR"):
        world, factory, use_session = scenarios[name]
        total = len(_steps_for(_clone_world(world), factory, use_session))
        for k in range(total + 1):
            prefix = _clone_world(world)
            run_steps(prefix, _steps_for(prefix, factory, use_session)[:k])
            prefix.fs.kill(prefix.admin)
            yield f"{name} prefix {k}", prefix


# Storage events between the dependent work and the second crash (design sections 5.6 and 5.8).
STORAGE_EVENTS = [
    ("a checkpoint; any flush sent fails", {"flush": "fail"}),
    ("a home write fails", {"home": "fail"}),
    ("the tail's superblock write fails, the abort's rewrite succeeds", {"sb": "fail", "rewrite": "ok"}),
    ("both superblock writes fail, the new tail on the medium",
     {"sb": "fail", "rewrite": "fail", "ambiguous": "new"}),
    ("a home write in flight", {"home": "inflight"}),
]

# Hosts: admitted storage, and storage the opening must refuse before any dependent work.
STORAGE_HOSTS = [
    ("admitted storage", None, "stable"),
    ("a volatile write-back cache", _storage_edit(write_cache="write back\n", fua="1\n"), "volatile"),
    ("a volatile cache hidden by queue/write_cache",
     _storage_edit(write_cache="write through\n", fua="1\n"), "unflushed"),
    ("a device-mapper device over a volatile cache", _relink("../../devices/virtual/block/dm-1"),
     "volatile"),
]

STORAGE_COMPOSED_STATS = {}


def check_c32_storage_composed():
    """[storage-composed] Composed: an administrative prefix and process death, an ordinary
    startup on each host, dependent work, a storage event, a second crash and the oracle. On
    admitted storage every acknowledged, unsealed generation stays discoverable or is refused
    explicitly; storage that is not admitted is refused before any dependent work; and a loss
    of qualification after dependent work refuses explicitly with the history retained."""
    marker = "[storage-composed]"
    stats = collections.Counter()
    lost_qualification = []
    for plabel, prefix in storage_prefixes():
        stats["prefixes"] += 1
        for hlabel, edit, device in STORAGE_HOSTS:
            base = _clone_world(prefix)
            if edit is not None:
                edit(base.mount)
            base.fs.device = device
            base.fs.mark_home_stable()
            work = owner_work(base)
            stats["openings"] += 1
            if work["tracked"] is None:
                stats[f"no dependent work: {hlabel}: {work['rep']['state']}"] += 1
                continue
            stats[f"runs with dependent work: {hlabel}"] += 1
            for elabel, event in STORAGE_EVENTS:
                world = _clone_world(base)
                world.fs.checkpoint(**dict(event))
                for crash in second_crashes(world):
                    x = _clone_world(world)
                    apply_crash(x, crash, [x.fs._procs["owner"]] if crash[0] == "F1" else [])
                    fresh, applied = oracle(
                        x, work["tracked"], f"{plabel}, {hlabel}, {elabel}, second {crash[0]} "
                        f"{crash[1] or ''} {crash[2] or ''}".strip(), marker)
                    stats["second crashes"] += 1
                    stats[f"after the second crash: {fresh['state']}"] += 1
                    for a in applied:
                        stats[f"recovery: {a}"] += 1
            if hlabel == "admitted storage" and plabel.startswith("P-SUCCESSOR"):
                lost_qualification.append((plabel, base, work))
    # A loss of qualification after dependent work: a firmware update outside any procedure,
    # then a second crash; the history must stay, refused explicitly.
    for plabel, base, work in lost_qualification:
        world = _clone_world(base)
        _storage_edit(firmware_rev="FW-0002 \n")(world.mount)
        for crash in second_crashes(world):
            x = _clone_world(world)
            apply_crash(x, crash, [x.fs._procs["owner"]] if crash[0] == "F1" else [])
            fresh, applied = oracle(x, work["tracked"], f"{plabel}, a firmware update, second "
                                    f"{crash[0]}", marker)
            assert fresh["state"] == "Unsupported", \
                f"{marker} {plabel}: storage that lost its qualification was used ({fresh['state']})"
            stats["loss of qualification: second crashes refused with the history kept"] += 1
    assert stats["runs with dependent work: admitted storage"], f"{marker} no dependent work on admitted storage"
    STORAGE_COMPOSED_STATS.clear()
    STORAGE_COMPOSED_STATS.update(stats)
    others = collections.Counter()
    for key, value in stats.items():
        if key.startswith("no dependent work") and not key.startswith("no dependent work: admitted"):
            others[key.rsplit(": ", 1)[1]] += value
    assert not any(k.startswith("runs with dependent work") and "admitted" not in k for k in stats), \
        f"{marker} dependent work on storage that is not admitted"
    return (f"{stats['prefixes']} administrative prefixes and process death, {stats['openings']} "
            f"openings on {len(STORAGE_HOSTS)} hosts: {stats['runs with dependent work: admitted storage']} "
            f"runs with dependent work on admitted storage, {stats['second crashes']} second crashes after "
            f"{len(STORAGE_EVENTS)} storage events, every acknowledged, unsealed generation discoverable "
            f"or explicitly refused; {stats['loss of qualification: second crashes refused with the history kept']} "
            f"second crashes after a loss of qualification, each refused with the history kept; on storage "
            f"that is not admitted, no dependent work: "
            + ", ".join(f"{n} {state}" for state, n in sorted(others.items())))


# ===========================================================================
# 14. The Architect's counterexamples, re-run on the unchanged R1 model
# ===========================================================================

R1_MODEL = pathlib.Path(__file__).resolve().parent.parent / "p2-v1-r3b-i3-p-r1" / "design_checks.py"
R1_MODEL_SHA256 = "17f606f2111aee1753b20874b2b16f15041abf4bae84178600ea83fb01705ac1"
_R1 = {}


def load_r1():
    if "module" not in _R1:
        data = R1_MODEL.read_bytes()
        if hashlib.sha256(data).hexdigest() != R1_MODEL_SHA256:
            raise RuntimeError("the R1 model is not the published R1 model")
        spec = importlib.util.spec_from_file_location("r1_design_checks", R1_MODEL)
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        _R1["module"] = module
    return _R1["module"]


def r1_admits(core, start_seq):
    """Source-derived admission predicate (core.rs:1639-1675): ledger not
    failed, start record acknowledged, no closure. R1 never closes admission."""
    return core.ledger.failed is None and core.ledger.next_ack > start_seq


def _r1_three_acked(r1):
    run = r1.Run("T1")
    for kind in (r1.RS(), r1.CS(), r1.AS()):
        run.core.issue(kind)
        run.core.flush(run.exchange)
        for _ in run.worker.steps():
            pass
        run.owner.apply()
    return run


def r1_reproductions():
    r1 = load_r1()
    out = []

    def add(rid, finding, reproduced, observed):
        out.append({"id": rid, "finding": finding, "reproduced": bool(reproduced), "observed": observed})

    run = _r1_three_acked(r1)
    run.worker.abort()
    run.owner.apply()
    add("R1-A", "F1", run.core.ledger.failed is None and r1_admits(run.core, 3),
        f"loss with 3 acknowledged and none issued: ledger.failed={run.core.ledger.failed}, "
        f"admission of the acknowledged start still permitted={r1_admits(run.core, 3)}")
    run = _r1_three_acked(r1)
    forged = r1.Record(r1.G_RUN, 1, 99, r1.RS(), r1.encode_record(r1.G_RUN, 1, 99, r1.RS())[-32:])
    run.exchange.submit(forged)
    run.owner.apply()
    add("R1-B", "F1", run.owner.failure_reported is None and run.owner.applied == 3,
        f"conflict on acknowledged sequence 1: latched {run.exchange.conflict}, failure never "
        f"delivered; R1 INV-4 (no sequence >= 1 acknowledged) contradicts 3 applied acknowledgements")
    for rid, rec in (("R1-C zero", r1.Record(r1.G_RUN, 0, 5, r1.RS(), bytes(32))),
                     ("R1-C out of range", r1.Record(r1.G_RUN, 17, 5, r1.RS(), bytes(32))),
                     ("R1-C foreign", r1.Record(bytes(16), 4, 5, r1.RS(), bytes(32)))):
        run = _r1_three_acked(r1)
        run.exchange.submit(rec)
        run.owner.apply()
        add(rid, "F1", run.owner.failure_reported is None and r1_admits(run.core, 3),
            f"latched {run.exchange.conflict}; never delivered; admission still permitted")
    run = _r1_three_acked(r1)
    future = r1.Record(r1.G_RUN, 7, 7, r1.ASET(), r1.encode_record(r1.G_RUN, 7, 7, r1.ASET())[-32:])
    run.exchange.submit(future)
    add("R1-C future", "F1", run.exchange.slots[6] is not None and not run.exchange.latched(),
        "a submission for unissued sequence 7 stored silently")
    run = _r1_three_acked(r1)
    for kind in (r1.ASET(), r1.CE(), r1.RE("Passed")):
        run.core.issue(kind)
    run.core.flush(run.exchange)
    bad = r1.Record(r1.G_RUN, 6, 50, r1.RE("Failed"), r1.encode_record(r1.G_RUN, 6, 50, r1.RE("Failed"))[-32:])
    run.exchange.submit(bad)
    for _ in run.worker.steps():
        pass
    run.owner.apply()
    add("R1-C beyond", "F1", run.core.ledger.failed is None and run.core.ledger.next_ack == 4,
        f"conflict at 6 with 3 applied: records 4-6 pending forever, failure never delivered "
        f"(ledger.failed={run.core.ledger.failed})")
    run = _r1_three_acked(r1)
    run.worker.abort()
    run.owner.apply()
    add("R1-D", "F1", r1_admits(run.core, 3),
        "ActionStarted acknowledged, reservation unadmitted, loss observed: admission permitted")
    run = r1.Run("T1")
    run.core.issue(r1.RS())
    run.core.flush(run.exchange)
    for _ in run.worker.steps():
        pass
    run.owner.apply()
    run.core.issue(r1.CS())
    run.core.flush(run.exchange)
    steps = run.worker.steps()
    next(steps)
    run.exchange.submit(forged)
    at_latch = run.exchange.durable_through
    for _ in steps:
        pass
    run.owner.apply()
    add("R1-E", "F1", run.exchange.durable_through > at_latch and run.owner.applied > at_latch,
        f"append in flight when a conflict latched: durable {at_latch} -> {run.exchange.durable_through}, "
        f"acknowledged {run.owner.applied}")
    w = r1.World(n=2, c_pool=16)
    p = w.fs.register(r1.Proc("owner"))
    rep = r1.startup(w, p, r1.CFG, claim=True)
    ex = r1.Exchange(rep["claim"].generation, 16)
    wk = r1.Worker(w, rep["claim"].io_fd, ex, rep["claim"].index)
    claim = wk.claim(rep["claim"].header)
    next(claim)
    wk.abort()
    run = r1.Run("T1").play()
    seal = run.worker.seal(r1.seal_for(run.header, run.core.intents))
    next(seal)
    run.worker.abort()
    add("R1-F", "F1", ex.claim == "None" and run.exchange.seal == "None",
        f"loss during the claim leaves claim state {ex.claim!r}; during the seal, seal state "
        f"{run.exchange.seal!r}: neither failed nor completed")
    # F2: maintenance holds the store lock; the standalone verifier is Busy.
    w = r1.World(n=2, c_pool=16)
    maint = w.fs.register(r1.Proc("maint"))
    fd = w.fs.open(maint, "main", w.store_root(), "LOCK")
    w.fs.flock(fd, "EX")
    busy = r1._verify(w)["state"]
    w2 = r1._unresolved_world()
    mark = len(w2.fs.log)
    before = r1._verify(w2)
    b = next(iter(before["blocking"]))
    for step in r1.disposition_steps(w2, r1._incident_fields(before, b)):
        step()
    locked = any(e[0] == "root" and e[1].startswith("flock") for e in w2.fs.log[mark:])
    add("R1-F2", "F2", busy == "Busy" and not locked,
        f"verifier inside maintenance: {busy}; R1 model procedures took a store lock: {locked}")
    # F3: startup reads A, pauses before locking; succession publishes B.
    w = r1.World(n=2, c_pool=16)
    original = r1.check_mount
    state = {}

    def interleave(env, prov):
        if not state:
            state["done"] = True
            lock = w.fs.open(w.admin, "main", w.store_root(), "LOCK")
            w.fs.flock(lock, "EX")
            for step in r1.provision_steps(w, bytes(range(0x20, 0x30)), "store-b",
                                           predecessor=r1.ROOT_ID, statement="successor"):
                step()
            w.fs.close(lock)
        return original(env, prov)

    r1.check_mount = interleave
    try:
        stale = r1.startup(w, w.fs.register(r1.Proc("stale")), r1.CFG, claim=True)
    finally:
        r1.check_mount = original
    fresh = r1.startup(w, w.fs.register(r1.Proc("fresh")), r1.CFG, claim=True)
    add("R1-F3", "F3", stale["state"] == fresh["state"] == "Ready" and
        stale["provision"]["root_id"] != fresh["provision"]["root_id"],
        f"stale startup {stale['state']} on {stale['provision']['root_id'].hex()[:8]}, fresh startup "
        f"{fresh['state']} on {fresh['provision']['root_id'].hex()[:8]}: two owners")
    # F4: durability only through fsync_dir; hidden syncs; eviction coupling.
    w = r1._unresolved_world()
    before = r1._verify(w)
    b = next(iter(before["blocking"]))
    steps = r1.disposition_steps(w, r1._incident_fields(before, b))
    for step in steps[:6]:
        step()
    w.fs.kill(w.admin)
    w.fs.power_loss()
    after = r1._verify(w)
    add("R1-F4a", "F4", not after["dispositioned"],
        "after the link and the unlink, before the directory sync, F2 has one outcome (absent): "
        "a link already committed is not representable")
    w = r1.World(n=2, c_pool=16, provision=False)
    calls = []
    original_sync = w.fs.fsync_dir

    def traced(d):
        calls.append(w.fs._name_of(d))
        return original_sync(d)

    w.fs.fsync_dir = traced
    for step in r1.provision_steps(w, r1.ROOT_ID, "store-a"):
        mark = len(calls)
        step()
        if step.__name__ == "make_lock":
            lock_syncs = calls[mark:]
    pool = r1.World(n=2, c_pool=16).pool_inode(0)
    add("R1-F4b", "F4", lock_syncs == ["store-a"] and bytes(pool.D) == bytes(pool.K),
        f"make_lock synced {lock_syncs} (not in the R1 text); pool files durable at creation "
        "without a sync step")
    run = r1.Run("T1")
    run.core.issue(r1.RS())
    run.core.flush(run.exchange)
    steps = run.worker.steps()
    next(steps)
    run.world.fs.wb_fail.add((run.inode.ino, 2))
    for _ in steps:
        pass
    add("R1-F4c", "F4", not run.world.fs.evict(run.inode),
        "clean pages cannot be reclaimed while the writer's descriptor is open")
    # Aggregate bounds: every disposition and revoked entry is processed.
    w = r1.World(n=2, c_pool=16)
    d = w.dir("dispositions")
    for i in range(1, 5001):
        binding = r1.bind_gap(r1.ROOT_ID, 10 ** 6 + i)
        fields = {"root": r1.ROOT_ID, "binding": binding, "kind": "claim-gap", "claim": 10 ** 6 + i,
                  "generation": None, "pool_index": None, "content": None, "class": "malformed",
                  "unsettled": 0, "reason": "other", "statement": "orphan", "operator": "owner",
                  "at": "2026-10-01T12:00:00Z"}
        inode = w.fs.create(d, binding.hex() + ".disposition", "file", 0, 0, 0o444)
        inode.K = bytearray(r1.disposition_text(fields))
        inode.D = bytearray(inode.K)
    mark = len(w.fs.log)
    rep = r1._verify(w)
    reads = sum(1 for e in w.fs.log[mark:] if e[1] == "pread" and e[2].endswith(".disposition"))
    w = r1.World(n=2, c_pool=16)
    rv = w.dir("dispositions", "revoked")
    for i in range(5000):
        w.fs.create(rv, f"{i:064x}-20261001T120000Z.disposition", "file", 0, 0, 0o444)
    mark = len(w.fs.log)
    rep2 = r1._verify(w)
    stats = sum(1 for e in w.fs.log[mark:] if e[1] == "fstatat" and e[2].endswith("Z.disposition"))
    add("R1-S8", "8", reads == 5000 and stats == 5000 and rep["state"] == rep2["state"] == "Ready",
        f"5000 orphan dispositions all read ({reads}); 5000 revoked entries all examined ({stats}); "
        "no aggregate bound")
    return out


# ===========================================================================
# 14b. The Architect's composed counterexample, re-run on the unchanged R2 model
# ===========================================================================

R2_MODEL = pathlib.Path(__file__).resolve().parent.parent / "p2-v1-r3b-i3-p-r2" / "design_checks.py"
R2_MODEL_SHA256 = "53b8dbb5da194d62da35178dd06c2c8f38dc0fb6c39076552fb74cb1a54934bc"
_R2 = {}


def load_r2():
    if "module" not in _R2:
        data = R2_MODEL.read_bytes()
        if hashlib.sha256(data).hexdigest() != R2_MODEL_SHA256:
            raise RuntimeError("the R2 model is not the published R2 model")
        spec = importlib.util.spec_from_file_location("r2_design_checks", R2_MODEL)
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        module.MUT.active = frozenset()
        _R2["module"] = module
    return _R2["module"]


def _r2_dependent_work(r2, world, rep):
    """A real claim and RunStarted, CaseStarted, ActionStarted, each made durable
    and acknowledged, then an admission, with the R2 model's own classes."""
    events = []
    claim = rep["claim"]
    core = r2.CoreSim(r2.G_RUN, events)
    exchange = r2.Exchange(r2.G_RUN, r2.CFG.record_capacity, events)
    worker = r2.Worker(world, claim.io_fd, exchange, claim.index)
    owner = r2.Owner(core, exchange, worker=worker)
    gate = r2.AdmissionGate(exchange)
    for _ in worker.claim(claim.header):
        pass
    if exchange.claim != "Claimed":
        return {"claim": exchange.claim, "fatal": exchange.fatal}

    def pump():
        core.flush(owner)
        for _ in worker.steps():
            pass
        owner.apply()

    core.start_run()
    pump()
    core.begin_case()
    pump()
    action = core.reserve()
    pump()
    admitted = gate.admit(core, action)
    if admitted == "Admitted":
        core.start_native(action)
    return {"claim": exchange.claim, "acknowledged": core.ledger.next_ack - 1, "admitted": admitted,
            "generation": claim.generation, "index": claim.index}


def _r2_every_f2(r2, world, state, index, generation):
    """Every F2 outcome of both families, then a fresh R2 startup: counts of
    (family, state, selected root, generation in the decision)."""
    counts = collections.Counter()
    for family in ("ordered", "perdir"):
        for schedule in world.fs.schedules(family):
            for data in ("old", "new"):
                x = r2._clone_world(world)
                x.fs.power_loss(schedule, data=data)
                fresh = r2.startup(x, x.fs.register(r2.Proc("fresh")), r2.CFG, claim=True)
                sel = fresh.get("selection")
                found = any(inc.get("generation") == generation for inc in fresh["incidents"].values())
                counts[(family, fresh["state"], sel.state if sel else None, found)] += 1
    return counts


def r2_reproductions():
    r2 = load_r2()
    out = []

    def add(rid, finding, reproduced, observed):
        out.append({"id": rid, "finding": finding, "reproduced": bool(reproduced), "observed": observed})

    # Witness: successor B visible (operation 28 of 29), F1, ordinary startup on B.
    world = r2.World(n=1, c_pool=16)
    session = r2.MaintenanceSession(world, world.admin)
    session.begin()
    r2.run_steps(world, r2.successor_steps(world, session, r2.SUCCESSOR_ID, "store-b")[:28])
    world.fs.kill(world.admin)
    pending = [(h.op, h.name, h.add) for h in world.fs.pending_meta]
    as_is = r2._clone_world(world)
    rep = r2.startup(as_is, as_is.fs.register(r2.Proc("owner")), r2.CFG, claim=True, generation=r2.G_RUN)
    work = _r2_dependent_work(r2, as_is, rep)
    add("R2-W-ASIS", "composed (analysis)", rep["state"] == "Ready" and work["claim"] == "Failed",
        f"unchanged R2 model as is: startup {rep['state']} on {rep['selection'].state}, but the claim "
        f"{work['claim']} ({work.get('fatal')}): its Worker rechecks identity (W5) through World.state = "
        f"{as_is.state!r}, the first root. Design W5 and section 8.1 use the opened root's retained "
        f"journals descriptor, so this is an unmodelled transition, not a design guard")
    rep = r2.startup(world, world.fs.register(r2.Proc("owner")), r2.CFG, claim=True, generation=r2.G_RUN)
    world.state = rep["selection"].state       # bind the fixture's default root to the opened root (W5)
    work = _r2_dependent_work(r2, world, rep)
    counts = _r2_every_f2(r2, world, "store-b", work["index"], work["generation"])
    omitted = sum(n for (fam, state, root, found), n in counts.items() if state == "Ready" and not found)
    total = sum(counts.values())
    b = r2.classify(bytes(world.pool_inode(work["index"], state="store-b").K), r2.SUCCESSOR_ID, 16,
                    work["index"])
    add("R2-W", "composed", work["acknowledged"] == 3 and omitted > 0,
        f"pending before the second crash: {pending}; startup Ready on store-b; acknowledged "
        f"{work['acknowledged']}, admission {work['admitted']}; B journal {b['class']}; after F2 a fresh "
        f"startup was Ready on store-a with B's generation outside the decision in {omitted} of {total} "
        f"outcomes {dict((str(k), v) for k, v in sorted(counts.items(), key=str))}")
    # Initial provisioning: PROVISION visible (operation 22 of 23), F1, dependent work.
    world = r2.World(n=1, c_pool=16, provision=False)
    r2.run_steps(world, r2.provision_steps(world, r2.ROOT_ID, "store-a")[:22])
    world.fs.kill(world.admin)
    rep = r2.startup(world, world.fs.register(r2.Proc("owner")), r2.CFG, claim=True, generation=r2.G_RUN)
    work = _r2_dependent_work(r2, world, rep)
    counts = _r2_every_f2(r2, world, "store-a", work["index"], work["generation"])
    unprovisioned = sum(n for (fam, state, root, found), n in counts.items() if state == "Unprovisioned")
    x = r2._clone_world(world)
    x.fs.power_loss(world.fs.schedules("ordered")[0], data="old")
    fresh = r2.startup(x, x.fs.register(r2.Proc("fresh")), r2.CFG, claim=True)
    a = r2.classify(bytes(x.pool_inode(work["index"], state="store-a").K), r2.ROOT_ID, 16, work["index"])
    r2.run_steps(x, r2.provision_steps(x, bytes(range(0x30, 0x40)), "store-c"))
    again = r2.startup(x, x.fs.register(r2.Proc("again")), r2.CFG, claim=True)
    add("R2-P", "composed", fresh["state"] == "Unprovisioned" and a["refused"] and again["state"] == "Ready"
        and not again["incidents"],
        f"after F2 a fresh startup is Unprovisioned in {unprovisioned} of {sum(counts.values())} outcomes "
        f"(fail closed), yet the unpublished root's journal is {a['class']}: R2 section 15.2's recovery "
        f"premise ('the incomplete root holds no history') is false; following it with the R2 "
        f"provisioning helper gives {again['state']} on {again['selection'].state} with no incident")
    return out


# ===========================================================================
# 14b. The R4 findings, re-run on the unchanged R3 model
# ===========================================================================

R3_MODEL = pathlib.Path(__file__).resolve().parent.parent / "p2-v1-r3b-i3-p-r3" / "design_checks.py"
R3_MODEL_SHA256 = "31afe335be8faaeb7a02d31ce5463ea9f7a256f5d49e6af40ebfbfbbbbd8b678"
_R3 = {}


def load_r3():
    if "module" not in _R3:
        data = R3_MODEL.read_bytes()
        if hashlib.sha256(data).hexdigest() != R3_MODEL_SHA256:
            raise RuntimeError("the R3 model is not the published R3 model")
        spec = importlib.util.spec_from_file_location("r3_design_checks", R3_MODEL)
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        module.MUT.active = frozenset()
        _R3["module"] = module
    return _R3["module"]


def r3_reproductions():
    r3 = load_r3()
    out = []

    def add(rid, finding, reproduced, observed):
        out.append({"id": rid, "finding": finding, "reproduced": bool(reproduced), "observed": observed})

    # F4: the probe's fsync after its transaction completed and a later commit failed.
    world = r3.World(n=1, c_pool=16)
    fs = world.fs
    fd = fs.open(fs.register(r3.Proc("prober")), "main", world.store_root(), "LOCK")
    touched = fs.touch(fd)
    states = sorted(k for k in vars(fs) if "probe" in k)
    fs.abort_journal()          # R3 has no state for "the probe's transaction completed"
    r3_result = fs.fsync_file(fd)
    w4 = World(n=1, c_pool=16)
    fd4 = w4.fs.open(w4.fs.register(Proc("prober")), "main", w4.store_root(), "LOCK")
    w4.fs.touch(fd4)
    w4.fs.commit_probe()
    w4.fs.abort_journal()
    r4_result = w4.fs.fsync_file(fd4)
    add("R3-F4", "F4", touched == 0 and r3_result == "EIO" and r4_result == 0,
        f"R3's probe state is {states}: no running/completed distinction. After the probe's "
        f"transaction completed and a later commit aborted the journal, R3's fsync_file returns "
        f"{r3_result}; jbd2_complete_transaction returns 0 for a completed tid without testing the "
        f"abort flag (journal.c:800-805), as the R4 model does ({r4_result}). R3's touch() has no "
        f"abort landing between ext4's and jbd2's handle tests either")
    # Did any R3 conclusion depend on that branch? Count it over every R3 check.
    original = r3.SimFS.fsync_file
    hits = collections.Counter()

    def counted(self, fd, read_only=False):
        hits["calls"] += 1
        if not self.emergency_ro and not read_only and self.journal_aborted:
            hits["journal-aborted branch"] += 1
        return original(self, fd, read_only)
    r3.SimFS.fsync_file = counted
    results = {}
    try:
        for check in r3.CHECKS:
            r3.MUT.active = frozenset()
            results[check[0]] = r3.run_check(check)[0]
    finally:
        r3.SimFS.fsync_file = original
        r3.MUT.active = frozenset()
    add("R3-F4-REACH", "F4 (scope)",
        all(s == "PASS" for s in results.values()) and hits["calls"] > 0
        and hits["journal-aborted branch"] == 0,
        f"over R3's 27 checks ({sum(s == 'PASS' for s in results.values())} pass), R3's fsync_file ran "
        f"{hits['calls']} times and took the inaccurate aborted-journal branch "
        f"{hits['journal-aborted branch']} times: no R3 check result depended on it")
    # Profile: R3 decides from the pinned mountinfo strings alone.
    world = r3.World(n=1, c_pool=16)
    rep = r3.startup(world, world.fs.register(r3.Proc("owner")), r3.CFG, claim=True)
    observed = sorted(vars(world.mount))
    w4 = World(n=1, c_pool=16)
    _swap_option("barrier", "nobarrier")(w4.mount)
    rep4 = startup(w4, w4.fs.register(Proc("owner")), CFG, claim=True)
    w5 = World(n=1, c_pool=16)
    next(edit for label, edit, _ in PROFILE_CASES if label == "an external journal")(w5.mount)
    rep5 = startup(w5, w5.fs.register(Proc("owner")), CFG, claim=True)
    add("R3-PROFILE", "section 5 (profile)",
        rep["state"] == "Ready" and rep["claim"] is not None and "ext4_options" not in observed
        and rep4["state"] == rep5["state"] == "Unsupported",
        f"R3's host model observes only {observed}: its opening decides the profile from the pinned "
        f"mountinfo strings {world.mount.lines[0][3:]}, which omit the superblock's defaults "
        f"(super.c:2949-2951), and has no journal location or kernel identity (R3: {rep['state']} with "
        f"a claim). On the same mountinfo the R4 opening refuses an effective nobarrier "
        f"({rep4['state']}: {rep4['reason']}) and an external journal ({rep5['state']}: {rep5['reason']})")
    return out


# ===========================================================================
# 14c. The R5 findings, re-run on the unchanged R4 model
# ===========================================================================

R4_MODEL = pathlib.Path(__file__).resolve().parent.parent / "p2-v1-r3b-i3-p-r4" / "design_checks.py"
R4_MODEL_SHA256 = "f25d2a79fe1192befae21203017eb8b22d6495778c7aab88697836f2812af9ce"
_R4 = {}


def load_r4():
    if "module" not in _R4:
        data = R4_MODEL.read_bytes()
        if hashlib.sha256(data).hexdigest() != R4_MODEL_SHA256:
            raise RuntimeError("the R4 model is not the published R4 model")
        spec = importlib.util.spec_from_file_location("r4_design_checks", R4_MODEL)
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        module.MUT.active = frozenset()
        _R4["module"] = module
    return _R4["module"]


def r4_reproductions():
    r4 = load_r4()
    out = []

    def add(rid, finding, reproduced, observed):
        out.append({"id": rid, "finding": finding, "reproduced": bool(reproduced), "observed": observed})

    # The checkpoint witness: a certified activation, then a failed checkpoint flush.
    j = r4.JournalSim()
    assert r4.journal_event(j, "d1") and r4.journal_event(j, "d2")
    certified, results = r4.journal_activation(j, (None, None, None, None), False)
    j.flushes = ["fail"]
    j.checkpoint()
    sites = dict(j.sites)
    txn = j.txn_of("d1")
    witness = certified and results == [0, 0, 0, 0] and not j.aborted and not j.survives("d1")
    k = r4.JournalSim()
    r4.journal_event(k, "d1")
    r4.journal_event(k, "d2")
    r4.journal_activation(k, (None, None, None, None), False)
    k.flushes = ["fail"]
    k.checkpoint()
    r4.journal_event(k, "u2")
    r4.journal_event(k, "bg-ok")
    add("R4-F2-WITNESS", "F2 (A-S5)", witness and k.survives("d1"),
        f"on the R4 model, activation certified with results {results}; after a checkpoint whose flush "
        f"fails (sites {sites}) the journal is not aborted ({j.aborted}) and d1 survives a power loss: "
        f"{j.survives('d1')} (its transaction: committed, in the log {txn['in_log']}, home "
        f"{txn['home']!r}, log durable {txn['log_durable']}). A later commit's successful flush before "
        f"the power loss keeps it ({k.survives('d1')}). The witness is conditional: it holds for a "
        f"volatile write-back cache that fails that flush and loses power before a later successful "
        f"flush; it was derived from the sources and the model, not observed live")
    # What R4's journal model does not represent.
    attrs = set(vars(r4.JournalSim()))
    missing = [name for name in ("device", "fua", "sb_plan", "home_plan", "dev_err") if name not in attrs]
    add("R4-PATHS", "F2 (scope)", len(missing) == 5 and "dev_err" not in attrs,
        f"R4's JournalSim has none of {missing}: it models neither the superblock's tail write and its "
        f"failure, nor native or emulated FUA, nor a failed or incomplete home write, nor a device "
        f"without a write cache. Its _tail_update moves the tail at once, so the abort's rewrite of the "
        f"new tail (journal.c:2592-2595) and emulated FUA's later flush are outside it. R5's JournalSim "
        f"adds them (design section 5.8)")
    # R4's fixture host: a device-mapper device, which R5 refuses as stacked storage.
    world = r4.World(n=1, c_pool=16)
    rep = r4.startup(world, world.fs.register(r4.Proc("owner")), r4.CFG, claim=True)
    fixture = dict(world.mount.sysfs)
    w5 = World(n=1, c_pool=16)
    _relink("../../devices/virtual/block/" + next(iter(fixture.values())))(w5.mount)
    rep5 = startup(w5, w5.fs.register(Proc("owner")), CFG, claim=True)
    add("R4-FIXTURE", "storage (scope)",
        rep["state"] == "Ready" and rep["claim"] is not None and rep5["state"] == "Unsupported"
        and rep5["reason"] == NOT_DIRECT,
        f"R4's fixture host is the device {fixture}, a device-mapper device; R4's opening was Ready with a "
        f"claim on it and observed nothing below the filesystem. On that device R5's opening refuses "
        f"({rep5['state']}: {rep5['reason']}). R5's fixture is a partition of an NVMe namespace")
    return out


# ===========================================================================
# 15. Check and control tables, assumptions, harness
# ===========================================================================

CHECKS = [
    ("C00", "[golden-codec]", check_c00_golden, "D-9", [], [], []),
    ("C01", "[F1-volatile]", check_c01_f1_volatile, "R1", [1], [], []),
    ("C02", "[F1-F2-sync]", check_c02_f1_f2_sync, "R1", [2], [], []),
    ("C03", "[errseq-reopen]", check_c03_errseq_reopen, "R1", [3], [], []),
    ("C04", "[tear-containment]", check_c04_tear_containment, "R2, D-10", [4], [], []),
    ("C05", "[lock-retained]", check_c05_lock_retained, "R3", [5], [], []),
    ("C06", "[busy-before-sync]", check_c06_busy_before_sync, "R4", [6], [], []),
    ("C07", "[dup-bounded]", check_c07_dup_bounded, "R5", [7], [], []),
    ("C08", "[fatal-latched]", check_c08_fatal_latched, "R5, D-5", [8], [], []),
    ("C09", "[late-uncertain]", check_c09_late_uncertain, "R6, D-3", [9], [], []),
    ("C10", "[exact-bytes]", check_c10_exact_bytes, "R7", [10], [], []),
    ("C11", "[archive-binding]", check_c11_archive_binding, "R9", [11], [], []),
    ("C12", "[admin-crash]", check_c12_admin_crash, "R9, D-7, D-8, F4", [12], [12], [10]),
    ("C13", "[grammar-conformance]", check_c13_grammar_conformance, "R6", [], [], []),
    ("C14", "[safe-open]", check_c14_safe_open, "R4, R8, D-6", [], [], []),
    ("C15", "[ack-durable]", check_c15_ack_durable, "R1, R5, F1", [], [], []),
    ("C16", "[no-false-resolution]", check_c16_no_false_resolution, "R6, D-3", [], [], []),
    ("C17", "[arith-bounds]", check_c17_arith_bounds, "R7, D-2", [], [], []),
    ("C18", "[fatal-total]", check_c18_fatal_total, "F1", [], [1, 2, 3, 4, 6], []),
    ("C19", "[admission-fence]", check_c19_admission_fence, "F1", [], [5], []),
    ("C20", "[session-verify]", check_c20_session_verify, "F2", [], [7, 8], []),
    ("C21", "[provision-selection]", check_c21_provision_selection, "F3", [], [9, 10], []),
    ("C22", "[metadata-schedules]", check_c22_metadata_schedules, "F4", [], [11, 12], [7]),
    ("C23", "[step-parity]", check_c23_step_parity, "F4", [], [13], []),
    ("C24", "[aggregate-bounds]", check_c24_aggregate_bounds, "Section 8", [], [14], []),
    ("C25", "[durable-activation]", check_c25_durable_activation, "R3 section 4", [], [],
     [1, 2, 3, 4, 5, 6, 8, 12, 13]),
    ("C26", "[composed-recovery]", check_c26_composed_recovery, "R3 sections 5, 6", [], [],
     [1, 2, 7, 8, 9, 10, 11]),
    ("C27", "[supported-profile]", check_c27_supported_profile, "R4 sections 5, 7", [], [], []),
    ("C28", "[activation-proof]", check_c28_activation_proof, "R4 sections 6, 7", [], [], []),
    ("C29", "[journal-conformance]", check_c29_journal_conformance, "R4 sections 4, 7", [], [], []),
    ("C30", "[storage-qualification]", check_c30_storage_qualification, "R5 sections 5, 6, 7", [], [],
     []),
    ("C31", "[stable-completion]", check_c31_stable_completion, "R5 sections 4, 5, 7, 8", [], [], []),
    ("C32", "[storage-composed]", check_c32_storage_composed, "R5 sections 7, 8", [], [], []),
]

# R4 mission items exercised by each check (the R1, R2 and R3 items stay in the
# tuples above).
R4_CHECK_ITEMS = {"C25": [4, 6], "C26": [11], "C27": [1, 2, 3, 4], "C28": [5, 6, 7, 10],
                  "C29": [8, 9, 10]}

# R5 mission items exercised by each check.
R5_CHECK_ITEMS = {"C26": [10, 11], "C28": [8, 11], "C29": [9, 11], "C30": [1, 2, 3, 4, 7],
                  "C31": [5, 6, 8, 12], "C32": [3, 7, 10]}

R5_MISSION_ITEMS = {
    1: "a qualified stable-completion fixture activates; ordinary claims, records and acknowledgements work",
    2: "completed succession works; prior unresolved history stays discoverable; disposition and "
       "retirement follow the existing authority",
    3: "volatile-completion storage is not admitted; unknown or contradictory storage refuses before any "
       "claim; an unsupported configuration gets no activation token",
    4: "a spoofed or stale qualification cannot authorize; decoding bytes manufactures no verified "
       "qualification",
    5: "a reported write failure never becomes an acknowledgement",
    6: "an incomplete write never becomes a durable record",
    7: "a loss of qualification discards no existing native custody or history",
    8: "on admitted storage R4's witness cannot lose required history: the completed home writes are "
       "stable, or a failure prevents the discard",
    9: "on a volatile cache the paths stay separate (a discarded flush, a checked superblock failure, "
       "native FUA, emulated FUA, a later flush); no abort invented; R4's witness retained",
    10: "composed administrative, process-death, startup, dependent-work and second-crash scenarios with "
        "storage events",
    11: "R4's 30 checks and 61 controls retained; each domain change documented",
    12: "stable completion shown necessary: a volatile cache the kernel registered as absent loses history "
        "undetected",
}

R2_MISSION_ITEMS = {
    1: "worker loss with no pending or unissued record",
    2: "loss between durable reservation and admission",
    3: "an acknowledged-sequence conflict",
    4: "zero, foreign, future and out-of-range submissions",
    5: "fatal signal versus publication and admission ordering",
    6: "unexpected acknowledgement outcomes",
    7: "lock-aware verification without lock conversion or reacquisition",
    8: "exclusion of independent competitors during maintenance",
    9: "PROVISION replacement between read and lock",
    10: "stale predecessor startup after successor publication",
    11: "metadata persistence before explicit directory sync",
    12: "administrative fault points with the corrected schedules",
    13: "protocol and model durability-step parity",
    14: "bounded aggregate scanning and reporting",
}

R3_MISSION_ITEMS = {
    1: "the root-changing witness: a successor visible but not durable, dependent work, power loss",
    2: "the initial-provisioning variant; an absent selection fails closed and never permits a fresh replacement",
    3: "successful activation of fully established stores and completed transitions",
    4: "activation failure before any claim: EIO, EINTR, emergency read-only, read-only, identity",
    5: "failure between persistence steps: process death or power loss during activation",
    6: "stale readers and competing owners",
    7: "persistence before an explicit sync and after it, under the ordered and per-directory families",
    8: "persistence domains: one filesystem required and verified; independent directories over-approximated",
    9: "a second crash after dependent records became durable",
    10: "safe conservative refusals and their recoveries",
    11: "every administrative procedure composed with dependent work",
    12: "a silent journal commit failure before activation (the certification probe)",
    13: "the read-only verifier never activates; a maintenance session activates before verifying",
}

R4_MISSION_ITEMS = {
    1: "the supported profile (internal journal, data=ordered, barriers, no fast or asynchronous commits, "
       "normal journal loading); each unsupported profile refused before the claim",
    2: "the profile established from the effective option listing, the journal's location and the "
       "qualified kernel; never from an absent string, a version prefix or an attestation",
    3: "unknown, contradictory or unqualified profile information refused; an unqualified profile "
       "cannot authorize from a claimed feature string",
    4: "a qualified profile activates; established stores claim and work; completed successions work",
    5: "activation's dependencies established durable across the transaction windows: running, "
       "committing or completed; an abort before, inside or after the probe's handle start",
    6: "a failed dependency persistence never becomes activation, even behind later successes",
    7: "a harmless late error is not mislabelled as lost history",
    8: "the completed-transaction return of jbd2_complete_transaction represented",
    9: "discarded flush statuses never replaced by invented aborts; the checkpoint flush shown to need A-S5",
    10: "checkpoint and log-tail interactions within the declared bounds",
    11: "every R3 composed scenario preserves discoverability or refuses",
}

NEGATIVE_CONTROLS = [
    ("NC01", "C01", "process death makes kernel-visible bytes durable (first candidate)"),
    ("NC02", "C02", "startup reports evidence preserved without syncing"),
    ("NC03", "C03", "a new description's sync treated as certifying visible records"),
    ("NC04", "C04", "first-candidate geometry: 128-byte slots in shared pages, seal in block 0"),
    ("NC05a", "C05", "the recorder thread owns the lock descriptions (first candidate)"),
    ("NC05b", "C05", "the store-lock description duplicated into the worker, which unlocks"),
    ("NC06", "C06", "startup syncs and reads journals before locking (first candidate)"),
    ("NC07", "C07", "bounded submission queue with re-acknowledgement (first candidate)"),
    ("NC08", "C08", "failure delivered as a message on a bounded event queue (first candidate)"),
    ("NC09", "C09", "refuse only when recorded outstanding work exists"),
    ("NC10a", "C10", "header reserved bytes unchecked"),
    ("NC10b", "C10", "seal reserved bytes and tail unchecked (first candidate's unassigned bytes)"),
    ("NC10c", "C10", "record-block padding unchecked"),
    ("NC10d", "C10", "checksum-valid invalid header taken as an abandoned claim (first candidate)"),
    ("NC10e", "C10", "non-canonical decimals accepted in text"),
    ("NC11a", "C11", "binding includes the file location (first candidate)"),
    ("NC11b", "C11", "binding ignores the content"),
    ("NC12a", "C12", "temporary disposition names read as dispositions"),
    ("NC12b", "C12", "a PROVISION inode mismatch ignored"),
    ("NC12c", "C12", "retirement without preconditions hides pool claims"),
    ("NC13", "C13", "grammar rule G16 (verdict iff closure) removed"),
    ("NC14a", "C14", "untrusted entries opened in blocking mode before the type check"),
    ("NC14b", "C14", "ext4 decided by the shared magic alone (first candidate)"),
    ("NC15a", "C15", "durable published before the sync"),
    ("NC15b", "C15", "sync retried after EIO"),
    ("NC16", "C16", "a visible RunEnded taken as resolution"),
    ("NC17", "C17", "claim counter unbounded"),
    ("NC18a", "C18", "no store admission gate: admission continues after a fatal condition (R1)"),
    ("NC18b", "C18", "R1 failure targeting: the cause's own sequence, delivered only at that position"),
    ("NC18c", "C18", "a submission for an unissued future sequence accepted silently (R1)"),
    ("NC18d", "C19", "publication after the fatal latch (R1)"),
    ("NC18e", "C18", "an unexpected acknowledgement outcome ignored"),
    ("NC18f", "C18", "record_failed for an unissued record counted as delivered"),
    ("NC19", "C19", "a health check followed by an unprotected admission call"),
    ("NC20a", "C20", "in-session verification requests a new shared lock (R1 workflow)"),
    ("NC20b", "C20", "in-session verification converts the session's lock to shared"),
    ("NC20c", "C20", "the session releases its lock around verification"),
    ("NC20d", "C20", "a Busy verification taken as success"),
    ("NC20e", "C20", "maintenance authority asserted by a flag instead of the retained lock"),
    ("NC20f", "C20", "in-session verification skipped: an earlier report reused"),
    ("NC21a", "C21", "no post-lock revalidation of the PROVISION selection (R1)"),
    ("NC21b", "C21", "revalidation through the descriptor kept from selection"),
    ("NC22a", "C22", "directory entries durable only through fsync_dir (R1 model)"),
    ("NC22b", "C22", "no page reclaim while a descriptor is open (R1 model)"),
    ("NC23", "C23", "an undocumented directory sync in recycling (a hidden model-only step)"),
    ("NC24a", "C24", "enumeration bounds checked only after examining the entries"),
    ("NC24b", "C24", "a startup authorized from the truncated report"),
    ("NC24c", "C24", "retirement preconditions evaluated on the truncated report"),
    ("NC-ACT-VISIBLE", "C25", "a freshly revalidated, visible selection treated as activated"),
    ("NC-ACT-ORDER", "C25", "claim, acknowledgement and admission before the activation"),
    ("NC-ACT-ERROR", "C25", "a failed or uncertain activation sync treated as success"),
    ("NC-ACT-PROBE", "C25", "no certification probe: a sync returning 0 after a silent commit failure trusted"),
    ("NC-ACT-REVALIDATE", "C25", "the identity and selection revalidation around activation omitted"),
    ("NC-ACT-COMPOSE", "C26", "the R2 protocol: no activation, and a fresh root provisioned after Unprovisioned"),
    ("NC-PROF-JOURNAL", "C27", "an external journal accepted: the journal's location never checked"),
    ("NC-PROF-MODE", "C27", "an excluded mode in the effective listing accepted (data=writeback, nobarrier, "
                            "journal_async_commit, norecovery)"),
    ("NC-ACT-UNQUALIFIED", "C27", "an unqualified profile authorizes: the profile taken from the pinned "
                                  "mountinfo strings and the attestation, the kernel never compared (R3)"),
    ("NC-PROF-GUARD", "C27", "the effective profile not checked again after the syncs (A4)"),
    ("NC-PROOF-ORDER", "C28", "the probe before the directory syncs"),
    ("NC-FSYNC-COMPLETED", "C29", "R3's fsync model: every aborted, unnoticed journal reported as EIO, "
                                  "whatever the state of the inode's transaction"),
    ("NC-ERR-DETECTED", "C29", "a discarded flush status treated as detected: an invented abort at an "
                               "unchecked flush site"),
    ("NC-STORAGE-VOLATILE", "C30", "a volatile-completion profile admitted as stable: queue/write_cache "
                                   "alone decides, so a write to it hides a volatile cache"),
    ("NC-STORAGE-CLAIM", "C30", "an unverified storage claim or a stale qualification treated as "
                                "authority: the identity PROVISION claims taken in place of the one "
                                "observed, and any admission object accepted at the claim"),
    ("NC-STORAGE-COMPLETION", "C31", "durability published before the operation completes: a home write "
                                     "still in flight counted as complete, so the tail passes it"),
    ("NC-STORAGE-ERROR", "C31", "a reported write error converted into success: the sync returns 0 for a "
                                "write the device reported failed, and the record is acknowledged"),
    ("NC-STORAGE-TAIL", "C31", "required journal history discarded without durable home data or a "
                               "retained log copy: the tail moves past a failed home write"),
    ("NC-STORAGE-COMPOSED", "C32", "the unsafe path restored: storage admitted on the Owner's "
                                   "attestation (A-S5, R4's domain), a volatile write-back cache included"),
]

# A control that must be detected by the composed check alone: the earlier
# snapshot checks must still pass under it (mission R3 section 7).
COMPOSE_ONLY = {"NC-ACT-COMPOSE": ("C12", "C20", "C21", "C22"),
                "NC-STORAGE-COMPOSED": ("C25", "C26", "C27", "C28")}

ASSUMPTIONS = [
    "A-S1 stable completion (R5; R2's flush honesty restated for admitted storage), A-S2 4096-byte "
    "containment, A-S3 no silent loss after a successful completion, A-S4 read stability (design "
    "section 5): assumed by the model, never established by it. C31 shows A-S1 necessary.",
    "A-M1: on the supported ext4 profile, metadata operations become durable atomically and in "
    "issue order. The 'ordered' schedules use it and are themselves a superset of ext4 outcomes; "
    "the 'per-directory' schedules are a conservative over-approximation, not ext4 behaviour.",
    "Kernel semantics as documented: errseq sampling at open and reporting per open file "
    "description; flock per open file description; fifo(7) non-blocking open; clean-page reclaim "
    "regardless of open descriptors. Modelled, not tested.",
    "ext4 overwrite of written, unshared extents needs no allocation (qualification, D-6).",
    "SHA-256 collision resistance (bindings, digests, the storage identity).",
    "Domain A (store-uid rewriting) is outside the model's guarantees: gate G-AUTH stays open.",
    "CoreSim and the fixtures T1-T15 are derived by reading the source; they are not the Rust "
    "core. Conformance of the real core's journals (I9) and of the implementation's mutex "
    "linearization are left to the implementation mission.",
    "Interleavings are enumerated over the model's atomic steps; a step stands for one critical "
    "section of the design.",
    "The maintenance session is a specified future interface; no executable exists or is authorized.",
    "A-M2 (design section 5.1, corrected in R4): on the supported ext4 profile, as read from the Linux "
    "v6.17 sources, a directory fsync waits for the running or committing transaction and then tests "
    "the abort flag, and returns 0 untested when there is neither; a regular file's fsync waits for "
    "its inode's transaction only while it runs or commits, and otherwise returns 0 untested after its "
    "own checked cache flush; an abort lasts until the journal is closed; the next handle start turns "
    "it into emergency read-only, which every fsync returns first; a timestamp update by the file's "
    "owner starts a handle. Read from the sources, not tested; the runtime kernel must be qualified "
    "(G-HOST).",
    "A-S5 (R4: the device completes every cache flush the kernel issues) is withdrawn: the Architect did "
    "not accept it as the supported-runtime contract. R5 admits only storage to which the kernel sends "
    "no cache flush (design section 5.5); C29 keeps R4's counterexample on a volatile write-back cache, "
    "which every opening refuses (C30).",
    "Hardware truthfulness (A-S1): the controller's report that it has no volatile write cache is true, "
    "and nothing below the completion (controller, firmware, any bridge or hypervisor) holds "
    "acknowledged data only in volatile memory. The store observes the kernel's registration of the "
    "device, never the device (design section 5.9).",
    "The supported profile's superblock facts (a journal at inode 8, no external journal device, no "
    "fast_commit feature) are read by root at qualification; the store uid cannot read them at "
    "runtime, and the Owner keeps them by re-qualifying after any change (design section 13.2).",
    "R5: the storage qualification's root-only facts (the controller's Identify data, that the host is "
    "not a virtual machine guest, the Owner's evidence for stable completion) are established by the "
    "Owner (G-HOST); the model holds them as fixture values that only the qualification reads.",
    "The effective option listing, the jbd2 entry names, the kernel identity and (R5) the storage "
    "attributes, identities and controller report of the model's host are fixtures in the form Linux "
    "v6.17 prints them, not host observations. They qualify no device and no host.",
    "JournalSim is the smallest state of the cited jbd2, ext4 and block-layer branches, not a kernel: "
    "one journal, commits one at a time in order, background commits and aborts at chosen points. R5: "
    "on admitted storage no flush is sent; on a volatile cache every flush is honoured unless a "
    "scenario fails it; on a cache the kernel registered as absent, a superblock write is taken to "
    "reach the medium before the home writes (the worst order). Its enumeration is bounded (initial "
    "states of at most four events, one background event before each activation step) and is not an "
    "exhaustive proof.",
    "Write-error detection rests on the device's errseq (design section 5.6): its counter has 19 bits, "
    "and a collision would need 2^19 further errors on the device, each observed through the device's "
    "own mapping by another reader, before the next tail update or metadata change. Not modelled; "
    "outside the supported failure model.",
    "The whole model's checkpoint covers the directory metadata durable when it runs; on a cache other "
    "than admitted storage, file data are not modelled as volatile (such storage is refused, or "
    "outside A-S1).",
    "One filesystem: the profile requires the PROVISION directory, the state root's parent and the state "
    "root on one filesystem, verified by st_dev. The model has one journal per world; the per-directory "
    "family over-approximates independent directories, and so independent filesystems.",
    "Composed scenarios are bounded: one pool file claimed, one generation of dependent work, every "
    "procedure prefix, a first crash F1 or F2 under every schedule, and a second crash F1 or F2 under "
    "every schedule. R5's C32: every prefix of first provisioning and of succession followed by F1, "
    "four hosts, five storage events and every second crash. They are not an exhaustive proof.",
    "The Python model is not the Rust implementation; no Cargo command or Rust test ran.",
]

ASSERTION_CHANGES = [
    "C05: recorder loss is read from the exchange's latched fatal cause (WorkerLost) instead of "
    "R1's separate 'lost' flag; the requirement (loss is latched while locks stay held) is unchanged.",
    "C08: R1 asserted that, without the drop guard, an apply step saw no signal until a separate "
    "join-handle call. R2 makes the join-handle check part of every apply step, so the first apply "
    "latches WorkerVanished and delivers the failure: the requirement is preserved and the timing "
    "strengthened.",
    "C11 and C12: procedures run inside a maintenance session (R1's modelled procedures held no "
    "lock). C12 now runs under ordered and per-directory persistence schedules instead of "
    "fsync_dir-only durability; its safety assertions are unchanged; re-qualification and a valid "
    "retirement series were added.",
    "C15: the INV-4 assertion 'acknowledged before the failed sequence' became 'acknowledged no "
    "further than P, the durable position at the latch', which covers every fatal cause.",
    "C03 and C05: R1's evict (pages and inode, only with no open descriptor) became evict_inode in "
    "the same scenarios; page reclaim with open descriptors is now separate (C22).",
    "C10: the PROVISION grammar has a revision field; two non-canonical variants were added.",
    "C03: the writer's EIO is read from the exchange's latched fatal cause (Sync) instead of R1's "
    "'failed' field; the requirement (the writer observes EIO; a new description's sync certifies "
    "nothing) is unchanged.",
    "C09 and C16: every restart image set now also includes page reclaim while descriptors are open "
    "(F1+page-reclaim), so their counts grew (C09 17 to 18 images, C16 25787 to 26078 images); every "
    "R1 assertion is applied to every image.",
    "C12: its summary counts outcomes (one per crash point after F1, and one per schedule and data "
    "choice after F2) instead of R1's crash points, and adds recovery counts.",
    "R3, owner startups and maintenance sessions activate before they read the store (design section "
    "10.6). Every retained control's assertion and every retained check's detail line is identical to "
    "the R2 output, except C23's, which now also checks the R3 re-publication recovery (11 procedures "
    "and recoveries instead of 10).",
    "R3 model correction: the Worker's identity recheck (W5) resolved the journals directory through "
    "the world's first root; it now uses the root the owner opened, as design W5 and section 8.1 "
    "specify. Without it no owner could run on a successor root.",
    "R3 model correction: flock ownership was keyed by Python object identity, so a lock held when a "
    "world was cloned could never be released in the clone (a spurious Busy after F1). It is keyed by a "
    "serial of the open file description. No retained check cloned a world with a lock held.",
    "R4 model correction (F4): fsync_file returned EIO for every aborted, unnoticed journal. It now "
    "follows jbd2_complete_transaction: a running transaction is waited for and the abort flag tested; a "
    "completed one returns 0 untested. R3 assumed the probe's fsync always waited for the probe's "
    "transaction. No R3 check took the old branch (R3-F4-REACH), the safety requirement is unchanged "
    "(no claim on a selection whose dependencies are not durable), and the refusals it gave are now "
    "derived from the probe's handle start (C28, C29).",
    "R4: touch() models an abort landing between ext4's and jbd2's handle tests, and activation's hook "
    "sees the two probe windows; no retained check uses either.",
    "R4: every opening checks the effective profile (design section 6.4, steps 9-11) and PROVISION has "
    "a kernel field. The model's host is the qualified profile, so every retained check's result, "
    "every retained control's assertion and every retained detail line is identical to R3's output.",
    "R5, C28, a domain change. Old assumption: A-S5, every cache flush succeeds; the later continuations "
    "ran on a volatile cache with every flush honoured. New domain: admitted storage (stable "
    "completion, design section 5.5), to which no flush is sent; the continuations add reported home "
    "and superblock write failures, a write in flight and the R4 witness's failed flush (12 instead of "
    "6). Retained historical counterexample: R4's checkpoint witness, re-run on the unchanged R4 model "
    "(R4-F2-WITNESS) and kept in C29 on a volatile cache. Replacement assertion: every certified "
    "dependency survives every continuation and a power loss on admitted storage. Unchanged safety "
    "requirement: no certified activation's dependency is lost in domain H. The activation runs and "
    "their counts are unchanged.",
    "R5, C29: R4's 'necessity of A-S5' witness is kept, with the same events, on a volatile write-back "
    "cache, and relabelled as R4's counterexample, retained (such storage is not admitted, C30); the "
    "three discarded-status cases run on a volatile cache, which is where those flushes are sent; the "
    "storage paths of design section 5.8 and the admitted-storage conformance were added. No R4 "
    "assertion was removed or weakened.",
    "R5 model: the fixture host's device changed from a device-mapper device (253:1, dm-1) to an NVMe "
    "partition (259:3, nvme0n1p2), because R5 refuses stacked storage (R4-FIXTURE). The profile cases "
    "edit the listing under the new name. MountEnv keeps the whole /sys/dev/block link instead of the "
    "name, and adds the storage attributes and the root-only facts. Every retained check's detail line "
    "and every retained control's assertion is identical to R4's output except C28's and C29's "
    "(above).",
    "R5 model: SimFS has a device, stable by default, and a checkpoint that follows JournalSim's "
    "transitions (C31 checks the two agree); on stable storage nothing a retained check reads changes.",
    "R5 model: JournalSim has a device (stable by default; R4's semantics are the volatile cache with "
    "native FUA), home-write outcomes, the device's errseq, the in-memory and on-medium tails and the "
    "abort's rewrite of the superblock. Its tail is the oldest transaction whose home writes have not "
    "all completed, as jbd2_journal_get_log_tail computes it; on every R4 run that gives R4's result.",
    "R5: PROVISION has the storage fields of design section 7.5; every opening, A4 and the claim check "
    "the storage and the opening's own admission. The model's fixture is admitted storage, so every "
    "retained check runs on it.",
]


def run_check(check):
    _CLASSIFY_CACHE.clear()
    MATRIX_RESULT.clear()
    try:
        detail = check[2]()
        return "PASS", detail
    except AssertionError as error:
        return "FAIL", str(error)
    except Exception:  # a tool failure, never a caught control
        return "ERROR", traceback.format_exc(limit=3).strip().splitlines()[-1]


def main(argv=None):
    sys.dont_write_bytecode = True
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--json", help="write coverage JSON to this path")
    args = parser.parse_args(argv)
    by_id = {c[0]: c for c in CHECKS}
    out = ["P2-V1-R3B-I3-P-R5 design model", f"source baseline: {SOURCE_BASELINE}", ""]
    tool_failures = []
    sections = [
        ("== R1 counterexamples, re-run on the unchanged R1 model ==", "R1", r1_reproductions),
        ("== R2 composed counterexample, re-run on the unchanged R2 model ==", "R2", r2_reproductions),
        ("== R4 findings, re-run on the unchanged R3 model ==", "R3", r3_reproductions),
        ("== R5 findings, re-run on the unchanged R4 model ==", "R4", r4_reproductions),
    ]
    reproductions = {}
    for title, key, function in sections:
        out.append(title)
        MUT.active = frozenset()
        try:
            items = function()
        except Exception:
            items = []
            tool_failures.append(f"{key} reproduction: "
                                 + traceback.format_exc(limit=3).strip().splitlines()[-1])
        reproductions[key] = items
        for item in items:
            verdict = "REPRODUCED" if item["reproduced"] else "NOT REPRODUCED"
            out.append(f"{item['id']} ({item['finding']}) {verdict}: {item['observed']}")
        out.append("")
    out.append("== baseline checks ==")
    baseline = {}
    for check in CHECKS:
        MUT.active = frozenset()
        status, detail = run_check(check)
        baseline[check[0]] = (status, detail)
        out.append(f"{check[0]} {check[1]} {status}: {detail}")
        if status == "ERROR":
            tool_failures.append(f"{check[0]}: {detail}")
    matrix = {name: [list(r) for r in rows] for name, rows in MATRIX_RESULT.items()}
    if not matrix:
        MUT.active = frozenset()
        try:
            check_c12_admin_crash()
        except Exception:
            tool_failures.append("crash matrix: " + traceback.format_exc(limit=3).strip().splitlines()[-1])
        matrix = {name: [list(r) for r in rows] for name, rows in MATRIX_RESULT.items()}
    out.append("")
    out.append("== crash matrix (section 15.2): crash points, after F1 | after F2 ordered | "
               "added by the per-directory over-approximation ==")
    for name, rows in sorted(matrix.items()):
        for first, last, f1, ordered, extra in rows:
            span = f"{first}" if first == last else f"{first}-{last}"
            out.append(f"{name} {span}: {', '.join(f1)} | {', '.join(ordered)} | {', '.join(extra) or '-'}")
    composed = dict(COMPOSED_STATS)
    out.append("")
    out.append("== composed recovery (section 15.4): dimensions and outcomes ==")
    for key, value in sorted(composed.items()):
        out.append(f"{key}: {value}")
    traces = {name: list(items) for name, items in COMPOSED_TRACES.items()}
    out.append("representative traces (the last operation before each procedure's final sync, "
               "first crash F1):")
    for name, items in sorted(traces.items()):
        for item in items:
            out.append(f"  {item}")
    journal = dict(JOURNAL_STATS)
    out.append("")
    out.append("== activation proof (section 10.7): journal transaction windows ==")
    for key, value in sorted(journal.items()):
        out.append(f"{key}: {value}")
    storage = dict(STORAGE_COMPOSED_STATS)
    out.append("")
    out.append("== storage composed (section 15.4, R5): dimensions and outcomes ==")
    for key, value in sorted(storage.items()):
        out.append(f"{key}: {value}")
    out.append("")
    out.append("== negative controls (in-memory mutants) ==")
    controls = {}
    compose_evidence = {}
    for nc, target, description in NEGATIVE_CONTROLS:
        MUT.active = frozenset([nc])
        ASSUMPTION_EVIDENCE.clear()
        status, detail = run_check(by_id[target])
        marker = by_id[target][1]
        if status == "FAIL" and marker in detail:
            verdict = "CAUGHT"
        elif status == "FAIL":
            verdict = "WRONG-MARKER"
        elif status == "PASS":
            verdict = "SURVIVED"
        else:
            verdict = "TOOL-ERROR"
            tool_failures.append(f"{nc}: {detail}")
        snapshot = []
        for other in COMPOSE_ONLY.get(nc, ()):
            ostatus, odetail = run_check(by_id[other])
            snapshot.append((other, ostatus))
            if ostatus == "ERROR":
                tool_failures.append(f"{nc} with {other}: {odetail}")
            if ostatus != "PASS" and verdict == "CAUGHT":
                verdict = "NOT-COMPOSED-ONLY"
        if snapshot:
            compose_evidence[nc] = snapshot
        controls[nc] = (target, description, verdict, detail)
        out.append(f"{nc} -> {target} {verdict}: {description}")
        out.append(f"    assertion: {detail}")
        if snapshot:
            out.append("    under the same mutant, the earlier snapshot checks: "
                       + ", ".join(f"{c}={st}" for c, st in snapshot))
    MUT.active = frozenset()
    out.append("")
    out.append("== restored baseline ==")
    restored = {}
    ASSUMPTION_EVIDENCE.clear()
    for check in CHECKS:
        status, detail = run_check(check)
        restored[check[0]] = status
        if status == "ERROR":
            tool_failures.append(f"restored {check[0]}: {detail}")
    out.append(" ".join(f"{k}={v}" for k, v in restored.items()))
    out.append("")
    out.append("== tool failures ==")
    out.extend(tool_failures or ["none"])
    out.append("")
    out.append("== assertion and model changes (requirement preserved) ==")
    out.extend(f"- {a}" for a in ASSERTION_CHANGES)
    out.append("")
    out.append("== assumptions not established by the model ==")
    out.extend(f"- {a}" for a in ASSUMPTIONS + ASSUMPTION_EVIDENCE)
    passed = sum(1 for s, _ in baseline.values() if s == "PASS")
    caught = sum(1 for c in controls.values() if c[2] == "CAUGHT")
    counts = {key: (sum(1 for r in items if r["reproduced"]), len(items))
              for key, items in reproductions.items()}
    ok = (all(n and r == n for r, n in counts.values())
          and passed == len(CHECKS) and caught == len(NEGATIVE_CONTROLS)
          and all(v == "PASS" for v in restored.values()) and not tool_failures)
    out.append("")
    out.append(f"summary: R1 counterexamples {counts['R1'][0]}/{counts['R1'][1]} reproduced; R2 composed "
               f"counterexample {counts['R2'][0]}/{counts['R2'][1]} reproduced; R4 findings on the R3 "
               f"model {counts['R3'][0]}/{counts['R3'][1]} reproduced; R5 findings on the R4 model "
               f"{counts['R4'][0]}/{counts['R4'][1]} reproduced; baseline "
               f"{passed}/{len(CHECKS)} pass; negative controls {caught}/{len(NEGATIVE_CONTROLS)} "
               f"caught; restored {sum(v == 'PASS' for v in restored.values())}/{len(CHECKS)} pass; "
               f"tool failures {len(tool_failures)}")
    out.append("RESULT: " + ("PASS" if ok else "FAIL"))
    print("\n".join(out))
    if args.json:
        coverage = {
            "model": "P2-V1-R3B-I3-P-R5 design model",
            "source_baseline": SOURCE_BASELINE,
            "r1_model_sha256": R1_MODEL_SHA256,
            "r2_model_sha256": R2_MODEL_SHA256,
            "r3_model_sha256": R3_MODEL_SHA256,
            "r4_model_sha256": R4_MODEL_SHA256,
            "r1_reproduction": reproductions["R1"],
            "r2_reproduction": reproductions["R2"],
            "r3_reproduction": reproductions["R3"],
            "r4_reproduction": reproductions["R4"],
            "checks": [{"id": c[0], "marker": c[1], "requirement": c[3],
                        "r1_mission_items": c[4], "r2_mission_items": c[5],
                        "r3_mission_items": c[6], "r4_mission_items": R4_CHECK_ITEMS.get(c[0], []),
                        "r5_mission_items": R5_CHECK_ITEMS.get(c[0], []),
                        "baseline": baseline[c[0]][0], "detail": baseline[c[0]][1],
                        "restored": restored[c[0]]} for c in CHECKS],
            "r2_mission_items": {str(k): v for k, v in R2_MISSION_ITEMS.items()},
            "r3_mission_items": {str(k): v for k, v in R3_MISSION_ITEMS.items()},
            "r4_mission_items": {str(k): v for k, v in R4_MISSION_ITEMS.items()},
            "r5_mission_items": {str(k): v for k, v in R5_MISSION_ITEMS.items()},
            "negative_controls": [{"id": nc, "target": v[0], "restores": v[1], "result": v[2],
                                   "assertion": v[3],
                                   "snapshot_checks_under_mutant": compose_evidence.get(nc, [])}
                                  for nc, v in controls.items()],
            "crash_matrix": matrix,
            "composed_recovery": composed,
            "composed_traces": traces,
            "activation_protocol": [list(op) for op in ACTIVATION_PROTOCOL],
            "activation_proof": journal,
            "profile": {"required": list(REQUIRED_OPTIONS), "excluded": list(EXCLUDED_OPTIONS),
                        "journal_inode": JOURNAL_INODE,
                        "refused_cases": [label for label, _, _ in PROFILE_CASES]
                        + ["contradictory: mountinfo shows barrier, the listing nobarrier"]},
            "storage": {"class": STORAGE_CLASS,
                        "cache_admitted": {"write_cache": STORAGE_CACHE_ADMITTED[0],
                                           "fua": STORAGE_CACHE_ADMITTED[1]},
                        "identity_attributes": list(STORAGE_IDENTITY_ATTRS),
                        "refused_cases": [[label, reason] for label, _, reason in STORAGE_CASES],
                        "paths": [[label, device, fua, list(events), aborts, kept]
                                  for label, device, fua, events, aborts, kept in STORAGE_PATHS],
                        "composed_hosts": [label for label, _, _ in STORAGE_HOSTS],
                        "composed_events": [label for label, _ in STORAGE_EVENTS],
                        "composed": storage},
            "assertion_changes": ASSERTION_CHANGES,
            "tool_failures": tool_failures,
            "assumptions_not_established": ASSUMPTIONS + ASSUMPTION_EVIDENCE,
            "result": "PASS" if ok else "FAIL",
        }
        with open(args.json, "w", encoding="ascii") as handle:
            json.dump(coverage, handle, indent=2, sort_keys=False)
            handle.write("\n")
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())

//! P2-V1-R3B-I2A fixture controls for the custody core's canonical evidence
//! and request codec (`support/custody/codec.rs`), tested as shared: the
//! custody module is included here exactly as the core target includes it,
//! never copied. Golden byte vectors and digests; strict single-frame
//! decoding (truncation, trailing bytes, appended frames, unknown versions,
//! domains and tags, invalid booleans and presence bytes, malicious lengths,
//! over-limit input, digest and payload mutation, cross-domain
//! substitution); a canonical, injective encoding in which every byte is
//! bound; the core's real record and request digest paths; and decoding as
//! plain data that can do nothing. I2-R1 adds the request receipt control:
//! the digest the real core retains in each request receipt, observed
//! through a read-only accessor compiled for tests only, is the canonical
//! digest of that request's frame.
//!
//! Golden vectors: every frame and digest below was assembled field by field
//! from the version-1 table by a stdlib-only script (Python `struct` and
//! `hashlib`) kept in session storage, outside the repository, and every
//! digest was re-hashed by coreutils `sha256sum`; none was produced by the
//! codec under test. Each control also re-derives every vector's digest from
//! its bytes with the sha2 crate directly, apart from the codec's own path.
//!
//! These are not live evidence. Everything is in-process and in memory: no
//! file, record storage, bus, process or service is touched.

#[path = "support/custody/mod.rs"]
mod custody;

use custody::codec::{self, CodecError, Domain, RecordEvidence, RequestEvidence};
use custody::*;
use sha2::{Digest, Sha256};

const G0: [u8; 16] = [0x00; 16];
const G1: [u8; 16] = [0x11; 16];
const G2: [u8; 16] = [
    0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f,
];
const GF: [u8; 16] = [0xff; 16];
const G3C: [u8; 16] = [0x3c; 16];
const GA5: [u8; 16] = [0xa5; 16];

/// An identity as a vector states it: (generation, sequence).
type Id = ([u8; 16], u64);

/// A record kind as a vector states it, identities as (generation,
/// sequence) pairs (they have no public constructor).
#[derive(Debug, Clone, Copy)]
enum Want {
    Plain(RecordKind),
    ActionStarted(Id, SlotKind),
    ActionFailed(Id),
    ActionSettled(Id, Settlement),
    IncidentOpened(Id, Id, Option<SlotKind>),
    IncidentSettled(Id, Settlement),
}

fn action_id(action: ActionId) -> Id {
    (action.generation().bytes(), action.seq())
}

fn incident_id(incident: IncidentId) -> Id {
    (incident.generation().bytes(), incident.seq())
}

fn record_id(id: RecordId) -> Id {
    (id.generation().bytes(), id.seq())
}

impl Want {
    fn matches(&self, kind: &RecordKind) -> bool {
        match (*self, *kind) {
            (Want::Plain(want), got) => want == got,
            (Want::ActionStarted(id, slot), RecordKind::ActionStarted { action, kind }) => {
                action_id(action) == id && kind == slot
            }
            (Want::ActionFailed(id), RecordKind::ActionFailed { action }) => {
                action_id(action) == id
            }
            (Want::ActionSettled(id, how), RecordKind::ActionSettled { action, how: got }) => {
                action_id(action) == id && got == how
            }
            (
                Want::IncidentOpened(incident, action, slot),
                RecordKind::IncidentOpened {
                    incident: got_incident,
                    action: got_action,
                    kind,
                },
            ) => {
                incident_id(got_incident) == incident
                    && action_id(got_action) == action
                    && kind == slot
            }
            (
                Want::IncidentSettled(id, how),
                RecordKind::IncidentSettled { incident, how: got },
            ) => incident_id(incident) == id && got == how,
            _ => false,
        }
    }
}

/// One golden record frame: its covered bytes (header and payload, fields
/// separated by spaces), its digest, and the values it encodes.
struct RecordVector {
    frame: &'static str,
    digest: &'static str,
    id: Id,
    at: u64,
    kind: Want,
}

/// One golden request frame.
struct RequestVector {
    frame: &'static str,
    digest: &'static str,
    request: Request,
}

/// The golden record vectors (generated outside the repository from the
/// version-1 table; see the module documentation).
const RECORD_VECTORS: &[RecordVector] = &[
    RecordVector {
        frame: "4e584344 52 01 0025 11111111111111111111111111111111 0000000000000001 0000000000000000 01 00000000",
        digest: "1df88a9cfe3b6dd027815af0cfb6a92c412c6de2d94cb8e84f80d3cf0ee516e3",
        id: (G1, 1),
        at: 0,
        kind: Want::Plain(RecordKind::RunStarted { dispositioned: 0 }),
    },
    RecordVector {
        frame: "4e584344 52 01 0025 ffffffffffffffffffffffffffffffff ffffffffffffffff ffffffffffffffff 01 ffffffff",
        digest: "75a67f6f239fad206b35209fcaa2bca3bf52e63def87aa12efac2ab777b12754",
        id: (GF, u64::MAX),
        at: u64::MAX,
        kind: Want::Plain(RecordKind::RunStarted { dispositioned: u32::MAX }),
    },
    RecordVector {
        frame: "4e584344 52 01 0026 11111111111111111111111111111111 0000000000000002 0000000000000001 02 00000000 01",
        digest: "6266f85645c857776ba08de4ea3f2c945b52efffdbed9ad1011412673b80afb8",
        id: (G1, 2),
        at: 1,
        kind: Want::Plain(RecordKind::CaseStarted { case: CaseId(0), expectation: Expectation::Clean }),
    },
    RecordVector {
        frame: "4e584344 52 01 0026 00000000000000000000000000000000 0000000000000000 1122334455667788 02 ffffffff 02",
        digest: "4d95799bca2b31230db6f44a4769fb7c0c4e6c0bc458b27d8369bdeac203f43c",
        id: (G0, 0),
        at: 0x1122_3344_5566_7788,
        kind: Want::Plain(RecordKind::CaseStarted { case: CaseId(u32::MAX), expectation: Expectation::RetainedBoundary }),
    },
    RecordVector {
        frame: "4e584344 52 01 0026 000102030405060708090a0b0c0d0e0f 0102030405060708 0000000000000003 02 01020304 03",
        digest: "ee05dad50310a8fff8cbcc33a09ac7514244a452365d091a386dfd3a30611ed6",
        id: (G2, 0x0102_0304_0506_0708),
        at: 3,
        kind: Want::Plain(RecordKind::CaseStarted { case: CaseId(0x0102_0304), expectation: Expectation::OutputDetached }),
    },
    RecordVector {
        frame: "4e584344 52 01 003a 11111111111111111111111111111111 0000000000000003 0000000000000004 03 000102030405060708090a0b0c0d0e0f 0000000000000000 01",
        digest: "8587df47a167e211ee9b2d086d2edf14fc779e4496e1547856c40f1aa4f90da8",
        id: (G1, 3),
        at: 4,
        kind: Want::ActionStarted((G2, 0), SlotKind::Process),
    },
    RecordVector {
        frame: "4e584344 52 01 003a 11111111111111111111111111111111 0000000000000004 0000000000000005 03 ffffffffffffffffffffffffffffffff ffffffffffffffff 02",
        digest: "35f55ec7504f80c3d1ff32562dc80fa44973511257aa883260efa3a7dd3721b6",
        id: (G1, 4),
        at: 5,
        kind: Want::ActionStarted((GF, u64::MAX), SlotKind::Workspace),
    },
    RecordVector {
        frame: "4e584344 52 01 003a 11111111111111111111111111111111 0000000000000005 0000000000000006 03 11111111111111111111111111111111 0000000000000007 03",
        digest: "8cec803c4c603d85e97b58d85865e6e071cb50593c507cf6c666a685538b374f",
        id: (G1, 5),
        at: 6,
        kind: Want::ActionStarted((G1, 7), SlotKind::Fixture),
    },
    RecordVector {
        frame: "4e584344 52 01 0039 11111111111111111111111111111111 0000000000000006 0000000000000007 04 11111111111111111111111111111111 0000000000000001",
        digest: "2c6b71ae14a5bee04123ba167cc03a6f0174a6bfb35d21ade9df45ad4237454e",
        id: (G1, 6),
        at: 7,
        kind: Want::ActionFailed((G1, 1)),
    },
    RecordVector {
        frame: "4e584344 52 01 003a 11111111111111111111111111111111 0000000000000007 0000000000000008 05 11111111111111111111111111111111 0000000000000001 01",
        digest: "4d912098c05741a18bfc5c13339d26d1cedccfd2488e991fffd2ed36c09b3af5",
        id: (G1, 7),
        at: 8,
        kind: Want::ActionSettled((G1, 1), Settlement::Confirmed),
    },
    RecordVector {
        frame: "4e584344 52 01 003a 11111111111111111111111111111111 0000000000000008 0000000000000009 05 11111111111111111111111111111111 0000000000000001 02",
        digest: "a67e2c8d8f9c96692387cdf23c73344cfe621f98d42aef5f59b515d3a25a6a87",
        id: (G1, 8),
        at: 9,
        kind: Want::ActionSettled((G1, 1), Settlement::OutputLost),
    },
    RecordVector {
        frame: "4e584344 52 01 003a 11111111111111111111111111111111 0000000000000009 000000000000000a 05 00000000000000000000000000000000 0000000000000000 03",
        digest: "7922ac776b03addca545e429b3171e28b1f441dde8b660a6434aed588580feb8",
        id: (G1, 9),
        at: 10,
        kind: Want::ActionSettled((G0, 0), Settlement::NothingCreated),
    },
    RecordVector {
        frame: "4e584344 52 01 003a 11111111111111111111111111111111 000000000000000a 000000000000000b 05 000102030405060708090a0b0c0d0e0f 0102030405060708 04",
        digest: "68cecad189f89fcf86ee152b4a1113298ea9b6a44a1a513aba3be2e0f213db8c",
        id: (G1, 10),
        at: 11,
        kind: Want::ActionSettled((G2, 0x0102_0304_0506_0708), Settlement::NotAdmitted),
    },
    RecordVector {
        frame: "4e584344 52 01 0026 11111111111111111111111111111111 000000000000000b 000000000000000c 06 00000001 00",
        digest: "036cce17c3a88ed633fb3f1c807e9a994d8b06de68b963a8400acd5445f55e0c",
        id: (G1, 11),
        at: 12,
        kind: Want::Plain(RecordKind::CaseEnded { case: CaseId(1), passed: false }),
    },
    RecordVector {
        frame: "4e584344 52 01 0026 11111111111111111111111111111111 000000000000000c 000000000000000d 06 00000001 01",
        digest: "0be591dbb25a40f0651a2991f483bbed188c4f5cd96f697607c794fa9518fd69",
        id: (G1, 12),
        at: 13,
        kind: Want::Plain(RecordKind::CaseEnded { case: CaseId(1), passed: true }),
    },
    RecordVector {
        frame: "4e584344 52 01 002c 11111111111111111111111111111111 0000000000000014 000000000000001e 07 01 01 01 0000000000000000",
        digest: "dbb39737cc969c9d9467ef2fff7f50d5a903669c9e6368334f533e95a205e465",
        id: (G1, 20),
        at: 30,
        kind: Want::Plain(RecordKind::Control { fact: ControlFact::AdmissionClosed { reason: ClosureReason::Cancelled(CancelReason::Requested), after: 0 } }),
    },
    RecordVector {
        frame: "4e584344 52 01 002c 11111111111111111111111111111111 0000000000000015 000000000000001f 07 01 01 02 0000000000000001",
        digest: "4e300222b14664e3acb2709138018c56c740a3b092f0196f966fa40f346d4223",
        id: (G1, 21),
        at: 31,
        kind: Want::Plain(RecordKind::Control { fact: ControlFact::AdmissionClosed { reason: ClosureReason::Cancelled(CancelReason::LeaseLost), after: 1 } }),
    },
    RecordVector {
        frame: "4e584344 52 01 002c 11111111111111111111111111111111 0000000000000016 0000000000000020 07 01 01 03 ffffffffffffffff",
        digest: "eebf9acedc715992f8d1ef20cea08ad2dd541e4e6973f80e93f721e09ae2ee00",
        id: (G1, 22),
        at: 32,
        kind: Want::Plain(RecordKind::Control { fact: ControlFact::AdmissionClosed { reason: ClosureReason::Cancelled(CancelReason::Shutdown), after: u64::MAX } }),
    },
    RecordVector {
        frame: "4e584344 52 01 002c 11111111111111111111111111111111 0000000000000017 0000000000000021 07 01 01 04 0102030405060708",
        digest: "4c8b83ea960bc819cd6a50b96c39b9c4728a476b3ce50f3173a30b5441b4fdd3",
        id: (G1, 23),
        at: 33,
        kind: Want::Plain(RecordKind::Control { fact: ControlFact::AdmissionClosed { reason: ClosureReason::Cancelled(CancelReason::Stop), after: 0x0102_0304_0506_0708 } }),
    },
    RecordVector {
        frame: "4e584344 52 01 002c 11111111111111111111111111111111 0000000000000018 0000000000000022 07 01 01 05 0000000000000002",
        digest: "d02e309dd2c7acff5802f97ac0a8b94cb5deb606c4144a3ef5544fb95147159a",
        id: (G1, 24),
        at: 34,
        kind: Want::Plain(RecordKind::Control { fact: ControlFact::AdmissionClosed { reason: ClosureReason::Cancelled(CancelReason::RunBudget), after: 2 } }),
    },
    RecordVector {
        frame: "4e584344 52 01 002c 11111111111111111111111111111111 0000000000000028 000000000000003c 07 01 02 01 0000000000000000",
        digest: "5c926c40256a4161178c2c8ace241886a017cb28e02ae649ea894093a1bbe2af",
        id: (G1, 40),
        at: 60,
        kind: Want::Plain(RecordKind::Control { fact: ControlFact::AdmissionClosed { reason: ClosureReason::Failed(FailureClass::Assertion), after: 0 } }),
    },
    RecordVector {
        frame: "4e584344 52 01 002c 11111111111111111111111111111111 0000000000000029 000000000000003d 07 01 02 02 0000000000000001",
        digest: "0f7a9b1ae736416ca465c2b4a875ad8abcffea76e1784689549049160baf6fa1",
        id: (G1, 41),
        at: 61,
        kind: Want::Plain(RecordKind::Control { fact: ControlFact::AdmissionClosed { reason: ClosureReason::Failed(FailureClass::UnexpectedRetained), after: 1 } }),
    },
    RecordVector {
        frame: "4e584344 52 01 002c 11111111111111111111111111111111 000000000000002a 000000000000003e 07 01 02 03 0000000000000002",
        digest: "a99b37298be48dcc6f9ab47bc6e7cf90021b3269ea65b7cc47bc9c7b706fe48b",
        id: (G1, 42),
        at: 62,
        kind: Want::Plain(RecordKind::Control { fact: ControlFact::AdmissionClosed { reason: ClosureReason::Failed(FailureClass::UnexpectedCleanup), after: 2 } }),
    },
    RecordVector {
        frame: "4e584344 52 01 002c 11111111111111111111111111111111 000000000000002b 000000000000003f 07 01 02 04 0000000000000003",
        digest: "f577db8f3108a08415e2088c73ceb906059de0180f580313fd11a290211b1c6a",
        id: (G1, 43),
        at: 63,
        kind: Want::Plain(RecordKind::Control { fact: ControlFact::AdmissionClosed { reason: ClosureReason::Failed(FailureClass::ExpectedConditionUnmet), after: 3 } }),
    },
    RecordVector {
        frame: "4e584344 52 01 002c 11111111111111111111111111111111 000000000000002c 0000000000000040 07 01 02 05 ffffffffffffffff",
        digest: "f1548c8e50f27d9b9e4ec1f6337cc1632a8f97db7c406d259f5706b1f276d5a1",
        id: (G1, 44),
        at: 64,
        kind: Want::Plain(RecordKind::Control { fact: ControlFact::AdmissionClosed { reason: ClosureReason::Failed(FailureClass::UnexpectedOwner), after: u64::MAX } }),
    },
    RecordVector {
        frame: "4e584344 52 01 002c 11111111111111111111111111111111 000000000000002d 0000000000000041 07 01 02 06 0000000000000005",
        digest: "0d0e03162de32a12d65d9b2752052df62179b62606255e6fefa0dc655b0021f8",
        id: (G1, 45),
        at: 65,
        kind: Want::Plain(RecordKind::Control { fact: ControlFact::AdmissionClosed { reason: ClosureReason::Failed(FailureClass::LateOwner), after: 5 } }),
    },
    RecordVector {
        frame: "4e584344 52 01 002c 11111111111111111111111111111111 000000000000002e 0000000000000042 07 01 02 07 0000000000000006",
        digest: "5eb24ae75a71aed445b94b4bb94b572a6d66de686616b0725030f2fb3dc1e08c",
        id: (G1, 46),
        at: 66,
        kind: Want::Plain(RecordKind::Control { fact: ControlFact::AdmissionClosed { reason: ClosureReason::Failed(FailureClass::UnknownOutcome), after: 6 } }),
    },
    RecordVector {
        frame: "4e584344 52 01 002c 11111111111111111111111111111111 000000000000002f 0000000000000043 07 01 02 08 0000000000000007",
        digest: "12bb5b920746116c031b01dcfb4e7899927ffda2b9ad4665801013ece379db92",
        id: (G1, 47),
        at: 67,
        kind: Want::Plain(RecordKind::Control { fact: ControlFact::AdmissionClosed { reason: ClosureReason::Failed(FailureClass::OutputLost), after: 7 } }),
    },
    RecordVector {
        frame: "4e584344 52 01 002c 11111111111111111111111111111111 0000000000000030 0000000000000044 07 01 02 09 0000000000000008",
        digest: "811014db56bd3d849b46d59cbf063bb6b7aeaa367e1baa2fffdb723bc2c6f71e",
        id: (G1, 48),
        at: 68,
        kind: Want::Plain(RecordKind::Control { fact: ControlFact::AdmissionClosed { reason: ClosureReason::Failed(FailureClass::AuthorityLost), after: 8 } }),
    },
    RecordVector {
        frame: "4e584344 52 01 002c 11111111111111111111111111111111 0000000000000031 0000000000000045 07 01 02 0a 0000000000000009",
        digest: "d237e9fff6c494921dc24300263235fb09f64e68bc7e474691a8077c076c654b",
        id: (G1, 49),
        at: 69,
        kind: Want::Plain(RecordKind::Control { fact: ControlFact::AdmissionClosed { reason: ClosureReason::Failed(FailureClass::RecordFailed), after: 9 } }),
    },
    RecordVector {
        frame: "4e584344 52 01 002c 11111111111111111111111111111111 0000000000000032 0000000000000046 07 01 02 0b 000000000000000a",
        digest: "a3011ca0fd78d22af9b7b2cc82cf843ff1a9c251fdd01f7f31033f075fdfaeb5",
        id: (G1, 50),
        at: 70,
        kind: Want::Plain(RecordKind::Control { fact: ControlFact::AdmissionClosed { reason: ClosureReason::Failed(FailureClass::RecorderFault), after: 10 } }),
    },
    RecordVector {
        frame: "4e584344 52 01 002c 11111111111111111111111111111111 0000000000000033 0000000000000047 07 01 02 0c 0102030405060708",
        digest: "62fc6d6e1767a20b779a3125770830ae573914366b1939ac6dd2a279232f9011",
        id: (G1, 51),
        at: 71,
        kind: Want::Plain(RecordKind::Control { fact: ControlFact::AdmissionClosed { reason: ClosureReason::Failed(FailureClass::Cancelled), after: 0x0102_0304_0506_0708 } }),
    },
    RecordVector {
        frame: "4e584344 52 01 0022 11111111111111111111111111111111 000000000000003c 0000000000000050 07 02",
        digest: "ccb5d38fd66e5fcbf971eb089fd78614eac039cbf87f29b1a5cf83d766d0ea57",
        id: (G1, 60),
        at: 80,
        kind: Want::Plain(RecordKind::Control { fact: ControlFact::ShutdownRefused }),
    },
    RecordVector {
        frame: "4e584344 52 01 0021 11111111111111111111111111111111 000000000000003d 0000000000000051 08",
        digest: "25cd018c756c393ab5b79706f2d08821308bcc9113b16bb649dfb8b7d667a248",
        id: (G1, 61),
        at: 81,
        kind: Want::Plain(RecordKind::RecoveryRequired),
    },
    RecordVector {
        frame: "4e584344 52 01 0026 11111111111111111111111111111111 000000000000003e 0000000000000052 09 00000001 00",
        digest: "3abf32d0e8cbbf9c6de478ca93c4d83b44a2e279354daf5e8a68738bb5949361",
        id: (G1, 62),
        at: 82,
        kind: Want::Plain(RecordKind::RecoveryAttempt { attempt: 1, resolved: false }),
    },
    RecordVector {
        frame: "4e584344 52 01 0026 11111111111111111111111111111111 000000000000003f 0000000000000053 09 ffffffff 01",
        digest: "f60d502edc0bdb8f646ce56b484a8a665568a759f383d529b6201ddf91fcbdd5",
        id: (G1, 63),
        at: 83,
        kind: Want::Plain(RecordKind::RecoveryAttempt { attempt: u32::MAX, resolved: true }),
    },
    RecordVector {
        frame: "4e584344 52 01 0023 11111111111111111111111111111111 0000000000000040 0000000000000054 0a 01 00",
        digest: "8a07b4e7cb23e30f58054986216699a1117cd870fbfdc1172d8c909d2c823ce5",
        id: (G1, 64),
        at: 84,
        kind: Want::Plain(RecordKind::RunEnded { verdict: Verdict::Pending, resolved_by: None }),
    },
    RecordVector {
        frame: "4e584344 52 01 0023 11111111111111111111111111111111 0000000000000041 0000000000000055 0a 02 00",
        digest: "a2d26f768e6ecfaff56605339726c8cf6500a4a7113738bde27cd6191803ff31",
        id: (G1, 65),
        at: 85,
        kind: Want::Plain(RecordKind::RunEnded { verdict: Verdict::Passed, resolved_by: None }),
    },
    RecordVector {
        frame: "4e584344 52 01 0027 11111111111111111111111111111111 0000000000000042 0000000000000056 0a 03 01 00000000",
        digest: "95c6ffa53dc4e4acd7a448f8ebdb293694a41b5ae43e7f6ec191b71ec69c2d59",
        id: (G1, 66),
        at: 86,
        kind: Want::Plain(RecordKind::RunEnded { verdict: Verdict::Failed, resolved_by: Some(0) }),
    },
    RecordVector {
        frame: "4e584344 52 01 0027 11111111111111111111111111111111 0000000000000043 0000000000000057 0a 02 01 ffffffff",
        digest: "05d1c57038aee796c78fdfa194da81a14ab774690bace971bc83e2ab46ea8b1d",
        id: (G1, 67),
        at: 87,
        kind: Want::Plain(RecordKind::RunEnded { verdict: Verdict::Passed, resolved_by: Some(u32::MAX) }),
    },
    RecordVector {
        frame: "4e584344 52 01 0052 11111111111111111111111111111111 0000000000000046 000000000000005a 0b 11111111111111111111111111111111 0000000000000001 11111111111111111111111111111111 0000000000000003 00",
        digest: "d1e2d6a959b71689485a3418447c5fb232e2a8de05ef51ad9c54b03f4959de24",
        id: (G1, 70),
        at: 90,
        kind: Want::IncidentOpened((G1, 1), (G1, 3), None),
    },
    RecordVector {
        frame: "4e584344 52 01 0053 11111111111111111111111111111111 0000000000000047 000000000000005b 0b 11111111111111111111111111111111 0000000000000002 11111111111111111111111111111111 0000000000000003 01 01",
        digest: "a4814e6bc5633b7c1424ba93f971040012c830db627fcb8d2d0bd1ce288f789e",
        id: (G1, 71),
        at: 91,
        kind: Want::IncidentOpened((G1, 2), (G1, 3), Some(SlotKind::Process)),
    },
    RecordVector {
        frame: "4e584344 52 01 0053 11111111111111111111111111111111 0000000000000048 000000000000005c 0b ffffffffffffffffffffffffffffffff ffffffffffffffff 00000000000000000000000000000000 0000000000000000 01 02",
        digest: "54400e4f3d95134fd68699b912e7f559caa1e4776007bc31e5acbffa616b7a80",
        id: (G1, 72),
        at: 92,
        kind: Want::IncidentOpened((GF, u64::MAX), (G0, 0), Some(SlotKind::Workspace)),
    },
    RecordVector {
        frame: "4e584344 52 01 0053 11111111111111111111111111111111 0000000000000049 000000000000005d 0b 000102030405060708090a0b0c0d0e0f 0102030405060708 000102030405060708090a0b0c0d0e0f 0000000000000001 01 03",
        digest: "c38beb76b437acf545c4b6e2b118846be46921b8a6bdee14be89e38c09c4ec2c",
        id: (G1, 73),
        at: 93,
        kind: Want::IncidentOpened((G2, 0x0102_0304_0506_0708), (G2, 1), Some(SlotKind::Fixture)),
    },
    RecordVector {
        frame: "4e584344 52 01 003a 11111111111111111111111111111111 000000000000004a 000000000000005e 0c 11111111111111111111111111111111 0000000000000001 01",
        digest: "014ef751cf6ec4548a32e95663f8a68cb70bb5b8fbc437193d329fd46c9bc79f",
        id: (G1, 74),
        at: 94,
        kind: Want::IncidentSettled((G1, 1), Settlement::Confirmed),
    },
    RecordVector {
        frame: "4e584344 52 01 003a 11111111111111111111111111111111 000000000000004b 000000000000005f 0c 11111111111111111111111111111111 0000000000000002 02",
        digest: "388313d7da15b1dff00effbd8408a6e36d8bba34beb7789e6a57245acedca8ba",
        id: (G1, 75),
        at: 95,
        kind: Want::IncidentSettled((G1, 2), Settlement::OutputLost),
    },
    RecordVector {
        frame: "4e584344 52 01 003a 11111111111111111111111111111111 000000000000004c 0000000000000060 0c 00000000000000000000000000000000 0000000000000000 03",
        digest: "0d61b1f16567bb350574b725a08f479007a3d49da0dda093bc48903685d0ee00",
        id: (G1, 76),
        at: 96,
        kind: Want::IncidentSettled((G0, 0), Settlement::NothingCreated),
    },
    RecordVector {
        frame: "4e584344 52 01 003a 11111111111111111111111111111111 000000000000004d 0000000000000061 0c ffffffffffffffffffffffffffffffff ffffffffffffffff 04",
        digest: "8fe5ff3aecf7b914573fc78452072762681430a2ed9bb71318c118e2ba225229",
        id: (G1, 77),
        at: 97,
        kind: Want::IncidentSettled((GF, u64::MAX), Settlement::NotAdmitted),
    },
    RecordVector {
        frame: "4e584344 52 01 003a 11111111111111111111111111111111 000000000000004e 0000000000000062 0c 000102030405060708090a0b0c0d0e0f 0000000000000000 01",
        digest: "2d0b890abbe20d9d28a09eda6cbb350c8e7d38775ae6dbcf83c732a7a661f9b5",
        id: (G1, 78),
        at: 98,
        kind: Want::IncidentSettled((G2, 0), Settlement::Confirmed),
    },
    RecordVector {
        frame: "4e584344 52 01 0025 000102030405060708090a0b0c0d0e0f 0000000000000000 0000000000000002 01 00000001",
        digest: "c73769a961f5df85874608a6d6d3092339969fb180100a19981dc7e4f4ac8f56",
        id: (G2, 0),
        at: 2,
        kind: Want::Plain(RecordKind::RunStarted { dispositioned: 1 }),
    },
];

/// The golden request vectors.
const REQUEST_VECTORS: &[RequestVector] = &[
    RequestVector {
        frame: "4e584344 51 01 0021 11111111111111111111111111111111 0000000000000001 01 0000000000000000",
        digest: "b2f4d076e902bf37e9fa8647a4f6343674a865a11b919fd9c0ec781f6991a017",
        request: Request { generation: Generation::new(G1), seq: 1, op: RequestOp::Retry { epoch: 0 } },
    },
    RequestVector {
        frame: "4e584344 51 01 0021 ffffffffffffffffffffffffffffffff ffffffffffffffff 01 ffffffffffffffff",
        digest: "4a4c2da9e2ff73f3c7335a7fff546cede7bdd0e81108e62c36fe282ac069acbe",
        request: Request { generation: Generation::new(GF), seq: u64::MAX, op: RequestOp::Retry { epoch: u64::MAX } },
    },
    RequestVector {
        frame: "4e584344 51 01 0019 00000000000000000000000000000000 0000000000000000 02",
        digest: "746f313a741620ed5570150d4e587198dc659132687826d030162d9955afac4e",
        request: Request { generation: Generation::new(G0), seq: 0, op: RequestOp::Shutdown },
    },
    RequestVector {
        frame: "4e584344 51 01 0021 000102030405060708090a0b0c0d0e0f 0000000000000002 01 0102030405060708",
        digest: "5d17d3c5bdb9c09780a1dd56e8013dc0f9ab8d930d0dcdd592e4bc3460d7df84",
        request: Request { generation: Generation::new(G2), seq: 2, op: RequestOp::Retry { epoch: 0x0102_0304_0506_0708 } },
    },
];

/// The request receipt vectors: the requests the receipt control serves
/// through the real core, each with its version-1 frame and digest from the
/// same independent derivation as the golden vectors.
const RECEIPT_VECTORS: &[RequestVector] = &[
    RequestVector {
        frame: "4e584344 51 01 0021 3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c 0000000000000001 01 0000000000000000",
        digest: "fa39efc25ef1f1b8d6e29a9235d66dd59b124a9081237aab0220590a950f6de4",
        request: Request { generation: Generation::new(G3C), seq: 1, op: RequestOp::Retry { epoch: 0 } },
    },
    RequestVector {
        frame: "4e584344 51 01 0019 3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c 0000000000000002 02",
        digest: "e65434b8803345efc5d1ff2b4911b4909356612cbe4a7d6daabf172300b82272",
        request: Request { generation: Generation::new(G3C), seq: 2, op: RequestOp::Shutdown },
    },
    RequestVector {
        frame: "4e584344 51 01 0021 3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c 0000000000000003 01 0000000000000001",
        digest: "9a227c00586d95650e6d5aa9493e09953b2d063269e30613fe83db67703ac694",
        request: Request { generation: Generation::new(G3C), seq: 3, op: RequestOp::Retry { epoch: 1 } },
    },
    RequestVector {
        frame: "4e584344 51 01 0021 3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c 0000000000000004 01 ffffffffffffffff",
        digest: "69a101a9786c39730c16ea2ee1c692aa96992667a1697b911ab378dd0b8c8948",
        request: Request { generation: Generation::new(G3C), seq: 4, op: RequestOp::Retry { epoch: u64::MAX } },
    },
    RequestVector {
        frame: "4e584344 51 01 0021 3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c 0000000000000005 01 0000000000000000",
        digest: "9ffecc155b972e94d992d533fa4cc4ff99e93081c861658491667ce03e831c2d",
        request: Request { generation: Generation::new(G3C), seq: 5, op: RequestOp::Retry { epoch: 0 } },
    },
    RequestVector {
        frame: "4e584344 51 01 0021 a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5 0000000000000001 01 0000000000000000",
        digest: "b55792d6176f5c3ce7a34a8177f994340e64c53faf2b1ea81a58b543d82e6749",
        request: Request { generation: Generation::new(GA5), seq: 1, op: RequestOp::Retry { epoch: 0 } },
    },
    RequestVector {
        frame: "4e584344 51 01 0019 3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c 0000000000000001 02",
        digest: "0bf26bf71b4c7345f3dff3dfe9ceb77bc7f77e45a9498e343acd392bbfa7bf23",
        request: Request { generation: Generation::new(G3C), seq: 1, op: RequestOp::Shutdown },
    },
    RequestVector {
        frame: "4e584344 51 01 0021 3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c 0000000000000002 01 0000000000000000",
        digest: "7d486ff6b72427b30ba1525b6586a7f991d527de6052c5780ef7ec045ecfa6ea",
        request: Request { generation: Generation::new(G3C), seq: 2, op: RequestOp::Retry { epoch: 0 } },
    },
    RequestVector {
        frame: "4e584344 51 01 0021 3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c 0000000000000003 01 0000000000000002",
        digest: "1a0c59e036b9e331eeedb39ee3976d23ec90e2afd8ce446d833d40a2bd5a43f2",
        request: Request { generation: Generation::new(G3C), seq: 3, op: RequestOp::Retry { epoch: 2 } },
    },
];

fn unhex(text: &str) -> Vec<u8> {
    let digits: Vec<u8> = text
        .bytes()
        .filter(|byte| !byte.is_ascii_whitespace())
        .collect();
    assert_eq!(digits.len() % 2, 0, "odd hex: {text}");
    digits
        .chunks(2)
        .map(|pair| {
            let text = std::str::from_utf8(pair).expect("ascii hex");
            u8::from_str_radix(text, 16).expect("hex digits")
        })
        .collect()
}

/// SHA-256 with the sha2 crate directly, apart from the codec's own path.
fn sha(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

/// A vector's complete frame: covered bytes, then digest.
fn whole(covered: &str, digest: &str) -> Vec<u8> {
    let mut bytes = unhex(covered);
    bytes.extend(unhex(digest));
    bytes
}

fn record_frames() -> Vec<Vec<u8>> {
    RECORD_VECTORS
        .iter()
        .map(|vector| whole(vector.frame, vector.digest))
        .collect()
}

fn request_frames() -> Vec<Vec<u8>> {
    REQUEST_VECTORS
        .iter()
        .map(|vector| whole(vector.frame, vector.digest))
        .collect()
}

/// The first golden record frame whose stated kind satisfies `pick`.
fn record_frame(pick: impl Fn(&Want) -> bool) -> Vec<u8> {
    let vector = RECORD_VECTORS
        .iter()
        .find(|vector| pick(&vector.kind))
        .expect("a matching vector");
    whole(vector.frame, vector.digest)
}

/// The evidence of the golden record frame with record identity `id`.
fn evidence_with_id(id: Id) -> RecordEvidence {
    let vector = RECORD_VECTORS
        .iter()
        .find(|vector| vector.id == id)
        .expect("a vector with that identity");
    codec::decode_record(&whole(vector.frame, vector.digest)).expect("a golden frame decodes")
}

/// Recompute a frame's digest trailer over the bytes before it, as anyone
/// who can rewrite the bytes can.
fn reseal(frame: &mut [u8]) {
    if frame.len() >= codec::DIGEST_LEN {
        let split = frame.len() - codec::DIGEST_LEN;
        let digest = sha(&frame[..split]);
        frame[split..].copy_from_slice(&digest);
    }
}

/// `frame` with byte `at` set to `value`, resealed.
fn resealed_with(frame: &[u8], at: usize, value: u8) -> Vec<u8> {
    let mut bytes = frame.to_vec();
    bytes[at] = value;
    reseal(&mut bytes);
    bytes
}

/// The declared payload length of a frame.
fn declared(frame: &[u8]) -> usize {
    usize::from(u16::from_be_bytes([frame[6], frame[7]]))
}

fn intent_of(evidence: &RecordEvidence) -> RecordIntent {
    RecordIntent {
        id: evidence.id,
        at: evidence.at,
        kind: evidence.kind,
        digest: evidence.digest,
    }
}

/// A stand-in native owner.
struct Token;

impl Resource for Token {
    fn kind(&self) -> SlotKind {
        SlotKind::Process
    }
}

/// A cleanup adapter that confirms every fact, or none.
struct Cleaner {
    confirm: bool,
}

impl Cleanup<Token> for Cleaner {
    fn attempt(&mut self, _owner: &mut Token, _entry: &EntryView) -> CleanupReport {
        let (fact, output) = if self.confirm {
            (Observed::Confirmed, OutputObserved::Complete)
        } else {
            (Observed::StillPending, OutputObserved::StillPending)
        };
        CleanupReport {
            subtree: fact,
            reaped: fact,
            output,
            removed: fact,
        }
    }
}

/// The recorder stand-in: it keeps what it is given, in memory.
#[derive(Default)]
struct Journal(Vec<RecordIntent>);

impl RecordSink for Journal {
    fn submit(&mut self, intent: &RecordIntent) {
        self.0.push(intent.clone());
    }
}

const GENERATION: Generation = Generation::new([0x3c; 16]);

/// A real custody with its stand-ins and an injected clock.
struct Run {
    custody: Custody<Token>,
    control: Control,
    journal: Journal,
    acked: usize,
    clock: u64,
    seq: u64,
}

impl Run {
    fn new(config: Config) -> Self {
        Self::with(config, GENERATION)
    }

    fn with(config: Config, generation: Generation) -> Self {
        let (custody, control) =
            Custody::new(config, generation, Vec::new(), Tick(0)).expect("a valid configuration");
        Self {
            custody,
            control,
            journal: Journal::default(),
            acked: 0,
            clock: 0,
            seq: 0,
        }
    }

    fn now(&mut self) -> Tick {
        self.clock += 1;
        Tick(self.clock)
    }

    fn flush(&mut self) {
        let now = self.now();
        self.custody
            .flush_records(&mut self.journal, now)
            .expect("the journal accepts every record");
    }

    fn ack_all(&mut self) {
        loop {
            self.flush();
            if self.acked == self.journal.0.len() {
                return;
            }
            while self.acked < self.journal.0.len() {
                let ack = RecordAck::of(&self.journal.0[self.acked]);
                let now = self.now();
                assert_eq!(
                    self.custody.acknowledge(&ack, now),
                    AckOutcome::Acknowledged
                );
                self.acked += 1;
            }
        }
    }

    fn request(&mut self, op: RequestOp) -> Request {
        self.seq += 1;
        Request {
            generation: self.custody.generation(),
            seq: self.seq,
            op,
        }
    }

    /// Queue `request` and serve it, past any recovery spacing.
    fn serve(&mut self, request: Request, confirm: bool) -> RequestOutcome {
        self.control.submit(request).expect("the queue is empty");
        self.clock += 10;
        let now = Tick(self.clock);
        let (served, outcome) = self
            .custody
            .serve(&mut Cleaner { confirm }, now)
            .expect("a request is queued");
        assert_eq!(served, request);
        outcome
    }

    /// Started, one case whose process owner's cleanup fails: the run
    /// requires recovery. Returns the operation's ticket.
    fn recovering(config: Config) -> (Run, OpTicket) {
        let mut run = Run::new(config);
        let now = run.now();
        run.custody.start_run(now).expect("the run starts");
        run.ack_all();
        let now = run.now();
        run.custody
            .begin_case(CaseId(1), Expectation::Clean, now)
            .expect("the case begins");
        let now = run.now();
        let reservation = run
            .custody
            .reserve(SlotKind::Process, now)
            .expect("reserved");
        run.ack_all();
        let now = run.now();
        let ticket = run.custody.admit(reservation, now).expect("admitted");
        let now = run.now();
        assert!(matches!(
            run.custody
                .complete(&ticket, NativeOutcome::Created(Token), now),
            Ok(Completion::Deposited(_))
        ));
        let now = run.now();
        let outcome = run
            .custody
            .end_case(&mut Cleaner { confirm: false }, now)
            .expect("the case ends");
        assert!(outcome.stopped && !outcome.resolved);
        assert_eq!(run.custody.snapshot().phase, RunPhase::RecoveryRequired);
        (run, ticket)
    }

    /// A run that issues every record kind: a failed cleanup and its
    /// recovery, the terminal record, a late owner after it, a refused
    /// shutdown and the incident's own recovery.
    fn every_kind() -> Run {
        let config = Config {
            recovery_spacing_millis: 1,
            ..Config::LIVE
        };
        let (mut run, ticket) = Run::recovering(config);
        let retry = run.request(RequestOp::Retry { epoch: 0 });
        assert!(matches!(
            run.serve(retry, true),
            RequestOutcome::Executed(Response::Retried { resolved: true, .. })
        ));
        run.ack_all();
        assert_eq!(run.custody.snapshot().phase, RunPhase::Finalized);
        run.custody.take_released();
        let now = run.now();
        assert!(matches!(
            run.custody
                .complete(&ticket, NativeOutcome::Created(Token), now),
            Ok(Completion::Late { .. })
        ));
        let shutdown = run.request(RequestOp::Shutdown);
        assert!(matches!(
            run.serve(shutdown, false),
            RequestOutcome::Executed(Response::Shutdown(ShutdownDecision::Refused(_)))
        ));
        let retry = run.request(RequestOp::Retry { epoch: 1 });
        assert!(matches!(
            run.serve(retry, true),
            RequestOutcome::Executed(Response::Retried { resolved: true, .. })
        ));
        run.ack_all();
        run
    }
}

/// The record kind's tag position and name, for coverage.
fn kind_name(kind: &RecordKind) -> &'static str {
    match kind {
        RecordKind::RunStarted { .. } => "RunStarted",
        RecordKind::CaseStarted { .. } => "CaseStarted",
        RecordKind::ActionStarted { .. } => "ActionStarted",
        RecordKind::ActionFailed { .. } => "ActionFailed",
        RecordKind::ActionSettled { .. } => "ActionSettled",
        RecordKind::CaseEnded { .. } => "CaseEnded",
        RecordKind::Control { .. } => "Control",
        RecordKind::RecoveryRequired => "RecoveryRequired",
        RecordKind::RecoveryAttempt { .. } => "RecoveryAttempt",
        RecordKind::RunEnded { .. } => "RunEnded",
        RecordKind::IncidentOpened { .. } => "IncidentOpened",
        RecordKind::IncidentSettled { .. } => "IncidentSettled",
    }
}

const KIND_NAMES: [&str; 12] = [
    "RunStarted",
    "CaseStarted",
    "ActionStarted",
    "ActionFailed",
    "ActionSettled",
    "CaseEnded",
    "Control",
    "RecoveryRequired",
    "RecoveryAttempt",
    "RunEnded",
    "IncidentOpened",
    "IncidentSettled",
];

/// The version-1 envelope constants, as the format table states them.
#[test]
fn c00_format_constants_are_the_documented_ones() {
    assert_eq!(codec::MAGIC, [0x4e, 0x58, 0x43, 0x44]);
    assert_eq!(codec::VERSION, 0x01);
    assert_eq!(
        (Domain::Record.byte(), Domain::Request.byte()),
        (0x52, 0x51)
    );
    assert_eq!(
        (
            codec::HEADER_LEN,
            codec::DIGEST_LEN,
            codec::MAX_FRAME,
            codec::MAX_PAYLOAD
        ),
        (8, 32, 4096, 4056)
    );
}

/// Every golden record vector: its digest is the SHA-256 of its covered
/// bytes; it decodes to exactly the values it states; the codec re-encodes
/// those values to exactly its bytes, and computes exactly its digest from
/// the typed fields; its length is `8 + L + 32`, at most 4096.
#[test]
fn c01_record_golden_vectors_decode_encode_and_digest_exactly() {
    let mut payloads = Vec::new();
    for (index, vector) in RECORD_VECTORS.iter().enumerate() {
        let covered = unhex(vector.frame);
        let digest = unhex(vector.digest);
        assert_eq!(sha(&covered).to_vec(), digest, "vector {index}");
        let bytes = whole(vector.frame, vector.digest);
        let evidence = codec::decode_record(&bytes)
            .unwrap_or_else(|error| panic!("[golden] vector {index} refused: {error:?}"));
        assert_eq!(record_id(evidence.id), vector.id, "[golden] vector {index}");
        assert_eq!(evidence.at, Tick(vector.at), "[golden] vector {index}");
        assert!(
            vector.kind.matches(&evidence.kind),
            "[golden] vector {index} decoded as {:?}, not {:?}",
            evidence.kind,
            vector.kind
        );
        assert_eq!(evidence.digest.to_vec(), digest, "[golden] vector {index}");
        assert_eq!(
            codec::record_digest(evidence.id, evidence.at, &evidence.kind).to_vec(),
            digest,
            "[golden] vector {index}: the typed digest"
        );
        assert_eq!(
            codec::encode_record(&intent_of(&evidence)),
            Ok(bytes.clone()),
            "[golden] vector {index}: re-encoding"
        );
        assert_eq!(
            bytes.len(),
            codec::HEADER_LEN + declared(&bytes) + codec::DIGEST_LEN
        );
        assert!(bytes.len() <= codec::MAX_FRAME);
        payloads.push(declared(&bytes));
    }
    assert_eq!(
        (payloads.iter().min(), payloads.iter().max()),
        (Some(&33), Some(&83)),
        "record payloads are 33 to 83 bytes"
    );
}

/// Every golden request vector, likewise; requests are plain data with
/// public fields, so they are compared whole.
#[test]
fn c02_request_golden_vectors_decode_encode_and_digest_exactly() {
    for (index, vector) in REQUEST_VECTORS.iter().enumerate() {
        let covered = unhex(vector.frame);
        let digest = unhex(vector.digest);
        assert_eq!(sha(&covered).to_vec(), digest, "vector {index}");
        let bytes = whole(vector.frame, vector.digest);
        let decoded = codec::decode_request(&bytes);
        let mut expected = [0; 32];
        expected.copy_from_slice(&digest);
        assert_eq!(
            decoded,
            Ok(RequestEvidence {
                request: vector.request,
                digest: expected
            }),
            "[golden] request vector {index}"
        );
        assert_eq!(
            codec::request_digest(&vector.request),
            expected,
            "[golden] request vector {index}"
        );
        assert_eq!(
            codec::encode_request(&vector.request),
            bytes,
            "[golden] request vector {index}"
        );
        assert!(matches!(declared(&bytes), 25 | 33));
    }
}

/// The vectors cover every record kind, every nested tag and both forms of
/// every option and boolean, every request operation, and zero, small,
/// patterned and maximal numbers.
#[test]
fn c03_vectors_cover_every_variant_tag_and_form() {
    let decoded: Vec<RecordEvidence> = record_frames()
        .iter()
        .map(|frame| codec::decode_record(frame).expect("a golden frame decodes"))
        .collect();
    let kinds: Vec<&str> = decoded.iter().map(|e| kind_name(&e.kind)).collect();
    for name in KIND_NAMES {
        assert!(kinds.contains(&name), "no vector for {name}");
    }
    let mut expectations = Vec::new();
    let mut slots = Vec::new();
    let mut settlements = Vec::new();
    let mut verdicts = Vec::new();
    let mut cancels = Vec::new();
    let mut failures = Vec::new();
    let mut facts = (false, false);
    let mut bools = Vec::new();
    let mut options = Vec::new();
    for evidence in &decoded {
        match evidence.kind {
            RecordKind::CaseStarted { expectation, .. } => expectations.push(expectation),
            RecordKind::ActionStarted { kind, .. } => slots.push(Some(kind)),
            RecordKind::ActionSettled { how, .. } => settlements.push(("action", how)),
            RecordKind::IncidentSettled { how, .. } => settlements.push(("incident", how)),
            RecordKind::CaseEnded { passed, .. } => bools.push(("passed", passed)),
            RecordKind::RecoveryAttempt { resolved, .. } => bools.push(("resolved", resolved)),
            RecordKind::RunEnded {
                verdict,
                resolved_by,
            } => {
                verdicts.push(verdict);
                options.push(("resolved by", resolved_by.is_some()));
            }
            RecordKind::IncidentOpened { kind, .. } => {
                slots.push(kind);
                options.push(("owner kind", kind.is_some()));
            }
            RecordKind::Control { fact } => match fact {
                ControlFact::AdmissionClosed { reason, .. } => {
                    facts.0 = true;
                    match reason {
                        ClosureReason::Cancelled(reason) => cancels.push(reason),
                        ClosureReason::Failed(class) => failures.push(class),
                    }
                }
                ControlFact::ShutdownRefused => facts.1 = true,
            },
            _ => {}
        }
    }
    for expectation in [
        Expectation::Clean,
        Expectation::RetainedBoundary,
        Expectation::OutputDetached,
    ] {
        assert!(expectations.contains(&expectation), "{expectation:?}");
    }
    for slot in [
        None,
        Some(SlotKind::Process),
        Some(SlotKind::Workspace),
        Some(SlotKind::Fixture),
    ] {
        assert!(slots.contains(&slot), "{slot:?}");
    }
    for owner in ["action", "incident"] {
        for how in [
            Settlement::Confirmed,
            Settlement::OutputLost,
            Settlement::NothingCreated,
            Settlement::NotAdmitted,
        ] {
            assert!(settlements.contains(&(owner, how)), "{owner} {how:?}");
        }
    }
    for verdict in [Verdict::Pending, Verdict::Passed, Verdict::Failed] {
        assert!(verdicts.contains(&verdict), "{verdict:?}");
    }
    for reason in [
        CancelReason::Requested,
        CancelReason::LeaseLost,
        CancelReason::Shutdown,
        CancelReason::Stop,
        CancelReason::RunBudget,
    ] {
        assert!(cancels.contains(&reason), "{reason:?}");
    }
    for class in FailureClass::ALL {
        assert!(failures.contains(&class), "{class:?}");
    }
    assert_eq!(facts, (true, true));
    for field in ["passed", "resolved"] {
        for value in [false, true] {
            assert!(bools.contains(&(field, value)), "{field} {value}");
        }
    }
    for field in ["resolved by", "owner kind"] {
        for present in [false, true] {
            assert!(options.contains(&(field, present)), "{field} {present}");
        }
    }
    let ops: Vec<RequestOp> = REQUEST_VECTORS.iter().map(|v| v.request.op).collect();
    assert!(ops.contains(&RequestOp::Shutdown));
    assert!(ops.contains(&RequestOp::Retry { epoch: 0 }));
    assert!(ops.contains(&RequestOp::Retry { epoch: u64::MAX }));
    let numbers: Vec<u64> = decoded.iter().map(|e| e.id.seq()).collect();
    for value in [0, 1, 0x0102_0304_0506_0708, u64::MAX] {
        assert!(numbers.contains(&value), "record sequence {value}");
    }
    let instants: Vec<u64> = decoded.iter().map(|e| e.at.0).collect();
    for value in [0, 1, 0x1122_3344_5566_7788, u64::MAX] {
        assert!(instants.contains(&value), "instant {value}");
    }
}

/// Decoding takes exactly one complete frame: every truncation is refused,
/// and so is anything after the frame (a zero byte, any other byte, the
/// same frame again, another frame).
#[test]
fn c04_decoding_takes_exactly_one_complete_frame() {
    let records = record_frames();
    let requests = request_frames();
    for (frame, record) in records
        .iter()
        .map(|frame| (frame, true))
        .chain(requests.iter().map(|frame| (frame, false)))
    {
        let decode = |bytes: &[u8]| -> Result<(), CodecError> {
            if record {
                codec::decode_record(bytes).map(|_| ())
            } else {
                codec::decode_request(bytes).map(|_| ())
            }
        };
        assert_eq!(decode(frame), Ok(()));
        for cut in 0..frame.len() {
            assert_eq!(
                decode(&frame[..cut]),
                Err(CodecError::Truncated),
                "[single-frame] a frame cut to {cut} of {} bytes",
                frame.len()
            );
        }
        let other = if record { &requests[0] } else { &records[0] };
        for tail in [vec![0x00], vec![0x5a], frame.clone(), other.clone()] {
            let mut bytes = frame.clone();
            bytes.extend_from_slice(&tail);
            assert_eq!(
                decode(&bytes),
                Err(CodecError::TrailingBytes { extra: tail.len() }),
                "[single-frame] {} trailing bytes were accepted",
                tail.len()
            );
        }
    }
}

/// The envelope is checked before any field: magic, domain, version, the
/// declared length (zero, beyond the limit, inconsistent with the bytes or
/// with the payload's own fields) and the overall size limit, which is
/// refused before anything is read.
#[test]
fn c05_envelope_refusals_come_before_any_field() {
    let frame = record_frame(|kind| matches!(kind, Want::Plain(RecordKind::RecoveryRequired)));
    let request = request_frames()[0].clone();
    for at in 0..4 {
        let mut bytes = frame.clone();
        bytes[at] ^= 0x20;
        assert_eq!(
            codec::decode_record(&bytes),
            Err(CodecError::BadMagic),
            "[envelope]"
        );
    }
    for found in [0x00, 0x51, 0x53, 0xff] {
        let bytes = resealed_with(&frame, 4, found);
        assert_eq!(
            codec::decode_record(&bytes),
            Err(CodecError::WrongDomain {
                expected: Domain::Record,
                found
            }),
            "[envelope]"
        );
    }
    for version in [0x00, 0x02, 0xff] {
        let bytes = resealed_with(&frame, 5, version);
        assert_eq!(
            codec::decode_record(&bytes),
            Err(CodecError::UnsupportedVersion { found: version }),
            "[envelope]"
        );
        let bytes = resealed_with(&request, 5, version);
        assert_eq!(
            codec::decode_request(&bytes),
            Err(CodecError::UnsupportedVersion { found: version }),
            "[envelope]"
        );
    }
    let with_length = |bytes: &[u8], length: u16| {
        let mut bytes = bytes.to_vec();
        bytes[6..8].copy_from_slice(&length.to_be_bytes());
        bytes
    };
    for length in [0, 4057, u16::MAX] {
        assert_eq!(
            codec::decode_record(&with_length(&frame, length)),
            Err(CodecError::BadLength { declared: length }),
            "[envelope]"
        );
    }
    let length = declared(&frame) as u16;
    assert_eq!(
        codec::decode_record(&with_length(&frame, length + 1)),
        Err(CodecError::Truncated),
        "[envelope]"
    );
    assert_eq!(
        codec::decode_record(&with_length(&frame, length - 1)),
        Err(CodecError::TrailingBytes { extra: 1 }),
        "[envelope]"
    );

    // A consistent envelope whose payload is longer, or shorter, than its
    // fields.
    let mut longer = frame[..codec::HEADER_LEN + declared(&frame)].to_vec();
    longer.push(0x00);
    longer.extend_from_slice(&[0; 32]);
    let mut longer = with_length(&longer, length + 1);
    reseal(&mut longer);
    assert_eq!(
        codec::decode_record(&longer),
        Err(CodecError::LongPayload { unused: 1 }),
        "[envelope]"
    );
    let started = record_frame(|kind| matches!(kind, Want::Plain(RecordKind::RunStarted { .. })));
    let cut = declared(&started) - 1;
    let mut shorter = started[..codec::HEADER_LEN + cut].to_vec();
    shorter.extend_from_slice(&[0; 32]);
    let mut shorter = with_length(&shorter, cut as u16);
    reseal(&mut shorter);
    assert_eq!(
        codec::decode_record(&shorter),
        Err(CodecError::ShortPayload),
        "[envelope]"
    );

    // Over the limit: refused by length alone, before the header is read.
    for len in [codec::MAX_FRAME + 1, 1 << 20] {
        let mut bytes = frame.clone();
        bytes.resize(len, 0x00);
        assert_eq!(
            codec::decode_record(&bytes),
            Err(CodecError::TooLong { len }),
            "[envelope]"
        );
        assert_eq!(
            codec::decode_request(&bytes),
            Err(CodecError::TooLong { len }),
            "[envelope]"
        );
    }
    // The largest permitted frame is bounded work and still refused when its
    // payload is not exactly its fields.
    let mut largest = frame[..codec::HEADER_LEN + declared(&frame)].to_vec();
    largest.resize(codec::MAX_FRAME, 0x00);
    let mut largest = with_length(&largest, codec::MAX_PAYLOAD as u16);
    reseal(&mut largest);
    assert_eq!(largest.len(), codec::MAX_FRAME);
    assert_eq!(
        codec::decode_record(&largest),
        Err(CodecError::LongPayload {
            unused: codec::MAX_PAYLOAD - 33
        }),
        "[envelope]"
    );
}

/// One tag position: what it is, a frame with it, its byte offset, the field
/// the refusal names, and three values that name nothing.
type TagPosition = (&'static str, Vec<u8>, usize, &'static str, [u8; 3]);

/// Every tag position refuses zero, the first unlisted value and the
/// largest byte, even with a matching digest (a forger can compute one);
/// booleans and option presence bytes accept only `00` and `01`, and a
/// presence byte must agree with what follows.
#[test]
fn c06_invalid_tags_booleans_and_presence_bytes_are_refused() {
    let tags: [TagPosition; 10] = [
        (
            "record kind",
            record_frame(|kind| matches!(kind, Want::Plain(RecordKind::RunStarted { .. }))),
            40,
            "record kind",
            [0x00, 0x0d, 0xff],
        ),
        (
            "expectation",
            record_frame(|kind| matches!(kind, Want::Plain(RecordKind::CaseStarted { .. }))),
            45,
            "expectation",
            [0x00, 0x04, 0xff],
        ),
        (
            "action slot kind",
            record_frame(|kind| matches!(kind, Want::ActionStarted(..))),
            65,
            "slot kind",
            [0x00, 0x04, 0xff],
        ),
        (
            "settlement",
            record_frame(|kind| matches!(kind, Want::ActionSettled(..))),
            65,
            "settlement",
            [0x00, 0x05, 0xff],
        ),
        (
            "verdict",
            record_frame(|kind| matches!(kind, Want::Plain(RecordKind::RunEnded { .. }))),
            41,
            "verdict",
            [0x00, 0x04, 0xff],
        ),
        (
            "control fact",
            record_frame(|kind| {
                matches!(
                    kind,
                    Want::Plain(RecordKind::Control {
                        fact: ControlFact::AdmissionClosed { .. }
                    })
                )
            }),
            41,
            "control fact",
            [0x00, 0x03, 0xff],
        ),
        (
            "closure reason",
            record_frame(|kind| {
                matches!(
                    kind,
                    Want::Plain(RecordKind::Control {
                        fact: ControlFact::AdmissionClosed { .. }
                    })
                )
            }),
            42,
            "closure reason",
            [0x00, 0x03, 0xff],
        ),
        (
            "cancel reason",
            record_frame(|kind| {
                matches!(
                    kind,
                    Want::Plain(RecordKind::Control {
                        fact: ControlFact::AdmissionClosed {
                            reason: ClosureReason::Cancelled(_),
                            ..
                        }
                    })
                )
            }),
            43,
            "cancel reason",
            [0x00, 0x06, 0xff],
        ),
        (
            "failure class",
            record_frame(|kind| {
                matches!(
                    kind,
                    Want::Plain(RecordKind::Control {
                        fact: ControlFact::AdmissionClosed {
                            reason: ClosureReason::Failed(_),
                            ..
                        }
                    })
                )
            }),
            43,
            "failure class",
            [0x00, 0x0d, 0xff],
        ),
        (
            "incident owner kind",
            record_frame(|kind| matches!(kind, Want::IncidentOpened(_, _, Some(_)))),
            90,
            "slot kind",
            [0x00, 0x04, 0xff],
        ),
    ];
    for (what, frame, at, field, values) in tags {
        for tag in values {
            assert_eq!(
                codec::decode_record(&resealed_with(&frame, at, tag)),
                Err(CodecError::UnknownTag { field, tag }),
                "[unknown-tag] {what} {tag:#04x}"
            );
        }
    }
    let retry = request_frames()[0].clone();
    for tag in [0x00, 0x03, 0xff] {
        assert_eq!(
            codec::decode_request(&resealed_with(&retry, 32, tag)),
            Err(CodecError::UnknownTag {
                field: "request operation",
                tag
            }),
            "[unknown-tag] request operation {tag:#04x}"
        );
    }

    let ended = record_frame(|kind| matches!(kind, Want::Plain(RecordKind::CaseEnded { .. })));
    let attempt =
        record_frame(|kind| matches!(kind, Want::Plain(RecordKind::RecoveryAttempt { .. })));
    for (frame, field) in [(ended, "case passed"), (attempt, "recovery resolved")] {
        for value in [0x02, 0x80, 0xff] {
            assert_eq!(
                codec::decode_record(&resealed_with(&frame, 45, value)),
                Err(CodecError::InvalidBool { field, value }),
                "[invalid-bool] {field} {value:#04x}"
            );
        }
    }
    let unresolved = record_frame(|kind| {
        matches!(
            kind,
            Want::Plain(RecordKind::RunEnded {
                resolved_by: None,
                ..
            })
        )
    });
    let resolved = record_frame(|kind| {
        matches!(
            kind,
            Want::Plain(RecordKind::RunEnded {
                resolved_by: Some(_),
                ..
            })
        )
    });
    let kindless = record_frame(|kind| matches!(kind, Want::IncidentOpened(_, _, None)));
    for (frame, at, field) in [
        (&unresolved, 42, "resolved by"),
        (&kindless, 89, "incident owner kind"),
    ] {
        for value in [0x02, 0xff] {
            assert_eq!(
                codec::decode_record(&resealed_with(frame, at, value)),
                Err(CodecError::InvalidPresence { field, value }),
                "[invalid-bool] presence of {field} {value:#04x}"
            );
        }
    }
    // A presence byte that disagrees with what follows.
    assert_eq!(
        codec::decode_record(&resealed_with(&unresolved, 42, 0x01)),
        Err(CodecError::ShortPayload)
    );
    assert_eq!(
        codec::decode_record(&resealed_with(&resolved, 42, 0x00)),
        Err(CodecError::LongPayload { unused: 4 })
    );
}

/// The digest binds every byte before it: changing any digest byte, or any
/// header or payload byte without recomputing the digest, is refused.
#[test]
fn c07_digest_and_payload_mutations_are_detected() {
    for (frame, record) in record_frames()
        .into_iter()
        .map(|frame| (frame, true))
        .chain(request_frames().into_iter().map(|frame| (frame, false)))
    {
        let decode = |bytes: &[u8]| -> Result<(), CodecError> {
            if record {
                codec::decode_record(bytes).map(|_| ())
            } else {
                codec::decode_request(bytes).map(|_| ())
            }
        };
        let covered = frame.len() - codec::DIGEST_LEN;
        for at in 0..frame.len() {
            for flip in [0x01, 0x80] {
                let mut bytes = frame.clone();
                bytes[at] ^= flip;
                let result = decode(&bytes);
                if at >= codec::HEADER_LEN {
                    assert_eq!(
                        result,
                        Err(CodecError::DigestMismatch),
                        "[digest-bound] byte {at} of {covered} covered changed"
                    );
                } else {
                    assert!(result.is_err(), "[digest-bound] header byte {at} changed");
                }
            }
        }
    }
}

/// Canonical and injective: changing any single payload byte (and resealing,
/// as a forger could) either yields a frame that is refused, or one that
/// decodes to a different value with a different digest and re-encodes to
/// exactly its own bytes. No byte is ignored, and no two byte strings mean
/// the same value.
#[test]
fn c08_every_payload_byte_is_bound_canonically() {
    let mut changed = 0;
    for frame in record_frames() {
        let original = codec::decode_record(&frame).expect("a golden frame decodes");
        for at in codec::HEADER_LEN..frame.len() - codec::DIGEST_LEN {
            for value in [frame[at] ^ 0x01, frame[at] ^ 0x80, 0x00, 0xff] {
                if value == frame[at] {
                    continue;
                }
                let bytes = resealed_with(&frame, at, value);
                if let Ok(evidence) = codec::decode_record(&bytes) {
                    changed += 1;
                    assert!(
                        (evidence.id, evidence.at, evidence.kind)
                            != (original.id, original.at, original.kind),
                        "[canonical] byte {at} set to {value:#04x} decoded to the same value"
                    );
                    assert_ne!(evidence.digest, original.digest, "[canonical]");
                    assert_eq!(
                        codec::encode_record(&intent_of(&evidence)),
                        Ok(bytes),
                        "[canonical] byte {at} set to {value:#04x}"
                    );
                }
            }
        }
    }
    for frame in request_frames() {
        let original = codec::decode_request(&frame).expect("a golden frame decodes");
        for at in codec::HEADER_LEN..frame.len() - codec::DIGEST_LEN {
            for value in [frame[at] ^ 0x01, frame[at] ^ 0x80, 0x00, 0xff] {
                if value == frame[at] {
                    continue;
                }
                let bytes = resealed_with(&frame, at, value);
                if let Ok(evidence) = codec::decode_request(&bytes) {
                    changed += 1;
                    assert_ne!(evidence.request, original.request, "[canonical]");
                    assert_ne!(evidence.digest, original.digest, "[canonical]");
                    assert_eq!(
                        codec::encode_request(&evidence.request),
                        bytes,
                        "[canonical]"
                    );
                }
            }
        }
    }
    assert!(
        changed > 1_000,
        "the mutations reached valid values: {changed}"
    );
}

/// Field sensitivity on typed values: changing any identity (a record's,
/// an embedded action's or incident's, generation or sequence alone), the
/// instant, the variant or any payload field changes the digest; every
/// distinct value has a distinct digest; and the same payload bytes under
/// the two domains have different digests.
#[test]
fn c09_digests_bind_every_identity_instant_variant_and_field() {
    let at = Tick(5);
    let base = evidence_with_id((G1, 1));
    let digest = |id: RecordId, at: Tick, kind: &RecordKind| codec::record_digest(id, at, kind);

    // Record identity: generation alone, sequence alone.
    let (zero, two) = (evidence_with_id((G0, 0)).id, evidence_with_id((G2, 0)).id);
    assert_ne!(
        digest(zero, at, &base.kind),
        digest(two, at, &base.kind),
        "[identity-bound] the record generation"
    );
    let (one, other) = (evidence_with_id((G1, 1)).id, evidence_with_id((G1, 2)).id);
    assert_ne!(
        digest(one, at, &base.kind),
        digest(other, at, &base.kind),
        "[identity-bound] the record sequence"
    );
    assert_ne!(
        digest(one, Tick(5), &base.kind),
        digest(one, Tick(6), &base.kind),
        "[field-bound] the instant"
    );

    // Embedded identities: generation alone, sequence alone.
    let action = |id: Id| -> ActionId {
        RECORD_VECTORS
            .iter()
            .filter_map(|vector| codec::decode_record(&whole(vector.frame, vector.digest)).ok())
            .find_map(|evidence| match evidence.kind {
                RecordKind::ActionStarted { action, .. }
                | RecordKind::ActionFailed { action }
                | RecordKind::ActionSettled { action, .. }
                | RecordKind::IncidentOpened { action, .. }
                    if action_id(action) == id =>
                {
                    Some(action)
                }
                _ => None,
            })
            .expect("a vector with that action")
    };
    let incident = |id: Id| -> IncidentId {
        RECORD_VECTORS
            .iter()
            .filter_map(|vector| codec::decode_record(&whole(vector.frame, vector.digest)).ok())
            .find_map(|evidence| match evidence.kind {
                RecordKind::IncidentOpened { incident, .. }
                | RecordKind::IncidentSettled { incident, .. }
                    if incident_id(incident) == id =>
                {
                    Some(incident)
                }
                _ => None,
            })
            .expect("a vector with that incident")
    };
    let started = |action: ActionId| RecordKind::ActionStarted {
        action,
        kind: SlotKind::Process,
    };
    assert_ne!(
        digest(one, at, &started(action((G2, 0)))),
        digest(one, at, &started(action((G0, 0)))),
        "[identity-bound] the embedded action generation"
    );
    assert_ne!(
        digest(one, at, &started(action((G1, 1)))),
        digest(one, at, &started(action((G1, 3)))),
        "[identity-bound] the embedded action sequence"
    );
    let settled = |incident: IncidentId| RecordKind::IncidentSettled {
        incident,
        how: Settlement::Confirmed,
    };
    assert_ne!(
        digest(one, at, &settled(incident((G0, 0)))),
        digest(one, at, &settled(incident((G2, 0)))),
        "[identity-bound] the embedded incident generation"
    );
    assert_ne!(
        digest(one, at, &settled(incident((G1, 1)))),
        digest(one, at, &settled(incident((G1, 2)))),
        "[identity-bound] the embedded incident sequence"
    );
    let opened = |incident: IncidentId, action: ActionId| RecordKind::IncidentOpened {
        incident,
        action,
        kind: None,
    };
    assert_ne!(
        digest(one, at, &opened(incident((G1, 1)), action((G2, 0)))),
        digest(one, at, &opened(incident((G1, 1)), action((G0, 0)))),
        "[identity-bound] an incident's action generation"
    );

    // Every variant and payload field: distinct values, distinct digests.
    let mut kinds: Vec<RecordKind> = record_frames()
        .iter()
        .map(|frame| codec::decode_record(frame).expect("decodes").kind)
        .collect();
    kinds.extend([
        RecordKind::RunStarted { dispositioned: 1 },
        RecordKind::CaseStarted {
            case: CaseId(1),
            expectation: Expectation::Clean,
        },
        RecordKind::RecoveryAttempt {
            attempt: 0,
            resolved: false,
        },
        RecordKind::RunEnded {
            verdict: Verdict::Failed,
            resolved_by: Some(1),
        },
        RecordKind::Control {
            fact: ControlFact::AdmissionClosed {
                reason: ClosureReason::Cancelled(CancelReason::Requested),
                after: 1,
            },
        },
    ]);
    let mut distinct: Vec<RecordKind> = Vec::new();
    for kind in kinds {
        if !distinct.contains(&kind) {
            distinct.push(kind);
        }
    }
    let digests: Vec<[u8; 32]> = distinct.iter().map(|kind| digest(one, at, kind)).collect();
    for (i, first) in digests.iter().enumerate() {
        for (j, second) in digests.iter().enumerate().skip(i + 1) {
            assert_ne!(
                first, second,
                "[field-bound] {:?} and {:?} share a digest",
                distinct[i], distinct[j]
            );
        }
    }

    // Requests: generation, sequence, operation and its epoch.
    let request = |generation: [u8; 16], seq: u64, op: RequestOp| {
        codec::request_digest(&Request {
            generation: Generation::new(generation),
            seq,
            op,
        })
    };
    let retry = RequestOp::Retry { epoch: 0 };
    assert_ne!(
        request(G0, 1, retry),
        request(G1, 1, retry),
        "[request-field-bound] generation"
    );
    assert_ne!(
        request(G1, 1, retry),
        request(G1, 2, retry),
        "[request-field-bound] sequence"
    );
    assert_ne!(
        request(G1, 1, retry),
        request(G1, 1, RequestOp::Shutdown),
        "[request-field-bound] operation"
    );
    assert_ne!(
        request(G1, 1, retry),
        request(G1, 1, RequestOp::Retry { epoch: 1 }),
        "[request-field-bound] epoch"
    );
}

/// No frame crosses domains: each decoder refuses the other domain's frame,
/// relabeling one without recomputing its digest is a digest mismatch, and
/// identical payload bytes under the two domains have different digests. A
/// frame relabeled and resealed is simply a new frame of the other domain:
/// the digest is integrity, never authentication.
#[test]
fn c10_frames_never_cross_domains() {
    for frame in record_frames() {
        assert_eq!(
            codec::decode_request(&frame),
            Err(CodecError::WrongDomain {
                expected: Domain::Request,
                found: 0x52
            }),
            "[domain-bound]"
        );
        let mut relabeled = frame.clone();
        relabeled[4] = Domain::Request.byte();
        assert!(matches!(
            codec::decode_request(&relabeled),
            Err(CodecError::DigestMismatch)
        ));
    }
    for frame in request_frames() {
        assert_eq!(
            codec::decode_record(&frame),
            Err(CodecError::WrongDomain {
                expected: Domain::Record,
                found: 0x51
            }),
            "[domain-bound]"
        );
        let mut relabeled = frame.clone();
        relabeled[4] = Domain::Record.byte();
        assert_eq!(
            codec::decode_record(&relabeled),
            Err(CodecError::DigestMismatch),
            "[domain-bound]"
        );
    }
    // A retry whose payload bytes also read as a complete record payload
    // (generation, sequence, an instant, then the `RecoveryRequired` tag).
    let request = REQUEST_VECTORS
        .iter()
        .find(|vector| {
            vector.request.op
                == RequestOp::Retry {
                    epoch: 0x0102_0304_0506_0708,
                }
        })
        .expect("the patterned retry");
    let bytes = whole(request.frame, request.digest);
    let as_record = resealed_with(&bytes, 4, Domain::Record.byte());
    let evidence = codec::decode_record(&as_record).expect("a forger can relabel and reseal");
    assert_eq!(evidence.kind, RecordKind::RecoveryRequired);
    assert_eq!(
        as_record[codec::HEADER_LEN..as_record.len() - codec::DIGEST_LEN],
        bytes[codec::HEADER_LEN..bytes.len() - codec::DIGEST_LEN],
        "the same payload bytes"
    );
    assert_ne!(
        evidence.digest,
        codec::request_digest(&request.request),
        "[domain-bound] the same payload under two domains shares a digest"
    );
}

/// The core's real records: every record a real run issues (all twelve
/// kinds) carries exactly the codec's digest of its typed fields, encodes
/// to a frame whose trailer is that digest, and decodes back to evidence
/// describing exactly that intent.
#[test]
fn c11_core_issued_records_encode_decode_and_check() {
    let run = Run::every_kind();
    let mut names = Vec::new();
    for intent in &run.journal.0 {
        assert_eq!(
            codec::record_digest(intent.id, intent.at, &intent.kind),
            intent.digest,
            "[core-digest] the core issued {:?} with another digest",
            intent.kind
        );
        let bytes = codec::encode_record(intent)
            .unwrap_or_else(|error| panic!("[core-digest] {:?}: {error:?}", intent.kind));
        let split = bytes.len() - codec::DIGEST_LEN;
        assert_eq!(bytes[split..], intent.digest);
        assert_eq!(sha(&bytes[..split]), intent.digest, "[core-digest]");
        let evidence = codec::decode_record(&bytes).expect("[core-digest] decodes");
        assert!(evidence.describes(intent), "[core-digest] {evidence:?}");
        names.push(kind_name(&intent.kind));
    }
    for name in KIND_NAMES {
        assert!(names.contains(&name), "the run issued no {name}");
    }
}

/// The real core binds acknowledgements to its own digests: evidence decoded
/// from altered bytes (resealed, so internally consistent) yields an
/// acknowledgement the core refuses, whatever field was altered, and an
/// intent whose digest is not its fields' is never encoded.
#[test]
fn c12_core_refuses_acknowledgements_for_altered_bytes() {
    let mut run = Run::new(Config::LIVE);
    let now = run.now();
    run.custody.start_run(now).expect("the run starts");
    run.ack_all();
    let now = run.now();
    run.custody
        .begin_case(CaseId(1), Expectation::Clean, now)
        .expect("the case begins");
    let now = run.now();
    assert!(
        run.custody
            .end_case(&mut Cleaner { confirm: true }, now)
            .expect("the case ends")
            .passed
    );
    run.flush();
    let next = run.journal.0[run.acked].clone();
    assert!(matches!(next.kind, RecordKind::CaseStarted { .. }));
    let genuine = codec::encode_record(&next).expect("encodes");
    let before = run.custody.snapshot().evidence.acknowledged;
    let alterations: [(&str, usize, AckOutcome); 6] = [
        ("generation", 8, AckOutcome::Foreign),
        ("sequence", 31, AckOutcome::OutOfOrder),
        ("sequence beyond", 30, AckOutcome::NotIssued),
        ("instant", 39, AckOutcome::Conflict),
        ("case", 44, AckOutcome::Conflict),
        ("expectation", 45, AckOutcome::Conflict),
    ];
    for (what, at, refused) in alterations {
        let value = match what {
            "sequence" => genuine[at] + 1,
            "expectation" => 0x02,
            _ => genuine[at] ^ 0x01,
        };
        let altered = resealed_with(&genuine, at, value);
        let evidence = codec::decode_record(&altered).expect("altered but consistent");
        assert!(!evidence.describes(&next), "{what}");
        let ack = RecordAck {
            id: evidence.id,
            digest: evidence.digest,
        };
        let now = run.now();
        assert_eq!(
            run.custody.acknowledge(&ack, now),
            refused,
            "[ack-bound] an acknowledgement for an altered {what} was not refused as such"
        );
        assert_eq!(run.custody.snapshot().evidence.acknowledged, before);
    }
    let evidence = codec::decode_record(&genuine).expect("decodes");
    let now = run.now();
    assert_eq!(
        run.custody.acknowledge(
            &RecordAck {
                id: evidence.id,
                digest: evidence.digest
            },
            now
        ),
        AckOutcome::Acknowledged
    );

    let mut forged = next.clone();
    forged.digest[0] ^= 0x01;
    assert_eq!(
        codec::encode_record(&forged),
        Err(CodecError::IntentDigestMismatch),
        "[intent-digest] an intent with another digest was encoded"
    );
    let mut forged = next.clone();
    forged.at = Tick(forged.at.0 + 1);
    assert_eq!(
        codec::encode_record(&forged),
        Err(CodecError::IntentDigestMismatch),
        "[intent-digest] an intent whose fields changed under its digest was encoded"
    );
}

/// The real core's request receipts bind the full payload: the same
/// request again is a duplicate with its first response; the same number
/// with any other operation or epoch is a conflict; another generation is
/// foreign; an old number whose receipt was evicted is a replay, never
/// executed again; a gap executes nothing.
#[test]
fn c13_request_receipts_bind_the_full_payload() {
    let config = Config {
        receipt_limit: 2,
        recovery_spacing_millis: 1,
        ..Config::LIVE
    };
    let (mut run, _ticket) = Run::recovering(config);
    let first = run.request(RequestOp::Retry { epoch: 0 });
    let response = Response::Retried {
        attempt: 1,
        resolved: false,
        epoch: 1,
    };
    assert_eq!(
        run.serve(first, false),
        RequestOutcome::Executed(response.clone())
    );
    assert_eq!(
        run.serve(first, false),
        RequestOutcome::Duplicate(response.clone())
    );
    for op in [RequestOp::Retry { epoch: 1 }, RequestOp::Shutdown] {
        let conflicting = Request { op, ..first };
        assert_eq!(
            run.serve(conflicting, false),
            RequestOutcome::Conflict,
            "[request-conflict] {op:?} under the same number"
        );
    }
    let foreign = Request {
        generation: Generation::new(G0),
        ..first
    };
    assert_eq!(run.serve(foreign, false), RequestOutcome::Foreign);
    let second = run.request(RequestOp::Shutdown);
    assert!(matches!(
        run.serve(second, false),
        RequestOutcome::Executed(Response::Shutdown(ShutdownDecision::Refused(_)))
    ));
    let third = run.request(RequestOp::Retry { epoch: 1 });
    assert!(matches!(
        run.serve(third, false),
        RequestOutcome::Executed(Response::Retried { attempt: 2, .. })
    ));
    assert!(matches!(
        run.serve(second, false),
        RequestOutcome::Duplicate(Response::Shutdown(_))
    ));
    assert_eq!(
        run.serve(first, false),
        RequestOutcome::Replayed,
        "[request-replay] an evicted request executed again"
    );
    let gap = Request { seq: 5, ..third };
    assert_eq!(run.serve(gap, false), RequestOutcome::Gap);
    assert_eq!(run.custody.snapshot().recovery.attempts, 2);
}

/// A deterministic, bounded malformed-input corpus (no fuzzing dependency):
/// random bytes, header-shaped bytes and mutated, cut, extended and spliced
/// golden frames, with and without recomputed digests. Neither decoder
/// panics, and whatever either accepts is exactly the canonical encoding of
/// what it decoded.
#[test]
fn c14_malformed_corpus_is_refused_or_canonical() {
    struct Rng(u64);
    impl Rng {
        fn next(&mut self) -> u64 {
            let mut x = self.0;
            x ^= x >> 12;
            x ^= x << 25;
            x ^= x >> 27;
            self.0 = x;
            x.wrapping_mul(0x2545_f491_4f6c_dd1d)
        }
        fn below(&mut self, bound: usize) -> usize {
            (self.next() % bound as u64) as usize
        }
        fn byte(&mut self) -> u8 {
            self.next() as u8
        }
    }
    let goldens: Vec<Vec<u8>> = record_frames()
        .into_iter()
        .chain(request_frames())
        .collect();
    let mut rng = Rng(0x0dd_c0de_5eed_1234);
    let (mut accepted, mut refused) = (0, 0);
    for _ in 0..40_000 {
        let mut input: Vec<u8> = match rng.below(6) {
            0 => (0..rng.below(160)).map(|_| rng.byte()).collect(),
            1 => {
                let mut bytes = codec::MAGIC.to_vec();
                bytes.push([0x52, 0x51, rng.byte()][rng.below(3)]);
                bytes.push([0x01, rng.byte()][rng.below(2)]);
                bytes.extend_from_slice(&(rng.below(200) as u16).to_be_bytes());
                bytes.extend((0..rng.below(240)).map(|_| rng.byte()));
                bytes
            }
            2 | 3 => {
                let mut bytes = goldens[rng.below(goldens.len())].clone();
                for _ in 0..=rng.below(4) {
                    let at = rng.below(bytes.len());
                    bytes[at] = rng.byte();
                }
                bytes
            }
            4 => {
                let mut bytes = goldens[rng.below(goldens.len())].clone();
                match rng.below(4) {
                    0 => bytes.truncate(rng.below(bytes.len())),
                    1 => bytes.extend((0..=rng.below(8)).map(|_| rng.byte())),
                    2 => {
                        let at = rng.below(bytes.len());
                        bytes.insert(at, rng.byte());
                    }
                    _ => {
                        let at = rng.below(bytes.len());
                        bytes.remove(at);
                    }
                }
                bytes
            }
            _ => {
                let mut bytes = codec::MAGIC.to_vec();
                bytes.push([0x52, 0x51][rng.below(2)]);
                bytes.push(0x01);
                let length = codec::MAX_PAYLOAD - 4 + rng.below(9);
                bytes.extend_from_slice(&(length as u16).to_be_bytes());
                let total = codec::MAX_FRAME - 4 + rng.below(9);
                bytes.extend((bytes.len()..total).map(|_| rng.byte()));
                bytes
            }
        };
        if rng.below(2) == 0 {
            reseal(&mut input);
        }
        let record = codec::decode_record(&input);
        let request = codec::decode_request(&input);
        if let Ok(evidence) = record {
            accepted += 1;
            assert_eq!(
                codec::encode_record(&intent_of(&evidence)),
                Ok(input.clone()),
                "[corpus] a record was accepted from non-canonical bytes"
            );
        }
        if let Ok(evidence) = request {
            accepted += 1;
            assert_eq!(
                codec::encode_request(&evidence.request),
                input,
                "[corpus] a request was accepted from non-canonical bytes"
            );
        }
        if record.is_err() && request.is_err() {
            refused += 1;
        }
    }
    assert!(
        accepted > 100 && refused > 30_000,
        "{accepted} accepted, {refused} refused"
    );
}

/// Decoding is plain data that can do nothing: its results own no heap,
/// handle or token; the codec's code names no custody, control, ticket,
/// reservation, lease capability, cleanup, recorder, validator or
/// acknowledgement and calls none of their operations, holds no global or
/// shared state and formats nothing; and decoding a live custody's records
/// (and garbage) leaves that custody exactly as it was.
#[test]
fn c15_decoding_is_plain_data_without_authority() {
    fn plain<T: Copy + Send + Sync + 'static>() {}
    plain::<RecordEvidence>();
    plain::<RequestEvidence>();
    plain::<CodecError>();

    let source = include_str!("support/custody/codec.rs");
    for pattern in [
        "process::",
        "abort(",
        "exit(",
        "mem::forget",
        "Box::leak",
        "ManuallyDrop",
        "into_raw",
        "impl Drop",
        "Drop for",
        "static mut",
        "OnceLock",
        "LazyLock",
        "thread_local!",
        "unsafe",
        "Rc<",
    ] {
        assert!(
            !source.contains(pattern),
            "[no-authority] codec.rs contains {pattern:?}"
        );
    }
    let forbidden = [
        "Custody",
        "Control",
        "Reservation",
        "OpTicket",
        "LeaseCap",
        "NoEffectProof",
        "NativeOutcome",
        "Cleanup",
        "DispositionValidator",
        "ValidatedDisposition",
        "IncidentBinding",
        "RecordAck",
        "RecordSink",
        "Resource",
        "acknowledge",
        "admit",
        "reserve",
        "complete",
        "lend",
        "end_entry",
        "end_case",
        "serve",
        "submit",
        "apply_disposition",
        "record_failed",
        "take_released",
        "unsafe",
        "thread_local",
        "OnceLock",
        "LazyLock",
        "Mutex",
        "RwLock",
        "RefCell",
        "Cell",
        "Arc",
        "Rc",
        "process",
        "fs",
        "net",
        "env",
        "format",
        "write",
        "print",
        "println",
        "eprintln",
        "to_string",
    ];
    for (number, line) in source.lines().enumerate() {
        let code = line.trim_start();
        if code.starts_with("//") {
            continue;
        }
        assert!(
            !code.starts_with("static ") && !code.starts_with("pub static "),
            "[no-authority] codec.rs:{} declares a global",
            number + 1
        );
        assert!(!code.contains("{:?}"), "codec.rs:{} formats", number + 1);
        // `RecordKind::Control` is a record variant, not the control handle.
        let code = code.replace("RecordKind::Control", "RecordKind::");
        for token in code.split(|c: char| !c.is_ascii_alphanumeric() && c != '_') {
            assert!(
                !forbidden.contains(&token),
                "[no-authority] codec.rs:{} uses {token}",
                number + 1
            );
        }
    }

    let mut run = Run::every_kind();
    let frames: Vec<Vec<u8>> = run
        .journal
        .0
        .iter()
        .map(|intent| codec::encode_record(intent).expect("encodes"))
        .collect();
    let snapshot = run.custody.snapshot();
    let status = run.control.status();
    for frame in &frames {
        codec::decode_record(frame).expect("decodes");
        let _ = codec::decode_request(frame);
        let _ = codec::decode_record(&frame[..frame.len() / 2]);
    }
    for frame in record_frames().iter().chain(request_frames().iter()) {
        let _ = codec::decode_record(frame);
        let _ = codec::decode_request(frame);
    }
    assert_eq!(*run.custody.snapshot(), *snapshot, "[no-authority]");
    assert_eq!(run.control.status(), status, "[no-authority]");
    assert_eq!(
        run.custody.take_released().len(),
        snapshot.released_waiting,
        "[no-authority] the owners waiting to be handed back are the run's own"
    );
}

/// Supplementary (source, not behavior): the core computes its record and
/// request digests through the codec, in one place each, and no other
/// hashing or formatting path for them remains in the custody sources.
#[test]
fn c16_core_digest_paths_use_the_codec() {
    let core = include_str!("support/custody/core.rs");
    let model = include_str!("support/custody/model.rs");
    let module = include_str!("support/custody/mod.rs");
    assert_eq!(
        core.matches("codec::record_digest(id, at, &kind)").count(),
        1,
        "[digest-wiring] the record path"
    );
    assert_eq!(
        core.matches("codec::request_digest(request)").count(),
        1,
        "[digest-wiring] the request path"
    );
    let push = core.find("    fn push(&mut self, at: Tick, kind: RecordKind) -> RecordId {");
    let sequence = core.find("    fn sequence<C: Cleanup<R>>(");
    let record_call = core.find("codec::record_digest(id, at, &kind)");
    let request_call = core.find("codec::request_digest(request)");
    assert!(push < record_call && push.is_some(), "[digest-wiring]");
    assert!(
        sequence < request_call && sequence.is_some(),
        "[digest-wiring]"
    );
    for (name, source) in [("core.rs", core), ("model.rs", model), ("mod.rs", module)] {
        for pattern in ["Sha256", "sha2", "{kind:?}\")", "request.op)"] {
            assert!(
                !source.contains(pattern),
                "[digest-wiring] {name} still contains {pattern}"
            );
        }
    }
    assert!(module.contains("pub mod codec;"));
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// The digest the real core retains in each request receipt is the
/// canonical one (I2-R1, finding A). Real requests go through
/// `Control::submit`, `Custody::serve`, sequencing and execution in three
/// custodies: A requires recovery (its retries run as real attempts), B has
/// another generation, C has A's. The digest each retained receipt holds,
/// read through a read-only observation compiled for tests only, must be the
/// digest of that request's version-1 frame, given literally above by the
/// independent derivation and re-derived here from the frame with the sha2
/// crate. Generation, sequence, operation and epoch are each bound: requests
/// differing in exactly one of them are retained under different digests. A
/// duplicate or a conflicting request leaves the retained digest as it was.
#[test]
fn c17_core_retains_the_canonical_request_digest_in_its_receipts() {
    assert_eq!(GENERATION.bytes(), G3C);
    let canonical = |request: &Request| -> [u8; 32] {
        let vector = RECEIPT_VECTORS
            .iter()
            .find(|vector| vector.request == *request)
            .unwrap_or_else(|| panic!("no receipt vector for {request:?}"));
        let mut digest = [0; 32];
        digest.copy_from_slice(&unhex(vector.digest));
        assert_eq!(
            sha(&unhex(vector.frame)),
            digest,
            "the literal is its frame's digest"
        );
        digest
    };
    let config = Config {
        recovery_spacing_millis: 1,
        ..Config::LIVE
    };

    let (mut a, _ticket) = Run::recovering(config);
    let mut a_requests = Vec::new();
    for op in [
        RequestOp::Retry { epoch: 0 },
        RequestOp::Shutdown,
        RequestOp::Retry { epoch: 1 },
        RequestOp::Retry { epoch: u64::MAX },
        RequestOp::Retry { epoch: 0 },
    ] {
        let request = a.request(op);
        assert!(
            matches!(a.serve(request, false), RequestOutcome::Executed(_)),
            "{request:?}"
        );
        a_requests.push(request);
    }
    assert_eq!(a.custody.snapshot().recovery.attempts, 2, "two retries ran");
    let first = a_requests[0];
    assert!(matches!(
        a.serve(first, false),
        RequestOutcome::Duplicate(_)
    ));
    let conflicting = Request {
        op: RequestOp::Shutdown,
        ..first
    };
    assert_eq!(a.serve(conflicting, false), RequestOutcome::Conflict);
    assert_eq!(a.custody.retained_request_digest(6), None);

    let mut b = Run::with(config, Generation::new(GA5));
    let b_request = b.request(RequestOp::Retry { epoch: 0 });
    assert!(matches!(
        b.serve(b_request, false),
        RequestOutcome::Executed(_)
    ));
    let mut c = Run::with(config, GENERATION);
    let mut c_requests = Vec::new();
    for op in [
        RequestOp::Shutdown,
        RequestOp::Retry { epoch: 0 },
        RequestOp::Retry { epoch: 2 },
    ] {
        let request = c.request(op);
        assert!(
            matches!(c.serve(request, false), RequestOutcome::Executed(_)),
            "{request:?}"
        );
        c_requests.push(request);
    }

    let retained = |label: &str, run: &Run, requests: &[Request]| -> Vec<[u8; 32]> {
        requests
            .iter()
            .map(|request| {
                let expected = canonical(request);
                let stored = run.custody.retained_request_digest(request.seq);
                assert_eq!(
                    stored,
                    Some(expected),
                    "[request-receipt] custody {label} retained {} for {request:?}, not its \
                     canonical digest {}",
                    stored.map_or_else(|| "nothing".to_string(), |digest| hex(&digest)),
                    hex(&expected)
                );
                expected
            })
            .collect()
    };
    let a_digests = retained("A", &a, &a_requests);
    let b_digests = retained("B", &b, &[b_request]);
    let c_digests = retained("C", &c, &c_requests);

    let pairs = [
        (
            "generation",
            a_requests[0],
            b_request,
            a_digests[0],
            b_digests[0],
        ),
        (
            "sequence",
            a_requests[0],
            a_requests[4],
            a_digests[0],
            a_digests[4],
        ),
        (
            "operation",
            a_requests[0],
            c_requests[0],
            a_digests[0],
            c_digests[0],
        ),
        (
            "operation",
            a_requests[1],
            c_requests[1],
            a_digests[1],
            c_digests[1],
        ),
        (
            "epoch",
            a_requests[2],
            c_requests[2],
            a_digests[2],
            c_digests[2],
        ),
    ];
    for (field, left, right, left_digest, right_digest) in pairs {
        let differing = [
            left.generation != right.generation,
            left.seq != right.seq,
            std::mem::discriminant(&left.op) != std::mem::discriminant(&right.op),
            left.op != right.op
                && matches!(left.op, RequestOp::Retry { .. })
                && matches!(right.op, RequestOp::Retry { .. }),
        ];
        assert_eq!(
            differing.iter().filter(|differs| **differs).count(),
            1,
            "{left:?} and {right:?} differ in exactly the {field}"
        );
        assert_ne!(
            left_digest, right_digest,
            "[request-receipt] requests differing only in the {field} share a retained digest"
        );
    }
}

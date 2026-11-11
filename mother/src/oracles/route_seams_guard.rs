//! SB-3E U4a-3: Mother's route guard (Step 0 item 8; AM-2 rev 4.2 A.2 item
//! 3 and rev 3 A.5; SB3E-R39.1, R40.6). It reads source text only, in the
//! standard `cargo test -p nexus-mother`, where no seam is compiled:
//! - every S5, S3-forwarder and S4b name sits at an audited site, each gated
//!   exactly by `#[cfg(mother_route_seams)]` (inventory, G2 leaks, pins and
//!   the generated sweep), with its marker and its test hooks unwidened;
//! - `route_cells/s5.rs` makes one counted model call and keeps no port;
//! - the harness `timeout` cannot unwind and ends in `std::process::abort`.
//!
//! The lexer is governed-control's (`model_call_tests.rs`), byte for byte.

const TEST: &str = "#[cfg(test)]";
const ROUTE: &str = "#[cfg(mother_route_seams)]";
const GC: &str =
    include_str!("../../../crates/nexus-governed-control/src/host_session/model_call_tests.rs");
const LEXER: [&str; 10] = [
    "seam_sites",
    "leaks_under",
    "mask",
    "test_only",
    "starts",
    "lex",
    "flat",
    "pinned",
    "header",
    "enclosing",
];
const S5: &str = "host/tests/route_cells/s5.rs";
/// Every name of Mother's route seams outside `route_seams.rs` itself.
const NAMES: [&str; 4] = [
    "route_seams",
    "route_ctx",
    "route_revoke_original_grant",
    "route_poison_coordinator",
];
/// Mother's route markers, each exactly once (SB3E-R32.5, R40.6).
const MARKERS: [&str; 3] = [
    "nexus-mother-route-seam-S3-port",
    "nexus-mother-route-seam-S4b",
    "nexus-mother-route-seam-S5",
];
/// The test hooks beside the sites stay exactly `#[cfg(test)]` (mission 11).
const HOOKS: [(&str, &str); 5] = [
    ("src/adapter/mod.rs", "model_record"),
    ("src/adapter/session_port.rs", "inner_for_test"),
    ("src/coordinator.rs", "fence_to_marker"),
    ("src/coordinator/lite.rs", "after_engagement"),
    ("src/coordinator/model_calls.rs", "hooks"),
];
/// Every route name's site: `file: [fn ]name < enclosing chain` (as AM-2 B.2).
const SITES: &str = "\
src/adapter/mod.rs: route_seams < fnmodel_call(&self,mission:MissionId,request:&ModelRequest,meter:&dynCallMeter,deadline:&dynCallDeadline,invocation:&InvocationObserver,)->Result<ModelAttempt,EffectRefusal>{>implEffectPortforGovernedPort{\n\
src/adapter/mod.rs: route_seams < fnmodel_call(&self,mission:MissionId,request:&ModelRequest,meter:&dynCallMeter,deadline:&dynCallDeadline,invocation:&InvocationObserver,)->Result<ModelAttempt,EffectRefusal>{>implEffectPortforGovernedPort{\n\
src/adapter/session_port.rs: fn route_revoke_original_grant < #[cfg(mother_route_seams)]implSessionPort{\n\
src/adapter/session_port.rs: route_revoke_original_grant < match&self.inner.door{>pubfnroute_revoke_original_grant(&self)->bool{>#[cfg(mother_route_seams)]implSessionPort{\n\
src/coordinator.rs: route_seams < =&entry.event{>fnwrite_record(&mutself,mission:MissionId,event:Event,poison:bool,)->Result<u64,CoordinatorError>{>implInner{\n\
src/coordinator.rs: route_seams < =&entry.event{>fnwrite_record(&mutself,mission:MissionId,event:Event,poison:bool,)->Result<u64,CoordinatorError>{>implInner{\n\
src/coordinator.rs: route_seams < =&entry.event{>fnwrite_record(&mutself,mission:MissionId,event:Event,poison:bool,)->Result<u64,CoordinatorError>{>implInner{\n\
src/coordinator.rs: route_seams < =&entry.event{>fnwrite_record(&mutself,mission:MissionId,event:Event,poison:bool,)->Result<u64,CoordinatorError>{>implInner{\n\
src/coordinator.rs: route_seams < for(index,role)inroles.iter().enumerate(){>fnrun_lanes(&self,id:MissionId,generation:u64,roles:&[LaneRole],report:&mutAdvance,)->Result<Ended,CoordinatorError>{>impl<P:EffectPort>Coordinator<P>{\n\
src/coordinator.rs: route_seams < for(index,role)inroles.iter().enumerate(){>fnrun_lanes(&self,id:MissionId,generation:u64,roles:&[LaneRole],report:&mutAdvance,)->Result<Ended,CoordinatorError>{>impl<P:EffectPort>Coordinator<P>{\n\
src/coordinator.rs: route_seams < letcontract=self.step(|inner|{>for(index,role)inroles.iter().enumerate(){>fnrun_lanes(&self,id:MissionId,generation:u64,roles:&[LaneRole],report:&mutAdvance,)->Result<Ended,CoordinatorError>{>impl<P:EffectPort>Coordinator<P>{\n\
src/coordinator.rs: route_seams < letcontract=self.step(|inner|{>for(index,role)inroles.iter().enumerate(){>fnrun_lanes(&self,id:MissionId,generation:u64,roles:&[LaneRole],report:&mutAdvance,)->Result<Ended,CoordinatorError>{>impl<P:EffectPort>Coordinator<P>{\n\
src/coordinator/lite.rs: fn route_poison_coordinator < #[cfg(mother_route_seams)]impl<P:EffectPort>FileSession<P>{\n\
src/coordinator/model_calls.rs: route_seams < \n\
src/coordinator/model_calls.rs: fn route_ctx < #[cfg(mother_route_seams)]impl<P:EffectPort>BorrowedMeter<'_,P>{\n\
src/coordinator/model_calls.rs: route_seams < #[cfg(mother_route_seams)]impl<P:EffectPort>BorrowedMeter<'_,P>{\n\
src/coordinator/model_calls.rs: route_seams < fnroute_ctx(&self)->route_seams::Ctx{>#[cfg(mother_route_seams)]impl<P:EffectPort>BorrowedMeter<'_,P>{\n\
src/coordinator/model_calls.rs: route_seams < fncharge(&self)->Result<Charged,MeterRefusal>{>impl<P:EffectPort>CallMeterforBorrowedMeter<'_,P>{\n\
src/coordinator/model_calls.rs: route_ctx < fncharge(&self)->Result<Charged,MeterRefusal>{>impl<P:EffectPort>CallMeterforBorrowedMeter<'_,P>{\n\
src/coordinator/model_calls.rs: route_seams < c.try_with_lock(|inner|{>fncharge(&self)->Result<Charged,MeterRefusal>{>impl<P:EffectPort>CallMeterforBorrowedMeter<'_,P>{\n\
src/coordinator/model_calls.rs: route_ctx < c.try_with_lock(|inner|{>fncharge(&self)->Result<Charged,MeterRefusal>{>impl<P:EffectPort>CallMeterforBorrowedMeter<'_,P>{\n\
src/coordinator/model_calls.rs: route_seams < c.try_with_lock(|inner|{>fncharge(&self)->Result<Charged,MeterRefusal>{>impl<P:EffectPort>CallMeterforBorrowedMeter<'_,P>{\n\
src/coordinator/model_calls.rs: route_ctx < c.try_with_lock(|inner|{>fncharge(&self)->Result<Charged,MeterRefusal>{>impl<P:EffectPort>CallMeterforBorrowedMeter<'_,P>{\n\
src/coordinator/model_calls.rs: route_seams < fnsettle(&self,call:u32){>impl<P:EffectPort>CallMeterforBorrowedMeter<'_,P>{\n\
src/coordinator/model_calls.rs: route_ctx < fnsettle(&self,call:u32){>impl<P:EffectPort>CallMeterforBorrowedMeter<'_,P>{\n\
src/coordinator/model_calls.rs: route_seams < fncall(&self,request:&ModelRequest)->Result<ModelReply,ModelCallError>{>impl<P:EffectPort>ModelCallerforCaller<'_,P>{\n\
src/coordinator/model_calls.rs: route_ctx < fncall(&self,request:&ModelRequest)->Result<ModelReply,ModelCallError>{>impl<P:EffectPort>ModelCallerforCaller<'_,P>{\n\
src/lib.rs: route_seams < \n\
";
/// A.2 item 3 (c), R40.6: each `SessionPort` or `SessionDoor` identifier in
/// `s5.rs`, with three tokens either side (no kept port, door or clone).
const PORT_USES: &str = "\
,FilePrivate,|SessionPort|};use\n\
)door:|SessionDoor|,pub(\n\
&FileSession<|SessionPort|>,folder\n\
:FileSession<|SessionPort|>,id\n\
:new(|SessionPort|::new\n\
";
/// The reviewed regions, whole and normalized: every site with its
/// neighbours (the two `mother.admit` conditions whole, rev 3 A.5), the S3
/// forwarder and S4b bodies, S5's module, and `s5.rs`'s counted call, its
/// one `Coordinator::open` and the harness `timeout`.
const PINS: [(&str, &str); 21] = [
    (
        "src/lib.rs",
        r##"
pub mod private;
#[cfg(mother_route_seams)]
pub mod route_seams;
pub mod state;
"##,
    ),
    (
        "src/coordinator/model_calls.rs",
        r##"
use super::*;
#[cfg(mother_route_seams)]
use crate::route_seams;
use nexus_governed_control::inference::{
"##,
    ),
    (
        "src/coordinator/model_calls.rs",
        r##"
#[cfg(mother_route_seams)]
impl<P: EffectPort> BorrowedMeter<'_, P> {
    fn route_ctx(&self) -> route_seams::Ctx {
        let context = self.coordinator.port.session_context();
        route_seams::Ctx::of(context.as_ref()).invocation(self.invocation)
    }
}
impl<P: EffectPort> CallMeter for BorrowedMeter<'_, P> {
"##,
    ),
    (
        "src/coordinator/model_calls.rs",
        r##"
let mut result = Err(MeterRefusal::Stale);
#[cfg(mother_route_seams)]
route_seams::at("mother.charge.entry", self.route_ctx());
c.try_with_lock(|inner| {
"##,
    ),
    (
        "src/coordinator/model_calls.rs",
        r##"
let Ok(charged) = result else {
    return Ok(());
};
result = Err(MeterRefusal::NotRecorded);
#[cfg(mother_route_seams)]
route_seams::at("mother.charge", self.route_ctx());
inner.append(
    self.invocation.mission,
    Event::ModelCallCharged {
        invocation: self.invocation,
        call: charged.call,
    },
)?;
#[cfg(mother_route_seams)]
route_seams::at("mother.charge.ok", self.route_ctx());
let m = inner.mission(self.invocation.mission)?;
"##,
    ),
    (
        "src/coordinator/model_calls.rs",
        r##"
fn settle(&self, call: u32) {
    let c = self.coordinator;
    #[cfg(mother_route_seams)]
    route_seams::at("mother.settle", self.route_ctx());
    let _ =
        c.try_with_lock(|i| {
            if i.usable().is_err() {
"##,
    ),
    (
        "src/coordinator/model_calls.rs",
        r##"
#[cfg(test)]
c.hooks.model_returned.fire();
#[cfg(mother_route_seams)]
route_seams::at("mother.recheck", self.meter.route_ctx());
if self.live() && self.deadline.expired() {
"##,
    ),
    (
        "src/coordinator.rs",
        r##"
#[cfg(all(test, target_os = "linux"))]
lite::session_terminal_fix2::before(&entry);
#[cfg(mother_route_seams)]
if let Event::LaneReturned { invocation, .. } = &entry.event {
    let ctx = crate::route_seams::Ctx::of(self.session_context.as_ref());
    crate::route_seams::at("mother.admit", ctx.invocation(*invocation));
}
if work {
    self.admit_session(nexus_governed_control::host_session::MotherStep::Transition)?;
}
#[cfg(mother_route_seams)]
if let Event::LaneReturned { invocation, .. } = &entry.event {
    let ctx = crate::route_seams::Ctx::of(self.session_context.as_ref());
    crate::route_seams::at("mother.admit.ok", ctx.invocation(*invocation));
}
"##,
    ),
    (
        "src/coordinator.rs",
        r##"
let hash = output_hash(&output);
#[cfg(mother_route_seams)]
crate::route_seams::at(
    "mother.lane.prelock",
    crate::route_seams::Ctx::of(self.port.session_context().as_ref())
        .invocation(input.invocation),
);
let contract = self.step(|inner| {
    inner.kept(id)?;
"##,
    ),
    (
        "src/coordinator.rs",
        r##"
inner.append(
    id,
    Event::LaneReturned {
        invocation: input.invocation,
        output_hash: hash.clone(),
    },
)?;
#[cfg(mother_route_seams)]
crate::route_seams::at(
    "mother.lane.returned",
    crate::route_seams::Ctx::of(inner.session_context.as_ref())
        .invocation(input.invocation),
);
let mission = inner.mission(id)?;
mission.pending_lane = None;
"##,
    ),
    (
        "src/adapter/mod.rs",
        r##"
let record = outcome.record();
#[cfg(test)]
let record = self.model_record.lock().unwrap().take().unwrap_or(record);
#[cfg(mother_route_seams)]
crate::route_seams::at(
    "mother.classify",
    crate::route_seams::Ctx::of(self.door.context().as_ref()).run(run),
);
Ok(classify(record, outcome.into_result()))
"##,
    ),
    (
        "src/adapter/session_port.rs",
        r##"
#[cfg(mother_route_seams)]
impl SessionPort {
    pub fn route_revoke_original_grant(&self) -> bool {
        const M: &str = "nexus-mother-route-seam-S3-port";
        std::hint::black_box(M.as_bytes());
        match &self.inner.door {
            PortDoor::Session(door) => door.route_revoke_original_grant(),
            PortDoor::Legacy(_) => false,
        }
    }
}
"##,
    ),
    (
        "src/coordinator/lite.rs",
        r##"
#[cfg(mother_route_seams)]
impl<P: EffectPort> FileSession<P> {
    pub fn route_poison_coordinator(&self) {
        const M: &str = "nexus-mother-route-seam-S4b";
        std::hint::black_box(M.as_bytes());
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _held = self.coordinator.inner.try_lock();
            panic!("route seam S4b: coordinator lock poisoned");
        }));
    }
}
"##,
    ),
    (
        "src/route_seams.rs",
        r##"
const M: &str = "nexus-mother-route-seam-S5";
const PATIENCE: Duration = Duration::from_secs(10);
pub(crate) const POINTS: [&str; 10] = [
    "mother.charge.entry",
    "mother.charge",
    "mother.charge.ok",
    "mother.settle",
    "mother.classify",
    "mother.recheck",
    "mother.lane.prelock",
    "mother.admit",
    "mother.admit.ok",
    "mother.lane.returned",
];
"##,
    ),
    (
        "src/route_seams.rs",
        r##"
#[derive(Clone, Copy, Default)]
pub(crate) struct Ctx {
    session: Option<SessionIdentity>,
    request: Option<u64>,
    run: Option<RunId>,
    invocation: Option<InvocationId>,
}
impl Ctx {
    pub(crate) fn of(context: Option<&SessionContext>) -> Self {
        Self {
            session: context.map(SessionContext::session),
            request: context.map(SessionContext::request),
            ..Self::default()
        }
    }
    pub(crate) fn invocation(self, invocation: InvocationId) -> Self {
        let invocation = Some(invocation);
        Self { invocation, ..self }
    }
    pub(crate) fn run(self, run: RunId) -> Self {
        let run = Some(run);
        Self { run, ..self }
    }
}
"##,
    ),
    (
        "src/route_seams.rs",
        r##"
pub type Timeout = fn(&str) -> !;

pub enum Action {
    Trace,
    Pause {
        cell: &'static str,
        timeout: Timeout,
    },
    Panic,
}
"##,
    ),
    (
        "src/route_seams.rs",
        r##"
struct Release(u64, SyncSender<()>);
impl Drop for Release {
    fn drop(&mut self) {
        state().pending.retain(|p| p.id != self.0);
        let _ = self.1.try_send(());
    }
}

pub fn arm(point: &'static str, action: Action) -> Held {
    assert!(POINTS.contains(&point), "not a Mother route point: {point}");
    let (ack, acknowledged) = sync_channel(1);
    let (release, released) = sync_channel(1);
    let mut s = state();
    s.tick += 1;
    let id = s.tick;
    let pause = match action {
        Action::Trace => None,
        Action::Pause { cell, timeout } => Some(Some((released, cell, timeout))),
        Action::Panic => Some(None),
    };
    if let Some(pause) = pause {
        s.pending.push(Pending {
            id,
            point,
            ack,
            pause,
        });
    }
    ARMED.store(true, Ordering::SeqCst);
    Held {
        acknowledged,
        _release: Release(id, release),
    }
}

pub fn drain_trace() -> Vec<Entry> {
    std::mem::take(&mut state().entries)
}

pub(crate) fn at(point: &'static str, ctx: Ctx) {
    std::hint::black_box(M.as_bytes());
    if !ARMED.load(Ordering::Relaxed) {
        return;
    }
    fire(point, ctx);
}
"##,
    ),
    (
        "src/route_seams.rs",
        r##"
fn fire(point: &'static str, ctx: Ctx) {
    let (entry, pending) = {
        let mut s = state();
        s.tick += 1;
        let entry = Entry {
            tick: s.tick,
            point,
            session: ctx.session,
            request: ctx.request,
            run: ctx.run,
            invocation: ctx.invocation,
            thread: std::thread::current().id(),
            instant: Instant::now(),
        };
        s.entries.push(entry.clone());
        let found = s.pending.iter().position(|p| p.point == point);
        (entry, found.map(|i| s.pending.remove(i)))
    };
    let Some(pending) = pending else { return };
    let acknowledged = pending.ack.try_send(entry).is_ok();
    match pending.pause {
        None => panic!("route seam: armed panic at {point}"),
        Some((released, cell, timeout)) => {
            if !acknowledged || released.recv_timeout(PATIENCE).is_err() {
                timeout(&format!("TIMEOUT {cell} {point}"));
            }
        }
    }
}
"##,
    ),
    (
        "host/tests/route_cells/s5.rs",
        r##"
fn drive_call(calls: &AtomicU32, context: &LaneContext<'_>) -> bool {
    let schema = ResponseSchema::new(r#"{"type":"object"}"#).unwrap();
    let request = ModelRequest::new("system".into(), "prompt".into(), schema).unwrap();
    calls.fetch_add(1, Ordering::SeqCst);
    context
        .model
        .expect("a model caller")
        .call(&request)
        .is_ok()
}
"##,
    ),
    (
        "host/tests/route_cells/s5.rs",
        r##"
let m = Coordinator::open(
    journal,
    Arc::new(SessionPort::new(b.door, b.name.clone())),
    lanes(&calls, &recorded),
    clock,
    private,
)
.unwrap()
.into_file_session()
.unwrap();
"##,
    ),
    (
        "host/tests/route_cells/s5.rs",
        r##"
fn timeout(line: &str) -> ! {
    use std::io::Write;
    let Some(root) = std::env::var_os(CHILD).map(PathBuf::from) else {
        std::process::abort() // SB3E-R40.5: never relative to the cwd.
    };
    let mut open = std::fs::OpenOptions::new();
    if let Ok(mut result) = open.create(true).append(true).open(root.join("result")) {
        let _ = writeln!(result, "{line}");
    }
    std::process::abort()
}
"##,
    ),
];

fn seam_sites(source: &str, seams: &[&str]) -> Vec<usize> {
    let code = mask(source);
    let word = |i: usize| {
        code.as_bytes()
            .get(i)
            .is_some_and(|b| b.is_ascii_alphanumeric() || *b == b'_')
    };
    let mut sites = Vec::new();
    for seam in seams {
        for (at, _) in code.match_indices(seam) {
            if !word(at + seam.len()) && !at.checked_sub(1).is_some_and(word) {
                sites.push(at);
            }
        }
    }
    sites
}
fn leaks_under(source: &str, seams: &[&str], attr: &str) -> usize {
    let code = mask(source);
    seam_sites(source, seams)
        .into_iter()
        .filter(|&at| !test_only(&code, at, attr))
        .count()
}
fn mask(source: &str) -> String {
    let b = source.as_bytes();
    let mut out = b.to_vec();
    let mut i = 0;
    let blank = |out: &mut Vec<u8>, from: usize, to: usize| {
        for c in &mut out[from..to.min(b.len())] {
            if *c != b'\n' {
                *c = b' ';
            }
        }
    };
    while i < b.len() {
        if b[i..].starts_with(b"//") {
            let end = b[i..]
                .iter()
                .position(|&c| c == b'\n')
                .map_or(b.len(), |n| i + n);
            blank(&mut out, i, end);
            i = end;
        } else if b[i..].starts_with(b"/*") {
            let (mut depth, mut j) = (1, i + 2);
            while j < b.len() && depth > 0 {
                if b[j..].starts_with(b"/*") {
                    (depth, j) = (depth + 1, j + 2);
                } else if b[j..].starts_with(b"*/") {
                    (depth, j) = (depth - 1, j + 2);
                } else {
                    j += 1;
                }
            }
            blank(&mut out, i, j);
            i = j;
        } else if b[i] == b'r'
            && (b[i + 1..].starts_with(b"#") || b[i + 1..].starts_with(b"\""))
            && (i == 0 || !b[i - 1].is_ascii_alphanumeric() || b[i - 1] == b'b')
        {
            let hashes = b[i + 1..].iter().take_while(|&&c| c == b'#').count();
            if b.get(i + 1 + hashes) != Some(&b'"') {
                i += 1;
                continue;
            }
            let close = format!("\"{}", "#".repeat(hashes));
            let start = i + 2 + hashes;
            let end = source[start..].find(&close).map_or(b.len(), |n| start + n);
            blank(&mut out, start, end);
            i = end + close.len();
        } else if b[i] == b'"' {
            let mut j = i + 1;
            while j < b.len() && b[j] != b'"' {
                j += if b[j] == b'\\' { 2 } else { 1 };
            }
            blank(&mut out, i + 1, j);
            i = j + 1;
        } else if b[i] == b'\'' && b.get(i + 1) == Some(&b'\\') && i + 3 <= b.len() {
            // An escaped char literal ('\'', '\"', '\\', '\u{..}'; b'..' alike).
            let end = b[i + 3..]
                .iter()
                .position(|&c| c == b'\'')
                .map_or(b.len(), |n| i + 3 + n);
            blank(&mut out, i + 1, end);
            i = end + 1;
        } else if b[i] == b'\'' && b.get(i + 2) == Some(&b'\'') {
            blank(&mut out, i + 1, i + 2);
            i += 3;
        } else {
            i += 1;
        }
    }
    String::from_utf8(out).unwrap()
}
fn test_only(code: &str, at: usize, attr: &str) -> bool {
    let mut starts = starts(code, at).into_iter();
    starts.any(|start| code[start..].trim_start().starts_with(attr))
}
fn starts(code: &str, at: usize) -> Vec<usize> {
    let (b, mut found) = (code.as_bytes(), Vec::new());
    let mut end = at;
    loop {
        let (mut depth, mut i) = (0usize, end);
        let start = loop {
            if i == 0 {
                break 0;
            }
            i -= 1;
            match b[i] {
                b')' | b']' | b'}' if depth > 0 || b[i] != b'}' => depth += 1,
                b'(' | b'[' | b'{' if depth > 0 => depth -= 1,
                b'(' | b'[' => {}
                b';' | b',' if depth > 0 => {}
                b'}' | b'{' | b';' | b',' => break i + 1,
                _ => {}
            }
        };
        found.push(start);
        // Up to the block that encloses this statement.
        let (mut depth, mut i) = (0usize, start);
        loop {
            if i == 0 {
                return found;
            }
            i -= 1;
            match b[i] {
                b'}' => depth += 1,
                b'{' if depth == 0 => break,
                b'{' => depth -= 1,
                _ => {}
            }
        }
        end = i;
    }
}
fn lex(source: &str) -> Vec<&str> {
    let (code, mut tokens, mut i) = (mask(source), Vec::new(), 0);
    let c = code.as_bytes();
    let word = |b: &u8| b.is_ascii_alphanumeric() || *b == b'_';
    while i < c.len() {
        let start = i;
        i += match c[i] {
            b if b.is_ascii_whitespace() => {
                i += 1;
                continue;
            }
            b'"' => code[i + 1..].find('"').map_or(c.len() - i, |n| n + 2),
            b'\'' if c.get(i + 2) == Some(&b'\'') => 3,
            b if word(&b) => c[i..].iter().take_while(|b| word(b)).count(),
            _ => source[i..].chars().next().map_or(1, char::len_utf8),
        };
        tokens.push(&source[start..i.min(c.len())]);
    }
    tokens
}
fn flat(tokens: &[&str]) -> String {
    tokens.concat().replace('"', "")
}
fn pinned(source: &str, literal: &str) -> usize {
    let (source, literal) = (lex(source), lex(literal));
    let windows = source.windows(literal.len());
    windows.filter(|w| *w == literal).count()
}
fn header(code: &str, range: std::ops::Range<usize>) -> (Vec<String>, String) {
    let tokens = lex(&code[range]);
    let (mut attrs, mut lead, mut i) = (Vec::new(), 0, 0);
    while i < tokens.len() {
        let open = i + 1 + usize::from(tokens.get(i + 1) == Some(&"!"));
        if tokens[i] != "#" || tokens.get(open) != Some(&"[") {
            i += 1;
            continue;
        }
        let mut depth = 0i32;
        let close = (open..tokens.len()).find(|&j| {
            depth += i32::from(tokens[j] == "[") - i32::from(tokens[j] == "]");
            depth == 0
        });
        let close = close.unwrap_or(tokens.len() - 1);
        attrs.push(tokens[i..=close].concat());
        lead = if i == lead { close + 1 } else { lead };
        i = close + 1;
    }
    (attrs, flat(&tokens[lead..]))
}
fn enclosing(code: &str, mut at: usize) -> Vec<std::ops::Range<usize>> {
    let (b, mut found) = (code.as_bytes(), Vec::new());
    loop {
        let (mut depth, mut i) = (0usize, at);
        let open = loop {
            if i == 0 {
                return found;
            }
            i -= 1;
            match b[i] {
                b'}' => depth += 1,
                b'{' if depth == 0 => break i,
                b'{' => depth -= 1,
                _ => {}
            }
        };
        let (mut depth, mut i) = (0usize, open);
        let start = loop {
            if i == 0 {
                break 0;
            }
            i -= 1;
            let chained = || code[i + 1..].trim_start().starts_with("else");
            match b[i] {
                b'}' if depth == 0 && !chained() => break i + 1,
                b')' | b']' | b'}' => depth += 1,
                b'(' | b'[' | b'{' if depth > 0 => depth -= 1,
                b'{' | b';' if depth == 0 => break i + 1,
                _ => {}
            }
        };
        found.push(start..open + 1);
        at = start;
    }
}
/// One item's text, from `fn name(` to its closing brace.
fn item<'a>(source: &'a str, name: &str) -> &'a str {
    let at = source.find(&format!("\nfn {name}(")).expect(name) + 1;
    let end = source[at..].find("\n}\n").expect(name);
    &source[at..at + end + 2]
}
/// The route-relevant sources, by path relative to `mother/`.
fn sources() -> Vec<(String, String)> {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let roots = ["src", "host/src", "desktop/src", "host/tests/route_cells"];
    let (mut dirs, mut files): (Vec<_>, _) = (roots.map(|d| root.join(d)).into(), Vec::new());
    while let Some(dir) = dirs.pop() {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                dirs.push(path);
            } else if path.extension().is_some_and(|e| e == "rs") {
                let name = path.strip_prefix(&root).unwrap().to_string_lossy();
                files.push((name.into_owned(), std::fs::read_to_string(&path).unwrap()));
            }
        }
    }
    files.sort();
    files
}
fn text<'a>(sources: &'a [(String, String)], file: &str) -> &'a str {
    &sources.iter().find(|(f, _)| f == file).expect(file).1
}
/// Files outside the inventory: S5's module (gated whole by its one
/// declaration), this guard, and the lane-only harness.
fn exempt(file: &str) -> bool {
    let skip = ["src/route_seams.rs", "src/oracles/route_seams_guard.rs"];
    skip.contains(&file) || file.starts_with("host/tests/route_cells/")
}
fn sites(sources: &[(String, String)]) -> Vec<String> {
    let mut rows = Vec::new();
    for (file, source) in sources.iter().filter(|(f, _)| !exempt(f)) {
        let code = mask(source);
        let mut at = seam_sites(source, &NAMES);
        at.sort_unstable();
        for at in at {
            let name = code[at..].split(|c: char| c != '_' && !c.is_alphanumeric());
            let word = if code[..at].trim_end().ends_with("fn") {
                "fn "
            } else {
                ""
            };
            let chain = enclosing(&code, at).into_iter().map(|r| {
                let (attrs, head) = header(&code, r);
                format!("{}{head}", attrs.concat())
            });
            let chain: Vec<String> = chain.collect();
            let name = name.into_iter().next().unwrap();
            rows.push(format!("{file}: {word}{name} < {}", chain.join(">")));
        }
    }
    rows
}
/// The byte after the bracket that closes the one opened at `open`.
fn closing(code: &str, open: usize) -> usize {
    let mut depth = 0i32;
    let close = code[open..].find(|c: char| {
        depth += i32::from("([{".contains(c)) - i32::from(")]}".contains(c));
        depth == 0
    });
    open + close.unwrap() + 1
}
/// G2 for Mother: route names not gated by exactly `#[cfg(mother_route_seams)]`.
/// `starts` visits neither a `route_seams::at(..)` call's own statement from
/// inside its arguments (its `Ctx`), nor an `if let` statement past its
/// pattern's braces (the `mother.admit` conditions), nor an item past a
/// generic's comma (`impl .. BorrowedMeter<'_, P>`); each is added here, an
/// enclosing item only when its attributes are exactly the route cfg.
fn route_leaks(source: &str) -> usize {
    let code = mask(source);
    let mut gated_ranges = Vec::new();
    for at in seam_sites(source, &["route_seams"]) {
        if code[at..].starts_with("route_seams::at(") && test_only(&code, at, ROUTE) {
            let open = at + "route_seams::at".len();
            gated_ranges.push(open..closing(&code, open));
        }
    }
    for (at, _) in code.match_indices(ROUTE) {
        let mut i = at + ROUTE.len();
        if !code[i..].trim_start().starts_with("if ") {
            continue;
        }
        loop {
            let open = i + code[i..].find('{').unwrap();
            let close = closing(&code, open);
            if !code[close..].trim_start().starts_with('=') {
                gated_ranges.push(open..close);
                break;
            }
            i = close; // A pattern's braces: its block follows.
        }
    }
    let item = |at: usize| {
        enclosing(&code, at)
            .into_iter()
            .any(|r| header(&code, r).0 == [ROUTE])
    };
    let gated = |at: usize| {
        test_only(&code, at, ROUTE) || item(at) || gated_ranges.iter().any(|r| r.contains(&at))
    };
    seam_sites(source, &NAMES)
        .into_iter()
        .filter(|&at| !gated(at))
        .count()
}
fn port_uses(s5: &str) -> Vec<String> {
    let tokens = lex(s5);
    let at = (0..tokens.len()).filter(|&i| ["SessionPort", "SessionDoor"].contains(&tokens[i]));
    let row = |i: usize| {
        let (lo, hi) = (i.saturating_sub(3), (i + 4).min(tokens.len()));
        format!(
            "{}|{}|{}",
            tokens[lo..i].concat(),
            tokens[i],
            tokens[i + 1..hi].concat()
        )
    };
    at.map(row).collect()
}
/// A.2 item 3 (a)-(d) and the harness `timeout` (SB3E-R39.1, R40.6) over
/// `s5.rs`, by identifier (its `use super::*` makes any helper nameable).
fn s5_findings(s5: &str) -> Vec<String> {
    let tokens = lex(s5);
    let n = |pat: &[&str]| tokens.windows(pat.len()).filter(|w| *w == pat).count();
    let mut found = Vec::new();
    let mut check = |ok: bool, what: &str| {
        if !ok {
            found.push(what.to_string());
        }
    };
    // (a) one `.call`, through the one `.model`, in the pinned `drive_call`.
    check(n(&[".", "call", "("]) == 1, "(a) one .call");
    check(n(&[".", "model"]) == 1, "(a) one .model");
    check(n(&["fetch_add"]) == 1, "(a) one count");
    let resets = [
        "store",
        "swap",
        "fetch_sub",
        "fetch_update",
        "fetch_and",
        "fetch_nand",
    ];
    let resets = resets
        .into_iter()
        .chain(["fetch_or", "fetch_xor", "fetch_max", "fetch_min"]);
    for reset in resets.chain(["compare_exchange", "compare_exchange_weak"]) {
        check(n(&[".", reset]) == 0, "(a) the count is never reset");
    }
    check(n(&["AtomicU32", ":", ":"]) == 0, "(a) no second counter");
    // (b) no other route to the broker's model call; no re-exported handle.
    for id in [
        "model_call",
        "call_model",
        "model_call_private",
        "EffectPort",
        "ModelCaller",
    ] {
        check(n(&[id]) == 0, &format!("(b) {id}"));
    }
    let reexport = tokens.windows(2).any(|w| w[0] == "pub" && w[1] == "use");
    let scoped = tokens
        .windows(5)
        .any(|w| w[0] == "pub" && w[1] == "(" && w[4] == "use");
    check(!reexport && !scoped, "(b) no re-export");
    // (c) the port is made once, inside `Coordinator::open`, and never kept.
    check(
        port_uses(s5) == PORT_USES.lines().collect::<Vec<_>>(),
        "(c) port uses",
    );
    // (d) no helper of `store.rs` or `locks.rs`.
    for module in ["store", "locks"] {
        check(n(&[module, ":", ":"]) == 0, &format!("(d) {module}::"));
    }
    // The pinned regions: `drive_call` counts first, one `Coordinator::open`,
    // and the harness `timeout`.
    for (_, pin) in PINS.iter().filter(|(f, _)| *f == S5) {
        check(
            pinned(s5, pin) == 1,
            &format!("pin {}", flat(&lex(pin)[..8])),
        );
    }
    // The timeout aborts and cannot unwind: no panic path, `abort` last.
    let body = item(s5, "timeout");
    let body = lex(body);
    let unwinds = [
        "unwrap",
        "expect",
        "panic",
        "assert",
        "unreachable",
        "todo",
        "?",
        "[",
    ];
    check(
        !body.iter().any(|t| unwinds.contains(t)),
        "timeout: a panic path",
    );
    let tail = ["std", ":", ":", "process", ":", ":", "abort", "(", ")", "}"];
    check(
        body.ends_with(&tail),
        "timeout: ends in std::process::abort",
    );
    found
}

#[test]
fn sb3e_mother_lexer_is_governed_controls() {
    let own = include_str!("route_seams_guard.rs");
    for name in LEXER {
        assert_eq!(item(own, name), item(GC, name), "{name}: the copy drifted");
    }
}

/// Step 0 item 8: exactly the audited sites, each name gated exactly by
/// `#[cfg(mother_route_seams)]` (never `cfg(test)` or broadened), every
/// reviewed region pinned once, the markers and S5's module shape.
#[test]
fn sb3e_mother_route_sites_are_exactly_the_inventory() {
    let sources = sources();
    assert_eq!(
        sites(&sources),
        SITES.lines().collect::<Vec<_>>(),
        "the route sites"
    );
    let (mut scanned, mut found) = (0, 0);
    for (file, source) in sources.iter().filter(|(f, _)| !exempt(f)) {
        found += seam_sites(source, &NAMES).len();
        assert_eq!(route_leaks(source), 0, "{file}: a route name ungated");
        scanned += 1;
    }
    assert!(
        scanned > 100 && found == SITES.lines().count(),
        "{scanned} {found}"
    );
    for (file, pin) in PINS {
        assert_eq!(pinned(text(&sources, file), pin), 1, "{file}: pin {pin}");
    }
    for (file, hook) in HOOKS {
        let source = text(&sources, file);
        assert!(
            !seam_sites(source, &[hook]).is_empty(),
            "{file}: {hook} scanned"
        );
        assert_eq!(
            leaks_under(source, &[hook], TEST),
            0,
            "{file}: {hook} widened"
        );
    }
    // Each marker exactly once in Mother's sources, none within another.
    let markers = sources
        .iter()
        .filter(|(f, _)| !exempt(f) || f == "src/route_seams.rs");
    let markers = markers.flat_map(|(_, s)| s.split("\"nexus-mother-route-seam").skip(1));
    let markers =
        markers.map(|rest| format!("nexus-mother-route-seam{}", rest.split('"').next().unwrap()));
    let mut markers: Vec<String> = markers.collect();
    markers.sort();
    assert_eq!(markers, MARKERS, "every route marker, once");
    // S5's module: `arm` and `drain_trace` its only `pub fn`, no further
    // function, no `#[used]`, the timeout called once, and every point armed
    // is an audited site's.
    let seams = text(&sources, "src/route_seams.rs");
    let tokens = lex(seams);
    let public = tokens
        .windows(3)
        .filter(|w| w[..2] == ["pub", "fn"])
        .map(|w| w[2]);
    assert_eq!(
        public.collect::<Vec<_>>(),
        ["arm", "drain_trace"],
        "public writers"
    );
    assert_eq!(
        tokens.iter().filter(|t| **t == "fn").count(),
        10,
        "no further function"
    );
    assert!(!seams.contains("#[used]"), "SB3E-R9");
    let calls = tokens.windows(2).filter(|w| w == &["timeout", "("]).count();
    assert_eq!(calls, 1, "SB3E-R39.1: one timeout call");
    let mut audited = Vec::new();
    for (_, source) in sources.iter().filter(|(f, _)| !exempt(f)) {
        let tokens = lex(source);
        let at = tokens
            .windows(6)
            .filter(|w| w[..5] == ["route_seams", ":", ":", "at", "("]);
        audited.extend(at.map(|w| w[5].trim_matches('"').to_string()));
    }
    audited.sort();
    let listed = seams.split("POINTS: [&str; 10] = [").nth(1).unwrap();
    let listed = listed
        .split("];")
        .next()
        .unwrap()
        .split('"')
        .skip(1)
        .step_by(2);
    let mut listed: Vec<String> = listed.map(String::from).collect();
    listed.sort();
    assert_eq!(
        audited, listed,
        "every armable point has its one audited site"
    );
    let declared = text(&sources, "src/lib.rs");
    assert!(declared.contains("\n#[cfg(mother_route_seams)]\npub mod route_seams;\n"));
}

/// AM-2 A.2 item 3 over `s5.rs`, with the rev 4.2 plants, R39.1's timeout
/// and rev 3 A.5's conditions: the file as it is passes; each plant fails.
#[test]
fn sb3e_s5_module_makes_one_counted_call() {
    let sources = sources();
    let s5 = text(&sources, S5);
    assert_eq!(s5_findings(s5), Vec::<String>::new(), "s5.rs as it is");
    let sub = |old: &str, new: &str| {
        assert_eq!(s5.matches(old).count(), 1, "{old}");
        s5.replace(old, new)
    };
    let invoke = "        let text = if drive_call(&self.0, context) {";
    let open = "        Arc::new(SessionPort::new(b.door, b.name.clone())),";
    let kept = "let port = Arc::new(SessionPort::new(b.door, b.name.clone()));";
    let count = "    calls.fetch_add(1, Ordering::SeqCst);\n";
    let plants = [
        sub(invoke, &format!("        let _ = context.model.map(|m| m.call(&r));\n{invoke}")),
        format!("{s5}pub(super) use nexus_governed_control::inference::ModelCaller as Handle;\n"),
        sub(count, "").replace(".is_ok()\n}", ".is_ok() && calls.fetch_add(1, Ordering::SeqCst) < 9\n}"),
        sub("    fn run(&self) {\n", "    fn run(&self) {\n        self.calls.store(0, Ordering::SeqCst);\n"),
        sub(open, "        port.clone(),").replace("    let m = Coordinator::open(", &format!("    {kept}\n    let _ = port.model_call(id, &q, &meter, &d);\n    let m = Coordinator::open(")),
        sub("    let door = o.session.open_file_door(&request).unwrap();\n", "    let door = o.session.open_file_door(&request).unwrap();\n    let _ = door.model_call(run, &q, &meter, &d, i);\n"),
        format!("use super::store::call_model;\n{s5}"),
        sub("    std::process::abort()\n}\n\n/// SB3E-R40.5", "    let _ = root;\n}\n\n/// SB3E-R40.5"),
        sub("let Some(root) = std::env::var_os(CHILD).map(PathBuf::from) else {", "let root = std::env::var_os(CHILD).map(PathBuf::from).unwrap();\n    {"),
    ];
    for (i, planted) in plants.iter().enumerate() {
        assert!(!s5_findings(planted).is_empty(), "A.2 plant {i} survived");
    }
    // Rev 3 A.5: each `mother.admit` condition, pinned whole.
    let coordinator = text(&sources, "src/coordinator.rs");
    let pin = PINS
        .iter()
        .find(|(_, p)| p.contains("\"mother.admit\""))
        .unwrap()
        .1;
    let lane = "if let Event::LaneReturned { invocation, .. } = &entry.event {";
    let widened = "if let Event::LaneReturned { invocation, .. } | Event::ModelCallCharged { invocation, .. } = &entry.event {";
    for planted in [
        coordinator.replacen(
            lane,
            "if let Some(invocation) = entry.event.invocation() {",
            1,
        ),
        coordinator.replacen(lane, widened, 1),
    ] {
        assert_ne!(pinned(&planted, pin), 1, "A.5: a condition changed");
    }
}

/// SB3E-R15.2's sweep over Mother's pins: at each statement boundary inside
/// a pinned region, and before each `&&` or `||`, a no-op-looking insertion;
/// every mutant must fail that region's pin.
#[test]
fn sb3e_mother_pins_reject_every_generated_mutant() {
    let sources = sources();
    let (mut mutants, mut survivors) = (0, Vec::new());
    for (file, pin) in PINS {
        let source = text(&sources, file);
        let (tokens, literal, base) = (lex(source), lex(pin), source.as_ptr() as usize);
        let at = tokens
            .windows(literal.len())
            .position(|w| w == literal)
            .unwrap();
        let last = tokens[at + literal.len() - 1];
        let start = tokens[at].as_ptr() as usize - base;
        let end = last.as_ptr() as usize - base + last.len();
        let b = mask(source).into_bytes();
        for p in start + 1..end {
            let after = b"{};,".contains(&b[p - 1]);
            let operator = b[p..].starts_with(b"&&") || b[p..].starts_with(b"||");
            for (_, insert) in [(after, " let _ = (); "), (operator, " && true ")]
                .iter()
                .filter(|i| i.0)
            {
                mutants += 1;
                let mutant = format!("{}{insert}{}", &source[..p], &source[p..]);
                if pinned(&mutant, pin) == 1 {
                    survivors.push(format!("{file}@{p}:{insert}"));
                }
            }
        }
    }
    println!(
        "U4a-3 sweep: {} regions, {mutants} mutants, {survivors:?} survived",
        PINS.len()
    );
    assert!(
        mutants > PINS.len() && survivors.is_empty(),
        "{survivors:?}"
    );
}

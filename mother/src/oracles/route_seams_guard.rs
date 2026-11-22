//! SB-3E U4a-3: Mother's route guard (Step 0 item 8; AM-2 rev 4.2 A.2 item
//! 3 and rev 3 A.5; SB3E-R39.1, R40.6). It reads source text only, in the
//! standard `cargo test -p nexus-mother`, where no seam is compiled:
//! - every S5, S3-forwarder and S4b name sits at an audited site, each gated
//!   exactly by `#[cfg(mother_route_seams)]` (inventory, G2 leaks, pins and
//!   the generated sweep), with its marker and its test hooks unwidened;
//! - `route_cells/s5.rs` makes one counted model call and keeps no port;
//! - the harness `timeout` cannot unwind and ends in `std::process::abort`;
//! - SB3E-R42: S5's module is pinned whole, Mother's include sites are an
//!   exact list, the pins data is digest-pinned, and `s5.rs` has no raw
//!   identifier, macro definition, glob or sibling path, nor an unbounded
//!   child wait. Every finding is collected before a test fails.
//!
//! The lexer is governed-control's (`model_call_tests.rs`), byte for byte.

const TEST: &str = "#[cfg(test)]";
const ROUTE: &str = "#[cfg(mother_route_seams)]";
/// SB3E-R51.2: a source whose first line is a shebang (`tokens`).
const SHEBANG: &str = "starts with a shebang";
const GC: &str =
    include_str!("../../../crates/nexus-governed-control/src/host_session/model_call_tests.rs");
const LEXER: [&str; 18] = [
    "seam_sites",
    "leaks_under",
    "mask",
    "tokens",
    "leaves",
    "place",
    "doc",
    "comment",
    "gap",
    "test_only",
    "starts",
    "lex",
    "words",
    "piece",
    "flat",
    "pinned",
    "header",
    "enclosing",
];
const S5: &str = "host/tests/route_cells/s5.rs";
/// U18's reader harness (B4-B7).
const READER: &str = "host/tests/route_cells/reader.rs";
/// The two seam modules, each gated whole by its one declaration.
const MODULES: [&str; 2] = ["src/route_seams.rs", "host/src/route_seams.rs"];
/// Every name of Mother's route seams outside `route_seams.rs` itself.
const NAMES: [&str; 5] = [
    "route_bind",
    "route_seams",
    "route_ctx",
    "route_revoke_original_grant",
    "route_poison_coordinator",
];
/// Mother's route markers, each exactly once (SB3E-R32.5, R40.6).
const MARKERS: [&str; 5] = [
    "nexus-mother-route-seam-S3-port",
    "nexus-mother-route-seam-S4b",
    "nexus-mother-route-seam-S5",
    "nexus-mother-route-seam-S6",
    "nexus-mother-route-seam-S7",
];
/// The test hooks beside the sites stay exactly `#[cfg(test)]` (mission 11).
const HOOKS: [(&str, &str); 5] = [
    // Built from parts: `c1_s3_record_seam_is_test_only` counts the name.
    ("src/adapter/mod.rs", concat!("model_", "record")),
    ("src/adapter/session_port.rs", "inner_for_test"),
    ("src/coordinator.rs", "fence_to_marker"),
    ("src/coordinator/lite.rs", "after_engagement"),
    ("src/coordinator/model_calls.rs", "hooks"),
];
/// Every route name's site: `file: [fn ]name < enclosing chain` (as AM-2 B.2).
const SITES: &str = "\
host/src/hands.rs: route_seams < \n\
host/src/hands.rs: route_seams < fnclose_report(&self,stage:&str)->String{>implRuntime{\n\
host/src/hands.rs: route_seams < fnclose_report(&self,stage:&str)->String{>implRuntime{\n\
host/src/hands.rs: route_seams < fnclose_report(&self,stage:&str)->String{>implRuntime{\n\
host/src/hands.rs: route_seams < fnclose_report(&self,stage:&str)->String{>implRuntime{\n\
host/src/hands.rs: route_seams < fnclose_report(&self,stage:&str)->String{>implRuntime{\n\
host/src/hands.rs: route_seams < fnclose_report(&self,stage:&str)->String{>implRuntime{\n\
host/src/hands.rs: route_seams < fnclose_report(&self,stage:&str)->String{>implRuntime{\n\
host/src/hands.rs: route_seams < fnclose_report(&self,stage:&str)->String{>implRuntime{\n\
host/src/hands.rs: route_seams < fnclose_report(&self,stage:&str)->String{>implRuntime{\n\
host/src/hands.rs: route_seams < fnclose_report(&self,stage:&str)->String{>implRuntime{\n\
host/src/lib.rs: route_seams < \n\
host/src/terminal.rs: route_seams < \n\
host/src/terminal.rs: fn route_bind < implTerminal{\n\
host/src/terminal.rs: route_seams < Ok(0)=>{>matchnix::unistd::read(stdin.as_raw_fd(),&mutbytes){>while!shared.input_end.load(Ordering::SeqCst){>fnread_input(shared:Arc<Shared>){\n\
host/src/terminal.rs: route_seams < Ok(0)=>{>matchnix::unistd::read(stdin.as_raw_fd(),&mutbytes){>while!shared.input_end.load(Ordering::SeqCst){>fnread_input(shared:Arc<Shared>){\n\
host/src/terminal.rs: route_seams < Ok(0)=>{>matchnix::unistd::read(stdin.as_raw_fd(),&mutbytes){>while!shared.input_end.load(Ordering::SeqCst){>fnread_input(shared:Arc<Shared>){\n\
host/src/terminal.rs: route_seams < Ok(n)=>{>matchnix::unistd::read(stdin.as_raw_fd(),&mutbytes){>while!shared.input_end.load(Ordering::SeqCst){>fnread_input(shared:Arc<Shared>){\n\
host/src/terminal.rs: route_seams < Ok(n)=>{>matchnix::unistd::read(stdin.as_raw_fd(),&mutbytes){>while!shared.input_end.load(Ordering::SeqCst){>fnread_input(shared:Arc<Shared>){\n\
host/src/terminal.rs: route_seams < Err(_)=>{>matchnix::unistd::read(stdin.as_raw_fd(),&mutbytes){>while!shared.input_end.load(Ordering::SeqCst){>fnread_input(shared:Arc<Shared>){\n\
host/src/terminal.rs: route_seams < Err(_)=>{>matchnix::unistd::read(stdin.as_raw_fd(),&mutbytes){>while!shared.input_end.load(Ordering::SeqCst){>fnread_input(shared:Arc<Shared>){\n\
host/src/terminal.rs: route_seams < Err(_)=>{>matchnix::unistd::read(stdin.as_raw_fd(),&mutbytes){>while!shared.input_end.load(Ordering::SeqCst){>fnread_input(shared:Arc<Shared>){\n\
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
{CommandKind,|SessionDoor|,SessionSeed}\n\
,FilePrivate,|SessionPort|};use\n\
)door:|SessionDoor|,pub(\n\
&FileSession<|SessionPort|>,folder\n\
:FileSession<|SessionPort|>,id\n\
:new(|SessionPort|::new\n\
";
/// The reviewed regions, whole and normalized: every site with its
/// neighbours (the two `mother.admit` conditions whole, rev 3 A.5), the S3
/// forwarder and S4b bodies, S5's module whole (SB3E-R42.2), and in `s5.rs`
/// the counted call, its one `Coordinator::open`, the harness `timeout`, the
/// bounded child wait, `Mother::driver`, `recorded` and the two credited
/// `credit` calls (SB3E-R42.4). They are data beside this file (`@@@ <file>`
/// then the region), so the crate's own source scans (host access,
/// `c1_s3_record_seam_is_test_only`) do not read quoted code. Before the
/// first region, the data lists every `include_str!`, `include_bytes!` and
/// `#[path]` site of Mother's sources (G-S1).
const PIN_DATA: &str = include_str!("route_seams_guard.pins");
/// SB3E-R42.3 (G-S2): the pins file's sha256 and the sweep's exact count.
const PINS_SHA256: &str = "085946eaef1db023decdf0b4c026c72c98aebe0968e82c189e69610a0c3b0ca3";
const MUTANTS: usize = 1848;
const REGIONS: usize = 38;
/// SB3E-R51.1: every file of the harness directory, byte for byte, exactly
/// this list (`name sha256`): the authority for all of `route_cells`, `s5.rs`
/// included (R44.1). A reviewed commit that changes a file changes its row.
/// The `MAIN_ITEMS`, `IMPLS` and `main.rs`/`store.rs` region pins stay as an
/// advisory layer.
const CELLS: &str = "host/tests/route_cells/";
const ROUTE_CELLS: &str = "\
cells.rs a96c7577e8e9fa07401cf4004468027f97ea7681216e2330788e55d8dd6e9040
grant.rs fbe4c4b9dc43dc86c77ed4ec9c09f8bf9ea61dc8091b8ed1866a847a99e27be4
locks.rs 9b82f75d8ad4f2d0c47194c69be15187dd3f96291033b3a57a4105bc731ccf78
main.rs 898d35e35d308cbf920923e67f70575298ddd5d7f13c60c7b60a422c883005f2
reader.rs 6afe8594053ce320a954501ab429d78c48e0cd6c85387e8ddb4d73ddb6ef86f6
route_read.c 05fa04a9a933dacf5c4d4a93776e695da0c885dfebba857751f16cb7f14c6b4b
route_fsync.c 6166fe8b9eee2e9c2a84a8bcbfcc1590f14de97d7bb0d60084da674a56f0e973
s5.rs c776bdbd6f224768de4e8a6c4ab140e005cb5999fdf4cbf952d50cca232a35c0
s5_close.rs 6127e5b54a2032998559214f4ef6efb8f7d9d765d6c55097367a386423434a0a
s5_reg.rs d12c3d1eeef6cbc58c620fa7648edf6dba3fdd28a06a419263b8dfa45b743a43
s5_r6.rs 42217130763f956fa59dbc92830b833a9321ca1cbfd55b269fe3d82211f2efea
s5_r34.rs 904e02d28e2f6752fe94f45e0ebc3b8a3558759e45a440c5dc3f372541a008a5
witness.rs 57b64cd879c7a85bc565e5d0de1e35dbf420204e1a644d4fe8ae2e7fff221635
store.rs 0dabb6132cfc184a99b4627b983226cdef3390785c8e60daa189bba5bc737b00
report.rs 7340ef6e54425d9655e70be86370e09d0db18615236c02a73655b272089423fd
s5_report.rs fed9e772fe9f00880077ca56f4a229742208bcdd904314cf5126fc91ba7166c9
";
fn route_cells() -> impl Iterator<Item = (&'static str, &'static str)> {
    ROUTE_CELLS.lines().filter_map(|l| l.split_once(' '))
}
/// F-S5-SPLIT (the adopted amendment to AM-2 A.2 item 1, section 4 items 1
/// and 2): the S5 boundary is `s5.rs` and these section files, a closed list,
/// one row each: `file|unit|sink:dim,..|ctl_..,..|imports|helpers` (`-` is
/// empty). `imports` is every item the file's `use` statements name, and
/// `helpers` the `pub(super)` names it may expose besides its drivers. `s5.rs`
/// is the first row; its imports are `S5_SUPER` (G-S3, in `s5_findings`). A
/// stub's lists are empty: its unit fills them.
const S5_SECTIONS: &str = "\
s5.rs|U5|call_chat:ingress,call_chat:lifecycle|ctl_call_chat|S5_SUPER|-
s5_close.rs|U9a|close_obligations:lifecycle,delivery:lifecycle,mother_end:lifecycle,primary_end:lifecycle,producer_join:lifecycle,seal:lifecycle|ctl_close_obligations,ctl_delivery,ctl_mother_end,ctl_primary_end,ctl_producer_join,ctl_seal|super::s5::closed,super::s5::Close,super::s5::Facts,super::s5::Mode,super::s5::Setup,super::Fixture,nexus_governed_control::host_session::close::CloseSeal,nexus_governed_control::host_session::SessionRequest|-
s5_reg.rs|U6a-1|registration_reserve:ingress,registration_record:ingress,registration_install:ingress,registration_readiness:ingress,registration_reserve:lifecycle,registration_record:lifecycle,registration_readiness:lifecycle|ctl_registration_reserve,ctl_registration_record,ctl_registration_install,ctl_registration_readiness|super::s5::read_registered,super::s5::registered,super::s5::Facts,super::Fixture|-
s5_r34.rs|U6b-2|attempt_ticket:ingress,call_slot_reserve:ingress,charge_append:ingress,meter_settle:ingress|ctl_attempt_ticket,ctl_call_slot_reserve,ctl_charge_append,ctl_meter_settle|-|-
s5_r6.rs|U9b|close_restrict:lifecycle,emergency_restrict:lifecycle|ctl_close_restrict,ctl_emergency_restrict|super::s5::closed,super::s5::Close,super::s5::Facts,super::s5::Mode,super::s5::Setup,super::Fixture|-
s5_report.rs|U21a|close_report:lifecycle,close_report:fault,close_report:schedule|ctl_close_report|super::report::reached,super::report::Hold,super::report::Owner,super::s5::collect_report,super::s5::Facts,super::Fixture|-
";
/// Item 1(a): the directory's files that are not sections, closed.
const NON_SECTIONS: [&str; 10] = [
    "cells.rs",
    "grant.rs",
    "locks.rs",
    "main.rs",
    "reader.rs",
    "report.rs",
    "route_fsync.c",
    "route_read.c",
    "store.rs",
    "witness.rs",
];
/// One `S5_SECTIONS` row.
struct Row<'a> {
    file: &'a str,
    unit: &'a str,
    pairs: Vec<&'a str>,
    controls: Vec<&'a str>,
    imports: Vec<&'a str>,
    helpers: Vec<&'a str>,
}
fn list(field: &str) -> Vec<&str> {
    let named = field.split(',').filter(|x| !x.is_empty() && *x != "-");
    named.collect()
}
/// The rows of `data`; a line that is not six fields is no row (a finding in
/// `sections_with`).
fn section_rows(data: &str) -> Vec<Row<'_>> {
    let mut rows = Vec::new();
    for line in data.lines() {
        if let [file, unit, pairs, controls, imports, helpers] =
            line.split('|').collect::<Vec<_>>()[..]
        {
            rows.push(Row {
                file,
                unit,
                pairs: list(pairs),
                controls: list(controls),
                imports: list(imports),
                helpers: list(helpers),
            });
        }
    }
    rows
}
fn stem(file: &str) -> &str {
    file.strip_suffix(".rs").unwrap_or(file)
}
/// SB3E-R55.6 (Fable A-N2): a row that is not `name sha256` is a finding.
fn rows_findings(rows: &str) -> Vec<String> {
    let hex = |d: &str| d.len() == 64 && d.bytes().all(|b| b"0123456789abcdef".contains(&b));
    rows.lines()
        .filter(|l| {
            !l.split_once(' ')
                .is_some_and(|(n, d)| !n.is_empty() && hex(d))
        })
        .map(|l| format!("R51.1: a malformed ROUTE_CELLS row {l:?}"))
        .collect()
}
/// SB3E-R44.3: the guards' lexer, a dev-dependency only, exactly this line
/// in each manifest (no feature, no other version or table).
const LEXER_DEP: &str = "proc-macro2 = { version = \"=1.0.106\", default-features = false }";
const MANIFESTS: [(&str, &str); 2] = [
    ("mother/Cargo.toml", include_str!("../../Cargo.toml")),
    (
        "crates/nexus-governed-control/Cargo.toml",
        include_str!("../../../crates/nexus-governed-control/Cargo.toml"),
    ),
];
/// SB3E-R44.2: `main.rs`'s top-level items (`items`), one per line (one
/// `:` escaped, so the host-access scan does not match this file).
const MAIN_ITEMS: &str = "\
#![cfg(all(target_os=linux,mother_route_seams))]#![deny(unsafe_code)]usenexus_governed_control::authority::approval::*;\n\
usenexus_governed_control::authority::clock::SystemClock;\n\
usenexus_governed_control::authority::ids::RunId;\n\
usenexus_governed_control::broker::route_seams::{arm,drain_trace,Action,Entry,Held};\n\
usenexus_governed_control::evidence_store::PrimaryEvidence;\n\
usenexus_governed_control::host_session::{CommandKind,HostSession,SessionSeed,SessionState};\n\
usenexus_governed_control::host_session::{SessionDoor,SessionIngress};\n\
usenexus_governed_control::inference::{CallDeadline,CallMeter,Charged,InvocationOwner};\n\
usenexus_governed_control::inference::{MeterRefusal,ModelIdentity,ModelPinSpec,ModelRequest};\n\
usenexus_governed_control::inference::{ResponseSchema,RuntimeConfig,Think};\n\
usestd::io::{Read,Write};\n\
usestd:\x3aos::unix::fs::DirBuilderExt;\n\
usestd::path::PathBuf;\n\
usestd::sync::atomic::{AtomicU32,Ordering};\n\
usestd::sync::mpsc::RecvTimeoutError;\n\
usestd::sync::{Arc,Mutex};\n\
usestd::time::{Duration,Instant};\n\
modgrant;\n\
modlocks;\n\
modreader;\n\
modreport;\n\
mods5;\n\
mods5_close;\n\
mods5_r34;\n\
mods5_r6;\n\
mods5_reg;\n\
mods5_report;\n\
modstore;\n\
macro_rules!cell\n\
macro_rules!ctl\n\
macro_rules!cells\n\
#[rustfmt::skip]modcells;\n\
modwitness;\n\
constPATIENCE:Duration=Duration::from_secs(10);\n\
constCHILD:&str=SB3E_ROUTE_CHILD;\n\
constNAME:&str=qwen3.6:35b-a3b;\n\
constDIGEST:&str=a7eb95c53bcf96b4bdd008d0fab4a5dac88047d9c1a7a9ab88ed453423fbd87c;\n\
#[test]fnroute_lane_cfg_reached_the_harness()\n\
fnchild(name:&str,body:fn(&Fixture))\n\
fnchild_with(name:&str,env:fn(&std::path::Path)->Vec<(&str,PathBuf)>,body:fn(&Fixture))\n\
structFixture(PathBuf);\n\
implDropforFixture\n\
structOpened\n\
implFixture\n\
implOpened\n\
structMeter(AtomicU32);\n\
implCallMeterforMeter\n\
structNever;\n\
implCallDeadlineforNever\n\
structOwner;\n\
implControlConfirmerforOwner\n\
fnfake_ollama()->(String,Arc<Mutex<Vec<String>>>)\n\
fnspec()->ModelPinSpec\n\
#[test]fnroute_unarmed_point_records_nothing_and_does_not_block()\n\
#[test]fnroute_armed_pause_acknowledges_blocks_and_releases()\n\
#[test]fnroute_correlated_identity_is_recorded()\n\
#[test]fnroute_armed_panic_begins_an_unwind()\n\
#[test]fnroute_at_cannot_return_into_the_route()\n\
#[test]fnroute_store_commit_hold_fires_on_an_ordinary_append()\n\
";
/// SB3E-R44.2 (S2): every `impl` in the harness but `s5.rs` (`impls`).
const IMPLS: &str = "\
host/tests/route_cells/main.rs|implDropforFixture\n\
host/tests/route_cells/main.rs|implFixture\n\
host/tests/route_cells/main.rs|implOpened\n\
host/tests/route_cells/main.rs|implCallMeterforMeter\n\
host/tests/route_cells/main.rs|implCallDeadlineforNever\n\
host/tests/route_cells/main.rs|implControlConfirmerforOwner\n\
host/tests/route_cells/reader.rs|implClock\n\
host/tests/route_cells/reader.rs|implTty\n\
host/tests/route_cells/reader.rs|implDropforTty\n\
host/tests/route_cells/report.rs|implTty\n\
host/tests/route_cells/report.rs|implDropforTty\n\
host/tests/route_cells/store.rs|impl<T>Faulted<T>\n\
";
/// SB3E-R42.4 (G-S3): `s5.rs`'s one import from its parent, by name.
const S5_SUPER: [&str; 10] = [
    "child",
    "fake_ollama",
    "reader",
    "spec",
    "Fixture",
    "Opened",
    "CHILD",
    "DIGEST",
    "NAME",
    "PATIENCE",
];
/// SB3E-R42.1 (G-B1): every macro `s5.rs` may invoke.
const S5_MACROS: [&str; 9] = [
    "assert",
    "assert_eq",
    "assert_ne",
    "cfg",
    "format",
    "panic",
    "println",
    "vec",
    "writeln",
];
/// The counter's write names besides `fetch_*` and `calls =` (rule (a)).
const WRITES: [&str; 4] = ["store", "swap", "compare_exchange", "compare_exchange_weak"];
fn pins_of(data: &str) -> Vec<(&str, &str)> {
    let regions = data.split("@@@ ").skip(1);
    regions.filter_map(|r| r.split_once('\n')).collect()
}
fn pins() -> Vec<(&'static str, &'static str)> {
    pins_of(PIN_DATA)
}
/// G-S1's allow-list: the data's head, one `file|kind|argument` per line.
fn allowed_includes() -> Vec<&'static str> {
    let head = PIN_DATA.split("@@@ ").next().unwrap_or("").lines();
    head.filter(|l| !l.is_empty() && !l.starts_with('#'))
        .collect()
}
/// G-S2: the pins data is the reviewed data, byte for byte.
fn pins_findings(data: &str) -> Vec<String> {
    use sha2::Digest;
    let mut found = Vec::new();
    let digest = hex::encode(sha2::Sha256::digest(data.as_bytes()));
    if digest != PINS_SHA256 {
        found.push(format!("G-S2: the pins file's sha256 is {digest}"));
    }
    let regions = pins_of(data).len();
    if regions != REGIONS {
        found.push(format!("G-S2: {regions} reviewed regions, not {REGIONS}"));
    }
    found
}
/// SB3E-R42.6: every finding is collected, then the test fails once.
fn fail_on(found: Vec<String>) {
    assert!(
        found.is_empty(),
        "{} findings:\n{}",
        found.len(),
        found.join("\n")
    );
}
/// A plant shows each of its own findings, not merely some failure.
fn killed(misses: &mut Vec<String>, plant: &str, found: &[String], expected: &[&str]) {
    for e in expected {
        if !found.iter().any(|f| f.contains(e)) {
            misses.push(format!("{plant}: no `{e}` in {found:?}"));
        }
    }
}

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
/// SB3E-R44.3: comments become spaces, and so does every literal's text
/// between its first and last quote, whose quotes become `"` (so a masked
/// char literal is still Rust). Byte offsets and lines are kept. The tokens
/// are proc-macro2's; a source that does not tokenize panics here, and each
/// guard reports it as a finding before it masks anything.
fn mask(source: &str) -> String {
    let found = tokens(source).unwrap_or_else(|e| panic!("does not tokenize: {e}"));
    let b = source.as_bytes();
    let mut out: Vec<u8> = b
        .iter()
        .map(|&c| if c == b'\n' { c } else { b' ' })
        .collect();
    for (range, literal) in found {
        out[range.clone()].copy_from_slice(&b[range.clone()]);
        let text = &source[range.clone()];
        let quotes = (text.find(['"', '\'']), text.rfind(['"', '\'']));
        if let (true, Some(first), Some(last)) = (literal, quotes.0, quotes.1) {
            for c in &mut out[range.start + first..=range.start + last] {
                if *c != b'\n' {
                    *c = b' ';
                }
            }
            out[range.start + first] = b'"';
            out[range.start + last] = b'"';
        }
    }
    String::from_utf8(out).unwrap()
}
/// SB3E-R44.3: every leaf token proc-macro2 reads in `source`, as its byte
/// range and whether it is a literal. The tokens must line up with the text,
/// apart only by whitespace and comments; a doc comment, which proc-macro2
/// reads as a `#[doc]` attribute, is a comment here. Anything else is an error,
/// and so is a first line `#!` that is not `#![` (SB3E-R51.2: rustc drops a
/// shebang line, proc-macro2 lexes it).
fn tokens(source: &str) -> Result<Vec<(std::ops::Range<usize>, bool)>, String> {
    if source.starts_with("#!") && !source.starts_with("#![") {
        return Err(SHEBANG.into());
    }
    let stream: proc_macro2::TokenStream = source.parse().map_err(|e| format!("{e:?}"))?;
    let (mut found, mut at) = (Vec::new(), 0);
    leaves(source, stream, &mut at, &mut found)?;
    match gap(source, at) == source.len() {
        true => Ok(found),
        false => Err(format!("text after the last token at byte {at}")),
    }
}
fn leaves(
    source: &str,
    stream: proc_macro2::TokenStream,
    at: &mut usize,
    found: &mut Vec<(std::ops::Range<usize>, bool)>,
) -> Result<(), String> {
    use proc_macro2::{Delimiter, TokenTree};
    let mut trees = stream.into_iter();
    while let Some(tree) = trees.next() {
        *at = gap(source, *at);
        let rest = &source[*at..];
        match tree {
            TokenTree::Punct(p) if p.as_char() == '#' && doc(rest) => {
                // Its `!` (inner) and `[doc = ..]` group have no text of their own.
                let inner = rest.starts_with("//!") || rest.starts_with("/*!");
                *at += comment(rest).ok_or("an unclosed doc comment")?;
                trees.nth(usize::from(inner));
            }
            TokenTree::Group(g) => {
                let (open, close) = match g.delimiter() {
                    Delimiter::Parenthesis => ("(", ")"),
                    Delimiter::Brace => ("{", "}"),
                    Delimiter::Bracket => ("[", "]"),
                    Delimiter::None => return Err("an invisible group".into()),
                };
                place(source, at, found, open, false)?;
                leaves(source, g.stream(), at, found)?;
                *at = gap(source, *at);
                place(source, at, found, close, false)?;
            }
            TokenTree::Ident(i) => place(source, at, found, &i.to_string(), false)?,
            TokenTree::Punct(p) => place(source, at, found, &p.to_string(), false)?,
            TokenTree::Literal(l) => place(source, at, found, &l.to_string(), true)?,
        }
    }
    Ok(())
}
fn place(
    source: &str,
    at: &mut usize,
    found: &mut Vec<(std::ops::Range<usize>, bool)>,
    text: &str,
    literal: bool,
) -> Result<(), String> {
    if !source[*at..].starts_with(text) {
        return Err(format!("`{text}` is not at byte {at}"));
    }
    found.push((*at..*at + text.len(), literal));
    *at += text.len();
    Ok(())
}
/// A doc comment opens `rest` (proc-macro2's rule).
fn doc(rest: &str) -> bool {
    let line = rest.starts_with("///") && !rest.starts_with("////");
    let block = rest.starts_with("/**") && !rest.starts_with("/***") && !rest.starts_with("/**/");
    line || block || rest.starts_with("//!") || rest.starts_with("/*!")
}
/// The length of the comment (block comments nest) that opens `rest`.
fn comment(rest: &str) -> Option<usize> {
    if rest.starts_with("//") {
        return Some(rest.find('\n').unwrap_or(rest.len()));
    }
    let (b, mut depth, mut i) = (rest.as_bytes(), 0usize, 0);
    while rest.starts_with("/*") && i + 1 < b.len() {
        match &b[i..i + 2] {
            b"/*" => (depth, i) = (depth + 1, i + 2),
            b"*/" => (depth, i) = (depth - 1, i + 2),
            _ => i += 1,
        }
        if depth == 0 {
            return Some(i);
        }
    }
    None
}
/// The byte after the whitespace and comments (not doc comments) at `at`.
fn gap(source: &str, mut at: usize) -> usize {
    loop {
        let rest = &source[at..];
        match rest.chars().next() {
            Some(c) if c.is_whitespace() => at += c.len_utf8(),
            _ => match comment(rest).filter(|_| !doc(rest)) {
                Some(n) => at += n,
                None => return at,
            },
        }
    }
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
/// The normalization (SB3E-R14.1, R44.3): proc-macro2's tokens of `source`
/// as text, each an identifier, a whole literal or one punctuation
/// character; a raw identifier is its `r`, `#` and name. Comments and
/// whitespace are not tokens.
fn lex(source: &str) -> Vec<&str> {
    let found = tokens(source).unwrap_or_else(|e| panic!("does not tokenize: {e}"));
    let mut out = Vec::new();
    for (r, literal) in found {
        if !literal && source[r.clone()].starts_with("r#") {
            let (a, b) = (r.start + 1, r.start + 2);
            out.extend([&source[r.start..a], &source[a..b], &source[b..r.end]]);
        } else {
            out.push(&source[r]);
        }
    }
    out
}
/// The words of text `mask` has masked (a header's range ends inside its
/// block, so it is not Rust on its own): an identifier or number, a string
/// of spaces, or one character.
fn words(code: &str) -> Vec<&str> {
    let (c, mut found, mut i) = (code.as_bytes(), Vec::new(), 0);
    let word = |b: &u8| b.is_ascii_alphanumeric() || *b == b'_';
    while i < c.len() {
        let start = i;
        i += match c[i] {
            b if b.is_ascii_whitespace() => {
                i += 1;
                continue;
            }
            b'"' => code[i + 1..].find('"').map_or(c.len() - i, |n| n + 2),
            b if word(&b) => c[i..].iter().take_while(|b| word(b)).count(),
            _ => code[i..].chars().next().map_or(1, char::len_utf8),
        };
        found.push(&code[start..i.min(c.len())]);
    }
    found
}
/// A reviewed region's tokens (SB3E-R44.3). A region may close or open a
/// block it does not hold, so the delimiters it lacks are put around it for
/// proc-macro2 and left out again. Only that padding is guessed (from the
/// region's own brackets outside its strings, chars and comments); a wrong
/// guess does not tokenize, and the region is one token that pins nothing.
fn piece(region: &str) -> Vec<&str> {
    let (b, mut i, mut open, mut lead) = (region.as_bytes(), 0, Vec::new(), String::new());
    let skip = |from: usize, end: u8| {
        let mut j = from;
        while j < b.len() && b[j] != end {
            j += if b[j] == b'\\' { 2 } else { 1 };
        }
        j + 1
    };
    let raw = |i: usize| {
        let hashes = b[i + 1..].iter().take_while(|&&c| c == b'#').count();
        let word = |c: &u8| c.is_ascii_alphanumeric() || *c == b'_';
        b.get(i + 1 + hashes) == Some(&b'"')
            && (i == 0 || !word(&b[i - 1]) || b"bc".contains(&b[i - 1]))
    };
    while i < b.len() {
        let rest = &region[i..];
        let len = |r: &str| r.chars().next().map_or(1, char::len_utf8);
        let char_end = i + 1 + len(rest.get(1..).unwrap_or(""));
        i = match b[i] {
            _ if comment(rest).is_some() => i + comment(rest).unwrap_or(1),
            b'"' => skip(i + 1, b'"'),
            b'r' if raw(i) => {
                let hashes = rest[1..].bytes().take_while(|&c| c == b'#').count();
                let close = format!("\"{}", "#".repeat(hashes));
                let body = &rest[2 + hashes..];
                i + 2 + hashes + body.find(&close).map_or(body.len(), |n| n + close.len())
            }
            b'\'' if b.get(i + 1) == Some(&b'\\') => skip(i + 2, b'\''),
            b'\'' if b.get(char_end) == Some(&b'\'') => char_end + 1,
            c => {
                match c {
                    b'(' | b'[' | b'{' => open.push(c),
                    b')' | b']' | b'}' if open.pop().is_none() => lead.insert(0, c as char),
                    _ => {}
                }
                i + len(rest)
            }
        };
    }
    let pair = |c: char| match c {
        ')' => '(',
        ']' => '[',
        '}' => '{',
        '(' => ')',
        '[' => ']',
        _ => '}',
    };
    let head: String = lead.chars().map(pair).collect();
    let tail: String = open.iter().rev().map(|&c| pair(c as char)).collect();
    let padded = format!("{head}{region}{tail}");
    let Ok(found) = tokens(&padded) else {
        return vec![region];
    };
    let (n, k) = (head.len(), found.len() - tail.len());
    let ranges = found[n.min(k)..k]
        .iter()
        .map(|(r, _)| r.start - n..r.end - n);
    ranges.map(|r| &region[r]).collect()
}
fn flat(tokens: &[&str]) -> String {
    tokens.concat().replace('"', "")
}
fn pinned(source: &str, literal: &str) -> usize {
    let (source, literal) = (lex(source), piece(literal));
    let windows = source.windows(literal.len());
    windows.filter(|w| *w == literal).count()
}
fn header(code: &str, range: std::ops::Range<usize>) -> (Vec<String>, String) {
    let tokens = words(&code[range]);
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
/// SB3E-R44.3 (B5): the one `fn name` item's text, from `fn` to the close of
/// its body's `{}` token tree (proc-macro2's); `None` unless exactly one.
fn item<'a>(source: &'a str, name: &str) -> Option<&'a str> {
    let found = tokens(source).ok()?;
    let t = |i: usize| &source[found[i].0.clone()];
    let at: Vec<usize> = (1..found.len())
        .filter(|&i| t(i - 1) == "fn" && t(i) == name)
        .collect();
    let [at] = at[..] else { return None };
    let (open, mut depth) = ((at..found.len()).find(|&i| t(i) == "{")?, 0i32);
    let close = (open..found.len()).find(|&i| {
        depth += i32::from(t(i) == "{") - i32::from(t(i) == "}");
        depth == 0
    })?;
    Some(&source[found[at - 1].0.start..found[close].0.end])
}
/// A region's first eight tokens, as its findings name it.
fn label(pin: &str) -> String {
    let tokens = piece(pin);
    flat(&tokens[..tokens.len().min(8)])
}
/// G-S1: a file's `include`, `include_str` and `include_bytes` tokens and
/// its `path = ..` keys, as `file|kind|argument` (`?`: no argument). The
/// argument is the next token whatever it is (SB3E-R44.3: a raw or byte
/// string as written; anything but a listed literal is a finding).
fn includes(file: &str, source: &str) -> Vec<String> {
    let tokens = lex(source);
    let mut rows = Vec::new();
    for (i, t) in tokens.iter().enumerate() {
        let next = |k: usize| tokens.get(i + k).copied().unwrap_or("");
        if ["include", "include_str", "include_bytes"].contains(t) {
            let invoked = next(1) == "!" && next(2) == "(";
            rows.push(format!(
                "{file}|{t}|{}",
                if invoked { next(3) } else { "?" }
            ));
        }
        let opens = i
            .checked_sub(1)
            .is_some_and(|p| ["[", ",", "("].contains(&tokens[p]));
        if *t == "path" && opens && next(1) == "=" && ![">", "="].contains(&next(2)) {
            rows.push(format!("{file}|path|{}", next(2)));
        }
    }
    rows
}
/// Step 0 item 8 and SB3E-R42.2 over Mother's sources, every finding.
fn inventory_findings(sources: &[(String, String)]) -> Vec<String> {
    // SB3E-R44.3: every source tokenizes (proc-macro2) and lines up with its
    // text, or nothing below can scan it: each one that does not is a finding.
    let lexed = sources
        .iter()
        .filter_map(|(f, s)| tokens(s).err().map(|e| (f, e)));
    let lexed: Vec<String> = lexed.map(|(f, e)| lex_finding(f, &e)).collect();
    if !lexed.is_empty() {
        return lexed;
    }
    let mut found = Vec::new();
    let mut check = |ok: bool, what: String| {
        if !ok {
            found.push(what);
        }
    };
    let rows = sites(sources);
    let expected: Vec<&str> = SITES.lines().collect();
    for row in rows.iter().filter(|r| !expected.contains(&r.as_str())) {
        check(false, format!("sites: unlisted {row}"));
    }
    for row in expected.iter().filter(|e| !rows.iter().any(|r| r == *e)) {
        check(false, format!("sites: missing {row}"));
    }
    check(rows == expected, "sites: the route sites, in order".into());
    let (mut scanned, mut named) = (0, 0);
    for (file, source) in sources.iter().filter(|(f, _)| !exempt(f)) {
        named += seam_sites(source, &NAMES).len();
        let leaks = route_leaks(source);
        check(leaks == 0, format!("{file}: {leaks} route names ungated"));
        scanned += 1;
    }
    let counted = scanned > 100 && named == expected.len();
    check(counted, format!("sites: {scanned} files, {named} names"));
    for (file, pin) in pins() {
        let n = pinned(text(sources, file), pin);
        check(n == 1, format!("pin {file}: {} matched {n}", label(pin)));
    }
    // G-S1: S5's module is its pin, whole, and defines no macro.
    let seams = text(sources, "src/route_seams.rs");
    let tokens = lex(seams);
    let whole = pins()
        .into_iter()
        .filter(|(f, _)| *f == "src/route_seams.rs");
    let whole: Vec<Vec<&str>> = whole.map(|(_, pin)| lex(pin)).collect();
    let is_whole = whole == [tokens.clone()];
    check(
        is_whole,
        "G-S1: src/route_seams.rs is not its pin, whole".into(),
    );
    let defines = ["macro_rules", "macro", "macro_export"];
    let defined = tokens.iter().any(|t| defines.contains(t));
    check(
        !defined,
        "G-S1: a macro definition in src/route_seams.rs".into(),
    );
    // G-S1: no `include!`; exactly the listed `include_str!`, `include_bytes!`
    // and `#[path]` sites.
    let allowed = allowed_includes();
    let listed: Vec<String> = sources.iter().flat_map(|(f, s)| includes(f, s)).collect();
    for row in &listed {
        let ok = allowed.contains(&row.as_str()) && !row.contains("|include|");
        check(ok, format!("G-S1: {row} is not allowed"));
    }
    for row in &allowed {
        check(
            listed.iter().any(|l| l == row),
            format!("G-S1: listed {row} is gone"),
        );
    }
    for (file, hook) in HOOKS {
        let source = text(sources, file);
        let scanned = !seam_sites(source, &[hook]).is_empty();
        check(scanned, format!("hooks: {file}: {hook} not scanned"));
        let widened = leaks_under(source, &[hook], TEST);
        check(widened == 0, format!("hooks: {file}: {hook} widened"));
    }
    // Each marker exactly once in Mother's sources, none within another.
    let markers = sources
        .iter()
        .filter(|(f, _)| !exempt(f) || MODULES.contains(&f.as_str()));
    let markers = markers.flat_map(|(_, s)| s.split("\"nexus-mother-route-seam").skip(1));
    let markers = markers.map(|rest| {
        let name = rest.split('"').next().unwrap_or("");
        format!("nexus-mother-route-seam{name}")
    });
    let mut markers: Vec<String> = markers.collect();
    markers.sort();
    check(markers == MARKERS, format!("markers: {markers:?}"));
    // Each marker's file feeds it to `black_box` once (SB3E-R9, R42.6).
    let used = [
        "std",
        ":",
        ":",
        "hint",
        ":",
        ":",
        "black_box",
        "(",
        "M",
        ".",
        "as_bytes",
    ];
    for marker in MARKERS {
        let quoted = format!("\"{marker}\"");
        let holds = |(f, s): &&(String, String)| {
            (!exempt(f) || MODULES.contains(&f.as_str())) && s.contains(&quoted)
        };
        for (file, source) in sources.iter().filter(holds) {
            let tokens = lex(source);
            let fed = tokens.windows(used.len()).filter(|w| *w == used).count();
            check(
                fed == 1,
                format!("markers: {file}: {marker} fed {fed} times"),
            );
        }
    }
    // S5's module: `arm` and `drain_trace` its only `pub fn`, no further
    // function, no `#[used]`, the timeout called once, and every point armed
    // is an audited site's.
    let public = tokens.windows(3).filter(|w| w[..2] == ["pub", "fn"]);
    let public: Vec<&str> = public.map(|w| w[2]).collect();
    check(
        public == ["arm", "drain_trace"],
        format!("S5: public {public:?}"),
    );
    let functions = tokens.iter().filter(|t| **t == "fn").count();
    check(functions == 10, format!("S5: {functions} functions"));
    check(!seams.contains("#[used]"), "S5: #[used] (SB3E-R9)".into());
    let calls = tokens.windows(2).filter(|w| w == &["timeout", "("]).count();
    check(
        calls == 1,
        format!("S5: {calls} timeout calls (SB3E-R39.1)"),
    );
    let mut audited = Vec::new();
    let mother = |f: &str| f.starts_with("src/");
    for (_, source) in sources.iter().filter(|(f, _)| !exempt(f) && mother(f)) {
        let tokens = lex(source);
        let at = tokens
            .windows(6)
            .filter(|w| w[..5] == ["route_seams", ":", ":", "at", "("]);
        audited.extend(at.map(|w| w[5].trim_matches('"').to_string()));
    }
    audited.sort();
    let listed = seams.split("POINTS: [&str; 10] = [").nth(1).unwrap_or("");
    let listed = listed.split("];").next().unwrap_or("");
    let mut listed: Vec<String> = listed
        .split('"')
        .skip(1)
        .step_by(2)
        .map(String::from)
        .collect();
    listed.sort();
    check(
        audited == listed,
        format!("S5: armable {listed:?}, audited {audited:?}"),
    );
    let declared = text(sources, "src/lib.rs");
    let declaration = "\n#[cfg(mother_route_seams)]\npub mod route_seams;\n";
    check(declared.contains(declaration), "S5: its declaration".into());
    let host = text(sources, "host/src/route_seams.rs");
    let ht = lex(host);
    let whole = pins()
        .into_iter()
        .filter(|(f, _)| *f == "host/src/route_seams.rs");
    let whole: Vec<Vec<&str>> = whole.map(|(_, pin)| lex(pin)).collect();
    check(
        whole == [ht.clone()],
        "S7: host/src/route_seams.rs is not its pin, whole".into(),
    );
    check(
        !ht.iter().any(|t| defines.contains(t)),
        "S7: a macro definition".into(),
    );
    let public = ht.windows(3).filter(|w| w[..2] == ["pub", "fn"]);
    let public: Vec<&str> = public.map(|w| w[2]).collect();
    check(
        public == ["arm", "drain_trace"],
        format!("S7: public {public:?}"),
    );
    check(!host.contains("#[used]"), "S7: #[used] (SB3E-R9)".into());
    let calls = ht.windows(2).filter(|w| w == &["timeout", "("]).count();
    check(calls == 1, format!("S7: {calls} timeout calls"));
    let host_lib = text(sources, "host/src/lib.rs");
    check(host_lib.contains(declaration), "S7: its declaration".into());
    let mut reached = Vec::new();
    for (_, source) in sources
        .iter()
        .filter(|(f, _)| !exempt(f) && f.starts_with("host/src/"))
    {
        let t = lex(source);
        let at = t
            .windows(6)
            .filter(|w| w[..5] == ["route_seams", ":", ":", "at", "("]);
        reached.extend(at.map(|w| w[5].trim_matches('"').to_string()));
    }
    reached.sort();
    reached.dedup();
    let points = host.split("POINTS: [&str; 6] = [").nth(1).unwrap_or("");
    let points = points.split("];").next().unwrap_or("");
    let points: Vec<String> = points
        .split('"')
        .skip(1)
        .step_by(2)
        .map(String::from)
        .collect();
    check(
        reached == points,
        format!("S7: armable {points:?}, reached {reached:?}"),
    );
    // SB3E-R44.2: `s5.rs`'s reach. Its parent's items are exactly these (the
    // reached ones also pinned whole, above), and no `impl` in the harness
    // is added (S2: no other `impl` of `Fixture` or `Opened`).
    let main = items(text(sources, "host/tests/route_cells/main.rs"));
    let harness = sources
        .iter()
        .filter(|(f, _)| f.starts_with("host/tests/route_cells/") && f != S5);
    let harness = harness.flat_map(|(f, s)| impls(s).into_iter().map(move |r| format!("{f}|{r}")));
    let harness: Vec<String> = harness.collect();
    for (what, rows, listed) in [
        ("main.rs items", main, MAIN_ITEMS),
        ("harness impls", harness, IMPLS),
    ] {
        let listed: Vec<&str> = listed.lines().collect();
        for row in rows.iter().filter(|r| !listed.contains(&r.as_str())) {
            check(false, format!("R44.2 {what}: unlisted {row}"));
        }
        for row in listed.iter().filter(|l| !rows.iter().any(|r| r == *l)) {
            check(false, format!("R44.2 {what}: missing {row}"));
        }
        check(
            rows == listed,
            format!("R44.2 {what}: not exactly the list"),
        );
    }
    for finding in MANIFESTS.iter().flat_map(|(f, m)| manifest_findings(f, m)) {
        check(false, finding);
    }
    // SB3E-R57.6, R86.4: the `cfg!` check over the harness, the `cells!`
    // macro verifier and the T7 composition guard.
    let harness = sources
        .iter()
        .filter(|(f, _)| f.starts_with(CELLS) && f.ends_with(".rs"));
    let mut more: Vec<String> = harness.flat_map(|(f, s)| cfg_findings(f, s)).collect();
    let (main, cells) = (format!("{CELLS}main.rs"), format!("{CELLS}cells.rs"));
    more.extend(macro_findings(text(sources, &main), text(sources, &cells)));
    more.extend(t7_findings(sources));
    more.extend(reader_timeout_findings(text(sources, READER)));
    more.extend(order_findings(text(sources, "host/src/hands.rs")));
    more.extend(close_caller_findings(sources));
    for finding in more {
        check(false, finding);
    }
    found
}
/// U21a (SB3E-R197 TR-1, R199 Q-U21-4): the production order that makes
/// the close_report rows x:222-234 unreachable and keeps the 11 hold rows'
/// reasons, over `hands.rs`'s tokens (no comment or whitespace): the reader
/// is joined before the `hands-close` worker spawns, the spawn is under
/// `!ingress.failed()` and the Quit request, `finish_delivery` follows the
/// worker's join, and authority settles before `close_report` runs. A
/// change to any of these reopens those rows (set B).
fn order_findings(hands: &str) -> Vec<String> {
    if tokens(hands).is_err() {
        return vec!["order: host/src/hands.rs does not tokenize".into()];
    }
    let code: String = lex(hands).concat();
    let once = |pat: &str| {
        (code.matches(pat).count() == 1)
            .then(|| code.find(pat))
            .flatten()
    };
    let stop = once("letreader_done=terminal.stop_reader(producers_until);");
    let cond = once(
        "ifreader_done&&work_done&&stops_done&&producers_done&&!screen.ended()&&!ingress.failed(){",
    );
    let quit = once("ifletSome(request)=request{letowned=std::mem::ManuallyDrop::new(runtime);");
    let spawn = once(".name(\"hands-close\".into())");
    let join = once("join(&mutclose,producers_until,&screen)");
    let finish = once("seal.finish_delivery()");
    let settle = once(".settle_close_authority(&request)");
    let sealed = once("(Some(seal),self.close_report(\"delivery_pending\"))");
    let refused = once("(None,self.close_report(stage))");
    let before = |a: Option<usize>, b: Option<usize>| a.zip(b).is_some_and(|(a, b)| a < b);
    let mut found = Vec::new();
    for (ok, what) in [
        (
            before(stop, spawn),
            "the reader is not joined before hands-close spawns",
        ),
        (
            before(stop, cond) && before(cond, quit),
            "hands-close is not under !ingress.failed()",
        ),
        (
            before(quit, spawn),
            "hands-close is not under the Quit request",
        ),
        (
            before(spawn, join) && before(join, finish),
            "finish_delivery is not after the close join",
        ),
        (
            before(settle, sealed) && before(settle, refused),
            "authority does not settle before close_report",
        ),
    ] {
        if !ok {
            found.push(format!("order: {what}"));
        }
    }
    found
}
/// U20a (SB3E-R214): the production callers that make the 45 set-B rows of
/// the five close sinks unreachable (docs 14.19, O1-O3). Each row is
/// `file|fn|method`: the enclosing `fn` of every production call site, over
/// tokens (a comment or a literal is no call). R9 sinks: only `Runtime::close`
/// and `close_report`, on the worker that spawns after the reader is joined.
/// R6 sinks and Stop: only the reader's `receive`, `read_input` and `press`.
const CLOSE_CALLERS: &str = "\
host/src/hands.rs|close|settle_close_resources
host/src/hands.rs|close|seal_close
host/src/hands.rs|close_report|close_obligations
host/src/stop.rs|press|emergency
host/src/terminal.rs|receive|accept
host/src/terminal.rs|receive|accept
host/src/terminal.rs|receive|press
host/src/terminal.rs|receive|press
host/src/terminal.rs|read_input|press
host/src/terminal.rs|read_input|press
";
/// Not production source: a `tests/` or `examples/` folder, a `_tests.rs`
/// file and the two legacy modules (clause (e) keeps them off the shipped
/// path). A module declared under `#[cfg(test)]` is excluded with its files.
const CALLER_EXCLUDED: [&str; 4] = ["tests/", "examples/", "_tests.rs", "legacy_"];
/// Clause (c) reads these roots: the desktop's `press` is its own
/// `StopRoute` (desktop/src/stop.rs), which holds no `SessionIngress`.
const PRESS_ROOTS: [&str; 2] = ["host/src/", "src/"];
/// Clause (e): the shipped host files, and `lib.rs` but for its `mod` lines.
const SHIPPED: [&str; 5] = [
    "host/src/bin/",
    "host/src/hands.rs",
    "host/src/terminal.rs",
    "host/src/stop.rs",
    "host/src/lib.rs",
];
fn caller_excluded(file: &str, test_mods: &[String]) -> bool {
    let name = file.rsplit('/').next().unwrap_or(file);
    let dir = |p: &str| file.starts_with(p) || file.contains(&format!("/{p}"));
    let [tests, examples, suffix, legacy] = CALLER_EXCLUDED;
    dir(tests)
        || dir(examples)
        || file.ends_with(suffix)
        || name.starts_with(legacy)
        || test_mods
            .iter()
            .any(|m| file == format!("{m}.rs") || file.starts_with(&format!("{m}/")))
}
/// `#[cfg(test)]` at `t[i..]`.
fn cfg_test(t: &[&str], i: usize) -> bool {
    t.get(i..i + 7) == Some(&["#", "[", "cfg", "(", "test", ")", "]"][..])
}
/// The module paths (without `.rs`) that a file declares under `#[cfg(test)]`.
fn test_mods(file: &str, t: &[&str]) -> Vec<String> {
    let parent = file.strip_suffix(".rs").unwrap_or(file);
    let parent = match ["/mod", "/lib", "/main"]
        .iter()
        .any(|e| parent.ends_with(e))
    {
        true => parent.rsplit_once('/').map_or("", |p| p.0),
        false => parent,
    };
    let (mut found, mut test) = (Vec::new(), false);
    for i in 0..t.len() {
        test = (test || cfg_test(t, i)) && !matches!(t[i], ";" | "{" | "}");
        if test && t[i] == "mod" && t.get(i + 2) == Some(&";") {
            found.push(format!("{parent}/{}", t[i + 1]));
        }
    }
    found
}
/// Each non-test call site of `t` as `(enclosing fn, method)`: `.accept(..)`
/// with arguments, `.emergency()`, `.press()`, and the three R9 methods.
fn close_calls<'a>(t: &[&'a str]) -> Vec<(&'a str, &'a str)> {
    let (mut found, mut stack) = (Vec::new(), Vec::<(Option<&str>, bool)>::new());
    let (mut name, mut test) = (None, false);
    for i in 0..t.len() {
        test |= cfg_test(t, i);
        match t[i] {
            "fn" => {
                name = t
                    .get(i + 1)
                    .copied()
                    .filter(|n| n.starts_with(char::is_alphabetic))
            }
            "{" => stack.push((name.take(), std::mem::take(&mut test))),
            // An attribute's item ends at its `;` or at the block it sits in.
            "}" => {
                stack.pop();
                (name, test) = (None, false);
            }
            ";" => (name, test) = (None, false),
            _ => {}
        }
        let (dot, method, open) = (t[i], t.get(i + 1).copied(), t.get(i + 2).copied());
        let empty = t.get(i + 3) == Some(&")");
        let call = match method {
            Some("accept") => !empty,
            Some("emergency" | "press") => empty,
            Some("close_obligations" | "settle_close_resources" | "seal_close") => true,
            _ => false,
        };
        if dot == "." && open == Some("(") && call && !test && !stack.iter().any(|s| s.1) {
            let within = stack.iter().rev().find_map(|s| s.0).unwrap_or("-");
            found.push((within, method.unwrap_or("")));
        }
    }
    found
}
fn close_caller_findings(sources: &[(String, String)]) -> Vec<String> {
    let lexed: Vec<(&str, Vec<&str>)> = sources
        .iter()
        .filter(|(_, s)| tokens(s).is_ok())
        .map(|(f, s)| (f.as_str(), lex(s)))
        .collect();
    let mods: Vec<String> = lexed.iter().flat_map(|(f, t)| test_mods(f, t)).collect();
    let mut rows = Vec::new();
    for (file, t) in lexed.iter().filter(|(f, _)| !caller_excluded(f, &mods)) {
        for (within, method) in close_calls(t) {
            if method != "press" || PRESS_ROOTS.iter().any(|r| file.starts_with(r)) {
                rows.push(format!("{file}|{within}|{method}"));
            }
        }
    }
    let clause = |row: &str| match row.rsplit('|').next() {
        Some("accept" | "emergency") => "(b)",
        Some("press") => "(c)",
        _ => "(a)",
    };
    let (mut listed, mut found): (Vec<&str>, _) = (CLOSE_CALLERS.lines().collect(), Vec::new());
    for row in &rows {
        match listed.iter().position(|l| l == row) {
            Some(at) => {
                listed.remove(at);
            }
            None => found.push(format!("close callers {}: unlisted {row}", clause(row))),
        }
    }
    found.extend(
        listed
            .iter()
            .map(|l| format!("close callers {}: missing {l}", clause(l))),
    );
    // (d): U21a's O1 rule itself, not a copy of it.
    let hands = sources.iter().find(|(f, _)| f == "host/src/hands.rs");
    let order = hands.map_or_else(
        || vec!["no host/src/hands.rs".into()],
        |h| order_findings(&h.1),
    );
    let kept = [
        "no host/src/hands.rs",
        "does not tokenize",
        "the reader is not",
    ];
    let reader = order.iter().filter(|o| kept.iter().any(|k| o.contains(k)));
    found.extend(reader.map(|o| format!("close callers (d): {o}")));
    for (file, t) in lexed
        .iter()
        .filter(|(f, _)| SHIPPED.iter().any(|s| f.starts_with(s)))
    {
        for (i, w) in t.iter().enumerate() {
            let modded = *file == "host/src/lib.rs" && i > 0 && t[i - 1] == "mod";
            if ["legacy_terminal", "legacy_stop"].contains(w) && !modded {
                found.push(format!("close callers (e): {file} uses {w}"));
            }
        }
    }
    found
}
/// SB3E-R51.1: each harness file not listed, missing, or not its digest.
fn cells_findings(files: &[(String, Vec<u8>)]) -> Vec<String> {
    use sha2::Digest;
    let pin = |f: &str| route_cells().find(|r| Some(r.0) == f.strip_prefix(CELLS));
    let mut found = rows_findings(ROUTE_CELLS);
    for (file, bytes) in files {
        let digest = hex::encode(sha2::Sha256::digest(bytes));
        match pin(file) {
            None => found.push(format!("R51.1: {file} is not listed")),
            Some(row) if row.1 != digest => {
                found.push(format!("R51.1: {file}'s sha256 is {digest}"))
            }
            Some(_) => {}
        }
    }
    for (name, _) in route_cells() {
        if !files
            .iter()
            .any(|(f, _)| f.strip_prefix(CELLS) == Some(name))
        {
            found.push(format!("R51.1: {CELLS}{name} is missing"));
        }
    }
    found
}
/// SB3E-R57.6: in every `.rs` file of the harness each `cfg!` invocation is
/// exactly `cfg!(mother_route_seams)`; any other predicate or delimiter is a
/// finding naming the file (a `#[cfg(..)]` attribute is not an invocation).
fn cfg_findings(file: &str, source: &str) -> Vec<String> {
    let t = lex(source);
    let exact = ["cfg", "!", "(", "mother_route_seams", ")"];
    let at = (0..t.len()).filter(|&i| t[i] == "cfg" && t.get(i + 1) == Some(&"!"));
    let other = at.filter(|&i| t.get(i..i + 5) != Some(&exact[..]));
    let shown = |i: usize| flat(&t[i..(i + 6).min(t.len())]);
    let other = other.map(|i| (file, shown(i)));
    other
        .map(|(f, s)| format!("R57.6: {f}: a cfg! other than cfg!(mother_route_seams): {s}"))
        .collect()
}
/// The index of the bracket that closes `t[open]` (`t.len()` if none).
fn close_of(t: &[&str], open: usize) -> usize {
    let mut depth = 0i32;
    for (i, x) in t.iter().enumerate().skip(open) {
        depth += i32::from(["(", "[", "{"].contains(x)) - i32::from([")", "]", "}"].contains(x));
        if depth == 0 {
            return i;
        }
    }
    t.len()
}
/// `cells.rs`'s literal manifest, as slugs (`rc_<route>_<sink>_<dim>_<value>`).
fn manifest_slugs(cells: &str) -> Vec<String> {
    let t = lex(cells);
    let start = (0..t.len()).find(|&i| t[i..].starts_with(&["cells", "!", "{"]));
    let Some(start) = start else {
        return Vec::new();
    };
    let body = &t[start + 3..close_of(&t, start + 2)];
    let entries = body.split(|w| *w == ";").filter(|e| !e.is_empty());
    entries
        .map(|e| format!("rc_{}", e.join("_").to_lowercase()))
        .collect()
}
/// W0b B5: each `<name>!( .. )` invocation in `body`, as its argument tokens.
fn invocations<'a, 'b>(body: &'b [&'a str], name: &str) -> Vec<&'b [&'a str]> {
    let at = (0..body.len()).filter(|&k| body[k..].starts_with(&[name, "!", "("]));
    at.map(|k| &body[k + 3..close_of(body, k + 2).min(body.len())])
        .collect()
}
/// SB3E-R86.4 (U5a; R49.4; mission Behaviours 1, 2), W0b B5 (R94.2): `main.rs`'s
/// `macro_rules! cells`, read through the guard's own sources. Each cell arm
/// is exactly `(<route> <sink> <dim> <value>; $($rest:tt)*) => { cell!(<slug>,
/// <route> <sink> <dim> <value>, <stem>::<fn>); cells! { $($rest)* } };`: one
/// test, its slug, its pattern's own literals, one driver no other arm names
/// in the section owning its pair, one compare (the helper's). The terminal
/// arm `()` is one `ctl!(<control>, <sink> <dim>, <stem>::<fn>);` per control,
/// each once (W0b C1: `<control>` is `ctl_<sink>`, its section owns the pair);
/// the arms are exactly `cells.rs`'s manifest. The helpers `cell` and `ctl`
/// and the item are one pinned region, whole and token-equal.
fn macro_findings(main: &str, cells: &str) -> Vec<String> {
    let t = lex(main);
    let head = ["macro_rules", "!", "cells", "{"];
    let Some(start) = (0..t.len()).find(|&i| t[i..].starts_with(&head)) else {
        return vec!["U5 macro: no macro_rules! cells in host/tests/route_cells/main.rs".into()];
    };
    let (end, mut i) = (close_of(&t, start + 3), start + 4);
    let (mut found, mut arms, mut terminal, mut used) = (Vec::new(), Vec::new(), 0, Vec::new());
    // R102.3 (allow-list): compact arms hold no `#` (a raw identifier or an
    // attribute), and each helper invocation has exactly its closed shape.
    if t[start..end.min(t.len())].contains(&"#") {
        found.push("U5 macro: a # in macro_rules! cells".to_string());
    }
    let ident = |k: &usize, a: &[&str]| {
        a.get(*k).is_some_and(|w| {
            !w.is_empty() && w.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_')
        })
    };
    let shaped = |a: &[&str], len: usize, commas: &[usize], ids: &[usize]| {
        a.len() == len
            && commas.iter().all(|&k| a[k] == ",")
            && a[len - 3..len - 1] == [":", ":"]
            && ids.iter().all(|k| ident(k, a))
    };
    let tail = [";", "$", "(", "$", "rest", ":", "tt", ")", "*"];
    let again = ["cells", "!", "{", "$", "(", "$", "rest", ")", "*", "}"];
    let compares = |body: &[&str]| {
        let path = |w: &&[&str]| {
            w[..3] == ["witness", ":", ":"] && ["credited", "control"].contains(&w[3])
        };
        body.windows(4).filter(path).count()
    };
    while i < end {
        if t[i] != "(" {
            found.push(format!("U5 macro: an arm opens with {}", t[i]));
            break;
        }
        let pattern = &t[i + 1..close_of(&t, i)];
        let open = (close_of(&t, i)..end).find(|&k| t[k] == "{").unwrap_or(end);
        let body = &t[(open + 1).min(end)..close_of(&t, open).min(end)];
        let named = drivers(body);
        if pattern.is_empty() {
            terminal += 1;
            let ctls = invocations(body, "ctl");
            let defined: Vec<&str> = ctls.iter().filter_map(|a| a.first().copied()).collect();
            let form: Vec<&str> = ctls
                .iter()
                .flat_map(|a| [&["ctl", "!", "("][..], a, &[")", ";"]].concat())
                .collect();
            if defined.is_empty() || body != form.as_slice() {
                found.push(format!("U5 macro: the terminal arm defines {defined:?}"));
            }
            for (k, c) in defined
                .iter()
                .enumerate()
                .filter(|(k, c)| defined[..*k].contains(c))
            {
                found.push(format!("U5 macro: the control {c} appears twice (at {k})"));
            }
            for args in ctls
                .iter()
                .filter(|a| !shaped(a, 9, &[1, 4], &[0, 2, 3, 5, 8]))
            {
                found.push(format!(
                    "U5 macro: the control {} is not ctl!(<control>, <sink> <dim>, <stem>::<fn>)",
                    flat(args)
                ));
            }
            used.extend(named.iter().map(|d| (*d, "the terminal arm".to_string())));
            found.extend(control_binding(body));
        } else {
            let words = pattern.iter().take_while(|w| **w != ";").count();
            // U9a M1 (F-B): the slug is lowercase, as the manifest's and the
            // register's; the pattern keeps the value's case.
            let slug = format!("rc_{}", pattern[..words].join("_").to_lowercase());
            let recurses = body.ends_with(&again);
            if words != 4 || pattern[words..] != tail || !recurses {
                found.push(format!(
                    "U5 macro: the arm {slug} is not a recursing cell arm"
                ));
            }
            if arms.contains(&slug) {
                found.push(format!("U5 macro: the arm {slug} appears twice"));
            }
            let calls = invocations(body, "cell");
            let defined: Vec<&str> = calls.iter().filter_map(|a| a.first().copied()).collect();
            if defined != [slug.as_str()] {
                found.push(format!("U5 macro: the arm {slug} defines {defined:?}"));
            }
            if calls
                .iter()
                .any(|a| !shaped(a, 11, &[1, 6], &[0, 2, 3, 4, 5, 7, 10]))
            {
                found.push(format!(
                    "U5 macro: the arm {slug} is not cell!(<slug>, <route> <sink> <dim> <value>, <stem>::<fn>)"
                ));
            }
            for args in &calls {
                if args.get(2..6) != Some(&pattern[..words.min(4)]) {
                    found.push(format!(
                        "U5 macro: the arm {slug} passes {}, not its own literals",
                        flat(args)
                    ));
                }
            }
            if named.len() != 1 {
                found.push(format!(
                    "U5 macro: the arm {slug} has {} drivers",
                    named.len()
                ));
            }
            let compared = calls.len() + compares(body);
            if compared != 1 {
                found.push(format!("U5 macro: the arm {slug} has {compared} compares"));
            }
            let line = calls
                .first()
                .map(|a| [&["cell", "!", "("][..], a, &[")", ";"], &again].concat());
            if line.as_deref() != Some(body) {
                found.push(format!("U5 macro: the arm {slug} is not one cell! line"));
            }
            used.extend(named.iter().map(|d| (*d, slug.clone())));
            if words == 4 {
                found.extend(arm_binding(&slug, (pattern[1], pattern[2]), body));
            }
            arms.push(slug);
        }
        i = close_of(&t, open) + 1;
        i += usize::from(t.get(i) == Some(&";"));
    }
    if terminal != 1 {
        found.push(format!("U5 macro: {terminal} terminal arms"));
    }
    // One driver, one witness: no driver is named by two arms or controls.
    for (k, (d, _)) in used.iter().enumerate() {
        if used[..k].iter().any(|(e, _)| e == d) {
            found.push(format!(
                "U5 macro: the driver {}::{} is named by two arms",
                d.0, d.1
            ));
        }
    }
    // W0b N7: `cells.rs` gets G-B1 as `s5.rs` and the sections do: no raw
    // identifier, no macro definition, no macro but its one `cells!`.
    let ct = lex(cells);
    let cells_bang = (0..ct.len()).filter(|&i| ct[i] == "cells" && ct.get(i + 1) == Some(&"!"));
    if cells_bang.count() != 1 {
        found.push("U5 macro: cells.rs does not invoke cells! once".into());
    }
    let listed = |f: &String| f != "G-B1: an unlisted macro cells!";
    found.extend(
        gb1_findings(&mask(cells), &ct)
            .into_iter()
            .filter(listed)
            .map(|f| format!("cells.rs {f}")),
    );
    let manifest = manifest_slugs(cells);
    for arm in arms.iter().filter(|a| !manifest.contains(a)) {
        found.push(format!("U5 macro: the arm {arm} has no manifest entry"));
    }
    for entry in manifest.iter().filter(|m| !arms.contains(m)) {
        found.push(format!("U5 macro: the manifest entry {entry} has no arm"));
    }
    found
}
/// F-S5-SPLIT item 1(d), W0b B5: the drivers a `cells!` arm names: each
/// `<stem>::<fn>` in a `cell!` or `ctl!` invocation (R94.2's compact form,
/// which its helper expands to `super::<stem>::<fn>`), and any other
/// `super::<stem>::<fn>` path; the runner `super::s5::cell` and
/// `super::witness::*` are not drivers.
fn drivers<'a>(body: &[&'a str]) -> Vec<(&'a str, &'a str)> {
    let path = |k: &usize| {
        body[*k..].starts_with(&["super", ":", ":"])
            && body.get(k + 4..k + 6) == Some(&[":", ":"][..])
    };
    let at = (0..body.len())
        .filter(path)
        .filter_map(|k| Some((body[k + 3], *body.get(k + 6)?)));
    let mut named: Vec<(&str, &str)> = at
        .filter(|&(s, f)| s != "witness" && (s, f) != ("s5", "cell"))
        .collect();
    for args in invocations(body, "cell")
        .into_iter()
        .chain(invocations(body, "ctl"))
    {
        named.extend(paths(args));
    }
    named
}
/// Item 1(d): a cell arm for `(sink, dim)` names its driver in the section
/// that owns the pair.
fn arm_binding(slug: &str, (sink, dim): (&str, &str), body: &[&str]) -> Vec<String> {
    let pair = format!("{sink}:{dim}");
    let rows = section_rows(S5_SECTIONS);
    let Some(owner) = rows.iter().find(|r| r.pairs.contains(&pair.as_str())) else {
        return vec![format!(
            "S5 sections: the arm {slug} has no section owning {pair}"
        )];
    };
    let named = drivers(body);
    let mut found: Vec<String> = named
        .iter()
        .filter(|(s, _)| *s != stem(owner.file))
        .map(|(s, _)| format!("S5 sections: the arm {slug} calls {s}, which does not own {pair}"))
        .collect();
    if named.is_empty() {
        found.push(format!("S5 sections: the arm {slug} names no driver"));
    }
    found
}
/// W0b B5: each `<stem>::<fn>` in an invocation's arguments.
fn paths<'a>(args: &[&'a str]) -> Vec<(&'a str, &'a str)> {
    let at = (1..args.len().saturating_sub(2)).filter(|&k| args[k..k + 2] == [":", ":"]);
    at.filter(|&k| args[k - 1] != "super")
        .map(|k| (args[k - 1], args[k + 2]))
        .collect()
}
/// Item 1(d), W0b B5, C1: each control of the terminal arm (`ctl!(<control>,
/// <sink> <dim>, <stem>::<fn>)`) is `ctl_<sink>`, and the section that lists
/// it owns `<sink>:<dim>` and has its driver.
fn control_binding(body: &[&str]) -> Vec<String> {
    let rows = section_rows(S5_SECTIONS);
    let mut found = Vec::new();
    for args in invocations(body, "ctl") {
        let Some(&control) = args.first() else {
            continue;
        };
        let Some(row) = rows.iter().find(|r| r.controls.contains(&control)) else {
            found.push(format!(
                "S5 sections: the control {control} is listed by no section"
            ));
            continue;
        };
        let (sink, dim) = (args.get(2).copied(), args.get(3).copied());
        let pair = format!("{}:{}", sink.unwrap_or(""), dim.unwrap_or(""));
        if Some(control) != sink.map(|s| format!("ctl_{s}")).as_deref()
            || !row.pairs.contains(&&*pair)
        {
            found.push(format!("S5 sections: the control {control} is not ctl_<sink> of {pair}, owned by its section"));
        }
        for (s, _) in paths(args)
            .into_iter()
            .filter(|(s, _)| *s != stem(row.file))
        {
            found.push(format!(
                "S5 sections: {control} calls {s}, which does not list it"
            ));
        }
    }
    found
}
/// SB3E-R86.4 (U5a; design 1.4 T7; mission Behaviour 12, I8): `s5.rs`
/// composes the route with production's constructors (`hands.rs`,
/// `session_home.rs`): each `file|tokens|count` row exactly, space-separated
/// tokens. The differences are `T7_DIFFERENCES`, by name. SB3E-R88.2: the
/// counts are supplementary; production's composition itself, `hands.rs`
/// `fn run` and `session_home.rs` `fn open_projected`, is two pinned regions,
/// whole and token-equal, so no inactive copy or other branch can stand in.
const T7: &str = "\
host/src/hands.rs|SessionPort : : new (|1
host/src/hands.rs|Coordinator : : open (|1
host/src/hands.rs|. into_file_session (|1
host/src/hands.rs|open_confirmed_file_door (|1
host/src/hands.rs|open_file_door (|0
host/src/hands.rs|SystemClock : : default (|1
host/src/hands.rs|Terminal : : open (|1
host/src/session_home.rs|open_with_history (|1
host/src/session_home.rs|HostSession : : open (|0
host/src/session_home.rs|bind_history_inspector (|1
host/tests/route_cells/s5.rs|SessionPort : : new (|1
host/tests/route_cells/s5.rs|Coordinator : : open (|1
host/tests/route_cells/s5.rs|. into_file_session (|1
host/tests/route_cells/s5.rs|open_confirmed_file_door (|0
host/tests/route_cells/s5.rs|open_file_door (|1
host/tests/route_cells/s5.rs|SystemClock : : default (|2
host/tests/route_cells/s5.rs|open_with_history (|1
host/tests/route_cells/s5.rs|bind_history_inspector (|0
host/tests/route_cells/reader.rs|Terminal : : open (|1
host/tests/route_cells/reader.rs|route_bind (|1
";
/// I8: the named differences of `s5.rs`'s composition from production's.
const T7_DIFFERENCES: [&str; 6] = [
    "open_file_door, not open_confirmed_file_door (no Owner dialog)",
    "no history inspector (no bind_history_inspector: the seal's history check is the broker's own replay, recovery.rs:168-200)",
    "test lanes (ModelLane, RecordingLane) for production's",
    "a test confirmer (Answers, as main.rs Owner)",
    "reader.rs: the terminal opened after the session (hands.rs opens it first)",
    "reader.rs: stop recording bound through S6 route_bind (hands.rs: StopRoute::bind)",
];
/// The only `Type::name(` paths of the composed types `s5.rs` may take.
const T7_PATHS: [&str; 7] = [
    "Coordinator::open(",
    "FileJournal::open(",
    "FilePrivate::open_session(",
    "HostSession::open_with_history(",
    "Lanes::new(",
    "PrimaryEvidence::open(",
    "SessionPort::new(",
];
const T7_TYPES: [&str; 8] = [
    "Coordinator",
    "FileJournal",
    "FilePrivate",
    "FileSession",
    "HostSession",
    "Lanes",
    "PrimaryEvidence",
    "SessionPort",
];
fn t7_findings(sources: &[(String, String)]) -> Vec<String> {
    let mut found = Vec::new();
    for row in T7.lines() {
        let row: Vec<&str> = row.split('|').collect();
        let (file, needle) = (row[0], row[1].split(' ').collect::<Vec<_>>());
        let want: usize = row[2].parse().unwrap_or(usize::MAX);
        let tokens = lex(text(sources, file));
        let got = tokens
            .windows(needle.len())
            .filter(|w| *w == needle)
            .count();
        if got != want {
            found.push(format!("T7: {file}: {} {got} times, not {want}", row[1]));
        }
    }
    let s5 = lex(text(sources, S5));
    for path in composed(&s5) {
        if !T7_PATHS.contains(&path.as_str()) {
            found.push(format!("T7: {S5} constructs {path}"));
        }
    }
    // F-S5-SPLIT section 4 item 6: in a section every `s5.rs` needle counts
    // 0, and no composed type is constructed.
    for row in section_rows(S5_SECTIONS)
        .iter()
        .filter(|r| r.file != "s5.rs")
    {
        let file = format!("{CELLS}{}", row.file);
        let Some(source) = find(sources, &file).filter(|s| tokens(s).is_ok()) else {
            continue;
        };
        let tokens = lex(source);
        let needles = T7.lines().filter_map(|r| r.strip_prefix(&format!("{S5}|")));
        for needle in needles.filter_map(|r| r.split('|').next()) {
            let words: Vec<&str> = needle.split(' ').collect();
            let got = tokens.windows(words.len()).filter(|w| *w == words).count();
            if got != 0 {
                found.push(format!("T7: {file}: {needle} {got} times, not 0"));
            }
        }
        for path in composed(&tokens) {
            found.push(format!("T7: {file} constructs {path}"));
        }
    }
    if !found.is_empty() {
        found.push(format!(
            "T7: the named differences are only {T7_DIFFERENCES:?}"
        ));
    }
    found
}
/// The `Type::name(` paths of the composed types (`T7_TYPES`) in `tokens`.
fn composed(tokens: &[&str]) -> Vec<String> {
    let paths = tokens
        .windows(5)
        .filter(|w| T7_TYPES.contains(&w[0]) && w[1..3] == [":", ":"] && w[4] == "(");
    paths.map(|w| format!("{}::{}(", w[0], w[3])).collect()
}
/// `file`'s text in `sources`, if it is there.
fn find<'a>(sources: &'a [(String, String)], file: &str) -> Option<&'a str> {
    sources
        .iter()
        .find(|(f, _)| f == file)
        .map(|(_, s)| s.as_str())
}
/// SB3E-R44.3, R51.2: a source `tokens` refuses, as its finding.
fn lex_finding(file: &str, e: &str) -> String {
    match e == SHEBANG {
        true => format!("lex: {file} {SHEBANG}"),
        false => format!("lex: {file} does not tokenize: {e}"),
    }
}
/// SB3E-R44.3: the manifest names proc-macro2 once, by exactly `LEXER_DEP`,
/// in its one `[dev-dependencies]` table.
fn manifest_findings(file: &str, manifest: &str) -> Vec<String> {
    let named = manifest
        .lines()
        .filter(|l| l.contains("proc-macro2") || l.contains("proc_macro2"));
    let named: Vec<&str> = named.collect();
    let dev = manifest
        .split("\n[dev-dependencies]\n")
        .nth(1)
        .unwrap_or("");
    let dev = dev
        .split("\n[")
        .next()
        .unwrap_or("")
        .lines()
        .any(|l| l == LEXER_DEP);
    let once = manifest.matches("[dev-dependencies]").count() == 1;
    match named == [LEXER_DEP] && dev && once {
        true => Vec::new(),
        false => vec![format!("R44.3: {file}: proc-macro2 {named:?}")],
    }
}
/// SB3E-R44.2: each top-level item of `source` as one word, attributes
/// included, to its body's `{` (an item that ends in `;`, whole).
fn items(source: &str) -> Vec<String> {
    let t = lex(source);
    let (mut rows, mut start, mut open, mut depth) = (Vec::new(), 0, 0, 0i32);
    for i in 0..t.len() {
        open = if depth == 0 && t[i] == "{" { i } else { open };
        depth +=
            i32::from(["(", "[", "{"].contains(&t[i])) - i32::from([")", "]", "}"].contains(&t[i]));
        let end = t[i] == ";" || t[i] == "}" && t.get(i + 1) != Some(&";");
        if depth == 0 && end {
            rows.push(flat(&t[start..if t[i] == "}" { open } else { i + 1 }]));
            start = i + 1;
        }
    }
    rows
}
/// SB3E-R44.2: every `impl` of `source`, wherever it stands, to its `{`; an
/// `impl Trait` type (after `->`, `:`, `&`, `(`, `<`, `,` or `=`) is not one.
fn impls(source: &str) -> Vec<String> {
    let t = lex(source);
    let item = |i: &usize| *i == 0 || [";", "{", "}", "]", "unsafe"].contains(&t[i - 1]);
    let at = (0..t.len()).filter(|&i| t[i] == "impl").filter(item);
    let head = |i: usize| {
        t[i..]
            .iter()
            .position(|x| *x == "{")
            .map_or(t.len(), |n| i + n)
    };
    at.map(|i| flat(&t[i..head(i)])).collect()
}
/// Every Rust file of the crate, by path relative to `mother/`, read by the
/// adapter's scan walker (only the adapter touches the host).
fn sources() -> Vec<(String, String)> {
    walked().0
}
/// SB3E-R55.2: the sources by path text, as the inventory is listed, and the
/// walk's own findings (A-1).
fn walked() -> (Vec<(String, String)>, Vec<String>) {
    crate::adapter::scan::route_guard_sources(&crate::adapter::scan::crate_root())
}
fn text<'a>(sources: &'a [(String, String)], file: &str) -> &'a str {
    &sources.iter().find(|(f, _)| f == file).expect(file).1
}
/// Files outside the inventory: S5's module (gated whole by its one
/// declaration), this guard, and the lane-only harness.
fn exempt(file: &str) -> bool {
    let skip = [
        "src/route_seams.rs",
        "host/src/route_seams.rs",
        "src/oracles/route_seams_guard.rs",
    ];
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
/// A.2 item 3 (c) and F-S5-SPLIT section 4 item 4: each `SessionPort` or
/// `SessionDoor` use in `file`, when they are not exactly its list (`s5.rs`:
/// `PORT_USES`; a section: none, as its row lists no port use).
fn port_uses(file: &str, source: &str) -> Option<Vec<String>> {
    let uses = port_rows(source);
    let listed: Vec<&str> = if file == S5 {
        PORT_USES.lines().collect()
    } else {
        Vec::new()
    };
    (uses != listed).then_some(uses)
}
fn port_rows(source: &str) -> Vec<String> {
    let tokens = lex(source);
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
/// The tokens that can begin an unwind in a pause timeout's body.
const UNWINDS: [&str; 12] = [
    "unwrap",
    "expect",
    "panic",
    "assert",
    "unreachable",
    "todo",
    "?",
    "[",
    "assert_eq",
    "assert_ne",
    "unimplemented",
    "println",
];
/// U18 B8: `reader.rs`'s pause timeout, as `s5.rs`'s: no panic path, and
/// `std::process::abort()` last.
fn reader_timeout_findings(reader: &str) -> Vec<String> {
    let body = lex(item(reader, "timeout").unwrap_or(""));
    let mut found: Vec<String> = body
        .iter()
        .filter(|t| UNWINDS.contains(t) || t.starts_with("debug_assert"))
        .map(|t| format!("reader timeout: a panic path {t}"))
        .collect();
    let tail = ["std", ":", ":", "process", ":", ":", "abort", "(", ")", "}"];
    if !body.ends_with(&tail) {
        found.push("reader timeout: ends in abort".into());
    }
    found
}
/// A.2 item 3 (a)-(d), SB3E-R42.1, R42.4 and R42.5, and the harness
/// `timeout` (SB3E-R39.1, R40.6) over `s5.rs`, by identifier token.
fn s5_findings(s5: &str) -> Vec<String> {
    use sha2::Digest;
    // SB3E-R44.1: the file is the reviewed file, byte for byte; the rules
    // below are a second, advisory layer.
    let digest = hex::encode(sha2::Sha256::digest(s5.as_bytes()));
    let mut pre = Vec::new();
    if route_cells().all(|r| r != ("s5.rs", &*digest)) {
        pre.push(format!("G-S5: {S5}'s sha256 is {digest}"));
    }
    if let Err(e) = tokens(s5) {
        pre.push(lex_finding(S5, &e));
        return pre;
    }
    let tokens = lex(s5);
    let n = |pat: &[&str]| tokens.windows(pat.len()).filter(|w| *w == pat).count();
    let base = s5.as_ptr() as usize;
    let at = |t: &str| t.as_ptr() as usize - base;
    let span = |name: &str| item(s5, name).map_or(0..0, |b| at(b)..at(b) + b.len());
    let (drive, bound) = (span("drive_call"), span("bounded"));
    let outside = |i: usize| !drive.contains(&at(tokens[i]));
    let code = mask(s5);
    let mut found = pre;
    let mut check = |ok: bool, what: String| {
        if !ok {
            found.push(what);
        }
    };
    // (a) one `.call`, through the one `.model`, in the pinned `drive_call`;
    // the bare tokens `call` and `model` nowhere else (R42.1), but for the
    // contract's one `model: Some(ModelChoice` field.
    check(n(&[".", "call", "("]) == 1, "(a) one .call".into());
    check(n(&[".", "model"]) == 1, "(a) one .model".into());
    check(n(&["fetch_add"]) == 1, "(a) one count".into());
    let field = ["model", ":", "Some", "(", "ModelChoice"];
    check(n(&field) <= 1, "(a) one contract model field".into());
    for name in ["call", "model"] {
        let stray = |&i: &usize| tokens[i] == name && !tokens[i..].starts_with(&field);
        let stray = (0..tokens.len())
            .filter(|i| outside(*i))
            .filter(stray)
            .count();
        check(
            stray == 0,
            format!("(a) a bare `{name}` outside drive_call"),
        );
    }
    // (a) the counter's writes only in `drive_call` (there is no `Driver::new`).
    for i in (0..tokens.len()).filter(|&i| outside(i)) {
        if counter_write(&tokens, i) {
            check(
                false,
                format!("(a) counter write `{}` outside drive_call", tokens[i]),
            );
        }
    }
    check(
        n(&["AtomicU32", ":", ":"]) == 0,
        "(a) no second counter".into(),
    );
    // G-B1: no raw identifier, no macro definition, only the listed macros.
    for finding in gb1_findings(&code, &tokens) {
        check(false, finding);
    }
    // (b) no other route to the broker's model call; no re-exported handle.
    for id in ROUTE_IDS {
        check(n(&[id]) == 0, format!("(b) {id}"));
    }
    check(!reexports(&tokens), "(b) no re-export".into());
    // (c) the port is made once, inside `Coordinator::open`, and never kept.
    let open = n(&["Coordinator", ":", ":", "open", "("]) == 1;
    let open = s5.find("Coordinator::open(").filter(|_| open);
    let open = open.map_or(0..0, |o| o..closing(&code, o + "Coordinator::open".len()));
    let made =
        (0..tokens.len()).filter(|&i| tokens[i..].starts_with(&["SessionPort", ":", ":", "new"]));
    let made: Vec<usize> = made.map(|i| at(tokens[i])).collect();
    let inside = made.len() == 1 && made.iter().all(|p| open.contains(p));
    check(inside, "(c) the port made outside Coordinator::open".into());
    if let Some(ports) = port_uses(S5, s5) {
        check(false, format!("(c) port uses {ports:?}"));
    }
    // (d), G-S3: the parent's items only by the pinned import list; no glob,
    // no path through `crate`, `self`, `super` or a sibling module.
    let mut imported = Vec::new();
    for i in (0..tokens.len()).filter(|&i| tokens[i..].starts_with(&["super", ":", ":"])) {
        if i == 0 || tokens[i - 1] != "use" {
            check(false, "G-S3: a super:: path outside the import".into());
            continue;
        }
        let end = tokens[i..]
            .iter()
            .position(|t| *t == ";")
            .map_or(tokens.len(), |e| i + e);
        let names = tokens[i + 3..end]
            .iter()
            .filter(|t| !["{", "}", ","].contains(t));
        imported.extend(names.copied());
    }
    check(
        imported == S5_SUPER,
        format!("G-S3: the super imports {imported:?}"),
    );
    check(n(&[":", ":", "*"]) == 0, "G-S3: a glob import".into());
    for module in ["crate", "self", "grant", "locks", "store"] {
        check(n(&[module, ":", ":"]) == 0, format!("(d) {module}::"));
    }
    // The pinned regions.
    for (_, pin) in pins().into_iter().filter(|(f, _)| *f == S5) {
        check(pinned(s5, pin) == 1, format!("pin {S5}: {}", label(pin)));
    }
    // G-S4: every child wait is `bounded`'s (killed after 30 s), and each
    // timeout test owns and removes its temporary directory.
    // A child is started or awaited only there: `.spawn()`, `.output(`,
    // `.status(`, `.wait(`, `try_wait`, `wait_with_output`, `kill`, or a
    // `Command::` path other than `Command::new`.
    for (i, t) in tokens.iter().enumerate() {
        if child_wait(&tokens, i) && !bound.contains(&at(tokens[i])) {
            check(false, format!("G-S4: an unbounded child wait ({t})"));
        }
    }
    for (test, owned) in [
        (
            "route_s5_timeout_without_a_child_root_writes_nothing",
            "let cwd = Fixture(cwd);",
        ),
        (
            "route_s5_unreleased_pause_times_out_and_aborts",
            "let root = Fixture(root);",
        ),
    ] {
        let body = item(s5, test).unwrap_or("");
        let bounded = lex(body)
            .windows(2)
            .filter(|w| w == &["bounded", "("])
            .count();
        check(
            bounded == 1,
            format!("G-S4: {test}: an unbounded child wait"),
        );
        check(
            pinned(body, owned) == 1,
            format!("G-S4: {test}: its temp dir is not removed"),
        );
    }
    // The timeout aborts and cannot unwind: no panic path, `abort` last.
    let body = lex(item(s5, "timeout").unwrap_or(""));
    for t in &body {
        if UNWINDS.contains(t) || t.starts_with("debug_assert") {
            check(false, format!("timeout: a panic path {t}"));
        }
    }
    let tail = ["std", ":", ":", "process", ":", ":", "abort", "(", ")", "}"];
    check(body.ends_with(&tail), "timeout: ends in abort".into());
    // SB3E-R57.6: `cfg` is a listed macro only as `cfg!(mother_route_seams)`.
    found.extend(cfg_findings(S5, s5));
    found.extend(passive_findings(S5, s5));
    // F-S5-SPLIT section 4 item 12: a new check, listed separately.
    found.extend(alias_findings(S5, s5));
    // W0b B4 (item 1(f)): the closed `pub` surface.
    found.extend(exports_findings(s5));
    found
}
/// Rule (b): the other routes to the broker's model call.
const ROUTE_IDS: [&str; 5] = [
    "model_call",
    "call_model",
    "model_call_private",
    "EffectPort",
    "ModelCaller",
];
/// Rule (b): a `pub use` or `pub(..) use` re-export.
fn reexports(tokens: &[&str]) -> bool {
    let reexport = tokens.windows(2).any(|w| w[0] == "pub" && w[1] == "use");
    let scoped = tokens
        .windows(5)
        .any(|w| w[0] == "pub" && w[1] == "(" && w[4] == "use");
    reexport || scoped
}
/// Rule (a): token `i` writes the counter (a `WRITES` or `fetch_*` name, or
/// `calls` assigned).
fn counter_write(tokens: &[&str], i: usize) -> bool {
    let next = |k: usize| tokens.get(i + k).copied().unwrap_or("");
    let op = next(1).len() == 1 && "+-*/%|&^".contains(next(1)) && next(2) == "=";
    let shift = ["<", ">"].contains(&next(1)) && next(2) == next(1) && next(3) == "=";
    let set = next(1) == "=" && !["=", ">"].contains(&next(2));
    let assigned = tokens[i] == "calls" && (set || op || shift);
    WRITES.contains(&tokens[i]) || tokens[i].starts_with("fetch_") || assigned
}
/// G-S4: token `i` starts or awaits a child: `.spawn()`, `.output(`,
/// `.status(`, `.wait(`, `try_wait`, `wait_with_output`, `kill`, or a
/// `Command::` path other than `Command::new`.
fn child_wait(tokens: &[&str], i: usize) -> bool {
    let (t, next) = (tokens[i], |k: usize| {
        tokens.get(i + k).copied().unwrap_or("")
    });
    let method = i > 0 && tokens[i - 1] == "." && next(1) == "(";
    let called = ["output", "status", "wait"].contains(&t) || t == "spawn" && next(2) == ")";
    let path = t == "Command" && next(1) == ":" && next(3) != "new";
    method && called || path || ["try_wait", "wait_with_output", "kill"].contains(&t)
}
/// G-B1 over `code` (`mask`ed) and its `tokens`: raw identifiers, macro
/// definitions and macros not in `S5_MACROS`, in that order.
fn gb1_findings(code: &str, tokens: &[&str]) -> Vec<String> {
    let mut found = Vec::new();
    let b = code.as_bytes();
    let word = |p: usize| {
        b.get(p)
            .is_some_and(|c| c.is_ascii_alphanumeric() || *c == b'_')
    };
    for (p, _) in code.match_indices("r#") {
        if word(p + 2) && !p.checked_sub(1).is_some_and(word) {
            let name = code[p + 2..].split(|c: char| !c.is_alphanumeric() && c != '_');
            let name = name.into_iter().next().unwrap_or("");
            found.push(format!("G-B1: a raw identifier r#{name}"));
        }
    }
    for (i, t) in tokens.iter().enumerate() {
        if ["macro_rules", "macro"].contains(t) {
            found.push(format!("G-B1: a macro definition {t}"));
        }
        let next = |k: usize| tokens.get(i + k).copied().unwrap_or("");
        let invoked = next(1) == "!" && ["(", "[", "{"].contains(&next(2));
        let ident = t.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_');
        if ident && invoked && !S5_MACROS.contains(t) {
            found.push(format!("G-B1: an unlisted macro {t}!"));
        }
    }
    found
}
/// SB3E-R88.3 (mission Behaviour 7, R49.7): a cell's collection is passive.
/// `collect` reads the journal only through `journal_view` (`journal()`
/// calls `touch()`, which can run `file_stop`), calls no `state()` or
/// `touch()`, and takes native deltas only from the projection, never from
/// S1's `store.commit` entries.
/// F-S5-SPLIT section 4 item 5: every `fn collect*` of every listed file is
/// passive; `s5.rs` has its `collect`.
fn passive_findings(file: &str, source: &str) -> Vec<String> {
    let t = lex(source);
    let mut names: Vec<&str> = (1..t.len())
        .filter(|&i| t[i - 1] == "fn" && t[i].starts_with("collect"))
        .map(|i| t[i])
        .collect();
    names.sort_unstable();
    names.dedup();
    let none = || format!("Passive: no fn collect in {S5}");
    if file == S5 && !names.contains(&"collect") {
        return vec![none()];
    }
    let mut found = Vec::new();
    for name in names {
        let Some(body) = item(source, name) else {
            found.push(match file == S5 && name == "collect" {
                true => none(),
                false => format!("Passive: fn {name} is not one item in {file}"),
            });
            continue;
        };
        // SB3E-R99.2 F3b: a "body" that is not whole (a brace in the signature)
        // is a finding, never a lexer panic or a passed check.
        if tokens(body).is_err() {
            found.push(format!("Passive: fn {name} has no whole body in {file}"));
            continue;
        }
        let t = lex(body);
        for (call, read) in [
            (
                "journal",
                "the journal through journal(), not journal_view()",
            ),
            ("state", "state()"),
            ("touch", "touch()"),
        ] {
            if t.windows(3).any(|w| w == [".", call, "("]) {
                found.push(format!("Passive: {name} reads {read}"));
            }
        }
        // SB3E-R101.2 F3a: each literal's value (escapes decoded, a raw one as
        // written); a literal that does not decode is itself a finding.
        let literals = tokens(body).unwrap_or_default().into_iter().filter(|x| x.1);
        let mut commit = t.iter().any(|x| x.contains("store.commit"));
        for (r, _) in literals {
            match literal_value(&body[r.clone()]) {
                Some(v) => commit |= v.contains("store.commit"),
                None => found.push(format!(
                    "Passive: {name} has a literal that does not decode: {}",
                    &body[r]
                )),
            }
        }
        if commit {
            found.push(format!("Passive: {name} reads store.commit"));
        }
    }
    found
}
/// SB3E-R101.2 F3a: a literal token's value: a string, byte string or char
/// with its escapes decoded (`\x`, `\u{..}`, `\n \r \t \\ \0 \' \"`, and a
/// backslash-newline with the whitespace after it); a raw one as written; a
/// number as written. `None` when it does not decode.
fn literal_value(token: &str) -> Option<String> {
    if token.starts_with(|c: char| c.is_ascii_digit()) {
        return Some(token.into());
    }
    let s = token.strip_prefix(['b', 'c']).unwrap_or(token);
    if let Some(raw) = s.strip_prefix('r') {
        let hashes = raw.len() - raw.trim_start_matches('#').len();
        let body = raw[hashes..].strip_prefix('"')?;
        return Some(body[..body.rfind(&format!("\"{}", "#".repeat(hashes)))?].into());
    }
    let quote = s.chars().next().filter(|q| ['"', '\''].contains(q))?;
    let mut chars = s[1..s.rfind(quote).filter(|&e| e > 0)?].chars().peekable();
    let mut out = String::new();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        let hex = |h: &str| u32::from_str_radix(&h.replace('_', ""), 16).ok();
        match chars.next()? {
            'n' => out.push('\n'),
            'r' => out.push('\r'),
            't' => out.push('\t'),
            '0' => out.push('\0'),
            e @ ('\\' | '\'' | '"') => out.push(e),
            'x' => out.push(char::from_u32(hex(&[chars.next()?, chars.next()?]
                .iter()
                .collect::<String>())?)?),
            'u' if chars.next()? == '{' => {
                let (mut digits, mut closed) = (String::new(), false);
                for d in chars.by_ref() {
                    closed = d == '}';
                    if closed {
                        break;
                    }
                    digits.push(d);
                }
                // An unterminated `\u{` does not decode.
                out.push(char::from_u32(hex(&digits).filter(|_| closed)?)?);
            }
            '\n' => while chars.next_if(|c| c.is_whitespace()).is_some() {},
            _ => return None,
        }
    }
    Some(out)
}
/// F-S5-SPLIT item 1(a), (d) and section 4 item 8 over the harness, every
/// finding: the list itself, the closed list, distinct digests, each
/// section's owner header and closed `pub(super)` surface, no section named
/// outside `main.rs`, S5 arming only inside the boundary; then each section's
/// profile (`section_findings`). `files` as `route_cells_files`, `sources` as
/// the walk.
fn sections_findings(sources: &[(String, String)], files: &[(String, Vec<u8>)]) -> Vec<String> {
    sections_with(ROUTE_CELLS, S5_SECTIONS, sources, files)
}
/// `sections_findings` over the given `ROUTE_CELLS` and `S5_SECTIONS` data.
fn sections_with(
    listed: &str,
    data: &str,
    sources: &[(String, String)],
    files: &[(String, Vec<u8>)],
) -> Vec<String> {
    let rows = section_rows(data);
    let mut found = rows_check(data, &rows);
    found.extend(closed_findings(listed, &rows, files));
    found.extend(digest_findings(&rows, files));
    let harness: Vec<(&str, &str)> = sources
        .iter()
        .filter_map(|(f, s)| Some((f.strip_prefix(CELLS)?, s.as_str())))
        .filter(|(f, _)| f.ends_with(".rs"))
        .collect();
    let lexed = harness.iter().filter_map(|(f, s)| {
        let e = tokens(s).err()?;
        Some(lex_finding(&format!("{CELLS}{f}"), &e))
    });
    let lexed: Vec<String> = lexed.collect();
    if !lexed.is_empty() {
        found.extend(lexed);
        return found;
    }
    let main = harness.iter().find(|(f, _)| *f == "main.rs");
    let main = main.map_or(Vec::new(), |(_, s)| macro_drivers(s));
    for row in rows.iter().filter(|r| r.file != "s5.rs") {
        let Some(&(_, source)) = harness.iter().find(|(f, _)| *f == row.file) else {
            continue;
        };
        found.extend(owner_findings(row, source, &main));
        found.extend(section_findings(row.file, row, source, &pins()));
    }
    found.extend(reach_findings(&rows, &harness));
    found
}
/// The list itself: six fields a row, `s5.rs` (U5, `S5_SUPER`) first, each
/// file, pair and control in one row, and no section row importing a composed
/// type or an item of `nexus_mother::adapter`, `nexus_mother::coordinator` or
/// `nexus_governed_control::evidence_store` (item 1(c)(iii)).
fn rows_check(data: &str, rows: &[Row]) -> Vec<String> {
    let malformed = data.lines().filter(|l| l.split('|').count() != 6);
    let mut found: Vec<String> = malformed
        .map(|l| format!("S5 sections: a malformed row {l:?}"))
        .collect();
    let first = rows.first().map(|r| (r.file, r.unit, r.imports.as_slice()));
    if first != Some(("s5.rs", "U5", &["S5_SUPER"][..])) {
        found.push("S5 sections: the first row is not s5.rs|U5 with S5_SUPER".into());
    }
    let banned = [
        "nexus_mother::adapter",
        "nexus_mother::coordinator",
        "nexus_governed_control::evidence_store",
    ];
    let mut seen: Vec<&str> = Vec::new();
    for (i, row) in rows.iter().enumerate() {
        let named = [row.file].into_iter().chain(row.pairs.iter().copied());
        for x in named.chain(row.controls.iter().copied()) {
            if seen.contains(&x) {
                found.push(format!("S5 sections: {x} is in two rows"));
            }
            seen.push(x);
        }
        for x in row.imports.iter().filter(|_| i > 0) {
            let typed = x.split("::").any(|s| T7_TYPES.contains(&s));
            if typed || banned.iter().any(|b| x.starts_with(b)) {
                found.push(format!("S5 sections: the row {} imports {x}", row.file));
            }
            // W0b B4 (SB3E-R106.3), an allow-list: through `s5`, exactly
            // `super::s5::<item>` of an `s` row of `S5_EXPORTS` (not the module,
            // `self`, a path below an item, nor an `h` row).
            let segs: Vec<&str> = x.split("::").collect();
            let through = segs.get(1).and_then(|s| s.split(' ').next()) == Some("s5");
            let item = (segs.len() == 3 && segs[0] == "super").then(|| segs[2]);
            if through && !item.is_some_and(|i| section_exports().contains(&i)) {
                found.push(format!(
                    "S5 sections: the row {} imports {x}, not a section export of s5.rs",
                    row.file
                ));
            }
        }
    }
    found
}
/// Item 1(a): every file of the directory and every `ROUTE_CELLS` name is a
/// listed section or a non-section file (an `s5*.rs` one is a section), and
/// every listed file has its `ROUTE_CELLS` row.
fn closed_findings(listed: &str, rows: &[Row], files: &[(String, Vec<u8>)]) -> Vec<String> {
    let sections: Vec<&str> = rows.iter().map(|r| r.file).collect();
    let pinned = listed.lines().filter_map(|l| l.split_once(' '));
    let pinned: Vec<&str> = pinned.map(|r| r.0).collect();
    let named = files.iter().filter_map(|(f, _)| f.strip_prefix(CELLS));
    let mut names: Vec<&str> = named.chain(pinned.iter().copied()).collect();
    names.sort_unstable();
    names.dedup();
    let mut found = Vec::new();
    let unknown = |n: &&str| !sections.contains(n) && !NON_SECTIONS.contains(n);
    for name in names.into_iter().filter(unknown) {
        found.push(match name.starts_with("s5") && name.ends_with(".rs") {
            true => format!("S5 sections: {name} is not a listed section"),
            false => format!("S5 sections: {name} is neither a section nor a non-section file"),
        });
    }
    let unpinned = sections
        .iter()
        .chain(&NON_SECTIONS)
        .filter(|n| !pinned.contains(n));
    for name in unpinned {
        found.push(format!("S5 sections: {name} has no ROUTE_CELLS row"));
    }
    found
}
/// Item 1(a): no two listed files have one digest.
fn digest_findings(rows: &[Row], files: &[(String, Vec<u8>)]) -> Vec<String> {
    use sha2::Digest;
    let digest = |file: &str| {
        let (_, bytes) = files
            .iter()
            .find(|(f, _)| f.strip_prefix(CELLS) == Some(file))?;
        Some(hex::encode(sha2::Sha256::digest(bytes)))
    };
    let digests: Vec<(&str, String)> = rows
        .iter()
        .filter_map(|r| Some((r.file, digest(r.file)?)))
        .collect();
    let mut found = Vec::new();
    for (i, (a, x)) in digests.iter().enumerate() {
        for (b, _) in digests[i + 1..].iter().filter(|(_, y)| y == x) {
            found.push(format!("S5 sections: {a} and {b} share one digest"));
        }
    }
    found
}
/// Item 1(d): a section's first line names its row's unit and file, and each
/// of its `pub` items is a `fn` an arm or control names in it, or a helper of
/// its row (a field is no item).
fn owner_findings(row: &Row, source: &str, drivers: &[(String, String)]) -> Vec<String> {
    let (file, mut found) = (row.file, Vec::new());
    let header = source.lines().next().unwrap_or("");
    let named = header.strip_prefix("//! SB-3E S5 section ");
    match named.and_then(|h| h.split_once(": ")) {
        Some((unit, named)) if (unit, named) == (row.unit, file) => {}
        Some((unit, named)) if named == file => found.push(format!(
            "S5 sections: {file} names {unit}, not {}",
            row.unit
        )),
        _ => found.push(format!(
            "S5 sections: {file}'s first line is not //! SB-3E S5 section {}: {file}",
            row.unit
        )),
    }
    let t = lex(source);
    for i in (0..t.len()).filter(|&i| t[i] == "pub") {
        let scoped = t.get(i + 1) == Some(&"(");
        let j = if scoped {
            close_of(&t, i + 1) + 1
        } else {
            i + 1
        };
        let (kind, name) = (
            t.get(j).copied().unwrap_or(""),
            t.get(j + 1).copied().unwrap_or(""),
        );
        let bound = kind == "fn" && drivers.iter().any(|(s, f)| s == stem(file) && f == name);
        if name == ":" || bound || row.helpers.contains(&name) {
            continue;
        }
        found.push(match kind {
            "fn" => format!("S5 sections: {file} exposes an unbound pub(super) fn {name}"),
            _ => format!("S5 sections: {file} exposes an unlisted pub(super) {kind} {name}"),
        });
    }
    found
}
/// Every driver `main.rs`'s `macro_rules! cells` names, as (stem, fn).
fn macro_drivers(main: &str) -> Vec<(String, String)> {
    let t = lex(main);
    let head = ["macro_rules", "!", "cells", "{"];
    let Some(start) = (0..t.len()).find(|&i| t[i..].starts_with(&head)) else {
        return Vec::new();
    };
    let named = drivers(&t[start..close_of(&t, start + 3).min(t.len())]);
    named
        .into_iter()
        .map(|(s, f)| (s.to_string(), f.to_string()))
        .collect()
}
/// Item 1(d): outside `main.rs` no harness file names a section other than
/// `s5`; outside the boundary `nexus_mother::route_seams` is only the type
/// path `nexus_mother::route_seams::Entry`, so S5 is armed only inside it.
fn reach_findings(rows: &[Row], harness: &[(&str, &str)]) -> Vec<String> {
    let stems: Vec<&str> = rows
        .iter()
        .map(|r| stem(r.file))
        .filter(|s| *s != "s5")
        .collect();
    let mut found = Vec::new();
    for (file, source) in harness {
        let t = lex(source);
        let mut named: Vec<&str> = t.iter().copied().filter(|x| stems.contains(x)).collect();
        named.sort_unstable();
        named.dedup();
        for section in named.into_iter().filter(|_| *file != "main.rs") {
            found.push(format!("S5 sections: {file} names section {section}"));
        }
        if rows.iter().any(|r| r.file == *file) {
            continue;
        }
        for i in (0..t.len()).filter(|&i| t[i] == "route_seams") {
            let after = |w: &[&str]| i >= 3 && t[i - 3..i] == *w;
            let entry = t.get(i + 1..i + 4) == Some(&[":", ":", "Entry"][..]);
            let entry = entry && t.get(i + 4) != Some(&":");
            // U18 (F-S0-1 (b)) and U21a: the two host drivers reach S7.
            let driver = ["reader.rs", "report.rs"].contains(file);
            let host = driver && after(&["nexus_mother_host", ":", ":"]);
            // U21a: S7's entry type, as S5's, by its type path only.
            let typed =
                after(&["nexus_mother", ":", ":"]) || after(&["nexus_mother_host", ":", ":"]);
            if !(host || after(&["broker", ":", ":"]) || typed && entry) {
                found.push(format!(
                    "S5 sections: {file} reaches nexus_mother::route_seams other than as ::Entry"
                ));
            }
        }
    }
    found
}
/// F-S5-SPLIT item 1(c): the profile of section `file` (its `row`), stricter
/// than `s5_findings` and through its rules: no model call, counter or other
/// route, no re-export, no port or door use (its row lists none), no
/// composed constructor, child, `Command::`, `fn timeout` or `abort`; G-B1;
/// `cfg!` exact; one parent import, no glob, no path through `crate`, `self`,
/// `grant`, `locks`, `store` or another section; every `fn collect*`
/// passive; each of its regions pinned once; each `impl` in `IMPLS`; no
/// composed type by any name, and exactly its row's imports (1(c)(iii)); then
/// the pause rule (`pause_findings`) and the alias check (`alias_findings`).
fn section_findings(file: &str, row: &Row, source: &str, pins: &[(&str, &str)]) -> Vec<String> {
    let full = format!("{CELLS}{file}");
    if let Err(e) = tokens(source) {
        return vec![lex_finding(&full, &e)];
    }
    let (t, code) = (lex(source), mask(source));
    let n = |pat: &[&str]| t.windows(pat.len()).filter(|w| *w == pat).count();
    let uses = use_ranges(&t);
    let in_use = |i: usize| uses.iter().any(|r| r.contains(&i));
    let mut found = Vec::new();
    let mut say = |what: String| found.push(format!("section {file}: {what}"));
    for (i, x) in t.iter().enumerate() {
        if ["call", "model"].contains(x) {
            say(format!("a model call ({x})"));
        }
        if counter_write(&t, i) {
            say(format!("a counter write `{x}`"));
        }
        if child_wait(&t, i) {
            say(format!("a child wait ({x})"));
        }
    }
    if n(&["AtomicU32", ":", ":"]) > 0 {
        say("a second counter (AtomicU32::)".into());
    }
    for id in ROUTE_IDS.iter().filter(|id| n(&[id]) > 0) {
        say(format!("(b) {id}"));
    }
    if reexports(&t) {
        say("a re-export".into());
    }
    if let Some(ports) = port_uses(&full, source) {
        say(format!("port uses {ports:?}"));
    }
    for built in composed(&t) {
        say(format!("constructs {built}"));
    }
    for (pat, what) in [
        (&["Command", ":", ":"][..], "a Command:: path"),
        (&["fn", "timeout"][..], "fn timeout"),
        (&["abort"][..], "abort"),
        (&[":", ":", "*"][..], "a glob import"),
    ] {
        if n(pat) > 0 {
            say(what.into());
        }
    }
    // SB3E-R99.2 F2: `Command` in any form but a `Command::` path is a child path.
    let commands = (0..t.len()).filter(|&i| t[i] == "Command");
    if commands
        .into_iter()
        .any(|i| t.get(i + 1..i + 3) != Some(&[":", ":"][..]))
    {
        say("a child path (Command)".into());
    }
    // SB3E-R99.2 F3b: no fn has a brace in its signature, so `item` finds its body.
    for name in braced_signatures(&t) {
        say(format!("a brace in the signature of fn {name}"));
    }
    let shared = gb1_findings(&code, &t)
        .into_iter()
        .chain(cfg_findings(&full, source));
    for finding in shared.chain(passive_findings(&full, source)) {
        say(finding);
    }
    let supers = (0..t.len()).filter(|&i| t[i..].starts_with(&["super", ":", ":"]));
    if supers.into_iter().any(|i| !in_use(i)) {
        say("a super:: path outside its import".into());
    }
    if uses
        .iter()
        .filter(|r| t[(*r).clone()].contains(&"super"))
        .count()
        > 1
    {
        say("more than one super:: import".into());
    }
    let rows = section_rows(S5_SECTIONS);
    let others = rows.iter().map(|r| stem(r.file)).filter(|s| *s != "s5");
    for module in ["crate", "self", "grant", "locks", "store"]
        .into_iter()
        .chain(others)
    {
        if n(&[module, ":", ":"]) > 0 {
            say(format!("a path through {module}"));
        }
    }
    for (_, pin) in pins.iter().filter(|(f, _)| *f == full) {
        let got = pinned(source, pin);
        if got != 1 {
            say(format!("pin {} matched {got}", label(pin)));
        }
    }
    for r in impls(source) {
        if !IMPLS.lines().any(|l| l == format!("{full}|{r}")) {
            say(format!("an unlisted impl {r}"));
        }
    }
    // Item 1(c)(iii): no composed type by any name; every `use` on its row.
    let mut named: Vec<&str> = t.iter().copied().filter(|x| T7_TYPES.contains(x)).collect();
    named.sort_unstable();
    named.dedup();
    for x in named {
        say(format!("a T7 type name {x}"));
    }
    if uses.iter().any(|r| t[r.clone()].contains(&"as")) {
        say("use .. as in a section".into());
    }
    if n(&["extern"]) > 0 {
        say("an extern item in a section".into());
    }
    for i in (0..t.len()).filter(|&i| t[i] == "mod" && t.get(i + 2) == Some(&"{")) {
        say(format!("an inline module in a section: {}", t[i + 1]));
    }
    if qualified(&t) {
        say("a qualified-self path <..>::".into());
    }
    for p in openers(&t) {
        say(format!("a qualified-self opener < after `{p}`"));
    }
    // Built from parts so the kernel-dependency scan does not match this file.
    let roots = [
        "nexus_mother",
        "nexus_governed_control",
        concat!("nexus_", "kernel"),
        "crate",
    ];
    for i in (0..t.len()).filter(|&i| roots.contains(&t[i]) && !in_use(i)) {
        if t.get(i + 1..i + 3) == Some(&[":", ":"][..]) {
            say(format!("a crate-root path outside use: {}::", t[i]));
        }
    }
    let mut leaves = Vec::new();
    for r in &uses {
        use_leaves(&t[r.clone()], "", &mut leaves);
    }
    for x in leaves.iter().filter(|x| !row.imports.contains(&x.as_str())) {
        say(format!("use not on its row: {x}"));
    }
    for x in row
        .imports
        .iter()
        .filter(|x| !leaves.iter().any(|l| l == *x))
    {
        say(format!("row import {x} has no use"));
    }
    found.extend(pause_findings(file, source));
    found.extend(alias_findings(&full, source));
    found
}
/// The token range of each `use` statement's tree (after `use`, to its `;`).
fn use_ranges(t: &[&str]) -> Vec<std::ops::Range<usize>> {
    let at = (0..t.len()).filter(|&i| t[i] == "use");
    let end = |i: usize| {
        t[i..]
            .iter()
            .position(|x| *x == ";")
            .map_or(t.len(), |e| i + e)
    };
    at.map(|i| i + 1..end(i)).collect()
}
/// A `use` tree's leaves as path text (`a::{b, c::d}` gives `a::b`, `a::c::d`).
fn use_leaves(t: &[&str], prefix: &str, out: &mut Vec<String>) {
    let (mut depth, mut start, mut items) = (0i32, 0, Vec::new());
    for (i, x) in t.iter().enumerate() {
        depth += i32::from(*x == "{") - i32::from(*x == "}");
        if *x == "," && depth == 0 {
            items.push(&t[start..i]);
            start = i + 1;
        }
    }
    items.push(&t[start..]);
    for item in items.into_iter().filter(|i| !i.is_empty()) {
        match item.iter().position(|x| *x == "{") {
            Some(o) => {
                let inner = &item[o + 1..close_of(item, o).min(item.len())];
                use_leaves(inner, &format!("{prefix}{}", item[..o].concat()), out);
            }
            None => {
                let text = item.iter().map(|x| if *x == "as" { " as " } else { x });
                out.push(format!("{prefix}{}", text.collect::<String>()));
            }
        }
    }
}
/// A qualified-self path `<..>::` in `t`, with or without `as` (SB3E-R99.2
/// F2, R101.2): each `>` before `::` is matched to its `<` by `<`/`>` depth
/// alone, with no stop at `;`, `{` or `}`; a match that does not follow `::`
/// (a turbofish), or no match, is one. `->` and `=>` close nothing; `>=` and
/// `<=` are comparisons (`lex` gives one token per punctuation character).
fn qualified(t: &[&str]) -> bool {
    let after = |k: usize, p: &[&str]| k > 0 && p.contains(&t[k - 1]);
    let closes = |k: usize| t[k] == ">" && !after(k, &["-", "="]) && t.get(k + 1) != Some(&"=");
    let opens = |k: usize| t[k] == "<" && t.get(k + 1) != Some(&"=");
    let ends = (0..t.len()).filter(|&j| closes(j) && t.get(j + 1..j + 3) == Some(&[":", ":"][..]));
    ends.into_iter().any(|j| {
        let mut depth = 0i32;
        for k in (0..=j).rev() {
            depth += i32::from(closes(k)) - i32::from(opens(k));
            if opens(k) && depth == 0 {
                return k < 2 || t[k - 2..k] != [":", ":"];
            }
        }
        true
    })
}
/// SB3E-R101.2 F2, fail-closed beside `qualified`: each `<` whose previous
/// token lets it only open a qualified-self path, as that token. Allowed
/// before `<`: an identifier that is not a keyword (`impl` is; `for` only as
/// `for<'a>`), a literal, `)`, a `]` that closes no attribute, `?` or `::`
/// (a turbofish). Refused: a lifetime, a `<`, any other punctuation,
/// nothing. A comparison inside `<..>` cannot move this rule's match.
fn openers<'a>(t: &[&'a str]) -> Vec<&'a str> {
    let keyword = |w: &str| {
        let words = "as async await box break const continue crate dyn else enum extern false fn if in let loop match mod move mut pub ref return self Self static struct super trait true try type unsafe use where while yield";
        words.split(' ').any(|k| k == w)
    };
    let allowed = |i: usize| {
        let p = t[i - 1];
        let word = p.starts_with(|c: char| c.is_alphanumeric() || c == '_');
        let lifetime = i >= 2 && t[i - 2] == "'" && word;
        let literal = p.starts_with(['"', '\'']) && p.len() > 1;
        let higher = p != "for" || t.get(i + 1) == Some(&"'");
        // A `]` that closes `#[..]` or `#![..]` is no operand.
        let mut depth = 0i32;
        let open = (0..i).rev().find(|&k| {
            depth += i32::from(t[k] == "]") - i32::from(t[k] == "[");
            depth == 0
        });
        let attribute = p == "]" && open.is_some_and(|k| k > 0 && ["#", "!"].contains(&t[k - 1]));
        (i >= 2 && t[i - 2..i] == [":", ":"])
            || [")", "]", "?"].contains(&p) && !attribute
            || literal
            || word && !lifetime && !keyword(p) && higher
    };
    let refused = (0..t.len()).filter(|&i| t[i] == "<" && (i == 0 || !allowed(i)));
    refused
        .map(|i| if i == 0 { "" } else { t[i - 1] })
        .collect()
}
/// SB3E-R99.2 F3b: each `fn` whose signature (from its name to its body or
/// `;`) holds a `{` inside `()`, `[]` or `<>`, by name.
fn braced_signatures<'a>(t: &[&'a str]) -> Vec<&'a str> {
    let mut found = Vec::new();
    let named = |k: &usize| {
        t[*k - 1] == "fn"
            && t[*k]
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'_')
    };
    for k in (1..t.len()).filter(named) {
        let mut depth = 0i32;
        for j in k + 1..t.len() {
            let arrow = j > 0 && ["-", "="].contains(&t[j - 1]);
            match t[j] {
                "(" | "[" | "<" => depth += 1,
                ")" | "]" => depth -= 1,
                ">" if !arrow => depth -= 1,
                "{" if depth > 0 => {
                    found.push(t[k]);
                    break;
                }
                "{" | ";" if depth <= 0 => break,
                _ => {}
            }
        }
    }
    found
}
/// F-S5-SPLIT item 1(c)(ii) (SB3E-R95.7): a section arms an S5 pause only
/// through `s5.rs`'s pause helper. No `use` brings `Action` or `arm` from a
/// `route_seams` module (by name, path or glob, or the module itself), and no
/// `Action`, `Pause`, `Timeout` or `timeout` token is in it at all.
fn pause_findings(file: &str, source: &str) -> Vec<String> {
    if tokens(source).is_err() {
        return Vec::new(); // `section_findings` reports it.
    }
    let t = lex(source);
    let mut found = Vec::new();
    for r in use_ranges(&t) {
        let u = &t[r];
        let module = u.last() == Some(&"route_seams")
            || u.windows(2)
                .any(|w| w[0] == "route_seams" && [",", "}"].contains(&w[1]));
        let brings = u.iter().any(|x| ["Action", "arm", "*", "self"].contains(x));
        if u.contains(&"route_seams") && (module || brings) {
            found.push(format!(
                "section {file}: Action or arm imported from route_seams"
            ));
        }
    }
    // U9b D7 (SB3E-R127.3 F-U9A-PAUSE, R130.1, R133 N4): a section never calls
    // `Fixture::pause`, in any form; the token is refused (a closed identifier rule).
    if t.contains(&"pause") {
        found.push(format!("section {file}: a pause call (F-U9A-PAUSE)"));
    }
    let callback = ["Action", "Pause", "Timeout", "timeout"];
    let mut named: Vec<&str> = t.iter().copied().filter(|x| callback.contains(x)).collect();
    named.sort_unstable();
    named.dedup();
    for x in named {
        found.push(format!(
            "section {file}: a pause callback that is not s5::timeout ({x})"
        ));
    }
    found
}
/// F-S5-SPLIT section 4 item 12 (W0a), on `s5.rs` and every section: no
/// `type` item names a composed type (`T7_TYPES`), no `use .. as` renames one,
/// and no module is inline, so no other name constructs a composed type.
fn alias_findings(file: &str, source: &str) -> Vec<String> {
    if tokens(source).is_err() {
        return Vec::new(); // Its caller reports it.
    }
    let t = lex(source);
    let mut found = Vec::new();
    for i in (0..t.len()).filter(|&i| t[i] == "type") {
        let end = t[i..]
            .iter()
            .position(|x| *x == ";")
            .map_or(t.len(), |e| i + e);
        let right = t[i..end].iter().position(|x| *x == "=");
        let right = right.map_or(&[][..], |e| &t[i + e..end]);
        if right.iter().any(|x| T7_TYPES.contains(x)) {
            let name = t.get(i + 1).copied().unwrap_or("");
            found.push(format!("{file}: type alias names a T7 type: {name}"));
        }
    }
    for r in use_ranges(&t) {
        let renamed = r.filter(|&k| t[k] == "as" && k > 0 && T7_TYPES.contains(&t[k - 1]));
        for k in renamed {
            let to = t.get(k + 1).copied().unwrap_or("");
            found.push(format!(
                "{file}: use .. as renames a T7 type: {} as {to}",
                t[k - 1]
            ));
        }
    }
    for i in (0..t.len()).filter(|&i| t[i] == "mod" && t.get(i + 2) == Some(&"{")) {
        found.push(format!("{file}: an inline module {}", t[i + 1]));
    }
    found
}
/// W0b B4 (F-S5-SPLIT item 1(f), SB3E-R95.4): `s5.rs`'s `pub` surface, exactly
/// this list in source order, one row per item (`<class>|<kind> <name>`, a
/// struct's exported fields after it in braces); an unlisted or a missing one
/// is a finding. Class `s`: a section's row may import it, and none hands out
/// a composed type but `S5_BY_VALUE`; `h`: only the harness's other files use
/// it (`grant.rs`, `main.rs`, `witness.rs`; SB3E-R106.3), and no section row
/// may import it. It holds the pause helper `hold`, not `timeout`; `Mother.m`
/// is the named by-value export of the Mother `FileSession` for the close
/// chain (R95.2, R106.4).
const S5_EXPORTS: &str = "\
h|type Recorded
h|fn lanes
h|struct Boot{door,context,name,request,peer}
h|fn boot
h|fn consented
s|struct Mother{m}
s|struct Profile{opening,recheck,classify,call_gate,bound}
s|type Neighbour
s|struct Expect{attempts,profile,roles,request_level,attempt_level,multiplicity,position}
s|const RECORDED
s|type Roles
s|fn sessions
s|const HELD
s|const REFUSED
s|struct Facts{session,request,recorded,claim,points,acks,value,row,applied,broker,mother,host,journal,native,peer,findings,read}
h|struct Read{value,trace,echo,since}
s|enum Ack
s|enum Hold
h|const BROKER_SINKS
h|const CLOSE_SINKS
h|const HELD_SINKS
h|struct Ran{id,nonce,result,passed,output}
h|fn cell
s|fn mission
s|fn hold
s|enum Anchor
s|struct Chain{row,anchor,points}
s|struct Start{o,mother}
s|fn start
s|fn driven
h|fn released
h|fn ctl_call_chat
h|fn original_cell_unscheduled
s|fn unscheduled
h|fn repeated
s|fn repeat
h|fn mission_stop
h|fn stop_all
h|fn fatal
h|fn quit
h|fn chat_emergency
h|fn chat_failed
h|fn chat_closingnormal
h|fn chat_closingrestricted
h|fn chat_cr
h|fn chat_lf
h|fn chat_crlf
h|fn chat_eof
h|fn chat_partial_ctrl_d
h|fn chat_master_close
s|fn shutdown
s|fn stop
h|fn ord3_quit
h|fn quit_other_attempt
h|fn early_quit
s|fn collect
s|struct Broker{o,endpoint}
s|fn open_broker
s|fn collect_broker
s|enum Setup
s|enum Mode
s|struct Close{o,mother}
s|fn accept_quit
s|fn closed
s|fn registered
s|fn read_registered
h|fn pressed_registered
s|fn collect_report
";
/// The one export that hands out a composed type (`T7_TYPES`): by value.
const S5_BY_VALUE: &str = "Mother.m";
/// `s5.rs`'s `pub` items as `S5_EXPORTS` rows (without the class), and each
/// export that hands out a composed type (a `fn` returning, a `const` typed or
/// a field typed by a `T7_TYPES` name) as its item's row label and finding.
fn exports_of(s5: &str) -> (Vec<String>, Vec<(String, String)>) {
    let t = lex(s5);
    let structs: Vec<(usize, usize, &str)> = (0..t.len())
        .filter(|&i| t[i] == "struct" && t.get(i + 2) == Some(&"{"))
        .map(|i| (i + 2, close_of(&t, i + 2), t[i + 1]))
        .collect();
    let (mut rows, mut handed) = (Vec::<(String, Vec<String>)>::new(), Vec::new());
    for i in (0..t.len()).filter(|&i| t[i] == "pub") {
        let j = match t.get(i + 1) == Some(&"(") {
            true => close_of(&t, i + 1) + 1,
            false => i + 1,
        };
        let (kind, name) = (
            t.get(j).copied().unwrap_or(""),
            t.get(j + 1).copied().unwrap_or(""),
        );
        let owner = structs.iter().find(|s| s.0 < j && j < s.1).map(|s| s.2);
        let item = owner.map_or(format!("{kind} {name}"), |o| format!("struct {o}"));
        let stop = |k: &&str| ["{", ";", "=", "where"].contains(k);
        let head = (j..t.len()).take_while(|&k| !stop(&t[k]));
        let (from, label) = match (name, kind, owner) {
            (":", _, Some(o)) => (j + 2, format!("{o}.{kind}")),
            // A `fn` hands out its return type; a `const` its type.
            (_, "fn", _) => {
                let arrow = head.clone().find(|&k| t[k] == ">" && t[k - 1] == "-");
                (arrow.map_or(t.len(), |k| k + 1), format!("{kind} {name}"))
            }
            (_, "const", _) => {
                let typed = head.clone().find(|&k| t[k] == ":");
                (typed.map_or(t.len(), |k| k + 1), format!("{kind} {name}"))
            }
            _ => (t.len(), format!("{kind} {name}")),
        };
        // A field's type ends at its depth-0 `,` or the struct's `}`.
        let mut depth = 0i32;
        let to = (from..t.len()).find(|&k| {
            depth += i32::from(["<", "(", "["].contains(&t[k]))
                - i32::from([">", ")", "]"].contains(&t[k]));
            stop(&t[k]) || t[k] == "}" || (name == ":" && t[k] == "," && depth == 0)
        });
        let to = to.unwrap_or(t.len());
        let typed = t[from.min(to)..to].iter().find(|x| T7_TYPES.contains(x));
        if let Some(ty) = typed.filter(|_| label != S5_BY_VALUE) {
            let found = format!("S5_EXPORTS: {label} hands out a composed type {ty}");
            handed.push((item, found));
        }
        match (name, owner) {
            (":", Some(_)) => match rows.last_mut() {
                Some((_, fields)) => fields.push(kind.to_string()),
                None => rows.push((label, Vec::new())),
            },
            _ => rows.push((label, Vec::new())),
        }
    }
    let rows = rows.into_iter().map(|(r, f)| match f.is_empty() {
        true => r,
        false => format!("{r}{{{}}}", f.join(",")),
    });
    (rows.collect(), handed)
}
/// W0b B4: the `pub` surface of `s5.rs` is exactly `S5_EXPORTS`; no `s` row's
/// item but `S5_BY_VALUE` hands out a composed type (SB3E-R106.3: the `h` rows
/// are `grant.rs`'s, which no section imports).
fn exports_findings(s5: &str) -> Vec<String> {
    let (rows, handed) = exports_of(s5);
    let harness: Vec<&str> = S5_EXPORTS
        .lines()
        .filter_map(|l| l.strip_prefix("h|"))
        .map(|r| r.split('{').next().unwrap_or(r))
        .collect();
    let section = handed
        .into_iter()
        .filter(|(item, _)| !harness.contains(&item.as_str()));
    let mut found: Vec<String> = section.map(|(_, f)| f).collect();
    let listed: Vec<&str> = S5_EXPORTS
        .lines()
        .filter_map(|l| l.split_once('|'))
        .map(|l| l.1)
        .collect();
    for row in rows.iter().filter(|r| !listed.contains(&r.as_str())) {
        found.push(format!("S5_EXPORTS: unlisted {row}"));
    }
    for row in listed.iter().filter(|l| !rows.iter().any(|r| r == *l)) {
        found.push(format!("S5_EXPORTS: missing {row}"));
    }
    if rows != listed {
        found.push("S5_EXPORTS: not exactly the list".into());
    }
    found
}
/// W0b B4: the `s5.rs` items a section's row may import (class `s`).
fn section_exports() -> Vec<&'static str> {
    let rows = S5_EXPORTS.lines().filter_map(|l| l.strip_prefix("s|"));
    rows.filter_map(|r| r.split([' ', '{']).nth(1)).collect()
}
/// SB3E-R15.2's sweep: at each statement boundary inside a pinned region,
/// and before each `&&` or `||`, a no-op-looking insertion; each region's
/// mutant count, and every mutant that still pins (or a region not found).
fn sweep(sources: &[(String, String)], pins: &[(&str, &str)]) -> (Vec<usize>, Vec<String>) {
    let (mut counts, mut survivors) = (Vec::new(), Vec::new());
    for (file, pin) in pins {
        let source = text(sources, file);
        let (tokens, literal, base) = (lex(source), piece(pin), source.as_ptr() as usize);
        let Some(at) = tokens.windows(literal.len()).position(|w| w == literal) else {
            survivors.push(format!("pin {file}: {} not found", label(pin)));
            counts.push(0);
            continue;
        };
        let last = tokens[at + literal.len() - 1];
        let start = tokens[at].as_ptr() as usize - base;
        let end = last.as_ptr() as usize - base + last.len();
        let b = mask(source).into_bytes();
        let mut mutants = 0;
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
        counts.push(mutants);
    }
    (counts, survivors)
}
/// `sources` with one exact replacement in `file`.
fn planted(
    sources: &[(String, String)],
    file: &str,
    old: &str,
    new: &str,
) -> Vec<(String, String)> {
    let mut planted = sources.to_vec();
    let at = planted.iter().position(|(f, _)| f == file).expect(file);
    assert_eq!(planted[at].1.matches(old).count(), 1, "{file}: {old}");
    planted[at].1 = planted[at].1.replace(old, new);
    planted
}

#[test]
fn sb3e_mother_lexer_is_governed_controls() {
    let own = include_str!("route_seams_guard.rs");
    for name in LEXER {
        assert!(item(GC, name).is_some(), "{name}: in governed-control");
        assert_eq!(item(own, name), item(GC, name), "{name}: the copy drifted");
    }
}

/// Step 0 item 8: exactly the audited sites, each name gated exactly by
/// `#[cfg(mother_route_seams)]` (never `cfg(test)` or broadened), every
/// reviewed region pinned once, the markers and S5's module shape; G-S1's
/// whole module and include sites; G-S2's pins digest. Each R42 plant shows
/// its own finding.
#[test]
fn sb3e_mother_route_sites_are_exactly_the_inventory() {
    let ((sources, mut found), cells) = (walked(), crate::adapter::scan::route_cells_files());
    found.extend(inventory_findings(&sources));
    found.extend(pins_findings(PIN_DATA));
    found.extend(cells_findings(&cells));
    found.extend(sections_findings(&sources, &cells));
    // The plants below are built from the reviewed text; on a changed tree
    // its findings are reported first, whole, so no plant's own anchor check
    // can mask them (SB3E-R42.6).
    if !found.is_empty() {
        return fail_on(found);
    }
    let mut misses = Vec::new();
    let seams = "src/route_seams.rs";
    let anchor = "pub use session_close::PreparedSessionEnd;\n";
    let state = "fn state() -> MutexGuard<'static, State> {\n";
    let export = "\n#[macro_export]\nmacro_rules! seam {\n    ($p:expr) => {\n        $crate::route_seams::at($p, Default::default())\n    };\n}\n";
    let plants = [
        (
            "G-S1 a #[macro_export] seam macro",
            planted(
                &sources,
                seams,
                "\nfn state()",
                &format!("{export}\nfn state()"),
            ),
            "G-S1: a macro definition in src/route_seams.rs",
        ),
        (
            "G-S1 code added to state()",
            planted(
                &sources,
                seams,
                state,
                &format!("{state}    ARMED.store(true, Ordering::SeqCst);\n"),
            ),
            "G-S1: src/route_seams.rs is not its pin, whole",
        ),
        (
            "G-S1 an include! site",
            planted(
                &sources,
                "src/coordinator.rs",
                anchor,
                &format!("{anchor}include!(\"more.rs\");\n"),
            ),
            "G-S1: src/coordinator.rs|include|\"more.rs\" is not allowed",
        ),
        (
            "G-S1 a #[path] module",
            planted(
                &sources,
                "src/coordinator.rs",
                anchor,
                &format!("{anchor}#[path = \"../../kernel/src/more.rs\"]\nmod more;\n"),
            ),
            "G-S1: src/coordinator.rs|path|\"../../kernel/src/more.rs\" is not allowed",
        ),
        (
            "G-S1 a #[path = r\"..\"] module (R44, B2)",
            planted(
                &sources,
                "src/coordinator.rs",
                anchor,
                &format!("{anchor}#[path = r\"../../kernel/src/more.rs\"]\nmod more;\n"),
            ),
            "G-S1: src/coordinator.rs|path|r\"../../kernel/src/more.rs\" is not allowed",
        ),
        (
            "R44.3 a source that does not tokenize",
            planted(
                &sources,
                "src/coordinator.rs",
                anchor,
                &format!("{anchor}}}\n"),
            ),
            "lex: src/coordinator.rs does not tokenize",
        ),
        (
            "R44.2 a new impl Fixture helper (S2)",
            planted(
                &sources,
                "host/tests/route_cells/main.rs",
                "struct Never;\n",
                "impl Fixture {\n    fn again(&self) {}\n}\nstruct Never;\n",
            ),
            "R44.2 harness impls: not exactly the list",
        ),
        // SB3E-R86.4 (T7): another constructor in s5.rs; production's changed.
        (
            "T7 another constructor in s5.rs",
            planted(
                &sources,
                S5,
                "HostSession::open_with_history(\n        seed,\n        &home.join(\"broker\"),\n        &home,\n",
                "HostSession::open(\n        seed,\n        &home.join(\"broker\"),\n",
            ),
            "T7: host/tests/route_cells/s5.rs constructs HostSession::open(",
        ),
        (
            "T7 a production constructor changed",
            planted(
                &sources,
                "host/src/hands.rs",
                ".open_confirmed_file_door(request, confirmation)",
                ".open_file_door(request)",
            ),
            "T7: host/src/hands.rs: open_confirmed_file_door ( 0 times, not 1",
        ),
        // SB3E-R88.2: the expected constructor kept as an inactive copy.
        (
            "T7 an inactive copy of production's constructor",
            planted(
                &sources,
                "host/src/session_home.rs",
                "            let mut session = HostSession::open_with_history(\n                seed,\n                &dir.join(\"broker\"),\n                dir,\n                evidence.clone(),\n                Arc::new(SystemClock::default()),\n                confirmer,\n            )\n",
                "            let mut session = if false {\n                HostSession::open_with_history(\n                    seed,\n                    &dir.join(\"broker\"),\n                    dir,\n                    evidence.clone(),\n                    Arc::new(SystemClock::default()),\n                    confirmer,\n                )\n            } else {\n                let clock = Arc::new(SystemClock::default());\n                HostSession::open(seed, &dir.join(\"broker\"), evidence.clone(), clock, confirmer)\n            }\n",
            ),
            "pin host/src/session_home.rs: pub(crate)fnopen_projected(seed",
        ),
    ];
    for (plant, sources, expected) in plants {
        killed(
            &mut misses,
            plant,
            &inventory_findings(&sources),
            &[expected],
        );
    }
    // SB3E-R51.1: a file added, a file removed, one byte changed.
    let mut added = cells.clone();
    added.push((format!("{CELLS}extra.rs"), b"fn extra() {}\n".to_vec()));
    let removed: Vec<_> = cells
        .iter()
        .filter(|c| !c.0.ends_with("/grant.rs"))
        .cloned()
        .collect();
    let mut changed = cells.clone();
    let c = changed
        .iter_mut()
        .find(|c| c.0.ends_with("/route_fsync.c"))
        .unwrap();
    c.1[0] ^= 1;
    // SB3E-R86.4: `witness.rs` removed; one byte of it changed.
    let unwitnessed: Vec<_> = cells
        .iter()
        .filter(|c| !c.0.ends_with("/witness.rs"))
        .cloned()
        .collect();
    let mut rewitnessed = cells.clone();
    let w = rewitnessed
        .iter_mut()
        .find(|c| c.0.ends_with("/witness.rs"))
        .unwrap();
    w.1[0] ^= 1;
    for (plant, files, expected) in [
        (
            "R51.1 a removed witness.rs",
            unwitnessed,
            "R51.1: host/tests/route_cells/witness.rs is missing",
        ),
        (
            "R51.1 one byte of witness.rs",
            rewitnessed,
            "R51.1: host/tests/route_cells/witness.rs's sha256",
        ),
        (
            "R51.1 an added extra.rs",
            added,
            "R51.1: host/tests/route_cells/extra.rs is not listed",
        ),
        (
            "R51.1 a removed grant.rs",
            removed,
            "R51.1: host/tests/route_cells/grant.rs is missing",
        ),
        (
            "R51.1 one byte of route_fsync.c",
            changed,
            "R51.1: host/tests/route_cells/route_fsync.c's sha256",
        ),
    ] {
        killed(&mut misses, plant, &cells_findings(&files), &[expected]);
    }
    let row = format!("{ROUTE_CELLS}cells.rs\n");
    let expected = ["R51.1: a malformed ROUTE_CELLS row \"cells.rs\""];
    killed(
        &mut misses,
        "R55.6 a malformed row",
        &rows_findings(&row),
        &expected,
    );
    // SB3E-R55.2 (A-1): a skipped directory below src/, a symlinked file and
    // a symlinked directory, each its own finding; the skipped one is read.
    let (files, mut walk) = crate::adapter::scan::route_guard_walk_plant();
    walk.extend(files.into_iter().map(|(f, _)| format!("read {f}")));
    for e in [
        "A-1: src/dist is a skipped directory below src/ or tests/",
        "A-1: src/linked.rs is a symlink",
        "A-1: src/linkdir is a symlink",
        "read src/dist/mod.rs",
    ] {
        killed(&mut misses, e, &walk, &[e]);
    }
    // SB3E-R51.2: a shebang line hiding a site from the lexer (Fable B1).
    let mut shebang = sources.clone();
    let at = shebang
        .iter()
        .position(|(f, _)| f == "src/coordinator.rs")
        .unwrap();
    let hidden = "#! /*\n#[cfg(mother_route_seams)] pub fn extra() {}\n// */\n";
    shebang[at].1 = format!("{hidden}{}", shebang[at].1.replace("//!", "//"));
    let expected = ["lex: src/coordinator.rs starts with a shebang"];
    killed(
        &mut misses,
        "R51.2 a shebang",
        &inventory_findings(&shebang),
        &expected,
    );
    // G-S2: one region shrunk by its last line.
    let second = PIN_DATA.match_indices("\n@@@ ").nth(1).unwrap().0;
    let cut = PIN_DATA[..second].rfind('\n').unwrap();
    let shrunk = format!("{}{}", &PIN_DATA[..cut], &PIN_DATA[second..]);
    let expected = ["G-S2: the pins file's sha256"];
    killed(
        &mut misses,
        "G-S2 a shrunk pin",
        &pins_findings(&shrunk),
        &expected,
    );
    // SB3E-R44.3: a feature added to, or the version changed in, its line.
    let (file, manifest) = MANIFESTS[0];
    let feature = LEXER_DEP.replace(" }", ", features = [\"span-locations\"] }");
    for new in [feature, LEXER_DEP.replace("=1.0.106", "1.0.106")] {
        let planted = manifest_findings(file, &manifest.replace(LEXER_DEP, &new));
        let expected = ["R44.3: mother/Cargo.toml: proc-macro2"];
        killed(&mut misses, &new, &planted, &expected);
    }
    found.extend(misses);
    fail_on(found);
}

/// AM-2 A.2 item 3 over `s5.rs`, with the rev 4.2 plants, R39.1's timeout,
/// rev 3 A.5's conditions and the R42 plants: the file as it is passes; each
/// plant shows its own finding.
#[test]
fn sb3e_s5_module_makes_one_counted_call() {
    let sources = sources();
    let s5 = text(&sources, S5);
    let mut found = s5_findings(s5);
    // F-S5-SPLIT: the counted call stays in s5.rs; no section makes one.
    for row in section_rows(S5_SECTIONS)
        .iter()
        .filter(|r| r.file != "s5.rs")
    {
        let section = find(&sources, &format!("{CELLS}{}", row.file)).unwrap_or("");
        found.extend(section_findings(row.file, row, section, &pins()));
    }
    // The plants below are built from the reviewed text; on a changed tree
    // its findings are reported first, whole, so no plant's own anchor check
    // can mask them (SB3E-R42.6).
    if !found.is_empty() {
        return fail_on(found);
    }
    let sub = |old: &str, new: &str| {
        assert_eq!(s5.matches(old).count(), 1, "{old}");
        s5.replace(old, new)
    };
    let invoke = "        let text = if drive_call(&self.0, context) {";
    let before = |line: &str| sub(invoke, &format!("        {line}\n{invoke}"));
    let open = "        Arc::new(SessionPort::new(b.door, b.name.clone())),";
    let kept = "let port = Arc::new(SessionPort::new(b.door, b.name.clone()));";
    let count = "    calls.fetch_add(1, Ordering::SeqCst);\n";
    let run = "    fn run(&self) {\n";
    let write = "    use std::io::Write;\n";
    let door = "    let door = o.session.open_file_door(&request).unwrap();\n";
    let credited = "let witness = credit(&driver, &RECORDED";
    let pause = "    let out = bounded(command.env(CHILD, &root.0));\n";
    let pause_test = "route_s5_unreleased_pause_times_out_and_aborts";
    let no_root = "route_s5_timeout_without_a_child_root_writes_nothing";
    let unbounded = format!("G-S4: {pause_test}: an unbounded child wait");
    let owned = format!("G-S4: {pause_test}: its temp dir is not removed");
    let kept_no_root = format!("G-S4: {no_root}: its temp dir is not removed");
    let mut plants = vec![
        ("A.2 a second call", before("let _ = context.model.map(|m| m.call(&r));"), vec!["(a) one .call"]),
        ("A.2 a re-export", format!("{s5}pub(super) use nexus_governed_control::inference::ModelCaller as Handle;\n"), vec!["(b) no re-export"]),
        ("A.2 the count after the call", sub(count, "").replace(".is_ok()\n}", ".is_ok() && calls.fetch_add(1, Ordering::SeqCst) < 9\n}"), vec!["pin host/tests/route_cells/s5.rs: fndrive_call("]),
        ("A.2 a reset", sub(run, &format!("{run}        self.calls.store(0, Ordering::SeqCst);\n")), vec!["(a) counter write `store` outside drive_call"]),
        ("A.2 a kept port", sub(open, "        port.clone(),").replace("    let m = Coordinator::open(", &format!("    {kept}\n    let _ = port.model_call(id, &q, &meter, &d);\n    let m = Coordinator::open(")), vec!["(c) the port made outside Coordinator::open"]),
        ("A.2 the door's call", sub(door, &format!("{door}    let _ = door.model_call(run, &q, &meter, &d, i);\n")), vec!["(b) model_call"]),
        ("A.2 a store helper", format!("use super::store::call_model;\n{s5}"), vec!["(b) call_model"]),
        // Built from parts so the host-access scan does not match this file.
        ("timeout without abort", sub(concat!("    std::", "process::abort()\n}\n\n/// SB3E-R42.5"), "    let _ = root;\n}\n\n/// SB3E-R42.5"), vec!["timeout: ends in abort"]),
        ("timeout unwraps", sub("let Some(root) = std::env::var_os(CHILD).map(PathBuf::from) else {", "let root = std::env::var_os(CHILD).map(PathBuf::from).unwrap();\n    {"), vec!["timeout: a panic path unwrap"]),
        ("G-B1 r#call", before("let r#call = 1;"), vec!["G-B1: a raw identifier r#call", "(a) a bare `call` outside drive_call"]),
        ("G-B1 r#model", before("let _ = context.r#model.is_some();"), vec!["G-B1: a raw identifier r#model", "(a) a bare `model` outside drive_call"]),
        ("G-B1 r#store", sub(run, &format!("{run}        self.calls.r#store(0, Ordering::SeqCst);\n")), vec!["G-B1: a raw identifier r#store", "(a) counter write `store` outside drive_call"]),
        ("G-B1 a local macro", format!("{s5}macro_rules! again {{\n    () => {{}};\n}}\n"), vec!["G-B1: a macro definition macro_rules"]),
        ("G-B1 an unlisted macro", before("let _ = concat!(\"a\", \"b\");"), vec!["G-B1: an unlisted macro concat!"]),
        // SB3E-R57.6 (P-CFG): `cfg` is listed only as `cfg!(mother_route_seams)`.
        ("P-CFG cfg!(test) in s5.rs", before("let _ = cfg!(test);"), vec!["R57.6: host/tests/route_cells/s5.rs: a cfg! other than cfg!(mother_route_seams): cfg!(test)"]),
        ("G-S3 a new sibling's helper", before("let _ = super::helper::drive(context);"), vec!["G-S3: a super:: path outside the import"]),
        ("G-S3 use super::{store as s}", format!("use super::{{store as s}};\n{s5}"), vec!["G-S3: the super imports"]),
        ("G-S3 a direct Driver credited", sub(credited, "let witness = credit(&Driver { calls: 1, ..driver.clone() }, &RECORDED"), vec!["pin host/tests/route_cells/s5.rs: let(driver,broker,mother)"]),
        ("G-S4 an unbounded wait", sub(pause, "    let out = command.env(CHILD, &root.0).output().unwrap();\n"), vec![&*unbounded]),
        ("G-S4 a kept temp dir", sub("    let root = Fixture(root); // Removed when the test ends.\n", ""), vec![&*owned]),
        ("G-S4 a kept no-root dir", sub("    let cwd = Fixture(cwd); // Removed when the test ends.\n", ""), vec![&*kept_no_root]),
    ];
    let banned = [
        "assert_eq",
        "assert_ne",
        "debug_assert",
        "debug_assert_eq",
        "debug_assert_ne",
        "unimplemented",
        "println",
    ];
    let banned: Vec<(String, String, String)> = banned
        .iter()
        .map(|m| {
            (
                format!("G-S4 {m}! in timeout"),
                sub(write, &format!("{write}    {m}!();\n")),
                format!("timeout: a panic path {m}"),
            )
        })
        .collect();
    let mut misses = Vec::new();
    for (plant, text, expected) in plants.drain(..) {
        killed(&mut misses, plant, &s5_findings(&text), &expected);
    }
    for (plant, text, expected) in &banned {
        killed(&mut misses, plant, &s5_findings(text), &[expected]);
    }
    // Rev 3 A.5: each `mother.admit` condition, pinned whole.
    let coordinator = text(&sources, "src/coordinator.rs");
    let pin = pins()
        .into_iter()
        .find(|(_, p)| p.contains("\"mother.admit\""));
    let pin = pin.map_or("", |p| p.1);
    let lane = "if let Event::LaneReturned { invocation, .. } = &entry.event {";
    let widened = "if let Event::LaneReturned { invocation, .. } | Event::ModelCallCharged { invocation, .. } = &entry.event {";
    let narrowed = "if let Some(invocation) = entry.event.invocation() {";
    for planted in [
        coordinator.replacen(lane, narrowed, 1),
        coordinator.replacen(lane, widened, 1),
    ] {
        if pin.is_empty() || pinned(&planted, pin) == 1 {
            misses.push("A.5: a changed condition still pins".into());
        }
    }
    found.extend(misses);
    fail_on(found);
}

/// A plant's findings, printed whole (a RED run without its check reads
/// them), then `killed`.
fn shown(misses: &mut Vec<String>, plant: &str, found: &[String], expected: &[&str]) {
    println!("PLANT {plant}: {found:?}");
    killed(misses, plant, found, expected);
}

/// F-S5-SPLIT (the adopted amendment, section 6.1): the S5 boundary's closed
/// list, its sections' headers, surfaces, reach and profile, the arm binding
/// and the alias rules pass on the tree as it is; then each new check's plant
/// shows its own finding.
#[test]
fn sb3e_s5_sections_are_the_closed_list() {
    use sha2::Digest;
    let ((sources, mut found), cells) = (walked(), crate::adapter::scan::route_cells_files());
    found.extend(sections_findings(&sources, &cells));
    // The plants below are built from the reviewed text; on a changed tree
    // its findings are reported first, whole (SB3E-R42.6).
    if !found.is_empty() {
        return fail_on(found);
    }
    let (rows, pins, mut misses) = (section_rows(S5_SECTIONS), pins(), Vec::new());
    let path = |f: &str| format!("{CELLS}{f}");
    let row = |f: &str| rows.iter().find(|r| r.file == f).expect(f);
    let stub = |f: &str| text(&sources, &path(f)).to_string();
    let section = |f: &str, more: &str| section_findings(f, row(f), &(stub(f) + more), &pins);
    // The tree with the directory's `file` set to `text`, as sources and files.
    let with = |file: &str, text: &str| {
        let (mut s, mut c) = (sources.clone(), cells.clone());
        s.retain(|(f, _)| *f != path(file));
        c.retain(|(f, _)| *f != path(file));
        s.push((path(file), text.to_string()));
        c.push((path(file), text.as_bytes().to_vec()));
        s.sort();
        c.sort();
        (s, c)
    };
    let digest = |text: &str| hex::encode(sha2::Sha256::digest(text.as_bytes()));
    let swap = |text: &str, old: &str, new: &str| {
        assert_eq!(text.matches(old).count(), 1, "{old}");
        text.replace(old, new)
    };
    let (s5, main, cells_rs) = (text(&sources, S5), stub("main.rs"), stub("cells.rs"));
    // P-S1, P-S2, P-C1, P-C2: the closed list and distinct digests.
    let (s, c) = with("s5_extra.rs", "//! SB-3E S5 section U9a: s5_extra.rs\n");
    let s = planted(
        &s,
        &path("main.rs"),
        "mod s5_reg;\n",
        "mod s5_extra;\nmod s5_reg;\n",
    );
    let mut got = sections_findings(&s, &c);
    got.extend(cells_findings(&c));
    got.extend(inventory_findings(&s));
    let expected = [
        "S5 sections: s5_extra.rs is not a listed section",
        "R51.1: host/tests/route_cells/s5_extra.rs is not listed",
        "R44.2 main.rs items: unlisted mods5_extra;",
    ];
    shown(&mut misses, "P-S1 an unlisted section", &got, &expected);
    for (plant, file, text, expected) in [
        (
            "P-S2 a listed file in neither list",
            "extra.rs",
            "fn extra() {}\n",
            "S5 sections: extra.rs is neither a section nor a non-section file",
        ),
        (
            "P-C1 a byte copy of s5.rs, listed",
            "s5_copy.rs",
            s5,
            "S5 sections: s5_copy.rs is not a listed section",
        ),
    ] {
        let ((s, c), listed) = (
            with(file, text),
            format!("{ROUTE_CELLS}{file} {}\n", digest(text)),
        );
        let got = sections_with(&listed, S5_SECTIONS, &s, &c);
        shown(&mut misses, plant, &got, &[expected]);
    }
    let (s, c) = with("s5_r34.rs", &stub("s5_close.rs"));
    let expected = ["S5 sections: s5_close.rs and s5_r34.rs share one digest"];
    shown(
        &mut misses,
        "P-C2 one digest",
        &sections_findings(&s, &c),
        &expected,
    );
    // P-W1, P-W2: an arm and a control bound to a section that does not own them.
    let head = "macro_rules! cells {\n    () => { ctl!(ctl_call_chat, call_chat ingress, s5::ctl_call_chat); ctl!(ctl_close_obligations, close_obligations lifecycle, s5_close::ctl_close_obligations); ctl!(ctl_delivery, delivery lifecycle, s5_close::ctl_delivery); ctl!(ctl_mother_end, mother_end lifecycle, s5_close::ctl_mother_end); ctl!(ctl_primary_end, primary_end lifecycle, s5_close::ctl_primary_end); ctl!(ctl_producer_join, producer_join lifecycle, s5_close::ctl_producer_join); ctl!(ctl_seal, seal lifecycle, s5_close::ctl_seal); ctl!(ctl_registration_record, registration_record ingress, s5_reg::ctl_registration_record); ctl!(ctl_close_restrict, close_restrict lifecycle, s5_r6::ctl_close_restrict); ctl!(ctl_emergency_restrict, emergency_restrict lifecycle, s5_r6::ctl_emergency_restrict); ctl!(ctl_registration_reserve, registration_reserve ingress, s5_reg::ctl_registration_reserve); ctl!(ctl_registration_readiness, registration_readiness ingress, s5_reg::ctl_registration_readiness); ctl!(ctl_close_report, close_report lifecycle, s5_report::ctl_close_report); };\n";
    let (cell, control) = (
        "    (r9 seal lifecycle closingnormal; $($rest:tt)*) => { cell!(rc_r9_seal_lifecycle_closingnormal, r9 seal lifecycle closingnormal, s5_r34::closingnormal); cells! { $($rest)* } };\n",
        "macro_rules! cells {\n    () => { ctl!(ctl_call_chat, call_chat ingress, s5::ctl_call_chat); ctl!(ctl_seal, seal lifecycle, s5_reg::ctl_seal); };\n",
    );
    let wrong = "S5 sections: the arm rc_r9_seal_lifecycle_closingnormal calls s5_r34, which does not own seal:lifecycle";
    let arm = swap(&main, head, &format!("{head}{cell}"));
    shown(
        &mut misses,
        "P-W1 an arm in another section",
        &macro_findings(&arm, &cells_rs),
        &[wrong],
    );
    let ctl = swap(&main, head, control);
    let expected = ["S5 sections: ctl_seal calls s5_reg, which does not list it"];
    shown(
        &mut misses,
        "P-W2 a control in another section",
        &macro_findings(&ctl, &cells_rs),
        &expected,
    );
    // P-W3, P-W4, P-W6 and S5 armed outside the boundary.
    let witness = swap(
        &stub("witness.rs"),
        "use super::s5::{Ack, Facts, Ran};\n",
        "use super::s5::{Ack, Facts, Ran};\nuse super::s5_close::seal;\n",
    );
    let grant = swap(
        &stub("grant.rs"),
        "use super::s5::{boot, consented, lanes, Recorded};\n",
        "use super::s5::{boot, consented, lanes, Recorded};\nuse nexus_mother::route_seams::arm;\n",
    );
    for (plant, file, text, expected) in [
        (
            "P-W3 a header naming another unit",
            "s5_close.rs",
            "//! SB-3E S5 section U6b-2: s5_close.rs\n".to_string(),
            "S5 sections: s5_close.rs names U6b-2, not U9a",
        ),
        (
            "P-W4 an unbound pub(super) fn",
            "s5_reg.rs",
            stub("s5_reg.rs") + "pub(super) fn spare() {}\n",
            "S5 sections: s5_reg.rs exposes an unbound pub(super) fn spare",
        ),
        (
            "P-W6 witness.rs naming a section",
            "witness.rs",
            witness,
            "S5 sections: witness.rs names section s5_close",
        ),
        (
            "rule: S5 armed in grant.rs",
            "grant.rs",
            grant,
            "S5 sections: grant.rs reaches nexus_mother::route_seams other than as ::Entry",
        ),
    ] {
        let (s, c) = with(file, &text);
        shown(&mut misses, plant, &sections_findings(&s, &c), &[expected]);
    }
    // P-C3, P-C4, P-W5, P-T1 to P-T6, P-A1 to P-A3, P-A7 to P-A10 and one plant
    // per section rule: a stub with the planted text.
    let drive = item(s5, "drive_call").unwrap_or("").to_string() + "\n";
    let timeout = item(s5, "timeout")
        .unwrap_or("")
        .replace("fn timeout", "fn section_timeout");
    let arm = "use nexus_mother::route_seams::{arm, Action};\n";
    let pause = |callback: &str| {
        format!("fn d() {{\n    let _h = arm(\"mother.charge.entry\", Action::Pause {{ cell: \"d\", {callback} }});\n}}\n")
    };
    let made =
        "fn d(f: &Fixture) {\n    let _j = J::open(&f.0.join(\"extra.jsonl\")).unwrap();\n}\n";
    let inline = "mod m {\n    pub use nexus_mother::adapter::FileJournal as Fj;\n}\nfn d(f: &Fixture) {\n    let _j = m::Fj::open(&f.0.join(\"extra.jsonl\")).unwrap();\n}\n";
    let compose = "fn compose(f: &Fixture, o: &Opened) {\n    let _m = Coordinator::open(f.0.join(\"m\"), o);\n}\n";
    let callback = "a pause callback that is not s5::timeout";
    let direct = "Action or arm imported from route_seams";
    let journal = "Passive: collect reads the journal through journal(), not journal_view()";
    let cfg = "R57.6: host/tests/route_cells/s5_reg.rs: a cfg! other than cfg!(mother_route_seams): cfg!(test)";
    // `std::process` is built from parts so the host-access scan does not match this file.
    let mut profile: Vec<(&str, &str, String, &str)> = vec![
        ("P-C3 a counted call copied", "s5_close.rs", drive, "a model call"),
        ("P-W5 a path through another section", "s5_r34.rs", "use super::s5_close::seal;\n".into(), "a path through s5_close"),
        ("P-T1 a closure callback", "s5_close.rs", arm.to_string() + &pause(concat!("timeout: |l| -> ! { std::", "process::abort() }")), callback),
        ("P-T2 a panic! closure", "s5_close.rs", arm.to_string() + &pause("timeout: |_| panic!(\"timeout\")"), callback),
        ("P-T3 a renamed import", "s5_close.rs", format!("use super::s5::{{timeout as t}};\n{arm}{}", pause("timeout: t")), callback),
        ("P-T4 an alias binding", "s5_close.rs", format!("use nexus_mother::route_seams::{{arm, Action, Timeout}};\nfn e(timeout: Timeout) {{\n    let t: Timeout = timeout;\n    let _ = t;\n}}\n{}", pause("timeout: t")), callback),
        ("P-T4 a rebinding", "s5_close.rs", format!("{arm}fn other_fn(_: &str) -> ! {{\n    loop {{}}\n}}\nfn e() {{\n    let timeout = other_fn;\n    let _h = arm(\"mother.charge.entry\", Action::Pause {{ cell: \"d\", timeout }});\n}}\n"), callback),
        ("P-T5 a look-alike timeout", "s5_close.rs", format!("{arm}{timeout}\n{}", pause("timeout: section_timeout")), callback),
        ("P-T6 Mother's Action and arm imported", "s5_close.rs", arm.into(), direct),
        ("P-T6 the broker's Action and arm imported", "s5_close.rs", "use nexus_governed_control::broker::route_seams::{arm, Action};\n".into(), direct),
        ("P-A1 a type alias", "s5_close.rs", format!("type J = nexus_mother::adapter::FileJournal;\n{made}"), "host/tests/route_cells/s5_close.rs: type alias names a T7 type: J"),
        ("P-A2 use .. as", "s5_close.rs", format!("use nexus_mother::adapter::FileJournal as J;\n{made}"), "use .. as in a section"),
        ("P-A3 an inline module", "s5_close.rs", inline.into(), "an inline module in a section"),
        ("P-A7 a crate-root path", "s5_close.rs", "fn d(f: &Fixture) {\n    let _m = nexus_mother::coordinator::Coordinator::open(f.0.join(\"m\"));\n}\n".into(), "a crate-root path outside use: nexus_mother::"),
        ("P-A8 an unlisted use", "s5_close.rs", "use nexus_mother::adapter::FileJournal;\n".into(), "use not on its row: nexus_mother::adapter::FileJournal"),
        ("P-A9 a qualified path", "s5_close.rs", "fn d() {\n    let _ = <Vec<u8> as Default>::default();\n}\n".into(), "a qualified-self path <..>::"),
        ("P-A10 an extern crate alias", "s5_close.rs", "extern crate nexus_mother as m;\n".into(), "an extern item in a section"),
        ("rule: a SessionPort token", "s5_reg.rs", "fn d(port: SessionPort) {\n    drop(port);\n}\n".into(), "port uses"),
        ("rule: a .output( child wait", "s5_reg.rs", "fn d(c: &mut Command) {\n    let _ = c.output();\n}\n".into(), "a child wait (output)"),
        ("rule: fn timeout", "s5_reg.rs", "fn timeout(_: &str) -> ! {\n    loop {}\n}\n".into(), "fn timeout"),
        ("rule: abort", "s5_reg.rs", concat!("fn d() {\n    std::", "process::abort()\n}\n").into(), "abort"),
        ("rule: macro_rules!", "s5_reg.rs", "macro_rules! again {\n    () => {};\n}\n".into(), "G-B1: a macro definition macro_rules"),
        ("rule: an unlisted macro", "s5_reg.rs", "fn d() {\n    let _ = concat!(\"a\", \"b\");\n}\n".into(), "G-B1: an unlisted macro concat!"),
        ("rule: a raw identifier", "s5_reg.rs", "fn d() {\n    let r#seal = 1;\n    let _ = r#seal;\n}\n".into(), "G-B1: a raw identifier r#seal"),
        ("rule: a glob import", "s5_reg.rs", "use super::*;\n".into(), "a glob import"),
        ("rule: cfg!(test)", "s5_reg.rs", "fn d() -> bool {\n    cfg!(test)\n}\n".into(), cfg),
        ("rule: collect reads journal()", "s5_r34.rs", "fn collect(m: &Opened) -> usize {\n    m.journal().unwrap().len()\n}\n".into(), journal),
        ("rule: collect reads store.commit", "s5_r34.rs", "fn collect_commits(broker: &[Entry]) -> usize {\n    broker.iter().filter(|e| e.point == \"store.commit\").count()\n}\n".into(), "Passive: collect_commits reads store.commit"),
        ("rule: an unlisted impl", "s5_r34.rs", "struct Z;\nimpl Z {\n    fn d(&self) {}\n}\n".into(), "an unlisted impl implZ"),
    ];
    // U9b D7: a pause call in every section file, in each form (R102.3).
    let calls = [
        "f.pause(\"ingress.accept\")",
        "Fixture::pause(f, \"ingress.accept\")",
        "(f).pause(\"ingress.accept\")",
        "{ let p = Fixture::pause; p(f, \"ingress.accept\") }",
    ];
    for file in ["s5_close.rs", "s5_reg.rs", "s5_r34.rs", "s5_r6.rs"] {
        for call in calls {
            let text = format!("fn d(f: &Fixture) {{\n    let _h = {call};\n}}\n");
            profile.push((
                "D7 a section pause call",
                file,
                text,
                "a pause call (F-U9A-PAUSE)",
            ));
        }
    }
    for (plant, file, text, expected) in &profile {
        let expected = match expected.starts_with(CELLS) {
            true => expected.to_string(),
            false => format!("section {file}: {expected}"),
        };
        shown(
            &mut misses,
            plant,
            &section(file, text),
            &[expected.as_str()],
        );
    }
    let (s, _) = with("s5_r34.rs", &(stub("s5_r34.rs") + compose));
    let mut got = section("s5_r34.rs", compose);
    got.extend(t7_findings(&s));
    let expected = [
        "section s5_r34.rs: constructs Coordinator::open(",
        "T7: host/tests/route_cells/s5_r34.rs: Coordinator : : open ( 1 times, not 0",
    ];
    shown(
        &mut misses,
        "P-C4 a section composing the route",
        &got,
        &expected,
    );
    // A section's pinned region, changed and kept.
    let region = "fn pinned_driver() {\n    let _ = 1;\n}\n";
    let close = path("s5_close.rs");
    let regions = [(close.as_str(), region)];
    let changed = stub("s5_close.rs") + &region.replace('1', "2");
    let got = section_findings("s5_close.rs", row("s5_close.rs"), &changed, &regions);
    let expected = ["section s5_close.rs: pin fnpinned_driver(){let_= matched 0"];
    shown(
        &mut misses,
        "rule: an unpinned change to a pinned region",
        &got,
        &expected,
    );
    let kept = section_findings(
        "s5_close.rs",
        row("s5_close.rs"),
        &(stub("s5_close.rs") + region),
        &regions,
    );
    if !kept.is_empty() {
        misses.push(format!("the kept region: {kept:?}"));
    }
    // P-A4 to P-A6: the alias check on s5.rs, beside its byte finding.
    let byte = "G-S5: host/tests/route_cells/s5.rs's sha256";
    for (plant, more, expected) in [
        (
            "P-A4 a type alias in s5.rs",
            format!("type J = nexus_mother::adapter::FileJournal;\n{made}"),
            "host/tests/route_cells/s5.rs: type alias names a T7 type: J",
        ),
        (
            "P-A5 use .. as in s5.rs",
            format!("use nexus_mother::adapter::FileJournal as J;\n{made}"),
            "host/tests/route_cells/s5.rs: use .. as renames a T7 type: FileJournal as J",
        ),
        (
            "P-A6 an inline module in s5.rs",
            inline.to_string(),
            "host/tests/route_cells/s5.rs: an inline module m",
        ),
    ] {
        shown(
            &mut misses,
            plant,
            &s5_findings(&format!("{s5}{more}")),
            &[expected, byte],
        );
    }
    // SB3E-R99.2 F1: `file`'s row of the list, parsed, with field `k` set by
    // `to` whatever it holds (a unit may fill its row; no plant reads a literal).
    let set = |file: &str, k: usize, to: &dyn Fn(&str) -> String| {
        let row = |l: &str| {
            let mut fields: Vec<String> = l.split('|').map(String::from).collect();
            if fields[0] == file {
                fields[k] = to(&fields[k]);
            }
            fields.join("|") + "\n"
        };
        S5_SECTIONS.lines().map(row).collect::<String>()
    };
    // SB3E-R99.2 F2, F3, note 4 (Codex's forms): each on a stub whose row lists
    // exactly the plant's imports, so no other finding stands beside the kill.
    let command = concat!("std::", "process::Command");
    let f2a = format!("#[allow(dead_code)]\nfn run_child(program: &str) {{\n    let mut c = <{command}>::new(program);\n}}\n");
    let f2b = format!("#[allow(dead_code)]\nfn wait_child(c: &mut {command}) {{\n    let _ = <{command}>::status(c);\n}}\n");
    let raw = "use super::Entry;\n#[allow(dead_code)]\nfn collect_native(broker: &[Entry]) -> usize {\n    broker.iter().filter(|e| e.point == r\"store.commit\").count()\n}\n";
    let hashes = "use super::Entry;\nfn collect_native(broker: &[Entry]) -> usize {\n    broker.iter().filter(|e| e.point.as_bytes() == br#\"store.commit\"#).count()\n}\n";
    let braced = "use super::Entry;\n#[allow(dead_code, unused_braces)]\nfn collect_native(broker: &[Entry]) -> [usize; { 1 }] {\n    [broker.iter().filter(|e| e.point == \"store.commit\").count()]\n}\n";
    let two = "use {super::Entry};\nuse {super::PATIENCE};\n";
    let qualified = "fn d() {\n    let _ = <Vec<u8>>::new();\n}\n";
    let held = format!("fn d(_: &{command}) {{}}\n");
    let fixed: [(&str, &str, &str, String, &[&str]); 10] = [
        ("F2 Codex's <Command>::new", "s5_reg.rs", "-", f2a, &["a qualified-self path <..>::", "a child path (Command)"]),
        ("F2 Codex's <Command>::status", "s5_reg.rs", "-", f2b, &["a qualified-self path <..>::", "a child path (Command)"]),
        ("F2 a qualified-self path, no as", "s5_reg.rs", "-", qualified.into(), &["a qualified-self path <..>::"]),
        ("F2 Command in another form", "s5_reg.rs", "-", held, &["a child path (Command)"]),
        ("F3a Codex's r\"store.commit\"", "s5_r34.rs", "super::Entry", raw.into(), &["Passive: collect_native reads store.commit"]),
        ("F3a a br#\"store.commit\"#", "s5_r34.rs", "super::Entry", hashes.into(), &["Passive: collect_native reads store.commit"]),
        ("F3b Codex's brace in a return type", "s5_r34.rs", "super::Entry", braced.into(), &["a brace in the signature of fn collect_native"]),
        ("note 4 two braced super imports", "s5_r34.rs", "super::Entry,super::PATIENCE", two.into(), &["more than one super:: import"]),
        ("F2 control: Vec::<u8>::new() is no qualified-self path", "s5_reg.rs", "super::s5::read_registered,super::s5::registered,super::s5::Facts,super::Fixture", "fn d() {\n    let _ = Vec::<u8>::new();\n}\n".into(), &[]),
        ("F3b control: a plain signature", "s5_r34.rs", "super::Entry", "use super::Entry;\nfn collect_native(broker: &[Entry]) -> usize {\n    broker.len()\n}\n".into(), &[]),
    ];
    for (plant, file, imports, text, expected) in fixed {
        let data = set(file, 4, &|_| imports.into());
        let rows = section_rows(&data);
        let row = rows.iter().find(|r| r.file == file).expect(file);
        let got = section_findings(file, row, &(stub(file) + &text), &pins);
        let expected: Vec<String> = expected
            .iter()
            .map(|e| format!("section {file}: {e}"))
            .collect();
        let expected: Vec<&str> = expected.iter().map(String::as_str).collect();
        shown(&mut misses, plant, &got, &expected);
        if expected.is_empty() && !got.is_empty() {
            misses.push(format!("{plant}: {got:?}"));
        }
    }
    // SB3E-R101.2 F2 and F3a (round 2): the ruled plants, the depth rule's own
    // counterexample (a comparison inside `<..>` in a turbofish const block,
    // refused by `openers`), the nearest variants tried, and controls.
    let q = "a qualified-self path <..>::";
    let opener = |p: &str| format!("a qualified-self opener < after `{p}`");
    let (attr, fored) = (opener("]"), opener("for"));
    let (eq, brace, lt, life, ret) = (
        opener("="),
        opener("{"),
        opener("<"),
        opener("a"),
        opener("return"),
    );
    let commit = "Passive: collect_native reads store.commit";
    let collect = |value: &str| {
        format!("use super::Entry;\nfn collect_native(broker: &[Entry]) -> usize {{\n    broker.iter().filter(|e| e.point == {value}).count()\n}}\n")
    };
    let round2: [(&str, &str, &str, String, Vec<&str>); 26] = [
        ("R2 F2 <[u8; 0]>::default()", "s5_reg.rs", "-", "fn d() {\n    let _ = <[u8; 0]>::default();\n}\n".into(), vec![q, &eq]),
        ("R2 F2 <[u8; { 0 }]>::default()", "s5_reg.rs", "-", "fn d() {\n    let _ = <[u8; { 0 }]>::default();\n}\n".into(), vec![q, &eq]),
        ("R2 F2 <Vec<[u8; 0]>>::new()", "s5_reg.rs", "-", "fn d() {\n    let _ = <Vec<[u8; 0]>>::new();\n}\n".into(), vec![q, &eq]),
        ("R2 F2 a comparison inside <..> in a turbofish block", "s5_reg.rs", "-", "fn d() {\n    let _ = f::<{ <[u8; { (2 > 1) as usize }]>::default().len() }>();\n}\n".into(), vec![&brace]),
        ("R2 F2 a nested <<..> as ..>:: in a turbofish", "s5_reg.rs", "-", "fn d() {\n    let _ = f::<<[u8; { (2 > 1) as usize }] as Default>::Output>();\n}\n".into(), vec![&lt]),
        ("R2 F2 a >:: with no <", "s5_reg.rs", "-", "fn d(a: u8) -> bool {\n    a > ::core::primitive::u8::MAX\n}\n".into(), vec![q]),
        ("R2 F2 after a lifetime", "s5_reg.rs", "-", "fn d<'a>(_: &'a <Vec<u8> as IntoIterator>::Item) {}\n".into(), vec![q, &life]),
        ("R2 F2 after return", "s5_reg.rs", "-", "fn d() -> usize {\n    return <[u8]>::len(&[]);\n}\n".into(), vec![q, &ret]),
        ("R2 F2 after an attribute, in a turbofish block", "s5_reg.rs", "-", "fn d() {\n    let _ = f::<{ #[allow(unused)] <[[u8; { (2 > 1) as usize }]]>::len(&[]); 0 }>();\n}\n".into(), vec![&attr]),
        ("R2 F2 after for, in a turbofish block", "s5_reg.rs", "-", "fn d() {\n    let _ = f::<{ for <[u8; { (2 > 1) as usize }]>::C in [] {} 0 }>();\n}\n".into(), vec![&fored]),
        ("R2 F2 control: a[0] < b", "s5_reg.rs", "super::s5::read_registered,super::s5::registered,super::s5::Facts,super::Fixture", "fn d(a: [u8; 1], b: u8) -> bool {\n    a[0] < b\n}\n".into(), vec![]),
        ("R2 F2 control: Vec::<[u8; 0]>::new()", "s5_reg.rs", "super::s5::read_registered,super::s5::registered,super::s5::Facts,super::Fixture", "fn d() {\n    let _ = Vec::<[u8; 0]>::new();\n}\n".into(), vec![]),
        ("R2 F2 control: a < b, a > b, a >= b, a <= b", "s5_reg.rs", "super::s5::read_registered,super::s5::registered,super::s5::Facts,super::Fixture", "fn d(a: u8, b: u8) -> bool {\n    a < b || a > b || a >= b || a <= b\n}\n".into(), vec![]),
        ("R2 F2 control: -> and => and >>", "s5_reg.rs", "super::s5::read_registered,super::s5::registered,super::s5::Facts,super::Fixture", "fn d(a: u8) -> u8 {\n    let f = |x: u8| -> u8 { x >> 1 };\n    match a {\n        _ => f(a),\n    }\n}\n".into(), vec![]),
        ("R2 F2 control: generics and turbofish >>", "s5_reg.rs", "super::s5::read_registered,super::s5::registered,super::s5::Facts,super::Fixture", "fn g<T: Into<u8>>(t: T) -> Vec<u8> {\n    let v: Vec<Vec<u8>> = Vec::<Vec<u8>>::new();\n    let _ = v.into_iter().collect::<Vec<Vec<u8>>>();\n    vec![t.into()]\n}\n".into(), vec![]),
        ("R2 F2 control: for<'a>", "s5_reg.rs", "super::s5::read_registered,super::s5::registered,super::s5::Facts,super::Fixture", "fn h<F>(_: F)\nwhere\n    F: for<'a> Fn(&'a u8),\n{\n}\n".into(), vec![]),
        ("R2 F3a \"store.\\x63ommit\"", "s5_r34.rs", "super::Entry", collect(r#""store.\x63ommit""#), vec![commit]),
        ("R2 F3a \"store.\\u{63}ommit\"", "s5_r34.rs", "super::Entry", collect(r#""store.\u{63}ommit""#), vec![commit]),
        ("R2 F3a a backslash-newline", "s5_r34.rs", "super::Entry", collect("\"store.\\\n        commit\""), vec![commit]),
        ("R2 F3a b\"store.\\x63ommit\"", "s5_r34.rs", "super::Entry", collect(r#"std::str::from_utf8(b"store.\x63ommit").unwrap()"#), vec![commit]),
        ("R2 F3a \"store\\x2ecommi\\u{0_074}\"", "s5_r34.rs", "super::Entry", collect(r#""store\x2ecommi\u{0_074}""#), vec![commit]),
        ("R2 F3a a backslash-CRLF the decoder refuses", "s5_r34.rs", "super::Entry", collect("\"store.\\\r\n        commit\""), vec!["Passive: collect_native has a literal that does not decode"]),
        ("R2 F3a \"store.\\q\" (the lexer refuses)", "s5_r34.rs", "super::Entry", collect(r#""store.\q""#), vec!["lex: host/tests/route_cells/s5_r34.rs does not tokenize"]),
        ("R2 F3a an unterminated \\u{ (the lexer refuses)", "s5_r34.rs", "super::Entry", collect(r#""store.\u{63ommit""#), vec!["lex: host/tests/route_cells/s5_r34.rs does not tokenize"]),
        ("R2 F3a control: \"store.commi\"", "s5_r34.rs", "super::Entry", collect(r#""store.commi""#), vec![]),
        ("R2 F3a control: \"a\\tb\\n\"", "s5_r34.rs", "super::Entry", collect(r#""a\tb\n""#), vec![]),
    ];
    for (plant, file, imports, text, expected) in round2 {
        let data = set(file, 4, &|_| imports.into());
        let rows = section_rows(&data);
        let row = rows.iter().find(|r| r.file == file).expect(file);
        let got = section_findings(file, row, &(stub(file) + &text), &pins);
        let named = |e: &&str| match e.starts_with("lex: ") {
            true => e.to_string(),
            false => format!("section {file}: {e}"),
        };
        let expected: Vec<String> = expected.iter().map(named).collect();
        let expected: Vec<&str> = expected.iter().map(String::as_str).collect();
        shown(&mut misses, plant, &got, &expected);
        if expected.is_empty() && !got.is_empty() {
            misses.push(format!("{plant}: {got:?}"));
        }
    }
    // `lex` gives one token per punctuation character (R101.2's first check).
    let split = lex("a -> b => c >= d <= e >> f << g");
    let want = [
        "a", "-", ">", "b", "=", ">", "c", ">", "=", "d", "<", "=", "e", ">", ">", "f", "<", "<",
        "g",
    ];
    if split != want {
        misses.push(format!("lex splits operators as {split:?}"));
    }
    // The list itself.
    for (plant, data, expected) in [
        (
            "P-R1 a malformed row",
            format!("{S5_SECTIONS}s5_x.rs|U1\n"),
            "S5 sections: a malformed row \"s5_x.rs|U1\"",
        ),
        (
            "P-R2 a row importing a composed type",
            set("s5_close.rs", 4, &|_| {
                "nexus_mother::adapter::FileJournal".into()
            }),
            "S5 sections: the row s5_close.rs imports nexus_mother::adapter::FileJournal",
        ),
        (
            "P-R3 a pair in two rows",
            set("s5_r34.rs", 2, &|pairs| format!("seal:lifecycle,{pairs}")),
            "S5 sections: seal:lifecycle is in two rows",
        ),
    ] {
        if data == S5_SECTIONS {
            misses.push(format!("{plant}: the list is unchanged"));
        }
        shown(
            &mut misses,
            plant,
            &sections_with(ROUTE_CELLS, &data, &sources, &cells),
            &[expected],
        );
    }
    found.extend(misses);
    fail_on(found);
}

/// SB3E-R15.2's sweep over Mother's pins, every mutant failing its region's
/// pin, and G-S2's exact count: a region shrunk by its last line still pins,
/// with fewer mutants, so the count must change.
#[test]
fn sb3e_mother_pins_reject_every_generated_mutant() {
    let (sources, pins) = (sources(), pins());
    let (counts, mut found) = sweep(&sources, &pins);
    let mutants: usize = counts.iter().sum();
    println!(
        "U4a-3 sweep: {} regions, {mutants} mutants, {found:?} survived",
        pins.len()
    );
    if mutants != MUTANTS {
        found.push(format!("G-S2: {mutants} mutants, not {MUTANTS}"));
    }
    // The plants below are built from the reviewed text; on a changed tree
    // its findings are reported first, whole, so no plant's own anchor check
    // can mask them (SB3E-R42.6).
    if !found.is_empty() {
        return fail_on(found);
    }
    let (file, pin) = pins[0];
    let shrunk = pin.trim_end().rsplit_once('\n').map_or("", |s| s.0);
    let (less, _) = sweep(&sources, &[(file, shrunk)]);
    if pinned(text(&sources, file), shrunk) != 1 || mutants - counts[0] + less[0] == MUTANTS {
        found.push("G-S2 a shrunk pin: the count kept".into());
    }
    fail_on(found);
}

/// W0b B3 and B4 (SB3E-R95.4, R95.7, R106.3, R106.4): on the tree as it is,
/// `s5.rs`'s surface is `S5_EXPORTS`; each plant then shows its own finding
/// through `s5_findings` or the list check; and a section using the section
/// exports (the pause helper, a first-arrival chain, the by-value
/// `FileSession`) passes the section profile.
#[test]
fn sb3e_w0b_s5_exports() {
    let sources = sources();
    let s5 = text(&sources, S5);
    let found = exports_findings(s5);
    if !found.is_empty() {
        return fail_on(found);
    }
    let swap = |text: &str, old: &str, new: &str| {
        assert_eq!(text.matches(old).count(), 1, "{old}");
        text.replace(old, new)
    };
    let mut misses = Vec::new();
    let hold = "pub(super) fn hold(point: &'static str) -> s5::Held {\n    let (cell, timeout) = (*RUNNING.lock().unwrap(), timeout as s5::Timeout);";
    let callback = "pub(super) fn hold(point: &'static str, timeout: s5::Timeout) -> s5::Held {\n    let cell = \"u5\";";
    let region = "pin host/tests/route_cells/s5.rs: pub(super)fnhold(point";
    let journal = "pub(super) fn journal(f: &Fixture) -> FileJournal {\n    FileJournal::open(&f.0).unwrap()\n}\n";
    let broker = "    let (broker, request) = (s1::drain_trace(), 0);\n    let session = b.o.session.identity();\n";
    let plants = [
        (
            "B4 an unlisted pub(super)",
            format!("{s5}pub(super) fn spare() {{}}\n"),
            "S5_EXPORTS: unlisted fn spare",
        ),
        (
            "B4 a listed export removed",
            swap(s5, "pub(super) fn collect_broker(", "fn collect_broker("),
            "S5_EXPORTS: missing fn collect_broker",
        ),
        (
            "B4 an exported constructor",
            format!("{s5}{journal}"),
            "S5_EXPORTS: fn journal hands out a composed type FileJournal",
        ),
        (
            "B4 the pause helper removed",
            swap(s5, "pub(super) fn hold(", "fn hold("),
            "S5_EXPORTS: missing fn hold",
        ),
        (
            "B4 timeout exported",
            swap(
                s5,
                "fn timeout(line: &str) -> ! {",
                "pub(super) fn timeout(line: &str) -> ! {",
            ),
            "S5_EXPORTS: unlisted fn timeout",
        ),
        (
            "B4 the helper given a callback",
            swap(s5, hold, callback),
            region,
        ),
        (
            "B4 the helper not passing timeout",
            swap(
                s5,
                "timeout as s5::Timeout);\n    s5::arm(point",
                "abort as s5::Timeout);\n    s5::arm(point",
            ),
            region,
        ),
        (
            "B4 an exported alias of a T7 type",
            format!("{s5}pub(super) type J = FileJournal;\n"),
            "host/tests/route_cells/s5.rs: type alias names a T7 type: J",
        ),
        (
            "B4 the by-value FileSession removed",
            swap(
                s5,
                "    pub(super) m: FileSession<SessionPort>,",
                "    m: FileSession<SessionPort>,",
            ),
            "S5_EXPORTS: missing struct Mother{m}",
        ),
        (
            "B3 collect_reg reads journal()",
            swap(
                s5,
                broker,
                &format!("{broker}    let _ = b.o.session.journal();\n"),
            ),
            "Passive: collect_reg reads the journal through journal(), not journal_view()",
        ),
        (
            "B3 collect_reg reads store.commit",
            swap(
                s5,
                broker,
                &format!(
                    "{broker}    let _ = broker.iter().any(|e| e.point == \"store.commit\");\n"
                ),
            ),
            "Passive: collect_reg reads store.commit",
        ),
    ];
    for (plant, text, expected) in plants {
        shown(&mut misses, plant, &s5_findings(&text), &[expected]);
    }
    // R106.3: a section row importing an `h` row (`lanes`) or the module.
    for (import, expected) in [
        ("super::s5::lanes", "S5 sections: the row s5_close.rs imports super::s5::lanes, not a section export of s5.rs"),
        ("super::s5", "S5 sections: the row s5_close.rs imports super::s5, not a section export of s5.rs"),
    ] {
        let data: Vec<String> = S5_SECTIONS
            .lines()
            .map(|l| match l.starts_with("s5_close.rs|") {
                true => {
                    let mut f: Vec<&str> = l.split('|').collect();
                    f[4] = import;
                    f.join("|")
                }
                false => l.to_string(),
            })
            .collect();
        let data = data.join("\n") + "\n";
        let plant = format!("B4 a section row importing {import}");
        shown(&mut misses, &plant, &rows_check(&data, &section_rows(&data)), &[expected]);
    }
    // A section using the section exports, its row listing exactly its imports.
    let sample = "//! SB-3E S5 section U6b-2: s5_r34.rs\nuse super::{s5::{driven, hold, start, Anchor, Chain, Facts, REFUSED}, Fixture, PATIENCE};\nconst SLOT: Chain = Chain {\n    row: (\"call_slot_reserve\", \"ingress\"),\n    anchor: Anchor::Arrival(1),\n    points: &[\"attempt.slot\"],\n};\npub(super) fn quit(f: &Fixture) -> Facts {\n    let (facts, _, ()) = driven(f, \"quit\", &SLOT, &REFUSED, |_, m, held, _| {\n        let _ = m.mission();\n        drop(held);\n    });\n    facts\n}\npub(super) fn ending(f: &Fixture) -> bool {\n    let held = hold(\"mother.settle\");\n    let _ = held.acknowledged.recv_timeout(PATIENCE);\n    let session = start(f).mother.m;\n    session.prepare_session_end().is_ok()\n}\n";
    let imports = "super::s5::driven,super::s5::hold,super::s5::start,super::s5::Anchor,super::s5::Chain,super::s5::Facts,super::s5::REFUSED,super::Fixture,super::PATIENCE";
    let rows = section_rows(S5_SECTIONS);
    let r34 = rows
        .iter()
        .find(|r| r.file == "s5_r34.rs")
        .expect("s5_r34.rs");
    let row = Row {
        file: r34.file,
        unit: r34.unit,
        pairs: r34.pairs.clone(),
        controls: r34.controls.clone(),
        imports: list(imports),
        helpers: Vec::new(),
    };
    let mut got = section_findings("s5_r34.rs", &row, sample, &pins());
    let data = S5_SECTIONS.replace(
        "ctl_meter_settle|-|",
        &format!("ctl_meter_settle|{imports}|"),
    );
    got.extend(rows_check(&data, &section_rows(&data)));
    if !got.is_empty() {
        misses.push(format!("a section using the exports: {got:?}"));
    }
    fail_on(misses);
}

/// W0b B5 (SB3E-R94.2): on the tree as it is the compact arms verify; then
/// each mismatch, cardinality and placement plant shows its own finding,
/// through `macro_findings` or, for the pinned region and the items,
/// `inventory_findings` (each is also a `mguard` row).
#[test]
fn sb3e_w0b_compact_arms() {
    let sources = sources();
    let main = text(&sources, &format!("{CELLS}main.rs"));
    let cells = text(&sources, &format!("{CELLS}cells.rs"));
    let found = macro_findings(main, cells);
    if !found.is_empty() {
        return fail_on(found);
    }
    let swap = |text: &str, old: &str, new: &str| {
        assert_eq!(text.matches(old).count(), 1, "{old}");
        text.replace(old, new)
    };
    let mut misses = Vec::new();
    let quit = "    (r5 call_chat ingress quit; $($rest:tt)*) => { cell!(rc_r5_call_chat_ingress_quit, r5 call_chat ingress quit, s5::quit); cells! { $($rest)* } };\n";
    let arm = |body: &str| {
        swap(
            main,
            quit,
            &format!("    (r5 call_chat ingress quit; $($rest:tt)*) => {{ {body} }};\n"),
        )
    };
    let terminal = "    () => { ctl!(ctl_call_chat, call_chat ingress, s5::ctl_call_chat); ctl!(ctl_close_obligations, close_obligations lifecycle, s5_close::ctl_close_obligations); ctl!(ctl_delivery, delivery lifecycle, s5_close::ctl_delivery); ctl!(ctl_mother_end, mother_end lifecycle, s5_close::ctl_mother_end); ctl!(ctl_primary_end, primary_end lifecycle, s5_close::ctl_primary_end); ctl!(ctl_producer_join, producer_join lifecycle, s5_close::ctl_producer_join); ctl!(ctl_seal, seal lifecycle, s5_close::ctl_seal); ctl!(ctl_registration_record, registration_record ingress, s5_reg::ctl_registration_record); ctl!(ctl_close_restrict, close_restrict lifecycle, s5_r6::ctl_close_restrict); ctl!(ctl_emergency_restrict, emergency_restrict lifecycle, s5_r6::ctl_emergency_restrict); ctl!(ctl_registration_reserve, registration_reserve ingress, s5_reg::ctl_registration_reserve); ctl!(ctl_registration_readiness, registration_readiness ingress, s5_reg::ctl_registration_readiness); ctl!(ctl_close_report, close_report lifecycle, s5_report::ctl_close_report); };\n";
    let again = "cells! { $($rest)* }";
    let slug = "U5 macro: the arm rc_r5_call_chat_ingress_quit";
    let other = "    (r5 call_chat ingress read_failure; $($rest:tt)*) => { cell!(rc_r5_call_chat_ingress_read_failure, r5 call_chat ingress read_failure, s5::read_failure); cells! { $($rest)* } };\n";
    for (plant, text, expected) in [
        ("B5 a test name that is not its slug", arm(&format!("cell!(rc_r5_call_chat_ingress_fatal, r5 call_chat ingress quit, s5::quit); {again}")), format!("{slug} defines [\"rc_r5_call_chat_ingress_fatal\"]")),
        ("B5 helper literals not its pattern's", arm(&format!("cell!(rc_r5_call_chat_ingress_quit, r5 call_chat ingress fatal, s5::quit); {again}")), format!("{slug} passes rc_r5_call_chat_ingress_quit,r5call_chatingressfatal,s5::quit, not its own literals")),
        ("B5 another cell's driver", arm(&format!("cell!(rc_r5_call_chat_ingress_quit, r5 call_chat ingress quit, s5::fatal); {again}")), "U5 macro: the driver s5::fatal is named by two arms".into()),
        ("B5 a driver in a section not owning its pair", arm(&format!("cell!(rc_r5_call_chat_ingress_quit, r5 call_chat ingress quit, s5_r34::quit); {again}")), "S5 sections: the arm rc_r5_call_chat_ingress_quit calls s5_r34, which does not own call_chat:ingress".into()),
        ("B5 a compare of another row", arm(&format!("cell!(rc_r5_call_chat_ingress_quit, r5 call_chat lifecycle quit, s5::quit); {again}")), format!("{slug} passes rc_r5_call_chat_ingress_quit,r5call_chatlifecyclequit,s5::quit, not its own literals")),
        ("B5 an arm with no compare", arm(&format!("#[test] fn rc_r5_call_chat_ingress_quit() {{ let _ = super::s5::quit; }} {again}")), format!("{slug} has 0 compares")),
        ("B5 an arm with no manifest entry", swap(main, quit, &format!("{quit}{other}")), "U5 macro: the arm rc_r5_call_chat_ingress_read_failure has no manifest entry".into()),
        ("U9a F-B a test name keeping its value's case", swap(main, quit, &format!("{quit}{}", other.replace("ingress read_failure", "ingress Read_failure").replace("ingress_read_failure,", "ingress_Read_failure,"))), "U5 macro: the arm rc_r5_call_chat_ingress_read_failure defines [\"rc_r5_call_chat_ingress_Read_failure\"]".into()),
        ("B5 a control listed twice", swap(main, terminal, "    () => { ctl!(ctl_call_chat, call_chat ingress, s5::ctl_call_chat); ctl!(ctl_call_chat, call_chat ingress, s5::ctl_call_chat); };\n"), "U5 macro: the control ctl_call_chat appears twice".into()),
        ("B5 a control in no section row", swap(main, terminal, "    () => { ctl!(ctl_other, call_chat ingress, s5::ctl_call_chat); };\n"), "S5 sections: the control ctl_other is listed by no section".into()),
        ("C1 a control of the old shape", swap(main, terminal, "    () => { ctl!(ctl_call_chat, s5::ctl_call_chat); };\n"), "U5 macro: the control ctl_call_chat,s5::ctl_call_chat is not ctl!(<control>, <sink> <dim>, <stem>::<fn>)".into()),
        ("C1 a control named for another sink", swap(main, terminal, "    () => { ctl!(ctl_call_chat, seal lifecycle, s5::ctl_call_chat); };\n"), "S5 sections: the control ctl_call_chat is not ctl_<sink> of seal:lifecycle, owned by its section".into()),
        ("C1 a pair its section does not own", swap(main, terminal, "    () => { ctl!(ctl_call_chat, call_chat fault, s5::ctl_call_chat); };\n"), "S5 sections: the control ctl_call_chat is not ctl_<sink> of call_chat:fault, owned by its section".into()),
        ("B5 a duplicate arm", swap(main, quit, &format!("{quit}{quit}")), format!("{slug} appears twice")),
        ("B5 an arm defining no test", arm(again), format!("{slug} defines []")),
        ("B5 an arm with zero drivers", arm(&format!("cell!(rc_r5_call_chat_ingress_quit, r5 call_chat ingress quit); {again}")), format!("{slug} has 0 drivers")),
        ("B5 an arm with two drivers", arm(&format!("cell!(rc_r5_call_chat_ingress_quit, r5 call_chat ingress quit, s5::quit, s5::stop); {again}")), format!("{slug} has 2 drivers")),
        ("B5 an arm with two compares", arm(&format!("cell!(rc_r5_call_chat_ingress_quit, r5 call_chat ingress quit, s5::quit); let _ = super::witness::control; {again}")), format!("{slug} has 2 compares")),
    ] {
        shown(&mut misses, plant, &macro_findings(&text, cells), &[&expected]);
    }
    // W0b N7: a `macro_rules! cell` planted in cells.rs.
    let in_cells = format!("{cells}macro_rules! cell {{\n    () => {{}};\n}}\n");
    let gb1 = ["cells.rs G-B1: a macro definition macro_rules"];
    shown(
        &mut misses,
        "N7 a macro in cells.rs",
        &macro_findings(main, &in_cells),
        &gb1,
    );
    // R102.3: the nearest variants of the new checks, each refused by class.
    for (plant, text, expected) in [
        ("self-attack a raw-identifier driver", arm(&format!("cell!(rc_r5_call_chat_ingress_quit, r5 call_chat ingress quit, s5::r#fatal); {again}")), "U5 macro: a # in macro_rules! cells".to_string()),
        ("self-attack an attribute in an arm", arm(&format!("#[allow(unused)] cell!(rc_r5_call_chat_ingress_quit, r5 call_chat ingress quit, s5::quit); {again}")), "U5 macro: a # in macro_rules! cells".into()),
        ("self-attack a cell! with an extra argument", arm(&format!("cell!(rc_r5_call_chat_ingress_quit, r5 call_chat ingress quit, s5::quit, x); {again}")), format!("{slug} is not cell!(<slug>, <route> <sink> <dim> <value>, <stem>::<fn>)")),
        ("self-attack a bracketed cell!", arm(&format!("cell![rc_r5_call_chat_ingress_quit, r5 call_chat ingress quit, s5::quit]; {again}")), format!("{slug} defines []")),
        ("self-attack a ctl! with no driver", swap(main, terminal, "    () => { ctl!(ctl_call_chat); };\n"), "U5 macro: the control ctl_call_chat is not ctl!(<control>, <sink> <dim>, <stem>::<fn>)".into()),
        ("self-attack a control sharing a cell's driver", swap(main, terminal, "    () => { ctl!(ctl_call_chat, call_chat ingress, s5::quit); };\n"), "U5 macro: the driver s5::quit is named by two arms".into()),
    ] {
        shown(&mut misses, plant, &macro_findings(&text, cells), &[&expected]);
    }
    let mains = format!("{CELLS}main.rs");
    let ctl = main[main.find("/// W0b B5, C1: one control's test").unwrap()
        ..main
            .find("/// SB3E-R86.4 (mission Behaviour 1), W0b B5")
            .unwrap()]
        .to_string();
    let moved = swap(main, &ctl, "").replace(
        "// The manifest's text form",
        &format!("{ctl}// The manifest's text form"),
    );
    let label = "pin host/tests/route_cells/main.rs: macro_rules!cell{($slug: matched 0";
    for (plant, text, expected) in [
        ("B5 a helper moved outside the region", moved, label),
        ("B5 the helper's body changed", swap(main, "                super::witness::control(&ran, stringify!($sink), stringify!($dim));\n", "                let _ = &ran;\n"), label),
        ("B5 a third top-level helper", swap(main, "/// W0b B5 (SB3E-R94.2): one credited cell's test", "macro_rules! extra {\n    () => {};\n}\n/// W0b B5 (SB3E-R94.2): one credited cell's test"), "R44.2 main.rs items: unlisted macro_rules!extra"),
    ] {
        let planted = planted(&sources, &mains, main, &text);
        shown(&mut misses, plant, &inventory_findings(&planted), &[expected]);
    }
    fail_on(misses);
}

/// U20a (SB3E-R214): each clause of `close_caller_findings` refuses its own
/// planted text, and the real tree passes. A commented-out call, a string, a
/// `#[cfg(test)]` item or module, a test, example or legacy file are no
/// production callers.
#[test]
fn sb3e_u20a_close_callers() {
    let sources = walked().0;
    let real = close_caller_findings(&sources);
    let mut misses = match real.is_empty() {
        true => Vec::new(),
        false => vec![format!("the real tree: {real:?}")],
    };
    let (h, run) = (
        "host/src/hands.rs",
        "    let request = screen.input().quit.take();\n",
    );
    let uses = "use crate::session_home::SessionServices;\n";
    let call = "fn f(i: &I) {\n    i.emergency();\n}\n";
    let edit = |file: &str, old: &str, new: &str| planted(&sources, file, old, new);
    let at = |old: &str, line: &str| edit(h, old, &format!("{line}\n{old}"));
    let add = |files: &[(&str, &str)], mut s: Vec<(String, String)>| {
        s.extend(files.iter().map(|(f, t)| (f.to_string(), t.to_string())));
        s
    };
    let report = "        let model = self.home.session.close_obligations();\n";
    let reader = "    let reader_done = terminal.stop_reader(producers_until);\n";
    let bin = (
        "host/src/bin/mother-hands.rs",
        "use std::process::ExitCode;\n",
    );
    for (planted, expected) in [
        (
            at(run, "let _ = session.close_obligations();"),
            "(a): unlisted host/src/hands.rs|run|close_obligations",
        ),
        (
            edit(h, report, "self.home.session.obligations();\n"),
            "(a): missing host/src/hands.rs|close_report|close_obligations",
        ),
        (
            at(run, "let _ = ingress.emergency();"),
            "(b): unlisted host/src/hands.rs|run|emergency",
        ),
        (
            add(
                &[("host/src/x.rs", "fn q(i: &I) { i.accept(k, 9); }")],
                sources.clone(),
            ),
            "(b): unlisted host/src/x.rs|q|accept",
        ),
        (
            at(run, "screen.stop.press();"),
            "(c): unlisted host/src/hands.rs|run|press",
        ),
        (
            edit(h, reader, "let reader_done = true;\n"),
            "(d): order: the reader is not joined before hands-close spawns",
        ),
        (
            at(uses, "use crate::legacy_terminal;"),
            "(e): host/src/hands.rs uses legacy_terminal",
        ),
        (
            edit(bin.0, bin.1, &format!("use x::legacy_stop;\n{}", bin.1)),
            "(e): host/src/bin/mother-hands.rs uses legacy_stop",
        ),
    ] {
        let expected = format!("close callers {expected}");
        shown(
            &mut misses,
            &expected,
            &close_caller_findings(&planted),
            &[&expected],
        );
    }
    let lib = edit(
        "host/src/lib.rs",
        "mod stop;\n",
        "mod stop;\n#[cfg(test)]\nmod u20a_t;\n",
    );
    for (plant, planted) in [
        (
            "a comment",
            at(run, "// let _ = session.close_obligations();"),
        ),
        (
            "a string",
            at(run, "let _ = \"session.close_obligations()\";"),
        ),
        (
            "a cfg(test) module",
            at(uses, &format!("#[cfg(test)]\nmod u20a {{\n{call}}}")),
        ),
        ("a cfg(test) fn", at(uses, &format!("#[cfg(test)]\n{call}"))),
        (
            "a test file",
            add(&[("host/src/u20a_tests.rs", call)], sources.clone()),
        ),
        (
            "an integration test",
            add(&[("host/tests/u20a.rs", call)], sources.clone()),
        ),
        (
            "an example",
            add(&[("host/examples/u20a.rs", call)], sources.clone()),
        ),
        (
            "a legacy module",
            add(&[("host/src/legacy_u20a.rs", call)], sources.clone()),
        ),
        (
            "a cfg(test)-declared module",
            add(&[("host/src/u20a_t.rs", call)], lib),
        ),
    ] {
        let found = close_caller_findings(&planted);
        if found != real {
            misses.push(format!("{plant} counted as a production caller: {found:?}"));
        }
    }
    fail_on(misses);
}

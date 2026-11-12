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
    // Built from parts: `c1_s3_record_seam_is_test_only` counts the name.
    ("src/adapter/mod.rs", concat!("model_", "record")),
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
const PINS_SHA256: &str = "88064bcd652fb3ff878b3a455ae9dc7b0342a81ccba3a2f860c36307d9fc7359";
const MUTANTS: usize = 617;
const REGIONS: usize = 29;
/// SB3E-R51.1: every file of the harness directory, byte for byte, exactly
/// this list (`name sha256`): the authority for all of `route_cells`, `s5.rs`
/// included (R44.1). A reviewed commit that changes a file changes its row.
/// The `MAIN_ITEMS`, `IMPLS` and `main.rs`/`store.rs` region pins stay as an
/// advisory layer.
const CELLS: &str = "host/tests/route_cells/";
const ROUTE_CELLS: &str = "\
cells.rs 0535a325de11a0121e89443e81001f0c83273b86a2b90328784d1b2c46d8d4f6
grant.rs fbe4c4b9dc43dc86c77ed4ec9c09f8bf9ea61dc8091b8ed1866a847a99e27be4
locks.rs 9b82f75d8ad4f2d0c47194c69be15187dd3f96291033b3a57a4105bc731ccf78
main.rs 8cdb6029434f3121d8844bf8ec1ca8ecaf3d8d236e827f6f44f1c840147c8694
route_fsync.c 6166fe8b9eee2e9c2a84a8bcbfcc1590f14de97d7bb0d60084da674a56f0e973
s5.rs 54bc7c29fd123056eebed2f46b31d5fcfa8dde919c795992c58e7bac7f628319
store.rs 0dabb6132cfc184a99b4627b983226cdef3390785c8e60daa189bba5bc737b00
";
fn route_cells() -> impl Iterator<Item = (&'static str, &'static str)> {
    ROUTE_CELLS.lines().filter_map(|l| l.split_once(' '))
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
mods5;\n\
modstore;\n\
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
host/tests/route_cells/store.rs|impl<T>Faulted<T>\n\
";
/// SB3E-R42.4 (G-S3): `s5.rs`'s one import from its parent, by name.
const S5_SUPER: [&str; 9] = [
    "child",
    "fake_ollama",
    "spec",
    "Fixture",
    "Opened",
    "CHILD",
    "DIGEST",
    "NAME",
    "PATIENCE",
];
/// SB3E-R42.1 (G-B1): every macro `s5.rs` may invoke.
const S5_MACROS: [&str; 8] = [
    "assert",
    "assert_eq",
    "assert_ne",
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
        .filter(|(f, _)| !exempt(f) || f == "src/route_seams.rs");
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
            (!exempt(f) || f == "src/route_seams.rs") && s.contains(&quoted)
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
    for (_, source) in sources.iter().filter(|(f, _)| !exempt(f)) {
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
    found
}
/// SB3E-R51.1: each harness file not listed, missing, or not its digest.
fn cells_findings(files: &[(String, Vec<u8>)]) -> Vec<String> {
    use sha2::Digest;
    let pin = |f: &str| route_cells().find(|r| Some(r.0) == f.strip_prefix(CELLS));
    let mut found = Vec::new();
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
    let mut sources = crate::adapter::scan::mother_sources();
    sources.sort(); // By path text, as the inventory is listed.
    sources
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
        let next = |k: usize| tokens.get(i + k).copied().unwrap_or("");
        let op = next(1).len() == 1 && "+-*/%|&^".contains(next(1)) && next(2) == "=";
        let shift = ["<", ">"].contains(&next(1)) && next(2) == next(1) && next(3) == "=";
        let set = next(1) == "=" && !["=", ">"].contains(&next(2));
        let assigned = tokens[i] == "calls" && (set || op || shift);
        let write = WRITES.contains(&tokens[i]) || tokens[i].starts_with("fetch_");
        if write || assigned {
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
    let b = code.as_bytes();
    let word = |p: usize| {
        b.get(p)
            .is_some_and(|c| c.is_ascii_alphanumeric() || *c == b'_')
    };
    for (p, _) in code.match_indices("r#") {
        if word(p + 2) && !p.checked_sub(1).is_some_and(word) {
            let name = code[p + 2..].split(|c: char| !c.is_alphanumeric() && c != '_');
            let name = name.into_iter().next().unwrap_or("");
            check(false, format!("G-B1: a raw identifier r#{name}"));
        }
    }
    for (i, t) in tokens.iter().enumerate() {
        if ["macro_rules", "macro"].contains(t) {
            check(false, format!("G-B1: a macro definition {t}"));
        }
        let next = |k: usize| tokens.get(i + k).copied().unwrap_or("");
        let invoked = next(1) == "!" && ["(", "[", "{"].contains(&next(2));
        let ident = t.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_');
        if ident && invoked && !S5_MACROS.contains(t) {
            check(false, format!("G-B1: an unlisted macro {t}!"));
        }
    }
    // (b) no other route to the broker's model call; no re-exported handle.
    for id in [
        "model_call",
        "call_model",
        "model_call_private",
        "EffectPort",
        "ModelCaller",
    ] {
        check(n(&[id]) == 0, format!("(b) {id}"));
    }
    let reexport = tokens.windows(2).any(|w| w[0] == "pub" && w[1] == "use");
    let scoped = tokens
        .windows(5)
        .any(|w| w[0] == "pub" && w[1] == "(" && w[4] == "use");
    check(!reexport && !scoped, "(b) no re-export".into());
    // (c) the port is made once, inside `Coordinator::open`, and never kept.
    let open = n(&["Coordinator", ":", ":", "open", "("]) == 1;
    let open = s5.find("Coordinator::open(").filter(|_| open);
    let open = open.map_or(0..0, |o| o..closing(&code, o + "Coordinator::open".len()));
    let made =
        (0..tokens.len()).filter(|&i| tokens[i..].starts_with(&["SessionPort", ":", ":", "new"]));
    let made: Vec<usize> = made.map(|i| at(tokens[i])).collect();
    let inside = made.len() == 1 && made.iter().all(|p| open.contains(p));
    check(inside, "(c) the port made outside Coordinator::open".into());
    let ports = port_uses(s5);
    check(
        ports == PORT_USES.lines().collect::<Vec<_>>(),
        format!("(c) port uses {ports:?}"),
    );
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
        let next = |k: usize| tokens.get(i + k).copied().unwrap_or("");
        let method = i > 0 && tokens[i - 1] == "." && next(1) == "(";
        let called = ["output", "status", "wait"].contains(t) || *t == "spawn" && next(2) == ")";
        let path = *t == "Command" && next(1) == ":" && next(3) != "new";
        let waits =
            method && called || path || ["try_wait", "wait_with_output", "kill"].contains(t);
        if waits && !bound.contains(&at(tokens[i])) {
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
    let unwinds = [
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
    for t in &body {
        if unwinds.contains(t) || t.starts_with("debug_assert") {
            check(false, format!("timeout: a panic path {t}"));
        }
    }
    let tail = ["std", ":", ":", "process", ":", ":", "abort", "(", ")", "}"];
    check(body.ends_with(&tail), "timeout: ends in abort".into());
    found
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
    let (sources, cells) = (sources(), crate::adapter::scan::route_cells_files());
    let mut found = inventory_findings(&sources);
    found.extend(pins_findings(PIN_DATA));
    found.extend(cells_findings(&cells));
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
    for (plant, files, expected) in [
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

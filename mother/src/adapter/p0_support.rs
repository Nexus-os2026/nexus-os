//! P0-A (design §5.4, oracles first): the canaries, the privacy scanner and
//! a file-based test world for the P0 privacy oracles.
//!
//! Registered with the oracles (commit 1), against main's API. Only the
//! bodies marked `HELPER (P0 API)` may change in a later commit, and only to
//! follow an API change (the coordinator's private store); the scanner,
//! the canaries and every check here keep their meaning.
//!
//! The world is file based (Mother's home in a temporary directory: the
//! journal file and, once it exists, `private/`), so the scan reads real
//! files: every file under the home except `private/` (a `-wal` or `-shm`
//! beside a store included), the broker evidence the port's authority kept,
//! and the `Debug` of every value an oracle noted (each returned Mother
//! error, `Advance` and `EffectRefusal`).

use super::FileJournal;
use crate::adapter::m2c_support::consent;
use crate::contract::{Budget, MissionContract, PassCriterion, Question};
use crate::coordinator::{Coordinator, CoordinatorError, Lanes};
use crate::effect::{EffectRequest, PendingAction};
use crate::journal::{Entry, Event, JournalError, JournalStore, MissionId, GENESIS};
use crate::lane::{Lane, LaneIdentity, LaneInput, LaneOutput, LaneRole, Verdict};
use crate::oracles::{RecordingPort, Yes};
use crate::state::MissionState;
use nexus_governed_control::authority::clock::ManualClock;
use std::collections::BTreeSet;
use std::fmt::Debug;
use std::hash::{BuildHasher, Hasher};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

// ---------------------------------------------------------------- canaries

/// The two canaries of design §5.4: `C` (`NXCANARY-` and 32 random hex)
/// and `Cl` (`nxsecret_` and 16 lowercase hex, shaped like a code).
pub(crate) struct Canaries {
    pub c: String,
    pub cl: String,
}

impl Canaries {
    pub fn both(&self) -> [(&'static str, &str); 2] {
        [("C", self.c.as_str()), ("Cl", self.cl.as_str())]
    }
}

static DRAW: AtomicU64 = AtomicU64::new(0);

/// Random lowercase hex (the hasher keys of `RandomState` come from the
/// operating system's generator; no dependency is added for tests).
fn random_hex(n: usize) -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let mut out = String::new();
    while out.len() < n {
        let mut h = std::collections::hash_map::RandomState::new().build_hasher();
        h.write_u128(nanos);
        h.write_u64(DRAW.fetch_add(1, Ordering::SeqCst));
        h.write_usize(out.len());
        out.push_str(&format!("{:016x}", h.finish()));
    }
    out.truncate(n);
    out
}

pub(crate) fn canaries() -> Canaries {
    Canaries {
        c: format!("NXCANARY-{}", random_hex(32)),
        cl: format!("nxsecret_{}", random_hex(16)),
    }
}

// ---------------------------------------------------------------- encodings

const STD64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
const URL64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

/// Base64 with padding, in `alphabet`.
pub(crate) fn base64(bytes: &[u8], alphabet: &[u8; 64]) -> String {
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            chunk.get(1).copied().unwrap_or(0),
            chunk.get(2).copied().unwrap_or(0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(alphabet[((n >> (18 - 6 * i)) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

pub(crate) fn base64_std(bytes: &[u8]) -> String {
    base64(bytes, STD64)
}

pub(crate) fn base64_url(bytes: &[u8]) -> String {
    base64(bytes, URL64)
}

/// The base64 characters that depend on `text` alone when it is encoded
/// after `offset` (0, 1 or 2) other bytes: what any larger encoded value
/// that holds `text` contains.
fn base64_core(text: &[u8], offset: usize, alphabet: &[u8; 64]) -> String {
    let mut padded = vec![0u8; offset];
    padded.extend_from_slice(text);
    let encoded = base64(&padded, alphabet);
    let trimmed = encoded.trim_end_matches('=');
    let skip = [0, 2, 3][offset];
    let mut core: Vec<char> = trimmed.chars().skip(skip).collect();
    if !padded.len().is_multiple_of(3) {
        core.pop();
    }
    core.into_iter().collect()
}

pub(crate) fn percent_all(text: &str, upper: bool) -> String {
    text.bytes()
        .map(|b| {
            if upper {
                format!("%{b:02X}")
            } else {
                format!("%{b:02x}")
            }
        })
        .collect()
}

pub(crate) fn json_u(text: &str, upper: bool) -> String {
    text.chars()
        .map(|c| {
            if upper {
                format!("\\u{:04X}", c as u32)
            } else {
                format!("\\u{:04x}", c as u32)
            }
        })
        .collect()
}

/// Four fragments of `text`, each at least 8 characters, that together
/// cover it (overlapping when `text` is shorter than 32).
pub(crate) fn fragments(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let n = chars.len();
    let len = n.div_ceil(4).max(8).min(n);
    (0..4)
        .map(|i| {
            let start = if n <= len { 0 } else { (n - len) * i / 3 };
            chars[start..start + len].iter().collect()
        })
        .collect()
}

/// Every form the scanner looks for, named: raw, upper and lower case,
/// hex of the bytes, base64 (standard and URL-safe, at each of the three
/// alignments), percent-encoded, JSON `\u`-escaped, and each fragment of
/// the raw, upper and lower forms.
pub(crate) fn needles(canary: &str) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = vec![
        ("raw".into(), canary.to_string()),
        ("upper".into(), canary.to_uppercase()),
        ("lower".into(), canary.to_lowercase()),
        ("hex".into(), hex::encode(canary.as_bytes())),
        ("HEX".into(), hex::encode_upper(canary.as_bytes())),
        ("percent".into(), percent_all(canary, true)),
        ("percent-lower".into(), percent_all(canary, false)),
        ("json-u".into(), json_u(canary, false)),
        ("json-U".into(), json_u(canary, true)),
    ];
    for offset in 0..3 {
        out.push((
            format!("base64+{offset}"),
            base64_core(canary.as_bytes(), offset, STD64),
        ));
        out.push((
            format!("base64url+{offset}"),
            base64_core(canary.as_bytes(), offset, URL64),
        ));
    }
    for (case, text) in [
        ("raw", canary.to_string()),
        ("upper", canary.to_uppercase()),
        ("lower", canary.to_lowercase()),
    ] {
        for (i, fragment) in fragments(&text).into_iter().enumerate() {
            out.push((format!("fragment-{case}-{i}"), fragment));
        }
    }
    out
}

/// What of `canaries` `haystack` holds, as `<where>: <canary>/<form>`.
pub(crate) fn hits_in(haystack: &[u8], place: &str, canaries: &Canaries) -> Vec<String> {
    let text = String::from_utf8_lossy(haystack);
    let mut hits = Vec::new();
    for (name, canary) in canaries.both() {
        for (form, needle) in needles(canary) {
            if text.contains(needle.as_str()) {
                hits.push(format!("{place}: {name}/{form}"));
            }
        }
    }
    hits
}

/// Every file under `dir`, recursively, except the subtree `skip`: a
/// store's `-wal` and `-shm` files are read like any other.
pub(crate) fn files_under(dir: &Path, skip: Option<&Path>) -> Vec<(PathBuf, Vec<u8>)> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return out;
    };
    let mut paths: Vec<PathBuf> = entries.filter_map(|e| e.ok().map(|e| e.path())).collect();
    paths.sort();
    for path in paths {
        if skip.is_some_and(|s| path == s) {
            continue;
        }
        let Ok(meta) = std::fs::symlink_metadata(&path) else {
            continue;
        };
        if meta.is_dir() {
            out.extend(files_under(&path, skip));
        } else if let Ok(bytes) = std::fs::read(&path) {
            out.push((path, bytes));
        }
    }
    out
}

/// The scan of a directory: every file under it but `skip`.
pub(crate) fn scan_dir(dir: &Path, skip: Option<&Path>, canaries: &Canaries) -> Vec<String> {
    files_under(dir, skip)
        .into_iter()
        .flat_map(|(path, bytes)| {
            let place = path
                .strip_prefix(dir)
                .unwrap_or(&path)
                .display()
                .to_string();
            hits_in(&bytes, &place, canaries)
        })
        .collect()
}

// ---------------------------------------------------------------- homes

static HOMES: AtomicU64 = AtomicU64::new(0);

/// A new, empty, private directory under the system's temporary directory.
pub(crate) fn fresh_dir(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "nexus-mother-p0-{label}-{}-{}",
        random_hex(12),
        HOMES.fetch_add(1, Ordering::SeqCst)
    ));
    super::create_private(&dir).unwrap();
    dir
}

/// The journal store of the P0 world: the journal file, recording for each
/// append how many calls the port had seen before it (so an oracle can
/// tell whether a withdrawal came before a journaled Stop).
pub(crate) struct OrderedJournal {
    file: FileJournal,
    port: Arc<RecordingPort>,
    appends: Arc<Mutex<Vec<(usize, String)>>>,
}

impl JournalStore for OrderedJournal {
    fn load(&self) -> Result<Vec<String>, JournalError> {
        self.file.load()
    }

    fn append(&mut self, line: &str) -> Result<(), JournalError> {
        let calls = self.port.calls().len();
        self.file.append(line)?;
        self.appends.lock().unwrap().push((calls, line.to_string()));
        Ok(())
    }

    fn poison(&mut self) -> Result<(), JournalError> {
        self.file.poison()
    }
}

// ---------------------------------------------------------------- lanes

/// A lane for the P0 oracles. Its answer is decided by markers in the
/// restatement: `P0-HALT-<TAG>` (Halt with `reason`), `P0-FAIL-<TAG>` (Fail
/// with `reason`), `P0-ASK-<TAG>` (asks for `effects(mission)`); otherwise
/// the Verifier passes and every other lane continues. Its text carries
/// `reason` too (lane output). It counts its calls.
pub(crate) struct P0Lane {
    pub role: LaneRole,
    pub reason: String,
    pub effects: Arc<dyn Fn(MissionId) -> Vec<EffectRequest> + Send + Sync>,
    pub calls: Arc<AtomicUsize>,
}

impl Lane for P0Lane {
    fn identity(&self) -> LaneIdentity {
        LaneIdentity {
            role: self.role,
            name: format!("p0-{}", self.role.as_str()),
            version: "p0-test".into(),
        }
    }

    fn invoke(&self, input: &LaneInput) -> LaneOutput {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let tag = self.role.as_str().to_uppercase().replace('_', "");
        let r = &input.restatement;
        let effects = if r.contains(&format!("P0-ASK-{tag}")) {
            (self.effects)(input.invocation.mission)
        } else {
            vec![]
        };
        let verdict = if r.contains(&format!("P0-HALT-{tag}")) {
            Verdict::Halt {
                reason: self.reason.clone(),
            }
        } else if r.contains(&format!("P0-FAIL-{tag}")) {
            Verdict::Fail {
                reason: self.reason.clone(),
            }
        } else if self.role == LaneRole::Verifier {
            Verdict::Pass
        } else {
            Verdict::Continue
        };
        LaneOutput {
            text: format!("{} says {}", self.role.as_str(), self.reason),
            effects,
            verdict,
        }
    }
}

/// One P0 lane per role, sharing one call counter.
pub(crate) fn p0_lanes(
    reason: &str,
    effects: impl Fn(MissionId) -> Vec<EffectRequest> + Send + Sync + 'static,
) -> (Vec<Arc<dyn Lane>>, Arc<AtomicUsize>) {
    let calls = Arc::new(AtomicUsize::new(0));
    let effects: Arc<dyn Fn(MissionId) -> Vec<EffectRequest> + Send + Sync> = Arc::new(effects);
    let lanes = LaneRole::ALL
        .iter()
        .map(|role| {
            Arc::new(P0Lane {
                role: *role,
                reason: reason.to_string(),
                effects: effects.clone(),
                calls: calls.clone(),
            }) as Arc<dyn Lane>
        })
        .collect();
    (lanes, calls)
}

// ---------------------------------------------------------------- contracts

/// A complete contract whose every field (words, restatement, folder,
/// question id, text and answer, criterion id and text) holds both
/// canaries, and whose restatement also carries `markers`.
pub(crate) fn canary_contract(cs: &Canaries, markers: &str) -> MissionContract {
    MissionContract {
        owner_words: format!("Make the site {} and {}", cs.c, cs.cl),
        restatement: format!("Restated {} {} {markers}", cs.c, cs.cl)
            .trim_end()
            .to_string(),
        requested_folder: Some(format!("~/Sites/{}/{}", cs.c, cs.cl)),
        budget: Budget {
            max_duration_ms: 3_600_000,
            max_model_calls: 50,
            max_money_minor: 0,
        },
        forbidden_effects: BTreeSet::new(),
        questions: vec![Question {
            id: format!("q-{}-{}", cs.c, cs.cl),
            text: format!("Which {} or {}?", cs.c, cs.cl),
            answer: Some(format!("This {} and {}", cs.c, cs.cl)),
        }],
        pass_criteria: vec![PassCriterion {
            id: format!("p-{}-{}", cs.c, cs.cl),
            text: format!("It shows {} and {}", cs.c, cs.cl),
        }],
    }
}

/// A valid pending action (broker id forms) numbered `n`.
pub(crate) fn action(n: u8) -> PendingAction {
    PendingAction {
        run: format!("run-{:032x}", u128::from(n) + 0x7000),
        commitment: format!("cmt-{:032x}", u128::from(n) + 0x7000),
        deadline_mono_ms: 600_000,
    }
}

/// The v1 journal (main's format, before P0): a session, one mission
/// drafted and submitted, a clean close. Written by main's coordinator.
pub(crate) const V1_JOURNAL: &str = r#"{"seq":0,"prev":"0000000000000000000000000000000000000000000000000000000000000000","mission":0,"event":{"event":"session_opened"},"hash":"a30af65828a767513386935f3edd5a2832cfcbd1e5fc9734536fc9a3c1c4da2c"}
{"seq":1,"prev":"a30af65828a767513386935f3edd5a2832cfcbd1e5fc9734536fc9a3c1c4da2c","mission":1,"event":{"event":"drafted","contract":{"owner_words":"  Make me a small website for my bakery, \"Rosa's\".\n  Keep it simple!  ","restatement":"A one-page site for Rosa's bakery","requested_folder":"~/Sites/rosas-bakery","budget":{"max_duration_ms":3600000,"max_model_calls":50,"max_money_minor":0},"forbidden_effects":[],"questions":[{"id":"q1","text":"Which colours do you like?","answer":"Blue and cream"}],"pass_criteria":[{"id":"p1","text":"The page names the bakery"}]}},"hash":"405abb65833d78c8a9243e70a7288592567c01058784e82500356d86e933976f"}
{"seq":2,"prev":"405abb65833d78c8a9243e70a7288592567c01058784e82500356d86e933976f","mission":1,"event":{"event":"transition","trigger":{"trigger":"submit","hash":"f7301d50be4f0027a09717beb500d1bac79b1a72fa0ab6b77b6fedcd4e9161c7"}},"hash":"4eba48fd4b602c176f36013cb7708ad35a597bfc32e7e8521b176839af072b08"}
{"seq":3,"prev":"4eba48fd4b602c176f36013cb7708ad35a597bfc32e7e8521b176839af072b08","mission":0,"event":{"event":"session_closed"},"hash":"1f0b77814b822d84d426f63437adc823e821f5ad2d5fb37b485a05304ebc86af"}
"#;

// ---------------------------------------------------------------- the world

/// Mother over a recording port, with her home in a temporary directory.
pub(crate) struct P0World {
    pub dir: PathBuf,
    pub port: Arc<RecordingPort>,
    pub clock: Arc<ManualClock>,
    pub lanes: Vec<Arc<dyn Lane>>,
    pub lane_calls: Arc<AtomicUsize>,
    pub mother: Arc<Coordinator<RecordingPort>>,
    /// Every journal append: how many port calls came before it, and the
    /// line.
    pub appends: Arc<Mutex<Vec<(usize, String)>>>,
    seen: Mutex<Vec<String>>,
}

impl Drop for P0World {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

impl P0World {
    pub fn new(label: &str, lanes: (Vec<Arc<dyn Lane>>, Arc<AtomicUsize>)) -> Self {
        let dir = fresh_dir(label);
        let port = Arc::new(RecordingPort::default());
        let clock = Arc::new(ManualClock::default());
        let appends = Arc::new(Mutex::new(Vec::new()));
        let mother =
            Arc::new(open_in(&dir, &port, &lanes.0, &clock, &appends).expect("a new home opens"));
        Self {
            dir,
            port,
            clock,
            lanes: lanes.0,
            lane_calls: lanes.1,
            mother,
            appends,
            seen: Mutex::new(Vec::new()),
        }
    }

    pub fn journal_path(&self) -> PathBuf {
        self.dir.join("mother-journal.jsonl")
    }

    pub fn private_dir(&self) -> PathBuf {
        self.dir.join("private")
    }

    /// A coordinator over this home (another session).
    pub fn open(&self) -> Result<Coordinator<RecordingPort>, CoordinatorError> {
        open_in(
            &self.dir,
            &self.port,
            &self.lanes,
            &self.clock,
            &self.appends,
        )
    }

    /// An orderly restart: the clean close (refused while broken), then a
    /// new coordinator over the same home.
    pub fn reopen_clean(&mut self) -> Result<(), CoordinatorError> {
        let closed = self.mother.close_ref();
        self.note(&closed);
        self.replace()
    }

    /// A crash: no clean close, a new coordinator over the same home.
    pub fn crash_reopen(&mut self) -> Result<(), CoordinatorError> {
        self.replace()
    }

    fn replace(&mut self) -> Result<(), CoordinatorError> {
        let opened = self.open();
        match opened {
            Ok(mother) => {
                self.mother = Arc::new(mother);
                Ok(())
            }
            Err(e) => {
                self.note(&e);
                Err(e)
            }
        }
    }

    /// Keep the `Debug` of a value Mother returned, for the scan.
    pub fn note<T: Debug>(&self, value: &T) {
        self.seen.lock().unwrap().push(format!("{value:?}"));
    }

    pub fn draft(&self, contract: MissionContract) -> MissionId {
        let drafted = self.mother.draft(contract);
        self.note(&drafted);
        drafted.expect("a valid contract drafts")
    }

    pub fn submit(&self, id: MissionId) -> Result<MissionState, CoordinatorError> {
        let submitted = self.mother.submit(id);
        self.note(&submitted);
        submitted
    }

    pub fn consent_yes(&self, id: MissionId) -> Result<MissionState, CoordinatorError> {
        let answered = consent(&self.mother, id, &Yes);
        self.note(&answered);
        answered
    }

    pub fn approved(&self, contract: MissionContract) -> MissionId {
        let id = self.draft(contract);
        assert_eq!(self.submit(id), Ok(MissionState::AwaitingConsent));
        assert_eq!(self.consent_yes(id), Ok(MissionState::Approved));
        id
    }

    /// Advance once; its result is noted.
    pub fn advance(&self, id: MissionId) -> Result<crate::coordinator::Advance, CoordinatorError> {
        let advanced = self.mother.advance(id);
        self.note(&advanced);
        advanced
    }

    pub fn stop(&self, id: MissionId) -> Result<MissionState, CoordinatorError> {
        let stopped = self.mother.stop(id);
        self.note(&stopped);
        stopped
    }

    /// Every place a canary must not be (design §5.4's scan): every file
    /// under the home except `private/` (the journal, and any store's main,
    /// `-wal` and `-shm` files), the evidence the port's broker authority
    /// kept, and every noted value.
    pub fn scan(&self, canaries: &Canaries) -> Vec<String> {
        let private = self.private_dir();
        let mut hits = scan_dir(&self.dir, Some(&private), canaries);
        for record in self.port.evidence.records() {
            hits.extend(hits_in(
                format!("{record:?}").as_bytes(),
                "broker evidence",
                canaries,
            ));
        }
        for value in self.seen.lock().unwrap().iter() {
            hits.extend(hits_in(value.as_bytes(), "a returned value", canaries));
        }
        hits
    }

    /// Whether the private store holds `canary` (verbatim).
    pub fn private_contains(&self, canary: &str) -> bool {
        files_under(&self.private_dir(), None)
            .iter()
            .any(|(_, bytes)| String::from_utf8_lossy(bytes).contains(canary))
    }

    pub fn journal_lines(&self) -> Vec<String> {
        std::fs::read_to_string(self.journal_path())
            .unwrap()
            .lines()
            .map(str::to_string)
            .collect()
    }

    pub fn journal_values(&self) -> Vec<serde_json::Value> {
        self.journal_lines()
            .iter()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect()
    }

    fn write_journal(&self, lines: &[String]) {
        let text: String = lines.iter().map(|l| format!("{l}\n")).collect();
        std::fs::write(self.journal_path(), text).unwrap();
    }

    /// Drop the journal's last line (Mother died before writing it).
    pub fn drop_last_line(&self) -> String {
        let mut lines = self.journal_lines();
        let last = lines.pop().unwrap();
        self.write_journal(&lines);
        last
    }

    /// The private content a mission's journal names last, by kind
    /// (`contract`: its draft or last revision; `paused_phase`: its last
    /// wait's rest), as `(content id, size)`, by the journal's design §3 I3
    /// fields. None if the journal names none.
    pub fn content_of(&self, mission: MissionId, kind: &str) -> Option<(String, u64)> {
        let mut found = None;
        for value in self.journal_values() {
            if value["mission"].as_u64() != Some(mission.0) {
                continue;
            }
            let event = &value["event"];
            let reference = match kind {
                "contract" => {
                    if event["event"] == "drafted" {
                        Some(&event["contract"])
                    } else if event["trigger"]["trigger"] == "revise" {
                        Some(&event["trigger"]["contract"])
                    } else {
                        None
                    }
                }
                _ if event["trigger"]["trigger"] == "await_approval" => {
                    Some(&event["trigger"]["rest"])
                }
                _ => None,
            };
            if let Some(r) = reference {
                if let (Some(content), Some(bytes)) = (r["content"].as_str(), r["bytes"].as_u64()) {
                    found = Some((content.to_string(), bytes));
                }
            }
        }
        found
    }

    /// Where the private store keeps a mission's content (design §5.2:
    /// `<home>/private/<kind>/<hex>`), if the journal names one.
    pub fn blob_of(&self, mission: MissionId, kind: &str) -> Option<PathBuf> {
        let (content, _) = self.content_of(mission, kind)?;
        Some(self.private_dir().join(kind).join(content))
    }

    /// Delete a mission's private content: whether a file was removed.
    pub fn delete_blob(&self, mission: MissionId, kind: &str) -> bool {
        self.blob_of(mission, kind)
            .is_some_and(|path| std::fs::remove_file(path).is_ok())
    }

    /// Replace a mission's private content with `change(old)`, under its
    /// name: whether it was there.
    pub fn replace_blob(
        &self,
        mission: MissionId,
        kind: &str,
        change: impl FnOnce(Vec<u8>) -> Vec<u8>,
    ) -> bool {
        let Some(path) = self.blob_of(mission, kind) else {
            return false;
        };
        let Ok(old) = std::fs::read(&path) else {
            return false;
        };
        let new = change(old.clone());
        assert_ne!(new, old, "a replacement changes the bytes");
        std::fs::write(&path, new).is_ok()
    }

    /// Put `bytes` in the private store under their own content id (design
    /// §3 I2), as no journal entry names them: an orphan.
    pub fn put_orphan(&self, kind: &str, bytes: &[u8]) -> PathBuf {
        let domain = format!("nexus.mother.private.v1/{kind}");
        let mut hasher = <sha2::Sha256 as sha2::Digest>::new();
        sha2::Digest::update(&mut hasher, domain.as_bytes());
        sha2::Digest::update(&mut hasher, (bytes.len() as u64).to_be_bytes());
        sha2::Digest::update(&mut hasher, bytes);
        let id = hex::encode(sha2::Digest::finalize(hasher));
        let dir = self.private_dir().join(kind);
        super::create_private(&dir).unwrap();
        let path = dir.join(id);
        std::fs::write(&path, bytes).unwrap();
        path
    }

    /// Rewrite the whole chain (it is unkeyed, design §1's non-claim), with
    /// `edit` applied to each entry's event as JSON: whether any entry
    /// changed. Every edited event must still be a well-formed event.
    pub fn rechain(&self, mut edit: impl FnMut(MissionId, &mut serde_json::Value) -> bool) -> bool {
        let mut changed = false;
        let mut prev = GENESIS.to_string();
        let mut lines = Vec::new();
        for (seq, value) in self.journal_values().into_iter().enumerate() {
            let mission = MissionId(value["mission"].as_u64().unwrap());
            let mut event = value["event"].clone();
            changed |= edit(mission, &mut event);
            let event: Event = serde_json::from_value(event).expect("an edited event parses");
            let entry = Entry::chained(seq as u64, &prev, mission, event);
            prev = entry.hash.clone();
            lines.push(entry.to_line());
        }
        self.write_journal(&lines);
        changed
    }

    /// Append one event, chained after the journal's last entry.
    pub fn append_chained(&self, mission: MissionId, event: Event) {
        let mut lines = self.journal_lines();
        let last: Entry = serde_json::from_str(lines.last().unwrap()).unwrap();
        lines.push(Entry::chained(last.seq + 1, &last.hash, mission, event).to_line());
        self.write_journal(&lines);
    }

    /// The journaled Stop transitions of `mission`: `(append index, reason)`.
    pub fn stops(&self, mission: MissionId) -> Vec<(usize, String)> {
        self.appends
            .lock()
            .unwrap()
            .iter()
            .enumerate()
            .filter_map(|(index, (_, line))| {
                let value: serde_json::Value = serde_json::from_str(line).ok()?;
                let trigger = &value["event"]["trigger"];
                (value["mission"].as_u64() == Some(mission.0) && trigger["trigger"] == "stop")
                    .then(|| (index, trigger["reason"].as_str().unwrap_or("").to_string()))
            })
            .collect()
    }

    /// How many port calls came before append `index`.
    pub fn calls_before_append(&self, index: usize) -> usize {
        self.appends.lock().unwrap()[index].0
    }

    /// How many mission consents the port's broker recorded.
    pub fn consent_records(&self) -> usize {
        self.port
            .evidence
            .records()
            .iter()
            .filter(|r| format!("{:?}", r.phase).contains("Mission"))
            .count()
    }

    /// Wait for an approval: advance a consented mission whose Builder asks
    /// (markers `P0-ASK-BUILDER`) until its first effect waits.
    pub fn to_waiting(&self, id: MissionId) {
        assert_eq!(
            self.advance(id).map(|a| a.to),
            Ok(MissionState::Investigating)
        );
        assert_eq!(self.advance(id).map(|a| a.to), Ok(MissionState::Building));
        self.port
            .reply(Ok(crate::effect::EffectOutcome::AwaitingApproval));
        assert_eq!(
            self.advance(id).map(|a| a.to),
            Ok(MissionState::AwaitingApproval)
        );
    }
}

/// HELPER (P0 API): a coordinator over the home at `dir`: its journal file
/// and (from the commit that adds it) its private store.
fn open_in(
    dir: &Path,
    port: &Arc<RecordingPort>,
    lanes: &[Arc<dyn Lane>],
    clock: &Arc<ManualClock>,
    appends: &Arc<Mutex<Vec<(usize, String)>>>,
) -> Result<Coordinator<RecordingPort>, CoordinatorError> {
    let file =
        FileJournal::open(&dir.join("mother-journal.jsonl")).map_err(CoordinatorError::Journal)?;
    let store = OrderedJournal {
        file,
        port: port.clone(),
        appends: appends.clone(),
    };
    let private =
        super::FilePrivate::open(&dir.join("private")).map_err(CoordinatorError::Private)?;
    Coordinator::open(
        store,
        port.clone(),
        Lanes::new(lanes.to_vec()).unwrap(),
        clock.clone(),
        private,
    )
}

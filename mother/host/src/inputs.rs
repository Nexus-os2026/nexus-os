//! M3-E M-1: the Owner's page inputs (facts, notes, optional goal file) as
//! immutable, bounded snapshots (ruling M3E-R1/R2).
//!
//! Each input is opened once, read-only, `O_NOFOLLOW | O_NONBLOCK`, checked
//! to be a regular file on that same descriptor, and read from it to at
//! most its cap plus one byte (never trusting metadata length). The digest
//! and the parse both come from that one retained buffer; nothing is ever
//! reread by path. This is host-owned input I/O for data the Owner chose,
//! not a Files capability, and it mints no authority: the parsed values are
//! the existing `nexus_mother` contract types and still pass contract
//! validation and the broker's own facts construction later.
//!
//! Errors are fixed codes (plus a 1-based line for the facts grammar and
//! word cap); no input text and no OS error text is ever carried.
//!
//! Data-only (ruling M3E-R2 1(a)): nothing here is called from the hands
//! runtime; wiring is M-3/M-5.

use nexus_governed_control::authority::evidence::shown;
use nexus_governed_control::inert_page::{ApprovedFacts, FactCode, POLICY_V1};
use nexus_mother::contract::Note;
use nexus_mother::page_inputs::PageFacts;
use nix::errno::Errno;
use nix::fcntl::OFlag;
use sha2::{Digest, Sha256};
use std::fmt;
use std::fs::{File, OpenOptions};
use std::io::Read;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

/// The largest facts file, in bytes.
pub const FACTS_MAX_BYTES: usize = 2048;
/// The largest notes file, in bytes.
pub const NOTES_MAX_BYTES: usize = 1792;
/// The largest goal file, in bytes.
pub const GOAL_MAX_BYTES: usize = 16384;
/// The longest stored goal, in Unicode scalar values (LFs included).
pub const GOAL_MAX_SCALARS: usize = 4096;
/// The one note's fixed label.
pub const NOTES_LABEL: &str = "notes";
/// The only page mission `--mission` names.
pub const PAGE_MISSION: &str = "one-page-site";

/// Which input a snapshot is: it fixes the cap and the parse.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputKind {
    Facts,
    Notes,
    Goal,
}

impl InputKind {
    /// The most bytes a file of this kind may hold.
    pub const fn cap(self) -> usize {
        match self {
            InputKind::Facts => FACTS_MAX_BYTES,
            InputKind::Notes => NOTES_MAX_BYTES,
            InputKind::Goal => GOAL_MAX_BYTES,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            InputKind::Facts => "facts",
            InputKind::Notes => "notes",
            InputKind::Goal => "goal",
        }
    }
}

/// Why an input was refused: a fixed code, never content.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputError {
    /// Missing, unopenable or unstattable.
    Unavailable,
    /// The final path component is a symbolic link (followed never).
    Link,
    /// A directory, FIFO, device or socket.
    NotRegular,
    /// More bytes than the kind's cap.
    TooLarge,
    /// A read error on the opened descriptor.
    Unreadable,
    NotUtf8,
    FactsFileInvalid {
        line: usize,
    },
    FactsWordTooLong {
        line: usize,
    },
    /// `ApprovedFacts::new` refused the values (no location).
    Fact(FactCode),
    GoalEmpty,
    GoalNotPlain,
    GoalTooLong,
}

impl InputError {
    pub fn code(&self) -> &'static str {
        match self {
            InputError::Unavailable => "input_unavailable",
            InputError::Link => "input_link",
            InputError::NotRegular => "input_not_regular",
            InputError::TooLarge => "input_too_large",
            InputError::Unreadable => "input_unreadable",
            InputError::NotUtf8 => "input_not_utf8",
            InputError::FactsFileInvalid { .. } => "facts_file_invalid",
            InputError::FactsWordTooLong { .. } => "facts_word_too_long",
            InputError::Fact(code) => code.as_str(),
            InputError::GoalEmpty => "goal_empty",
            InputError::GoalNotPlain => "goal_not_plain",
            InputError::GoalTooLong => "goal_too_long",
        }
    }

    /// The 1-based facts-file line, for grammar and word-cap refusals only.
    pub fn line(&self) -> Option<usize> {
        match self {
            InputError::FactsFileInvalid { line } | InputError::FactsWordTooLong { line } => {
                Some(*line)
            }
            _ => None,
        }
    }
}

/// An input opened and checked regular, not yet read.
pub struct OpenedInput {
    kind: InputKind,
    file: File,
}

impl fmt::Debug for OpenedInput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OpenedInput")
            .field("kind", &self.kind)
            .finish_non_exhaustive()
    }
}

impl OpenedInput {
    /// Open `path` read-only without following a final symbolic link and
    /// without blocking (a FIFO is refused, not waited on), then check the
    /// descriptor is a regular file.
    pub fn open(kind: InputKind, path: &Path) -> Result<Self, InputError> {
        let flags = OFlag::O_NOFOLLOW | OFlag::O_NONBLOCK | OFlag::O_CLOEXEC;
        let file = OpenOptions::new()
            .read(true)
            .custom_flags(flags.bits())
            .open(path)
            .map_err(|e| {
                if e.raw_os_error() == Some(Errno::ELOOP as i32) {
                    InputError::Link
                } else {
                    InputError::Unavailable
                }
            })?;
        let meta = file.metadata().map_err(|_| InputError::Unavailable)?;
        if !meta.file_type().is_file() {
            return Err(InputError::NotRegular);
        }
        Ok(Self { kind, file })
    }

    /// Read the snapshot from this descriptor: at most cap + 1 bytes.
    pub fn read(self) -> Result<InputSnapshot, InputError> {
        let cap = self.kind.cap();
        let mut bytes = Vec::with_capacity(cap + 1);
        (&self.file)
            .take(cap as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| InputError::Unreadable)?;
        if bytes.len() > cap {
            return Err(InputError::TooLarge);
        }
        let sha256 = Sha256::digest(&bytes).into();
        Ok(InputSnapshot {
            kind: self.kind,
            bytes,
            sha256,
        })
    }
}

/// [`OpenedInput::open`] then [`OpenedInput::read`].
pub fn read_input(kind: InputKind, path: &Path) -> Result<InputSnapshot, InputError> {
    OpenedInput::open(kind, path)?.read()
}

/// One input's complete bytes, retained, and their SHA-256. The digest is
/// over the raw file bytes; [`InputSnapshot::parse`] reads the same buffer.
pub struct InputSnapshot {
    kind: InputKind,
    bytes: Vec<u8>,
    sha256: [u8; 32],
}

impl fmt::Debug for InputSnapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("InputSnapshot")
            .field("kind", &self.kind)
            .field("bytes", &self.bytes.len())
            .field("sha256", &self.sha256_hex())
            .finish()
    }
}

/// A parsed input, in the existing contract types.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InputValue {
    Facts(PageFacts),
    Notes(Note),
    /// The goal, verbatim but for one dropped final LF.
    Goal(String),
}

impl InputSnapshot {
    pub fn kind(&self) -> InputKind {
        self.kind
    }

    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    pub fn sha256(&self) -> [u8; 32] {
        self.sha256
    }

    pub fn sha256_hex(&self) -> String {
        sha256_hex(&self.sha256)
    }

    /// Parse the retained bytes by kind; no normalization.
    pub fn parse(&self) -> Result<InputValue, InputError> {
        let body = self.body()?;
        match self.kind {
            InputKind::Facts => parse_facts(body).map(InputValue::Facts),
            InputKind::Notes => Ok(InputValue::Notes(note(body))),
            InputKind::Goal => parse_goal(body).map(InputValue::Goal),
        }
    }

    // UTF-8, less one optional final LF (the digest keeps it).
    fn body(&self) -> Result<&str, InputError> {
        let text = std::str::from_utf8(&self.bytes).map_err(|_| InputError::NotUtf8)?;
        Ok(text.strip_suffix('\n').unwrap_or(text))
    }
}

fn note(body: &str) -> Note {
    Note {
        label: NOTES_LABEL.into(),
        text: body.into(),
    }
}

/// Lowercase hex of a digest.
pub fn sha256_hex(digest: &[u8; 32]) -> String {
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

// D4 + M3E-R2 4: grammar (name, hours 1..7, phone, in that order, split at
// the first ": ", no CR, no blank or unknown line) → word cap → facts
// construction.
fn parse_facts(body: &str) -> Result<PageFacts, InputError> {
    let mut name = None;
    let mut hours = Vec::new();
    let mut phone = None;
    let mut values = Vec::new();
    let mut lines = 0;
    for (index, line) in body.split('\n').enumerate() {
        lines = index + 1;
        let invalid = InputError::FactsFileInvalid { line: lines };
        if line.contains('\r') {
            return Err(invalid);
        }
        let Some((key, value)) = line.split_once(": ") else {
            return Err(invalid);
        };
        match key {
            "name" if name.is_none() => name = Some(value),
            "hours"
                if name.is_some()
                    && phone.is_none()
                    && hours.len() < POLICY_V1.facts_hours_max_lines =>
            {
                hours.push(value)
            }
            "phone" if !hours.is_empty() && phone.is_none() => phone = Some(value),
            _ => return Err(invalid),
        }
        values.push((lines, value));
    }
    let (Some(name), Some(phone)) = (name, phone) else {
        return Err(InputError::FactsFileInvalid { line: lines + 1 });
    };
    for (line, value) in values {
        if value
            .split(' ')
            .any(|word| word.chars().count() > POLICY_V1.max_word_chars)
        {
            return Err(InputError::FactsWordTooLong { line });
        }
    }
    let approved = ApprovedFacts::new(name, &hours, phone).map_err(InputError::Fact)?;
    Ok(PageFacts {
        name: approved.name().into(),
        hours: approved.hours().to_vec(),
        phone: approved.phone().into(),
    })
}

// M3E-R2 3: LF separates lines; every other scalar must be shown.
fn parse_goal(body: &str) -> Result<String, InputError> {
    if body.trim().is_empty() {
        return Err(InputError::GoalEmpty);
    }
    if !body.chars().all(|c| c == '\n' || shown(c)) {
        return Err(InputError::GoalNotPlain);
    }
    if body.chars().count() > GOAL_MAX_SCALARS {
        return Err(InputError::GoalTooLong);
    }
    Ok(body.into())
}

/// The page flags: `--mission one-page-site --notes <abs> --facts <abs>
/// [--goal <abs>]`. Parsed only; no mission is enabled by them. A goal file
/// together with a typed goal is refused when M-3 adds typed mode.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PageArgs {
    pub notes: PathBuf,
    pub facts: PathBuf,
    pub goal: Option<PathBuf>,
}

/// Parse exactly the page flags: unknown, duplicate or valueless flags,
/// a missing or other mission, missing notes/facts and relative paths are
/// refused with a fixed code. No default location is ever supplied.
pub fn parse_page_args(args: impl IntoIterator<Item = String>) -> Result<PageArgs, &'static str> {
    let mut values = std::collections::BTreeMap::new();
    let mut args = args.into_iter();
    while let Some(key) = args.next() {
        if !matches!(key.as_str(), "--mission" | "--notes" | "--facts" | "--goal")
            || values.contains_key(&key)
        {
            return Err("invalid_arguments");
        }
        values.insert(key, args.next().ok_or("missing_value")?);
    }
    match values.remove("--mission").as_deref() {
        None => return Err("mission_required"),
        Some(PAGE_MISSION) => {}
        Some(_) => return Err("mission_unknown"),
    }
    let absolute = |value: String| {
        let path = PathBuf::from(value);
        if path.is_absolute() {
            Ok(path)
        } else {
            Err("absolute_path_required")
        }
    };
    let notes = absolute(values.remove("--notes").ok_or("notes_required")?)?;
    let facts = absolute(values.remove("--facts").ok_or("facts_required")?)?;
    let goal = values.remove("--goal").map(absolute).transpose()?;
    Ok(PageArgs { notes, facts, goal })
}

/// Every page input, parsed, with the digests of the exact bytes parsed.
#[derive(Clone, PartialEq, Eq)]
pub struct PageInputs {
    pub facts: PageFacts,
    pub note: Note,
    pub goal: Option<String>,
    pub facts_sha256: [u8; 32],
    pub notes_sha256: [u8; 32],
    pub goal_sha256: Option<[u8; 32]>,
}

impl fmt::Debug for PageInputs {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PageInputs")
            .field("facts_sha256", &sha256_hex(&self.facts_sha256))
            .field("notes_sha256", &sha256_hex(&self.notes_sha256))
            .field("goal_sha256", &self.goal_sha256.as_ref().map(sha256_hex))
            .finish_non_exhaustive()
    }
}

/// Read and parse the inputs `args` names, each exactly once.
pub fn read_page_inputs(args: &PageArgs) -> Result<PageInputs, (InputKind, InputError)> {
    let load = |kind, path: &Path| read_input(kind, path).map_err(|e| (kind, e));
    let at = |kind| move |e| (kind, e);
    let facts = load(InputKind::Facts, &args.facts)?;
    let notes = load(InputKind::Notes, &args.notes)?;
    let goal = args
        .goal
        .as_deref()
        .map(|path| load(InputKind::Goal, path))
        .transpose()?;
    Ok(PageInputs {
        facts: parse_facts(facts.body().map_err(at(InputKind::Facts))?)
            .map_err(at(InputKind::Facts))?,
        note: note(notes.body().map_err(at(InputKind::Notes))?),
        goal: match &goal {
            None => None,
            Some(g) => Some(g.body().and_then(parse_goal).map_err(at(InputKind::Goal))?),
        },
        facts_sha256: facts.sha256(),
        notes_sha256: notes.sha256(),
        goal_sha256: goal.as_ref().map(InputSnapshot::sha256),
    })
}

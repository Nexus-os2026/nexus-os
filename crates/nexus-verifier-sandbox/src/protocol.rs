//! The private control protocol between the backend and the verifier
//! sandbox helper.
//!
//! One `SOCK_SEQPACKET` socketpair carries whole messages; descriptors
//! travel only as `SCM_RIGHTS` alongside a [`ToHelper::Launch`] message,
//! each tagged with the role it plays ([`FdRole`]). A role decides the rights
//! the helper grants (they are compiled into the helper's policy); the
//! backend never sends rights, a seccomp rule or a namespace choice.
//!
//! Encoding is a fixed binary layout: a tag byte, big-endian integers,
//! length-prefixed byte strings and counted sequences. Every size is bounded
//! and decoding must consume exactly the message, so a malformed or oversized
//! message is refused rather than partially trusted.

use crate::policy::Role;

pub const PROTOCOL_VERSION: u32 = 1;
/// Largest message in either direction.
pub const MAX_MESSAGE_BYTES: usize = 64 * 1024;
/// Most descriptors a launch may carry.
pub const MAX_FDS: usize = 32;
/// Most argv entries, including argv\[0\].
pub const MAX_ARGS: usize = 64;
/// Most environment entries.
pub const MAX_ENV: usize = 64;
/// Largest single argv or environment entry.
pub const MAX_ENTRY_BYTES: usize = 4096;

/// The role of one descriptor carried by a launch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FdRole {
    /// The verified executable, launched by descriptor.
    Executable,
    /// The verifier's working directory.
    WorkingDirectory,
    /// An object granted to the verifier with the role's fixed rights.
    Rule(Role),
}

/// Everything the helper needs to launch, all backend-constructed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchMessage {
    pub generation: u64,
    /// Must equal the policy the helper enforces, or it refuses.
    pub policy_hash: [u8; 32],
    /// argv, including argv\[0\]. Bytes without NUL.
    pub argv: Vec<Vec<u8>>,
    /// `KEY=VALUE` entries; the verifier's whole environment.
    pub env: Vec<Vec<u8>>,
    /// Roles of the descriptors, in the order they are carried.
    pub fds: Vec<FdRole>,
}

/// Backend to helper.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToHelper {
    Launch(LaunchMessage),
    /// The identity uid/gid maps of the helper's new user namespace are
    /// written.
    Mapped,
}

/// Where sandbox setup stopped. Nothing untrusted has run at any of them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetupStage {
    Protocol = 1,
    Unshare = 2,
    Mapping = 3,
    NamespaceCheck = 4,
    Fork = 5,
    InitCheck = 6,
    NetworkCheck = 7,
    Stdio = 8,
    NoNewPrivs = 9,
    Landlock = 10,
    Seccomp = 11,
    Descriptors = 12,
    LayerCheck = 13,
    WorkingDirectory = 14,
    Exec = 15,
}

impl SetupStage {
    const ALL: [SetupStage; 15] = [
        Self::Protocol,
        Self::Unshare,
        Self::Mapping,
        Self::NamespaceCheck,
        Self::Fork,
        Self::InitCheck,
        Self::NetworkCheck,
        Self::Stdio,
        Self::NoNewPrivs,
        Self::Landlock,
        Self::Seccomp,
        Self::Descriptors,
        Self::LayerCheck,
        Self::WorkingDirectory,
        Self::Exec,
    ];

    pub fn from_code(code: u8) -> Option<Self> {
        Self::ALL.into_iter().find(|stage| *stage as u8 == code)
    }
}

/// How the verifier ended, as observed by the namespace init.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerifierStatus {
    Exited(i32),
    Signalled(i32),
}

/// Helper to backend.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FromHelper {
    Hello {
        version: u32,
        policy_hash: [u8; 32],
    },
    /// The namespaces exist; the backend may write the identity maps.
    NamespacesReady,
    /// Every layer is established and verified, and the verifier executable
    /// is running.
    Running,
    SetupFailed {
        stage: SetupStage,
        errno: i32,
    },
    Finished(VerifierStatus),
    /// The namespace init ended without a final report.
    InitLost,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecodeError {
    Empty,
    Truncated,
    TrailingBytes,
    UnknownTag,
    TooLarge,
    Invalid,
}

const TAG_LAUNCH: u8 = 1;
const TAG_MAPPED: u8 = 2;
const TAG_HELLO: u8 = 10;
const TAG_NS_READY: u8 = 11;
const TAG_RUNNING: u8 = 12;
const TAG_SETUP_FAILED: u8 = 13;
const TAG_FINISHED: u8 = 14;
const TAG_INIT_LOST: u8 = 15;

const ROLE_EXECUTABLE: u8 = 1;
const ROLE_WORKDIR: u8 = 2;
const ROLE_RULE_BASE: u8 = 16;

fn role_code(role: FdRole) -> u8 {
    match role {
        FdRole::Executable => ROLE_EXECUTABLE,
        FdRole::WorkingDirectory => ROLE_WORKDIR,
        FdRole::Rule(role) => {
            ROLE_RULE_BASE
                + Role::ALL
                    .iter()
                    .position(|r| *r == role)
                    .expect("every role is listed") as u8
        }
    }
}

fn role_from_code(code: u8) -> Option<FdRole> {
    match code {
        ROLE_EXECUTABLE => Some(FdRole::Executable),
        ROLE_WORKDIR => Some(FdRole::WorkingDirectory),
        code if code >= ROLE_RULE_BASE => Role::ALL
            .get(usize::from(code - ROLE_RULE_BASE))
            .map(|role| FdRole::Rule(*role)),
        _ => None,
    }
}

struct Writer(Vec<u8>);

impl Writer {
    fn u8(&mut self, value: u8) {
        self.0.push(value);
    }
    fn u32(&mut self, value: u32) {
        self.0.extend_from_slice(&value.to_be_bytes());
    }
    fn i32(&mut self, value: i32) {
        self.0.extend_from_slice(&value.to_be_bytes());
    }
    fn u64(&mut self, value: u64) {
        self.0.extend_from_slice(&value.to_be_bytes());
    }
    fn bytes(&mut self, value: &[u8]) {
        self.u32(value.len() as u32);
        self.0.extend_from_slice(value);
    }
}

struct Reader<'a>(&'a [u8]);

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], DecodeError> {
        if self.0.len() < n {
            return Err(DecodeError::Truncated);
        }
        let (head, rest) = self.0.split_at(n);
        self.0 = rest;
        Ok(head)
    }
    fn u8(&mut self) -> Result<u8, DecodeError> {
        Ok(self.take(1)?[0])
    }
    fn u32(&mut self) -> Result<u32, DecodeError> {
        Ok(u32::from_be_bytes(self.take(4)?.try_into().expect("4")))
    }
    fn i32(&mut self) -> Result<i32, DecodeError> {
        Ok(i32::from_be_bytes(self.take(4)?.try_into().expect("4")))
    }
    fn u64(&mut self) -> Result<u64, DecodeError> {
        Ok(u64::from_be_bytes(self.take(8)?.try_into().expect("8")))
    }
    fn hash(&mut self) -> Result<[u8; 32], DecodeError> {
        Ok(self.take(32)?.try_into().expect("32"))
    }
    fn bytes(&mut self, max: usize) -> Result<Vec<u8>, DecodeError> {
        let len = self.u32()? as usize;
        if len > max {
            return Err(DecodeError::TooLarge);
        }
        Ok(self.take(len)?.to_vec())
    }
    fn count(&mut self, max: usize) -> Result<usize, DecodeError> {
        let count = self.u32()? as usize;
        if count > max {
            return Err(DecodeError::TooLarge);
        }
        Ok(count)
    }
    fn end(&self) -> Result<(), DecodeError> {
        if self.0.is_empty() {
            Ok(())
        } else {
            Err(DecodeError::TrailingBytes)
        }
    }
}

fn valid_entry(entry: &[u8]) -> bool {
    !entry.contains(&0)
}

fn valid_env(entry: &[u8]) -> bool {
    match entry.iter().position(|b| *b == b'=') {
        Some(0) | None => false,
        Some(eq) => {
            valid_entry(entry)
                && entry[..eq]
                    .iter()
                    .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || *b == b'_')
        }
    }
}

impl LaunchMessage {
    /// Whether the message is structurally acceptable: bounded sizes, argv
    /// present, no NUL bytes, well-formed unique environment keys, exactly
    /// one executable and one working directory, and at most
    /// [`MAX_FDS`] descriptors.
    pub fn is_well_formed(&self) -> bool {
        let executables = self
            .fds
            .iter()
            .filter(|role| **role == FdRole::Executable)
            .count();
        let workdirs = self
            .fds
            .iter()
            .filter(|role| **role == FdRole::WorkingDirectory)
            .count();
        let mut keys: Vec<&[u8]> = self
            .env
            .iter()
            .filter_map(|entry| entry.iter().position(|b| *b == b'=').map(|eq| &entry[..eq]))
            .collect();
        let key_count = keys.len();
        keys.sort_unstable();
        keys.dedup();
        !self.argv.is_empty()
            && self.argv.len() <= MAX_ARGS
            && self.env.len() <= MAX_ENV
            && self.fds.len() <= MAX_FDS
            && self
                .argv
                .iter()
                .all(|arg| arg.len() <= MAX_ENTRY_BYTES && valid_entry(arg))
            && self
                .env
                .iter()
                .all(|entry| entry.len() <= MAX_ENTRY_BYTES && valid_env(entry))
            && key_count == self.env.len()
            && keys.len() == key_count
            && executables == 1
            && workdirs == 1
    }
}

impl ToHelper {
    pub fn encode(&self) -> Vec<u8> {
        let mut w = Writer(Vec::new());
        match self {
            Self::Launch(launch) => {
                w.u8(TAG_LAUNCH);
                w.u32(PROTOCOL_VERSION);
                w.u64(launch.generation);
                w.0.extend_from_slice(&launch.policy_hash);
                w.u32(launch.argv.len() as u32);
                for arg in &launch.argv {
                    w.bytes(arg);
                }
                w.u32(launch.env.len() as u32);
                for entry in &launch.env {
                    w.bytes(entry);
                }
                w.u32(launch.fds.len() as u32);
                for role in &launch.fds {
                    w.u8(role_code(*role));
                }
            }
            Self::Mapped => w.u8(TAG_MAPPED),
        }
        w.0
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, DecodeError> {
        if bytes.is_empty() {
            return Err(DecodeError::Empty);
        }
        if bytes.len() > MAX_MESSAGE_BYTES {
            return Err(DecodeError::TooLarge);
        }
        let mut r = Reader(bytes);
        let message = match r.u8()? {
            TAG_LAUNCH => {
                if r.u32()? != PROTOCOL_VERSION {
                    return Err(DecodeError::Invalid);
                }
                let generation = r.u64()?;
                let policy_hash = r.hash()?;
                let argc = r.count(MAX_ARGS)?;
                let argv = (0..argc)
                    .map(|_| r.bytes(MAX_ENTRY_BYTES))
                    .collect::<Result<Vec<_>, _>>()?;
                let envc = r.count(MAX_ENV)?;
                let env = (0..envc)
                    .map(|_| r.bytes(MAX_ENTRY_BYTES))
                    .collect::<Result<Vec<_>, _>>()?;
                let fdc = r.count(MAX_FDS)?;
                let fds = (0..fdc)
                    .map(|_| role_from_code(r.u8()?).ok_or(DecodeError::Invalid))
                    .collect::<Result<Vec<_>, _>>()?;
                let launch = LaunchMessage {
                    generation,
                    policy_hash,
                    argv,
                    env,
                    fds,
                };
                if !launch.is_well_formed() {
                    return Err(DecodeError::Invalid);
                }
                Self::Launch(launch)
            }
            TAG_MAPPED => Self::Mapped,
            _ => return Err(DecodeError::UnknownTag),
        };
        r.end()?;
        Ok(message)
    }
}

impl FromHelper {
    pub fn encode(&self) -> Vec<u8> {
        let mut w = Writer(Vec::new());
        match self {
            Self::Hello {
                version,
                policy_hash,
            } => {
                w.u8(TAG_HELLO);
                w.u32(*version);
                w.0.extend_from_slice(policy_hash);
            }
            Self::NamespacesReady => w.u8(TAG_NS_READY),
            Self::Running => w.u8(TAG_RUNNING),
            Self::SetupFailed { stage, errno } => {
                w.u8(TAG_SETUP_FAILED);
                w.u8(*stage as u8);
                w.i32(*errno);
            }
            Self::Finished(status) => {
                w.u8(TAG_FINISHED);
                match status {
                    VerifierStatus::Exited(code) => {
                        w.u8(0);
                        w.i32(*code);
                    }
                    VerifierStatus::Signalled(signal) => {
                        w.u8(1);
                        w.i32(*signal);
                    }
                }
            }
            Self::InitLost => w.u8(TAG_INIT_LOST),
        }
        w.0
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, DecodeError> {
        if bytes.is_empty() {
            return Err(DecodeError::Empty);
        }
        if bytes.len() > MAX_MESSAGE_BYTES {
            return Err(DecodeError::TooLarge);
        }
        let mut r = Reader(bytes);
        let message = match r.u8()? {
            TAG_HELLO => Self::Hello {
                version: r.u32()?,
                policy_hash: r.hash()?,
            },
            TAG_NS_READY => Self::NamespacesReady,
            TAG_RUNNING => Self::Running,
            TAG_SETUP_FAILED => Self::SetupFailed {
                stage: SetupStage::from_code(r.u8()?).ok_or(DecodeError::Invalid)?,
                errno: r.i32()?,
            },
            TAG_FINISHED => match r.u8()? {
                0 => Self::Finished(VerifierStatus::Exited(r.i32()?)),
                1 => Self::Finished(VerifierStatus::Signalled(r.i32()?)),
                _ => return Err(DecodeError::Invalid),
            },
            TAG_INIT_LOST => Self::InitLost,
            _ => return Err(DecodeError::UnknownTag),
        };
        r.end()?;
        Ok(message)
    }
}

#[cfg(test)]
mod tests;

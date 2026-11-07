//! M3-E M-3a (rulings M3E-R1, R8): the Owner's typed goal, one line read by
//! the terminal's single reader in a separate data mode.
//!
//! Frozen rules (R8 3 (a)–(h)):
//! - one line: Return (CR or LF) ends it, so a typed goal holds no LF or CR;
//! - its own buffer of at most 16,384 bytes; more is `goal_too_long`, and
//!   every byte of it is discarded;
//! - UTF-8, then the SAME validator as a goal file (`inputs::validate_goal`):
//!   empty, not shown, more than 4,096 scalars are refused; the goal is kept
//!   verbatim, spaces included;
//! - a line that is exactly `stop` or `quit` is Stop or Quit, Ctrl-D is EOF;
//!   none of them is a goal. Every other line (`approve`, a code) is data;
//! - bytes typed before the prompt are stale and never become a goal;
//! - only `shown` characters are echoed; Backspace removes one scalar,
//!   Ctrl-U the line; the 64-byte command mode is unchanged;
//! - the accepted goal is displayed through T1 with the goal escaped, its
//!   byte count and SHA-256; it never appears in `Debug`, errors or evidence;
//! - when a goal file was given, typed mode is refused (`goal_already_given`).
//!
//! Data-only (R8 2): nothing in the scripted `run` flow calls it; M-5 does.

use crate::inputs::{sha256_hex, validate_goal, InputError, GOAL_MAX_BYTES};
use nexus_governed_control::authority::evidence::{escaped, shown};
use nexus_mother::teaching::t1;
use sha2::{Digest, Sha256};
use std::fmt;
use std::time::Instant;

/// What one typed goal line became.
#[derive(Clone, PartialEq, Eq)]
pub(crate) enum GoalLine {
    Stop,
    Quit,
    Eof,
    Goal(Result<String, InputError>),
}

impl fmt::Debug for GoalLine {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GoalLine::Stop => f.write_str("Stop"),
            GoalLine::Quit => f.write_str("Quit"),
            GoalLine::Eof => f.write_str("Eof"),
            GoalLine::Goal(Ok(goal)) => write!(f, "Goal(chars={})", goal.chars().count()),
            GoalLine::Goal(Err(e)) => write!(f, "Goal({})", e.code()),
        }
    }
}

/// The goal mode's line buffer, beside the command buffer.
#[derive(Default)]
pub(crate) struct GoalBuffer {
    bytes: Vec<u8>,
    overflow: bool,
    stale: bool,
    after_cr: bool,
}

impl GoalBuffer {
    #[cfg(test)]
    pub(crate) fn buffered(&self) -> usize {
        self.bytes.len()
    }

    /// Bytes already typed can never complete a goal (the buffer's unit
    /// contract; the reader's invalidation is the line owner's, M3E-R13).
    #[cfg(test)]
    pub(crate) fn invalidate(&mut self) {
        if !self.bytes.is_empty() || self.overflow {
            self.stale = true;
        }
    }

    pub(crate) fn push(&mut self, byte: u8) -> Option<GoalLine> {
        if byte == 4 {
            return Some(GoalLine::Eof);
        }
        let after_cr = std::mem::replace(&mut self.after_cr, byte == b'\r');
        if byte == b'\n' && after_cr {
            return None;
        }
        if byte == b'\r' || byte == b'\n' {
            let line = if self.bytes == b"stop" {
                Some(GoalLine::Stop)
            } else if self.bytes == b"quit" {
                Some(GoalLine::Quit)
            } else if self.stale {
                None
            } else if self.overflow {
                Some(GoalLine::Goal(Err(InputError::GoalTooLong)))
            } else {
                Some(GoalLine::Goal(validate_goal(&self.bytes)))
            };
            self.bytes.clear();
            self.overflow = false;
            self.stale = false;
            return line;
        }
        match byte {
            0x7f | 0x08 => {
                self.pop_scalar();
                return None;
            }
            0x15 => {
                // Editing cannot make stale or overflowed input fresh.
                self.bytes.clear();
                return None;
            }
            _ => {}
        }
        if self.overflow {
            return None;
        }
        if self.bytes.len() < GOAL_MAX_BYTES {
            self.bytes.push(byte);
        } else {
            self.overflow = true;
            self.bytes.clear();
        }
        None
    }

    fn pop_scalar(&mut self) {
        while let Some(b) = self.bytes.pop() {
            if b & 0xC0 != 0x80 {
                break;
            }
        }
    }

    /// The scalar the buffer ends with, if it is complete.
    fn last_scalar(&self) -> Option<char> {
        let n = self.bytes.len();
        let start = (n.saturating_sub(4)..n)
            .rev()
            .find(|&i| self.bytes[i] & 0xC0 != 0x80)?;
        std::str::from_utf8(&self.bytes[start..])
            .ok()?
            .chars()
            .next()
    }

    /// The scalar `byte` completes, if any.
    fn completed(&self, byte: u8) -> Option<char> {
        let n = self.bytes.len();
        let pending = (n.saturating_sub(3)..n)
            .rev()
            .find(|&i| self.bytes[i] & 0xC0 != 0x80)
            .filter(|&i| width(self.bytes[i]) > n - i);
        let mut seq = pending.map_or_else(Vec::new, |i| self.bytes[i..].to_vec());
        seq.push(byte);
        let text = std::str::from_utf8(&seq).ok()?;
        let mut chars = text.chars();
        match (chars.next(), chars.next()) {
            (Some(c), None) => Some(c),
            _ => None,
        }
    }

    /// Echo before `push`: shown characters, and erasure of shown ones.
    pub(crate) fn echo(&self, byte: u8, out: &mut String) {
        if self.stale || self.overflow {
            return;
        }
        match byte {
            b'\n' if self.after_cr => {}
            b'\r' | b'\n' => out.push('\n'),
            0x7f | 0x08 => {
                if self.last_scalar().is_some_and(shown) {
                    out.push_str("\x08 \x08");
                }
            }
            0x15 => {
                for chunk in self.bytes.utf8_chunks() {
                    for _ in chunk.valid().chars().filter(|c| shown(*c)) {
                        out.push_str("\x08 \x08");
                    }
                }
            }
            _ if self.bytes.len() < GOAL_MAX_BYTES => {
                if let Some(c) = self.completed(byte).filter(|c| shown(*c)) {
                    out.push(c);
                }
            }
            _ => {}
        }
    }
}

// The length of the UTF-8 sequence `lead` starts (0 for a non-lead byte).
fn width(lead: u8) -> usize {
    match lead {
        0x00..=0x7f => 1,
        0xc2..=0xdf => 2,
        0xe0..=0xef => 3,
        0xf0..=0xf4 => 4,
        _ => 0,
    }
}

/// An accepted typed goal. `Debug` shows its size only.
#[derive(Clone, PartialEq, Eq)]
pub(crate) struct TypedGoal(String);

impl fmt::Debug for TypedGoal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "TypedGoal(chars={})", self.0.chars().count())
    }
}

impl TypedGoal {
    pub(crate) fn new(goal: String) -> Self {
        TypedGoal(goal)
    }

    /// The goal, verbatim.
    #[allow(dead_code)] // Consumed by M-5's page-mode wiring.
    pub(crate) fn text(&self) -> &str {
        &self.0
    }
}

/// The goal's display: T1 with the goal escaped, then its size and digest.
pub(crate) fn goal_display(goal: &str) -> String {
    format!(
        "{}\ngoal: {} bytes, sha256 {}\n",
        t1(&escaped(goal)),
        goal.len(),
        sha256_hex(&Sha256::digest(goal.as_bytes()).into())
    )
}

/// The prompt shown before goal input is accepted.
pub(crate) const GOAL_PROMPT: &str = "Type your goal on one line, then press Return. It is kept exactly as typed. It is data only: it never answers a dialog or runs a command. Type stop to stop.\n";

/// D3 + R8 (h): typed mode only when no goal file was given. Data-only:
/// wired in M-5.
#[allow(dead_code)]
pub(crate) fn read_typed_goal(
    shared: &crate::terminal::Shared,
    goal_file_given: bool,
    deadline: Instant,
) -> Result<TypedGoal, &'static str> {
    if goal_file_given {
        return Err("goal_already_given");
    }
    shared.read_goal(deadline)
}

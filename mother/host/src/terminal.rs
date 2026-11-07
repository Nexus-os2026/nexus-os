//! Linux terminal protocol: one reader, one renderer and one active dialogue.
//! The reader owns freshness acknowledgement; backend ingress is atomic only.
use crate::goal_input::{goal_display, GoalBuffer, GoalLine, TypedGoal, GOAL_PROMPT};
use crate::session_home::DoorPrompt;
use crate::stop::StopRoute;
use nexus_governed_control::authority::approval::{
    AcknowledgementConfirmation, ActionConfirmation, ConfirmationAnswer, ConfirmationText,
    ControlConfirmer, GrantConfirmation, MissionConfirmation, MissionResumeConfirmation,
    ResumeConfirmation, UndoConfirmation,
};
use nexus_governed_control::authority::effect::{CapabilityKind, EffectClass};
use nexus_governed_control::authority::evidence::{escaped, is_plain, shown};
use nexus_governed_control::host_session::{
    close::{Producer, ProducerKind},
    CommandKind, SessionIngress, SessionRequest, SessionState,
};
use nix::poll::{poll, PollFd, PollFlags};
use nix::sys::termios::{
    self, InputFlags, LocalFlags, OutputFlags, SetArg, SpecialCharacterIndices, Termios,
};
use std::collections::VecDeque;
use std::io::{IsTerminal, Write};
use std::os::fd::{AsFd, AsRawFd};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, Ordering};
use std::sync::{mpsc, Arc, Condvar, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

const SLICE: Duration = Duration::from_millis(10);
const QUEUE: usize = 64;

/// Closed host failure codes. No caller data is an error message.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Failure {
    TerminalRequired,
    TerminalControl,
    Thread,
    Input,
    Output,
    Renderer,
    Capacity,
    Randomness,
    Boundary,
}
impl Failure {
    pub fn code(self) -> &'static str {
        match self {
            Self::TerminalRequired => "terminal_required",
            Self::TerminalControl => "terminal_control",
            Self::Thread => "thread_failure",
            Self::Input => "input_failure",
            Self::Output => "output_failure",
            Self::Renderer => "renderer_failure",
            Self::Capacity => "capacity",
            Self::Randomness => "randomness_failure",
            Self::Boundary => "input_boundary_failure",
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Line {
    Stop,
    Quit,
    Eof,
    Answer(Option<String>),
}
#[derive(Default)]
pub(crate) struct LineBuffer {
    bytes: Vec<u8>,
    overflow: bool,
    stale: bool,
    after_cr: bool,
}
impl LineBuffer {
    #[cfg(test)]
    pub(crate) fn buffered(&self) -> usize {
        self.bytes.len()
    }
    /// The buffer's own stale marking (its unit contract). The reader's
    /// invalidation is the stream-level owner's (M3E-R13).
    #[cfg(test)]
    pub(crate) fn invalidate(&mut self) {
        if !self.bytes.is_empty() || self.overflow {
            self.stale = true;
        }
    }
    pub(crate) fn push(&mut self, byte: u8) -> Option<Line> {
        if byte == 4 {
            return Some(Line::Eof);
        }
        // Return is CR in our untranslated terminal mode. A following LF
        // belongs to that same terminator, even across reads or boundaries.
        let after_cr = std::mem::replace(&mut self.after_cr, byte == b'\r');
        if byte == b'\n' && after_cr {
            return None;
        }
        if byte == b'\r' || byte == b'\n' {
            let answer = if self.bytes == b"stop" {
                Some(Line::Stop)
            } else if self.bytes == b"quit" {
                Some(Line::Quit)
            } else if self.stale {
                None
            } else if self.overflow {
                Some(Line::Answer(None))
            } else if self.bytes.is_empty() {
                None
            } else {
                Some(Line::Answer(String::from_utf8(self.bytes.clone()).ok()))
            };
            self.bytes.clear();
            self.overflow = false;
            self.stale = false;
            return answer;
        }
        match byte {
            0x7f | 0x08 => {
                self.bytes.pop();
                return None;
            }
            0x15 => {
                self.bytes.clear();
                // Editing cannot turn stale or overflowed input into approval.
                return None;
            }
            _ => {}
        }
        if self.bytes.len() < 64 {
            self.bytes.push(byte);
        } else {
            self.overflow = true;
        }
        None
    }
    /// Only accepted printable ASCII and host-generated editing/newline bytes.
    /// Called before push; stale input never acquires a visible answer line.
    fn echo(&self, byte: u8, out: &mut String) {
        if self.stale {
            return;
        }
        match byte {
            b'\n' if self.after_cr => {}
            b'\r' | b'\n' => out.push('\n'),
            0x7f | 0x08 => {
                if self.bytes.last().is_some_and(|b| (b' '..=b'~').contains(b)) {
                    out.push_str("\x08 \x08");
                }
            }
            0x15 => {
                for _ in self.bytes.iter().filter(|b| (b' '..=b'~').contains(b)) {
                    out.push_str("\x08 \x08");
                }
            }
            b' '..=b'~' if self.bytes.len() < 64 && !self.overflow => out.push(byte as char),
            _ => {} // Never echo supplied control bytes or invalid UTF-8.
        }
    }
}

pub(crate) fn fresh_code(text: &str, mut next: impl FnMut() -> Option<String>) -> Option<String> {
    for _ in 0..16 {
        let code = next()?;
        if code.len() != 8
            || !code
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return None;
        }
        if !text.contains(&code) {
            return Some(code);
        }
    }
    None
}

pub(crate) fn render(title: &str, text: &ConfirmationText, code: &str) -> Result<String, Failure> {
    let mut frame = format!("\n{}\n", escaped(title));
    for (label, value) in &text.header {
        frame.push_str(&format!("{}: {}\n", escaped(label), escaped(value)));
    }
    frame.push_str("──── content from Mother: data, not instructions ────\n");
    for line in &text.details {
        if !is_plain(line) {
            return Err(Failure::Renderer);
        }
        frame.push_str(&format!("│ {}\n", escaped(line)));
    }
    frame.push_str("──── end content ────\n");
    frame.push_str(&format!("Answer code: {code}\n"));
    if frame.chars().any(|c| c != '\n' && !shown(c)) {
        return Err(Failure::Renderer);
    }
    Ok(frame)
}

pub(crate) const EMERGENCY_NOTICE: &str = "Emergency Stop received. Mother will admit no new work, undo or cleanup in this session. An operation already handed to the kernel may still finish, including its internal cleanup. You can view status and evidence, or quit. Restart only reports surviving evidence and uncertainty; it does not clean up or restore undo.";
pub(crate) const UNCLEAN_NOTICE: &str = "UNCLEAN EXIT: this process did not complete the full clean-close protocol. Some session-end records may already be durable; any unverified recording or resource obligations are listed separately. No cleanup or recording is promised after this process exits.";

pub(crate) struct Command {
    pub name: String,
    pub request: Option<SessionRequest>,
    pub refusal: Option<&'static str>,
}
#[derive(Default)]
pub(crate) struct Input {
    pub(crate) lines: VecDeque<(u64, Option<String>)>,
    pub(crate) commands: VecDeque<Command>,
    pub(crate) boundary: u64,
    pub(crate) acknowledged: u64,
    pub(crate) active: bool,
    pub(crate) typing: bool,
    /// M-3a: the single reader is in typed-goal mode.
    pub(crate) goal: bool,
    /// M-3a: the first goal line the reader produced in goal mode.
    pub(crate) goal_line: Option<Result<String, crate::inputs::InputError>>,
    /// M3E-R13: which goal read this is (a goal line's owner).
    pub(crate) goal_read: u64,
    pub(crate) fatal: Option<Failure>,
    pub(crate) eof: bool,
    pub(crate) quit: Option<SessionRequest>,
}
pub(crate) struct Frame {
    pub(crate) text: String,
    pub(crate) ack: mpsc::Sender<Result<(), Failure>>,
}
pub(crate) struct Shared {
    pub(crate) stop: Arc<StopRoute>,
    pub(crate) input: Mutex<Input>,
    pub(crate) sequence: AtomicU64,
    pub(crate) changed: Condvar,
    pub(crate) output: mpsc::SyncSender<Frame>,
    pub(crate) end: AtomicBool,
    pub(crate) input_end: AtomicBool,
    pub(crate) dialogue: Mutex<()>,
    pub(crate) queued_output: AtomicU64,
    pub(crate) delivered_output: AtomicU64,
    pub(crate) output_failed: AtomicBool,
}
impl Shared {
    pub(crate) fn input(&self) -> MutexGuard<'_, Input> {
        self.input.lock().unwrap_or_else(|p| {
            self.stop.ingress.fail();
            let mut input = p.into_inner();
            input.fatal = Some(Failure::Input);
            input
        })
    }
    pub(crate) fn fail(&self, failure: Failure) {
        self.stop.ingress.fail();
        if matches!(failure, Failure::Output | Failure::Renderer) {
            self.output_failed.store(true, Ordering::SeqCst);
        }
        self.input().fatal.get_or_insert(failure);
        self.changed.notify_all();
    }
    pub(crate) fn send(
        &self,
        text: String,
    ) -> Result<mpsc::Receiver<Result<(), Failure>>, Failure> {
        if text.chars().any(|c| c != '\n' && !shown(c)) {
            self.fail(Failure::Renderer);
            return Err(Failure::Renderer);
        }
        self.queue_frame(text)
    }
    // The sole renderer owns echo as well as ordinary frames. Never wait on
    // output from the ingress reader; full output queues fail closed.
    fn queue_frame(&self, text: String) -> Result<mpsc::Receiver<Result<(), Failure>>, Failure> {
        let result = self.enqueue_frame(text);
        if result.is_err() {
            self.fail(Failure::Output);
        }
        result
    }
    // Does not acquire input or call fail, so readiness and marker ordering
    // can be published while holding the input metadata lock.
    fn enqueue_frame(&self, text: String) -> Result<mpsc::Receiver<Result<(), Failure>>, Failure> {
        let (tx, rx) = mpsc::channel();
        if self
            .queued_output
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| n.checked_add(1))
            .is_err()
            || self.output.try_send(Frame { text, ack: tx }).is_err()
        {
            return Err(Failure::Output);
        }
        Ok(rx)
    }
    pub(crate) fn emit(&self, text: &str) {
        let frame = text.split('\n').map(escaped).collect::<Vec<_>>().join("\n");
        let _queued = self.send(frame);
    }
    pub(crate) fn delivered(&self, text: &str, deadline: Instant) -> bool {
        let text = text.split('\n').map(escaped).collect::<Vec<_>>().join("\n");
        let Ok(ack) = self.send(text) else {
            return false;
        };
        match ack.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
            Ok(Ok(())) => !self.output_failed.load(Ordering::SeqCst),
            _ => {
                self.fail(Failure::Output);
                false
            }
        }
    }
    /// M-3a: a goal-mode line from the reader. Stop, Quit and EOF are the
    /// same ingress as in command mode; a goal never reaches the queue.
    pub(crate) fn goal_line(&self, line: GoalLine) {
        match line {
            GoalLine::Stop => self.receive(Line::Stop, false),
            GoalLine::Quit => self.receive(Line::Quit, false),
            GoalLine::Eof => self.receive(Line::Eof, true),
            GoalLine::Goal(goal) => {
                let mut input = self.input();
                if input.goal {
                    input.goal_line.get_or_insert(goal);
                }
                drop(input);
                self.changed.notify_all();
            }
        }
    }
    /// M-3a (R8): read one typed goal through the single reader, by
    /// `deadline`. The prompt is displayed, earlier input is invalidated at
    /// a boundary, then `INPUT READY`; the goal is validated by the goal-file
    /// grammar and displayed (T1, escaped, size, digest). Data-only: wired in
    /// M-5.
    #[allow(dead_code)]
    pub(crate) fn read_goal(&self, deadline: Instant) -> Result<TypedGoal, &'static str> {
        let epoch = self.stop.ingress.generation();
        if !self.goal_valid(deadline, epoch) {
            return Err("goal_unavailable");
        }
        let _dialogue = match self.dialogue.try_lock() {
            Ok(lock) => lock,
            Err(std::sync::TryLockError::WouldBlock) => return Err("goal_unavailable"),
            Err(std::sync::TryLockError::Poisoned(_)) => {
                self.fail(Failure::Input);
                return Err("goal_unavailable");
            }
        };
        self.goal_begin();
        let result = self.read_goal_inner(deadline, epoch);
        self.goal_end();
        result
    }
    /// M3E-R15: a goal read begins (not yet ready): its id is the next one.
    pub(crate) fn goal_begin(&self) -> u64 {
        let mut input = self.input();
        input.active = true;
        input.typing = false;
        input.goal = true;
        input.goal_line = None;
        input.goal_read = input.goal_read.wrapping_add(1);
        input.goal_read
    }
    /// The goal read is ready: `INPUT READY` is queued under the input lock.
    pub(crate) fn goal_ready(&self) -> Result<mpsc::Receiver<Result<(), Failure>>, Failure> {
        let mut input = self.input();
        let queued = self.enqueue_frame("INPUT READY\n".into());
        input.typing = queued.is_ok();
        queued
    }
    /// The goal read ended (returned, refused, interrupted or timed out).
    pub(crate) fn goal_end(&self) {
        let mut input = self.input();
        input.active = false;
        input.typing = false;
        input.goal = false;
        input.goal_line = None;
        drop(input);
        self.changed.notify_all();
    }
    /// The reader acknowledges `boundary`: earlier input is gone.
    pub(crate) fn acknowledge(&self, boundary: u64) {
        let mut input = self.input();
        input.lines.clear();
        input.acknowledged = boundary;
        self.changed.notify_all();
    }
    fn goal_valid(&self, deadline: Instant, epoch: u64) -> bool {
        let ingress = &self.stop.ingress;
        Instant::now() < deadline
            && epoch == ingress.generation()
            && !ingress.emergency_seen()
            && !ingress.failed()
            && matches!(
                ingress.state(),
                SessionState::Bootstrap | SessionState::Normal
            )
            && !self.ended()
            && self.input().quit.is_none()
    }
    fn goal_ended(&self, deadline: Instant) -> &'static str {
        if Instant::now() >= deadline {
            "goal_timed_out"
        } else {
            "goal_interrupted"
        }
    }
    fn read_goal_inner(&self, deadline: Instant, epoch: u64) -> Result<TypedGoal, &'static str> {
        let Ok(ack) = self.send(GOAL_PROMPT.into()) else {
            return Err("goal_unavailable");
        };
        loop {
            if !self.goal_valid(deadline, epoch) {
                return Err(self.goal_ended(deadline));
            }
            match ack.recv_timeout(SLICE.min(deadline.saturating_duration_since(Instant::now()))) {
                Ok(Ok(())) => break,
                Ok(Err(e)) => {
                    self.fail(e);
                    return Err("goal_unavailable");
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(_) => {
                    self.fail(Failure::Output);
                    return Err("goal_unavailable");
                }
            }
        }
        let boundary = {
            let mut input = self.input();
            let Some(next) = input.boundary.checked_add(1) else {
                drop(input);
                self.fail(Failure::Capacity);
                return Err("goal_unavailable");
            };
            input.boundary = next;
            self.changed.notify_all();
            next
        };
        loop {
            if !self.goal_valid(deadline, epoch) {
                return Err(self.goal_ended(deadline));
            }
            if self.input().acknowledged == boundary {
                break;
            }
            self.wait();
        }
        if self.goal_ready().is_err() {
            self.fail(Failure::Output);
            return Err("goal_unavailable");
        }
        loop {
            if !self.goal_valid(deadline, epoch) {
                return Err(self.goal_ended(deadline));
            }
            let line = self.input().goal_line.take();
            match line {
                Some(Ok(goal)) => {
                    // Stop, Quit or EOF in the same read was seen first.
                    if !self.goal_valid(deadline, epoch) {
                        return Err(self.goal_ended(deadline));
                    }
                    if self.send(goal_display(&goal)).is_err() {
                        return Err("goal_unavailable");
                    }
                    return Ok(TypedGoal::new(goal));
                }
                Some(Err(e)) => {
                    let _queued = self.send(format!("GOAL REFUSED {}\n", e.code()));
                    return Err(e.code());
                }
                None => self.wait(),
            }
        }
    }
    pub(crate) fn command_if_idle(&self, work_idle: bool) -> Option<Command> {
        let mut input = self.input();
        if input.active {
            return None;
        }
        if work_idle {
            return input.commands.pop_front();
        }
        let at = input
            .commands
            .iter()
            .position(|command| command.name == "status" || command.refusal.is_some())?;
        input.commands.remove(at)
    }
    pub(crate) fn ended(&self) -> bool {
        let input = self.input();
        input.eof || input.fatal.is_some()
    }
    pub(crate) fn wait(&self) {
        let _wait = self.changed.wait_timeout(self.input(), SLICE);
    }
    fn next(&self) -> Option<u64> {
        match self
            .sequence
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| n.checked_add(1))
        {
            Ok(n) if n < u64::MAX - 1 => Some(n + 1),
            _ => {
                self.stop.ingress.fail();
                None
            }
        }
    }
    pub(crate) fn receive(&self, line: Line, discard: bool) {
        match line {
            Line::Stop => {
                self.stop.press();
                self.changed.notify_all();
            }
            Line::Quit => {
                // Closing precedes the input metadata lock and queue access.
                let request = self
                    .next()
                    .and_then(|seq| self.stop.ingress.accept(CommandKind::Quit, seq).ok());
                if let Some(request) = request {
                    self.input().quit.get_or_insert(request);
                } else {
                    self.fail(Failure::Capacity);
                }
                self.changed.notify_all();
            }
            Line::Eof => {
                self.stop.press();
                self.stop.ingress.fail();
                self.input().eof = true;
                self.changed.notify_all();
            }
            Line::Answer(text) => {
                if text.as_deref() == Some("") {
                    return;
                }
                let Some(next) = self.next() else {
                    self.fail(Failure::Capacity);
                    return;
                };
                let mut input = self.input();
                if !discard {
                    if input.lines.len() + input.commands.len() >= QUEUE {
                        drop(input);
                        self.fail(Failure::Capacity);
                        return;
                    }
                    let kind = match text.as_deref() {
                        Some("run") => Some(CommandKind::Run),
                        Some("approve") => Some(CommandKind::Approve),
                        Some("undo") => Some(CommandKind::Undo),
                        Some("cleanup") => Some(CommandKind::Cleanup),
                        Some("detail") => Some(CommandKind::Detail),
                        Some("status") => Some(CommandKind::Status),
                        Some("evidence") => Some(CommandKind::Evidence),
                        Some("lift-for-undo") => Some(CommandKind::Lift),
                        _ => None,
                    };
                    if input.active && kind.is_none() {
                        input.lines.push_back((next, text));
                    } else {
                        let name = text.unwrap_or_default();
                        let (request, refusal) = match kind {
                            Some(kind) => match self.stop.ingress.accept(kind, next) {
                                Ok(request) => (Some(request), None),
                                Err(_) => (
                                    None,
                                    Some(if kind == CommandKind::Lift {
                                        "unavailable"
                                    } else if self.stop.ingress.emergency_seen() {
                                        "recovery_only"
                                    } else {
                                        "request_unavailable"
                                    }),
                                ),
                            },
                            None => (None, Some("unknown_command")),
                        };
                        input.commands.push_back(Command {
                            name,
                            request,
                            refusal,
                        });
                    }
                }
                self.changed.notify_all();
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub(crate) enum AnswerState {
    NotAsked,
    Awaiting,
    Declined,
    Affirmative,
    Invalidated,
    ProtocolFailed,
}
#[derive(Clone, Copy)]
struct RequestFacts {
    sequence: u64,
    generation: u64,
    accepted: Instant,
    deadline: Instant,
}
/// The one production terminal confirmer. Unknown requests and recovery
/// acknowledgement deny. The broker's binding remains authoritative.
pub struct TerminalConfirmer {
    pub(crate) shared: Arc<Shared>,
    facts: Option<RequestFacts>,
    answer: AtomicU8,
    answers: [AtomicU8; 5],
}
impl TerminalConfirmer {
    pub(crate) fn new(shared: Arc<Shared>) -> Self {
        Self {
            shared,
            facts: None,
            answer: AtomicU8::new(AnswerState::NotAsked as u8),
            answers: std::array::from_fn(|_| AtomicU8::new(AnswerState::NotAsked as u8)),
        }
    }
    pub(crate) fn for_request(&self, request: &SessionRequest) -> Arc<Self> {
        let seconds = if matches!(request.kind(), CommandKind::Run | CommandKind::Approve) {
            300
        } else {
            120
        };
        Arc::new(Self {
            shared: self.shared.clone(),
            facts: Some(RequestFacts {
                sequence: request.sequence(),
                generation: request.generation(),
                accepted: request.deadline() - Duration::from_secs(seconds),
                deadline: request.deadline(),
            }),
            answer: AtomicU8::new(AnswerState::NotAsked as u8),
            answers: std::array::from_fn(|_| AtomicU8::new(AnswerState::NotAsked as u8)),
        })
    }
    pub(crate) fn answers(&self) -> [AnswerState; 5] {
        std::array::from_fn(|index| Self::decode_answer(self.answers[index].load(Ordering::SeqCst)))
    }
    pub(crate) fn answer(&self) -> AnswerState {
        Self::decode_answer(self.answer.load(Ordering::SeqCst))
    }
    fn confirmation_answer(&self) -> ConfirmationAnswer {
        match self.answer() {
            AnswerState::NotAsked => ConfirmationAnswer::NotAsked,
            AnswerState::Awaiting => ConfirmationAnswer::Awaiting,
            AnswerState::Declined => ConfirmationAnswer::Declined,
            AnswerState::Affirmative => ConfirmationAnswer::Affirmative,
            AnswerState::Invalidated => ConfirmationAnswer::Invalidated,
            AnswerState::ProtocolFailed => ConfirmationAnswer::ProtocolFailed,
        }
    }
    fn decode_answer(value: u8) -> AnswerState {
        match value {
            0 => AnswerState::NotAsked,
            1 => AnswerState::Awaiting,
            2 => AnswerState::Declined,
            3 => AnswerState::Affirmative,
            4 => AnswerState::Invalidated,
            _ => AnswerState::ProtocolFailed,
        }
    }
    fn answered(&self, answer: AnswerState) {
        self.answer.store(answer as u8, Ordering::SeqCst);
    }
    #[cfg(test)]
    pub(crate) fn request_sequence(&self) -> Option<u64> {
        self.facts.map(|f| f.sequence)
    }
    #[cfg(test)]
    pub(crate) fn request_deadline(&self) -> Option<Instant> {
        self.facts.map(|f| f.deadline)
    }

    pub(crate) fn ask(
        &self,
        received: Instant,
        title: &str,
        text: ConfirmationText,
        seconds: u64,
        recovery: bool,
    ) -> bool {
        let ingress = &self.shared.stop.ingress;
        let (deadline, epoch) = match self.facts {
            Some(facts) => {
                debug_assert!(facts.sequence > 0);
                (
                    (facts.accepted + Duration::from_secs(seconds)).min(facts.deadline),
                    facts.generation,
                )
            }
            None => (
                received + Duration::from_secs(seconds),
                ingress.generation(),
            ),
        };
        if seconds == 0 || !self.valid(deadline, epoch, recovery) {
            self.answered(AnswerState::Invalidated);
            return false;
        }
        // try_lock denies overlapping dialogue rather than queuing a request
        // with an old lifetime behind another Owner decision.
        let _dialogue = match self.shared.dialogue.try_lock() {
            Ok(lock) => lock,
            Err(std::sync::TryLockError::WouldBlock) => {
                self.answered(AnswerState::ProtocolFailed);
                return false;
            }
            Err(std::sync::TryLockError::Poisoned(_)) => {
                self.answered(AnswerState::ProtocolFailed);
                self.shared.fail(Failure::Input);
                return false;
            }
        };
        {
            let mut input = self.shared.input();
            input.active = true;
            input.typing = false;
        }
        self.answered(AnswerState::Awaiting);
        let answer = self.ask_inner(title, text, deadline, epoch, recovery);
        if self.answer() == AnswerState::Awaiting {
            self.answered(if self.shared.ended() {
                AnswerState::ProtocolFailed
            } else {
                AnswerState::Invalidated
            });
        }
        let mut input = self.shared.input();
        input.active = false;
        // Preserve commands typed after this answer. A subsequent dialogue
        // independently invalidates its own queued answer input at its boundary.
        self.shared.changed.notify_all();
        answer
    }
    fn valid(&self, deadline: Instant, epoch: u64, recovery: bool) -> bool {
        let ingress = &self.shared.stop.ingress;
        let _ = recovery; // Hands has no recovery dialogue.
        Instant::now() < deadline
            && epoch == ingress.generation()
            && !ingress.emergency_seen()
            && !ingress.failed()
            && matches!(
                ingress.state(),
                SessionState::Bootstrap | SessionState::Normal
            )
    }
    fn protocol_valid(
        &self,
        deadline: Instant,
        epoch: u64,
        recovery: bool,
        failure: Failure,
    ) -> bool {
        if self.valid(deadline, epoch, recovery) {
            return true;
        }
        let ingress = &self.shared.stop.ingress;
        // An interrupted dialogue is normal denial. A terminal protocol that
        // cannot acknowledge within the original lifetime is a host failure.
        if Instant::now() >= deadline
            && ingress.generation() == epoch
            && ingress.close_deadline().is_none()
        {
            self.shared.fail(failure);
        }
        false
    }
    fn ask_inner(
        &self,
        title: &str,
        text: ConfirmationText,
        deadline: Instant,
        epoch: u64,
        recovery: bool,
    ) -> bool {
        let fixed = match render(title, &text, "") {
            Ok(f) => f,
            Err(e) => {
                self.shared.fail(e);
                return false;
            }
        };
        let code = fresh_code(&fixed, || {
            std::panic::catch_unwind(|| uuid::Uuid::new_v4().simple().to_string()[..8].to_string())
                .ok()
        });
        let Some(code) = code else {
            self.shared.fail(Failure::Randomness);
            return false;
        };
        let frame = match render(title, &text, &code) {
            Ok(f) => f,
            Err(e) => {
                self.shared.fail(e);
                return false;
            }
        };
        let Ok(ack) = self.shared.send(frame) else {
            return false;
        };
        loop {
            if !self.protocol_valid(deadline, epoch, recovery, Failure::Output) {
                return false;
            }
            match ack.recv_timeout(SLICE.min(deadline.saturating_duration_since(Instant::now()))) {
                Ok(Ok(())) => break,
                Ok(Err(e)) => {
                    self.shared.fail(e);
                    return false;
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(_) => {
                    self.shared.fail(Failure::Output);
                    return false;
                }
            }
        }
        let boundary = {
            let mut input = self.shared.input();
            let Some(next) = input.boundary.checked_add(1) else {
                drop(input);
                self.shared.fail(Failure::Capacity);
                return false;
            };
            input.boundary = next;
            self.shared.changed.notify_all();
            next
        };
        loop {
            if !self.protocol_valid(deadline, epoch, recovery, Failure::Boundary) {
                return false;
            }
            if self.shared.input().acknowledged == boundary {
                break;
            }
            self.shared.wait();
        }
        let ready = {
            let mut input = self.shared.input();
            // Queue the marker before permitting any answer echo. The reader
            // keeps invalidating partial input throughout the display gap.
            let queued = self.shared.enqueue_frame("INPUT READY\n".into());
            input.typing = queued.is_ok();
            queued
        };
        if ready.is_err() {
            self.shared.fail(Failure::Output);
            return false;
        }
        loop {
            if !self.valid(deadline, epoch, recovery) {
                return false;
            }
            let line = self.shared.input().lines.pop_front();
            if let Some((_, line)) = line {
                if !self.valid(deadline, epoch, recovery) {
                    self.answered(AnswerState::Invalidated);
                    return false;
                }
                match line.as_deref() {
                    Some(answer) if answer == code => {
                        self.answered(AnswerState::Affirmative);
                        return true;
                    }
                    Some("no") => {
                        self.answered(AnswerState::Declined);
                        return false;
                    }
                    _ => {
                        self.answered(AnswerState::ProtocolFailed);
                        self.shared.fail(Failure::Input);
                        return false;
                    }
                }
            }
            self.shared.wait();
        }
    }
    fn confirmed(
        &self,
        question: usize,
        title: &str,
        text: ConfirmationText,
        seconds: u64,
    ) -> bool {
        self.answered(AnswerState::NotAsked);
        self.answers[question].store(AnswerState::NotAsked as u8, Ordering::SeqCst);
        if self.facts.is_none() {
            return false;
        }
        self.answers[question].store(AnswerState::Awaiting as u8, Ordering::SeqCst);
        let result = self.ask(Instant::now(), title, text, seconds, false);
        self.answers[question].store(self.answer() as u8, Ordering::SeqCst);
        result
    }
    fn action(&self, r: &ActionConfirmation) -> bool {
        r.kind == CapabilityKind::Files
            && r.class == EffectClass::R2
            && r.operation == "create"
            && self.confirmed(3, r.title(), r.text(), r.expires_in_secs.min(300))
    }
    fn grant(&self, r: &GrantConfirmation) -> bool {
        self.answered(AnswerState::NotAsked);
        self.answers[0].store(AnswerState::NotAsked as u8, Ordering::SeqCst);
        r.kind == CapabilityKind::Files && self.confirmed(0, r.title(), r.text(), 120)
    }
    fn mission(&self, r: &MissionConfirmation) -> bool {
        self.confirmed(2, r.title(), r.text(), 300)
    }
    fn door(&self, r: &DoorPrompt) -> bool {
        self.confirmed(1, r.title(), r.text(), 120)
    }
    fn undo(&self, r: &UndoConfirmation) -> bool {
        self.confirmed(4, r.title(), r.text(), 120)
    }
}

impl ControlConfirmer for TerminalConfirmer {
    fn confirm_grant_answer(&self, request: &GrantConfirmation) -> ConfirmationAnswer {
        self.grant(request);
        self.confirmation_answer()
    }
    fn confirm_agent_door_answer(&self, request: &DoorPrompt) -> ConfirmationAnswer {
        self.door(request);
        self.confirmation_answer()
    }
    fn confirm_mission_answer(&self, request: &MissionConfirmation) -> ConfirmationAnswer {
        self.mission(request);
        self.confirmation_answer()
    }
    fn confirm_action(&self, request: &ActionConfirmation) -> bool {
        self.action(request)
    }
    fn confirm_grant(&self, request: &GrantConfirmation) -> bool {
        self.grant(request)
    }
    fn confirm_resume(&self, _: &ResumeConfirmation) -> bool {
        false
    }
    fn confirm_acknowledgement(&self, _: &AcknowledgementConfirmation) -> bool {
        false
    }
    fn confirm_mission(&self, request: &MissionConfirmation) -> bool {
        self.mission(request)
    }
    fn confirm_agent_door(&self, request: &DoorPrompt) -> bool {
        self.door(request)
    }
    fn confirm_held_mission(&self, _: &MissionResumeConfirmation) -> bool {
        false
    }
    fn confirm_undo(&self, request: &UndoConfirmation) -> bool {
        self.undo(request)
    }
}

/// Owns terminal modes, reader and renderer. A caller must perform its
/// backend shutdown first, then finish these workers within the same deadline.
pub struct Terminal {
    pub(crate) shared: Arc<Shared>,
    pub confirmer: Arc<TerminalConfirmer>,
    reader: Option<JoinHandle<()>>,
    writer: Option<JoinHandle<()>>,
    reporter: Option<JoinHandle<()>>,
    reporter_end: Arc<AtomicBool>,
    modes: Option<(Termios, Termios)>,
    finish_deadline: Option<Instant>,
    render_crlf: Arc<AtomicBool>,
    finalization_ok: bool,
    reader_debt: Option<Producer>,
    reporter_debt: Option<Producer>,
}
impl Terminal {
    pub fn open(ingress: SessionIngress) -> Result<Self, Failure> {
        let stdin = std::io::stdin();
        let stdout = std::io::stdout();
        if !stdin.is_terminal() || !stdout.is_terminal() {
            return Err(Failure::TerminalRequired);
        }
        let before_in = termios::tcgetattr(stdin.as_fd()).map_err(|_| Failure::TerminalControl)?;
        let before_out =
            termios::tcgetattr(stdout.as_fd()).map_err(|_| Failure::TerminalControl)?;
        let mut input = before_in.clone();
        input
            .local_flags
            .remove(LocalFlags::ICANON | LocalFlags::ECHO | LocalFlags::ECHONL);
        input
            .input_flags
            .remove(InputFlags::ICRNL | InputFlags::INLCR | InputFlags::IGNCR | InputFlags::IXON);
        input.output_flags.remove(OutputFlags::OPOST);
        input.control_chars[SpecialCharacterIndices::VMIN as usize] = 1;
        input.control_chars[SpecialCharacterIndices::VTIME as usize] = 0;
        termios::tcsetattr(stdin.as_fd(), SetArg::TCSANOW, &input)
            .map_err(|_| Failure::TerminalControl)?;
        let mut output = match termios::tcgetattr(stdout.as_fd()) {
            Ok(output) => output,
            Err(_) => {
                let _restored = termios::tcsetattr(stdin.as_fd(), SetArg::TCSANOW, &before_in);
                return Err(Failure::TerminalControl);
            }
        };
        output.output_flags.remove(OutputFlags::OPOST);
        if termios::tcsetattr(stdout.as_fd(), SetArg::TCSANOW, &output).is_err() {
            let _restored = termios::tcsetattr(stdin.as_fd(), SetArg::TCSANOW, &before_in);
            return Err(Failure::TerminalControl);
        }
        let reader_debt = ingress
            .producer(ProducerKind::Input)
            .map_err(|_| Failure::Thread)?;
        let reporter_debt = ingress
            .producer(ProducerKind::Reporter)
            .map_err(|_| Failure::Thread)?;
        let stop = StopRoute::new(ingress);
        let (tx, rx) = mpsc::sync_channel::<Frame>(QUEUE);
        let shared = Arc::new(Shared {
            stop,
            input: Mutex::default(),
            sequence: AtomicU64::new(0),
            queued_output: AtomicU64::new(0),
            delivered_output: AtomicU64::new(0),
            output_failed: AtomicBool::new(false),
            changed: Condvar::new(),
            output: tx,
            end: AtomicBool::new(false),
            input_end: AtomicBool::new(false),
            dialogue: Mutex::new(()),
        });
        let mut terminal = Self {
            confirmer: Arc::new(TerminalConfirmer::new(shared.clone())),
            shared,
            reader: None,
            writer: None,
            reporter: None,
            reporter_end: Arc::new(AtomicBool::new(false)),
            modes: Some((before_in, before_out)),
            finish_deadline: None,
            render_crlf: Arc::new(AtomicBool::new(true)),
            finalization_ok: true,
            reader_debt: Some(reader_debt),
            reporter_debt: Some(reporter_debt),
        };
        let s = terminal.shared.clone();
        let render_crlf = terminal.render_crlf.clone();
        terminal.writer = Some(
            std::thread::Builder::new()
                .name("hands-screen".into())
                .spawn(move || {
                    let stdout = std::io::stdout();
                    loop {
                        match rx.recv_timeout(SLICE) {
                            Ok(frame) => {
                                // Release stdout before waiting for another frame;
                                // the walk also writes ordinary progress lines.
                                let result = if render_crlf.load(Ordering::SeqCst) {
                                    write_frame(&mut stdout.lock(), &frame.text)
                                } else {
                                    let mut out = stdout.lock();
                                    out.write_all(frame.text.as_bytes())
                                        .and_then(|()| out.flush())
                                        .map_err(|_| Failure::Output)
                                };
                                if result.is_ok() {
                                    s.delivered_output.fetch_add(1, Ordering::SeqCst);
                                }
                                let _ack = frame.ack.send(result);
                                if let Err(e) = result {
                                    s.fail(e);
                                    break;
                                }
                            }
                            Err(mpsc::RecvTimeoutError::Timeout)
                                if !s.end.load(Ordering::SeqCst) => {}
                            Err(_) => break,
                        }
                    }
                })
                .map_err(|_| Failure::Thread)?,
        );
        let s = terminal.shared.clone();
        let end = terminal.reporter_end.clone();
        terminal.reporter = Some(
            std::thread::Builder::new()
                .name("hands-stop-receipts".into())
                .spawn(move || crate::stop::report_receipts(s, end))
                .map_err(|_| Failure::Thread)?,
        );
        let s = terminal.shared.clone();
        terminal.reader = Some(
            std::thread::Builder::new()
                .name("hands-input".into())
                .spawn(move || read_input(s))
                .map_err(|_| Failure::Thread)?,
        );
        Ok(terminal)
    }
    pub(crate) fn quiesce(&mut self, deadline: Instant) -> bool {
        let mut ok = self.stop_reader(deadline);
        self.reporter_end.store(true, Ordering::SeqCst);
        self.shared.changed.notify_all();
        while self.reporter.as_ref().is_some_and(|h| !h.is_finished()) && Instant::now() < deadline
        {
            self.shared.wait();
        }
        if self.reporter.as_ref().is_some_and(|h| h.is_finished()) {
            ok &= self.reporter.take().is_some_and(|h| h.join().is_ok());
            if let Some(debt) = self.reporter_debt.take() {
                debt.joined();
            }
        } else if self.reporter.is_some() {
            ok = false;
        }
        ok && self.shared.stop.delivery_debt().is_empty()
            && self.shared.delivered("", deadline)
            && self.shared.queued_output.load(Ordering::SeqCst)
                == self.shared.delivered_output.load(Ordering::SeqCst)
    }
    pub fn finish(&mut self, deadline: Instant) -> bool {
        let deadline = *self.finish_deadline.get_or_insert(deadline);
        let reader_ok = self.stop_reader(deadline);
        // Drain the final receipt snapshot before allowing the renderer to end.
        self.reporter_end.store(true, Ordering::SeqCst);
        self.shared.changed.notify_all();
        while self.reporter.as_ref().is_some_and(|h| !h.is_finished()) && Instant::now() < deadline
        {
            self.shared.wait();
        }
        self.shared.end.store(true, Ordering::SeqCst);
        self.shared.changed.notify_all();
        loop {
            let done = [&self.reader, &self.reporter, &self.writer]
                .iter()
                .all(|h| h.as_ref().is_none_or(|h| h.is_finished()));
            if done || Instant::now() >= deadline {
                break;
            }
            self.shared.wait();
        }
        let mut ok = reader_ok;
        for worker in [&mut self.reader, &mut self.reporter, &mut self.writer] {
            if worker.as_ref().is_some_and(|h| h.is_finished()) {
                if let Some(handle) = worker.take() {
                    ok &= handle.join().is_ok();
                }
            } else if worker.is_some() {
                ok = false;
            }
        }
        if self.reporter.is_none() {
            if let Some(debt) = self.reporter_debt.take() {
                debt.joined();
            }
        }
        let modes_ok = self.restore_modes(deadline);
        ok && modes_ok
            && !self.shared.output_failed.load(Ordering::SeqCst)
            && Instant::now() < deadline
    }
    /// Input and ordinary output have quiesced. Restore modes before any
    /// successful close receipt; the reserved renderer remains for delivery.
    pub(crate) fn restore_modes(&mut self, deadline: Instant) -> bool {
        if let Some((input, output)) = self.modes.take() {
            self.finalization_ok &= Instant::now() < deadline;
            self.finalization_ok &=
                termios::tcsetattr(std::io::stdin().as_fd(), SetArg::TCSANOW, &input).is_ok();
            let restored =
                termios::tcsetattr(std::io::stdout().as_fd(), SetArg::TCSANOW, &output).is_ok();
            self.finalization_ok &= restored;
            if restored {
                let translated = output
                    .output_flags
                    .contains(OutputFlags::OPOST | OutputFlags::ONLCR);
                self.render_crlf.store(!translated, Ordering::SeqCst);
            }
        }
        self.finalization_ok && Instant::now() < deadline
    }
    pub(crate) fn stop_reader(&mut self, deadline: Instant) -> bool {
        self.shared.input_end.store(true, Ordering::SeqCst);
        while self.reader.as_ref().is_some_and(|h| !h.is_finished()) && Instant::now() < deadline {
            self.shared.wait();
        }
        if self.reader.as_ref().is_some_and(|h| h.is_finished()) {
            let ok = self.reader.take().is_some_and(|h| h.join().is_ok());
            if let Some(debt) = self.reader_debt.take() {
                debt.joined();
            }
            ok
        } else {
            self.reader.is_none()
        }
    }
}
impl Drop for Terminal {
    fn drop(&mut self) {
        if self.modes.is_some() {
            self.shared.stop.ingress.fail();
            if let Some(deadline) = self.shared.stop.ingress.close_deadline() {
                let _joined = self.finish(deadline);
            }
        }
    }
}

/// The single reader's two line buffers: commands and codes (64 bytes) and
/// the typed goal (M-3a). They are storage only (M3E-R13): the line's owner
/// and the CR flag are the stream's.
#[derive(Default)]
pub(crate) struct Buffers {
    pub(crate) line: LineBuffer,
    pub(crate) goal: GoalBuffer,
    owner: Owner,
    last_was_cr: bool,
    /// M3E-R15 test seam: runs on the reader at each [`HookPoint`].
    #[cfg(test)]
    pub(crate) hook: Option<Box<dyn FnMut(HookPoint) + Send>>,
}
/// Where the reader stops for a test: after its mode snapshot, before each
/// byte, and before a completed goal line is published.
#[cfg(test)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum HookPoint {
    Start,
    Byte(usize),
    Publish,
}
/// M3E-R13: who owns the line under way. Set by its first byte, cleared only
/// by its CR, LF or EOF; never derived from buffer contents.
#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum Owner {
    #[default]
    None,
    Command,
    Goal(u64),
    DiscardGoal,
    DiscardCommand,
}
impl Buffers {
    #[cfg(test)]
    fn fire(&mut self, point: HookPoint) {
        if let Some(hook) = self.hook.as_mut() {
            hook(point);
        }
    }
    /// A boundary: the line under way can no longer act as live input.
    pub(crate) fn invalidate(&mut self) {
        self.owner = match self.owner {
            Owner::Command => Owner::DiscardCommand,
            Owner::Goal(_) => Owner::DiscardGoal,
            other => other,
        };
    }
}

/// The reader's mode for one read (M3E-R15): whether its bytes are
/// discarded, and the id of the goal read that is ready (has displayed
/// `INPUT READY`), if any. One snapshot, under one lock.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ReaderMode {
    pub(crate) discard: bool,
    pub(crate) read: Option<u64>,
}
pub(crate) fn reader_state(shared: &Shared, boundary: Option<u64>) -> ReaderMode {
    let input = shared.input();
    let discard = boundary.is_some()
        || input.boundary != input.acknowledged
        || (input.active && !input.typing);
    let ready = !discard && input.goal && input.active && input.typing;
    ReaderMode {
        discard,
        read: ready.then_some(input.goal_read),
    }
}
/// Test form of [`reader_state`]: whether discarded, and whether goal input.
#[cfg(test)]
pub(crate) fn reader_mode(shared: &Shared, boundary: Option<u64>) -> (bool, bool) {
    let mode = reader_state(shared, boundary);
    (mode.discard, mode.read.is_some())
}
/// Test form of [`route_read`]: goal input is the current goal read's.
#[cfg(test)]
pub(crate) fn route(
    shared: &Shared,
    buffers: &mut Buffers,
    bytes: &[u8],
    discard: bool,
    goal: bool,
) {
    let read = goal.then(|| shared.input().goal_read);
    route_read(shared, buffers, bytes, ReaderMode { discard, read });
}

/// One read's bytes through the buffers. A line finishes with the owner its
/// first byte gave it. After a goal-mode line the rest of the read is
/// discarded input (Stop, Quit and EOF still act).
pub(crate) fn route_read(shared: &Shared, buffers: &mut Buffers, bytes: &[u8], mode: ReaderMode) {
    #[cfg(test)]
    buffers.fire(HookPoint::Start);
    let mut discard = mode.discard;
    if discard {
        buffers.invalidate();
    }
    let mut read = mode.read.map(|_| shared.input().goal_read);
    let mut echo = String::new();
    let mut pending = None;
    for (_i, &b) in bytes.iter().enumerate() {
        #[cfg(test)]
        buffers.fire(HookPoint::Byte(_i));
        // A LF directly after a CR on the stream is that CR's partner.
        let partner = b == b'\n' && buffers.last_was_cr;
        buffers.last_was_cr = b == b'\r';
        if partner {
            continue;
        }
        if b == 4 {
            buffers.owner = Owner::None;
            buffers.line = LineBuffer::default();
            buffers.goal = GoalBuffer::default();
            shared.receive(Line::Eof, true);
            continue;
        }
        let owner = match buffers.owner {
            Owner::None if discard => Owner::DiscardCommand,
            Owner::None => read.map_or(Owner::Command, Owner::Goal),
            Owner::Command if discard || read.is_some() => Owner::DiscardCommand,
            Owner::Goal(r) if read != Some(r) => Owner::DiscardGoal,
            other => other,
        };
        let end = b == b'\r' || b == b'\n';
        buffers.owner = if end { Owner::None } else { owner };
        // The buffers see one terminator: pairing is the stream's.
        let b = if end { b'\n' } else { b };
        if let Owner::Goal(_) | Owner::DiscardGoal = owner {
            let live = owner != Owner::DiscardGoal;
            if live {
                buffers.goal.echo(b, &mut echo);
            }
            if let Some(line) = buffers.goal.push(b) {
                match line {
                    // Delivered after the rest of this read, so a Stop, Quit
                    // or EOF typed with it is seen first.
                    GoalLine::Goal(result) if live => pending = Some(result),
                    GoalLine::Goal(_) => {}
                    other => shared.goal_line(other),
                }
                if live {
                    read = None;
                    discard = true;
                }
            }
            continue;
        }
        let line_discard = owner == Owner::DiscardCommand;
        if !line_discard {
            buffers.line.echo(b, &mut echo);
        }
        if let Some(line) = buffers.line.push(b) {
            // Emergency/close ingress always precedes echo queueing.
            // Ordinary lines are echoed before waking their consumer.
            if matches!(line, Line::Answer(_)) && !echo.is_empty() {
                let _queued = shared.queue_frame(std::mem::take(&mut echo));
            }
            shared.receive(line, line_discard);
        }
    }
    if !echo.is_empty() {
        let _queued = shared.queue_frame(echo);
    }
    #[cfg(test)]
    buffers.fire(HookPoint::Publish);
    if let Some(result) = pending {
        shared.goal_line(GoalLine::Goal(result));
    }
}

fn read_input(shared: Arc<Shared>) {
    let stdin = std::io::stdin();
    let mut buffers = Buffers::default();
    while !shared.input_end.load(Ordering::SeqCst) {
        let boundary = {
            let input = shared.input();
            (input.boundary != input.acknowledged).then_some(input.boundary)
        };
        if boundary.is_some() {
            buffers.invalidate();
        }
        let mut fds = [PollFd::new(stdin.as_fd(), PollFlags::POLLIN)];
        let ready = match poll(&mut fds, if boundary.is_some() { 0u16 } else { 10u16 }) {
            Ok(n) => n,
            Err(nix::errno::Errno::EINTR) => continue,
            Err(_) => {
                shared.stop.press();
                shared.fail(Failure::Input);
                break;
            }
        };
        if ready == 0 {
            if let Some(boundary) = boundary {
                shared.acknowledge(boundary);
            }
            continue;
        }
        let mut bytes = [0u8; 256];
        match nix::unistd::read(stdin.as_raw_fd(), &mut bytes) {
            Ok(0) => {
                shared.receive(Line::Eof, true);
                break;
            }
            Ok(n) => {
                let mode = reader_state(&shared, boundary);
                route_read(&shared, &mut buffers, &bytes[..n], mode);
                if shared.ended() {
                    break;
                }
            }
            Err(nix::errno::Errno::EINTR) => {}
            Err(_) => {
                shared.stop.press();
                shared.fail(Failure::Input);
                break;
            }
        }
    }
}

/// Acknowledgement follows both complete write and flush.
pub(crate) fn write_frame(out: &mut impl Write, text: &str) -> Result<(), Failure> {
    // OPOST is disabled; trusted frame separators alone become CRLF.
    // Untrusted CR has already been escaped by the renderer.
    out.write_all(text.replace('\n', "\r\n").as_bytes())
        .and_then(|()| out.flush())
        .map_err(|_| Failure::Output)
}

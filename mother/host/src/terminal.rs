//! Linux terminal protocol: one reader, one renderer and one active dialogue.
//! The reader owns freshness acknowledgement; backend ingress is atomic only.
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
            let answer = if !self.overflow && self.bytes == b"stop" {
                Some(Line::Stop)
            } else if !self.overflow && self.bytes == b"quit" {
                Some(Line::Quit)
            } else if self.stale {
                None
            } else if self.overflow {
                Some(Line::Answer(None))
            } else {
                Some(Line::Answer(String::from_utf8(self.bytes.clone()).ok()))
            };
            self.bytes.clear();
            self.overflow = false;
            self.stale = false;
            return answer;
        }
        if self.bytes.len() < 64 {
            self.bytes.push(byte);
        } else {
            self.overflow = true;
        }
        None
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
        let (tx, rx) = mpsc::channel();
        if self
            .queued_output
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| n.checked_add(1))
            .is_err()
            || self.output.try_send(Frame { text, ack: tx }).is_err()
        {
            self.fail(Failure::Output);
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
        self.shared.input().active = true;
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
        self.shared.emit("INPUT READY\n");
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

fn read_input(shared: Arc<Shared>) {
    let stdin = std::io::stdin();
    let mut buffer = LineBuffer::default();
    while !shared.input_end.load(Ordering::SeqCst) {
        let boundary = {
            let input = shared.input();
            (input.boundary != input.acknowledged).then_some(input.boundary)
        };
        if boundary.is_some() {
            buffer.invalidate();
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
                let mut input = shared.input();
                input.lines.clear();
                input.acknowledged = boundary;
                shared.changed.notify_all();
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
                for b in &bytes[..n] {
                    if let Some(line) = buffer.push(*b) {
                        shared.receive(line, boundary.is_some());
                    }
                }
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

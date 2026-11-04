//! Linux terminal protocol: one reader, one renderer and one active dialogue.
//! The reader owns freshness acknowledgement and never calls the backend.
use crate::stop::{Gate, StopRoute};
use nexus_governed_control::authority::approval::{
    AcknowledgementConfirmation, ActionConfirmation, AgentDoorConfirmation, ConfirmationText,
    ControlConfirmer, GrantConfirmation, MissionConfirmation, MissionResumeConfirmation,
    ResumeConfirmation, UndoConfirmation,
};
use nexus_governed_control::authority::effect::{CapabilityKind, EffectClass};
use nexus_governed_control::authority::evidence::{escaped, is_plain, shown};
use nix::poll::{poll, PollFd, PollFlags};
use nix::sys::termios::{
    self, InputFlags, LocalFlags, OutputFlags, SetArg, SpecialCharacterIndices, Termios,
};
use std::collections::VecDeque;
use std::io::{IsTerminal, Write};
use std::os::fd::{AsFd, AsRawFd};
use std::sync::atomic::{AtomicBool, Ordering};
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
    Eof,
    Answer(Option<String>),
}
#[derive(Default)]
pub(crate) struct LineBuffer {
    bytes: Vec<u8>,
    overflow: bool,
    stale: bool,
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
        if byte == b'\n' {
            let answer = if !self.overflow && self.bytes == b"stop" {
                Some(Line::Stop)
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

#[derive(Default)]
pub(crate) struct Input {
    pub(crate) sequence: u64,
    pub(crate) lines: VecDeque<(u64, Option<String>)>,
    pub(crate) boundary: u64,
    pub(crate) acknowledged: u64,
    pub(crate) active: bool,
    pub(crate) fatal: Option<Failure>,
    pub(crate) eof: bool,
}
pub(crate) struct Frame {
    pub(crate) text: String,
    pub(crate) ack: mpsc::Sender<Result<(), Failure>>,
}
pub(crate) struct Shared {
    pub(crate) stop: Arc<StopRoute>,
    pub(crate) input: Mutex<Input>,
    pub(crate) changed: Condvar,
    pub(crate) output: mpsc::SyncSender<Frame>,
    pub(crate) end: AtomicBool,
    pub(crate) input_end: AtomicBool,
    pub(crate) dialogue: Mutex<()>,
}
impl Shared {
    pub(crate) fn input(&self) -> MutexGuard<'_, Input> {
        self.input.lock().unwrap_or_else(|p| {
            self.stop.gate.fail();
            let mut input = p.into_inner();
            input.fatal = Some(Failure::Input);
            input
        })
    }
    fn fail(&self, failure: Failure) {
        self.stop.gate.fail();
        self.input().fatal.get_or_insert(failure);
        self.changed.notify_all();
    }
    fn send(&self, text: String) -> Result<mpsc::Receiver<Result<(), Failure>>, Failure> {
        if text.chars().any(|c| c != '\n' && !shown(c)) {
            self.fail(Failure::Renderer);
            return Err(Failure::Renderer);
        }
        let (tx, rx) = mpsc::channel();
        if self.output.try_send(Frame { text, ack: tx }).is_err() {
            self.fail(Failure::Output);
            return Err(Failure::Output);
        }
        Ok(rx)
    }
    pub(crate) fn emit(&self, text: &str) {
        // Every arbitrary value is escaped, line by line; fixed newlines alone
        // delimit output. This also bounds combining stacks in lane text.
        let frame = text.split('\n').map(escaped).collect::<Vec<_>>().join("\n");
        let _queued = self.send(frame);
    }
    pub(crate) fn command(&self) -> Option<String> {
        let mut input = self.input();
        if input.active {
            return None;
        }
        input
            .lines
            .pop_front()
            .map(|(_, text)| text.unwrap_or_default())
    }
    pub(crate) fn ended(&self) -> bool {
        let input = self.input();
        input.eof || input.fatal.is_some()
    }
    pub(crate) fn wait(&self) {
        let _wait = self.changed.wait_timeout(self.input(), SLICE);
    }
    pub(crate) fn receive(&self, line: Line, discard: bool) {
        match line {
            Line::Stop => {
                self.stop.press();
                self.changed.notify_all();
            }
            Line::Eof => {
                self.stop.press();
                self.input().eof = true;
                self.stop.gate.close();
                self.changed.notify_all();
            }
            Line::Answer(text) => {
                let mut input = self.input();
                let Some(next) = input.sequence.checked_add(1) else {
                    drop(input);
                    self.fail(Failure::Capacity);
                    return;
                };
                input.sequence = next;
                if !discard {
                    if input.lines.len() >= QUEUE {
                        drop(input);
                        self.fail(Failure::Capacity);
                        return;
                    }
                    input.lines.push_back((next, text));
                }
                self.changed.notify_all();
            }
        }
    }
}

/// The one production terminal confirmer. Unknown requests and recovery
/// acknowledgement deny. The broker's binding remains authoritative.
pub struct TerminalConfirmer {
    pub(crate) shared: Arc<Shared>,
}
impl TerminalConfirmer {
    pub(crate) fn ask(
        &self,
        received: Instant,
        title: &str,
        text: ConfirmationText,
        seconds: u64,
        recovery: bool,
    ) -> bool {
        let deadline = received + Duration::from_secs(seconds);
        let gate = &self.shared.stop.gate;
        let epoch = gate.epoch();
        if seconds == 0 || gate.closing() || gate.failed() || (!recovery && gate.recovery()) {
            return false;
        }
        // try_lock denies overlapping dialogue rather than queuing a request
        // with an old lifetime behind another Owner decision.
        let _dialogue = match self.shared.dialogue.try_lock() {
            Ok(lock) => lock,
            Err(std::sync::TryLockError::WouldBlock) => return false,
            Err(std::sync::TryLockError::Poisoned(_)) => {
                self.shared.fail(Failure::Input);
                return false;
            }
        };
        self.shared.input().active = true;
        let answer = self.ask_inner(title, text, deadline, epoch, recovery);
        let mut input = self.shared.input();
        input.active = false;
        // Preserve commands typed after this answer. A subsequent dialogue
        // independently invalidates its own queued answer input at its boundary.
        self.shared.changed.notify_all();
        answer
    }
    fn valid(&self, deadline: Instant, epoch: u64, recovery: bool) -> bool {
        let gate = &self.shared.stop.gate;
        Instant::now() < deadline
            && epoch == gate.epoch()
            && !gate.closing()
            && !gate.failed()
            && (recovery || !gate.recovery())
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
            if !self.valid(deadline, epoch, recovery) {
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
            if !self.valid(deadline, epoch, recovery) {
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
                return line.as_deref() == Some(code.as_str())
                    && self.valid(deadline, epoch, recovery);
            }
            self.shared.wait();
        }
    }
    fn action(&self, r: &ActionConfirmation) -> bool {
        r.kind == CapabilityKind::Files
            && r.class == EffectClass::R2
            && r.operation == "create"
            && self.ask(
                Instant::now(),
                r.title(),
                r.text(),
                r.expires_in_secs.min(300),
                false,
            )
    }
    fn grant(&self, r: &GrantConfirmation) -> bool {
        r.kind == CapabilityKind::Files && self.ask(Instant::now(), r.title(), r.text(), 120, false)
    }
    fn mission(&self, r: &MissionConfirmation) -> bool {
        self.ask(Instant::now(), r.title(), r.text(), 300, false)
    }
    fn door(&self, r: &AgentDoorConfirmation) -> bool {
        self.ask(Instant::now(), r.title(), r.text(), 120, false)
    }
    fn held(&self, r: &MissionResumeConfirmation) -> bool {
        self.ask(Instant::now(), r.title(), r.text(), 300, false)
    }
    fn undo(&self, r: &UndoConfirmation) -> bool {
        self.ask(Instant::now(), r.title(), r.text(), 120, true)
    }
    fn resume(&self, r: &ResumeConfirmation) -> bool {
        self.shared.stop.gate.recovery() && self.shared.stop.receipt().outstanding == 0
            && self.ask(Instant::now(), "Recovery only; missions remain stopped", ConfirmationText {
                header: vec![("Stop number", r.stop_number.to_string()), ("Runs cancelled", r.runs_cancelled_by_the_stop.to_string())],
                details: vec!["Only lift the core halt for Owner undo. Missions remain stopped for this process.".into()],
            }, 120, true)
    }
}
impl ControlConfirmer for TerminalConfirmer {
    fn confirm_action(&self, request: &ActionConfirmation) -> bool {
        self.action(request)
    }
    fn confirm_grant(&self, request: &GrantConfirmation) -> bool {
        self.grant(request)
    }
    fn confirm_resume(&self, request: &ResumeConfirmation) -> bool {
        self.resume(request)
    }
    fn confirm_acknowledgement(&self, _: &AcknowledgementConfirmation) -> bool {
        false
    }
    fn confirm_mission(&self, request: &MissionConfirmation) -> bool {
        self.mission(request)
    }
    fn confirm_agent_door(&self, request: &AgentDoorConfirmation) -> bool {
        self.door(request)
    }
    fn confirm_held_mission(&self, request: &MissionResumeConfirmation) -> bool {
        self.held(request)
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
    modes: Option<(Termios, Termios)>,
}
impl Terminal {
    pub fn open() -> Result<Self, Failure> {
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
        let gate = Arc::new(Gate::default());
        let stop = StopRoute::new(gate);
        let (tx, rx) = mpsc::sync_channel::<Frame>(QUEUE);
        let shared = Arc::new(Shared {
            stop,
            input: Mutex::default(),
            changed: Condvar::new(),
            output: tx,
            end: AtomicBool::new(false),
            input_end: AtomicBool::new(false),
            dialogue: Mutex::new(()),
        });
        let mut terminal = Self {
            confirmer: Arc::new(TerminalConfirmer {
                shared: shared.clone(),
            }),
            shared,
            reader: None,
            writer: None,
            modes: Some((before_in, before_out)),
        };
        let s = terminal.shared.clone();
        terminal.writer = Some(
            std::thread::Builder::new()
                .name("hands-screen".into())
                .spawn(move || {
                    let stdout = std::io::stdout();
                    let mut out = stdout.lock();
                    loop {
                        match rx.recv_timeout(SLICE) {
                            Ok(frame) => {
                                let result = write_frame(&mut out, &frame.text);
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
        terminal.reader = Some(
            std::thread::Builder::new()
                .name("hands-input".into())
                .spawn(move || read_input(s))
                .map_err(|_| Failure::Thread)?,
        );
        Ok(terminal)
    }
    pub fn bind_stop(&self, owner: Arc<nexus_governed_control::governed::GovernedControl>) -> bool {
        self.shared.stop.bind(owner)
    }
    pub fn finish(&mut self, deadline: Instant) -> bool {
        let reader_ok = self.stop_reader(deadline);
        self.shared.end.store(true, Ordering::SeqCst);
        self.shared.changed.notify_all();
        loop {
            let done = [&self.reader, &self.writer]
                .iter()
                .all(|h| h.as_ref().is_none_or(|h| h.is_finished()));
            if done || Instant::now() >= deadline {
                break;
            }
            self.shared.wait();
        }
        let mut ok = reader_ok;
        for worker in [&mut self.reader, &mut self.writer] {
            if worker.as_ref().is_some_and(|h| h.is_finished()) {
                if let Some(handle) = worker.take() {
                    ok &= handle.join().is_ok();
                }
            } else if worker.is_some() {
                ok = false;
            }
        }
        if let Some((input, output)) = self.modes.take() {
            ok &= termios::tcsetattr(std::io::stdin().as_fd(), SetArg::TCSANOW, &input).is_ok();
            ok &= termios::tcsetattr(std::io::stdout().as_fd(), SetArg::TCSANOW, &output).is_ok();
        }
        ok
    }
    pub(crate) fn stop_reader(&mut self, deadline: Instant) -> bool {
        self.shared.input_end.store(true, Ordering::SeqCst);
        while self.reader.as_ref().is_some_and(|h| !h.is_finished()) && Instant::now() < deadline {
            self.shared.wait();
        }
        if self.reader.as_ref().is_some_and(|h| h.is_finished()) {
            self.reader.take().is_some_and(|h| h.join().is_ok())
        } else {
            self.reader.is_none()
        }
    }
}
impl Drop for Terminal {
    fn drop(&mut self) {
        let _joined = self.finish(Instant::now() + Duration::from_millis(100));
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
    out.write_all(text.as_bytes())
        .and_then(|()| out.flush())
        .map_err(|_| Failure::Output)
}

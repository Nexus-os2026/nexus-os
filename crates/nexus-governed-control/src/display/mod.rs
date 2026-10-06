//! P3-D and P3-E: perception and input, on the agent display only.
//!
//! The agent display is a backend-owned isolated X server ([`server`]); the
//! owner's own desktop is never observed or driven (nothing here reads
//! `DISPLAY`). Perception and input are separate authorities with separate
//! grants.
//!
//! Perception (R0): an observation of a region or of one window, bound to
//! the display instance and, for a window, to its identity (window id and
//! geometry; a title only selects it). The image is returned as data, kept
//! in memory only, and the evidence records its digest and size, never its
//! pixels.
//!
//! Input: every action binds the display instance, the window under the
//! point (id and geometry) and the run's latest observation; all three are
//! checked again immediately before the effect, and the window under the
//! point before every press. Pointer moves and scrolls are R1; clicks,
//! drags and keys are R2 unless the owner granted an input session that
//! scopes them to R1. Every action consumes a step of the grant, is paced,
//! and observes cancellation between events. Typed text is shown to the
//! owner for approval and recorded only as a digest.

#[cfg(target_os = "linux")]
mod server;

use crate::authority::commitment::{ExecutionGuard, FailureClass, PreparedAction, TargetIdentity};
use crate::authority::effect::{CapabilityKind, EffectClass};
use crate::authority::evidence::{escaped, hint, quoted};
use crate::authority::ids::{Digest, GrantId, RunId};
use crate::authority::policy::GrantScope;
use crate::authority::{Authority, AuthorityError};
use crate::control::{EffectOutput, PendingEffect, Preparation};
use crate::runtime_root::RuntimeRoot;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// The one display Phase Three acts on.
pub const AGENT_DISPLAY: &str = "agent";
/// The most steps one input grant may allow.
pub const MAX_INPUT_STEPS: u32 = 500;
const MAX_TYPED: usize = 256;
const OBSERVATION_TTL: Duration = Duration::from_secs(10 * 60);
const ACTION_TTL: Duration = Duration::from_secs(5 * 60);
/// The least time between two input actions.
const PACE: Duration = Duration::from_millis(30);

/// A rectangle on the display.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rect {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}

impl Rect {
    pub fn contains(&self, x: u16, y: u16) -> bool {
        x >= self.x
            && y >= self.y
            && u32::from(x) < u32::from(self.x) + u32::from(self.width)
            && u32::from(y) < u32::from(self.y) + u32::from(self.height)
    }

    fn within(&self, width: u16, height: u16) -> bool {
        self.width > 0
            && self.height > 0
            && u32::from(self.x) + u32::from(self.width) <= u32::from(width)
            && u32::from(self.y) + u32::from(self.height) <= u32::from(height)
    }

    fn bytes(&self) -> [u8; 8] {
        let mut out = [0u8; 8];
        out[..2].copy_from_slice(&self.x.to_be_bytes());
        out[2..4].copy_from_slice(&self.y.to_be_bytes());
        out[4..6].copy_from_slice(&self.width.to_be_bytes());
        out[6..].copy_from_slice(&self.height.to_be_bytes());
        out
    }
}

/// A top-level window on the agent display.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WindowInfo {
    pub id: u32,
    pub title: String,
    pub rect: Rect,
}

/// An observation request, as data.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum PerceptionIntent {
    Screen {
        #[serde(default)]
        region: Option<Rect>,
    },
    /// The title selects the window; the observation binds its id.
    Window { title: String },
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Button {
    Left,
    Middle,
    Right,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ScrollDirection {
    Up,
    Down,
    Left,
    Right,
}

/// An input request, as data.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum InputIntent {
    Move {
        x: u16,
        y: u16,
    },
    Click {
        x: u16,
        y: u16,
        #[serde(default)]
        button: Option<Button>,
    },
    DoubleClick {
        x: u16,
        y: u16,
    },
    Drag {
        from_x: u16,
        from_y: u16,
        to_x: u16,
        to_y: u16,
    },
    Type {
        text: String,
    },
    Press {
        key: String,
    },
    Shortcut {
        keys: Vec<String>,
    },
    Scroll {
        direction: ScrollDirection,
        amount: u8,
    },
}

/// One low-level event of an input action.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Step {
    Pointer(u16, u16),
    Button(u8, bool),
    Key(u8, bool),
    Pause(u16),
}

/// What the owner sees about the display.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DisplayStatus {
    pub generation: u64,
    pub number: u32,
    pub width: u16,
    pub height: u16,
}

struct LatestObservation {
    id: String,
    generation: u64,
    at: Instant,
}

#[cfg(target_os = "linux")]
type Server = server::AgentServer;

/// Stand-in so the platform-neutral API compiles where there is no server.
#[cfg(not(target_os = "linux"))]
struct Server;

#[cfg(not(target_os = "linux"))]
impl Server {
    fn start(_root: &RuntimeRoot, _width: u16, _height: u16) -> Result<Self, AuthorityError> {
        Err(unavailable())
    }
    fn size(&self) -> (u16, u16) {
        (0, 0)
    }
    fn number(&self) -> u32 {
        0
    }
    fn alive(&self) -> bool {
        false
    }
    fn capture(&self, _rect: Rect) -> Result<Vec<u8>, AuthorityError> {
        Err(unavailable())
    }
    fn windows(&self) -> Vec<WindowInfo> {
        Vec::new()
    }
    fn window_at(&self, _x: u16, _y: u16) -> Option<WindowInfo> {
        None
    }
    fn pointer(&self, _x: u16, _y: u16) -> Result<(), AuthorityError> {
        Err(unavailable())
    }
    fn button(&self, _button: u8, _press: bool) -> Result<(), AuthorityError> {
        Err(unavailable())
    }
    fn key(&self, _keycode: u8, _press: bool) -> Result<(), AuthorityError> {
        Err(unavailable())
    }
    fn keycode(&self, _keysym: u32) -> Option<(u8, bool)> {
        None
    }
    fn pointer_position(&self) -> (u16, u16) {
        (0, 0)
    }
    fn hold(&self) -> Result<(), AuthorityError> {
        Err(unavailable())
    }
    fn keys_reach(&self, _window: u32) -> bool {
        false
    }
    fn no_active_grab(&self) -> bool {
        false
    }
}

#[cfg(not(target_os = "linux"))]
fn unavailable() -> AuthorityError {
    AuthorityError::Unavailable("the agent display is available on Linux only")
}

struct Session {
    generation: u64,
    server: Arc<Server>,
}

/// The agent display actuator.
pub struct AgentDisplay {
    root: RuntimeRoot,
    generation: Mutex<u64>,
    /// One start at a time; a stop never waits for one.
    starting: Mutex<()>,
    /// Counts stops: a start that a stop overtook ends its server unused.
    stops: AtomicU64,
    shared: Arc<Shared>,
}

/// State shared with pending effects. Effects never keep the server: they
/// look up the running session by its generation, so stopping or
/// restarting the display ends the server and fails whatever was bound to
/// it.
#[derive(Default)]
struct Shared {
    session: Mutex<Option<Session>>,
    observations: Mutex<HashMap<RunId, LatestObservation>>,
    steps: Mutex<HashMap<GrantId, u32>>,
    last_input: Mutex<Option<Instant>>,
    /// Held for the whole of an input action: the events of two actions
    /// never interleave.
    acting: Mutex<()>,
}

impl Shared {
    /// The running server of `generation`, if it still answers.
    fn server(&self, generation: u64) -> Result<Arc<Server>, AuthorityError> {
        let session = self.session.lock().expect("display");
        match session.as_ref() {
            Some(s) if s.generation == generation && s.server.alive() => Ok(s.server.clone()),
            _ => Err(AuthorityError::TargetChanged),
        }
    }
}

impl AgentDisplay {
    pub(crate) fn new(root: RuntimeRoot) -> Self {
        Self {
            root,
            generation: Mutex::new(0),
            starting: Mutex::new(()),
            stops: AtomicU64::new(0),
            shared: Arc::new(Shared::default()),
        }
    }

    /// Start the agent display (a no-op while it runs). `admit` is asked
    /// first (it records the start: an unrecorded display does not start)
    /// and `stopped` is asked before the launch; a stop that comes while the
    /// server starts wins, and that server ends unused. The display's lock
    /// is not held across the launch, so a stop never waits for one.
    pub(crate) fn start(
        &self,
        width: u16,
        height: u16,
        admit: impl FnOnce() -> Result<(), AuthorityError>,
        stopped: impl Fn() -> bool,
    ) -> Result<DisplayStatus, AuthorityError> {
        let _starting = self.starting.lock().expect("starting");
        if let Some(current) = self.shared.session.lock().expect("display").as_ref() {
            if current.server.alive() {
                return Ok(status(current));
            }
        }
        if !(320..=3840).contains(&width) || !(240..=2160).contains(&height) {
            return Err(AuthorityError::InvalidAction("display size out of bounds"));
        }
        let stops = self.stops.load(Ordering::SeqCst);
        if stopped() {
            return Err(AuthorityError::EmergencyStopped);
        }
        admit()?;
        let server = Arc::new(Server::start(&self.root, width, height)?);
        let mut session = self.shared.session.lock().expect("display");
        if self.stops.load(Ordering::SeqCst) != stops || stopped() {
            return Err(AuthorityError::Closed(
                "the agent display was stopped while it started",
            ));
        }
        let generation = {
            let mut counter = self.generation.lock().expect("generation");
            *counter += 1;
            *counter
        };
        *session = Some(Session { generation, server });
        Ok(status(session.as_ref().expect("started")))
    }

    /// Stop the agent display: everything bound to it fails revalidation,
    /// and a start in progress ends unused.
    pub fn stop(&self) {
        self.stops.fetch_add(1, Ordering::SeqCst);
        let session = self.shared.session.lock().expect("display").take();
        self.shared
            .observations
            .lock()
            .expect("observations")
            .clear();
        // The server ends here, outside the display's lock.
        drop(session);
    }

    pub fn status(&self) -> Option<DisplayStatus> {
        self.shared
            .session
            .lock()
            .expect("display")
            .as_ref()
            .map(status)
    }

    /// A client connection to the running display (tests only).
    #[cfg(all(test, target_os = "linux"))]
    fn test_client(&self) -> x11rb::rust_connection::RustConnection {
        let session = self.shared.session.lock().expect("display");
        session.as_ref().expect("running").server.client()
    }

    fn current(&self) -> Result<(u64, Arc<Server>), AuthorityError> {
        let session = self.shared.session.lock().expect("display");
        let session = session.as_ref().ok_or(AuthorityError::Unavailable(
            "the agent display is not running",
        ))?;
        if !session.server.alive() {
            return Err(AuthorityError::Unavailable("the agent display stopped"));
        }
        Ok((session.generation, session.server.clone()))
    }

    /// The perception grant scope.
    pub fn perception_scope() -> GrantScope {
        GrantScope::Perception {
            display: AGENT_DISPLAY.into(),
        }
    }

    /// The input grant scope: at most `max_steps` actions; `session_r1`
    /// lets clicks and keys run as R1 under it.
    pub fn input_scope(max_steps: u32, session_r1: bool) -> Result<GrantScope, AuthorityError> {
        if max_steps == 0 || max_steps > MAX_INPUT_STEPS {
            return Err(AuthorityError::InvalidAction("input steps out of bounds"));
        }
        Ok(GrantScope::Input {
            display: AGENT_DISPLAY.into(),
            max_steps,
            session_r1,
        })
    }

    /// Prepare an observation for `run`.
    pub(crate) fn prepare_observation(
        &self,
        authority: &Authority,
        run: RunId,
        intent: &PerceptionIntent,
    ) -> Result<Preparation, AuthorityError> {
        let (generation, server) = self.current()?;
        let grant = perception_grant(authority)?;
        let (width, height) = server.size();
        let (rect, window, label) = match intent {
            PerceptionIntent::Screen { region } => {
                let rect = region.unwrap_or(Rect {
                    x: 0,
                    y: 0,
                    width,
                    height,
                });
                if !rect.within(width, height) {
                    return Err(AuthorityError::InvalidAction("region out of the display"));
                }
                (rect, None, "the agent display".to_string())
            }
            PerceptionIntent::Window { title } => {
                if title.is_empty() || title.chars().count() > 256 {
                    return Err(AuthorityError::InvalidAction(
                        "a window title is bounded text",
                    ));
                }
                let matching: Vec<WindowInfo> = server
                    .windows()
                    .into_iter()
                    .filter(|w| w.title == *title)
                    .collect();
                let [window] = matching.as_slice() else {
                    return Err(AuthorityError::InvalidAction(
                        "the title must name exactly one window",
                    ));
                };
                let rect = clip(window.rect, width, height).ok_or(
                    AuthorityError::InvalidAction("the window is off the display"),
                )?;
                (
                    rect,
                    Some(window.id),
                    format!("window \"{}\"", quoted_title(&window.title)),
                )
            }
        };
        let target = Digest::of(
            "nexus.p3.display.observe.v1",
            &[
                &generation.to_be_bytes(),
                &window.unwrap_or(0).to_be_bytes(),
                &rect.bytes(),
            ],
        );
        let parameters = Digest::of("nexus.p3.display.observe.params.v1", &[&rect.bytes()]);
        let action = PreparedAction {
            kind: CapabilityKind::Perception,
            class: EffectClass::R0,
            operation: "display.observe",
            target: TargetIdentity {
                display: escaped(&format!("{label} on agent display {}", server.number())),
                digest: target,
            },
            parameters,
            grants: vec![grant],
            leases: vec![],
            summary: vec![format!(
                "Observe {}x{} at ({}, {}) of {}",
                rect.width,
                rect.height,
                rect.x,
                rect.y,
                escaped(&label)
            )],
        };
        Ok(Preparation {
            action,
            effect: Box::new(Observe {
                generation,
                window,
                rect,
                run,
                target,
                parameters,
                shared: self.shared.clone(),
            }),
            ttl: OBSERVATION_TTL,
        })
    }

    /// Prepare an input action for `run`.
    pub(crate) fn prepare_input(
        &self,
        authority: &Authority,
        run: RunId,
        intent: &InputIntent,
    ) -> Result<Preparation, AuthorityError> {
        let (generation, server) = self.current()?;
        let (grant, max_steps, session_r1) = input_grant(authority)?;
        let used = self
            .shared
            .steps
            .lock()
            .expect("steps")
            .get(&grant)
            .copied()
            .unwrap_or(0);
        if used >= max_steps {
            return Err(AuthorityError::Closed(
                "the input grant's steps are used up",
            ));
        }
        let observation = {
            let observations = self.shared.observations.lock().expect("observations");
            match observations.get(&run) {
                Some(o) if o.generation == generation && o.at.elapsed() < OBSERVATION_TTL => {
                    o.id.clone()
                }
                _ => {
                    return Err(AuthorityError::InvalidAction(
                        "observe the agent display before acting on it",
                    ))
                }
            }
        };
        let (width, height) = server.size();
        let inside = |x: u16, y: u16| -> Result<(), AuthorityError> {
            if x < width && y < height {
                Ok(())
            } else {
                Err(AuthorityError::InvalidAction("a point is off the display"))
            }
        };
        let key = |keysym: u32| -> Result<(u8, bool), AuthorityError> {
            server.keycode(keysym).ok_or(AuthorityError::InvalidAction(
                "a key is not on the display's keyboard",
            ))
        };
        let shift = key(KEYSYM_SHIFT)?.0;
        let follows_pointer = matches!(
            intent,
            InputIntent::Type { .. }
                | InputIntent::Press { .. }
                | InputIntent::Shortcut { .. }
                | InputIntent::Scroll { .. }
        );
        let (point, steps, gesture, summary) = match intent {
            InputIntent::Move { x, y } => {
                inside(*x, *y)?;
                (
                    (*x, *y),
                    vec![Step::Pointer(*x, *y)],
                    false,
                    vec![format!("Move the pointer to ({x}, {y})")],
                )
            }
            InputIntent::Click { x, y, button } => {
                inside(*x, *y)?;
                let (code, name) = button_code(button.unwrap_or(Button::Left));
                (
                    (*x, *y),
                    vec![
                        Step::Pointer(*x, *y),
                        Step::Button(code, true),
                        Step::Button(code, false),
                    ],
                    true,
                    vec![format!("Click {name} at ({x}, {y})")],
                )
            }
            InputIntent::DoubleClick { x, y } => {
                inside(*x, *y)?;
                (
                    (*x, *y),
                    vec![
                        Step::Pointer(*x, *y),
                        Step::Button(1, true),
                        Step::Button(1, false),
                        Step::Pause(60),
                        Step::Button(1, true),
                        Step::Button(1, false),
                    ],
                    true,
                    vec![format!("Double-click at ({x}, {y})")],
                )
            }
            InputIntent::Drag {
                from_x,
                from_y,
                to_x,
                to_y,
            } => {
                inside(*from_x, *from_y)?;
                inside(*to_x, *to_y)?;
                let mut steps = vec![Step::Pointer(*from_x, *from_y), Step::Button(1, true)];
                for i in 1..=8u32 {
                    let x =
                        i32::from(*from_x) + (i32::from(*to_x) - i32::from(*from_x)) * i as i32 / 8;
                    let y =
                        i32::from(*from_y) + (i32::from(*to_y) - i32::from(*from_y)) * i as i32 / 8;
                    steps.push(Step::Pointer(x as u16, y as u16));
                    steps.push(Step::Pause(15));
                }
                steps.push(Step::Button(1, false));
                (
                    (*from_x, *from_y),
                    steps,
                    true,
                    vec![format!(
                        "Drag from ({from_x}, {from_y}) to ({to_x}, {to_y})"
                    )],
                )
            }
            InputIntent::Type { text } => {
                if text.is_empty() || text.chars().count() > MAX_TYPED {
                    return Err(AuthorityError::InvalidAction(
                        "typed text is 1 to 256 characters",
                    ));
                }
                let mut steps = Vec::new();
                for c in text.chars() {
                    let (code, shifted) = key(char_keysym(c)?)?;
                    if shifted {
                        steps.push(Step::Key(shift, true));
                    }
                    steps.push(Step::Key(code, true));
                    steps.push(Step::Key(code, false));
                    if shifted {
                        steps.push(Step::Key(shift, false));
                    }
                    steps.push(Step::Pause(8));
                }
                (
                    server_pointer(&server),
                    steps,
                    true,
                    quoted(&format!("Type {} characters", text.chars().count()), text),
                )
            }
            InputIntent::Press { key: name } => {
                let code = key(named_keysym(name)?)?.0;
                (
                    server_pointer(&server),
                    vec![Step::Key(code, true), Step::Key(code, false)],
                    true,
                    vec![format!("Press {}", escaped(name))],
                )
            }
            InputIntent::Shortcut { keys } => {
                let Some((last, modifiers)) = keys.split_last() else {
                    return Err(AuthorityError::InvalidAction("a shortcut has keys"));
                };
                if modifiers.is_empty() || modifiers.len() > 3 {
                    return Err(AuthorityError::InvalidAction(
                        "a shortcut is 1 to 3 modifiers and a key",
                    ));
                }
                let mut down = Vec::new();
                for modifier in modifiers {
                    down.push(key(modifier_keysym(modifier)?)?.0);
                }
                let code = key(named_keysym(last)?)?.0;
                let mut steps: Vec<Step> = down.iter().map(|c| Step::Key(*c, true)).collect();
                steps.push(Step::Key(code, true));
                steps.push(Step::Key(code, false));
                steps.extend(down.iter().rev().map(|c| Step::Key(*c, false)));
                (
                    server_pointer(&server),
                    steps,
                    true,
                    vec![format!("Shortcut {}", escaped(&keys.join("+")))],
                )
            }
            InputIntent::Scroll { direction, amount } => {
                if *amount == 0 || *amount > 10 {
                    return Err(AuthorityError::InvalidAction("scroll 1 to 10 notches"));
                }
                let code = match direction {
                    ScrollDirection::Up => 4,
                    ScrollDirection::Down => 5,
                    ScrollDirection::Left => 6,
                    ScrollDirection::Right => 7,
                };
                let mut steps = Vec::new();
                for _ in 0..*amount {
                    steps.push(Step::Button(code, true));
                    steps.push(Step::Button(code, false));
                    steps.push(Step::Pause(20));
                }
                (
                    server_pointer(&server),
                    steps,
                    false,
                    vec![format!("Scroll {direction:?} {amount} notches").to_lowercase()],
                )
            }
        };
        let window = server.window_at(point.0, point.1);
        let window_rect = window.as_ref().map(|w| w.rect).unwrap_or(Rect {
            x: 0,
            y: 0,
            width,
            height,
        });
        let window_id = window.as_ref().map_or(0, |w| w.id);
        // A drag lands where it is dropped: the window there is bound too.
        let drop_point = match intent {
            InputIntent::Drag { to_x, to_y, .. } => Some((*to_x, *to_y)),
            _ => None,
        };
        let drop_window = drop_point.map(|(x, y)| server.window_at(x, y));
        let mut summary = summary;
        if let Some(target) = &drop_window {
            summary.push(match target {
                Some(w) => format!(
                    "Dropped on window \"{}\" (id {})",
                    quoted_title(&w.title),
                    w.id
                ),
                None => "Dropped on the display background".to_string(),
            });
        }
        let drop_window = drop_window.map(|w| w.map_or(0, |w| w.id));
        let class = match (gesture, session_r1) {
            (false, _) => EffectClass::R1,
            (true, true) => EffectClass::R1,
            (true, false) => EffectClass::R2,
        };
        let target = input_target(
            generation,
            window_id,
            window_rect,
            &observation,
            drop_window,
        );
        let canonical =
            serde_json::to_vec(intent).map_err(|_| AuthorityError::InvalidAction("input"))?;
        let parameters = Digest::of("nexus.p3.display.input.v1", &[&canonical]);
        let place = window
            .as_ref()
            .map(|w| format!("window \"{}\" (id {})", quoted_title(&w.title), w.id))
            .unwrap_or_else(|| "the display background".into());
        let action = PreparedAction {
            kind: CapabilityKind::Input,
            class,
            operation: "display.input",
            target: TargetIdentity {
                display: escaped(&format!("{place} on agent display {}", server.number())),
                digest: target,
            },
            parameters,
            grants: vec![grant],
            leases: vec![],
            summary: {
                let mut lines = summary;
                lines.push(format!(
                    "Step {} of {max_steps} under the input grant",
                    used + 1
                ));
                lines
            },
        };
        Ok(Preparation {
            action,
            effect: Box::new(Input {
                generation,
                point,
                follows_pointer,
                window_id,
                window_rect,
                drop_point,
                observation,
                run,
                grant,
                max_steps,
                steps,
                target,
                parameters,
                shared: self.shared.clone(),
            }),
            ttl: ACTION_TTL,
        })
    }

    /// Forget a run's observations (it ended).
    pub fn forget_run(&self, run: RunId) {
        self.shared
            .observations
            .lock()
            .expect("observations")
            .remove(&run);
    }
}

fn status(session: &Session) -> DisplayStatus {
    let (width, height) = session.server.size();
    DisplayStatus {
        generation: session.generation,
        number: session.server.number(),
        width,
        height,
    }
}

fn clip(rect: Rect, width: u16, height: u16) -> Option<Rect> {
    let right = (u32::from(rect.x) + u32::from(rect.width)).min(u32::from(width));
    let bottom = (u32::from(rect.y) + u32::from(rect.height)).min(u32::from(height));
    (right > u32::from(rect.x) && bottom > u32::from(rect.y)).then(|| Rect {
        x: rect.x,
        y: rect.y,
        width: (right - u32::from(rect.x)) as u16,
        height: (bottom - u32::from(rect.y)) as u16,
    })
}

fn input_target(
    generation: u64,
    window: u32,
    rect: Rect,
    observation: &str,
    drop_window: Option<u32>,
) -> Digest {
    let drop: Vec<u8> = match drop_window {
        Some(window) => [&[1u8][..], &window.to_be_bytes()].concat(),
        None => vec![0],
    };
    Digest::of(
        "nexus.p3.display.input.target.v2",
        &[
            &generation.to_be_bytes(),
            &window.to_be_bytes(),
            &rect.bytes(),
            observation.as_bytes(),
            &drop,
        ],
    )
}

/// Keyboard actions go to the window under the pointer.
fn server_pointer(server: &Server) -> (u16, u16) {
    server.pointer_position()
}

fn button_code(button: Button) -> (u8, &'static str) {
    match button {
        Button::Left => (1, "left"),
        Button::Middle => (2, "middle"),
        Button::Right => (3, "right"),
    }
}

const KEYSYM_SHIFT: u32 = 0xffe1;

fn char_keysym(c: char) -> Result<u32, AuthorityError> {
    match c {
        ' '..='~' => Ok(c as u32),
        '\n' => Ok(0xff0d),
        '\t' => Ok(0xff09),
        _ => Err(AuthorityError::InvalidAction(
            "only printable ASCII, tab and newline can be typed",
        )),
    }
}

fn named_keysym(name: &str) -> Result<u32, AuthorityError> {
    let lower = name.to_ascii_lowercase();
    let keysym = match lower.as_str() {
        "return" | "enter" => 0xff0d,
        "tab" => 0xff09,
        "escape" | "esc" => 0xff1b,
        "backspace" => 0xff08,
        "delete" => 0xffff,
        "home" => 0xff50,
        "end" => 0xff57,
        "page_up" | "pageup" => 0xff55,
        "page_down" | "pagedown" => 0xff56,
        "left" => 0xff51,
        "up" => 0xff52,
        "right" => 0xff53,
        "down" => 0xff54,
        "space" => 0x20,
        f if f.len() >= 2
            && f.starts_with('f')
            && f[1..].parse::<u32>().is_ok_and(|n| (1..=12).contains(&n)) =>
        {
            0xffbe + f[1..].parse::<u32>().expect("checked") - 1
        }
        one if one.len() == 1 && one.chars().all(|c| c.is_ascii_alphanumeric()) => {
            u32::from(one.as_bytes()[0])
        }
        _ => return Err(AuthorityError::InvalidAction("unknown key")),
    };
    Ok(keysym)
}

fn modifier_keysym(name: &str) -> Result<u32, AuthorityError> {
    match name.to_ascii_lowercase().as_str() {
        "control" | "ctrl" => Ok(0xffe3),
        "alt" => Ok(0xffe9),
        "shift" => Ok(0xffe1),
        _ => Err(AuthorityError::InvalidAction(
            "a modifier is control, alt or shift",
        )),
    }
}

fn perception_grant(authority: &Authority) -> Result<GrantId, AuthorityError> {
    authority
        .grants()
        .live_of(CapabilityKind::Perception)
        .into_iter()
        .find(
            |g| matches!(&g.scope, GrantScope::Perception { display } if display == AGENT_DISPLAY),
        )
        .map(|g| g.id)
        .ok_or(AuthorityError::NoCoveringGrant)
}

fn input_grant(authority: &Authority) -> Result<(GrantId, u32, bool), AuthorityError> {
    authority
        .grants()
        .live_of(CapabilityKind::Input)
        .into_iter()
        .find_map(|g| match &g.scope {
            GrantScope::Input {
                display,
                max_steps,
                session_r1,
            } if display == AGENT_DISPLAY => Some((g.id, *max_steps, *session_r1)),
            _ => None,
        })
        .ok_or(AuthorityError::NoCoveringGrant)
}

struct Observe {
    generation: u64,
    window: Option<u32>,
    rect: Rect,
    run: RunId,
    target: Digest,
    parameters: Digest,
    shared: Arc<Shared>,
}

impl PendingEffect for Observe {
    fn revalidate(&self) -> Result<Digest, AuthorityError> {
        let server = self.shared.server(self.generation)?;
        if let Some(window) = self.window {
            let (width, height) = server.size();
            let now = server
                .windows()
                .into_iter()
                .find(|w| w.id == window)
                .and_then(|w| clip(w.rect, width, height));
            if now != Some(self.rect) {
                return Err(AuthorityError::TargetChanged);
            }
        }
        Ok(self.target)
    }

    fn parameters(&self) -> Digest {
        self.parameters
    }

    fn execute(
        self: Box<Self>,
        guard: &ExecutionGuard,
    ) -> Result<EffectOutput, (FailureClass, String)> {
        if guard.is_cancelled() {
            return Err((FailureClass::Actuator, "cancelled".into()));
        }
        let server = self.shared.server(self.generation).map_err(|_| {
            (
                FailureClass::TargetChanged,
                "the agent display changed".to_string(),
            )
        })?;
        let rgb = server
            .capture(self.rect)
            .map_err(|e| (FailureClass::Unavailable, e.class().to_string()))?;
        let png = encode_png(&rgb, self.rect.width, self.rect.height).map_err(|_| {
            (
                FailureClass::Actuator,
                "the observation could not be encoded".to_string(),
            )
        })?;
        let digest = Digest::of("nexus.p3.display.pixels.v1", &[&png]);
        let mut id_bytes = [0u8; 8];
        getrandom::getrandom(&mut id_bytes)
            .map_err(|_| (FailureClass::Unavailable, "no randomness".to_string()))?;
        let id = format!("obs-{}", hex::encode(id_bytes));
        {
            let mut observations = self.shared.observations.lock().expect("observations");
            if observations.len() >= 1024 {
                observations.retain(|_, o| o.at.elapsed() < OBSERVATION_TTL);
            }
            observations.insert(
                self.run,
                LatestObservation {
                    id: id.clone(),
                    generation: self.generation,
                    at: Instant::now(),
                },
            );
        }
        Ok(EffectOutput {
            text: None,
            bytes: Some(png),
            meta: vec![
                ("observation".into(), id),
                ("width".into(), self.rect.width.to_string()),
                ("height".into(), self.rect.height.to_string()),
                ("sha256".into(), digest.to_hex()),
                ("format".into(), "png".into()),
            ],
        })
    }
}

fn encode_png(rgb: &[u8], width: u16, height: u16) -> Result<Vec<u8>, png::EncodingError> {
    let mut out = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut out, u32::from(width), u32::from(height));
        encoder.set_color(png::ColorType::Rgb);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header()?;
        writer.write_image_data(rgb)?;
    }
    Ok(out)
}

struct Input {
    generation: u64,
    point: (u16, u16),
    /// Keyboard and scroll events go where the pointer is: the window
    /// under the live pointer must still be the bound one.
    follows_pointer: bool,
    window_id: u32,
    window_rect: Rect,
    /// Where a drag is dropped: the window there is part of the binding.
    drop_point: Option<(u16, u16)>,
    observation: String,
    run: RunId,
    grant: GrantId,
    /// The grant's step budget, spent at the effect.
    max_steps: u32,
    steps: Vec<Step>,
    target: Digest,
    parameters: Digest,
    shared: Arc<Shared>,
}

impl Input {
    /// The display, the window under the point and the run's latest
    /// observation are still what the action was bound to.
    fn current_target(&self, server: &Server) -> Digest {
        let (width, height) = server.size();
        let (x, y) = if self.follows_pointer {
            server.pointer_position()
        } else {
            self.point
        };
        let window = server.window_at(x, y);
        let rect = window.as_ref().map(|w| w.rect).unwrap_or(Rect {
            x: 0,
            y: 0,
            width,
            height,
        });
        let observation = self
            .shared
            .observations
            .lock()
            .expect("observations")
            .get(&self.run)
            .filter(|o| o.generation == self.generation)
            .map(|o| o.id.clone())
            .unwrap_or_default();
        let drop_window = self
            .drop_point
            .map(|(x, y)| server.window_at(x, y).map_or(0, |w| w.id));
        input_target(
            self.generation,
            window.as_ref().map_or(0, |w| w.id),
            rect,
            &observation,
            drop_window,
        )
    }
}

/// A window title as the owner reads it, between quotes: shortened like any
/// hint, with its own quotes escaped so it cannot close them.
fn quoted_title(text: &str) -> String {
    hint(text).replace('"', "\\\"")
}

/// What an action has pressed and not released yet: released on every
/// exit, so a failed or cancelled action never leaves a key or a button
/// held down for the next one.
#[derive(Default)]
struct Pressed {
    server: Option<Arc<Server>>,
    keys: Vec<u8>,
    buttons: Vec<u8>,
    /// Where the pointer was when a button went down: an action that ends
    /// early lets its button go there, so an interrupted drag drops nowhere
    /// new.
    pressed_at: Option<(u16, u16)>,
}

impl Pressed {
    fn note(held: &mut Vec<u8>, code: u8, press: bool) {
        if press {
            held.push(code);
        } else {
            held.retain(|c| *c != code);
        }
    }
}

impl Drop for Pressed {
    fn drop(&mut self) {
        if let Some(server) = &self.server {
            for code in self.keys.iter().rev() {
                let _ = server.key(*code, false);
            }
            if let (false, Some((x, y))) = (self.buttons.is_empty(), self.pressed_at) {
                let _ = server.pointer(x, y);
            }
            for code in self.buttons.iter().rev() {
                let _ = server.button(*code, false);
            }
        }
    }
}

impl PendingEffect for Input {
    fn revalidate(&self) -> Result<Digest, AuthorityError> {
        let server = self.shared.server(self.generation)?;
        Ok(self.current_target(&server))
    }

    fn parameters(&self) -> Digest {
        self.parameters
    }

    fn execute(
        self: Box<Self>,
        guard: &ExecutionGuard,
    ) -> Result<EffectOutput, (FailureClass, String)> {
        // One action at a time: the events of two actions never interleave.
        let _acting = self.shared.acting.lock().expect("acting");
        // The grant's step budget is spent here, at the effect, under its
        // lock: actions prepared together cannot exceed it.
        {
            let mut steps = self.shared.steps.lock().expect("steps");
            let used = steps.entry(self.grant).or_insert(0);
            if *used >= self.max_steps {
                return Err((
                    FailureClass::Bounds,
                    "the input grant's steps are used up".into(),
                ));
            }
            *used += 1;
        }
        // Paced: never faster than one action per PACE.
        {
            let mut last = self.shared.last_input.lock().expect("pace");
            if let Some(at) = *last {
                let since = at.elapsed();
                if since < PACE {
                    std::thread::sleep(PACE - since);
                }
            }
            *last = Some(Instant::now());
        }
        let failed = |error: AuthorityError| (FailureClass::Unavailable, error.class().to_string());
        let changed = || {
            (
                FailureClass::TargetChanged,
                "the window under the point changed".to_string(),
            )
        };
        let mut sent = 0u32;
        // Whatever is still pressed when the action ends early is released.
        let mut pressed = Pressed::default();
        for step in &self.steps {
            if guard.is_cancelled() {
                return Err((FailureClass::Actuator, "cancelled".into()));
            }
            // The display must still be the bound one at every event.
            let server = self.shared.server(self.generation).map_err(|_| changed())?;
            pressed.server = Some(server.clone());
            match *step {
                Step::Pointer(x, y) => server.pointer(x, y).map_err(failed)?,
                // A press is checked and sent while the server is held, so
                // nothing can appear over the point, move the pointer, take
                // the keyboard focus or grab the input between the check and
                // the event.
                Step::Button(code, press) => {
                    // A drag's release is its effect: it is checked at the
                    // drop point, under the hold. The client the press went
                    // to holds the pointer until the release (checked at the
                    // press), so no other grab can be taken in between.
                    let drop = self.drop_point.filter(|_| !press);
                    let _held = if press || drop.is_some() {
                        Some(server.hold().map_err(failed)?)
                    } else {
                        None
                    };
                    if press
                        && (self.current_target(&server) != self.target
                            || (!self.follows_pointer && server.pointer_position() != self.point)
                            || !server.no_active_grab())
                    {
                        return Err(changed());
                    }
                    if let Some(point) = drop {
                        if self.current_target(&server) != self.target
                            || server.pointer_position() != point
                        {
                            return Err(changed());
                        }
                    }
                    if press {
                        pressed.pressed_at = Some(server.pointer_position());
                    }
                    server.button(code, press).map_err(failed)?;
                    Pressed::note(&mut pressed.buttons, code, press);
                }
                // Keys go to the keyboard focus: it must still deliver to
                // the bound window.
                Step::Key(code, press) => {
                    let _held = if press {
                        Some(server.hold().map_err(failed)?)
                    } else {
                        None
                    };
                    if press
                        && (self.current_target(&server) != self.target
                            || !server.keys_reach(self.window_id)
                            || !server.no_active_grab())
                    {
                        return Err(changed());
                    }
                    server.key(code, press).map_err(failed)?;
                    Pressed::note(&mut pressed.keys, code, press);
                }
                Step::Pause(ms) => std::thread::sleep(Duration::from_millis(u64::from(ms))),
            }
            sent += 1;
        }
        Ok(EffectOutput {
            text: None,
            bytes: None,
            meta: vec![
                ("events".into(), sent.to_string()),
                ("window".into(), self.window_id.to_string()),
                (
                    "window_rect".into(),
                    format!(
                        "{}x{}+{}+{}",
                        self.window_rect.width,
                        self.window_rect.height,
                        self.window_rect.x,
                        self.window_rect.y
                    ),
                ),
                ("observation".into(), self.observation.clone()),
            ],
        })
    }
}

#[cfg(all(test, target_os = "linux"))]
mod tests;

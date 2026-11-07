//! The broker's local-model capability (Mother Nexus M2e-B, design v2 §6
//! step 3 and §8, with the M2e amendment's return time; AGENTS.md §18
//! security core).
//!
//! - **Registration is the host's.** [`GovernedControl::register_local_model`]
//!   (the front door; no door route) verifies the process's one model
//!   through the kernel's [`VerifiedOllama`], with no broker lock held,
//!   under a watch of the broker's own kernel stop, the session's end and
//!   an absolute 30 s deadline, and records it (Restricted: its two
//!   digests) before it exists. A second model is refused.
//! - **Admission.** A door binds one of its runs, once, to the Owner's yes
//!   cached at that door (M2d-B1's table with every H1′ rule, shared with
//!   Files admission) and to the registered identity.
//! - **A call.** In one hold of the runs lock: the run is this door's live
//!   agent run, the door is not halted, the run holds its inference
//!   admission for the registered identity, and the run has made fewer
//!   than [`MAX_MODEL_ATTEMPTS`] attempts (then counted, never refunded).
//!   The run's token and the door's halt flag are taken; the lock is
//!   released. A fresh watch is admitted from the broker's kernel stop, the
//!   deadline is the earlier of the caller's and `CALL_HARD_TIMEOUT` from
//!   now (both absolute), and the kernel transport makes the call.
//! - **Evidence.** One Restricted `model_call` record per attempt, written
//!   after the call returns, holding counts, the two digests, the outcome's
//!   code and the host CLOCK_MONOTONIC time it returned at: never a prompt,
//!   a reply, a metadata body or a model name.
//! - **Locks.** No broker lock is held across the meter, the transport, or
//!   the record write.
//! - **Deferred to M2e-A1b** (the kernel's dispatch observer): the send
//!   time ([`CallTiming::sent_mono_ms`], omitted until then, never 0), the
//!   `Dispatch` notification and the host observer at registration.
//!
//! [`GovernedControl::register_local_model`]: crate::governed::GovernedControl::register_local_model

use crate::authority::evidence::EndStatus;
use crate::authority::ids::{AgentId, RunId};
use crate::authority::run::{CallTicket, RunRegistry};
use crate::authority::{Authority, AuthorityError};
use nexus_kernel::emergency_stop::StopAuthority;
use nexus_kernel::inference::{
    CallCancel, CallContext, CancelCause, LoopbackEndpoint, ModelTransport, VerifiedOllama,
    CALL_HARD_TIMEOUT,
};

/// The kernel's local-model types a door's caller needs (design v2 §13:
/// Mother names the kernel types through these re-exports; the §9.3
/// ports; Codex review 1 of PR #75, MEDIUM 2): building a request and its
/// schema, reading a reply and its error codes, implementing the meter and
/// the absolute deadline, and the pin and its identity. The transport
/// itself stays the broker's: its endpoint, the verified runtime, the
/// transport trait, a call's context, and the cancellation and dispatch
/// hooks are not re-exported.
pub use nexus_kernel::inference::{
    grammar, AbsoluteDeadline, CallDeadline, CallMeter, Charged, DeadlineClock, MeterRefusal,
    ModelError, ModelIdentity, ModelPinSpec, ModelReply, ModelRequest, ResponseSchema,
    RuntimeConfig, SystemClocks, Think, MAX_RENDERED_CONTENT_BYTES,
};
use std::sync::{Arc, Mutex, OnceLock, PoisonError};
use std::time::Duration;

#[cfg(all(test, target_os = "linux"))]
#[path = "inference_tests.rs"]
mod tests;

/// Most attempts one run may make (design v2 §6 step 3): the ninth is
/// refused before the runtime is asked anything or the meter charged.
pub(crate) const MAX_MODEL_ATTEMPTS: u32 = 8;

/// The absolute bound on a registration's requests.
const REGISTRATION_DEADLINE: Duration = Duration::from_secs(30);

/// The registered model, as a door may see it: its name, its manifest
/// digest and its identity digest (each plain hex or the name grammar).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModelIdentityView {
    pub name: String,
    pub digest: String,
    /// The hex of `ModelIdentity::digest()`.
    pub identity: String,
}

/// The one local model of this process.
pub(crate) struct RegisteredModel {
    pub(crate) transport: Arc<VerifiedOllama>,
    pub(crate) view: ModelIdentityView,
    pub(crate) identity: [u8; 32],
}

impl RegisteredModel {
    /// A verified transport with its identity and Owner-facing view. Only
    /// this file names the kernel transport trait (E-O65 allowlist).
    pub(crate) fn verified(transport: VerifiedOllama) -> Self {
        let identity = transport.identity().digest();
        let view = ModelIdentityView {
            name: transport.identity().name().to_string(),
            digest: transport.identity().model_digest().to_string(),
            identity: hex::encode(identity),
        };
        Self {
            transport: Arc::new(transport),
            view,
            identity,
        }
    }
}

#[derive(Default)]
enum Slot {
    #[default]
    Empty,
    /// A registration is verifying (no lock held meanwhile).
    Registering,
    Registered(Arc<RegisteredModel>),
}

/// The process's model slot: one model, registered once.
#[derive(Default)]
pub(crate) struct LocalModels {
    slot: Mutex<Slot>,
}

/// A registration's hold on the empty slot: filled, or given back when
/// dropped (refused, failed or unwound).
struct Reservation<'a> {
    slot: &'a Mutex<Slot>,
    filled: bool,
}

impl<'a> Reservation<'a> {
    fn take(slot: &'a Mutex<Slot>) -> Result<Self, AuthorityError> {
        let mut state = slot.lock().unwrap_or_else(PoisonError::into_inner);
        if !matches!(*state, Slot::Empty) {
            // One model per process, and one registration at a time.
            return Err(AuthorityError::Capacity);
        }
        *state = Slot::Registering;
        Ok(Self {
            slot,
            filled: false,
        })
    }

    fn fill(mut self, model: RegisteredModel) {
        *self.slot.lock().unwrap_or_else(PoisonError::into_inner) =
            Slot::Registered(Arc::new(model));
        self.filled = true;
    }
}

impl Drop for Reservation<'_> {
    fn drop(&mut self) {
        if !self.filled {
            let mut state = self.slot.lock().unwrap_or_else(PoisonError::into_inner);
            if matches!(*state, Slot::Registering) {
                *state = Slot::Empty;
            }
        }
    }
}

/// A registration ends when the session does (the desktop is quitting).
struct Quitting(Arc<RunRegistry>);

impl CallCancel for Quitting {
    fn cancelled(&self) -> Option<CancelCause> {
        self.0.is_closed().then_some(CancelCause::Closed)
    }
}

impl LocalModels {
    /// Register the process's one model (the front door's; see the module
    /// documentation). Refused under Stop, while quitting, off loopback,
    /// for a second model, and for any verification failure (its code);
    /// unrecorded, it is not registered.
    pub(crate) fn register(
        &self,
        authority: &Authority,
        endpoint: &str,
        spec: &ModelPinSpec,
        stop: &Arc<StopAuthority>,
    ) -> Result<ModelIdentityView, AuthorityError> {
        // Nothing is asked of the runtime while the store cannot record or
        // owes a record.
        authority.inference_admissible()?;
        authority.mission_admissible()?;
        let endpoint = LoopbackEndpoint::parse(endpoint).map_err(AuthorityError::ModelCall)?;
        let reservation = Reservation::take(&self.slot)?;
        let watch = stop.admit().map_err(|_| AuthorityError::EmergencyStopped)?;
        let deadline = AbsoluteDeadline::within(SystemClocks, REGISTRATION_DEADLINE);
        let quitting = Quitting(authority.runs().clone());
        let transport = VerifiedOllama::verify(endpoint, spec, &quitting, &deadline, &watch)
            .map_err(AuthorityError::ModelCall)?;
        let model = RegisteredModel::verified(transport);
        let view = model.view.clone();
        // Evidence first: an unrecorded model is not registered.
        authority.record_model_registered(vec![
            ("model_digest".into(), view.digest.clone()),
            ("identity".into(), view.identity.clone()),
        ])?;
        reservation.fill(model);
        Ok(view)
    }

    pub(crate) fn registered(&self) -> Option<Arc<RegisteredModel>> {
        match &*self.slot.lock().unwrap_or_else(PoisonError::into_inner) {
            Slot::Registered(model) => Some(model.clone()),
            Slot::Empty | Slot::Registering => None,
        }
    }
}

/// What ends an admitted call besides the kernel stop: the session's end,
/// the door's halt, and the run's own cancellation (a halt, a Stop and a
/// finish each set the run's token too).
struct RunCancel {
    ticket: CallTicket,
    runs: Arc<RunRegistry>,
}

impl CallCancel for RunCancel {
    fn cancelled(&self) -> Option<CancelCause> {
        if self.runs.is_closed() {
            Some(CancelCause::Closed)
        } else if self.ticket.door_halted() {
            Some(CancelCause::Halted)
        } else if self.ticket.token.is_cancelled() {
            Some(CancelCause::Cancelled)
        } else {
            None
        }
    }
}

/// The earlier of the caller's absolute deadline and the broker's own.
struct Earliest<'a> {
    caller: &'a dyn CallDeadline,
    own: AbsoluteDeadline,
}

impl CallDeadline for Earliest<'_> {
    fn expired(&self) -> bool {
        self.caller.expired() || self.own.expired()
    }
}

/// The caller's meter, passed through; it notes the index it assigned.
struct Tap<'a> {
    meter: &'a dyn CallMeter,
    charged: OnceLock<u32>,
}

impl CallMeter for Tap<'_> {
    fn charge(&self) -> Result<Charged, MeterRefusal> {
        let charged = self.meter.charge()?;
        let _ = self.charged.set(charged.call);
        Ok(charged)
    }

    fn settle(&self, call: u32) {
        self.meter.settle(call);
    }
}

/// When a call was handed to the runtime and when it returned, on the
/// host's CLOCK_MONOTONIC, in milliseconds. Each is recorded only when it
/// was taken: an absent time is omitted, never 0 and never estimated.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct CallTiming {
    /// M2e-A1b seam: the kernel's dispatch observer reports the instant the
    /// chat request is handed to the HTTP client. Until that hook lands it
    /// is never set, so no record holds a send time.
    pub(crate) sent_mono_ms: Option<u64>,
    /// Taken as the transport's call returns, on every path.
    pub(crate) returned_mono_ms: Option<u64>,
}

/// The host's CLOCK_MONOTONIC in milliseconds (not a process-relative
/// clock), or `None` if it cannot be read.
///
/// Read through the broker's existing `libc` (Codex review 1 of PR #75,
/// LOW): no safe reader is reachable without a new dependency. `nix` is
/// not a dependency of this crate, and adding it would change Cargo.lock;
/// the kernel's safe reader (`nix::time::clock_gettime`) is private to its
/// transport. The one call below is the whole unsafe surface.
#[cfg(target_os = "linux")]
#[allow(unsafe_code)]
pub(crate) fn host_monotonic_ms() -> Option<u64> {
    let mut now = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // SAFETY: clock_gettime writes one timespec through a valid pointer to
    // a local it owns; nothing else aliases it.
    if unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut now) } != 0 {
        return None;
    }
    let seconds = u64::try_from(now.tv_sec).ok()?;
    let millis = u64::try_from(now.tv_nsec).ok()? / 1_000_000;
    seconds.checked_mul(1000)?.checked_add(millis)
}

/// No host monotonic clock is read off Linux: the time is omitted.
#[cfg(not(target_os = "linux"))]
pub(crate) fn host_monotonic_ms() -> Option<u64> {
    None
}

/// What one attempted model call came to (design v2 §8; Codex review 1 of
/// PR #75, MEDIUM 1): the transport's result, and what became of the
/// attempt's record. A record that was not written (owed, or uncertain
/// because the store latched during the call) is reported here beside the
/// result, never silent; the caller decides what an unrecorded call is
/// worth. A refusal before any attempt is the door's own `Err`: nothing
/// was charged, sent or recorded.
#[derive(Debug)]
#[must_use = "an attempt's record status must be read"]
pub struct ModelCallOutcome {
    result: Result<ModelReply, AuthorityError>,
    record: EndStatus,
}

impl ModelCallOutcome {
    /// What became of the attempt's record: recorded, owed or uncertain.
    pub fn record(&self) -> EndStatus {
        self.record
    }

    /// The transport's result (a reply is untrusted data).
    pub fn result(&self) -> &Result<ModelReply, AuthorityError> {
        &self.result
    }

    pub fn into_result(self) -> Result<ModelReply, AuthorityError> {
        self.result
    }
}

/// A call the runs lock admitted (see [`RunRegistry::admit_model_call`]).
pub(crate) struct Admitted<'a> {
    pub(crate) run: RunId,
    pub(crate) agent: &'a AgentId,
    pub(crate) ticket: CallTicket,
}

/// One admitted call: the watch, the deadline and the cancellation it is
/// ended by, the kernel transport, then its record, whose status is
/// returned with the result. No broker lock is held here.
pub(crate) fn call(
    authority: &Authority,
    stop: &Arc<StopAuthority>,
    model: &RegisteredModel,
    admitted: Admitted<'_>,
    request: &ModelRequest,
    (meter, deadline): (&dyn CallMeter, &dyn CallDeadline),
) -> ModelCallOutcome {
    let Admitted { run, agent, ticket } = admitted;
    let tap = Tap {
        meter,
        charged: OnceLock::new(),
    };
    let result = match stop.admit() {
        Err(_) => Err(ModelError::Stopped),
        Ok(watch) => {
            let deadline = Earliest {
                caller: deadline,
                own: AbsoluteDeadline::within(SystemClocks, CALL_HARD_TIMEOUT),
            };
            let cancel = RunCancel {
                ticket,
                runs: authority.runs().clone(),
            };
            let ctx = CallContext {
                meter: &tap,
                cancel: &cancel,
                deadline: &deadline,
                stop: &watch,
                // M2e-A1b seam: the broker's dispatch observer (the send
                // time, the `Dispatch` notification) is M2e-B's deferred
                // half; until it is added, no observer is given.
                observer: None,
            };
            model.transport.call(request, &ctx)
        }
    };
    let timing = CallTiming {
        sent_mono_ms: None,
        returned_mono_ms: host_monotonic_ms(),
    };
    let detail = call_detail(
        model,
        request.rendered_bytes(),
        tap.charged.get().copied(),
        &result,
        timing,
    );
    // The call has happened: its record is an end record (owed, never
    // waited for), written with no lock held; what became of it is
    // reported with the result.
    let record = authority.record_model_call(run, agent, detail);
    ModelCallOutcome {
        result: result.map_err(AuthorityError::ModelCall),
        record,
    }
}

/// A `model_call` record's detail: the registered keys only.
fn call_detail(
    model: &RegisteredModel,
    prompt_bytes: usize,
    call: Option<u32>,
    result: &Result<ModelReply, ModelError>,
    timing: CallTiming,
) -> Vec<(String, String)> {
    let mut detail = vec![("op".to_string(), "model_call".to_string())];
    if let Some(call) = call {
        detail.push(("call".into(), call.to_string()));
    }
    detail.push(("prompt_bytes".into(), prompt_bytes.to_string()));
    if let Ok(reply) = result {
        detail.push(("bytes".into(), reply.content().len().to_string()));
        detail.push(("tokens_in".into(), reply.prompt_tokens().to_string()));
        detail.push(("tokens_out".into(), reply.output_tokens().to_string()));
        detail.push(("latency_ms".into(), reply.latency_ms().to_string()));
    }
    detail.push(("model_digest".into(), model.view.digest.clone()));
    detail.push(("identity".into(), model.view.identity.clone()));
    let outcome = match result {
        Ok(_) => "ok",
        Err(error) => error.code(),
    };
    detail.push(("outcome".into(), outcome.into()));
    if let Some(returned) = timing.returned_mono_ms {
        detail.push(("returned_mono_ms".into(), returned.to_string()));
    }
    if let Some(sent) = timing.sent_mono_ms {
        detail.push(("sent_mono_ms".into(), sent.to_string()));
    }
    detail
}

/// The class of a model-call refusal: `model_` and the kernel's code.
pub(crate) fn model_class(error: ModelError) -> &'static str {
    MODEL_CLASSES[error as usize]
}

/// `model_` and each of `ModelError::CODES`, in its order (a test keeps
/// them in step; a new kernel code changes the length and fails to build
/// here).
pub(crate) const MODEL_CLASSES: [&str; ModelError::CODES.len()] = [
    "model_not_local",
    "model_invalid_address",
    "model_invalid_model_name",
    "model_cloud_name",
    "model_cloud_enabled",
    "model_not_installed",
    "model_digest_mismatch",
    "model_runtime_mismatch",
    "model_remote",
    "model_config_mismatch",
    "model_unavailable",
    "model_timeout",
    "model_request_too_large",
    "model_response_too_large",
    "model_malformed",
    "model_truncated",
    "model_tool_calls",
    "model_thinking_leaked",
    "model_empty_content",
    "model_over_budget",
    "model_stopped",
    "model_cancelled",
    "model_late",
    "model_unsafe_delimiter",
];

/// The `outcome` codes a `model_call` record may hold: `ok` and each of
/// `ModelError::CODES`.
pub(crate) const OUTCOMES: [&str; ModelError::CODES.len() + 1] = outcomes();

const fn outcomes() -> [&'static str; ModelError::CODES.len() + 1] {
    let mut out = ["ok"; ModelError::CODES.len() + 1];
    let mut i = 0;
    while i < ModelError::CODES.len() {
        out[i + 1] = ModelError::CODES[i];
        i += 1;
    }
    out
}

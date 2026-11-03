//! The kernel's local model transport (M2e-A1).
//!
//! One qualified local model, served by the operator's Ollama on a loopback
//! address, reached only through [`VerifiedOllama`]:
//!
//! - **Address.** A [`LoopbackEndpoint`] is made only by parsing, holds an
//!   IP literal (`127.0.0.1` or `::1`), and is checked again before every
//!   connection. No proxy, no redirect.
//! - **Identity.** A [`ModelIdentity`] is the model's name and manifest
//!   digest, the runtime's version and a fixed [`RuntimeConfig`].
//!   Registration checks all of it, plus the runtime's cloud status and the
//!   model's description. Before every charge the runtime's version, its
//!   cloud status and the model's digest are checked again.
//! - **Charge.** Every call is charged to the caller's [`CallMeter`]
//!   immediately before the chat request and settled on every path after
//!   it. A refused charge sends nothing. There is no retry and no refund.
//! - **Cancellation.** Every request, metadata included, runs through one
//!   wrapper that ends it on the caller's cancellation, the caller's own
//!   emergency-stop watch, or the caller's absolute deadline, and closes its
//!   connection. Its thread is joined before the call returns.
//! - **No silent truncation.** The request is exactly two messages whose
//!   rendered content is bounded in bytes, sent with `truncate: false`, and
//!   the reply is refused when the runtime reports anything that could mean
//!   the prompt or the answer was cut.
//!
//! Errors and `Debug` output carry fixed codes, sizes and counts only: never
//! a prompt, a reply, a metadata body or a model name.

mod endpoint;
mod exchange;
pub mod grammar;
mod identity;
pub mod ollama;
mod request;

#[cfg(test)]
mod tests;

use thiserror::Error;

use crate::emergency_stop::StopWatch;

pub use endpoint::LoopbackEndpoint;
pub use exchange::{AbsoluteDeadline, DeadlineClock, SystemClocks, CALL_HARD_TIMEOUT};
pub use identity::{ModelIdentity, ModelPinSpec, RuntimeConfig, Think, LOCAL_PROVIDER};
pub use ollama::VerifiedOllama;
pub use request::{
    Conversation, ConversationConfig, MessageRole, ModelReply, ModelRequest, ResponseSchema,
    RetainedAnswer, ANSWER_CAP, CONVERSATION_CONFIG, DELIMITER_INVENTORY,
    DELIMITER_INVENTORY_SHA256, FRAME_CLOSE, FRAME_OPEN, MARGIN, MAX_CONVERSATION_MESSAGES,
    MAX_RENDERED_CONTENT_BYTES, PER_MESSAGE_ALLOWANCE, QUALIFIED_CONVERSATION_CONFIG_SHA256,
    RETAINED_ANSWER_ENCODING, SCAFFOLD_FLOOR, SUFFIX_ALLOWANCE, TEMPLATE_ALLOWANCE,
    USER_FRAME_VERSION,
};

/// Whether N1's conversation route (the retained worker's native ordered
/// history, M2e N1 spec) has passed its installed re-qualification Q-R-N1.
/// While it is `false`, every conversation request is refused at
/// [`ModelTransport::call`] and by the coding run's `LocalOllama::bind`
/// with [`ModelError::ConfigMismatch`], before any charge or connection.
/// Only a separate commit carrying the Q-R-N1 evidence may set it.
pub const N1_QUALIFIED: bool = false;

/// Whether the conversation route may be used: [`N1_QUALIFIED`], or, in
/// this crate's own tests only, the private qualification seam.
#[allow(dead_code)] // M2e-A2 stub: consulted from the implementation commit
pub(crate) fn conversation_route_open() -> bool {
    N1_QUALIFIED || qualification_override()
}

/// No override exists outside this crate's own tests.
#[cfg(not(test))]
#[allow(dead_code)] // M2e-A2 stub: consulted from the implementation commit
const fn qualification_override() -> bool {
    false
}

#[cfg(test)]
#[allow(dead_code)] // M2e-A2 stub: consulted from the implementation commit
fn qualification_override() -> bool {
    qualification_seam::is_open()
}

/// The private, test-only qualification seam (N1 spec E-O65): it opens the
/// conversation route on the calling thread only, for the `#[ignore]`d
/// E-O66 and the Q-R-N1 harness, which run the same request-building and
/// validation code as production. No environment variable, feature or
/// public item reaches it.
#[cfg(test)]
pub(crate) mod qualification_seam {
    use std::cell::Cell;

    thread_local! {
        static OPEN: Cell<bool> = const { Cell::new(false) };
    }

    /// Run `body` with the route open on this thread.
    pub(crate) fn open<T>(body: impl FnOnce() -> T) -> T {
        struct Close;
        impl Drop for Close {
            fn drop(&mut self) {
                OPEN.with(|open| open.set(false));
            }
        }
        OPEN.with(|open| open.set(true));
        let _close = Close;
        body()
    }

    pub(crate) fn is_open() -> bool {
        OPEN.with(Cell::get)
    }
}

/// Why a model call or a registration was refused. Each variant is a fixed
/// code ([`ModelError::code`]) with a fixed phrase: no name, path or body.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum ModelError {
    #[error("the model address is not a loopback address")]
    NotLocal,
    #[error("the model address is invalid")]
    InvalidAddress,
    #[error("the model name is invalid")]
    InvalidModelName,
    #[error("the model is a cloud model")]
    CloudName,
    #[error("the local runtime does not report its cloud features disabled")]
    CloudEnabled,
    #[error("the model is not installed in the local runtime")]
    NotInstalled,
    #[error("the installed model's digest differs from the pinned one")]
    DigestMismatch,
    #[error("the local runtime's version differs from the pinned one")]
    RuntimeMismatch,
    #[error("the local runtime relays the model to a remote host")]
    Remote,
    #[error("the model configuration is not the qualified one")]
    ConfigMismatch,
    #[error("the local model is unavailable")]
    Unavailable,
    #[error("the model call reached its deadline")]
    Timeout,
    #[error("the model request exceeds its size bound")]
    RequestTooLarge,
    #[error("the model response exceeds its size bound")]
    ResponseTooLarge,
    #[error("the model response is malformed")]
    Malformed,
    #[error("the model reply may have been truncated")]
    Truncated,
    #[error("the model replied through tool calls")]
    ToolCalls,
    #[error("the model replied with thinking that was switched off")]
    ThinkingLeaked,
    #[error("the model reply is empty")]
    EmptyContent,
    #[error("the model call budget refused the call")]
    OverBudget,
    #[error("the owner's emergency stop ended the model call")]
    Stopped,
    #[error("the model call was cancelled")]
    Cancelled,
    #[error("the model reply arrived after the call was ended")]
    Late,
    #[error("the model request contains a delimiter of the pinned template")]
    UnsafeDelimiter,
}

impl ModelError {
    /// Every code, in declaration order.
    pub const CODES: [&'static str; 24] = [
        "not_local",
        "invalid_address",
        "invalid_model_name",
        "cloud_name",
        "cloud_enabled",
        "not_installed",
        "digest_mismatch",
        "runtime_mismatch",
        "remote",
        "config_mismatch",
        "unavailable",
        "timeout",
        "request_too_large",
        "response_too_large",
        "malformed",
        "truncated",
        "tool_calls",
        "thinking_leaked",
        "empty_content",
        "over_budget",
        "stopped",
        "cancelled",
        "late",
        "unsafe_delimiter",
    ];

    /// The fixed code of this error (one of [`ModelError::CODES`]).
    pub fn code(self) -> &'static str {
        Self::CODES[self as usize]
    }
}

/// The index a meter assigned to one charged call (1-based).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Charged {
    pub call: u32,
}

/// Why a meter refused a charge. The transport treats every refusal alike:
/// [`ModelError::OverBudget`], and nothing is sent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeterRefusal {
    /// The calls or the budget are used up.
    Exhausted,
    /// Another charged call of the same entitlement has not settled.
    InFlight,
    /// The entitlement no longer belongs to the live invocation.
    Stale,
    /// The caller's time is used up.
    OutOfTime,
    /// The charge could not be recorded before dispatch.
    NotRecorded,
}

/// Why the caller cancelled a call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CancelCause {
    /// The run or the mission was cancelled.
    Cancelled,
    /// The door or the lane was halted or overtaken.
    Halted,
    /// The session ended.
    Closed,
}

/// The caller's per-call budget. `charge` assigns the call index; the
/// transport calls it immediately before the chat request and calls
/// `settle` with that index on every path after it. Nothing is refunded.
pub trait CallMeter: Send + Sync {
    fn charge(&self) -> Result<Charged, MeterRefusal>;
    fn settle(&self, call: u32);
}

/// The caller's cancellation, polled while a request is in flight.
pub trait CallCancel: Send + Sync {
    fn cancelled(&self) -> Option<CancelCause>;
}

/// The caller's absolute deadline: fixed once by its owner and never
/// rebuilt (see [`AbsoluteDeadline`]).
pub trait CallDeadline: Send + Sync {
    fn expired(&self) -> bool;
}

/// The caller's content-free notice that a chat request was handed to the
/// HTTP client (M2e amendment, M3 design §8.5).
///
/// `dispatched` is called at most once per call, for the chat request
/// only (never for the version, status, tags or show requests), with the
/// host's CLOCK_MONOTONIC in milliseconds read immediately before the
/// request is handed to the HTTP client. It is never called for a call
/// refused before that (an identity check, the cloud status, the byte
/// bound, the meter, or a Stop, cancel or deadline before the send), and no
/// value is ever estimated: where the clock cannot be read, there is no
/// notice. It marks the hand-off, not decoding.
///
/// It runs on the request's own thread while the request waits for it, so
/// it must be fast and must never block: until it returns, the call cannot
/// be ended by a cancel, a Stop or a deadline. It holds no lock of the
/// transport's. In an unwinding build a panicking observer cannot prevent or
/// fake a send: its unwinding is caught, its panic payload is forgotten
/// rather than dropped (its destructor is the observer's code and never
/// runs on the transport path; a buggy observer leaks at most one payload
/// per call), the request is handed to the client as if it had returned,
/// and whatever it recorded is the observer's own affair. Nothing is
/// claimed under `panic = "abort"`, or for a panic raised while already
/// unwinding: both end the process.
pub trait CallObserver: Send + Sync {
    fn dispatched(&self, sent_mono_ms: u64);
}

/// What a call is charged to and ended by. `stop` is the watch the caller's
/// work was admitted under; the transport never admits a watch of its own.
/// `observer`, if any, is told when the chat request is dispatched.
pub struct CallContext<'a> {
    pub meter: &'a dyn CallMeter,
    pub cancel: &'a dyn CallCancel,
    pub deadline: &'a dyn CallDeadline,
    pub stop: &'a StopWatch,
    pub observer: Option<&'a dyn CallObserver>,
}

/// A model a caller may consult. Its reply is untrusted data.
pub trait ModelTransport: Send + Sync {
    /// The identity this transport was verified for.
    fn identity(&self) -> &ModelIdentity;
    /// One charged call.
    fn call(&self, request: &ModelRequest, ctx: &CallContext<'_>)
        -> Result<ModelReply, ModelError>;
}

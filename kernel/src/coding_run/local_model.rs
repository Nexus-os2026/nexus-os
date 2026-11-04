//! Local-only model access for coding-run workers (Phase One, decision D5;
//! refitted onto the kernel transport in M2e-A2).
//!
//! A coding run talks to exactly one model, pinned into the run before the
//! worker starts: a model served by the operator's local Ollama and verified
//! by the kernel transport ([`VerifiedOllama`]) for one full identity (name,
//! manifest digest, runtime version and request configuration). The address
//! is the operator-authorized Ollama address (the backend passes it in) and
//! must be a loopback address; any other host, including a remote Ollama,
//! makes local models unavailable. There is no provider choice, no fallback
//! and no cloud route: if the local model cannot answer, the run fails.
//!
//! Every request goes through the kernel transport: in-process, without a
//! proxy, without following redirects, bounded in time and size, charged to
//! the run's meter before the chat, and ended by the run's cancellation,
//! its absolute deadline or the emergency-stop watch the run was admitted
//! under. The retained worker's complete ordered history is carried as one
//! [`Conversation`] (M2e N1). That route stays shut until its installed
//! re-qualification (`inference::N1_QUALIFIED`): until then
//! [`LocalOllama::bind`] refuses.

use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use sha2::{Digest, Sha256};

use crate::emergency_stop::StopWatch;
use crate::inference::{
    grammar, AbsoluteDeadline, CallCancel, CallContext, CallDeadline, CallMeter, Conversation,
    DeadlineClock, LoopbackEndpoint, MessageRole, ModelRequest, ModelTransport, RetainedAnswer,
    RuntimeConfig, SystemClocks, VerifiedOllama, CONVERSATION_CONFIG,
};

/// The local model errors: the kernel transport's codes.
pub use crate::inference::ModelError;
/// The only provider a coding run can use.
pub use crate::inference::LOCAL_PROVIDER;

/// A checked loopback Ollama address (see [`loopback_endpoint`]).
pub type LocalEndpoint = LoopbackEndpoint;

/// The provider, loopback endpoint and full identity of the model a run is
/// bound to: the model's name and manifest digest, the runtime's version
/// and the digest of the qualified request configuration. Two pins are
/// equal only when every field is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelPin {
    provider: &'static str,
    endpoint: String,
    model: String,
    digest: String,
    runtime: String,
    config_digest: [u8; 32],
}

impl ModelPin {
    /// The pin of a verified local model: the only way to make one.
    pub fn of(transport: &VerifiedOllama) -> Self {
        let identity = transport.identity();
        Self {
            provider: LOCAL_PROVIDER,
            endpoint: format!("http://{}", transport.endpoint().addr()),
            model: identity.name().to_string(),
            digest: identity.model_digest().to_string(),
            runtime: identity.runtime_version().to_string(),
            config_digest: model_config_digest(identity.config()),
        }
    }

    pub fn provider(&self) -> &str {
        self.provider
    }

    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }

    pub fn model(&self) -> &str {
        &self.model
    }

    /// The model's manifest digest (hex, as the runtime lists it).
    pub fn digest(&self) -> &str {
        &self.digest
    }

    /// The runtime's version.
    pub fn runtime(&self) -> &str {
        &self.runtime
    }

    /// SHA-256 of the qualified request configuration: the runtime
    /// configuration and the conversation configuration.
    pub fn config_digest(&self) -> [u8; 32] {
        self.config_digest
    }
}

/// SHA-256 over `nexus.coding_run.model_config.v1` and the length-prefixed
/// digests of the runtime configuration and the conversation configuration.
pub(crate) fn model_config_digest(config: &RuntimeConfig) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"nexus.coding_run.model_config.v1");
    for part in [config.digest(), CONVERSATION_CONFIG.digest()] {
        hash.update(32u32.to_be_bytes());
        hash.update(part);
    }
    hash.finalize().into()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelRole {
    System,
    User,
    Assistant,
}

impl ModelRole {
    fn as_str(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::User => "user",
            Self::Assistant => "assistant",
        }
    }

    fn of(self) -> MessageRole {
        match self {
            Self::System => MessageRole::System,
            Self::User => MessageRole::User,
            Self::Assistant => MessageRole::Assistant,
        }
    }
}

/// One message of a worker's history. `Debug` shows its role and size only.
#[derive(Clone, PartialEq, Eq)]
pub struct ModelMessage {
    pub role: ModelRole,
    pub content: String,
}

impl std::fmt::Debug for ModelMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ModelMessage")
            .field("role", &self.role.as_str())
            .field("content_bytes", &self.content.len())
            .finish()
    }
}

/// A model a coding-run worker may consult. Its answer is untrusted data.
pub trait LocalModel: Send + Sync {
    /// The pin this model answers under.
    fn pin(&self) -> &ModelPin;
    /// One chat completion, bounded in time and size.
    fn complete(
        &self,
        messages: &[ModelMessage],
        timeout: Duration,
        max_response_bytes: u64,
    ) -> Result<String, ModelError>;
}

/// Check the operator-authorized Ollama address and require loopback: the
/// kernel transport's [`LoopbackEndpoint::parse`] (`127.0.0.1` or `::1`
/// with a written port; `localhost` becomes `127.0.0.1`). Any other host —
/// a LAN or remote Ollama, a cloud endpoint — is refused: coding runs use
/// local models only.
pub fn loopback_endpoint(raw: &str) -> Result<LocalEndpoint, ModelError> {
    LoopbackEndpoint::parse(raw)
}

/// The operator's local Ollama, bound to one run's worker.
pub struct LocalOllama {
    pin: ModelPin,
    transport: Arc<VerifiedOllama>,
    meter: Arc<dyn CallMeter>,
    cancel: Arc<dyn CallCancel>,
    deadline: Arc<dyn CallDeadline>,
    stop: StopWatch,
    slot: Mutex<Slot>,
    /// The clocks a call's own deadline is kept on.
    clock: CallClock,
}

/// The clocks of a call's own deadline: the system's, or in this crate's
/// tests a manual pair.
#[derive(Clone)]
enum CallClock {
    System,
    #[cfg(test)]
    Manual(Arc<dyn DeadlineClock>),
}

impl DeadlineClock for CallClock {
    fn wall_ms(&self) -> u64 {
        match self {
            CallClock::System => SystemClocks.wall_ms(),
            #[cfg(test)]
            CallClock::Manual(clock) => clock.wall_ms(),
        }
    }

    fn monotonic_ms(&self) -> u64 {
        match self {
            CallClock::System => SystemClocks.monotonic_ms(),
            #[cfg(test)]
            CallClock::Manual(clock) => clock.monotonic_ms(),
        }
    }
}

/// The one conversation this bound model carries (N1 spec §2.1: its
/// continuity state belongs to one bound run and is never mixed).
#[derive(Default)]
struct Slot {
    conversation: Option<Conversation>,
    /// A reply to the conversation's current history was accepted.
    answered: bool,
    /// A call is in flight (the conversation is checked out).
    busy: bool,
    /// A history that was not this conversation's next one was offered.
    poisoned: bool,
}

impl std::fmt::Debug for LocalOllama {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LocalOllama")
            .field("pin", &self.pin)
            .finish_non_exhaustive()
    }
}

/// The bound deadline, shortened by one call's own timeout.
struct Shortened<'a> {
    bound: &'a dyn CallDeadline,
    call: AbsoluteDeadline<CallClock>,
}

impl CallDeadline for Shortened<'_> {
    fn expired(&self) -> bool {
        self.bound.expired() || self.call.expired()
    }
}

impl LocalOllama {
    /// Bind the verified local model for one run's worker: every call is
    /// charged to `meter` and ended by `cancel`, the absolute `deadline` and
    /// `stop`, the watch the run's work was admitted under (never one of
    /// its own). While N1's route is shut ([`crate::inference::N1_QUALIFIED`])
    /// this is [`ModelError::ConfigMismatch`]; it is also refused for an
    /// unqualified conversation configuration or context, and for a watch
    /// already stopped.
    pub fn bind(
        transport: Arc<VerifiedOllama>,
        meter: Arc<dyn CallMeter>,
        cancel: Arc<dyn CallCancel>,
        deadline: Arc<dyn CallDeadline>,
        stop: StopWatch,
    ) -> Result<Self, ModelError> {
        if !crate::inference::conversation_route_open() {
            return Err(ModelError::ConfigMismatch);
        }
        CONVERSATION_CONFIG.check()?;
        if transport.identity().config().num_ctx != CONVERSATION_CONFIG.num_ctx {
            return Err(ModelError::ConfigMismatch);
        }
        stop.check().map_err(|_| ModelError::Stopped)?;
        Ok(Self {
            pin: ModelPin::of(&transport),
            transport,
            meter,
            cancel,
            deadline,
            stop,
            slot: Mutex::default(),
            clock: CallClock::System,
        })
    }

    /// Keep this model's per-call deadlines on `clock` (tests only).
    #[cfg(test)]
    pub(crate) fn with_call_clock(mut self, clock: Arc<dyn DeadlineClock>) -> Self {
        self.clock = CallClock::Manual(clock);
        self
    }

    fn slot(&self) -> std::sync::MutexGuard<'_, Slot> {
        self.slot.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Take the conversation out for one call, brought up to `messages`:
    /// a new conversation for the system and opening messages; the same
    /// history again only if no reply to it was accepted; or the held
    /// history plus one retained answer and one feedback after an accepted
    /// reply. Anything else is [`ModelError::Malformed`] and ends this
    /// model's use. No lock is held across the call itself.
    fn checkout(&self, messages: &[ModelMessage]) -> Result<Conversation, ModelError> {
        let mut slot = self.slot();
        if slot.poisoned || slot.busy {
            return Err(ModelError::Malformed);
        }
        let Some(mut conversation) = slot.conversation.take() else {
            let [system, opening] = messages else {
                return Err(ModelError::Malformed);
            };
            if system.role != ModelRole::System || opening.role != ModelRole::User {
                return Err(ModelError::Malformed);
            }
            let conversation = Conversation::new(system.content.clone(), opening.content.clone())?;
            slot.busy = true;
            return Ok(conversation);
        };
        let held = conversation.len();
        let extends = messages.len() >= held
            && messages[..held].iter().enumerate().all(|(index, message)| {
                conversation.message(index) == Some((message.role.of(), message.content.as_str()))
            });
        let next = match (&messages[held.min(messages.len())..], slot.answered) {
            ([], false) if extends => Ok(()),
            ([answer, feedback], true) if extends => extend(&mut conversation, answer, feedback),
            _ => Err(ModelError::Malformed),
        };
        match next {
            Ok(()) => {
                slot.answered = false;
                slot.busy = true;
                Ok(conversation)
            }
            Err(error) => {
                slot.poisoned = true;
                Err(error)
            }
        }
    }

    /// Return the conversation after its call.
    fn checkin(&self, conversation: Conversation, answered: bool) {
        let mut slot = self.slot();
        slot.conversation = Some(conversation);
        slot.answered = answered;
        slot.busy = false;
    }
}

/// Append the worker's retained answer and the backend's feedback.
fn extend(
    conversation: &mut Conversation,
    answer: &ModelMessage,
    feedback: &ModelMessage,
) -> Result<(), ModelError> {
    if answer.role != ModelRole::Assistant || feedback.role != ModelRole::User {
        return Err(ModelError::Malformed);
    }
    // The worker sends back exactly the retained form it was given.
    let retained = RetainedAnswer::of(&answer.content)?;
    if retained.as_str() != answer.content {
        return Err(ModelError::Malformed);
    }
    conversation.push_answer(retained)?;
    conversation.push_feedback(feedback.content.clone())
}

impl LocalModel for LocalOllama {
    fn pin(&self) -> &ModelPin {
        &self.pin
    }

    /// One call carrying the complete history. `timeout` can only shorten
    /// the bound deadline. A reply longer than `max_response_bytes` is
    /// [`ModelError::ResponseTooLarge`].
    fn complete(
        &self,
        messages: &[ModelMessage],
        timeout: Duration,
        max_response_bytes: u64,
    ) -> Result<String, ModelError> {
        let mut conversation = self.checkout(messages)?;
        let outcome = ModelRequest::conversation(&conversation, grammar::worker_schema())
            .and_then(|request| {
                let deadline = Shortened {
                    bound: &*self.deadline,
                    call: AbsoluteDeadline::within(self.clock.clone(), timeout),
                };
                let ctx = CallContext {
                    meter: &*self.meter,
                    cancel: &*self.cancel,
                    deadline: &deadline,
                    stop: &self.stop,
                    observer: None,
                };
                self.transport.call(&request, &ctx)
            })
            .and_then(|reply| conversation.accept(&reply).map(|()| reply));
        self.checkin(conversation, outcome.is_ok());
        let content = outcome?.into_content();
        if content.len() as u64 > max_response_bytes {
            return Err(ModelError::ResponseTooLarge);
        }
        Ok(content)
    }
}

/// The qualified test pin's model manifest digest and runtime version.
#[cfg(test)]
pub(crate) const TEST_PIN_DIGEST: &str =
    "a7eb95c53bcf96b4bdd008d0fab4a5dac88047d9c1a7a9ab88ed453423fbd87c";
#[cfg(test)]
pub(crate) const TEST_PIN_RUNTIME: &str = "0.40.13";

/// The qualified runtime configuration, for test pins.
#[cfg(test)]
pub(crate) fn test_pin_config() -> RuntimeConfig {
    RuntimeConfig {
        num_ctx: 8192,
        temperature_milli: 0,
        seed: 7,
        think: crate::inference::Think::Off,
        truncate: false,
    }
}

/// A pin to the qualified identity at the default loopback endpoint, with
/// this model name (the scripted test models' pins).
#[cfg(test)]
pub(crate) fn pin_for_test(model: &str) -> ModelPin {
    pin_for_test_at("http://127.0.0.1:11434", model)
}

/// A pin with any endpoint and model spelling, unchecked: the P0-B ledger
/// oracles plant canaries in both, and the record must not depend on the
/// spelling being one the constructor accepts.
#[cfg(test)]
pub(crate) fn pin_for_test_at(endpoint: &str, model: &str) -> ModelPin {
    ModelPin {
        provider: LOCAL_PROVIDER,
        endpoint: endpoint.to_string(),
        model: model.to_string(),
        digest: TEST_PIN_DIGEST.to_string(),
        runtime: TEST_PIN_RUNTIME.to_string(),
        config_digest: model_config_digest(&test_pin_config()),
    }
}

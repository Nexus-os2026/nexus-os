//! The desktop's dispatch adapters (P3-HM-CONTRACT-02 §11.0, §12.4;
//! mission P3-C11-HM-M3 R3–R5).
//!
//! Every model request of a unit goes through
//! [`nexus_kernel::hm::DispatchIssuer::dispatch`]: the envelope is built
//! from the unit's context, the transport is chosen from that context alone
//! (its route class and local-only restriction, never a thread-local, R4),
//! attested by `attest_route` on the provider actually selected, admitted
//! (§12.4) and its reply accepted (§12.6). The issuer is read through
//! [`AppState::hm_issuer`]; without one nothing is dispatched (R6).
//!
//! - A local unit may use only an attested local transport: the verified
//!   loopback Ollama runtime (L1), proven inside each dispatch's build
//!   (AMENDMENT 1 A1.6), or the in-process Flash provider (L2).
//! - Flash comes only from the backend-owned provider cache, chosen by the
//!   cache's own keys, never by a caller-supplied session id; a Flash error
//!   is an error, and no other provider is tried (A1.7).
//! - [`GatewayPlannerLlm`] implements the six model traits for agent units
//!   (and dream units); a HiveMind session's admission is requalified before
//!   each of its dispatches (§11.3).
//! - Which model is asked ([`ModelChoice`]) never decides whether a prompt
//!   may leave the machine: a goal's own loop asks for the agent's route
//!   (its goal override, model mapping or assignment), as the cognitive
//!   cycle always did; the work around agents (a team, a HiveMind session's
//!   decomposition and merge, evolution, dreams) asks for the default.
//! - Chat turns (AC, HD), the temporal fork (HB), the fixed connection test
//!   (SY) and the excluded subsystems (XS: RAG, builder) dispatch their
//!   human-supplied or constant text through [`dispatch_text`] under an
//!   explicit context; a subsystem that calls a provider itself gets an
//!   [`IssuedProvider`] (A2.4 I1).

use std::cell::{Cell, RefCell};
use std::sync::Arc;

use nexus_connectors_llm::attest::{attest_route, verify_local_ollama, VerifiedLocalOllama};
use nexus_connectors_llm::providers::{EmbeddingResponse, FlashProvider, LlmProvider, LlmResponse};
use nexus_kernel::errors::AgentError;
use nexus_kernel::hm::{
    DerivedItem, DispatchContext, DispatchError, DispatchIssuer, Principal, PromptBuilder,
    PromptSource, RouteAttestation, RouteClass, Unit,
};
use uuid::Uuid;

use crate::commands::cognitive::{
    local_only_target, planner_provider, strip_think_tags, HivemindAdmission, LocalOllamaRuntime,
    OllamaLocalRuntime,
};
use crate::{AgentLlmPolicy, AppState};

/// The transport a dispatch selected.
pub(crate) enum AgentTransport {
    /// The in-process Flash provider of the backend-owned cache.
    Flash(Arc<FlashProvider>),
    /// The verified loopback Ollama runtime (L1 proof).
    Local(VerifiedLocalOllama),
    /// Any other provider and the model it is asked for (non-local).
    Provider(Box<dyn LlmProvider>, String),
}

impl AgentTransport {
    /// `attest_route` on the provider object itself (§2.4, §11.0).
    pub(crate) fn attest(&self) -> RouteAttestation {
        match self {
            AgentTransport::Flash(flash) => attest_route(flash),
            AgentTransport::Local(local) => attest_route(local),
            AgentTransport::Provider(provider, _) => attest_route(provider),
        }
    }

    /// Sends the admitted prompt; returns the reply text and its tokens.
    fn send(self, prompt: &str, max_tokens: u32) -> Result<(String, u64), String> {
        #[cfg(test)]
        capture::record(prompt);
        match self {
            AgentTransport::Flash(flash) => {
                eprintln!(
                    "[planner] using Flash Inference, prompt len={} chars",
                    prompt.len()
                );
                // In a dedicated OS thread: a crash in llama.cpp (FFI) ends
                // the thread, not the app.
                let prompt_owned = prompt.to_string();
                let handle = std::thread::spawn(move || {
                    flash.query(&prompt_owned, max_tokens.max(2048), "flash")
                });
                match handle.join() {
                    Ok(Ok(response)) => Ok((response.output_text, response.token_count as u64)),
                    Ok(Err(e)) => Err(e.to_string()),
                    Err(_) => Err(
                        "Flash Inference crashed during query — the model may be corrupted or \
                         out of memory. Try reloading the model from the Flash Inference page."
                            .to_string(),
                    ),
                }
            }
            AgentTransport::Local(local) => {
                let model = local.model().to_string();
                guarded_query(&local, prompt, max_tokens, &model)
            }
            AgentTransport::Provider(provider, model) => {
                guarded_query(provider.as_ref(), prompt, max_tokens, &model)
            }
        }
    }
}

/// `provider.query` under catch_unwind: a provider crash is an error.
fn guarded_query(
    provider: &dyn LlmProvider,
    prompt: &str,
    max_tokens: u32,
    model: &str,
) -> Result<(String, u64), String> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        provider.query(prompt, max_tokens, model)
    }))
    .unwrap_or_else(|_panic| {
        Err(AgentError::SupervisorError(
            "LLM provider panicked during query — model may be corrupted or out of memory"
                .to_string(),
        ))
    })
    .map(|response| (response.output_text, response.token_count as u64))
    .map_err(|e| e.to_string())
}

/// The local runtime of a local unit's dispatch: the D-2 R2 queries of
/// [`OllamaLocalRuntime`], keeping the (L1) proof its verification yields.
#[derive(Default)]
struct ProvingRuntime {
    proof: RefCell<Option<VerifiedLocalOllama>>,
}

impl LocalOllamaRuntime for ProvingRuntime {
    fn installed_models(&self, base: &str) -> Result<Vec<String>, String> {
        OllamaLocalRuntime.installed_models(base)
    }

    fn verify_local_model(&self, base: &str, model: &str) -> Result<(), String> {
        let proof = verify_local_ollama(base, model).map_err(|e| e.to_string())?;
        *self.proof.borrow_mut() = Some(proof);
        Ok(())
    }
}

/// Whether a route names a Flash model.
fn wants_flash(model: &str) -> bool {
    model.starts_with("flash:") || model.starts_with("flash/") || model == "flash"
}

/// The Flash provider of the backend-owned cache (§2.4 L2 item 2): the
/// entry with the least key, chosen from the cache itself; a session id in
/// a route is never used to look one up.
fn cached_flash(state: &AppState) -> Option<Arc<FlashProvider>> {
    let cache = state
        .flash_providers
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    cache
        .iter()
        .min_by(|a, b| a.0.cmp(b.0))
        .map(|(_, provider)| provider.clone())
}

/// Which model a dispatch asks for. The unit's context alone decides
/// whether it may leave the machine (R4); this only picks the model.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum ModelChoice {
    /// The agent's own route: the planning and steps of its goal loop.
    Agent,
    /// The default selection: the work around agents.
    Default,
}

/// How one dispatch is made.
#[derive(Clone, Copy)]
pub(crate) struct DispatchOptions<'a> {
    /// A HiveMind session's admission, requalified before the dispatch.
    pub(crate) hive: Option<&'a HivemindAdmission>,
    /// The model asked for.
    pub(crate) model: ModelChoice,
    /// The reply's token limit.
    pub(crate) max_tokens: u32,
    /// How the reply text is post-processed.
    pub(crate) text: ReplyText,
}

/// Whether the unit may use only an attested local transport (R4).
pub(crate) fn unit_is_local(ctx: &DispatchContext) -> bool {
    ctx.route_class() == RouteClass::Local || ctx.restrictions().local_only()
}

/// The transport for the unit `ctx` (R4: from the context alone). A local
/// unit gets the verified loopback runtime (its proof made here, for this
/// dispatch) or the cached Flash provider; anything else is non-local.
pub(crate) fn select_transport(
    state: &AppState,
    ctx: &DispatchContext,
    model: ModelChoice,
) -> Result<AgentTransport, String> {
    let local = unit_is_local(ctx);
    let route = match (model, ctx.principal()) {
        (ModelChoice::Agent, Principal::Agent(id)) => {
            crate::resolve_agent_llm_route(state, ctx, &id.to_string(), local)
        }
        _ => None,
    };
    if let Some(route) = route.as_ref().filter(|r| wants_flash(&r.model)) {
        // A Flash route with no loaded model is an error, never another
        // provider (§2.4 L2 item 3).
        let _ = route;
        return cached_flash(state)
            .map(AgentTransport::Flash)
            .ok_or_else(|| {
                "Flash Inference is selected but no model is loaded. \
             Go to the Agents page and click 'Load Model' to start a Flash session."
                    .to_string()
            });
    }
    let ollama_base = crate::commands::chat_llm::authorized_ollama_base_url();
    if local {
        let runtime = ProvingRuntime::default();
        local_only_target(
            route.as_ref().map(|r| r.model.as_str()),
            ollama_base,
            &runtime,
        )?;
        let proof = runtime
            .proof
            .into_inner()
            .ok_or_else(|| "local-only: the local runtime was not verified".to_string())?;
        return Ok(AgentTransport::Local(proof));
    }
    let config = nexus_kernel::config::load_config().unwrap_or_default();
    let prov_config = crate::build_provider_config(&config);
    let (provider, model) = planner_provider(
        AgentLlmPolicy::Unrestricted,
        route.map(|r| r.model),
        &prov_config,
        ollama_base,
        &OllamaLocalRuntime,
    )
    .map_err(|e| e.to_string())?;
    Ok(AgentTransport::Provider(provider, model))
}

/// How a reply is post-processed.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ReplyText {
    /// `<think>` blocks removed (planner-family replies).
    StripThink,
    /// As the model returned it.
    Raw,
}

/// The single dispatch of a unit (§12.4, §12.6): the envelope `prompt`
/// builds, on the transport selected for the (possibly rebuilt) context.
/// Returns the accepted reply and the tokens it used.
pub(crate) fn dispatch_unit(
    state: &AppState,
    ctx: &mut DispatchContext,
    prompt: &mut PromptSource<'_>,
    options: DispatchOptions<'_>,
) -> Result<(DerivedItem, u64), AgentError> {
    dispatch_unit_with(state, ctx, prompt, options, &|_, reply| Ok(reply))
}

/// [`dispatch_unit`] whose reply is checked against the admitted prompt by
/// `finish` before it is accepted; a refused reply fails the dispatch (its
/// disclosure stands).
pub(crate) fn dispatch_unit_with(
    state: &AppState,
    ctx: &mut DispatchContext,
    prompt: &mut PromptSource<'_>,
    options: DispatchOptions<'_>,
    finish: &dyn Fn(&str, String) -> Result<String, String>,
) -> Result<(DerivedItem, u64), AgentError> {
    let DispatchOptions {
        hive,
        model,
        max_tokens,
        text,
    } = options;
    // P3 HiveMind INFOFLOW: a session's work dispatches only while the
    // session still qualifies as admitted; this runs before §12.4.
    if let Some(admission) = hive {
        admission.requalify().map_err(AgentError::SupervisorError)?;
    }
    let issuer = state
        .hm_issuer()
        .map_err(|e| AgentError::SupervisorError(format!("no dispatch issuer: {e}")))?;
    let selection_error: RefCell<Option<String>> = RefCell::new(None);
    let tokens = Cell::new(0u64);
    let result = issuer.dispatch(
        ctx,
        &mut |c: &mut DispatchContext| {
            let envelope = prompt(c)?;
            match select_transport(state, c, model) {
                Ok(transport) => {
                    let attestation = transport.attest();
                    Some((envelope, transport, attestation))
                }
                Err(reason) => {
                    *selection_error.borrow_mut() = Some(reason);
                    None
                }
            }
        },
        &mut |transport: AgentTransport, admitted: &str| {
            let (reply, used) = transport.send(admitted, max_tokens)?;
            tokens.set(used);
            let reply = match text {
                ReplyText::StripThink => {
                    let stripped = strip_think_tags(&reply);
                    eprintln!(
                        "[planner] prompt={} chars -> response={} chars (stripped={} chars)",
                        admitted.len(),
                        reply.len(),
                        stripped.len()
                    );
                    stripped
                }
                ReplyText::Raw => reply,
            };
            finish(admitted, reply)
        },
    );
    match result {
        Ok(reply) => Ok((reply, tokens.get())),
        Err(DispatchError::Refused(nexus_kernel::hm::Refused::NoTransport))
            if selection_error.borrow().is_some() =>
        {
            Err(AgentError::SupervisorError(
                selection_error.into_inner().unwrap_or_default(),
            ))
        }
        Err(DispatchError::Transport(e)) => Err(AgentError::SupervisorError(e)),
        Err(e) => Err(AgentError::SupervisorError(format!(
            "local-only: {e} (P3-HM-CONTRACT-02 §12.4)"
        ))),
    }
}

/// The desktop's model of agent units (and dream units): the six kernel
/// model traits over [`dispatch_unit`]. A HiveMind session's adapter
/// carries the session's admission, requalified before each dispatch.
#[derive(Clone)]
pub(crate) struct GatewayPlannerLlm {
    state: AppState,
    hive: Option<Arc<HivemindAdmission>>,
    model: ModelChoice,
}

impl GatewayPlannerLlm {
    /// The adapter of the work around agents (the default model).
    pub(crate) fn new(state: &AppState) -> Self {
        Self::for_session(state, None)
    }

    /// The adapter of a HiveMind session's work (the default model).
    pub(crate) fn for_session(state: &AppState, admission: Option<Arc<HivemindAdmission>>) -> Self {
        Self {
            state: state.clone(),
            hive: admission,
            model: ModelChoice::Default,
        }
    }

    /// The adapter of a goal's own loop (its planning and steps): the
    /// agent's route; a HiveMind sub-task's loop carries the session's
    /// admission.
    pub(crate) fn for_loop(state: &AppState, admission: Option<Arc<HivemindAdmission>>) -> Self {
        Self {
            state: state.clone(),
            hive: admission,
            model: ModelChoice::Agent,
        }
    }

    fn options(&self, max_tokens: u32, text: ReplyText) -> DispatchOptions<'_> {
        DispatchOptions {
            hive: self.hive.as_deref(),
            model: self.model,
            max_tokens,
            text,
        }
    }

    fn ask(
        &self,
        ctx: &mut DispatchContext,
        prompt: &mut PromptSource<'_>,
    ) -> Result<DerivedItem, AgentError> {
        dispatch_unit(
            &self.state,
            ctx,
            prompt,
            self.options(2048, ReplyText::StripThink),
        )
        .map(|(reply, _)| reply)
    }
}

impl nexus_kernel::cognitive::PlannerLlm for GatewayPlannerLlm {
    fn plan_query(
        &self,
        ctx: &mut DispatchContext,
        prompt: &mut PromptSource<'_>,
    ) -> Result<DerivedItem, AgentError> {
        self.ask(ctx, prompt)
    }
}

impl nexus_kernel::cognitive::LlmQueryHandler for GatewayPlannerLlm {
    fn query(
        &self,
        ctx: &mut DispatchContext,
        prompt: &mut PromptSource<'_>,
    ) -> Result<DerivedItem, String> {
        self.ask(ctx, prompt).map_err(|e| e.to_string())
    }
}

impl nexus_kernel::cognitive::EvolutionLlm for GatewayPlannerLlm {
    fn optimize_prompt(
        &self,
        ctx: &mut DispatchContext,
        prompt: &mut PromptSource<'_>,
    ) -> Result<DerivedItem, String> {
        self.ask(ctx, prompt).map_err(|e| e.to_string())
    }
}

impl nexus_kernel::genome::AutoEvolveLlm for GatewayPlannerLlm {
    fn score_response(
        &self,
        ctx: &mut DispatchContext,
        prompt: &mut PromptSource<'_>,
    ) -> Result<DerivedItem, String> {
        self.ask(ctx, prompt).map_err(|e| e.to_string())
    }

    fn mutate_prompt(
        &self,
        ctx: &mut DispatchContext,
        prompt: &mut PromptSource<'_>,
    ) -> Result<DerivedItem, String> {
        self.ask(ctx, prompt).map_err(|e| e.to_string())
    }

    fn generate_with_prompt(
        &self,
        ctx: &mut DispatchContext,
        prompt: &mut PromptSource<'_>,
    ) -> Result<DerivedItem, String> {
        self.ask(ctx, prompt).map_err(|e| e.to_string())
    }
}

impl nexus_kernel::cognitive::HivemindLlm for GatewayPlannerLlm {
    fn decompose(
        &self,
        ctx: &mut DispatchContext,
        prompt: &mut PromptSource<'_>,
    ) -> Result<DerivedItem, AgentError> {
        self.ask(ctx, prompt)
    }

    fn merge(
        &self,
        ctx: &mut DispatchContext,
        prompt: &mut PromptSource<'_>,
    ) -> Result<DerivedItem, AgentError> {
        self.ask(ctx, prompt)
    }
}

impl nexus_kernel::dreams::engine::DreamLlm for GatewayPlannerLlm {
    fn query(
        &self,
        ctx: &mut DispatchContext,
        prompt: &mut PromptSource<'_>,
        max_tokens: u32,
    ) -> Result<(DerivedItem, u64), String> {
        dispatch_unit(
            &self.state,
            ctx,
            prompt,
            self.options(max_tokens, ReplyText::Raw),
        )
        .map_err(|e| e.to_string())
    }
}

// ── Non-agent units and chat turns (§12.1 AC, HD, HB, SY, XS) ───────────────

/// The issuer of `state`; without one nothing is dispatched (R6).
fn issuer_of(state: &AppState) -> Result<&Arc<DispatchIssuer>, String> {
    state
        .hm_issuer()
        .map_err(|e| format!("no dispatch issuer: {e}"))
}

/// A request unit of a non-agent principal: `OwnerDirect` (HD, HB with no
/// registered agent) or `System(component)` (SY, XS). It carries no
/// retained data and admits no inclusion.
pub(crate) fn request_unit(
    state: &AppState,
    principal: Principal,
) -> Result<DispatchContext, String> {
    issuer_of(state)?
        .begin_unit(
            principal,
            Unit::Request {
                request_id: Uuid::new_v4(),
            },
        )
        .map_err(|e| {
            format!(
                "{}: {e}",
                crate::commands::cognitive::unresolved_policy_error()
            )
        })
}

/// The unit of a chat turn (§2.2 `ChatTurn`; §12.1 AC and HD).
///
/// - With no agent: an `OwnerDirect` turn (HD).
/// - Routed to an agent by name (AC): the name selects the registered agents
///   of that name ([`crate::agents_named`]); none refuses the turn. The
///   principal is the least of their real ids (never the ephemeral chat
///   id). The turn is local-only when any of them is (each by its own §4.2
///   policy) or an assignment under the name is: never less restrictive
///   than the D-2 R2 name rule. A policy that cannot be established, or a
///   configuration that cannot be read, refuses the turn.
pub(crate) fn chat_turn(
    state: &AppState,
    agent_name: Option<&str>,
) -> Result<DispatchContext, String> {
    let issuer = issuer_of(state)?;
    let turn = Unit::ChatTurn {
        turn_id: Uuid::new_v4(),
    };
    let refused = |e: String| {
        format!(
            "{}: {e}",
            crate::commands::cognitive::unresolved_policy_error()
        )
    };
    let Some(name) = agent_name else {
        return issuer
            .begin_unit(Principal::OwnerDirect, turn)
            .map_err(|e| refused(e.to_string()));
    };
    let ids = crate::agents_named(state, name);
    let Some(principal) = ids.first().copied() else {
        return Err(refused(format!("'{name}' names no registered agent")));
    };
    let mut ctx = issuer
        .begin_unit(Principal::Agent(principal), turn)
        .map_err(|e| refused(e.to_string()))?;
    let mut local_only = false;
    for id in &ids[1..] {
        local_only |= issuer
            .resolve_agent_route_policy(*id)
            .map_err(|e| refused(e.to_string()))?
            .local_only();
    }
    let config = nexus_kernel::config::load_config().map_err(|e| refused(e.to_string()))?;
    let keys = [name.to_string(), crate::normalize_agent_config_key(name)];
    local_only |= crate::agent_llm_policy_in(&config, &keys) == AgentLlmPolicy::LocalOnly;
    if local_only {
        ctx.tighten_local_only();
    }
    Ok(ctx)
}

/// The model of a chat turn (AC, HD) or of a human request (HB): the model
/// the human chose (`model_id`), or the configured default. A local-only
/// unit gets only the verified loopback runtime, its L1 proof made here, for
/// this dispatch (A1.6); any other provider is attested non-local.
pub(crate) fn human_provider(
    ctx: &DispatchContext,
    model_id: Option<&str>,
) -> Result<(Box<dyn LlmProvider>, String, RouteAttestation), String> {
    let ollama_base = crate::commands::chat_llm::authorized_ollama_base_url();
    if unit_is_local(ctx) {
        let runtime = ProvingRuntime::default();
        local_only_target(model_id, ollama_base, &runtime)?;
        let proof = runtime
            .proof
            .into_inner()
            .ok_or_else(|| "local-only: the local runtime was not verified".to_string())?;
        let attestation = attest_route(&proof);
        let model = proof.model().to_string();
        return Ok((Box::new(proof), model, attestation));
    }
    let config = nexus_kernel::config::load_config().map_err(|e| e.to_string())?;
    let provider_config = crate::build_provider_config(&config);
    let (provider, model) = match model_id {
        Some(full_model) => crate::provider_from_prefixed_model(full_model, &provider_config)?,
        None => {
            let provider = crate::select_provider(&provider_config).map_err(|e| e.to_string())?;
            let model = if config.llm.default_model.trim().is_empty() {
                "mock-1".to_string()
            } else {
                config.llm.default_model.clone()
            };
            (provider, model)
        }
    };
    let attestation = attest_route(&provider);
    Ok((provider, model, attestation))
}

/// The transport selection of [`dispatch_text`]: the transport chosen for
/// the (possibly rebuilt) context, with the `attest_route` attestation of
/// the provider it chose.
pub(crate) type SelectTransport<'a, T> =
    dyn FnMut(&DispatchContext) -> Result<(T, RouteAttestation), String> + 'a;

/// One dispatch, through the issuer, of a unit whose prompt is
/// human-supplied or constant text (AC, HD, HB, SY, XS): no inclusion and
/// no carried item. `select` chooses the transport for the (possibly
/// rebuilt) context and returns it with the `attest_route` attestation of
/// the provider it chose; `send` transmits the admitted prompt, with no lock
/// or transaction held. The reply leaves only as an accepted derived item.
pub(crate) fn dispatch_text<T>(
    state: &AppState,
    ctx: &mut DispatchContext,
    text: &str,
    select: &mut SelectTransport<'_, T>,
    send: &mut dyn FnMut(T, &str) -> Result<String, String>,
) -> Result<DerivedItem, String> {
    let issuer = issuer_of(state)?;
    let selection_error: RefCell<Option<String>> = RefCell::new(None);
    #[cfg(test)]
    let send = &mut capture::recording(send);
    let result = issuer.dispatch(
        ctx,
        &mut |c: &mut DispatchContext| match select(c) {
            Ok((transport, attestation)) => {
                let mut b = PromptBuilder::new(c);
                b.text(text);
                Some((b.build(), transport, attestation))
            }
            Err(reason) => {
                *selection_error.borrow_mut() = Some(reason);
                None
            }
        },
        send,
    );
    match result {
        Ok(reply) => Ok(reply),
        Err(DispatchError::Refused(nexus_kernel::hm::Refused::NoTransport))
            if selection_error.borrow().is_some() =>
        {
            Err(selection_error.into_inner().unwrap_or_default())
        }
        Err(DispatchError::Transport(e)) => Err(e),
        Err(e) => Err(format!("local-only: {e} (P3-HM-CONTRACT-02 §12.4)")),
    }
}

/// A provider handed to a subsystem that calls it itself (XS: RAG, builder;
/// P3-HM-CONTRACT-02 §12.1, §12.8; A2.4 I1). Every query and embedding it
/// makes is one dispatch, through the issuer, of a fresh
/// `System(component)` request (Plain class, no inclusion), on the wrapped
/// provider, attested by `attest_route` on it.
pub(crate) struct IssuedProvider {
    state: AppState,
    component: &'static str,
    inner: Box<dyn LlmProvider>,
}

impl IssuedProvider {
    pub(crate) fn new(
        state: &AppState,
        component: &'static str,
        inner: Box<dyn LlmProvider>,
    ) -> Self {
        Self {
            state: state.clone(),
            component,
            inner,
        }
    }

    /// One dispatch of `text` under a fresh request unit; `send` returns the
    /// reply text and keeps whatever else the caller needs.
    fn issued(
        &self,
        text: &str,
        send: &mut dyn FnMut(&dyn LlmProvider, &str) -> Result<String, String>,
    ) -> Result<String, AgentError> {
        let fail = |e: String| AgentError::SupervisorError(format!("{}: {e}", self.component));
        let mut ctx = request_unit(&self.state, Principal::System(self.component)).map_err(fail)?;
        let inner = self.inner.as_ref();
        dispatch_text(
            &self.state,
            &mut ctx,
            text,
            &mut |_| Ok(((), attest_route(&self.inner))),
            &mut |(), admitted| send(inner, admitted),
        )
        .map(|reply| reply.text().to_string())
        .map_err(fail)
    }
}

impl LlmProvider for IssuedProvider {
    fn query(&self, prompt: &str, max_tokens: u32, model: &str) -> Result<LlmResponse, AgentError> {
        let response: RefCell<Option<LlmResponse>> = RefCell::new(None);
        let text = self.issued(prompt, &mut |inner, admitted| {
            let reply = inner
                .query(admitted, max_tokens, model)
                .map_err(|e| e.to_string())?;
            let text = reply.output_text.clone();
            *response.borrow_mut() = Some(reply);
            Ok(text)
        })?;
        let mut response = response
            .into_inner()
            .ok_or_else(|| AgentError::SupervisorError("no reply".to_string()))?;
        response.output_text = text;
        Ok(response)
    }

    fn name(&self) -> &str {
        self.inner.name()
    }

    fn cost_per_token(&self) -> f64 {
        self.inner.cost_per_token()
    }

    fn is_paid(&self) -> bool {
        self.inner.is_paid()
    }

    fn estimate_input_tokens(&self, prompt: &str) -> u32 {
        self.inner.estimate_input_tokens(prompt)
    }

    fn endpoint_url(&self) -> String {
        self.inner.endpoint_url()
    }

    /// The texts are the admitted prompt (as a JSON array, sent as given);
    /// the embeddings come back through the accepted reply.
    fn embed(&self, texts: &[&str], model: &str) -> Result<EmbeddingResponse, AgentError> {
        let prompt =
            serde_json::to_string(texts).map_err(|e| AgentError::SupervisorError(e.to_string()))?;
        let reply = self.issued(&prompt, &mut |inner, admitted| {
            let texts: Vec<String> = serde_json::from_str(admitted).map_err(|e| e.to_string())?;
            let refs: Vec<&str> = texts.iter().map(String::as_str).collect();
            let response = inner.embed(&refs, model).map_err(|e| e.to_string())?;
            serde_json::to_string(&response).map_err(|e| e.to_string())
        })?;
        serde_json::from_str(&reply).map_err(|e| AgentError::SupervisorError(e.to_string()))
    }
}

/// The desktop's test-only capture seam (mission P3-C11-HM-M4 N4, AD-6;
/// `cfg(test)` only): every request a desktop adapter hands to a transport,
/// with the class and admission id the issuer marks in progress.
#[cfg(test)]
pub(crate) mod capture {
    use std::sync::Mutex;

    /// One outbound request.
    #[derive(Debug, Clone)]
    pub(crate) struct Captured {
        pub(crate) class: String,
        pub(crate) admission_id: Option<String>,
        pub(crate) prompt: String,
    }

    static CAPTURED: Mutex<Vec<Captured>> = Mutex::new(Vec::new());

    /// Records one request at a transport boundary.
    pub(crate) fn record(prompt: &str) {
        let transmission = nexus_kernel::hm::transmission_in_progress();
        CAPTURED
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .push(Captured {
                class: transmission
                    .as_ref()
                    .map_or("NONE", |t| t.class())
                    .to_string(),
                admission_id: transmission
                    .as_ref()
                    .and_then(|t| t.admission_id().map(str::to_string)),
                prompt: prompt.to_string(),
            });
    }

    /// `send`, recording each request it is handed first.
    pub(crate) fn recording<'a, T>(
        send: &'a mut dyn FnMut(T, &str) -> Result<String, String>,
    ) -> impl FnMut(T, &str) -> Result<String, String> + 'a {
        move |transport: T, admitted: &str| {
            record(admitted);
            send(transport, admitted)
        }
    }

    /// Every request captured so far in this process (and forgets them).
    pub(crate) fn take() -> Vec<Captured> {
        std::mem::take(&mut *CAPTURED.lock().unwrap_or_else(|p| p.into_inner()))
    }
}

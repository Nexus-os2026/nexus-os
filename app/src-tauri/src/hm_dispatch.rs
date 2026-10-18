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

use std::cell::{Cell, RefCell};
use std::sync::Arc;

use nexus_connectors_llm::attest::{attest_route, verify_local_ollama, VerifiedLocalOllama};
use nexus_connectors_llm::providers::{FlashProvider, LlmProvider};
use nexus_kernel::errors::AgentError;
use nexus_kernel::hm::{
    DerivedItem, DispatchContext, DispatchError, Principal, PromptSource, RouteAttestation,
    RouteClass,
};

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

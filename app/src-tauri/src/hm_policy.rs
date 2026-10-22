//! The desktop's governed route-policy changes (P3-HM-CONTRACT-02 §4.3–§4.6,
//! §15.3, §16; mission P3-C11-HM-M4 N2).
//!
//! The production writers of an `agent_llm_assignments` `local_only` value
//! the webview reaches (`set_agent_llm_provider` and the interface's
//! whole-configuration save) decide nothing themselves:
//!
//! - `local_only = true` is a governed tighten of every registered agent the
//!   assignment key matches (§4.2 step 3 keys), committed in the store
//!   first; then the config mirror is written.
//! - `local_only = false` is a loosening request for exactly the one
//!   registered agent the key resolves to (none or several: refused and
//!   reported), shown to the Owner by the kernel through a native
//!   confirmation; then, §4.5, the config mirror (`false` on every key that
//!   matches the agent), then the store commit. Declined, refused or failed:
//!   the config keeps `local_only = true`.
//!
//! A failed config mirror write is reported and never changes the store
//! decision. The webview argument alone loosens nothing.

use nexus_kernel::config::{AgentLlmAssignment, NexusConfig};
use nexus_kernel::hm::RoutePolicyConfirmer;
use uuid::Uuid;

use crate::hm_wiring::{agent_config_keys, DesktopAgentRegistry};
use crate::AppState;

/// The config file the governed changes write their mirror into.
pub(crate) trait ConfigFile {
    fn load(&self) -> Result<NexusConfig, String>;
    fn save(&self, config: &NexusConfig) -> Result<(), String>;
}

/// The operator's config file (`nexus_kernel::config`).
pub(crate) struct LiveConfigFile;

impl ConfigFile for LiveConfigFile {
    fn load(&self) -> Result<NexusConfig, String> {
        nexus_kernel::config::load_config().map_err(|e| e.to_string())
    }

    fn save(&self, config: &NexusConfig) -> Result<(), String> {
        nexus_kernel::config::save_config(config).map_err(|e| e.to_string())
    }
}

/// Text for the native message dialog. On Linux the dialog plugin's GTK
/// backend passes the message to `gtk_message_dialog_format_secondary_text`
/// as a printf format string, so every `%` (which an agent's registered
/// name may contain) is doubled to print literally, as the coding flow's
/// native dialogs do.
pub(crate) fn dialog_text(message: &str) -> String {
    message.replace('%', "%%")
}

/// The Owner's native confirmation of a route-policy loosening
/// (P3-HM-CONTRACT-02 §2.10, §4.4): the desktop's native message dialog
/// (`tauri_plugin_dialog`, the pattern of the coding flow's native dialogs
/// and of governed control's confirmations), rendered and invoked by the
/// backend; the webview holds no dialog permission. The kernel builds the
/// request. It is never shown on the GUI event loop thread (a blocking
/// dialog there would wait for itself): asked there, it answers no.
#[cfg(all(target_os = "linux", feature = "tauri-runtime"))]
pub(crate) struct NativeRoutePolicyDialog(pub(crate) tauri::AppHandle<tauri::Wry>);

#[cfg(all(target_os = "linux", feature = "tauri-runtime"))]
impl RoutePolicyConfirmer for NativeRoutePolicyDialog {
    fn confirm_loosening(&self, request: &nexus_kernel::hm::LooseningRequest) -> bool {
        use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};
        if std::thread::current().name() == Some("main") {
            return false;
        }
        self.0
            .dialog()
            .message(dialog_text(&request.message()))
            .title(request.title())
            .kind(MessageDialogKind::Warning)
            .buttons(MessageDialogButtons::OkCancelCustom(
                "Allow non-local providers".to_string(),
                "Cancel".to_string(),
            ))
            .blocking_show()
    }
}

/// The supervisor- and meta-registered agents of `state` (§2.5).
fn registry(state: &AppState) -> DesktopAgentRegistry {
    DesktopAgentRegistry::new(state.supervisor.clone(), state.meta.clone())
}

/// Every registered agent an assignment `key` matches: the agents whose
/// §4.2 step 3 keys (id, normalized id, each registered name normalized and
/// as given) contain it.
pub(crate) fn agents_matching_key(state: &AppState, key: &str) -> Vec<Uuid> {
    use nexus_kernel::hm::AgentRegistry;
    let registry = registry(state);
    registry
        .registered()
        .into_iter()
        .filter(|id| {
            agent_config_keys(&id.to_string(), &registry.names(id))
                .iter()
                .any(|k| k == key)
        })
        .collect()
}

/// A requested `local_only` value as decided.
#[derive(Debug, Default)]
pub(crate) struct Governed {
    /// The `local_only` value the config may hold for the key: `false` only
    /// after a confirmed loosening, or when the one agent is already
    /// unrestricted.
    pub(crate) local_only: bool,
    /// What did not happen, for the caller to report; empty when everything
    /// asked for happened.
    pub(crate) reported: Vec<String>,
    /// Whether the config mirror was already written (a confirmed
    /// loosening writes it before its store commit, §4.5).
    pub(crate) mirrored: bool,
}

/// `local_only = true` under `key`: the governed tighten of every registered
/// agent the key matches, in the store first. A tighten that cannot commit
/// is reported (the agent is held local-only for the life of the process).
pub(crate) fn govern_tighten(state: &AppState, key: &str) -> Governed {
    let mut governed = Governed {
        local_only: true,
        ..Governed::default()
    };
    let issuer = match state.hm_issuer() {
        Ok(issuer) => issuer,
        Err(e) => {
            governed
                .reported
                .push(format!("{key}: the tightening was not committed: {e}"));
            return governed;
        }
    };
    for agent in agents_matching_key(state, key) {
        if let Err(e) = issuer.governed_tighten(agent) {
            governed.reported.push(format!("{key}: {e}"));
        }
    }
    governed
}

/// `local_only = false` under `key`: a loosening request for exactly the
/// one registered agent the key resolves to, through the kernel's native
/// confirmation. On confirmation `mirror_entry` is applied to the config
/// with `local_only = false` on every key that matches the agent, written
/// (§4.5 step 1), then the store commits (step 2).
pub(crate) fn govern_loosen(
    state: &AppState,
    confirmer: Option<&dyn RoutePolicyConfirmer>,
    file: &dyn ConfigFile,
    key: &str,
    mirror_entry: &dyn Fn(&mut NexusConfig),
) -> Governed {
    let refused = |reason: String| Governed {
        local_only: true,
        reported: vec![format!("{key}: loosening refused: {reason}")],
        mirrored: false,
    };
    let agents = agents_matching_key(state, key);
    let agent = match agents.as_slice() {
        [agent] => *agent,
        [] => return refused("the key names no registered agent".to_string()),
        _ => return refused(format!("the key names {} registered agents", agents.len())),
    };
    let issuer = match state.hm_issuer() {
        Ok(issuer) => issuer,
        Err(e) => return refused(e),
    };
    let mirror_keys = {
        use nexus_kernel::hm::AgentRegistry;
        agent_config_keys(&agent.to_string(), &registry(state).names(&agent))
    };
    let mut mirror = || {
        let mut config = file.load()?;
        for k in &mirror_keys {
            if let Some(assignment) = config.agent_llm_assignments.get_mut(k) {
                assignment.local_only = false;
            }
        }
        mirror_entry(&mut config);
        file.save(&config)
    };
    match issuer.request_loosening(agent, confirmer, &mut mirror) {
        Ok(loosened) if loosened.changed => Governed {
            local_only: false,
            reported: match loosened.mirror {
                Ok(()) => Vec::new(),
                Err(e) => vec![format!(
                    "{key}: the agent was loosened, but its config mirror was not written: {e}"
                )],
            },
            mirrored: true,
        },
        // Already unrestricted in the store: nothing to ask or loosen.
        Ok(_) => Governed {
            local_only: false,
            reported: Vec::new(),
            mirrored: false,
        },
        Err(reason) => refused(reason.to_string()),
    }
}

/// `set_agent_llm_provider` (§4.4): the provider and budgets are saved as
/// before; `local_only` is governed. Every refusal or failure is reported
/// as an error after what could be saved was saved.
#[allow(clippy::too_many_arguments)]
pub(crate) fn set_agent_llm_provider(
    state: &AppState,
    confirmer: Option<&dyn RoutePolicyConfirmer>,
    file: &dyn ConfigFile,
    key: String,
    provider_id: String,
    local_only: bool,
    budget_dollars: u32,
    budget_tokens: u64,
) -> Result<(), String> {
    let entry = |local_only: bool| AgentLlmAssignment {
        provider_id: provider_id.clone(),
        local_only,
        budget_dollars,
        budget_tokens,
    };
    let governed = if local_only {
        govern_tighten(state, &key)
    } else {
        govern_loosen(state, confirmer, file, &key, &|config: &mut NexusConfig| {
            config
                .agent_llm_assignments
                .insert(key.clone(), entry(false));
        })
    };
    let mut reported = governed.reported;
    if !governed.mirrored {
        let written = file.load().and_then(|mut config| {
            config
                .agent_llm_assignments
                .insert(key.clone(), entry(governed.local_only));
            file.save(&config)
        });
        if let Err(e) = written {
            reported.push(format!("{key}: the config mirror was not written: {e}"));
        }
    }
    if reported.is_empty() {
        Ok(())
    } else {
        Err(format!("set_agent_llm_provider: {}", reported.join("; ")))
    }
}

/// The interface's whole-configuration save (`save_config`), governed: every
/// assignment key whose `local_only` the save changes is decided first (a
/// tighten in the store first; a loosening only through the native
/// confirmation, and kept `true` otherwise); then the save itself runs
/// unchanged (its security and credential checks included). A save its
/// security check will deny is passed through without deciding anything.
pub(crate) fn save_config(
    state: &AppState,
    confirmer: Option<&dyn RoutePolicyConfirmer>,
    file: &dyn ConfigFile,
    mut config: NexusConfig,
) -> Result<(), String> {
    let baseline = nexus_kernel::config::load_current_security_baseline();
    let mut reported = Vec::new();
    if baseline.as_ref().ok() == Some(&config.security) {
        let current = file
            .load()
            .map(|c| c.agent_llm_assignments)
            .unwrap_or_default();
        let mut keys: Vec<String> = current
            .keys()
            .chain(config.agent_llm_assignments.keys())
            .cloned()
            .collect();
        keys.sort();
        keys.dedup();
        for key in keys {
            let was = current.get(&key).is_some_and(|a| a.local_only);
            let asked = config
                .agent_llm_assignments
                .get(&key)
                .is_some_and(|a| a.local_only);
            if asked && !was {
                reported.extend(govern_tighten(state, &key).reported);
            } else if was && !asked {
                let governed = govern_loosen(state, confirmer, file, &key, &|_| {});
                reported.extend(governed.reported);
                if governed.local_only {
                    // Not loosened: the key stays local-only (re-inserted
                    // when the save dropped it).
                    let kept = current.get(&key).cloned().expect("was local-only");
                    config
                        .agent_llm_assignments
                        .entry(key)
                        .or_insert(kept)
                        .local_only = true;
                }
            }
        }
    }
    crate::save_config(state, config)?;
    if reported.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "save_config: the configuration was saved; {}",
            reported.join("; ")
        ))
    }
}

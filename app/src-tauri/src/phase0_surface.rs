//! P0-002C5A: Phase Zero reachability closure.
//!
//! A desktop surface closed here had no approved backend-owned authority for
//! what it did: a raw frontend or model path, the process working directory,
//! a caller-asserted approval, arbitrary program text, or an ambient resource
//! location. Rather than inventing that authority, the surface fails closed.
//! An unavailable safe feature is preferred to a working unsafe one.
//!
//! Nothing here grants authority; it only denies. A closed IPC command stays
//! registered so the frontend receives one bounded, human-readable reason
//! instead of an unknown-command error. Its handler takes no arguments and its
//! whole body is the denial, so no caller input is read, echoed or acted on.
//!
//! Re-opening a surface requires an Architect-approved authority mechanism.
//! The reachability guard in `phase0_surface/tests.rs` fails when a closed
//! command, or a closed non-IPC path, becomes reachable again without that
//! classification; `docs/security/phase0-c5-authority-inventory.md` explains
//! how to update both.

/// Why a surface is unavailable in Phase Zero.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Closure {
    /// E1: a raw user-selected path is not authority, and no backend-owned
    /// selection mechanism exists in Phase Zero.
    FileSelection,
    /// E2: the retired raw-path Builder surface. The governed Builder project
    /// flow (C1-C4) is the approved authority path.
    LegacyBuilder,
    /// E3: arbitrary program or command text is not process authority.
    ProcessExecution,
    /// E3: a caller's assertion is not proof of user approval.
    ApprovalRequired,
    /// E3: external CLI agents run outside every Nexus authority boundary.
    ExternalCliAgent,
    /// E4: agent actions holding filesystem or process authority (the process
    /// working directory is not an agent workspace).
    AgentExecution,
    /// E5: a code- or authority-bearing resource located through the process
    /// working directory, the environment or a developer checkout path, for
    /// which no deterministic Nexus-owned installed location exists.
    AmbientResource,
    /// C5C: keyboard and mouse input chosen by the interface or a model. No
    /// approved mechanism authorizes OS input in Phase Zero, and the agent
    /// executor already refuses screen and input actions.
    OsInput,
    /// C5C (Architect decision): screen capture, capture plus analysis, or
    /// enabling screen observation, requested over desktop IPC. No brokered
    /// mechanism authorizes observing the screen in Phase Zero, and an IPC
    /// request is not proof of the user's consent.
    ScreenObservation,
    // Final Gate items B, C, F and I: egress, credential transport, peers and
    // helper programs.
    /// Final Gate item B: a network destination chosen by the interface or a
    /// model. A syntactically valid URL, host name or `host:port` is not an
    /// egress grant, and no backend-owned destination policy authorizes such
    /// a route in Phase Zero.
    NetworkDestination,
    /// Final Gate item F: a model transfer to a Nexus Link peer. A peer
    /// address is not authority, and Phase Zero has no pairing or peer
    /// authentication.
    PeerTransfer,
    /// Final Gate item C: a request whose credential would be placed on a
    /// process command line, where other local processes can read it, and
    /// for which no in-process transport is approved in Phase Zero.
    CredentialTransport,
    /// Final Gate item I: starting a helper service, or running a helper
    /// program found on `PATH` only to report on it. Nexus holds no approved
    /// executable authority or lifecycle ownership for such a program in
    /// Phase Zero; it connects only to a service started outside it.
    HelperLaunch,
    // Final Gate items A and H: stored secrets.
    /// Final Gate items A and H: the surface's only effect is to store a
    /// credential or token, and no approved secret store exists for it in
    /// Phase Zero. Stored values stay readable; nothing new is stored.
    SecretStorage,
    // Phase One charter §12: residual closed before coding authority ships.
    /// Time Machine "what if": a caller-chosen variable key and value set an
    /// agent's fuel, forced an agent's state across illegal transitions, or
    /// toggled Warden review in the saved configuration. A string is not
    /// that authority, and no governed simulation mechanism exists.
    SimulationReplay,
    // P2-ENTRY-H1: residual closed before Phase Two authority work.
    /// Time Machine undo, redo and undo-to-checkpoint: replaying a recorded
    /// checkpoint restored an agent's fuel and memories, set its state
    /// (forced across illegal lifecycle transitions when the lifecycle
    /// refused), or toggled Warden review, with no owner approval. A recorded
    /// checkpoint is not that authority, and no governed replay mechanism
    /// exists.
    CheckpointReplay,
}

impl Closure {
    pub(crate) const fn reason(self) -> &'static str {
        match self {
            Self::FileSelection => {
                "governed file selection is unavailable in Phase Zero; a raw file path is not authority"
            }
            Self::LegacyBuilder => {
                "the legacy Builder project surface is retired in Phase Zero; use the governed Builder project flow"
            }
            Self::ProcessExecution => "ungoverned process execution is unavailable in Phase Zero",
            Self::ApprovalRequired => {
                "approval required: a caller-asserted approval is not user approval, and backend-verified approval is unavailable in Phase Zero"
            }
            Self::ExternalCliAgent => "external CLI agent providers are unavailable in Phase Zero",
            Self::AgentExecution => {
                "agent filesystem and process actions are unavailable in Phase Zero"
            }
            Self::AmbientResource => {
                "this feature depends on an ambient resource location and is unavailable in Phase Zero"
            }
            Self::OsInput => {
                "keyboard and mouse input from the interface or a model is unavailable in Phase Zero"
            }
            Self::ScreenObservation => "governed screen observation is unavailable in Phase Zero",
            // Final Gate items B, C, F and I.
            Self::NetworkDestination => {
                "a network destination chosen by the interface or a model is not egress authority in Phase Zero"
            }
            Self::PeerTransfer => {
                "peer model transfer is unavailable in Phase Zero: a peer address is not authority, and no peer is paired or authenticated"
            }
            Self::CredentialTransport => {
                "this request would place a credential on a process command line and is unavailable in Phase Zero"
            }
            Self::HelperLaunch => {
                "starting or running a helper program is unavailable in Phase Zero; Nexus only connects to a service started outside it"
            }

            // Final Gate items A and H.
            Self::SecretStorage => {
                "storing a credential or token outside an approved secret store is unavailable in Phase Zero"
            }

            // Phase One charter §12.
            Self::SimulationReplay => {
                "time-machine what-if is unavailable: a caller-chosen key and value is not authority over agent fuel, agent state or Warden review"
            }

            // P2-ENTRY-H1.
            Self::CheckpointReplay => {
                "time-machine undo and redo are unavailable: a recorded checkpoint is not authority over agent state, fuel, memories or configuration"
            }
        }
    }
}

/// The bounded error for a closed surface: `"<surface>: <reason>"`.
pub(crate) fn closed(surface: &'static str, closure: Closure) -> String {
    format!("{surface}: {}", closure.reason())
}

#[cfg(test)]
mod tests;

// P0-FINAL-GATE-CLOSURE guards, one module per workstream.
#[cfg(test)]
mod fg_approval;
#[cfg(test)]
mod fg_egress;
#[cfg(test)]
mod fg_reliability;
#[cfg(test)]
mod fg_secrets;
#[cfg(test)]
mod fg_standalone;
#[cfg(test)]
mod fg_webview;

// XA-R4-FINAL: the structural Rust path resolver the guards share. Like the
// guards it lives in a file named `tests.rs`, so the production-source
// scanners never read its tables and fixtures as production code.
#[cfg(test)]
#[path = "phase0_surface/rust_paths/tests.rs"]
pub(crate) mod rust_paths;

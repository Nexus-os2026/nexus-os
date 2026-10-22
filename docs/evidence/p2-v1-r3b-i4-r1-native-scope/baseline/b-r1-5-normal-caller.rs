//! B-R1-5 (and B-R1-2's API half): a normal external caller of the sandbox
//! crate at bb0dfec0, built with no feature of that crate enabled.
use nexus_verifier_sandbox::launcher::Helper;
use nexus_verifier_sandbox::policy::ResourcePolicy;
use nexus_verifier_sandbox::scope::{PendingScope, ScopeManager, StartFailed};

/// The direct start, whose failure splits the operation from its helper.
fn split(scopes: &ScopeManager, helper: Helper) {
    match scopes.start(&helper, &ResourcePolicy::RUST_OFFLINE_V1) {
        Ok(_scope) => {}
        Err(StartFailed { error: _, unresolved }) => {
            let pending: Option<Box<PendingScope>> = unresolved;
            drop(pending);
            let _ = helper.reap();
        }
    }
}

fn main() {
    // A caller-selected socket as the verifier's manager.
    let selected = ScopeManager::connect_at("/run/user/1000/a-socket-the-caller-chose");
    let _ = (selected.is_ok(), split as fn(&ScopeManager, Helper));
}

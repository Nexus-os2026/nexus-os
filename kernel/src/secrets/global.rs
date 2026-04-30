//! Bug AK Commit 2 — process-singleton facade installer.
//!
//! `kernel::startup::run_migrations` constructs a `SecretsFacade`,
//! runs the one-shot config-to-vault migration, and then
//! `install`s the facade here. Production consumers
//! (connectors/web, agents/social-poster, the upcoming Tauri
//! `vault_*` commands) read it via `facade()`.
//!
//! Tests construct facades locally and pass them through normal
//! function arguments — the global is a production wiring artifact
//! and exists ONLY so that consumers deep in call graphs that
//! cannot accept a facade reference (e.g., the herald adapter
//! described in OQ-2B) can still reach the singleton without
//! re-engineering their entire signature.
//!
//! Initialization is one-shot and panics on double-`install`.
//! Reading via `facade()` panics with a clear message if the
//! singleton has not been installed; consumers in startup-sensitive
//! contexts should use `try_facade()`.

use super::SecretsFacade;
use std::sync::{Arc, OnceLock};

static FACADE: OnceLock<Arc<SecretsFacade>> = OnceLock::new();

/// Install the process facade. Panics if already installed —
/// the singleton is intentionally one-shot to make double-init
/// a hard fail-loud rather than a silent data race.
pub fn install(facade: Arc<SecretsFacade>) {
    if FACADE.set(facade).is_err() {
        panic!(
            "kernel::secrets::global::install called twice; \
             SecretsFacade is a one-shot process singleton"
        );
    }
}

/// Return the installed facade if any. Returns `None` until
/// `install` has run — useful in startup paths where the caller
/// needs to gracefully degrade rather than panic.
pub fn try_facade() -> Option<Arc<SecretsFacade>> {
    FACADE.get().cloned()
}

/// Return the installed facade. Panics with a clear message if
/// the singleton has not been installed. Use this from
/// production paths that run AFTER `kernel::startup::run_migrations`
/// has completed.
pub fn facade() -> Arc<SecretsFacade> {
    FACADE.get().cloned().expect(
        "kernel::secrets::global::FACADE not installed; \
             call kernel::startup::run_migrations() during \
             process startup before reaching this code path",
    )
}

//! Governed connector framework for external integrations in NEXUS OS.

pub mod challenge;
pub mod connector;
pub mod github_connector;
pub mod http_connector;
pub mod idempotency;
pub mod rate_limit;
pub mod registry;
pub mod validation;
// Bug AK Commit 5: the connector-local secrets module is
// removed. Auth-secret resolution is owned by the kernel
// SecretsFacade (kernel/src/secrets/backend_*.rs).
// http_connector reads via
// `kernel::secrets::global::try_facade()` under scope "http".
// See ADR 0004 Implementation phasing.

//! Enterprise integration plugins for Nexus OS.
//!
//! ```text
//! Agent Action → IntegrationRouter → Provider Plugin → External API
//!                                   ↓
//!                           Audit Trail Entry
//! ```
//!
//! All integrations are:
//! - Capability-gated (agent must have `integration.<provider>` capability)
//! - Audited (every external call is logged via hash-chain AuditTrail)
//! - Rate-limited (per-provider token-bucket limits)
//! - PII-redacted (sensitive data stripped before sending)

pub mod config;
pub mod error;
pub mod events;
pub mod providers;
pub mod router;

pub use config::{IntegrationConfig, ProviderConfig};
pub use error::IntegrationError;
pub use events::{NexusEvent, Notification, StatusUpdate, TicketRequest, TicketResponse};
pub use router::IntegrationRouter;

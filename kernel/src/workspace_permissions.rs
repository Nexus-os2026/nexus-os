//! The filesystem permission level used by workspace grants and coding runs.
//!
//! Extracted unchanged from `manifest` (K02) so the retained workspace
//! authority and coding run no longer depend on the legacy agent manifest. The
//! enum keeps its derives, variant order and serde names exactly; the legacy
//! glob-pattern permission model stays in `manifest` until that module retires.

use serde::{Deserialize, Serialize};

/// Permission level for a filesystem path scope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FsPermissionLevel {
    ReadOnly,
    ReadWrite,
    Deny,
}

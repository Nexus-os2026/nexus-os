//! Phase 1.5 Group B — live AT-SPI enumerator.
//!
//! External observer that queries the accessibility tree of a running
//! Nexus OS process via AT-SPI over D-Bus. This module does **not**
//! modify the app under test; it is a peer process that walks the
//! a11y tree the same way Playwright, axe-core, and screen readers do.
//!
//! Element IDs are composed as `"role:accessible_name"` (e.g.
//! `"button:New Compare"`) so downstream consumers reading React
//! source files can locate the matching element by label instead of
//! by CSS class.

use std::collections::VecDeque;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::specialists::element::{BoundingBox, InteractiveElement};

/// Configuration for a live AT-SPI enumeration run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiveTargetConfig {
    /// The accessible application name to target, e.g. `"nexus-os"`
    /// or `"Nexus OS"`. Matching is case-insensitive `contains`.
    pub app_name: String,
    /// The page route currently loaded in the webview, e.g. `"/chat"`.
    /// Metadata only — the enumerator does not navigate.
    pub page_route: String,
    /// Optional sub-tab name, e.g. `"Compare"`. Metadata only.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tab: Option<String>,
}

/// Errors returned by [`enumerate_live`].
#[derive(Debug)]
pub enum LiveEnumeratorError {
    /// Failed to open the AT-SPI bus or construct the registry proxy.
    AtSpiConnectionFailed(String),
    /// The AT-SPI registry did not contain any application whose
    /// accessible name matched `app_name`.
    AppNotFound { app_name: String },
    /// Any other failure during the tree walk, including the 10-second
    /// timeout.
    EnumerationFailed(String),
}

impl std::fmt::Display for LiveEnumeratorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AtSpiConnectionFailed(msg) => {
                write!(f, "AT-SPI connection failed: {msg}")
            }
            Self::AppNotFound { app_name } => {
                write!(f, "no AT-SPI application found matching name {app_name:?}")
            }
            Self::EnumerationFailed(msg) => {
                write!(f, "AT-SPI enumeration failed: {msg}")
            }
        }
    }
}

impl std::error::Error for LiveEnumeratorError {}

/// Result of a live enumeration run.
#[derive(Debug, Clone)]
pub struct LiveEnumerationResult {
    pub elements: Vec<InteractiveElement>,
    pub app_name: String,
    pub page_route: String,
    pub tab: Option<String>,
    /// Number of AT-SPI nodes visited during the walk (diagnostics).
    pub nodes_visited: usize,
}

/// Hard ceiling on a single `enumerate_live` invocation. If the walk
/// exceeds this, we abort and return [`LiveEnumeratorError::EnumerationFailed`].
const WALK_TIMEOUT: Duration = Duration::from_secs(10);

/// Maximum number of AT-SPI nodes to visit in a single walk. Prevents
/// runaway traversal on a pathologically large tree.
const MAX_NODES: usize = 10_000;

/// Interactive AT-SPI role names we collect. Matched case-insensitively
/// against the value returned by `AccessibleProxy::get_role_name()`.
const INTERACTIVE_ROLES: &[&str] = &[
    "button",
    "push button",
    "toggle button",
    "radio button",
    "check box",
    "link",
    "text",
    "entry",
    "combo box",
    "menu item",
];

/// Connects to the AT-SPI accessibility tree, locates the application
/// whose accessible name contains `config.app_name` (case-insensitive),
/// walks its tree, and returns every interactive element found.
///
/// The walk is bounded by a 10-second wall-clock timeout and a
/// 10 000-node visit budget.
///
/// This function does **not** navigate the application. It reads
/// whatever is currently visible in the accessibility tree.
pub async fn enumerate_live(
    config: &LiveTargetConfig,
) -> Result<LiveEnumerationResult, LiveEnumeratorError> {
    let config = config.clone();
    let fut = async move { walk(config).await };

    match tokio::time::timeout(WALK_TIMEOUT, fut).await {
        Ok(res) => res,
        Err(_) => Err(LiveEnumeratorError::EnumerationFailed(
            "AT-SPI walk timed out after 10s".to_string(),
        )),
    }
}

async fn walk(config: LiveTargetConfig) -> Result<LiveEnumerationResult, LiveEnumeratorError> {
    use atspi::proxy::accessible::{AccessibleProxy, ObjectRefExt};
    use atspi::AccessibilityConnection;
    use atspi::{ObjectRef, State};

    let conn = AccessibilityConnection::new()
        .await
        .map_err(|e| LiveEnumeratorError::AtSpiConnectionFailed(e.to_string()))?;
    let zconn = conn.connection().clone();

    // Build an AccessibleProxy pointed at the AT-SPI registry root.
    // Children of this accessible are the top-level application roots.
    let registry_root = AccessibleProxy::builder(&zconn)
        .destination("org.a11y.atspi.Registry")
        .and_then(|b| b.path("/org/a11y/atspi/accessible/root"))
        .map_err(|e| LiveEnumeratorError::AtSpiConnectionFailed(e.to_string()))?
        .cache_properties(zbus::proxy::CacheProperties::No)
        .build()
        .await
        .map_err(|e| LiveEnumeratorError::AtSpiConnectionFailed(e.to_string()))?;

    let registry_children: Vec<ObjectRef> = registry_root
        .get_children()
        .await
        .map_err(|e| LiveEnumeratorError::EnumerationFailed(e.to_string()))?;

    // Find the application whose accessible name contains `app_name`
    // (case-insensitive).
    let wanted = config.app_name.to_lowercase();
    let mut app_root: Option<ObjectRef> = None;
    for child in registry_children {
        let proxy: AccessibleProxy<'_> = match child.clone().into_accessible_proxy(&zconn).await {
            Ok(p) => p,
            Err(_) => continue,
        };
        let name = proxy.name().await.unwrap_or_default();
        if name.to_lowercase().contains(&wanted) {
            app_root = Some(child);
            break;
        }
    }

    let app_root = app_root.ok_or_else(|| LiveEnumeratorError::AppNotFound {
        app_name: config.app_name.clone(),
    })?;

    let mut elements: Vec<InteractiveElement> = Vec::new();
    let mut nodes_visited: usize = 0;
    let mut fallback_counter: usize = 0;

    // Iterative BFS. Async recursion without boxing is awkward; a queue
    // keeps the logic flat and bounds stack usage.
    let mut queue: VecDeque<ObjectRef> = VecDeque::new();
    queue.push_back(app_root);

    while let Some(obj) = queue.pop_front() {
        if nodes_visited >= MAX_NODES {
            break;
        }
        nodes_visited += 1;

        let proxy: AccessibleProxy<'_> = match obj.clone().into_accessible_proxy(&zconn).await {
            Ok(p) => p,
            Err(_) => continue,
        };

        // Enqueue children before we bail out of this particular node.
        if let Ok(children) = proxy.get_children().await {
            for c in children {
                queue.push_back(c);
            }
        }

        // Fetch role. If it fails, skip but keep walking.
        let role_name = match proxy.get_role_name().await {
            Ok(r) => r.to_lowercase(),
            Err(_) => continue,
        };
        if !INTERACTIVE_ROLES.contains(&role_name.as_str()) {
            continue;
        }

        let accessible_name = proxy.name().await.unwrap_or_default().trim().to_string();
        let description = proxy
            .description()
            .await
            .unwrap_or_default()
            .trim()
            .to_string();

        let element_id = if !accessible_name.is_empty() {
            format!("{role_name}:{accessible_name}")
        } else if !description.is_empty() {
            format!("{role_name}:{description}")
        } else {
            let id = format!("{role_name}:{fallback_counter}");
            fallback_counter += 1;
            id
        };

        // Reuse the destination/path from the AccessibleProxy's inner
        // zbus proxy to build sibling interface proxies.
        let dest = proxy.inner().destination().to_owned();
        let path = proxy.inner().path().to_owned();

        // Component interface for bounding box. Built lazily — many
        // accessibles do not implement Component.
        let bounding_box = build_bbox(&zconn, dest.clone(), path.clone()).await;

        let (is_enabled, is_visible) = match proxy.get_state().await {
            Ok(set) => (set.contains(State::Enabled), set.contains(State::Visible)),
            Err(_) => (false, false),
        };

        let text_content = build_text(&zconn, dest, path).await;

        elements.push(InteractiveElement {
            element_id,
            role: role_name,
            accessible_name,
            description,
            bounding_box,
            is_enabled,
            is_visible,
            text_content,
        });
    }

    Ok(LiveEnumerationResult {
        elements,
        app_name: config.app_name,
        page_route: config.page_route,
        tab: config.tab,
        nodes_visited,
    })
}

async fn build_bbox(
    zconn: &zbus::Connection,
    dest: zbus::names::BusName<'static>,
    path: zbus::zvariant::ObjectPath<'static>,
) -> BoundingBox {
    use atspi::proxy::component::ComponentProxy;
    use atspi::CoordType;

    let zero = BoundingBox {
        x: 0,
        y: 0,
        width: 0,
        height: 0,
    };
    let builder = match ComponentProxy::builder(zconn).destination(dest) {
        Ok(b) => b,
        Err(_) => return zero,
    };
    let builder = match builder.path(path) {
        Ok(b) => b,
        Err(_) => return zero,
    };
    let comp = match builder
        .cache_properties(zbus::proxy::CacheProperties::No)
        .build()
        .await
    {
        Ok(c) => c,
        Err(_) => return zero,
    };
    match comp.get_extents(CoordType::Screen).await {
        Ok((x, y, width, height)) => BoundingBox {
            x,
            y,
            width,
            height,
        },
        Err(_) => zero,
    }
}

async fn build_text(
    zconn: &zbus::Connection,
    dest: zbus::names::BusName<'static>,
    path: zbus::zvariant::ObjectPath<'static>,
) -> String {
    use atspi::proxy::text::TextProxy;

    let builder = match TextProxy::builder(zconn).destination(dest) {
        Ok(b) => b,
        Err(_) => return String::new(),
    };
    let builder = match builder.path(path) {
        Ok(b) => b,
        Err(_) => return String::new(),
    };
    let tp = match builder
        .cache_properties(zbus::proxy::CacheProperties::No)
        .build()
        .await
    {
        Ok(t) => t,
        Err(_) => return String::new(),
    };
    let count = match tp.character_count().await {
        Ok(c) if c > 0 => c,
        _ => return String::new(),
    };
    let end = count.min(200);
    let s = tp.get_text(0, end).await.unwrap_or_default();
    if s.chars().count() > 200 {
        s.chars().take(200).collect()
    } else {
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_live_target_config_fields() {
        let cfg = LiveTargetConfig {
            app_name: "nexus-os".to_string(),
            page_route: "/chat".to_string(),
            tab: Some("Compare".to_string()),
        };
        assert_eq!(cfg.app_name, "nexus-os");
        assert_eq!(cfg.page_route, "/chat");
        assert_eq!(cfg.tab.as_deref(), Some("Compare"));
    }

    #[test]
    fn test_live_enumerator_error_display() {
        let e1 = LiveEnumeratorError::AtSpiConnectionFailed("boom".to_string());
        let e2 = LiveEnumeratorError::AppNotFound {
            app_name: "x".to_string(),
        };
        let e3 = LiveEnumeratorError::EnumerationFailed("nope".to_string());
        assert!(!e1.to_string().is_empty());
        assert!(!e2.to_string().is_empty());
        assert!(!e3.to_string().is_empty());
    }

    #[test]
    fn test_app_not_found_error() {
        let e = LiveEnumeratorError::AppNotFound {
            app_name: "nonexistent-app-xyz".to_string(),
        };
        assert!(e.to_string().contains("nonexistent-app-xyz"));
    }

    #[tokio::test]
    async fn test_enumerate_live_returns_error_when_atspi_unavailable() {
        let cfg = LiveTargetConfig {
            app_name: "test-no-such-app-nexus-qa".to_string(),
            page_route: "/".to_string(),
            tab: None,
        };
        let result = enumerate_live(&cfg).await;
        assert!(matches!(result, Err(_)));
    }

    #[test]
    fn test_live_target_config_tab_optional() {
        let cfg = LiveTargetConfig {
            app_name: "nexus-os".to_string(),
            page_route: "/".to_string(),
            tab: None,
        };
        let json = serde_json::to_string(&cfg).unwrap();
        // `tab: None` should be skipped during serialization.
        assert!(!json.contains("\"tab\""));
        let back: LiveTargetConfig = serde_json::from_str(&json).unwrap();
        assert!(back.tab.is_none());
    }
}

use crate::autonomy::AutonomyLevel;
use crate::errors::AgentError;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

// ── Filesystem permission model (C.6 granular FS permissions) ───────────────

/// Permission level for a filesystem path scope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FsPermissionLevel {
    ReadOnly,
    ReadWrite,
    Deny,
}

/// A single path-scoped filesystem permission entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FilesystemPermission {
    /// Glob-like pattern: "/src/*.rs", "/output/", "/tmp/**", "*.rs"
    pub path_pattern: String,
    /// Permission level for paths matching this pattern.
    pub permission: FsPermissionLevel,
}

/// Normalize a path for matching: strip leading "./", ensure no trailing slash
/// (unless the path is exactly "/"). Returns an owned `String` because we may
/// need to prepend a `/` after stripping `./`.
fn normalize_path(path: &str) -> String {
    let p = path.strip_prefix("./").unwrap_or(path);
    // Ensure a leading '/' for consistent matching against absolute patterns.
    let p = if !p.starts_with('/') {
        format!("/{p}")
    } else {
        p.to_string()
    };
    if p.len() > 1 {
        p.strip_suffix('/').unwrap_or(&p).to_string()
    } else {
        p
    }
}

/// Check whether `path` matches a glob-like `pattern`.
///
/// Supported patterns:
/// - Exact: `"/src/main.rs"` matches only that path.
/// - Directory prefix: `"/src/"` (trailing slash) matches anything under `/src/`.
/// - Single wildcard: `"/src/*.rs"` matches one level (no `/` crossing).
/// - Double wildcard: `"/src/**"` matches everything recursively.
/// - Extension wildcard: `"*.rs"` matches any `.rs` file at any depth.
pub fn path_matches_pattern(path: &str, pattern: &str) -> bool {
    let norm_path = normalize_path(path);

    // Directory prefix pattern (trailing slash): "/src/" matches "/src/foo.rs"
    if pattern.ends_with('/') {
        let prefix = pattern.strip_suffix('/').unwrap_or(pattern);
        return norm_path == prefix || norm_path.starts_with(pattern);
    }

    // Double wildcard: "/src/**" → everything under /src/ recursively
    if let Some(prefix) = pattern.strip_suffix("/**") {
        return norm_path == prefix || norm_path.starts_with(&format!("{prefix}/"));
    }

    // Patterns with "**" in the middle: "prefix/**/suffix"
    if pattern.contains("**") {
        if let Some((before, after)) = pattern.split_once("**") {
            let before = if before.is_empty() {
                ""
            } else {
                before.strip_suffix('/').unwrap_or(before)
            };
            let after = if after.is_empty() {
                ""
            } else {
                after.strip_prefix('/').unwrap_or(after)
            };
            if !before.is_empty() && !norm_path.starts_with(before) {
                return false;
            }
            if !after.is_empty() && !norm_path.ends_with(after) {
                return false;
            }
            return true;
        }
    }

    // Single wildcard: "/src/*.rs" or "*.rs"
    if pattern.contains('*') {
        if let Some((prefix, suffix)) = pattern.split_once('*') {
            // Extension-only wildcard: "*.rs" matches any .rs file anywhere
            if prefix.is_empty() {
                return norm_path.ends_with(suffix);
            }
            // Scoped single wildcard: "/src/*.rs"
            // The matched segment must not contain '/'
            if !norm_path.starts_with(prefix) {
                return false;
            }
            let rest = &norm_path[prefix.len()..];
            if !rest.ends_with(suffix) {
                return false;
            }
            let middle = &rest[..rest.len() - suffix.len()];
            return !middle.contains('/');
        }
    }

    // Exact match (after normalizing path)
    norm_path == normalize_path(pattern)
}

// ── Manifest constants ──────────────────────────────────────────────────────

const MIN_NAME_LEN: usize = 3;
const MAX_NAME_LEN: usize = 64;
const MAX_FUEL_BUDGET: u64 = 1_000_000;
const CAPABILITY_REGISTRY: [&str; 23] = [
    "web.search",
    "web.read",
    "llm.query",
    "fs.read",
    "fs.write",
    "process.exec",
    "social.post",
    "social.x.post",
    "social.x.read",
    "messaging.send",
    "audit.read",
    "rag.ingest",
    "rag.query",
    "mcp.call",
    "computer.control",
    "screen.capture",
    "screen.analyze",
    "input.mouse",
    "input.keyboard",
    "input.autonomous",
    "computer.use",
    "self.modify",
    "cognitive_modify",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentManifest {
    pub name: String,
    pub version: String,
    pub capabilities: Vec<String>,
    pub fuel_budget: u64,
    pub autonomy_level: Option<u8>,
    pub consent_policy_path: Option<String>,
    pub requester_id: Option<String>,
    pub schedule: Option<String>,
    /// Default goal text assigned on each scheduled cron tick.
    #[serde(default)]
    pub default_goal: Option<String>,
    pub llm_model: Option<String>,
    pub fuel_period_id: Option<String>,
    pub monthly_fuel_cap: Option<u64>,
    /// URL prefixes this agent is allowed to call. `None` = default deny all.
    #[serde(default)]
    pub allowed_endpoints: Option<Vec<String>>,
    /// Domain tags for EU AI Act risk classification (e.g. "biometric", "critical-infrastructure").
    #[serde(default)]
    pub domain_tags: Vec<String>,
    /// Path-scoped filesystem permissions (C.6). When empty, the flat `fs.read`/`fs.write`
    /// capabilities govern access with no path restrictions (backward compatible).
    #[serde(default)]
    pub filesystem_permissions: Vec<FilesystemPermission>,
}

impl AgentManifest {
    /// Check whether a filesystem operation on `path` is allowed.
    ///
    /// * If `filesystem_permissions` is empty **and** the agent has the relevant flat
    ///   capability (`fs.read` / `fs.write`), access is allowed (backward compatible).
    /// * If `filesystem_permissions` is non-empty, matching rules are evaluated:
    ///   - `Deny` on any matching pattern → denied (deny wins).
    ///   - `needs_write` requires at least one `ReadWrite` match.
    ///   - Read requires at least one `ReadOnly` or `ReadWrite` match.
    ///   - No matching pattern at all → denied (default-deny).
    pub fn check_fs_permission(&self, path: &str, needs_write: bool) -> Result<(), String> {
        // Backward compatibility: no scoped permissions → fall back to flat capability check.
        if self.filesystem_permissions.is_empty() {
            let required_cap = if needs_write { "fs.write" } else { "fs.read" };
            if self.capabilities.contains(&required_cap.to_string()) {
                return Ok(());
            }
            return Err(format!(
                "Filesystem access denied: {path} (requires {required_cap})"
            ));
        }

        // Collect all matching permissions.
        let matches: Vec<&FilesystemPermission> = self
            .filesystem_permissions
            .iter()
            .filter(|fp| path_matches_pattern(path, &fp.path_pattern))
            .collect();

        // No matching pattern → default-deny.
        if matches.is_empty() {
            let mode = if needs_write { "write" } else { "read" };
            return Err(format!(
                "Filesystem access denied: {path} (requires {mode})"
            ));
        }

        // Deny takes priority over everything.
        if matches
            .iter()
            .any(|fp| fp.permission == FsPermissionLevel::Deny)
        {
            let mode = if needs_write { "write" } else { "read" };
            return Err(format!(
                "Filesystem access denied: {path} (requires {mode})"
            ));
        }

        if needs_write {
            if matches
                .iter()
                .any(|fp| fp.permission == FsPermissionLevel::ReadWrite)
            {
                return Ok(());
            }
            return Err(format!("Filesystem access denied: {path} (requires write)"));
        }

        // Read: ReadOnly or ReadWrite both suffice.
        if matches.iter().any(|fp| {
            fp.permission == FsPermissionLevel::ReadOnly
                || fp.permission == FsPermissionLevel::ReadWrite
        }) {
            return Ok(());
        }

        Err(format!("Filesystem access denied: {path} (requires read)"))
    }
}

#[derive(Debug, Deserialize)]
struct RawManifest {
    name: Option<String>,
    version: Option<String>,
    capabilities: Option<Vec<String>>,
    fuel_budget: Option<u64>,
    autonomy_level: Option<u8>,
    consent_policy_path: Option<String>,
    requester_id: Option<String>,
    schedule: Option<String>,
    default_goal: Option<String>,
    llm_model: Option<String>,
    fuel_period_id: Option<String>,
    monthly_fuel_cap: Option<u64>,
    allowed_endpoints: Option<Vec<String>>,
    domain_tags: Option<Vec<String>>,
    #[serde(default)]
    filesystem_permissions: Vec<FilesystemPermission>,
}

pub fn parse_manifest(input: &str) -> Result<AgentManifest, AgentError> {
    let raw: RawManifest =
        toml::from_str(input).map_err(|e| AgentError::ManifestError(e.to_string()))?;

    let name = raw
        .name
        .ok_or_else(|| AgentError::ManifestError("missing required field: name".to_string()))?;
    validate_name(&name)?;

    let version = raw
        .version
        .ok_or_else(|| AgentError::ManifestError("missing required field: version".to_string()))?;
    if version.trim().is_empty() {
        return Err(AgentError::ManifestError(
            "version cannot be empty".to_string(),
        ));
    }

    let capabilities = raw.capabilities.ok_or_else(|| {
        AgentError::ManifestError("missing required field: capabilities".to_string())
    })?;
    validate_capabilities(&capabilities)?;

    let fuel_budget = raw.fuel_budget.ok_or_else(|| {
        AgentError::ManifestError("missing required field: fuel_budget".to_string())
    })?;
    validate_fuel_budget(fuel_budget)?;

    let autonomy_level = parse_autonomy_level(raw.autonomy_level)?;
    let consent_policy_path = parse_optional_non_empty(raw.consent_policy_path);
    let requester_id = parse_optional_non_empty(raw.requester_id);
    validate_fuel_period_id(raw.fuel_period_id.as_deref())?;
    validate_monthly_fuel_cap(raw.monthly_fuel_cap)?;

    Ok(AgentManifest {
        name,
        version,
        capabilities,
        fuel_budget,
        autonomy_level,
        consent_policy_path,
        requester_id,
        schedule: raw.schedule,
        default_goal: raw.default_goal,
        llm_model: raw.llm_model,
        fuel_period_id: raw.fuel_period_id,
        monthly_fuel_cap: raw.monthly_fuel_cap,
        allowed_endpoints: raw.allowed_endpoints,
        domain_tags: raw.domain_tags.unwrap_or_default(),
        filesystem_permissions: raw.filesystem_permissions,
    })
}

fn validate_name(name: &str) -> Result<(), AgentError> {
    let len = name.chars().count();
    if !(MIN_NAME_LEN..=MAX_NAME_LEN).contains(&len) {
        return Err(AgentError::ManifestError(format!(
            "name must be {}-{} characters",
            MIN_NAME_LEN, MAX_NAME_LEN
        )));
    }

    let valid = name
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '-');
    if !valid {
        return Err(AgentError::ManifestError(
            "name must be alphanumeric plus hyphens only".to_string(),
        ));
    }

    Ok(())
}

fn validate_capabilities(capabilities: &[String]) -> Result<(), AgentError> {
    if capabilities.is_empty() {
        return Err(AgentError::ManifestError(
            "capabilities cannot be empty".to_string(),
        ));
    }

    let known: BTreeSet<&str> = CAPABILITY_REGISTRY.iter().copied().collect();
    for capability in capabilities {
        if !known.contains(capability.as_str()) {
            return Err(AgentError::CapabilityDenied(capability.clone()));
        }
    }

    Ok(())
}

fn validate_fuel_budget(fuel_budget: u64) -> Result<(), AgentError> {
    if fuel_budget == 0 {
        return Err(AgentError::ManifestError(
            "fuel_budget must be greater than 0".to_string(),
        ));
    }
    if fuel_budget > MAX_FUEL_BUDGET {
        return Err(AgentError::ManifestError(format!(
            "fuel_budget must be <= {}",
            MAX_FUEL_BUDGET
        )));
    }
    Ok(())
}

fn parse_autonomy_level(value: Option<u8>) -> Result<Option<u8>, AgentError> {
    let Some(value) = value else {
        return Ok(None);
    };

    let _ = AutonomyLevel::from_numeric(value).ok_or_else(|| {
        AgentError::ManifestError("autonomy_level must be one of 0, 1, 2, 3, 4, 5, 6".to_string())
    })?;
    Ok(Some(value))
}

fn parse_optional_non_empty(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn validate_fuel_period_id(period_id: Option<&str>) -> Result<(), AgentError> {
    if let Some(period_id) = period_id {
        if period_id.trim().is_empty() {
            return Err(AgentError::ManifestError(
                "fuel_period_id cannot be empty".to_string(),
            ));
        }
    }
    Ok(())
}

fn validate_monthly_fuel_cap(monthly_fuel_cap: Option<u64>) -> Result<(), AgentError> {
    if monthly_fuel_cap == Some(0) {
        return Err(AgentError::ManifestError(
            "monthly_fuel_cap must be greater than 0".to_string(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        parse_manifest, path_matches_pattern, AgentManifest, FilesystemPermission,
        FsPermissionLevel,
    };
    use crate::errors::AgentError;

    #[test]
    fn test_parse_valid_manifest() {
        let toml = r#"
name = "my-social-poster"
version = "0.1.0"
capabilities = ["web.search", "llm.query", "fs.read"]
fuel_budget = 10000
schedule = "*/10 * * * *"
llm_model = "claude-sonnet-4-5"
"#;

        let parsed = parse_manifest(toml);
        assert!(parsed.is_ok());

        let manifest = parsed.expect("valid manifest should parse");
        let expected = AgentManifest {
            name: "my-social-poster".to_string(),
            version: "0.1.0".to_string(),
            capabilities: vec![
                "web.search".to_string(),
                "llm.query".to_string(),
                "fs.read".to_string(),
            ],
            fuel_budget: 10_000,
            autonomy_level: None,
            consent_policy_path: None,
            requester_id: None,
            schedule: Some("*/10 * * * *".to_string()),
            default_goal: None,
            llm_model: Some("claude-sonnet-4-5".to_string()),
            fuel_period_id: None,
            monthly_fuel_cap: None,
            allowed_endpoints: None,
            domain_tags: vec![],
            filesystem_permissions: vec![],
        };
        assert_eq!(manifest, expected);
    }

    #[test]
    fn test_reject_invalid_manifest() {
        let missing_name = r#"
version = "0.1.0"
capabilities = ["web.search"]
fuel_budget = 100
"#;
        let empty_capabilities = r#"
name = "valid-name"
version = "0.1.0"
capabilities = []
fuel_budget = 100
"#;
        let zero_fuel = r#"
name = "valid-name"
version = "0.1.0"
capabilities = ["web.search"]
fuel_budget = 0
"#;

        let missing_name_error = parse_manifest(missing_name);
        assert!(matches!(
            missing_name_error,
            Err(AgentError::ManifestError(_))
        ));

        let empty_capabilities_error = parse_manifest(empty_capabilities);
        assert!(matches!(
            empty_capabilities_error,
            Err(AgentError::ManifestError(_))
        ));

        let zero_fuel_error = parse_manifest(zero_fuel);
        assert!(matches!(zero_fuel_error, Err(AgentError::ManifestError(_))));
    }

    #[test]
    fn test_parse_autonomy_level() {
        let toml = r#"
name = "agent-with-autonomy"
version = "0.1.0"
capabilities = ["web.search"]
fuel_budget = 100
autonomy_level = 2
consent_policy_path = "/tmp/consent.toml"
requester_id = "agent.alpha"
"#;

        let parsed = parse_manifest(toml).expect("manifest with autonomy level should parse");
        assert_eq!(parsed.autonomy_level, Some(2));
        assert_eq!(
            parsed.consent_policy_path,
            Some("/tmp/consent.toml".to_string())
        );
        assert_eq!(parsed.requester_id, Some("agent.alpha".to_string()));
    }

    #[test]
    fn test_reject_invalid_autonomy_level() {
        let invalid_autonomy = r#"
name = "valid-name"
version = "0.1.0"
capabilities = ["web.search"]
fuel_budget = 100
autonomy_level = 9
"#;

        let parsed = parse_manifest(invalid_autonomy);
        assert!(matches!(parsed, Err(AgentError::ManifestError(_))));
    }

    // ── Path matching tests ─────────────────────────────────────────────

    #[test]
    fn test_exact_path_match() {
        assert!(path_matches_pattern("/src/main.rs", "/src/main.rs"));
        assert!(!path_matches_pattern("/src/other.rs", "/src/main.rs"));
    }

    #[test]
    fn test_directory_prefix_match() {
        assert!(path_matches_pattern("/src/foo.rs", "/src/"));
        assert!(path_matches_pattern("/src/sub/bar.rs", "/src/"));
        assert!(!path_matches_pattern("/etc/passwd", "/src/"));
    }

    #[test]
    fn test_single_wildcard() {
        assert!(path_matches_pattern("/src/main.rs", "/src/*.rs"));
        assert!(!path_matches_pattern("/src/sub/main.rs", "/src/*.rs"));
        assert!(!path_matches_pattern("/src/main.py", "/src/*.rs"));
    }

    #[test]
    fn test_double_wildcard() {
        assert!(path_matches_pattern("/src/main.rs", "/src/**"));
        assert!(path_matches_pattern("/src/deep/nested/file.rs", "/src/**"));
        assert!(!path_matches_pattern("/etc/passwd", "/src/**"));
    }

    #[test]
    fn test_extension_wildcard() {
        assert!(path_matches_pattern("/any/path/file.rs", "*.rs"));
        assert!(path_matches_pattern("/file.rs", "*.rs"));
        assert!(!path_matches_pattern("/file.py", "*.rs"));
    }

    #[test]
    fn test_no_match() {
        assert!(!path_matches_pattern("/src/main.rs", "/output/"));
    }

    #[test]
    fn test_normalize_leading_dot_slash() {
        assert!(path_matches_pattern("./src/main.rs", "/src/*.rs"));
        assert!(path_matches_pattern("./src/main.rs", "/src/main.rs"));
    }

    #[test]
    fn test_normalize_trailing_slash_on_path() {
        assert!(path_matches_pattern("/src/", "/src/**"));
        assert!(path_matches_pattern("/src", "/src/**"));
    }

    // ── Permission checking tests ───────────────────────────────────────

    fn manifest_with_caps(caps: Vec<&str>) -> AgentManifest {
        AgentManifest {
            name: "test-agent".to_string(),
            version: "1.0.0".to_string(),
            capabilities: caps.into_iter().map(String::from).collect(),
            fuel_budget: 1000,
            autonomy_level: None,
            consent_policy_path: None,
            requester_id: None,
            schedule: None,
            default_goal: None,
            llm_model: None,
            fuel_period_id: None,
            monthly_fuel_cap: None,
            allowed_endpoints: None,
            domain_tags: vec![],
            filesystem_permissions: vec![],
        }
    }

    fn manifest_with_perms(caps: Vec<&str>, perms: Vec<FilesystemPermission>) -> AgentManifest {
        AgentManifest {
            name: "test-agent".to_string(),
            version: "1.0.0".to_string(),
            capabilities: caps.into_iter().map(String::from).collect(),
            fuel_budget: 1000,
            autonomy_level: None,
            consent_policy_path: None,
            requester_id: None,
            schedule: None,
            default_goal: None,
            llm_model: None,
            fuel_period_id: None,
            monthly_fuel_cap: None,
            allowed_endpoints: None,
            domain_tags: vec![],
            filesystem_permissions: perms,
        }
    }

    #[test]
    fn test_empty_permissions_backward_compat() {
        let m = manifest_with_caps(vec!["fs.read"]);
        assert!(m.check_fs_permission("/any/path", false).is_ok());
        assert!(m.check_fs_permission("/any/path", true).is_err());

        let m2 = manifest_with_caps(vec!["fs.read", "fs.write"]);
        assert!(m2.check_fs_permission("/any/path", false).is_ok());
        assert!(m2.check_fs_permission("/any/path", true).is_ok());
    }

    #[test]
    fn test_empty_permissions_no_cap() {
        let m = manifest_with_caps(vec!["llm.query"]);
        assert!(m.check_fs_permission("/any/path", false).is_err());
        assert!(m.check_fs_permission("/any/path", true).is_err());
    }

    #[test]
    fn test_scoped_read_allowed() {
        let m = manifest_with_perms(
            vec!["fs.read"],
            vec![FilesystemPermission {
                path_pattern: "/src/".to_string(),
                permission: FsPermissionLevel::ReadOnly,
            }],
        );
        assert!(m.check_fs_permission("/src/foo.rs", false).is_ok());
    }

    #[test]
    fn test_scoped_read_denied_wrong_path() {
        let m = manifest_with_perms(
            vec!["fs.read"],
            vec![FilesystemPermission {
                path_pattern: "/src/".to_string(),
                permission: FsPermissionLevel::ReadOnly,
            }],
        );
        assert!(m.check_fs_permission("/etc/passwd", false).is_err());
    }

    #[test]
    fn test_scoped_write_needs_readwrite() {
        let m = manifest_with_perms(
            vec!["fs.read", "fs.write"],
            vec![FilesystemPermission {
                path_pattern: "/src/".to_string(),
                permission: FsPermissionLevel::ReadOnly,
            }],
        );
        assert!(m.check_fs_permission("/src/foo.rs", true).is_err());
    }

    #[test]
    fn test_scoped_write_allowed() {
        let m = manifest_with_perms(
            vec!["fs.write"],
            vec![FilesystemPermission {
                path_pattern: "/output/".to_string(),
                permission: FsPermissionLevel::ReadWrite,
            }],
        );
        assert!(m.check_fs_permission("/output/result.txt", true).is_ok());
    }

    #[test]
    fn test_deny_overrides_allow() {
        let m = manifest_with_perms(
            vec!["fs.read", "fs.write"],
            vec![
                FilesystemPermission {
                    path_pattern: "/src/".to_string(),
                    permission: FsPermissionLevel::ReadWrite,
                },
                FilesystemPermission {
                    path_pattern: "/src/secret.rs".to_string(),
                    permission: FsPermissionLevel::Deny,
                },
            ],
        );
        // secret.rs matches both patterns — Deny wins
        assert!(m.check_fs_permission("/src/secret.rs", false).is_err());
        assert!(m.check_fs_permission("/src/secret.rs", true).is_err());
        // Other files under /src/ still allowed
        assert!(m.check_fs_permission("/src/main.rs", false).is_ok());
        assert!(m.check_fs_permission("/src/main.rs", true).is_ok());
    }

    #[test]
    fn test_multiple_patterns() {
        let m = manifest_with_perms(
            vec!["fs.read", "fs.write"],
            vec![
                FilesystemPermission {
                    path_pattern: "/src/*.rs".to_string(),
                    permission: FsPermissionLevel::ReadOnly,
                },
                FilesystemPermission {
                    path_pattern: "/output/".to_string(),
                    permission: FsPermissionLevel::ReadWrite,
                },
            ],
        );
        assert!(m.check_fs_permission("/src/main.rs", false).is_ok());
        assert!(m.check_fs_permission("/output/data.json", true).is_ok());
        assert!(m.check_fs_permission("/tmp/scratch", false).is_err());
    }

    #[test]
    fn test_default_deny_with_scopes() {
        let m = manifest_with_perms(
            vec!["fs.read"],
            vec![FilesystemPermission {
                path_pattern: "/safe/".to_string(),
                permission: FsPermissionLevel::ReadOnly,
            }],
        );
        // Unlisted path → denied even though agent has flat fs.read cap
        assert!(m.check_fs_permission("/unsafe/data", false).is_err());
    }

    #[test]
    fn test_parse_manifest_with_filesystem_permissions() {
        let toml = r#"
name = "scoped-agent"
version = "1.0.0"
capabilities = ["fs.read", "fs.write"]
fuel_budget = 5000

[[filesystem_permissions]]
path_pattern = "/src/"
permission = "ReadOnly"

[[filesystem_permissions]]
path_pattern = "/output/"
permission = "ReadWrite"

[[filesystem_permissions]]
path_pattern = "/secrets/"
permission = "Deny"
"#;
        let m = parse_manifest(toml).expect("should parse manifest with fs permissions");
        assert_eq!(m.filesystem_permissions.len(), 3);
        assert_eq!(m.filesystem_permissions[0].path_pattern, "/src/");
        assert_eq!(
            m.filesystem_permissions[0].permission,
            FsPermissionLevel::ReadOnly
        );
        assert_eq!(
            m.filesystem_permissions[1].permission,
            FsPermissionLevel::ReadWrite
        );
        assert_eq!(
            m.filesystem_permissions[2].permission,
            FsPermissionLevel::Deny
        );

        // Verify the permission checks work end-to-end
        assert!(m.check_fs_permission("/src/main.rs", false).is_ok());
        assert!(m.check_fs_permission("/src/main.rs", true).is_err()); // ReadOnly
        assert!(m.check_fs_permission("/output/data.txt", true).is_ok());
        assert!(m.check_fs_permission("/secrets/keys.txt", false).is_err()); // Deny
        assert!(m.check_fs_permission("/etc/passwd", false).is_err()); // default-deny
    }

    #[test]
    fn test_parse_manifest_without_filesystem_permissions() {
        // Existing manifest format — backward compatible
        let toml = r#"
name = "legacy-agent"
version = "1.0.0"
capabilities = ["fs.read", "llm.query"]
fuel_budget = 1000
"#;
        let m = parse_manifest(toml).expect("should parse legacy manifest");
        assert!(m.filesystem_permissions.is_empty());
        assert!(m.check_fs_permission("/any/path", false).is_ok());
    }
}

//! Canonical source for all security patterns used across Nexus OS.
//!
//! This module consolidates injection, PII, exfiltration, and sensitive-path
//! patterns that were previously duplicated in `prompt_firewall.rs`,
//! `defense.rs`, and `bridge.rs`. Every consumer MUST import from here —
//! no local copies.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Injection patterns (20 unique, deduplicated superset)
// ---------------------------------------------------------------------------

/// Canonical prompt-injection detection patterns (case-insensitive matching).
///
/// Merged from:
/// - `prompt_firewall.rs` (20 patterns — the superset)
/// - `defense.rs` (5 patterns — subset)
/// - `bridge.rs` (12 patterns — subset)
pub const INJECTION_PATTERNS: &[&str] = &[
    // -- existing (deduplicated across bridge.rs / shadow_sandbox / defense / messaging) --
    "ignore previous instructions",
    "ignore all previous",
    "disregard previous",
    "forget your instructions",
    "forget everything",
    "you are now",
    "new role:",
    "new instructions:",
    "system prompt:",
    "system:",
    "act as",
    "pretend you are",
    "jailbreak",
    "do anything now",
    "developer mode",
    // -- 5 new patterns --
    "base64_decode", // base64-encoded instruction smuggling
    "aW1wb3J0IG",    // common base64 prefix for "import " (encoded instructions)
    "\\u0430",       // Cyrillic "а" homoglyph for Latin "a" (unicode homoglyph attack)
    "](javascript:", // markdown link injection  ](javascript:...)
    "<system>",      // XML/tag injection attempting to create fake system blocks
];

// ---------------------------------------------------------------------------
// PII patterns
// ---------------------------------------------------------------------------

/// PII detection strings for redaction checks (simplified heuristic).
///
/// Sourced from `bridge.rs` `scan_mcp_params`.
pub const PII_PATTERNS: &[&str] = &[
    "social security",
    "credit card",
    "password:",
    "secret:",
    "api_key:",
    "private_key:",
];

// ---------------------------------------------------------------------------
// Exfiltration patterns
// ---------------------------------------------------------------------------

/// Data exfiltration indicators: internal IP prefixes, sensitive file paths,
/// and system-info commands.
///
/// Sourced from `prompt_firewall.rs` `OutputFilter`.
pub const EXFIL_PATTERNS: &[&str] = &[
    "10.", "172.16.", "192.168.", "/etc/", "/proc/", "/sys/", "uname",
];

// ---------------------------------------------------------------------------
// Sensitive paths
// ---------------------------------------------------------------------------

/// Sensitive filesystem paths checked during MCP param scanning.
///
/// Sourced from `bridge.rs` `scan_mcp_params`.
pub const SENSITIVE_PATHS: &[&str] = &["/etc/", "/sys/", "/proc/"];

// ---------------------------------------------------------------------------
// Regex pattern strings
// ---------------------------------------------------------------------------

/// SSN regex pattern string (US Social Security Number: NNN-NN-NNNN).
pub const SSN_PATTERN: &str = r"\b\d{3}-\d{2}-\d{4}\b";

/// Passport number regex pattern string (1-2 uppercase letters followed by 6-9 digits).
pub const PASSPORT_PATTERN: &str = r"(?i)\b[A-Z]{1,2}\d{6,9}\b";

/// RFC 1918 internal IP address regex pattern string.
pub const INTERNAL_IP_PATTERN: &str = r"\b(?:10\.\d{1,3}\.\d{1,3}\.\d{1,3}|172\.(?:1[6-9]|2\d|3[01])\.\d{1,3}\.\d{1,3}|192\.168\.\d{1,3}\.\d{1,3})\b";

// ---------------------------------------------------------------------------
// Thresholds
// ---------------------------------------------------------------------------

/// Maximum prompt size in bytes before the firewall triggers a context-overflow
/// block. Prompts exceeding this length could push system instructions out of
/// the LLM context window.
pub const CONTEXT_OVERFLOW_THRESHOLD_BYTES: usize = 100_000;

// ---------------------------------------------------------------------------
// PatternSummary
// ---------------------------------------------------------------------------

/// Summary counts of each pattern category, for use by the CLI and UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternSummary {
    pub injection_count: usize,
    pub pii_count: usize,
    pub exfil_count: usize,
    pub sensitive_path_count: usize,
    pub has_ssn_detection: bool,
    pub has_passport_detection: bool,
    pub has_internal_ip_detection: bool,
    pub context_overflow_threshold_bytes: usize,
}

/// Return a [`PatternSummary`] reflecting the current canonical pattern counts.
pub fn pattern_summary() -> PatternSummary {
    PatternSummary {
        injection_count: INJECTION_PATTERNS.len(),
        pii_count: PII_PATTERNS.len(),
        exfil_count: EXFIL_PATTERNS.len(),
        sensitive_path_count: SENSITIVE_PATHS.len(),
        has_ssn_detection: true,
        has_passport_detection: true,
        has_internal_ip_detection: true,
        context_overflow_threshold_bytes: CONTEXT_OVERFLOW_THRESHOLD_BYTES,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pattern_summary() {
        let summary = pattern_summary();
        assert_eq!(summary.injection_count, 20);
        assert_eq!(summary.pii_count, 6);
        assert_eq!(summary.exfil_count, 7);
        assert_eq!(summary.sensitive_path_count, 3);
        assert!(summary.has_ssn_detection);
        assert!(summary.has_passport_detection);
        assert!(summary.has_internal_ip_detection);
        assert_eq!(summary.context_overflow_threshold_bytes, 100_000);
    }
}

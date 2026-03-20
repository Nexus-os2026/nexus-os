//! Privacy scanner — deep PII and secret detection with regex and Luhn checks.
//!
//! Goes beyond the simple substring matching in
//! [`crate::firewall::patterns::PII_PATTERNS`] by using compiled regex rules,
//! credit-card Luhn validation, and configurable custom patterns.

use regex::Regex;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// PrivacyCategory
// ---------------------------------------------------------------------------

/// Category of a detected privacy-sensitive value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PrivacyCategory {
    ApiKey,
    Password,
    PrivateIp,
    Pii,
    CreditCard,
    Ssn,
    Custom,
}

// ---------------------------------------------------------------------------
// PrivacyRule
// ---------------------------------------------------------------------------

/// A single privacy detection rule.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrivacyRule {
    pub name: String,
    pub pattern: String,
    pub category: PrivacyCategory,
}

// ---------------------------------------------------------------------------
// PrivacyViolation
// ---------------------------------------------------------------------------

/// A single privacy violation found during scanning.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrivacyViolation {
    pub rule_name: String,
    pub category: PrivacyCategory,
    /// Redacted form of the matched text (first 4 chars + "***").
    pub matched_text_redacted: String,
    /// Byte offset in the scanned string.
    pub position: usize,
}

// ---------------------------------------------------------------------------
// ScanResult
// ---------------------------------------------------------------------------

/// Aggregated result of a privacy scan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanResult {
    pub violations: Vec<PrivacyViolation>,
}

impl ScanResult {
    pub fn is_clean(&self) -> bool {
        self.violations.is_empty()
    }

    pub fn violation_count(&self) -> usize {
        self.violations.len()
    }
}

// ---------------------------------------------------------------------------
// Compiled rule (not serialized)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct CompiledRule {
    name: String,
    regex: Regex,
    category: PrivacyCategory,
    /// If true, matched values are also validated with the Luhn algorithm.
    luhn_check: bool,
}

// ---------------------------------------------------------------------------
// PrivacyScanner
// ---------------------------------------------------------------------------

/// Configurable privacy scanner with built-in and custom rules.
///
/// Built-in rules cover:
/// - API keys (OpenAI `sk-`, AWS `AKIA`, generic `key-`)
/// - Passwords in key-value format
/// - Private/RFC-1918 IP addresses
/// - Credit card numbers (with Luhn validation)
/// - US Social Security Numbers
/// - Email addresses
#[derive(Debug, Clone)]
pub struct PrivacyScanner {
    rules: Vec<CompiledRule>,
}

impl PrivacyScanner {
    /// Create a scanner with default built-in rules.
    pub fn new() -> Self {
        let mut scanner = Self { rules: Vec::new() };
        scanner.add_builtin_rules();
        scanner
    }

    /// Create a scanner with no rules (add custom rules manually).
    pub fn empty() -> Self {
        Self { rules: Vec::new() }
    }

    /// Add a custom rule from a [`PrivacyRule`].
    pub fn add_rule(&mut self, rule: &PrivacyRule) {
        if let Ok(regex) = Regex::new(&rule.pattern) {
            self.rules.push(CompiledRule {
                name: rule.name.clone(),
                regex,
                category: rule.category,
                luhn_check: false,
            });
        }
    }

    /// Return the serializable rule definitions (without compiled regexes).
    pub fn rule_definitions(&self) -> Vec<PrivacyRule> {
        self.rules
            .iter()
            .map(|r| PrivacyRule {
                name: r.name.clone(),
                pattern: r.regex.as_str().to_string(),
                category: r.category,
            })
            .collect()
    }

    /// Scan outgoing data for privacy violations.
    pub fn scan_outgoing(&self, data: &str) -> ScanResult {
        let mut violations = Vec::new();

        for rule in &self.rules {
            for mat in rule.regex.find_iter(data) {
                let matched = mat.as_str();

                // For credit card rules, apply Luhn validation
                if rule.luhn_check {
                    let digits_only: String =
                        matched.chars().filter(|c| c.is_ascii_digit()).collect();
                    if !luhn_check(&digits_only) {
                        continue;
                    }
                }

                violations.push(PrivacyViolation {
                    rule_name: rule.name.clone(),
                    category: rule.category,
                    matched_text_redacted: redact(matched),
                    position: mat.start(),
                });
            }
        }

        ScanResult { violations }
    }

    // -----------------------------------------------------------------------
    // Built-in rules
    // -----------------------------------------------------------------------

    fn add_builtin_rules(&mut self) {
        let builtin: &[(&str, &str, PrivacyCategory, bool)] = &[
            (
                "openai_api_key",
                r"sk-[A-Za-z0-9]{20,}",
                PrivacyCategory::ApiKey,
                false,
            ),
            (
                "aws_access_key",
                r"AKIA[0-9A-Z]{16}",
                PrivacyCategory::ApiKey,
                false,
            ),
            (
                "generic_api_key",
                r#"(?i)(?:api[_-]?key|apikey)\s*[:=]\s*['"]?([A-Za-z0-9_\-]{16,})"#,
                PrivacyCategory::ApiKey,
                false,
            ),
            (
                "password_value",
                r"(?i)(?:password|passwd|pwd)\s*[:=]\s*\S+",
                PrivacyCategory::Password,
                false,
            ),
            (
                "private_ip",
                r"\b(?:10\.\d{1,3}\.\d{1,3}\.\d{1,3}|172\.(?:1[6-9]|2\d|3[01])\.\d{1,3}\.\d{1,3}|192\.168\.\d{1,3}\.\d{1,3})\b",
                PrivacyCategory::PrivateIp,
                false,
            ),
            (
                "credit_card",
                r"\b(?:\d[ -]*?){13,19}\b",
                PrivacyCategory::CreditCard,
                true,
            ),
            ("ssn", r"\b\d{3}-\d{2}-\d{4}\b", PrivacyCategory::Ssn, false),
            (
                "email_address",
                r"\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}\b",
                PrivacyCategory::Pii,
                false,
            ),
        ];

        for (name, pattern, category, luhn_check) in builtin {
            if let Ok(regex) = Regex::new(pattern) {
                self.rules.push(CompiledRule {
                    name: (*name).into(),
                    regex,
                    category: *category,
                    luhn_check: *luhn_check,
                });
            }
        }
    }
}

impl Default for PrivacyScanner {
    fn default() -> Self {
        Self::new()
    }
}

// We need Serialize/Deserialize for PrivacyScanner to match module-level exports,
// but compiled Regex is not serializable. We serialize rule definitions and
// recompile on deserialization.
impl Serialize for PrivacyScanner {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.rule_definitions().serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for PrivacyScanner {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let rules = Vec::<PrivacyRule>::deserialize(deserializer)?;
        let mut scanner = Self::empty();
        for rule in &rules {
            scanner.add_rule(rule);
        }
        Ok(scanner)
    }
}

// ---------------------------------------------------------------------------
// Luhn algorithm
// ---------------------------------------------------------------------------

/// Validate a digit string using the Luhn algorithm (ISO/IEC 7812-1).
fn luhn_check(digits: &str) -> bool {
    if digits.len() < 13 {
        return false;
    }
    let mut sum = 0u32;
    let mut double = false;

    for ch in digits.chars().rev() {
        let Some(d) = ch.to_digit(10) else {
            return false;
        };
        let val = if double {
            let v = d * 2;
            if v > 9 {
                v - 9
            } else {
                v
            }
        } else {
            d
        };
        sum += val;
        double = !double;
    }

    sum.is_multiple_of(10)
}

/// Redact a matched string: keep first 4 chars, replace the rest with `***`.
fn redact(s: &str) -> String {
    if s.len() <= 4 {
        "****".to_string()
    } else {
        format!("{}***", &s[..4])
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_openai_key() {
        let scanner = PrivacyScanner::new();
        let result = scanner.scan_outgoing("My key is sk-abcdefghijklmnopqrstuvwxyz");
        assert!(!result.is_clean());
        assert_eq!(result.violations[0].category, PrivacyCategory::ApiKey);
        assert!(result.violations[0].matched_text_redacted.contains("***"));
    }

    #[test]
    fn test_detect_aws_key() {
        let scanner = PrivacyScanner::new();
        let result = scanner.scan_outgoing("AWS key: AKIAIOSFODNN7EXAMPLE");
        assert!(result
            .violations
            .iter()
            .any(|v| v.rule_name == "aws_access_key"));
    }

    #[test]
    fn test_detect_password() {
        let scanner = PrivacyScanner::new();
        let result = scanner.scan_outgoing("password: hunter2");
        assert!(result
            .violations
            .iter()
            .any(|v| v.category == PrivacyCategory::Password));
    }

    #[test]
    fn test_detect_private_ip() {
        let scanner = PrivacyScanner::new();
        let result = scanner.scan_outgoing("Server at 192.168.1.100");
        assert!(result
            .violations
            .iter()
            .any(|v| v.category == PrivacyCategory::PrivateIp));
    }

    #[test]
    fn test_detect_ssn() {
        let scanner = PrivacyScanner::new();
        let result = scanner.scan_outgoing("SSN: 123-45-6789");
        assert!(result
            .violations
            .iter()
            .any(|v| v.category == PrivacyCategory::Ssn));
    }

    #[test]
    fn test_detect_email() {
        let scanner = PrivacyScanner::new();
        let result = scanner.scan_outgoing("Contact user@example.com for info");
        assert!(result
            .violations
            .iter()
            .any(|v| v.category == PrivacyCategory::Pii));
    }

    #[test]
    fn test_luhn_valid() {
        // Visa test number
        assert!(luhn_check("4111111111111111"));
        // Mastercard test number
        assert!(luhn_check("5500000000000004"));
    }

    #[test]
    fn test_luhn_invalid() {
        assert!(!luhn_check("4111111111111112"));
        assert!(!luhn_check("12345"));
    }

    #[test]
    fn test_credit_card_with_luhn() {
        let scanner = PrivacyScanner::new();
        // Valid Visa test number
        let result = scanner.scan_outgoing("Card: 4111111111111111");
        assert!(result
            .violations
            .iter()
            .any(|v| v.category == PrivacyCategory::CreditCard));
    }

    #[test]
    fn test_clean_text() {
        let scanner = PrivacyScanner::new();
        let result = scanner.scan_outgoing("Hello, how are you today?");
        assert!(result.is_clean());
    }

    #[test]
    fn test_custom_rule() {
        let mut scanner = PrivacyScanner::empty();
        scanner.add_rule(&PrivacyRule {
            name: "internal_token".into(),
            pattern: r"INTERNAL-[A-Z0-9]{10}".into(),
            category: PrivacyCategory::Custom,
        });
        let result = scanner.scan_outgoing("Token: INTERNAL-ABCDE12345");
        assert_eq!(result.violation_count(), 1);
        assert_eq!(result.violations[0].category, PrivacyCategory::Custom);
    }

    #[test]
    fn test_redact() {
        assert_eq!(redact("sk-abcdef"), "sk-a***");
        assert_eq!(redact("ab"), "****");
    }

    #[test]
    fn test_serialize_deserialize() {
        let scanner = PrivacyScanner::new();
        let json = serde_json::to_string(&scanner).unwrap();
        let restored: PrivacyScanner = serde_json::from_str(&json).unwrap();
        assert_eq!(scanner.rules.len(), restored.rules.len());
    }
}

use regex::Regex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::sync::OnceLock;

fn empty_regex() -> Regex {
    // "^$" is always valid — or_else chain provides multiple trivial fallbacks.
    Regex::new("^$")
        .or_else(|_| Regex::new(""))
        .or_else(|_| Regex::new("x"))
        .or_else(|_| Regex::new("a"))
        .unwrap_or_else(|e| {
            eprintln!("regex engine broken, cannot compile trivial patterns: {e}");
            // Return a regex that matches nothing — last resort construction
            Regex::new("[^\\s\\S]").unwrap_or_else(|_| Regex::new(".").unwrap_or_else(|_| {
                // If the regex engine truly cannot compile ANY pattern, we have a fundamentally
                // broken runtime. Abort is the only safe option vs silently proceeding.
                std::process::abort()
            }))
        })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum FindingKind {
    Email,
    Phone,
    CreditCard,
    ApiKey,
    SecretBlock,
    AddressLike,
    Other,
}

impl FindingKind {
    pub fn as_key(self) -> &'static str {
        match self {
            FindingKind::Email => "email",
            FindingKind::Phone => "phone",
            FindingKind::CreditCard => "credit_card",
            FindingKind::ApiKey => "api_key",
            FindingKind::SecretBlock => "secret_block",
            FindingKind::AddressLike => "address_like",
            FindingKind::Other => "other",
        }
    }

    fn replacement_token(self) -> &'static str {
        match self {
            FindingKind::Email => "<redacted:email>",
            FindingKind::Phone => "<redacted:phone>",
            FindingKind::CreditCard => "<redacted:credit_card>",
            FindingKind::ApiKey => "<redacted:api_key>",
            FindingKind::SecretBlock => "<redacted:secret_block>",
            FindingKind::AddressLike => "<redacted:address>",
            FindingKind::Other => "<redacted:other>",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Finding {
    pub kind: FindingKind,
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RedactionMode {
    Strict,
    Balanced,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RedactionPolicy {
    pub mode: RedactionMode,
    pub allowlist: Vec<String>,
    pub max_context_chars: usize,
    pub strip_log_sections: bool,
}

impl Default for RedactionPolicy {
    fn default() -> Self {
        Self {
            mode: RedactionMode::Strict,
            allowlist: Vec::new(),
            max_context_chars: 12_000,
            strip_log_sections: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct RedactionSummary {
    pub counts_by_kind: BTreeMap<String, usize>,
    pub total_findings: usize,
}

impl RedactionSummary {
    fn from_findings(findings: &[Finding]) -> Self {
        let mut counts_by_kind = BTreeMap::new();
        for finding in findings {
            *counts_by_kind
                .entry(finding.kind.as_key().to_string())
                .or_insert(0) += 1;
        }
        Self {
            counts_by_kind,
            total_findings: findings.len(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PromptEnvelope {
    pub purpose: String,
    pub sensitivity: String,
    pub context_ids: Vec<String>,
    pub redaction_summary: RedactionSummary,
    pub payload_redacted: String,
    pub payload_hash: [u8; 32],
}

impl PromptEnvelope {
    pub fn payload_hash_hex(&self) -> String {
        bytes_to_hex(self.payload_hash)
    }

    pub fn render_prompt(&self) -> String {
        let context_ids = if self.context_ids.is_empty() {
            "none".to_string()
        } else {
            self.context_ids.join(",")
        };
        let summary = if self.redaction_summary.counts_by_kind.is_empty() {
            "none".to_string()
        } else {
            self.redaction_summary
                .counts_by_kind
                .iter()
                .map(|(kind, count)| format!("{kind}:{count}"))
                .collect::<Vec<_>>()
                .join(",")
        };

        format!(
            "[nexus.prompt]\npurpose={}\nsensitivity={}\ncontext_ids={}\nredaction_summary={}\npayload_hash={}\n[/nexus.prompt]\n{}",
            self.purpose,
            self.sensitivity,
            context_ids,
            summary,
            self.payload_hash_hex(),
            self.payload_redacted
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RedactionResult {
    pub minimized_payload: String,
    pub findings: Vec<Finding>,
    pub redacted_payload: String,
    pub payload_hash: [u8; 32],
    pub payload_hash_hex: String,
    pub redacted_hash: [u8; 32],
    pub redacted_hash_hex: String,
    pub outbound_prompt: String,
    pub outbound_prompt_hash_hex: String,
    pub summary: RedactionSummary,
    pub envelope: PromptEnvelope,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RedactionMetrics {
    pub total_scans: u64,
    pub total_findings: u64,
    pub total_scans_with_findings: u64,
    pub findings_by_kind: BTreeMap<String, u64>,
    pub residual_findings_after_redaction: u64,
}

impl RedactionMetrics {
    pub fn zero_pii_leakage_kpi(&self) -> bool {
        self.residual_findings_after_redaction == 0
    }
}

#[derive(Debug, Clone)]
pub struct RedactionEngine {
    policy: RedactionPolicy,
    metrics: RedactionMetrics,
}

impl Default for RedactionEngine {
    fn default() -> Self {
        Self::new(RedactionPolicy::default())
    }
}

impl RedactionEngine {
    pub fn new(policy: RedactionPolicy) -> Self {
        Self {
            policy,
            metrics: RedactionMetrics::default(),
        }
    }

    pub fn policy(&self) -> &RedactionPolicy {
        &self.policy
    }

    pub fn set_policy(&mut self, policy: RedactionPolicy) {
        self.policy = policy;
    }

    pub fn metrics(&self) -> &RedactionMetrics {
        &self.metrics
    }

    pub fn scan(text: &str) -> Vec<Finding> {
        Self::scan_with_policy(text, &RedactionPolicy::default())
    }

    pub fn apply(text: &str, findings: &[Finding]) -> String {
        apply_findings(text, findings)
    }

    pub fn minimize_context(&self, text: &str) -> String {
        minimize_context_with_policy(text, &self.policy)
    }

    pub fn process_prompt(
        &mut self,
        purpose: &str,
        sensitivity: &str,
        context_ids: Vec<String>,
        payload: &str,
    ) -> RedactionResult {
        let minimized_payload = self.minimize_context(payload);
        let findings = Self::scan_with_policy(minimized_payload.as_str(), &self.policy);
        let redacted_payload = Self::apply(minimized_payload.as_str(), findings.as_slice());
        let summary = RedactionSummary::from_findings(findings.as_slice());
        let payload_hash = sha256_bytes(minimized_payload.as_bytes());
        let redacted_hash = sha256_bytes(redacted_payload.as_bytes());
        let envelope = PromptEnvelope {
            purpose: purpose.to_string(),
            sensitivity: sensitivity.to_string(),
            context_ids,
            redaction_summary: summary.clone(),
            payload_redacted: redacted_payload.clone(),
            payload_hash,
        };
        let outbound_prompt = envelope.render_prompt();
        let outbound_prompt_hash = sha256_bytes(outbound_prompt.as_bytes());
        let residual_findings =
            Self::scan_with_policy(redacted_payload.as_str(), &self.policy).len() as u64;

        self.record_metrics(findings.as_slice(), residual_findings);

        RedactionResult {
            minimized_payload,
            findings,
            redacted_payload,
            payload_hash,
            payload_hash_hex: bytes_to_hex(payload_hash),
            redacted_hash,
            redacted_hash_hex: bytes_to_hex(redacted_hash),
            outbound_prompt,
            outbound_prompt_hash_hex: bytes_to_hex(outbound_prompt_hash),
            summary,
            envelope,
        }
    }

    pub fn scan_with_policy(text: &str, policy: &RedactionPolicy) -> Vec<Finding> {
        let mut findings = Vec::new();

        collect_regex_findings(
            email_pattern(),
            FindingKind::Email,
            text,
            policy,
            findings.as_mut(),
        );
        collect_regex_findings(
            phone_pattern(),
            FindingKind::Phone,
            text,
            policy,
            findings.as_mut(),
        );
        collect_credit_card_findings(text, policy, findings.as_mut());
        collect_regex_findings(
            sk_key_pattern(),
            FindingKind::ApiKey,
            text,
            policy,
            findings.as_mut(),
        );
        collect_regex_findings(
            akia_key_pattern(),
            FindingKind::ApiKey,
            text,
            policy,
            findings.as_mut(),
        );
        collect_regex_findings(
            bearer_pattern(),
            FindingKind::ApiKey,
            text,
            policy,
            findings.as_mut(),
        );
        collect_regex_findings(
            pem_pattern(),
            FindingKind::SecretBlock,
            text,
            policy,
            findings.as_mut(),
        );

        if policy.mode == RedactionMode::Strict {
            collect_regex_findings(
                address_pattern(),
                FindingKind::AddressLike,
                text,
                policy,
                findings.as_mut(),
            );
        }

        normalize_findings(findings)
    }

    fn record_metrics(&mut self, findings: &[Finding], residual_findings: u64) {
        self.metrics.total_scans = self.metrics.total_scans.saturating_add(1);
        self.metrics.total_findings = self
            .metrics
            .total_findings
            .saturating_add(findings.len() as u64);
        self.metrics.residual_findings_after_redaction = self
            .metrics
            .residual_findings_after_redaction
            .saturating_add(residual_findings);
        if !findings.is_empty() {
            self.metrics.total_scans_with_findings =
                self.metrics.total_scans_with_findings.saturating_add(1);
        }
        for finding in findings {
            let entry = self
                .metrics
                .findings_by_kind
                .entry(finding.kind.as_key().to_string())
                .or_insert(0);
            *entry = entry.saturating_add(1);
        }
    }
}

fn normalize_findings(mut findings: Vec<Finding>) -> Vec<Finding> {
    findings.sort_by(|left, right| {
        left.start
            .cmp(&right.start)
            .then(right.end.cmp(&left.end))
            .then(left.kind.cmp(&right.kind))
    });

    let mut normalized: Vec<Finding> = Vec::new();
    for finding in findings {
        if finding.end <= finding.start {
            continue;
        }
        if let Some(previous) = normalized.last() {
            if finding.start < previous.end {
                continue;
            }
        }
        normalized.push(finding);
    }

    normalized
}

fn apply_findings(text: &str, findings: &[Finding]) -> String {
    if findings.is_empty() {
        return text.to_string();
    }

    let mut ordered = findings.to_vec();
    ordered.sort_by(|left, right| left.start.cmp(&right.start).then(left.end.cmp(&right.end)));

    let mut output = String::new();
    let mut cursor = 0usize;
    for finding in ordered {
        if finding.start < cursor || finding.start >= text.len() || finding.end > text.len() {
            continue;
        }
        output.push_str(&text[cursor..finding.start]);
        output.push_str(finding.kind.replacement_token());
        cursor = finding.end;
    }
    output.push_str(&text[cursor..]);
    output
}

fn collect_regex_findings(
    pattern: &Regex,
    kind: FindingKind,
    text: &str,
    policy: &RedactionPolicy,
    findings: &mut Vec<Finding>,
) {
    for matched in pattern.find_iter(text) {
        if is_allowlisted(&text[matched.start()..matched.end()], policy) {
            continue;
        }
        findings.push(Finding {
            kind,
            start: matched.start(),
            end: matched.end(),
        });
    }
}

fn collect_credit_card_findings(text: &str, policy: &RedactionPolicy, findings: &mut Vec<Finding>) {
    for matched in credit_card_pattern().find_iter(text) {
        let candidate = &text[matched.start()..matched.end()];
        let digits: String = candidate.chars().filter(char::is_ascii_digit).collect();
        if digits.len() < 13 || digits.len() > 19 {
            continue;
        }
        if !passes_luhn(digits.as_str()) || is_allowlisted(candidate, policy) {
            continue;
        }
        findings.push(Finding {
            kind: FindingKind::CreditCard,
            start: matched.start(),
            end: matched.end(),
        });
    }
}

fn is_allowlisted(candidate: &str, policy: &RedactionPolicy) -> bool {
    let candidate_lower = candidate.to_ascii_lowercase();
    policy
        .allowlist
        .iter()
        .any(|entry| candidate_lower.contains(&entry.to_ascii_lowercase()))
}

fn passes_luhn(digits: &str) -> bool {
    let mut sum = 0u32;
    let mut double = false;
    for ch in digits.chars().rev() {
        let mut value = match ch.to_digit(10) {
            Some(value) => value,
            None => return false,
        };
        if double {
            value *= 2;
            if value > 9 {
                value -= 9;
            }
        }
        sum += value;
        double = !double;
    }
    sum.is_multiple_of(10)
}

fn minimize_context_with_policy(text: &str, policy: &RedactionPolicy) -> String {
    let stripped = if policy.strip_log_sections {
        strip_obvious_logs(text)
    } else {
        text.to_string()
    };

    if stripped.chars().count() <= policy.max_context_chars {
        return stripped;
    }

    let truncated: String = stripped.chars().take(policy.max_context_chars).collect();
    if policy.mode == RedactionMode::Strict {
        format!("{truncated}\n<context_truncated/>")
    } else {
        truncated
    }
}

fn strip_obvious_logs(text: &str) -> String {
    let mut kept_lines = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim_start();
        if log_line_pattern().is_match(trimmed) {
            continue;
        }
        kept_lines.push(line);
    }
    kept_lines.join("\n")
}

fn sha256_bytes(input: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(input);
    let digest = hasher.finalize();
    let mut output = [0_u8; 32];
    output.copy_from_slice(digest.as_ref());
    output
}

fn bytes_to_hex(bytes: [u8; 32]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn email_pattern() -> &'static Regex {
    static EMAIL_PATTERN: OnceLock<Regex> = OnceLock::new();
    EMAIL_PATTERN.get_or_init(|| {
        Regex::new(r"(?i)\b[a-z0-9._%+\-]+@[a-z0-9.\-]+\.[a-z]{2,}\b").unwrap_or_else(|e| {
            eprintln!("Failed to compile email regex: {e}");
            empty_regex()
        })
    })
}

fn phone_pattern() -> &'static Regex {
    static PHONE_PATTERN: OnceLock<Regex> = OnceLock::new();
    PHONE_PATTERN.get_or_init(|| {
        Regex::new(r"\b(?:\+?\d[\d .()\-]{7,}\d)\b").unwrap_or_else(|e| {
            eprintln!("Failed to compile phone regex: {e}");
            empty_regex()
        })
    })
}

fn credit_card_pattern() -> &'static Regex {
    static CREDIT_CARD_PATTERN: OnceLock<Regex> = OnceLock::new();
    CREDIT_CARD_PATTERN.get_or_init(|| {
        Regex::new(r"\b(?:\d[ -]?){13,19}\b").unwrap_or_else(|e| {
            eprintln!("Failed to compile credit card regex: {e}");
            empty_regex()
        })
    })
}

fn sk_key_pattern() -> &'static Regex {
    static SK_KEY_PATTERN: OnceLock<Regex> = OnceLock::new();
    SK_KEY_PATTERN.get_or_init(|| {
        Regex::new(r"\bsk-[A-Za-z0-9]{16,}\b").unwrap_or_else(|e| {
            eprintln!("Failed to compile sk key regex: {e}");
            empty_regex()
        })
    })
}

fn akia_key_pattern() -> &'static Regex {
    static AKIA_KEY_PATTERN: OnceLock<Regex> = OnceLock::new();
    AKIA_KEY_PATTERN.get_or_init(|| {
        Regex::new(r"\bAKIA[0-9A-Z]{16}\b").unwrap_or_else(|e| {
            eprintln!("Failed to compile AKIA key regex: {e}");
            empty_regex()
        })
    })
}

fn bearer_pattern() -> &'static Regex {
    static BEARER_PATTERN: OnceLock<Regex> = OnceLock::new();
    BEARER_PATTERN.get_or_init(|| {
        Regex::new(r"(?i)\bbearer\s+[A-Za-z0-9._\-]{16,}\b").unwrap_or_else(|e| {
            eprintln!("Failed to compile bearer token regex: {e}");
            empty_regex()
        })
    })
}

fn pem_pattern() -> &'static Regex {
    static PEM_PATTERN: OnceLock<Regex> = OnceLock::new();
    PEM_PATTERN.get_or_init(|| {
        Regex::new(r"(?s)-----BEGIN [A-Z0-9 ]+-----.*?-----END [A-Z0-9 ]+-----").unwrap_or_else(
            |e| {
                eprintln!("Failed to compile PEM regex: {e}");
                empty_regex()
            },
        )
    })
}

fn address_pattern() -> &'static Regex {
    static ADDRESS_PATTERN: OnceLock<Regex> = OnceLock::new();
    ADDRESS_PATTERN.get_or_init(|| {
        Regex::new(
            r"(?i)\b\d{1,5}\s+[A-Za-z0-9.\- ]+\s(?:street|st|avenue|ave|road|rd|boulevard|blvd|drive|dr|lane|ln|way)\b",
        )
        .unwrap_or_else(|e| {
            eprintln!("Failed to compile address regex: {e}");
            empty_regex()
        })
    })
}

fn log_line_pattern() -> &'static Regex {
    static LOG_LINE_PATTERN: OnceLock<Regex> = OnceLock::new();
    LOG_LINE_PATTERN.get_or_init(|| {
        Regex::new(
            r"^(?:\[\s*(?:TRACE|DEBUG|INFO|WARN|ERROR)\s*\]|\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2})",
        )
        .unwrap_or_else(|e| {
            eprintln!("Failed to compile log line regex: {e}");
            empty_regex()
        })
    })
}

#[cfg(test)]
mod tests {
    use super::{FindingKind, RedactionEngine, RedactionMode, RedactionPolicy};

    #[test]
    fn test_redact_email() {
        let input = "Contact me at a@b.com for details";
        let findings = RedactionEngine::scan(input);
        let redacted = RedactionEngine::apply(input, findings.as_slice());

        assert!(findings
            .iter()
            .any(|finding| finding.kind == FindingKind::Email));
        assert!(redacted.contains("<redacted:email>"));
        assert!(!redacted.contains("a@b.com"));
    }

    #[test]
    fn test_redact_api_key() {
        let input = "token=sk-1234567890ABCDEFGHIJKLMNOP";
        let findings = RedactionEngine::scan(input);
        let redacted = RedactionEngine::apply(input, findings.as_slice());

        assert!(findings
            .iter()
            .any(|finding| finding.kind == FindingKind::ApiKey));
        assert!(redacted.contains("<redacted:api_key>"));
        assert!(!redacted.contains("sk-1234567890ABCDEFGHIJKLMNOP"));
    }

    #[test]
    fn test_context_minimizer_respects_budget() {
        let mut engine = RedactionEngine::new(RedactionPolicy {
            mode: RedactionMode::Strict,
            allowlist: Vec::new(),
            max_context_chars: 16,
            strip_log_sections: false,
        });

        let result = engine.process_prompt(
            "test",
            "strict",
            vec!["ctx-1".to_string()],
            "0123456789abcdefXYZ",
        );

        assert!(result.minimized_payload.len() <= 16 + "<context_truncated/>".len() + 1);
    }

    #[test]
    fn test_metrics_zero_pii_leakage_kpi() {
        let mut engine = RedactionEngine::default();
        let _ = engine.process_prompt(
            "test",
            "strict",
            vec!["ctx-2".to_string()],
            "email a@b.com and token sk-1234567890ABCDEFGHIJKLMNOP",
        );

        assert_eq!(engine.metrics().total_scans, 1);
        assert!(engine.metrics().total_findings >= 2);
        assert!(engine.metrics().zero_pii_leakage_kpi());
    }
}

//! Streaming build events for real-time progress during website generation.
//!
//! Defines event types emitted on the `build-stream` Tauri channel, phase
//! detection from accumulated output, and cost calculation helpers.

use serde::Serialize;

// ─── Build Stream Events ──────────────────────────────────────────────────

/// Events emitted on the `build-stream` Tauri channel during website generation.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum BuildStreamEvent {
    /// Emitted when build starts. Frontend shows estimate panel.
    BuildStarted {
        project_name: String,
        estimated_cost: f64,
        estimated_tasks: usize,
        model_name: String,
        timestamp: String,
    },

    /// Emitted as the LLM generates tokens. Frontend shows phase + progress.
    GenerationProgress {
        phase: GenerationPhase,
        tokens_generated: usize,
        estimated_total_tokens: usize,
        elapsed_seconds: f64,
        raw_chunk: Option<String>,
    },

    /// Emitted when generation completes. Frontend shows receipt.
    BuildCompleted {
        project_name: String,
        total_lines: usize,
        total_chars: usize,
        input_tokens: usize,
        output_tokens: usize,
        actual_cost: f64,
        model_name: String,
        elapsed_seconds: f64,
        checkpoint_id: String,
        governance_status: GovernanceStatus,
        /// The output directory containing the generated files.
        output_dir: String,
    },

    /// Emitted on failure.
    BuildFailed {
        error: String,
        tokens_consumed: usize,
        cost_consumed: f64,
    },
}

/// Current phase of the generation process, detected from accumulated output.
#[derive(Debug, Clone, Serialize)]
pub enum GenerationPhase {
    /// First ~5% of tokens: model is "thinking"
    Analyzing,
    /// 5-15%: DOCTYPE, head, meta tags appearing
    Scaffolding,
    /// 15-40%: CSS variables, styles
    Styling,
    /// 40-85%: HTML body, sections, components
    Building,
    /// 85-95%: JavaScript, interactivity
    Scripting,
    /// 95-100%: closing tags, cleanup
    Finalizing,
}

/// Quick governance scan results for the generated output.
#[derive(Debug, Clone, Serialize)]
pub struct GovernanceStatus {
    pub owasp_passed: bool,
    pub xss_clean: bool,
    pub aria_present: bool,
    pub signed: bool,
}

// ─── Phase Detection ──────────────────────────────────────────────────────

/// Detect the current generation phase from accumulated output and progress.
///
/// Content-based detection takes priority over position-based fallback.
pub fn detect_phase(
    accumulated: &str,
    token_count: usize,
    estimated_total: usize,
) -> GenerationPhase {
    let progress = if estimated_total > 0 {
        token_count as f64 / estimated_total as f64
    } else {
        0.0
    };
    let lower = accumulated.to_lowercase();

    // Content-based detection takes priority over position
    if lower.contains("<script")
        || lower.contains("function ")
        || lower.contains("addeventlistener")
    {
        return GenerationPhase::Scripting;
    }
    if (lower.contains("<main") || lower.contains("<section") || lower.contains("<div class"))
        && progress > 0.4
    {
        return GenerationPhase::Building;
    }
    if lower.contains("<style") || lower.contains("--color-") || lower.contains("font-family") {
        return GenerationPhase::Styling;
    }
    if lower.contains("<!doctype") || lower.contains("<head") || lower.contains("<meta") {
        return GenerationPhase::Scaffolding;
    }
    if progress > 0.95 {
        return GenerationPhase::Finalizing;
    }

    // Fallback to position-based
    if progress < 0.05 {
        GenerationPhase::Analyzing
    } else if progress < 0.15 {
        GenerationPhase::Scaffolding
    } else if progress < 0.40 {
        GenerationPhase::Styling
    } else if progress < 0.85 {
        GenerationPhase::Building
    } else {
        GenerationPhase::Scripting
    }
}

// ─── Cost Calculation ─────────────────────────────────────────────────────

/// Calculate the actual USD cost for a generation based on model and token counts.
///
/// Rates are per million tokens, sourced from provider pricing pages.
pub fn calculate_cost(model_id: &str, input_tokens: usize, output_tokens: usize) -> f64 {
    let (input_rate, output_rate) = model_token_rates(model_id);
    (input_tokens as f64 * input_rate / 1_000_000.0)
        + (output_tokens as f64 * output_rate / 1_000_000.0)
}

/// Estimate cost before generation starts.
pub fn estimate_cost(model_id: &str, est_input: usize, est_output: usize) -> f64 {
    calculate_cost(model_id, est_input, est_output)
}

/// Per-million-token rates (input, output) for a model.
///
/// Pricing verified April 2026 from platform.claude.com and developers.openai.com.
fn model_token_rates(model_id: &str) -> (f64, f64) {
    let m = model_id.to_lowercase();

    // ── CLI providers (subscription-covered, $0) ──
    if m.contains("via codex cli") || m.contains("via cli") {
        return (0.0, 0.0);
    }

    // ── Anthropic ──
    if m.contains("opus") {
        return (5.0, 25.0);
    }
    if m.contains("sonnet") {
        return (3.0, 15.0);
    }
    if m.contains("haiku") {
        return (1.0, 5.0);
    }

    // ── OpenAI (order matters: check specific before generic) ──
    if m.contains("gpt-5-mini") || m.contains("gpt5-mini") {
        return (0.25, 2.0);
    }
    if m.contains("gpt-5") {
        return (1.25, 10.0);
    }
    if m.contains("gpt-4.1-nano") || m.contains("gpt-4-1-nano") {
        return (0.10, 0.40);
    }
    if m.contains("gpt-4.1-mini") || m.contains("gpt-4-1-mini") {
        return (0.40, 1.60);
    }
    if m.contains("gpt-4.1") || m.contains("gpt-4-1") {
        return (2.0, 8.0);
    }
    if m.contains("gpt-4o-mini") {
        return (0.15, 0.60);
    }
    if m.contains("gpt-4o") {
        return (2.50, 10.0);
    }

    // ── Local / Ollama (free) ──
    if m.contains("gemma")
        || m.contains("llama")
        || m.contains("phi")
        || m.contains("mistral")
        || m.contains("qwen")
    {
        return (0.0, 0.0);
    }

    (0.0, 0.0) // unknown models default to free
}

// ─── Quick Governance Scan ────────────────────────────────────────────────

/// Quick governance scan of generated HTML for basic security/accessibility checks.
pub fn quick_governance_scan(html: &str) -> GovernanceStatus {
    let lower = html.to_lowercase();

    // OWASP: check for dangerous patterns
    let has_eval = lower.contains("eval(") || lower.contains("document.write(");
    let has_inner_html_injection = lower.contains(".innerhtml") && lower.contains("user");

    // XSS: check for inline event handlers with user input patterns
    let has_dangerous_handlers = lower.contains("onerror=") || lower.contains("onload=javascript:");

    // ARIA: check for accessibility attributes
    let has_aria = lower.contains("aria-") || lower.contains("role=");

    GovernanceStatus {
        owasp_passed: !has_eval && !has_inner_html_injection,
        xss_clean: !has_dangerous_handlers,
        aria_present: has_aria,
        signed: false, // signing happens at a higher governance layer
    }
}

// ─── Checkpoint ID Generation ─────────────────────────────────────────────

/// Generate a unique checkpoint ID for a build.
pub fn generate_checkpoint_id() -> String {
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    format!("cp_{timestamp:013x}")
}

// ─── Estimated Total Tokens ───────────────────────────────────────────────

/// Default estimated total output tokens for a single-page website build.
/// Real data: Sonnet 4.6 generates ~16,000 output tokens for a 325-line site.
pub const ESTIMATED_TOTAL_TOKENS: usize = 16000;

/// Default estimated input tokens for a fresh build prompt.
pub const ESTIMATED_INPUT_TOKENS: usize = 2500;

/// Default estimated input tokens for an iteration (includes full HTML context).
pub const ESTIMATED_ITERATION_INPUT_TOKENS: usize = 8000;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_phase_analyzing() {
        let phase = detect_phase("", 0, 8000);
        assert!(matches!(phase, GenerationPhase::Analyzing));
    }

    #[test]
    fn test_detect_phase_scaffolding_by_content() {
        let phase = detect_phase("<!DOCTYPE html>\n<head>", 500, 8000);
        assert!(matches!(phase, GenerationPhase::Scaffolding));
    }

    #[test]
    fn test_detect_phase_styling_by_content() {
        let phase = detect_phase(
            "<!DOCTYPE html>\n<head>\n<style>\nbody { font-family: sans-serif; }",
            1500,
            8000,
        );
        assert!(matches!(phase, GenerationPhase::Styling));
    }

    #[test]
    fn test_detect_phase_building_by_content() {
        let accumulated = "<!DOCTYPE html>\n<head>\n<style>...</style>\n</head>\n<body>\n<main>\n<section class=\"hero\">\n<div class=\"container\">";
        let phase = detect_phase(accumulated, 4000, 8000);
        assert!(matches!(phase, GenerationPhase::Building));
    }

    #[test]
    fn test_detect_phase_scripting_by_content() {
        let accumulated =
            "...lots of html...\n<script>\nfunction init() {\n  document.addEventListener('click',";
        let phase = detect_phase(accumulated, 7000, 8000);
        assert!(matches!(phase, GenerationPhase::Scripting));
    }

    #[test]
    fn test_detect_phase_finalizing_by_progress() {
        let phase = detect_phase("closing tags...", 15500, 16000);
        assert!(matches!(phase, GenerationPhase::Finalizing));
    }

    #[test]
    fn test_calculate_cost_sonnet() {
        let cost = calculate_cost("claude-sonnet-4-6", 2000, 8000);
        // input: 2000 * 3.0 / 1M = 0.006
        // output: 8000 * 15.0 / 1M = 0.12
        let expected = 0.006 + 0.12;
        assert!((cost - expected).abs() < 1e-10);
    }

    #[test]
    fn test_calculate_cost_free_model() {
        let cost = calculate_cost("ollama/llama3", 2000, 8000);
        assert_eq!(cost, 0.0);
    }

    #[test]
    fn test_calculate_cost_codex_cli_is_zero() {
        let cost = calculate_cost("gpt-5.4 (via Codex CLI)", 2500, 16000);
        assert_eq!(cost, 0.0);
    }

    #[test]
    fn test_calculate_cost_claude_code_cli_is_zero() {
        let cost = calculate_cost("claude-sonnet-4-6 (via CLI)", 2500, 16000);
        assert_eq!(cost, 0.0);
    }

    #[test]
    fn test_calculate_cost_gpt5_api_is_paid() {
        // GPT-5 via API (not CLI) should have a cost
        let cost = calculate_cost("gpt-5.4", 2000, 8000);
        assert!(cost > 0.0);
    }

    #[test]
    fn test_governance_scan_clean() {
        let html = r#"<html><body><main role="main"><section aria-label="hero">Hello</section></main></body></html>"#;
        let status = quick_governance_scan(html);
        assert!(status.owasp_passed);
        assert!(status.xss_clean);
        assert!(status.aria_present);
    }

    #[test]
    fn test_governance_scan_eval() {
        let html = r#"<script>eval(userInput)</script>"#;
        let status = quick_governance_scan(html);
        assert!(!status.owasp_passed);
    }

    #[test]
    fn test_checkpoint_id_format() {
        let id = generate_checkpoint_id();
        assert!(id.starts_with("cp_"));
        assert!(id.len() > 4);
    }
}

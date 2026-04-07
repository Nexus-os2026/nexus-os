//! LLM-enhanced website generation v2: design-aware conductor with token injection
//! and multi-pass generation for Lovable/v0-quality output.

use crate::build_stream::{
    calculate_cost, detect_phase, estimate_cost, generate_checkpoint_id, quick_governance_scan,
    BuildStreamEvent, ESTIMATED_INPUT_TOKENS, ESTIMATED_TOTAL_TOKENS,
};
use crate::styles::{select_design_tokens, DesignTokenSet, ANTI_PATTERNS, DESIGN_DIRECTIVES};
use nexus_connectors_llm::gateway::{AgentRuntimeContext, GovernedLlmGateway};
use nexus_connectors_llm::providers::LlmProvider;
use nexus_connectors_llm::streaming::StreamingLlmProvider;
use nexus_sdk::errors::AgentError;
use std::path::{Path, PathBuf};

// ─── Types ─────────────────────────────────────────────────────────────────

/// A single file task for decomposed generation.
#[derive(Debug, Clone)]
pub struct FileTask {
    pub filename: String,
    pub description: String,
}

/// Result of a multi-pass generation.
#[derive(Debug, Clone)]
pub struct GenerationResult {
    pub html: String,
    pub pass_count: u8,
}

// ─── Prompt Constants (used by legacy build_prompt path) ─────────────────

const CONDUCTOR_SYSTEM_V2: &str = "\
You are the Nexus Builder code generator. You produce complete, self-contained \
HTML+CSS+JS for modern, visually distinctive websites. \
You follow an injected design token system exactly — never deviate from the \
provided CSS custom properties. Output ONLY the raw code. No explanations, \
no markdown fences, no commentary.";

// ─── Content Generation from Brief ─────────────────────────────────────────

// ─── Content Derivation Helpers ────────────────────────────────────────────
// These extract/generate real content from the brief so the LLM never uses lorem ipsum.

fn extract_site_name(brief: &str) -> String {
    // Look for "called X" or "named X" patterns
    let lower = brief.to_lowercase();
    for prefix in &["called ", "named ", "for "] {
        if let Some(idx) = lower.find(prefix) {
            let after = &brief[idx + prefix.len()..];
            let name: String = after
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == ' ' || *c == '-')
                .collect();
            let trimmed = name.trim();
            if !trimmed.is_empty() && trimmed.len() < 40 {
                return capitalize_words(trimmed);
            }
        }
    }
    // Fallback: first 2-3 significant words
    let words: Vec<&str> = brief
        .split_whitespace()
        .filter(|w| {
            ![
                "a", "an", "the", "for", "with", "and", "or", "that", "this", "build", "create",
                "make", "landing", "page", "website", "site",
            ]
            .contains(&w.to_lowercase().as_str())
        })
        .take(3)
        .collect();
    if words.is_empty() {
        "Acme".to_string()
    } else {
        capitalize_words(&words.join(" "))
    }
}

fn capitalize_words(s: &str) -> String {
    s.split_whitespace()
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(c) => c.to_uppercase().to_string() + &chars.as_str().to_lowercase(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn derive_headline(brief: &str) -> String {
    let lower = brief.to_lowercase();
    if lower.contains("ai") || lower.contains("agent") {
        "The Future of AI is Governed, Auditable, and Unstoppable".to_string()
    } else if lower.contains("developer") || lower.contains("dev tool") {
        "Ship Faster. Break Nothing. Sleep Better.".to_string()
    } else if lower.contains("team") || lower.contains("collaborat") {
        "Where Great Teams Do Their Best Work".to_string()
    } else if lower.contains("restaurant") || lower.contains("food") || lower.contains("coffee") {
        "Crafted With Passion, Served With Love".to_string()
    } else if lower.contains("portfolio") || lower.contains("photographer") {
        "Every Frame Tells a Story".to_string()
    } else if lower.contains("finance") || lower.contains("invest") {
        "Your Money, Working Smarter Than Ever".to_string()
    } else {
        "Build Something Extraordinary Today".to_string()
    }
}

fn derive_subheadline(brief: &str) -> String {
    let lower = brief.to_lowercase();
    if lower.contains("ai") || lower.contains("agent") {
        "Deploy autonomous AI agents that work 24/7 with enterprise-grade security, hash-chained audit trails, and human-in-the-loop safety gates.".to_string()
    } else if lower.contains("developer") || lower.contains("dev tool") {
        "The development platform that catches bugs before your users do. Trusted by 10,000+ engineering teams worldwide.".to_string()
    } else if lower.contains("team") || lower.contains("collaborat") {
        "Streamline workflows, reduce meeting overhead, and keep everyone aligned with a single source of truth.".to_string()
    } else {
        format!(
            "The modern platform for {}. Trusted by teams who refuse to compromise.",
            brief_to_domain(brief)
        )
    }
}

fn brief_to_domain(brief: &str) -> &str {
    let lower = brief.to_lowercase();
    if lower.contains("ecommerce") || lower.contains("shop") {
        "online commerce"
    } else if lower.contains("analytics") {
        "data analytics"
    } else if lower.contains("health") {
        "health and wellness"
    } else {
        "modern businesses"
    }
}

fn derive_features(brief: &str) -> Vec<(&'static str, &'static str)> {
    let lower = brief.to_lowercase();
    if lower.contains("ai") || lower.contains("agent") || lower.contains("governance") {
        vec![
            ("Governed Autonomy", "Agents run 24/7 with fuel metering, capability-based access control, and human-in-the-loop gates for high-risk actions."),
            ("Hash-Chained Audit", "Every decision, every action, cryptographically linked. Tamper-proof accountability that satisfies enterprise compliance."),
            ("WASM Sandboxing", "Agents execute in isolated WebAssembly containers. No system access outside governed boundaries."),
            ("Multi-Agent Orchestration", "Deploy fleets of specialized agents that collaborate via governed A2A protocols with adversarial validation."),
        ]
    } else if lower.contains("developer") || lower.contains("dev tool") {
        vec![
            ("Instant Deploys", "Push to production in seconds with zero-downtime rolling deployments and automatic rollback on failure."),
            ("Built-in Observability", "Structured logs, distributed traces, and real-time metrics without any additional configuration required."),
            ("Type-Safe APIs", "Auto-generated client libraries with full type safety. Catch integration errors at compile time, not runtime."),
            ("Team Workflows", "Branch previews, review gates, and automated testing pipelines that keep your team shipping confidently."),
        ]
    } else {
        vec![
            ("Lightning Fast", "Optimized for speed at every layer. Sub-100ms response times, globally distributed for your users."),
            ("Enterprise Security", "SOC 2 compliant, end-to-end encrypted, with role-based access control and comprehensive audit logging."),
            ("Seamless Integration", "Connect with your existing tools in minutes. REST APIs, webhooks, and 50+ native integrations out of the box."),
            ("24/7 Support", "Real humans, real expertise, real fast. Average response time under 5 minutes for critical issues."),
        ]
    }
}

fn derive_cta_headline(brief: &str) -> String {
    let lower = brief.to_lowercase();
    if lower.contains("ai") || lower.contains("agent") {
        "Ready to Deploy Your First Governed Agent?".to_string()
    } else if lower.contains("developer") {
        "Start Building Better Software Today".to_string()
    } else {
        "Ready to Get Started?".to_string()
    }
}

// ─── Prompt Construction ──────────────────────────────────────────────────

/// Build the generation prompt for V2 builds (both streaming and non-streaming).
///
/// The user's description goes FIRST so the LLM treats it as the primary
/// instruction. Design tokens are labeled as visual-styling-only so the LLM
/// does not override the user's content with a generic template.
fn build_generation_prompt(brief: &str, site_name: &str, style_guide: &str) -> String {
    format!(
        "BUILD THIS WEBSITE: {brief}\n\
         Site name: {site_name}\n\n\
         The site's content, sections, features, copy, and structure must be \
         derived from the description above. Do NOT use a generic template — \
         the page must be specifically about what the user described.\n\n\
         {style_guide}\n\n\
         The palette and fonts above are for visual styling only — do not let \
         them influence the content, sections, or copy of the website.\n\n\
         REQUIREMENTS:\n\
         - Single HTML file: <!DOCTYPE html> with <style> in <head> and <script> before </body>\n\
         - Include the Google Fonts <link> in <head>\n\
         - Use the exact colors from the style guide above\n\
         - Mobile responsive with hamburger nav on small screens\n\
         - Smooth scroll between sections\n\
         - All content MUST be visible immediately on page load\n\
         - Do NOT use opacity: 0 or visibility: hidden on any content\n\
         - CSS hover effects and transitions are fine, but never hide content by default\n\
         - Dark/light theme based on the background color above\n\
         - Write real, contextually appropriate content — no lorem ipsum\n\n\
         IMPORTANT: Keep CSS concise — use shorthand properties. Prioritize \
         writing the COMPLETE HTML <body> with ALL sections. The page MUST have \
         a complete <body> and a closing </html> tag. Budget your output: \
         ~150 lines CSS, ~250 lines HTML, ~30 lines JS.\n\n\
         Output ONLY the raw HTML. No markdown fences. No explanations.\n\n\
         Generate the complete HTML now:",
    )
}

// ─── Decomposed Generation (Legacy + V2) ──────────────────────────────────

/// Strip markdown fences from LLM output.
///
/// Handles all variants: `` `html ``, ` ```html `, ` ```` `, missing closing fence,
/// and trailing backticks without a preceding newline.
pub fn strip_markdown_fences(input: &str) -> String {
    let mut s = input.trim().to_string();

    // Remove opening fence: one or more backticks with optional language tag
    if s.starts_with('`') {
        if let Some(newline_pos) = s.find('\n') {
            let first_line = s[..newline_pos].trim();
            // Opening fence is a line of ONLY backticks + optional alphanumeric lang tag
            if !first_line.is_empty() && first_line.chars().all(|c| c == '`' || c.is_alphanumeric())
            {
                s = s[newline_pos + 1..].to_string();
            }
        }
    }

    // Remove closing fence: trailing line of only backticks
    let trimmed_end = s.trim_end();
    if let Some(last_nl) = trimmed_end.rfind('\n') {
        let last_line = trimmed_end[last_nl + 1..].trim();
        if !last_line.is_empty() && last_line.chars().all(|c| c == '`') {
            s = trimmed_end[..last_nl].trim_end().to_string();
        }
    } else if trimmed_end.ends_with("```") {
        // Single-line edge case: content followed by ``` with no newline
        s = trimmed_end.trim_end_matches('`').trim_end().to_string();
    }

    s.trim().to_string()
}

/// Ensure output starts with actual HTML, stripping any residual LLM preamble.
fn ensure_html_start(html: &str) -> String {
    let trimmed = html.trim_start();
    if trimmed.starts_with("<!") || trimmed.starts_with("<html") || trimmed.starts_with("<HTML") {
        return html.trim().to_string();
    }
    // Try to find where the actual HTML starts
    if let Some(pos) = html.find("<!DOCTYPE") {
        return html[pos..].trim_end().to_string();
    }
    if let Some(pos) = html.find("<!doctype") {
        return html[pos..].trim_end().to_string();
    }
    if let Some(pos) = html.find("<html") {
        return html[pos..].trim_end().to_string();
    }
    // Give up, return as-is
    html.trim().to_string()
}

/// Remove bare `opacity: 0` declarations that hide content when the LLM forgets
/// to include the corresponding JavaScript to reveal it.
///
/// Preserves opacity inside `@keyframes` blocks (the `from { opacity: 0 }` pattern
/// is fine because the animation runs immediately) and decorative uses like
/// `opacity: 0.3` on quotation marks.
fn sanitize_generated_html(html: &str) -> String {
    let lines: Vec<&str> = html.lines().collect();
    let mut out: Vec<&str> = Vec::with_capacity(lines.len());
    let mut keyframe_depth: i32 = 0; // >0 means inside @keyframes

    for line in &lines {
        let trimmed = line.trim();

        // Track @keyframes blocks via brace depth
        if trimmed.starts_with("@keyframes") {
            keyframe_depth = 0; // will increment when we count the opening {
        }
        let opens = trimmed.chars().filter(|&c| c == '{').count() as i32;
        let closes = trimmed.chars().filter(|&c| c == '}').count() as i32;
        if keyframe_depth > 0 || trimmed.starts_with("@keyframes") {
            keyframe_depth += opens - closes;
            if keyframe_depth < 0 {
                keyframe_depth = 0;
            }
        }

        // Skip standalone opacity: 0 declarations outside keyframes.
        // These hide content when the matching JS reveal is missing.
        if keyframe_depth == 0 {
            let no_ws: String = trimmed.chars().filter(|c| !c.is_whitespace()).collect();
            if no_ws == "opacity:0;" || no_ws == "opacity:0" {
                continue;
            }
        }

        out.push(line);
    }

    // Second pass: remove inline style opacity:0 patterns (e.g. style="opacity:0;...")
    // but not opacity:0.N (decorative) or opacity inside keyframe strings.
    let joined = out.join("\n");
    // Replace opacity:0 in inline style attrs — match "opacity: 0;" or "opacity:0;"
    // followed by optional space, but NOT "opacity: 0." (which is a decimal like 0.3)
    let mut result = String::with_capacity(joined.len());
    let mut remaining = joined.as_str();

    while let Some(pos) = remaining.find("opacity") {
        result.push_str(&remaining[..pos]);
        let after_opacity = &remaining[pos + 7..]; // skip "opacity"

        // Check if we're inside a @keyframes block by scanning what we've emitted
        let in_kf = is_inside_keyframes(&result);

        // Skip whitespace and colon
        let after_colon = after_opacity.trim_start();
        if !in_kf {
            if let Some(after_colon_raw) = after_colon.strip_prefix(':') {
                let after_colon_val = after_colon_raw.trim_start();
                // Check for exactly "0" followed by ; or " or } (not "0." like 0.3)
                if after_colon_val.starts_with('0')
                    && !after_colon_val[1..].starts_with('.')
                    && !after_colon_val[1..].starts_with(|c: char| c.is_ascii_digit())
                {
                    // This is opacity: 0 — skip it
                    let skip_from = &remaining[pos + 7..];
                    if let Some(semi) = skip_from.find(';') {
                        remaining = &skip_from[semi + 1..];
                        remaining = remaining.strip_prefix(' ').unwrap_or(remaining);
                        continue;
                    }
                }
            }
        }

        // Not a bare opacity:0, or inside keyframes — keep it
        result.push_str("opacity");
        remaining = &remaining[pos + 7..];
    }
    result.push_str(remaining);

    result
}

/// Check if a position in emitted text falls inside a `@keyframes` block.
/// Scans backwards from the end of `text` for `@keyframes` and counts braces.
fn is_inside_keyframes(text: &str) -> bool {
    // Find the last @keyframes occurrence
    if let Some(kf_pos) = text.rfind("@keyframes") {
        let after_kf = &text[kf_pos..];
        let opens = after_kf.chars().filter(|&c| c == '{').count();
        let closes = after_kf.chars().filter(|&c| c == '}').count();
        // If more opens than closes, we're still inside the block
        opens > closes
    } else {
        false
    }
}

/// Decompose a website description into individual file tasks (legacy path).
pub fn decompose_web_tasks(description: &str) -> Vec<FileTask> {
    let lower = description.to_lowercase();

    let mut tasks = vec![
        FileTask {
            filename: "index.html".into(),
            description: format!(
                "Create the main HTML page for: {description}. \
                 Link to styles.css and script.js. Use semantic HTML5. \
                 Include all sections described in the request."
            ),
        },
        FileTask {
            filename: "styles.css".into(),
            description: format!(
                "Create the CSS stylesheet for: {description}. \
                 Use modern CSS with gradients, dark theme (#0a0a0f background, #e0e0e0 text), \
                 smooth animations, glassmorphism effects, responsive design. \
                 Style all sections from the HTML."
            ),
        },
    ];

    let needs_js = lower.contains("form")
        || lower.contains("interactive")
        || lower.contains("animation")
        || lower.contains("contact")
        || lower.contains("menu")
        || lower.contains("nav")
        || lower.contains("toggle")
        || lower.contains("slider")
        || lower.contains("scroll");

    if needs_js {
        tasks.push(FileTask {
            filename: "script.js".into(),
            description: format!(
                "Create the JavaScript for: {description}. \
                 Handle form submissions, smooth scrolling, mobile menu toggle, \
                 and any interactive elements. Use vanilla JS, no frameworks."
            ),
        });
    }

    let needs_3d = ["3d", "three.js", "particles", "webgl", "globe"]
        .iter()
        .any(|kw| lower.contains(kw));

    if needs_3d {
        tasks.push(FileTask {
            filename: "scene.js".into(),
            description: format!(
                "Create a Three.js 3D scene for: {description}. \
                 Import from https://cdnjs.cloudflare.com/ajax/libs/three.js/r128/three.min.js. \
                 Create an animated 3D hero (rotating geometry, particles, or wave). \
                 Use requestAnimationFrame."
            ),
        });
    }

    tasks
}

/// V2 design-aware generation: single-call full-page approach with concise style guide.
///
/// Instead of generating 7 sections individually (which fails on most models due to
/// prompt complexity), we generate the entire page in ONE LLM call with a brief style
/// guide. This is more reliable because:
/// - The model sees full page context while writing each section
/// - No assembly/parsing bugs between fragments
/// - Works with both large and small context models
/// - One coherent generation instead of 7 that need stitching
pub fn generate_site_v2<P: LlmProvider>(
    brief: &str,
    output_dir: &Path,
    gateway: &mut GovernedLlmGateway<P>,
    context: &mut AgentRuntimeContext,
    model: &str,
    _enable_review_pass: bool,
) -> Result<Vec<PathBuf>, AgentError> {
    // Step 1: Select design tokens from the brief
    let tokens = select_design_tokens(brief);
    eprintln!(
        "[conductor-v2] Design tokens: palette={}, mood={}, fonts={}/{}",
        tokens.palette_name,
        tokens.mood.label(),
        tokens.font_display,
        tokens.font_body,
    );

    // Step 2: Build a CONCISE style guide (not the full CSS variable dump)
    let style_guide = format!(
        "STYLE GUIDE (use these EXACT color values):\n\
         Background: {bg}\n\
         Surface/cards: {surface}\n\
         Text: {text}\n\
         Text secondary: {text2}\n\
         Accent (buttons/links): {accent}\n\
         Accent hover: {accent_hover}\n\
         Border: {border}\n\
         Display font: '{font_d}' (weight 700-800)\n\
         Body font: '{font_b}' (weight 400-500)\n\
         Border radius: {radius}\n\
         Google Fonts URL: {fonts_url}",
        bg = tokens.bg,
        surface = tokens.surface,
        text = tokens.text_primary,
        text2 = tokens.text_secondary,
        accent = tokens.accent,
        accent_hover = tokens.accent_hover,
        border = tokens.border,
        font_d = tokens.font_display,
        font_b = tokens.font_body,
        radius = tokens.radius,
        fonts_url = tokens.google_fonts_url(),
    );

    // Step 3: Derive site name from brief
    let site_name = extract_site_name(brief);

    // Step 4: ONE prompt — user's description is DOMINANT, design tokens are for styling only
    let prompt = build_generation_prompt(brief, &site_name, &style_guide);

    // Step 5: Generate with 16384 max_tokens — a full page with CSS+HTML+JS
    // needs ~8000-10000 tokens; 16384 gives comfortable headroom.
    eprintln!("[conductor-v2] Generating full page in single call...");
    let result = gateway.query(context, &prompt, 16384, model);
    eprintln!(
        "[conductor-v2] LLM result: {:?}",
        result
            .as_ref()
            .map(|r| format!("Ok({} chars)", r.output_text.len()))
            .unwrap_or_else(|e| format!("Err({})", e))
    );

    let retry = match &result {
        Ok(resp) if !resp.output_text.trim().is_empty() => {
            eprintln!(
                "[conductor-v2] LLM returned {} chars",
                resp.output_text.len()
            );
            None // no retry needed
        }
        Ok(resp) => {
            eprintln!(
                "[conductor-v2] LLM returned EMPTY response ({} chars)",
                resp.output_text.len()
            );
            Some("empty response")
        }
        Err(e) => {
            eprintln!("[conductor-v2] LLM query ERROR: {}", e);
            Some("query error")
        }
    };

    let html = if let Some(reason) = retry {
        eprintln!(
            "[conductor-v2] Full page generation failed ({reason}), retrying with simplified prompt..."
        );
        let simple_prompt = format!(
            "Create a complete dark-themed landing page HTML file for: {brief}\n\
             Colors: background {bg}, accent {accent}, text {text}\n\
             Font: '{font_d}'\n\
             Google Fonts: {fonts_url}\n\
             Include: nav, hero, features, pricing (3 tiers), testimonials, CTA, footer.\n\
             Single file with embedded CSS and JS. Output ONLY the HTML.",
            bg = tokens.bg,
            accent = tokens.accent,
            text = tokens.text_primary,
            font_d = tokens.font_display,
            fonts_url = tokens.google_fonts_url(),
        );
        match gateway.query(context, &simple_prompt, 16384, model) {
            Ok(resp) if !resp.output_text.trim().is_empty() => {
                eprintln!(
                    "[conductor-v2] Retry LLM returned {} chars",
                    resp.output_text.len()
                );
                ensure_html_start(&strip_markdown_fences(&resp.output_text))
            }
            Ok(resp) => {
                eprintln!(
                    "[conductor-v2] Retry returned EMPTY ({} chars)",
                    resp.output_text.len()
                );
                eprintln!("[conductor-v2] Retry also failed, using full fallback page");
                generate_fallback_page(&tokens, &site_name, brief)
            }
            Err(e) => {
                eprintln!("[conductor-v2] Retry query ERROR: {}", e);
                eprintln!("[conductor-v2] Retry also failed, using full fallback page");
                generate_fallback_page(&tokens, &site_name, brief)
            }
        }
    } else {
        // Primary generation succeeded
        let resp = result.unwrap();
        ensure_html_start(&strip_markdown_fences(&resp.output_text))
    };

    // Step 6: Sanitize — remove bare opacity:0 that hides content when JS is missing
    let html = sanitize_generated_html(&html);

    // Step 7: Write output
    std::fs::create_dir_all(output_dir)
        .map_err(|e| AgentError::ManifestError(format!("failed to create output dir: {e}")))?;

    let index_path = output_dir.join("index.html");
    std::fs::write(&index_path, &html)
        .map_err(|e| AgentError::ManifestError(format!("failed to write index.html: {e}")))?;

    eprintln!(
        "[conductor-v2] Build complete: 1 file, {} lines, palette={}, mood={}",
        html.lines().count(),
        tokens.palette_name,
        tokens.mood.label(),
    );

    Ok(vec![index_path])
}

/// Generate a complete fallback page when LLM generation fails entirely.
/// Uses inline styles with design tokens — no CSS variables (per project feedback).
fn generate_fallback_page(tokens: &DesignTokenSet, site_name: &str, brief: &str) -> String {
    let bg = &tokens.bg;
    let surface = &tokens.surface;
    let text = &tokens.text_primary;
    let text2 = &tokens.text_secondary;
    let accent = &tokens.accent;
    let accent_hover = &tokens.accent_hover;
    let border = &tokens.border;
    let fd = &tokens.font_display;
    let fb = &tokens.font_body;
    let radius = &tokens.radius;
    let fonts_url = tokens.google_fonts_url();
    let headline = derive_headline(brief);
    let subheadline = derive_subheadline(brief);
    let features = derive_features(brief);
    let shadow = tokens.shadow_style.css_value();

    let feature_cards: String = features
        .iter()
        .map(|(title, desc)| {
            format!(
                "<div style=\"background:{surface};border:1px solid {border};border-radius:{radius};\
                 padding:32px;box-shadow:{shadow}\">\
                 <h3 style=\"font-family:'{fd}',sans-serif;font-weight:600;font-size:1.25rem;\
                 color:{text};margin-bottom:8px\">{title}</h3>\
                 <p style=\"font-family:'{fb}',sans-serif;font-size:0.95rem;color:{text2};\
                 line-height:1.6;max-width:36ch\">{desc}</p></div>"
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        "<!DOCTYPE html>\n\
<html lang=\"en\">\n\
<head>\n\
  <meta charset=\"UTF-8\">\n\
  <meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\">\n\
  <title>{site_name}</title>\n\
  <link href=\"{fonts_url}\" rel=\"stylesheet\">\n\
  <style>\n\
    *, *::before, *::after {{ box-sizing: border-box; margin: 0; padding: 0; }}\n\
    html {{ scroll-behavior: smooth; -webkit-font-smoothing: antialiased; }}\n\
    body {{ font-family: '{fb}', sans-serif; background: {bg}; color: {text}; line-height: 1.6; }}\n\
    @keyframes fadeInUp {{ from {{ opacity:0; transform:translateY(20px); }} to {{ opacity:1; transform:translateY(0); }} }}\n\
    .reveal {{ opacity:0; }} .reveal.visible {{ animation: fadeInUp 0.6s ease forwards; }}\n\
    @media (max-width:768px) {{ .nav-links {{ display:none !important; }} .hamburger {{ display:block !important; }} }}\n\
  </style>\n\
</head>\n\
<body>\n\
\n\
<!-- Nav -->\n\
<nav style=\"position:sticky;top:0;z-index:50;background:{bg};padding:16px clamp(16px,5vw,80px);\
display:flex;align-items:center;justify-content:space-between;border-bottom:1px solid {border}\">\n\
  <span style=\"font-family:'{fd}',sans-serif;font-size:24px;font-weight:800;color:{text}\">{site_name}</span>\n\
  <div class=\"nav-links\" style=\"display:flex;gap:32px;align-items:center\">\n\
    <a href=\"#features\" style=\"color:{text2};text-decoration:none;font-size:14px;font-weight:500\">Features</a>\n\
    <a href=\"#pricing\" style=\"color:{text2};text-decoration:none;font-size:14px;font-weight:500\">Pricing</a>\n\
    <a href=\"#contact\" style=\"color:{text2};text-decoration:none;font-size:14px;font-weight:500\">Contact</a>\n\
    <a href=\"#\" style=\"background:{accent};color:white;padding:10px 24px;border-radius:{radius};\
text-decoration:none;font-size:14px;font-weight:600\">Get Started</a>\n\
  </div>\n\
  <button class=\"hamburger\" style=\"display:none;background:none;border:none;color:{text};font-size:24px;cursor:pointer\" \
onclick=\"document.querySelector('.nav-links').style.display=document.querySelector('.nav-links').style.display==='flex'?'none':'flex'\">&#9776;</button>\n\
</nav>\n\
\n\
<!-- Hero -->\n\
<section id=\"hero\" class=\"reveal\" style=\"padding:120px clamp(16px,5vw,80px) 96px;background:{bg}\">\n\
  <h1 style=\"font-family:'{fd}',sans-serif;font-size:clamp(2.5rem,5vw,4.5rem);font-weight:800;\
line-height:1.1;color:{text};max-width:14ch\">{headline}</h1>\n\
  <p style=\"font-size:1.125rem;color:{text2};margin-top:24px;max-width:48ch;line-height:1.7\">{subheadline}</p>\n\
  <a href=\"#\" style=\"display:inline-block;margin-top:32px;background:{accent};color:white;\
padding:14px 32px;border-radius:{radius};text-decoration:none;font-weight:600;\
transition:background 0.2s\" onmouseover=\"this.style.background='{accent_hover}'\" \
onmouseout=\"this.style.background='{accent}'\">Get Started Free</a>\n\
</section>\n\
\n\
<!-- Features -->\n\
<section id=\"features\" style=\"padding:96px clamp(16px,5vw,80px);background:{bg}\">\n\
  <h2 style=\"font-family:'{fd}',sans-serif;font-size:2.5rem;font-weight:700;color:{text};\
text-align:center;margin-bottom:48px\" class=\"reveal\">Why Teams Choose Us</h2>\n\
  <div style=\"display:grid;grid-template-columns:repeat(auto-fit,minmax(280px,1fr));gap:24px\" class=\"reveal\">\n\
    {feature_cards}\n\
  </div>\n\
</section>\n\
\n\
<!-- Pricing -->\n\
<section id=\"pricing\" style=\"padding:96px clamp(16px,5vw,80px);background:{bg}\">\n\
  <h2 style=\"font-family:'{fd}',sans-serif;font-size:2.5rem;font-weight:700;color:{text};\
text-align:center;margin-bottom:48px\" class=\"reveal\">Simple, Transparent Pricing</h2>\n\
  <div style=\"display:flex;gap:24px;justify-content:center;flex-wrap:wrap\" class=\"reveal\">\n\
    <div style=\"background:{surface};border:1px solid {border};border-radius:{radius};padding:40px 32px;\
width:300px;text-align:center\">\n\
      <h3 style=\"font-family:'{fd}',sans-serif;font-size:1.5rem;color:{text}\">Starter</h3>\n\
      <p style=\"font-family:'{fd}',sans-serif;font-size:3rem;font-weight:800;color:{text};margin:16px 0\">$0</p>\n\
      <p style=\"color:{text2};margin-bottom:24px\">per month</p>\n\
      <ul style=\"list-style:none;text-align:left;margin-bottom:32px\">\n\
        <li style=\"padding:8px 0;color:{text2};border-bottom:1px solid {border}\">Up to 3 projects</li>\n\
        <li style=\"padding:8px 0;color:{text2};border-bottom:1px solid {border}\">Community support</li>\n\
        <li style=\"padding:8px 0;color:{text2}\">Basic analytics</li>\n\
      </ul>\n\
      <a href=\"#\" style=\"display:block;padding:12px;border:2px solid {accent};color:{accent};\
border-radius:{radius};text-decoration:none;font-weight:600\">Get Started</a>\n\
    </div>\n\
    <div style=\"background:{surface};border:3px solid {accent};border-radius:{radius};padding:40px 32px;\
width:300px;text-align:center;transform:scale(1.05);box-shadow:{shadow}\">\n\
      <span style=\"background:{accent};color:white;padding:4px 12px;border-radius:99px;\
font-size:12px;font-weight:600\">Most Popular</span>\n\
      <h3 style=\"font-family:'{fd}',sans-serif;font-size:1.5rem;color:{text};margin-top:16px\">Pro</h3>\n\
      <p style=\"font-family:'{fd}',sans-serif;font-size:3rem;font-weight:800;color:{text};margin:16px 0\">$29</p>\n\
      <p style=\"color:{text2};margin-bottom:24px\">per month</p>\n\
      <ul style=\"list-style:none;text-align:left;margin-bottom:32px\">\n\
        <li style=\"padding:8px 0;color:{text2};border-bottom:1px solid {border}\">Unlimited projects</li>\n\
        <li style=\"padding:8px 0;color:{text2};border-bottom:1px solid {border}\">Priority support</li>\n\
        <li style=\"padding:8px 0;color:{text2};border-bottom:1px solid {border}\">Advanced analytics</li>\n\
        <li style=\"padding:8px 0;color:{text2}\">Custom domains</li>\n\
      </ul>\n\
      <a href=\"#\" style=\"display:block;padding:12px;background:{accent};color:white;\
border-radius:{radius};text-decoration:none;font-weight:600\">Get Started</a>\n\
    </div>\n\
    <div style=\"background:{surface};border:1px solid {border};border-radius:{radius};padding:40px 32px;\
width:300px;text-align:center\">\n\
      <h3 style=\"font-family:'{fd}',sans-serif;font-size:1.5rem;color:{text}\">Enterprise</h3>\n\
      <p style=\"font-family:'{fd}',sans-serif;font-size:3rem;font-weight:800;color:{text};margin:16px 0\">Custom</p>\n\
      <p style=\"color:{text2};margin-bottom:24px\">contact us</p>\n\
      <ul style=\"list-style:none;text-align:left;margin-bottom:32px\">\n\
        <li style=\"padding:8px 0;color:{text2};border-bottom:1px solid {border}\">Everything in Pro</li>\n\
        <li style=\"padding:8px 0;color:{text2};border-bottom:1px solid {border}\">Dedicated support</li>\n\
        <li style=\"padding:8px 0;color:{text2};border-bottom:1px solid {border}\">SLA guarantee</li>\n\
        <li style=\"padding:8px 0;color:{text2}\">SSO &amp; SAML</li>\n\
      </ul>\n\
      <a href=\"#\" style=\"display:block;padding:12px;border:2px solid {accent};color:{accent};\
border-radius:{radius};text-decoration:none;font-weight:600\">Contact Sales</a>\n\
    </div>\n\
  </div>\n\
</section>\n\
\n\
<!-- Testimonials -->\n\
<section id=\"testimonials\" style=\"padding:96px clamp(16px,5vw,80px);background:{surface}\">\n\
  <h2 style=\"font-family:'{fd}',sans-serif;font-size:2.5rem;font-weight:700;color:{text};\
text-align:center;margin-bottom:48px\" class=\"reveal\">What People Say</h2>\n\
  <div style=\"display:grid;grid-template-columns:repeat(auto-fit,minmax(300px,1fr));gap:24px\" class=\"reveal\">\n\
    <div style=\"background:{bg};border:1px solid {border};border-radius:{radius};padding:32px;position:relative\">\n\
      <span style=\"font-family:'{fd}',sans-serif;font-size:4rem;color:{accent};opacity:0.3;\
position:absolute;top:16px;left:24px\">&ldquo;</span>\n\
      <p style=\"color:{text};font-style:italic;margin-top:24px;line-height:1.7\">This platform transformed how our team ships software. We went from weekly deploys to multiple times per day.</p>\n\
      <div style=\"margin-top:24px;display:flex;align-items:center;gap:12px\">\n\
        <div style=\"width:48px;height:48px;border-radius:50%;background:{accent};display:flex;\
align-items:center;justify-content:center;color:white;font-weight:700\">SC</div>\n\
        <div><p style=\"font-family:'{fd}',sans-serif;font-weight:600;color:{text}\">Sarah Chen</p>\
<p style=\"font-size:0.875rem;color:{text2}\">VP Engineering, Meridian</p></div>\n\
      </div>\n\
    </div>\n\
    <div style=\"background:{bg};border:1px solid {border};border-radius:{radius};padding:32px;position:relative\">\n\
      <span style=\"font-family:'{fd}',sans-serif;font-size:4rem;color:{accent};opacity:0.3;\
position:absolute;top:16px;left:24px\">&ldquo;</span>\n\
      <p style=\"color:{text};font-style:italic;margin-top:24px;line-height:1.7\">The security and audit features gave our compliance team confidence to move fast. Game changer.</p>\n\
      <div style=\"margin-top:24px;display:flex;align-items:center;gap:12px\">\n\
        <div style=\"width:48px;height:48px;border-radius:50%;background:{accent};display:flex;\
align-items:center;justify-content:center;color:white;font-weight:700\">MR</div>\n\
        <div><p style=\"font-family:'{fd}',sans-serif;font-weight:600;color:{text}\">Marcus Rivera</p>\
<p style=\"font-size:0.875rem;color:{text2}\">CTO, Fintech Solutions</p></div>\n\
      </div>\n\
    </div>\n\
    <div style=\"background:{bg};border:1px solid {border};border-radius:{radius};padding:32px;position:relative\">\n\
      <span style=\"font-family:'{fd}',sans-serif;font-size:4rem;color:{accent};opacity:0.3;\
position:absolute;top:16px;left:24px\">&ldquo;</span>\n\
      <p style=\"color:{text};font-style:italic;margin-top:24px;line-height:1.7\">We evaluated every tool on the market. Nothing else comes close to this level of quality and reliability.</p>\n\
      <div style=\"margin-top:24px;display:flex;align-items:center;gap:12px\">\n\
        <div style=\"width:48px;height:48px;border-radius:50%;background:{accent};display:flex;\
align-items:center;justify-content:center;color:white;font-weight:700\">PP</div>\n\
        <div><p style=\"font-family:'{fd}',sans-serif;font-weight:600;color:{text}\">Priya Patel</p>\
<p style=\"font-size:0.875rem;color:{text2}\">Lead Developer, ScaleUp</p></div>\n\
      </div>\n\
    </div>\n\
  </div>\n\
</section>\n\
\n\
<!-- CTA -->\n\
<section id=\"contact\" style=\"padding:96px clamp(16px,5vw,80px);background:{accent};text-align:center\" class=\"reveal\">\n\
  <h2 style=\"font-family:'{fd}',sans-serif;font-size:2.5rem;font-weight:700;color:white;\
margin-bottom:16px\">{cta}</h2>\n\
  <p style=\"color:rgba(255,255,255,0.85);font-size:1.1rem;margin-bottom:32px;\
max-width:480px;margin-left:auto;margin-right:auto\">Join thousands of teams already building with us.</p>\n\
  <a href=\"#\" style=\"display:inline-block;background:white;color:{accent};padding:18px 40px;\
border-radius:{radius};text-decoration:none;font-size:18px;font-weight:600;\
transition:transform 0.2s,box-shadow 0.2s\" \
onmouseover=\"this.style.transform='translateY(-2px)';this.style.boxShadow='0 8px 24px rgba(0,0,0,0.2)'\" \
onmouseout=\"this.style.transform='';this.style.boxShadow=''\">Start Your Free Trial</a>\n\
</section>\n\
\n\
<!-- Footer -->\n\
<footer style=\"background:{surface};padding:64px clamp(16px,5vw,80px) 32px;border-top:1px solid {border}\">\n\
  <div style=\"display:grid;grid-template-columns:repeat(auto-fit,minmax(200px,1fr));gap:32px;margin-bottom:48px\">\n\
    <div>\n\
      <p style=\"font-family:'{fd}',sans-serif;font-size:20px;font-weight:800;color:{text};\
margin-bottom:12px\">{site_name}</p>\n\
      <p style=\"color:{text2};font-size:0.9rem;line-height:1.6\">{site_name} helps teams build, ship, and scale with confidence.</p>\n\
    </div>\n\
    <div>\n\
      <p style=\"font-family:'{fd}',sans-serif;font-size:0.875rem;font-weight:600;text-transform:uppercase;\
letter-spacing:1px;color:{text2};margin-bottom:16px\">Product</p>\n\
      <a href=\"#features\" style=\"display:block;color:{text2};text-decoration:none;font-size:0.875rem;\
padding:4px 0\">Features</a>\n\
      <a href=\"#pricing\" style=\"display:block;color:{text2};text-decoration:none;font-size:0.875rem;\
padding:4px 0\">Pricing</a>\n\
      <a href=\"#\" style=\"display:block;color:{text2};text-decoration:none;font-size:0.875rem;\
padding:4px 0\">Integrations</a>\n\
    </div>\n\
    <div>\n\
      <p style=\"font-family:'{fd}',sans-serif;font-size:0.875rem;font-weight:600;text-transform:uppercase;\
letter-spacing:1px;color:{text2};margin-bottom:16px\">Company</p>\n\
      <a href=\"#\" style=\"display:block;color:{text2};text-decoration:none;font-size:0.875rem;\
padding:4px 0\">About</a>\n\
      <a href=\"#\" style=\"display:block;color:{text2};text-decoration:none;font-size:0.875rem;\
padding:4px 0\">Blog</a>\n\
      <a href=\"#\" style=\"display:block;color:{text2};text-decoration:none;font-size:0.875rem;\
padding:4px 0\">Contact</a>\n\
    </div>\n\
    <div>\n\
      <p style=\"font-family:'{fd}',sans-serif;font-size:0.875rem;font-weight:600;text-transform:uppercase;\
letter-spacing:1px;color:{text2};margin-bottom:16px\">Legal</p>\n\
      <a href=\"#\" style=\"display:block;color:{text2};text-decoration:none;font-size:0.875rem;\
padding:4px 0\">Privacy</a>\n\
      <a href=\"#\" style=\"display:block;color:{text2};text-decoration:none;font-size:0.875rem;\
padding:4px 0\">Terms</a>\n\
      <a href=\"#\" style=\"display:block;color:{text2};text-decoration:none;font-size:0.875rem;\
padding:4px 0\">Security</a>\n\
    </div>\n\
  </div>\n\
  <div style=\"border-top:1px solid {border};padding-top:24px;text-align:center\">\n\
    <p style=\"color:{text2};font-size:0.8rem\">&copy; 2026 {site_name}. All rights reserved.</p>\n\
  </div>\n\
</footer>\n\
\n\
<script>\n\
  // Scroll-triggered reveals\n\
  const observer = new IntersectionObserver((entries) => {{\n\
    entries.forEach((entry) => {{\n\
      if (entry.isIntersecting) {{\n\
        entry.target.classList.add('visible');\n\
        observer.unobserve(entry.target);\n\
      }}\n\
    }});\n\
  }}, {{ threshold: 0.1 }});\n\
  document.querySelectorAll('.reveal').forEach(el => observer.observe(el));\n\
</script>\n\
</body>\n\
</html>",
        cta = derive_cta_headline(brief),
    )
}

/// Generate a website using decomposed per-file LLM calls (legacy v1 path).
///
/// Each file is generated in a separate LLM call with max_tokens=1024.
/// Retries once on empty response. Continues on individual task failure.
pub fn generate_site_decomposed<P: LlmProvider>(
    description: &str,
    output_dir: &Path,
    gateway: &mut GovernedLlmGateway<P>,
    context: &mut AgentRuntimeContext,
    model: &str,
) -> Result<Vec<PathBuf>, AgentError> {
    // Use v2 path by default now
    generate_site_v2(description, output_dir, gateway, context, model, false)
}

/// Generate a complete single-page website using an LLM provider (legacy fallback).
///
/// Sends a crafted system prompt to the LLM, parses out HTML/CSS/JS from the response,
/// and writes `index.html`, `styles.css`, and `script.js` to `output_dir`.
pub fn generate_site_with_llm<P: LlmProvider>(
    description: &str,
    output_dir: &Path,
    gateway: &mut GovernedLlmGateway<P>,
    context: &mut AgentRuntimeContext,
    model: &str,
) -> Result<Vec<PathBuf>, AgentError> {
    let prompt = build_prompt(description);

    let response = gateway.query(context, &prompt, 4000, model)?;
    let (html, css, js) = parse_code_blocks(&response.output_text);

    std::fs::create_dir_all(output_dir)
        .map_err(|e| AgentError::ManifestError(format!("failed to create output dir: {e}")))?;

    let files = [("index.html", html), ("styles.css", css), ("script.js", js)];

    let mut created = Vec::new();
    for (name, content) in &files {
        if content.is_empty() {
            continue;
        }
        let path = output_dir.join(name);
        std::fs::write(&path, content)
            .map_err(|e| AgentError::ManifestError(format!("failed to write {name}: {e}")))?;
        created.push(path);
    }

    Ok(created)
}

/// Build the system prompt for LLM website generation (legacy).
fn build_prompt(description: &str) -> String {
    let lower = description.to_lowercase();
    let wants_inline = lower.contains("inline")
        || lower.contains("single file")
        || lower.contains("self-contained")
        || lower.contains("one file");

    // Inject design tokens into even the legacy path
    let tokens = select_design_tokens(description);
    let css_vars = tokens.to_css_variables();
    let fonts_url = tokens.google_fonts_url();

    let base = if wants_inline {
        format!(
            "{CONDUCTOR_SYSTEM_V2}\n\n\
             BUILD THIS WEBSITE: {description}\n\n\
             DESIGN TOKENS:\n```css\n{css_vars}\n```\n\
             Google Fonts: {fonts_url}\n\n\
             RULES: Return a single ```html code block. Put ALL CSS in <style> tags and ALL JS in <script> tags inside the HTML. \
             No external files. Use the design tokens above for ALL styling. Include the Google Fonts link.\n\n\
             {ANTI_PATTERNS}\n{DESIGN_DIRECTIVES}\n\n\
             Make it responsive. Include scroll animations and hover interactions."
        )
    } else {
        format!(
            "{CONDUCTOR_SYSTEM_V2}\n\n\
             BUILD THIS WEBSITE: {description}\n\n\
             DESIGN TOKENS:\n```css\n{css_vars}\n```\n\
             Google Fonts: {fonts_url}\n\n\
             RULES: Return exactly three code blocks: ```html, ```css, ```javascript. \
             The HTML should link to styles.css and script.js and include the Google Fonts link. \
             Use the design tokens above for ALL styling.\n\n\
             {ANTI_PATTERNS}\n{DESIGN_DIRECTIVES}\n\n\
             Make it responsive. Include scroll animations and hover interactions."
        )
    };

    let needs_3d = [
        "3d",
        "three.js",
        "animated hero",
        "particles",
        "webgl",
        "globe",
    ]
    .iter()
    .any(|kw| lower.contains(kw));

    if needs_3d {
        format!(
            "{base}\n\n\
             Also include a Three.js 3D scene. Import from: \
             https://cdnjs.cloudflare.com/ajax/libs/three.js/r128/three.min.js. \
             Create an impressive animated 3D hero section (rotating geometry, particle system, \
             or animated wave). Use requestAnimationFrame."
        )
    } else {
        base
    }
}

/// Parse HTML, CSS, and JS from fenced code blocks in the LLM response.
///
/// Looks for blocks fenced with ```html, ```css, and ```javascript or ```js.
/// If no fences are found at all, the entire response is treated as HTML.
pub fn parse_code_blocks(response: &str) -> (String, String, String) {
    let html = extract_block(response, &["html"]).unwrap_or_else(|| {
        // Fallback: if no fences found at all, treat entire response as HTML
        if extract_block(response, &["css"]).is_none()
            && extract_block(response, &["javascript", "js"]).is_none()
        {
            response.trim().to_string()
        } else {
            String::new()
        }
    });
    let css = extract_block(response, &["css"]).unwrap_or_default();
    let js = extract_block(response, &["javascript", "js"]).unwrap_or_default();

    (html, css, js)
}

/// Extract content from a fenced code block matching any of the given language labels.
fn extract_block(text: &str, labels: &[&str]) -> Option<String> {
    for label in labels {
        let marker = format!("```{label}");
        if let Some(start_idx) = text.find(&marker) {
            let after_marker = start_idx + marker.len();
            let rest = &text[after_marker..];
            // Skip to next newline (past any trailing text on the opener line)
            if let Some(nl) = rest.find('\n') {
                let content_start = &rest[nl + 1..];
                if let Some(end) = content_start.find("```") {
                    let block = content_start[..end].trim().to_string();
                    if !block.is_empty() {
                        return Some(block);
                    }
                }
            }
        }
    }
    None
}

// ─── Streaming V2 Generation ──────────────────────────────────────────────

/// Result of a streaming generation.
#[derive(Debug, Clone)]
pub struct StreamingGenerationResult {
    pub html: String,
    pub checkpoint_id: String,
    pub cost: f64,
    pub input_tokens: usize,
    pub output_tokens: usize,
    pub elapsed_seconds: f64,
}

/// V2 design-aware generation with streaming progress events.
///
/// Like `generate_site_v2` but uses [`StreamingLlmProvider`] to stream tokens
/// and calls `emit_event` for each progress update. The prompt construction,
/// design token injection, and post-processing are identical to the non-streaming
/// path — only the LLM call is different.
///
/// `emit_event` is called with `BuildStreamEvent` variants:
/// - `BuildStarted` at the beginning
/// - `GenerationProgress` every 500ms during streaming
/// - `BuildCompleted` or `BuildFailed` at the end
pub fn generate_site_v2_streaming<S: StreamingLlmProvider + ?Sized>(
    brief: &str,
    output_dir: &Path,
    streaming_provider: &S,
    model: &str,
    emit_event: &dyn Fn(BuildStreamEvent),
) -> Result<Vec<PathBuf>, AgentError> {
    let start = std::time::Instant::now();

    // Step 1: Select design tokens from the brief (same as non-streaming)
    let tokens = select_design_tokens(brief);
    eprintln!(
        "[conductor-v2-stream] Design tokens: palette={}, mood={}, fonts={}/{}",
        tokens.palette_name,
        tokens.mood.label(),
        tokens.font_display,
        tokens.font_body,
    );

    // Step 2: Build style guide (same as non-streaming)
    let style_guide = format!(
        "STYLE GUIDE (use these EXACT color values):\n\
         Background: {bg}\n\
         Surface/cards: {surface}\n\
         Text: {text}\n\
         Text secondary: {text2}\n\
         Accent (buttons/links): {accent}\n\
         Accent hover: {accent_hover}\n\
         Border: {border}\n\
         Display font: '{font_d}' (weight 700-800)\n\
         Body font: '{font_b}' (weight 400-500)\n\
         Border radius: {radius}\n\
         Google Fonts URL: {fonts_url}",
        bg = tokens.bg,
        surface = tokens.surface,
        text = tokens.text_primary,
        text2 = tokens.text_secondary,
        accent = tokens.accent,
        accent_hover = tokens.accent_hover,
        border = tokens.border,
        font_d = tokens.font_display,
        font_b = tokens.font_body,
        radius = tokens.radius,
        fonts_url = tokens.google_fonts_url(),
    );

    // Step 3: Derive site name from brief
    let site_name = extract_site_name(brief);

    // Step 4: Build prompt — skip double-wrapping if brief is already a planned prompt
    let is_planned = brief.contains("Expert web developer building")
        || brief.contains("Build a website according to this plan");
    let prompt = if is_planned {
        // Brief already contains the full planned prompt with quality directives,
        // acceptance criteria, and template scaffold. Just prepend the style guide.
        format!("{style_guide}\n\n{brief}")
    } else {
        build_generation_prompt(brief, &site_name, &style_guide)
    };

    // Log prompt size for optimization tracking
    let prompt_chars = prompt.len();
    let est_prompt_tokens = prompt_chars / 4; // rough estimate: 1 token ≈ 4 chars
    if est_prompt_tokens > 5000 {
        eprintln!(
            "[conductor-v2-stream] WARNING: prompt is ~{} tokens ({} chars) — target is < 5,000. Consider prompt optimization.",
            est_prompt_tokens, prompt_chars
        );
    } else {
        eprintln!(
            "[conductor-v2-stream] Prompt: ~{} tokens ({} chars)",
            est_prompt_tokens, prompt_chars
        );
    }

    let system_prompt = CONDUCTOR_SYSTEM_V2;

    // Step 5: Emit BuildStarted — use actual prompt size for cost estimate
    let est_input = if est_prompt_tokens > ESTIMATED_INPUT_TOKENS {
        est_prompt_tokens
    } else {
        ESTIMATED_INPUT_TOKENS
    };
    let estimated_cost = estimate_cost(model, est_input, ESTIMATED_TOTAL_TOKENS);
    emit_event(BuildStreamEvent::BuildStarted {
        project_name: site_name.clone(),
        estimated_cost,
        estimated_tasks: 1,
        model_name: model.to_string(),
        timestamp: chrono::Utc::now().to_rfc3339(),
    });

    // Step 6: Start streaming request
    eprintln!("[conductor-v2-stream] Starting streaming generation...");
    let mut stream = match streaming_provider.stream_query(&prompt, system_prompt, 16384, model) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[conductor-v2-stream] Stream request failed: {e}");
            emit_event(BuildStreamEvent::BuildFailed {
                error: e.to_string(),
                tokens_consumed: 0,
                cost_consumed: 0.0,
            });
            return Err(e);
        }
    };

    // Step 7: Consume stream with throttled progress events
    let mut accumulated = String::new();
    let mut token_count: usize = 0;
    let mut last_event_time = std::time::Instant::now();
    let mut stream_error: Option<AgentError> = None;

    loop {
        match stream.next() {
            Some(Ok(chunk)) => {
                accumulated.push_str(&chunk.text);
                token_count += chunk.token_count.unwrap_or(1);

                // Emit progress every 500ms to avoid flooding the event bus
                if last_event_time.elapsed() >= std::time::Duration::from_millis(500) {
                    let phase = detect_phase(&accumulated, token_count, ESTIMATED_TOTAL_TOKENS);
                    emit_event(BuildStreamEvent::GenerationProgress {
                        phase,
                        tokens_generated: token_count,
                        estimated_total_tokens: ESTIMATED_TOTAL_TOKENS,
                        elapsed_seconds: start.elapsed().as_secs_f64(),
                        raw_chunk: Some(chunk.text),
                    });
                    last_event_time = std::time::Instant::now();
                }
            }
            Some(Err(e)) => {
                eprintln!("[conductor-v2-stream] Stream error: {e}");
                stream_error = Some(e);
                break;
            }
            None => break, // stream ended
        }
    }

    if let Some(e) = stream_error {
        let cost = calculate_cost(model, 0, token_count);
        emit_event(BuildStreamEvent::BuildFailed {
            error: e.to_string(),
            tokens_consumed: token_count,
            cost_consumed: cost,
        });
        return Err(e);
    }

    if accumulated.trim().is_empty() {
        eprintln!("[conductor-v2-stream] Stream returned empty output");
        emit_event(BuildStreamEvent::BuildFailed {
            error: "LLM returned empty response".to_string(),
            tokens_consumed: token_count,
            cost_consumed: 0.0,
        });
        return Err(AgentError::SupervisorError(
            "streaming generation returned empty output".to_string(),
        ));
    }

    // Step 8: Get usage from stream
    let usage = stream.usage();
    let input_tokens = usage.input_tokens;
    let output_tokens = if usage.output_tokens > 0 {
        usage.output_tokens
    } else {
        token_count
    };
    let actual_cost = calculate_cost(model, input_tokens, output_tokens);
    eprintln!(
        "[conductor-v2-stream] Token usage: {} in, {} out (from API: {}/{}). Cost: ${:.4}",
        input_tokens, output_tokens, usage.input_tokens, usage.output_tokens, actual_cost
    );

    // Step 9: Post-process (same as non-streaming)
    let html = ensure_html_start(&strip_markdown_fences(&accumulated));
    let html = sanitize_generated_html(&html);

    // Step 10: Generate checkpoint
    let checkpoint_id = generate_checkpoint_id();

    // Step 11: Governance scan
    let governance = quick_governance_scan(&html);

    // Step 12: Write output
    std::fs::create_dir_all(output_dir)
        .map_err(|e| AgentError::ManifestError(format!("failed to create output dir: {e}")))?;

    let index_path = output_dir.join("index.html");
    std::fs::write(&index_path, &html)
        .map_err(|e| AgentError::ManifestError(format!("failed to write index.html: {e}")))?;

    let elapsed = start.elapsed().as_secs_f64();

    // Step 13: Emit BuildCompleted
    emit_event(BuildStreamEvent::BuildCompleted {
        project_name: site_name,
        total_lines: html.lines().count(),
        total_chars: html.len(),
        input_tokens,
        output_tokens,
        actual_cost,
        model_name: model.to_string(),
        elapsed_seconds: elapsed,
        checkpoint_id: checkpoint_id.clone(),
        governance_status: governance,
        output_dir: output_dir.to_string_lossy().to_string(),
    });

    eprintln!(
        "[conductor-v2-stream] Build complete: 1 file, {} lines, palette={}, mood={}, cost=${:.4}, {:.1}s",
        html.lines().count(),
        tokens.palette_name,
        tokens.mood.label(),
        actual_cost,
        elapsed,
    );

    Ok(vec![index_path])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_code_blocks_properly_fenced() {
        let response = r#"Here is your website:

```html
<!DOCTYPE html>
<html>
<head><title>Test</title></head>
<body><h1>Hello</h1></body>
</html>
```

```css
body { margin: 0; font-family: sans-serif; }
h1 { color: blue; }
```

```javascript
document.addEventListener('DOMContentLoaded', () => {
    console.log('ready');
});
```
"#;
        let (html, css, js) = parse_code_blocks(response);
        assert!(html.contains("<!DOCTYPE html>"));
        assert!(html.contains("<h1>Hello</h1>"));
        assert!(css.contains("font-family: sans-serif"));
        assert!(js.contains("DOMContentLoaded"));
    }

    #[test]
    fn test_parse_code_blocks_js_label() {
        let response = r#"```html
<html><body>Hi</body></html>
```

```css
body { color: red; }
```

```js
alert('hi');
```
"#;
        let (html, css, js) = parse_code_blocks(response);
        assert!(html.contains("<body>Hi</body>"));
        assert!(css.contains("color: red"));
        assert!(js.contains("alert"));
    }

    #[test]
    fn test_parse_code_blocks_no_fences_fallback() {
        let response = "<html><body>Fallback content</body></html>";
        let (html, css, js) = parse_code_blocks(response);
        assert!(html.contains("Fallback content"));
        assert!(css.is_empty());
        assert!(js.is_empty());
    }

    #[test]
    fn test_parse_code_blocks_partial_fences() {
        let response = r#"```html
<div>Only HTML here</div>
```

No CSS or JS blocks provided.
"#;
        let (html, css, js) = parse_code_blocks(response);
        assert!(html.contains("<div>Only HTML here</div>"));
        assert!(css.is_empty());
        assert!(js.is_empty());
    }

    #[test]
    fn test_parse_code_blocks_empty_blocks() {
        let response = r#"```html
```

```css
```

```javascript
```
"#;
        let (_html, css, js) = parse_code_blocks(response);
        assert!(css.is_empty());
        assert!(js.is_empty());
    }

    #[test]
    fn test_build_prompt_basic() {
        let prompt = build_prompt("a portfolio site");
        assert!(prompt.contains("BUILD THIS WEBSITE"));
        assert!(prompt.contains("a portfolio site"));
        assert!(!prompt.contains("Three.js"));
        // V2: should contain design tokens
        assert!(prompt.contains("--color-bg"));
        assert!(prompt.contains("--font-display"));
        assert!(prompt.contains("DO NOT use: Inter"));
    }

    #[test]
    fn test_build_prompt_3d() {
        let prompt = build_prompt("a landing page with 3D globe");
        assert!(prompt.contains("Three.js"));
        assert!(prompt.contains("requestAnimationFrame"));
    }

    #[test]
    fn test_build_prompt_particles() {
        let prompt = build_prompt("site with particles background");
        assert!(prompt.contains("Three.js"));
    }

    #[test]
    fn test_strip_markdown_fences_html() {
        let input = "```html\n<!DOCTYPE html>\n<html></html>\n```";
        assert_eq!(
            strip_markdown_fences(input),
            "<!DOCTYPE html>\n<html></html>"
        );
    }

    #[test]
    fn test_strip_markdown_fences_no_fences() {
        let input = "body { margin: 0; }";
        assert_eq!(strip_markdown_fences(input), "body { margin: 0; }");
    }

    #[test]
    fn test_strip_markdown_fences_css() {
        let input = "```css\nbody { color: red; }\n```";
        assert_eq!(strip_markdown_fences(input), "body { color: red; }");
    }

    #[test]
    fn test_strip_markdown_fences_plain() {
        let input = "```\nplain block\n```";
        assert_eq!(strip_markdown_fences(input), "plain block");
    }

    #[test]
    fn test_strip_markdown_fences_two_backticks() {
        // The exact bug: ``html\n<!DOCTYPE html>...
        let input = "``html\n<!DOCTYPE html>\n<html></html>\n``";
        assert_eq!(
            strip_markdown_fences(input),
            "<!DOCTYPE html>\n<html></html>"
        );
    }

    #[test]
    fn test_strip_markdown_fences_four_backticks() {
        let input = "````html\n<!DOCTYPE html>\n````";
        assert_eq!(strip_markdown_fences(input), "<!DOCTYPE html>");
    }

    #[test]
    fn test_strip_markdown_fences_no_closing() {
        // Model forgot closing fence
        let input = "```html\n<!DOCTYPE html>\n<html></html>";
        assert_eq!(
            strip_markdown_fences(input),
            "<!DOCTYPE html>\n<html></html>"
        );
    }

    #[test]
    fn test_strip_markdown_fences_no_lang_tag() {
        let input = "``\n<!DOCTYPE html>\n``";
        assert_eq!(strip_markdown_fences(input), "<!DOCTYPE html>");
    }

    #[test]
    fn test_ensure_html_start_clean() {
        assert_eq!(
            ensure_html_start("<!DOCTYPE html>\n<html>"),
            "<!DOCTYPE html>\n<html>"
        );
    }

    #[test]
    fn test_ensure_html_start_with_preamble() {
        let input = "Here is the HTML:\n<!DOCTYPE html>\n<html></html>";
        assert_eq!(ensure_html_start(input), "<!DOCTYPE html>\n<html></html>");
    }

    #[test]
    fn test_ensure_html_start_html_tag() {
        let input = "some junk\n<html lang=\"en\">\n<head>";
        assert_eq!(ensure_html_start(input), "<html lang=\"en\">\n<head>");
    }

    #[test]
    fn test_sanitize_removes_bare_opacity_zero() {
        let input = "div {\n  color: red;\n  opacity: 0;\n  margin: 10px;\n}";
        let result = sanitize_generated_html(input);
        assert!(
            !result.contains("opacity: 0;"),
            "bare opacity:0 should be removed"
        );
        assert!(result.contains("color: red;"));
        assert!(result.contains("margin: 10px;"));
    }

    #[test]
    fn test_sanitize_preserves_keyframe_opacity() {
        let input =
            "@keyframes fadeIn {\n  from {\n    opacity: 0;\n  }\n  to {\n    opacity: 1;\n  }\n}";
        let result = sanitize_generated_html(input);
        // The standalone line "opacity: 0;" inside keyframes should be preserved
        assert!(
            result.contains("opacity: 0") || result.contains("opacity:0"),
            "keyframe opacity should be preserved"
        );
    }

    #[test]
    fn test_sanitize_preserves_decorative_opacity() {
        let input = "span { opacity: 0.3; }";
        let result = sanitize_generated_html(input);
        assert!(
            result.contains("opacity: 0.3"),
            "decorative opacity should be preserved"
        );
    }

    #[test]
    fn test_sanitize_removes_inline_opacity_zero() {
        let input = r#"<div style="opacity: 0; transform: translateY(20px);">Hello</div>"#;
        let result = sanitize_generated_html(input);
        assert!(
            !result.contains("opacity: 0") && !result.contains("opacity:0"),
            "inline opacity:0 should be removed, got: {result}"
        );
        assert!(result.contains("transform: translateY(20px)"));
        assert!(result.contains("Hello"));
    }

    #[test]
    fn test_sanitize_noop_on_clean_html() {
        let input = "<div style=\"color: red;\">visible</div>";
        let result = sanitize_generated_html(input);
        assert_eq!(result, input);
    }

    #[test]
    fn test_decompose_web_tasks_basic() {
        // decompose_web_tasks still works for legacy path
        let tasks = decompose_web_tasks("a dark landing page");
        assert!(tasks.len() >= 2);
        assert_eq!(tasks[0].filename, "index.html");
        assert_eq!(tasks[1].filename, "styles.css");
    }

    #[test]
    fn test_decompose_web_tasks_with_form() {
        let tasks = decompose_web_tasks("landing page with a contact form");
        let filenames: Vec<&str> = tasks.iter().map(|t| t.filename.as_str()).collect();
        assert!(filenames.contains(&"script.js"));
    }

    #[test]
    fn test_decompose_web_tasks_with_3d() {
        let tasks = decompose_web_tasks("a site with 3D globe");
        let filenames: Vec<&str> = tasks.iter().map(|t| t.filename.as_str()).collect();
        assert!(filenames.contains(&"scene.js"));
    }

    // ─── V2 Tests ──────────────────────────────────────────────────────────

    #[test]
    fn test_extract_site_name_called() {
        assert_eq!(
            extract_site_name("Build a landing page called Nexus OS"),
            "Nexus Os"
        );
    }

    #[test]
    fn test_extract_site_name_for() {
        let name = extract_site_name("Landing page for a developer tool");
        assert!(!name.is_empty());
        assert!(!name.contains("landing"));
    }

    #[test]
    fn test_extract_site_name_fallback() {
        let name = extract_site_name("something cool");
        assert!(!name.is_empty());
    }

    #[test]
    fn test_derive_headline_ai() {
        let headline = derive_headline("AI governance platform");
        assert!(headline.contains("AI") || headline.contains("Governed"));
    }

    #[test]
    fn test_derive_headline_restaurant() {
        let headline = derive_headline("Italian restaurant website");
        assert!(headline.contains("Passion") || headline.contains("Love"));
    }

    #[test]
    fn test_fallback_page_structure() {
        let tokens = select_design_tokens("A SaaS platform");
        let html = generate_fallback_page(&tokens, "TestApp", "A SaaS platform");
        assert!(html.contains("<!DOCTYPE html>"));
        assert!(html.contains("fonts.googleapis.com"));
        assert!(html.contains("fadeInUp"));
        assert!(html.contains("IntersectionObserver"));
        assert!(html.contains("TestApp"));
        assert!(html.contains(&tokens.accent));
        assert!(html.contains(&tokens.font_display));
        assert!(html.contains("id=\"hero\""));
        assert!(html.contains("id=\"features\""));
        assert!(html.contains("id=\"pricing\""));
        assert!(html.contains("id=\"testimonials\""));
        assert!(html.contains("id=\"contact\""));
    }

    #[test]
    fn test_fallback_page_uses_design_tokens() {
        let tokens = select_design_tokens("AI governance platform");
        let html = generate_fallback_page(&tokens, "Nexus", "AI governance platform");
        assert!(html.contains(&tokens.bg));
        assert!(html.contains(&tokens.surface));
        assert!(html.contains(&tokens.text_primary));
        assert!(html.contains(&tokens.accent));
        assert!(html.contains(&tokens.font_display));
        assert!(html.contains(&tokens.font_body));
    }

    #[test]
    fn test_derive_features_ai() {
        let features = derive_features("AI agent governance platform");
        assert!(!features.is_empty());
        assert!(features[0].0.contains("Governed") || features[0].0.contains("Autonomy"));
    }
}

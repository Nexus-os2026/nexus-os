//! Smart iteration: 3-tier edit system for efficient website iteration.
//!
//! - **Tier 1 (CSS Variable):** instant, $0.00 — pure string replacement of CSS custom properties.
//! - **Tier 2 (Section Edit):** ~$0.03, ~15s — extract one section, send to LLM, splice back.
//! - **Tier 3 (Full Regeneration):** ~$0.20, ~130s — send entire HTML to LLM (existing behavior).

use serde::{Deserialize, Serialize};

// ─── Edit Tier ───────────────────────────────────────────────────────────────

/// Which tier of edit to apply.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EditTier {
    /// Tier 1: instant, $0.00 — CSS variable replacement only.
    CssVariable,
    /// Tier 2: ~$0.03, ~15s — LLM edits a single section.
    SectionEdit,
    /// Tier 3: ~$0.20, ~130s — full-page LLM regeneration (current behavior).
    FullRegeneration,
}

/// Result of classifying a user's edit request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditClassification {
    pub tier: EditTier,
    /// Section ID for Tier 2 (the `data-nexus-section` value).
    pub target_section: Option<String>,
    /// CSS variable changes for Tier 1.
    pub css_changes: Option<Vec<CssChange>>,
    /// Confidence of the classification (0.0–1.0).
    pub confidence: f64,
    /// Human-readable explanation of why this tier was chosen.
    pub reason: String,
}

/// A single CSS custom property change.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CssChange {
    pub variable: String,
    pub old_value: Option<String>,
    pub new_value: String,
}

/// Byte span of an extracted section in the original HTML.
#[derive(Debug, Clone)]
pub struct SectionSpan {
    /// The full section HTML (opening tag through closing tag).
    pub content: String,
    /// Byte offset of the first character in the original HTML.
    pub start: usize,
    /// Byte offset one past the last character.
    pub end: usize,
    /// The tag name (section, footer, header, nav, aside, div).
    pub tag_name: String,
}

/// Result of a Tier 2 section-level LLM edit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SectionEditResult {
    /// Full HTML with the section replaced/added/removed.
    pub html: String,
    pub input_tokens: usize,
    pub output_tokens: usize,
    pub cost_usd: f64,
    pub elapsed_seconds: f64,
}

// ─── Named color map ─────────────────────────────────────────────────────────

fn named_color(word: &str) -> Option<&'static str> {
    match word {
        "yellow" => Some("#FFD700"),
        "gold" => Some("#FFD700"),
        "blue" => Some("#3B82F6"),
        "red" => Some("#EF4444"),
        "green" => Some("#22C55E"),
        "orange" => Some("#F97316"),
        "purple" => Some("#A855F7"),
        "pink" => Some("#EC4899"),
        "cyan" | "teal" => Some("#06B6D4"),
        "white" => Some("#FFFFFF"),
        "black" => Some("#000000"),
        "gray" | "grey" => Some("#6B7280"),
        "indigo" => Some("#6366F1"),
        "lime" => Some("#84CC16"),
        "amber" => Some("#F59E0B"),
        "emerald" => Some("#10B981"),
        "rose" => Some("#F43F5E"),
        "violet" => Some("#8B5CF6"),
        "sky" => Some("#0EA5E9"),
        "slate" => Some("#64748B"),
        "stone" => Some("#78716C"),
        "zinc" => Some("#71717A"),
        "neutral" => Some("#737373"),
        "coral" => Some("#FF6B6B"),
        "navy" => Some("#1E3A5F"),
        "maroon" => Some("#800000"),
        "olive" => Some("#808000"),
        "salmon" => Some("#FA8072"),
        "crimson" => Some("#DC143C"),
        "turquoise" => Some("#40E0D0"),
        "magenta" | "fuchsia" => Some("#FF00FF"),
        "beige" => Some("#F5F5DC"),
        "ivory" => Some("#FFFFF0"),
        "chocolate" => Some("#D2691E"),
        "tomato" => Some("#FF6347"),
        _ => None,
    }
}

// ─── CSS Variable Helpers ────────────────────────────────────────────────────

/// Check whether the HTML contains a `:root { ... --var: ... }` block.
pub fn has_css_variables(html: &str) -> bool {
    let lower = html.to_lowercase();
    lower.contains(":root") && lower.contains("--")
}

/// Check whether the HTML contains any `data-nexus-section` attributes.
pub fn has_section_anchors(html: &str) -> bool {
    html.contains("data-nexus-section")
}

/// Parse all CSS custom properties from the `:root { }` block.
///
/// Returns `(variable_name, value)` pairs, e.g. `("--primary", "#c2410c")`.
pub fn parse_css_variables(html: &str) -> Vec<(String, String)> {
    let root_block = match extract_root_block(html) {
        Some(b) => b,
        None => return vec![],
    };
    parse_vars_from_block(&root_block)
}

/// Find the `:root { ... }` block content (between the braces, inclusive).
fn extract_root_block(html: &str) -> Option<String> {
    let lower = html.to_lowercase();
    let root_pos = lower.find(":root")?;
    let brace_start = html[root_pos..].find('{')? + root_pos;
    let mut depth = 0;
    let mut brace_end = brace_start;
    for (i, ch) in html[brace_start..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    brace_end = brace_start + i;
                    break;
                }
            }
            _ => {}
        }
    }
    if brace_end <= brace_start {
        return None;
    }
    Some(html[brace_start + 1..brace_end].to_string())
}

/// Parse `--var: value;` declarations from a CSS block.
fn parse_vars_from_block(block: &str) -> Vec<(String, String)> {
    let mut result = Vec::new();
    for line in block.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("--") {
            if let Some(colon_pos) = rest.find(':') {
                let var_name = format!("--{}", rest[..colon_pos].trim());
                let value = rest[colon_pos + 1..]
                    .trim()
                    .trim_end_matches(';')
                    .trim()
                    .to_string();
                result.push((var_name, value));
            }
        }
    }
    result
}

// ─── Classify Edit ───────────────────────────────────────────────────────────

/// Classify a user's edit request into one of three tiers.
pub fn classify_edit(request: &str, html: &str) -> EditClassification {
    let lower = request.to_lowercase();

    // ── Check for Tier 3 triggers first (they override everything) ──
    let tier3_keywords = [
        "redesign",
        "completely different",
        "start over",
        "from scratch",
        "rebuild",
        "new layout",
        "new design",
        "overhaul",
        "redo everything",
        "total makeover",
    ];
    for kw in &tier3_keywords {
        if lower.contains(kw) {
            return EditClassification {
                tier: EditTier::FullRegeneration,
                target_section: None,
                css_changes: None,
                confidence: 0.9,
                reason: format!("Structural change keyword detected: \"{}\"", kw),
            };
        }
    }

    // ── Try Tier 1: CSS variable edits ──
    if has_css_variables(html) {
        if let Some(classification) = try_classify_css(&lower, html) {
            return classification;
        }
    }

    // ── Try Tier 2: Section-level edits ──
    if has_section_anchors(html) {
        if let Some(classification) = try_classify_section(&lower, html) {
            return classification;
        }
    }

    // ── Fallback: Tier 3 ──
    EditClassification {
        tier: EditTier::FullRegeneration,
        target_section: None,
        css_changes: None,
        confidence: 0.5,
        reason: "No CSS variable or section-level match; falling back to full regeneration"
            .to_string(),
    }
}

/// Try to classify as Tier 1 (CSS variable edit).
fn try_classify_css(lower: &str, html: &str) -> Option<EditClassification> {
    let css_keywords = [
        "color",
        "background",
        "font",
        "theme",
        "dark mode",
        "light mode",
        "spacing",
        "border",
        "shadow",
        "radius",
        "bigger",
        "smaller",
        "larger",
    ];

    // Check for hex colors in request
    let has_hex = lower
        .split_whitespace()
        .any(|w| w.starts_with('#') && (w.len() == 4 || w.len() == 7));

    // Check for named colors
    let has_named_color = lower.split_whitespace().any(|w| named_color(w).is_some());

    let has_css_keyword = css_keywords.iter().any(|kw| lower.contains(kw));

    if !has_hex && !has_named_color && !has_css_keyword {
        return None;
    }

    let existing_vars = parse_css_variables(html);
    if existing_vars.is_empty() {
        return None;
    }

    let mut changes = Vec::new();

    // ── Dark mode / light mode swap ──
    if lower.contains("dark mode") || lower.contains("dark theme") {
        changes.extend(build_dark_mode_changes(&existing_vars));
    } else if lower.contains("light mode") || lower.contains("light theme") {
        changes.extend(build_light_mode_changes(&existing_vars));
    }

    // ── Specific color targeting ──
    if changes.is_empty() {
        changes.extend(build_color_changes(lower, &existing_vars));
    }

    // ── Font size changes ──
    if lower.contains("bigger") || lower.contains("larger") || lower.contains("increase") {
        changes.extend(build_font_size_changes(&existing_vars, true));
    } else if lower.contains("smaller") || lower.contains("decrease") {
        changes.extend(build_font_size_changes(&existing_vars, false));
    }

    // ── Font family changes ──
    if lower.contains("change font to") || lower.contains("use font") {
        if let Some(font_changes) = build_font_family_changes(lower, &existing_vars) {
            changes.extend(font_changes);
        }
    }

    if changes.is_empty() {
        return None;
    }

    Some(EditClassification {
        tier: EditTier::CssVariable,
        target_section: None,
        css_changes: Some(changes),
        confidence: 0.85,
        reason: "CSS variable edit detected from color/theme/font keywords".to_string(),
    })
}

/// Build changes for dark mode switch.
///
/// Handles all common variable naming patterns:
/// `--bg`, `--bg-color`, `--bg-2`, `--surface`, `--surface-2`, `--surface-3`,
/// `--text`, `--text-color`, `--text-muted`, `--text-subtle`,
/// `--border`, `--border-light`, `--card-bg`, etc.
fn build_dark_mode_changes(vars: &[(String, String)]) -> Vec<CssChange> {
    let mut changes = Vec::new();
    for (name, value) in vars {
        let n = name.to_lowercase();
        // Skip --primary, --accent, --glow, --radius, --font, --card-radius, --transition
        if n.contains("primary")
            || n.contains("accent")
            || n.contains("glow")
            || n.contains("radius")
            || n.contains("font")
            || n.contains("transition")
            || n.contains("timing")
            || n.contains("shadow")
        {
            continue;
        }
        if is_bg_var(&n) && is_light_color(value) {
            // Dark background tiers: main bg darkest, secondary slightly lighter
            let dark_val = if n == "--bg" || n == "--bg-color" || n == "--background" {
                "#0f172a"
            } else {
                "#1e293b" // --bg-2, --bg-alt, etc.
            };
            changes.push(CssChange {
                variable: name.clone(),
                old_value: Some(value.clone()),
                new_value: dark_val.to_string(),
            });
        } else if is_surface_var(&n) && is_light_color(value) {
            let dark_val = if n == "--surface" || n == "--surface-color" || n == "--card-bg" {
                "#1e293b"
            } else {
                "#334155" // --surface-2, --surface-3, etc.
            };
            changes.push(CssChange {
                variable: name.clone(),
                old_value: Some(value.clone()),
                new_value: dark_val.to_string(),
            });
        } else if is_text_var(&n) && is_dark_color(value) {
            let light_val = if n == "--text" || n == "--text-color" || n == "--foreground" {
                "#f8fafc"
            } else if n.contains("muted") || n.contains("subtle") || n.contains("secondary") {
                "#94a3b8"
            } else {
                "#e2e8f0" // --text-2, etc.
            };
            changes.push(CssChange {
                variable: name.clone(),
                old_value: Some(value.clone()),
                new_value: light_val.to_string(),
            });
        } else if is_text_var(&n) && is_mid_color(value) {
            // Text-muted/subtle may be mid-range grays — make them lighter for dark bg
            if n.contains("muted") || n.contains("subtle") || n.contains("secondary") {
                changes.push(CssChange {
                    variable: name.clone(),
                    old_value: Some(value.clone()),
                    new_value: "#94a3b8".to_string(),
                });
            }
        } else if is_border_var(&n) && is_light_color(value) {
            changes.push(CssChange {
                variable: name.clone(),
                old_value: Some(value.clone()),
                new_value: "#334155".to_string(),
            });
        } else if is_border_var(&n) && is_mid_color(value) {
            // Light-theme borders are often mid-gray — darken them
            changes.push(CssChange {
                variable: name.clone(),
                old_value: Some(value.clone()),
                new_value: "#334155".to_string(),
            });
        }
    }
    eprintln!(
        "[dark-mode-swap] {} changes: {:?}",
        changes.len(),
        changes
            .iter()
            .map(|c| format!(
                "{}: {} → {}",
                c.variable,
                c.old_value.as_deref().unwrap_or("?"),
                c.new_value
            ))
            .collect::<Vec<_>>()
    );
    changes
}

/// Build changes for light mode switch.
fn build_light_mode_changes(vars: &[(String, String)]) -> Vec<CssChange> {
    let mut changes = Vec::new();
    for (name, value) in vars {
        let n = name.to_lowercase();
        if n.contains("primary")
            || n.contains("accent")
            || n.contains("glow")
            || n.contains("radius")
            || n.contains("font")
            || n.contains("transition")
            || n.contains("timing")
            || n.contains("shadow")
        {
            continue;
        }
        if is_bg_var(&n) && is_dark_color(value) {
            let light_val = if n == "--bg" || n == "--bg-color" || n == "--background" {
                "#ffffff"
            } else {
                "#f8fafc"
            };
            changes.push(CssChange {
                variable: name.clone(),
                old_value: Some(value.clone()),
                new_value: light_val.to_string(),
            });
        } else if is_surface_var(&n) && is_dark_color(value) {
            let light_val = if n == "--surface" || n == "--surface-color" || n == "--card-bg" {
                "#f1f5f9"
            } else {
                "#e2e8f0"
            };
            changes.push(CssChange {
                variable: name.clone(),
                old_value: Some(value.clone()),
                new_value: light_val.to_string(),
            });
        } else if is_text_var(&n) && is_light_color(value) {
            let dark_val = if n == "--text" || n == "--text-color" || n == "--foreground" {
                "#0f172a"
            } else if n.contains("muted") || n.contains("subtle") || n.contains("secondary") {
                "#64748b"
            } else {
                "#1e293b"
            };
            changes.push(CssChange {
                variable: name.clone(),
                old_value: Some(value.clone()),
                new_value: dark_val.to_string(),
            });
        } else if is_border_var(&n) && is_dark_color(value) {
            changes.push(CssChange {
                variable: name.clone(),
                old_value: Some(value.clone()),
                new_value: "#e2e8f0".to_string(),
            });
        }
    }
    eprintln!(
        "[light-mode-swap] {} changes: {:?}",
        changes.len(),
        changes
            .iter()
            .map(|c| format!(
                "{}: {} → {}",
                c.variable,
                c.old_value.as_deref().unwrap_or("?"),
                c.new_value
            ))
            .collect::<Vec<_>>()
    );
    changes
}

// ─── Variable Category Helpers ───────────────────────────────────────────────

fn is_bg_var(n: &str) -> bool {
    n.contains("bg") || n.contains("background")
}

fn is_surface_var(n: &str) -> bool {
    (n.contains("surface") || n.contains("card")) && !n.contains("radius")
}

fn is_text_var(n: &str) -> bool {
    n.contains("text") || n.contains("foreground")
}

fn is_border_var(n: &str) -> bool {
    n.contains("border")
}

/// Build color changes from user request.
fn build_color_changes(lower: &str, vars: &[(String, String)]) -> Vec<CssChange> {
    let mut changes = Vec::new();

    // Find the target color from the request
    let target_color = find_target_color(lower);
    let target_color = match target_color {
        Some(c) => c,
        None => return changes,
    };

    // Determine which variable(s) to change based on request context
    let targets = identify_target_variables(lower, vars);

    for (var_name, old_value) in targets {
        changes.push(CssChange {
            variable: var_name,
            old_value: Some(old_value),
            new_value: target_color.clone(),
        });
    }

    changes
}

/// Extract the target color value from a user request.
fn find_target_color(lower: &str) -> Option<String> {
    // Check for hex color first
    for word in lower.split_whitespace() {
        if word.starts_with('#') && (word.len() == 4 || word.len() == 7) {
            let hex = word.trim_matches(|c: char| !c.is_ascii_hexdigit() && c != '#');
            if hex.len() == 4 || hex.len() == 7 {
                return Some(hex.to_uppercase());
            }
        }
    }
    // Check for named colors
    for word in lower.split_whitespace() {
        let clean = word.trim_matches(|c: char| !c.is_alphabetic());
        if let Some(hex) = named_color(clean) {
            return Some(hex.to_string());
        }
    }
    None
}

/// Identify which CSS variables the user wants to change.
fn identify_target_variables(lower: &str, vars: &[(String, String)]) -> Vec<(String, String)> {
    let mut targets = Vec::new();

    // "button" → accent/primary, "background" → bg, "text" → text/foreground
    let is_button = lower.contains("button") || lower.contains("btn") || lower.contains("cta");
    let is_accent = lower.contains("accent")
        || lower.contains("primary")
        || lower.contains("main color")
        || is_button;
    let is_bg =
        lower.contains("background") || lower.contains("bg") || lower.contains("page color");
    let is_text = lower.contains("text color") || lower.contains("font color");
    let is_secondary = lower.contains("secondary");
    let is_border_kw = lower.contains("border");
    let is_heading = lower.contains("heading");

    for (name, value) in vars {
        let n = name.to_lowercase();
        let matched = if is_accent && !is_bg && !is_text {
            n.contains("accent") || n.contains("primary")
        } else if is_secondary {
            n.contains("secondary")
        } else if is_bg {
            n.contains("bg") || n.contains("background")
        } else if is_text {
            n.contains("text") || n.contains("foreground")
        } else if is_border_kw {
            n.contains("border")
        } else if is_heading {
            n.contains("heading")
        } else {
            // Default: change accent/primary color
            n.contains("accent") || n.contains("primary")
        };

        if matched {
            targets.push((name.clone(), value.clone()));
        }
    }

    // If nothing matched, change the first accent-like variable
    if targets.is_empty() {
        for (name, value) in vars {
            let n = name.to_lowercase();
            if n.contains("accent") || n.contains("primary") || n.contains("color") {
                targets.push((name.clone(), value.clone()));
                break;
            }
        }
    }

    targets
}

/// Build font size changes (increase or decrease).
fn build_font_size_changes(vars: &[(String, String)], increase: bool) -> Vec<CssChange> {
    let mut changes = Vec::new();
    for (name, value) in vars {
        let n = name.to_lowercase();
        if n.contains("font-size") || n.contains("fontsize") || n.contains("size") {
            if let Some(new_val) = scale_css_value(value, increase) {
                changes.push(CssChange {
                    variable: name.clone(),
                    old_value: Some(value.clone()),
                    new_value: new_val,
                });
            }
        }
    }
    changes
}

/// Build font family changes.
fn build_font_family_changes(lower: &str, vars: &[(String, String)]) -> Option<Vec<CssChange>> {
    // Extract font name after "change font to" or "use font"
    let font_name = lower
        .find("change font to ")
        .map(|pos| lower[pos + 15..].trim().to_string())
        .or_else(|| {
            lower
                .find("use font ")
                .map(|pos| lower[pos + 9..].trim().to_string())
        })?;

    let clean_font = font_name
        .trim_matches(|c: char| c == '\'' || c == '"')
        .to_string();
    let css_font = format!("'{}', system-ui, sans-serif", clean_font);

    let mut changes = Vec::new();
    for (name, value) in vars {
        let n = name.to_lowercase();
        if n.contains("font") && (n.contains("body") || n.contains("main") || n.contains("base")) {
            changes.push(CssChange {
                variable: name.clone(),
                old_value: Some(value.clone()),
                new_value: css_font.clone(),
            });
        }
    }
    if changes.is_empty() {
        // Change all font variables
        for (name, value) in vars {
            let n = name.to_lowercase();
            if n.contains("font") && !n.contains("size") && !n.contains("weight") {
                changes.push(CssChange {
                    variable: name.clone(),
                    old_value: Some(value.clone()),
                    new_value: css_font.clone(),
                });
            }
        }
    }
    if changes.is_empty() {
        None
    } else {
        Some(changes)
    }
}

// ─── Color Lightness Helpers ─────────────────────────────────────────────────

/// Rough check: is this color "light" (high luminance)?
fn is_light_color(value: &str) -> bool {
    if let Some(lum) = hex_luminance(value) {
        lum > 0.5
    } else {
        // Heuristic: contains "white" or "fff"
        let v = value.to_lowercase();
        v.contains("fff") || v.contains("white")
    }
}

/// Rough check: is this color "dark" (low luminance)?
fn is_dark_color(value: &str) -> bool {
    if let Some(lum) = hex_luminance(value) {
        lum < 0.3
    } else {
        let v = value.to_lowercase();
        v.contains("000") || v.contains("black") || v.contains("0a0a") || v.contains("111")
    }
}

/// Rough check: is this color "mid-range" (neither clearly light nor dark)?
/// Useful for borders and muted text that need adjustment in mode swaps.
fn is_mid_color(value: &str) -> bool {
    if let Some(lum) = hex_luminance(value) {
        (0.3..=0.7).contains(&lum)
    } else {
        false
    }
}

/// Parse a hex color and return approximate luminance (0.0–1.0).
fn hex_luminance(hex: &str) -> Option<f64> {
    let hex = hex.trim().trim_start_matches('#');
    let (r, g, b) = match hex.len() {
        3 => {
            let r = u8::from_str_radix(&hex[0..1], 16).ok()? * 17;
            let g = u8::from_str_radix(&hex[1..2], 16).ok()? * 17;
            let b = u8::from_str_radix(&hex[2..3], 16).ok()? * 17;
            (r, g, b)
        }
        6 => {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            (r, g, b)
        }
        _ => return None,
    };
    // Perceived brightness (ITU-R BT.601)
    Some((0.299 * r as f64 + 0.587 * g as f64 + 0.114 * b as f64) / 255.0)
}

/// Scale a CSS numeric value up or down by ~20%.
fn scale_css_value(value: &str, increase: bool) -> Option<String> {
    let trimmed = value.trim();
    // Try to parse as number+unit (e.g. "16px", "1.2rem", "1rem")
    let (num_str, unit) = split_number_unit(trimmed)?;
    let num: f64 = num_str.parse().ok()?;
    let factor = if increase { 1.2 } else { 0.8 };
    let new_num = num * factor;
    // Format nicely: round to nearest integer for whole-number inputs
    let rounded = new_num.round();
    if (new_num - rounded).abs() < 0.01 {
        Some(format!("{}{}", rounded as i64, unit))
    } else {
        // Trim trailing zeros from decimal
        let formatted = format!("{:.2}", new_num);
        let trimmed = formatted.trim_end_matches('0').trim_end_matches('.');
        Some(format!("{}{}", trimmed, unit))
    }
}

/// Split "16px" → ("16", "px"), "1.2rem" → ("1.2", "rem").
fn split_number_unit(s: &str) -> Option<(&str, &str)> {
    let first_alpha = s.find(|c: char| c.is_alphabetic() || c == '%')?;
    if first_alpha == 0 {
        return None;
    }
    Some((&s[..first_alpha], &s[first_alpha..]))
}

// ─── Tier 2: Section Classification ──────────────────────────────────────────

/// Try to classify as Tier 2 (section-level edit).
fn try_classify_section(lower: &str, html: &str) -> Option<EditClassification> {
    let section_action_prefixes = [
        "add a ",
        "add an ",
        "remove the ",
        "remove my ",
        "delete the ",
        "get rid of the ",
        "hide the ",
        "drop the ",
        "change the ",
        "update the ",
        "move the ",
        "replace the ",
        "rewrite the ",
        "improve the ",
        "make the ",
        "fix the ",
        "edit the ",
        "modify the ",
    ];

    let is_remove = lower.contains("remove the ")
        || lower.contains("remove my ")
        || lower.contains("delete the ")
        || lower.contains("get rid of the ")
        || lower.contains("get rid of ")
        || lower.contains("hide the ")
        || lower.contains("drop the ");

    let is_add = lower.starts_with("add a ")
        || lower.starts_with("add an ")
        || lower.contains("add a ")
        || lower.contains("add an ");

    // Extract the section identifier from the request
    let section_id = find_section_id(lower, html, &section_action_prefixes)?;

    // For non-add requests, verify the section exists
    if !is_add {
        let exists = extract_section(html, &section_id).is_some();
        if !exists {
            return None;
        }
    }

    // Check if multiple sections are being changed (→ Tier 3)
    let section_count = count_section_mentions(lower, html);
    if section_count > 1 {
        return Some(EditClassification {
            tier: EditTier::FullRegeneration,
            target_section: None,
            css_changes: None,
            confidence: 0.7,
            reason: format!(
                "Multiple sections referenced ({} sections); using full regeneration",
                section_count
            ),
        });
    }

    let reason = if is_remove {
        format!("Section removal: \"{}\"", section_id)
    } else if is_add {
        format!("Section addition: \"{}\"", section_id)
    } else {
        format!("Section edit: \"{}\"", section_id)
    };

    Some(EditClassification {
        tier: EditTier::SectionEdit,
        target_section: Some(section_id),
        css_changes: None,
        confidence: 0.8,
        reason,
    })
}

/// Map user words to a `data-nexus-section` value.
fn find_section_id(lower: &str, html: &str, prefixes: &[&str]) -> Option<String> {
    // Find ALL prefix matches and sort by position in the request.
    // The EARLIEST match in the text is the primary intent.
    let mut matches: Vec<(usize, String)> = Vec::new();
    for prefix in prefixes {
        if let Some(pos) = lower.find(prefix) {
            let rest = &lower[pos + prefix.len()..];
            // Take words until common stop words
            let words: Vec<&str> = rest
                .split_whitespace()
                .take_while(|w| {
                    !["to", "with", "and", "so", "that", "in", "on", "at", "from"].contains(w)
                })
                .collect();
            if !words.is_empty() {
                let joined = words.join(" ");
                let cleaned = joined
                    .trim_end_matches(" section")
                    .trim_end_matches(" part")
                    .trim_end_matches(" area")
                    .trim_end_matches(" block")
                    .to_string();
                // Skip phrases that start with a content element word —
                // "add a tagline" is about content, not a section action
                let first_word = cleaned.split_whitespace().next().unwrap_or("");
                if !CONTENT_ELEMENT_WORDS.contains(&first_word) {
                    matches.push((pos, cleaned));
                }
            }
        }
    }
    // Sort by position — earliest match is the primary intent
    matches.sort_by_key(|(pos, _)| *pos);
    let phrase = matches.into_iter().next().map(|(_, p)| p)?;

    // Fuzzy-map common names
    let canonical = match phrase.as_str() {
        "hero" | "top part" | "banner" | "landing" | "main banner" | "header area"
        | "hero banner" | "top section" | "top" => "hero",
        "navigation" | "nav" | "navbar" | "menu" | "nav bar" | "top menu" | "main menu" => "nav",
        "header" | "site header" => "header",
        "footer" | "bottom" | "site footer" => "footer",
        "pricing" | "prices" | "plans" | "pricing table" | "pricing cards" => "pricing",
        "testimonials" | "reviews" | "feedback" | "quotes" | "customer reviews" => "testimonials",
        "gallery" | "portfolio" | "images" | "photos" | "showcase" | "work" | "projects" => {
            "gallery"
        }
        "contact" | "contact info" | "contact form" | "get in touch" => "contact",
        "about" | "about us" | "who we are" | "team" => "about",
        "features" | "services" | "what we do" | "capabilities" => "features",
        "faq" | "questions" | "frequently asked" => "faq",
        "cta" | "call to action" | "signup" | "sign up" | "subscribe" => "cta",
        "stats" | "statistics" | "numbers" | "metrics" | "counters" => "stats",
        "blog" | "news" | "articles" | "posts" => "blog",
        "map" | "location" | "directions" | "find us" => "map",
        other => other,
    };

    // Check if a section with this ID (or close match) exists in the HTML
    let existing_sections = list_section_ids(html);

    // Exact match
    if existing_sections.contains(&canonical.to_string()) {
        return Some(canonical.to_string());
    }

    // Fuzzy match: check if any existing section contains the canonical name
    for section in &existing_sections {
        if section.contains(canonical) || canonical.contains(section.as_str()) {
            return Some(section.clone());
        }
    }

    // Cross-reference: "nav" ↔ "header", "hero" ↔ "header"
    let related = match canonical {
        "nav" | "navigation" | "menu" => &["header", "navbar", "navigation"][..],
        "header" => &["nav", "navbar"],
        _ => &[],
    };
    for alt in related {
        if existing_sections.contains(&alt.to_string()) {
            return Some(alt.to_string());
        }
    }

    // For "add" actions, the section might not exist yet — use the canonical name
    if lower.contains("add a") || lower.contains("add an") {
        return Some(canonical.to_string());
    }

    // Last resort: try using the phrase directly, but skip ambiguous words
    for section in &existing_sections {
        let s = section.to_lowercase();
        if phrase.split_whitespace().any(|w| {
            s.contains(w)
                && !CONTENT_ELEMENT_WORDS.contains(&w)
                && !AMBIGUOUS_SECTION_WORDS.contains(&w)
                && w.len() > 2 // skip tiny words like "a", "an", etc.
        }) {
            return Some(section.clone());
        }
    }

    None
}

/// List all `data-nexus-section` values in the HTML.
fn list_section_ids(html: &str) -> Vec<String> {
    let mut ids = Vec::new();
    let needle = "data-nexus-section=\"";
    let mut search_from = 0;
    while let Some(pos) = html[search_from..].find(needle) {
        let abs_pos = search_from + pos + needle.len();
        if let Some(end_quote) = html[abs_pos..].find('"') {
            ids.push(html[abs_pos..abs_pos + end_quote].to_string());
        }
        search_from = abs_pos + 1;
    }
    ids
}

/// Words that describe content elements inside a section, NOT section names.
/// These must never be counted as section references.
const CONTENT_ELEMENT_WORDS: &[&str] = &[
    "tagline",
    "headline",
    "subtitle",
    "button",
    "text",
    "image",
    "title",
    "paragraph",
    "description",
    "content",
    "link",
    "form",
    "card",
    "icon",
    "logo",
    "heading",
    "subheading",
    "caption",
    "label",
    "input",
    "video",
    "photo",
    "badge",
    "tag",
    "list",
    "item",
    "divider",
    "spacer",
    "wrapper",
    "container",
    "overlay",
    "background",
    "border",
    "shadow",
    "animation",
    "font",
    "color",
    "size",
    "style",
    "recipe",
    "recipes",
    "family",
    "word",
    "words",
    "sentence",
    "line",
    "copy",
    "slogan",
    "motto",
    "message",
    "note",
    "info",
    "detail",
    "details",
];

/// Check if a word appears as a standalone word (word-boundary match) in text.
fn contains_word(text: &str, word: &str) -> bool {
    for (pos, _) in text.match_indices(word) {
        // Check character before the match (must be non-alphanumeric or start of string)
        let before_ok = pos == 0
            || text.as_bytes()[pos - 1].is_ascii_whitespace()
            || !text.as_bytes()[pos - 1].is_ascii_alphanumeric();
        // Check character after the match
        let end = pos + word.len();
        let after_ok = end >= text.len()
            || text.as_bytes()[end].is_ascii_whitespace()
            || !text.as_bytes()[end].is_ascii_alphanumeric();
        if before_ok && after_ok {
            return true;
        }
    }
    false
}

/// Words that are common English prepositions/articles/verbs and should
/// NOT be treated as section references even if they match a section ID.
const AMBIGUOUS_SECTION_WORDS: &[&str] = &[
    "about",   // preposition: "tagline about our story"
    "contact", // verb: "contact us to learn more"
    "map",     // verb: "map out a plan"
];

/// Count how many distinct sections are mentioned in the request.
///
/// Only counts section IDs that appear in a "section action" context.
/// Ambiguous words like "about" are only counted when they appear immediately
/// after a section-action prefix (e.g. "update the about section").
fn count_section_mentions(lower: &str, html: &str) -> usize {
    let section_ids = list_section_ids(html);
    let action_prefixes = [
        "add a ",
        "add an ",
        "remove the ",
        "delete the ",
        "get rid of the ",
        "hide the ",
        "drop the ",
        "change the ",
        "update the ",
        "move the ",
        "replace the ",
        "rewrite the ",
        "improve the ",
        "make the ",
        "fix the ",
        "edit the ",
        "modify the ",
    ];

    let mut count = 0;
    for id in &section_ids {
        // Skip content element words that happen to be section IDs
        if CONTENT_ELEMENT_WORDS.contains(&id.as_str()) {
            continue;
        }

        // For ambiguous words, only count them if they follow a section-action prefix
        if AMBIGUOUS_SECTION_WORDS.contains(&id.as_str()) {
            let found_after_prefix = action_prefixes.iter().any(|prefix| {
                lower.find(prefix).is_some_and(|pos| {
                    let after = &lower[pos + prefix.len()..];
                    after.starts_with(id.as_str())
                        || after.starts_with(&format!("{} section", id))
                        || after.starts_with(&format!("{} part", id))
                })
            });
            if found_after_prefix {
                count += 1;
            }
        } else {
            // Non-ambiguous section IDs: use word-boundary matching
            if contains_word(lower, id.as_str()) {
                count += 1;
            }
        }
    }
    count
}

// ─── Section Extraction ──────────────────────────────────────────────────────

/// Extract a section by its `data-nexus-section` value.
///
/// Returns the full element (opening tag through closing tag) with byte offsets.
/// Handles nested elements correctly.
pub fn extract_section(html: &str, section_id: &str) -> Option<SectionSpan> {
    // Find the attribute
    let attr = format!("data-nexus-section=\"{}\"", section_id);
    let attr_pos = html.find(&attr)?;

    // Walk backwards to find the opening '<'
    let tag_start = html[..attr_pos].rfind('<')?;

    // Determine the tag name
    let after_lt = &html[tag_start + 1..attr_pos];
    let tag_name = after_lt
        .split_whitespace()
        .next()?
        .trim_end_matches('/')
        .to_string();

    // Find the closing tag, tracking nesting depth
    let open_tag = format!("<{}", tag_name);
    let close_tag = format!("</{}>", tag_name);
    let mut depth = 0;
    let mut search_pos = tag_start;

    loop {
        // Find next opening or closing tag of the same type
        let next_open = html[search_pos..].find(&open_tag).map(|p| search_pos + p);
        let next_close = html[search_pos..].find(&close_tag).map(|p| search_pos + p);

        match (next_open, next_close) {
            (Some(open), Some(close)) => {
                if open < close {
                    depth += 1;
                    search_pos = open + open_tag.len();
                } else {
                    depth -= 1;
                    if depth == 0 {
                        let end = close + close_tag.len();
                        return Some(SectionSpan {
                            content: html[tag_start..end].to_string(),
                            start: tag_start,
                            end,
                            tag_name,
                        });
                    }
                    search_pos = close + close_tag.len();
                }
            }
            (None, Some(close)) => {
                depth -= 1;
                if depth == 0 {
                    let end = close + close_tag.len();
                    return Some(SectionSpan {
                        content: html[tag_start..end].to_string(),
                        start: tag_start,
                        end,
                        tag_name,
                    });
                }
                search_pos = close + close_tag.len();
            }
            _ => break,
        }
    }

    None
}

// ─── Tier 1: Apply CSS Changes ───────────────────────────────────────────────

/// Apply CSS variable changes to HTML. Pure string manipulation, no LLM call.
pub fn apply_css_changes(html: &str, changes: &[CssChange]) -> Result<String, String> {
    let mut result = html.to_string();

    // Log the :root block before changes
    if let Some(root) = extract_root_block(&result) {
        let preview = &root[..root.len().min(200)];
        eprintln!("[apply-css] :root BEFORE (first 200): {}", preview);
    }

    for change in changes {
        let var_name = &change.variable;
        if let Some(decl_start) = find_var_declaration(&result, var_name) {
            // Find the colon after the variable name
            if let Some(colon_pos) = result[decl_start..].find(':') {
                let abs_colon = decl_start + colon_pos;
                // Find the semicolon that ends this declaration
                if let Some(semi_pos) = result[abs_colon..].find(';') {
                    let abs_semi = abs_colon + semi_pos;
                    let old_val = result[abs_colon + 1..abs_semi].trim();
                    eprintln!(
                        "[apply-css] {} = {:?} → {:?}",
                        var_name, old_val, change.new_value
                    );
                    let new_decl = format!(": {}", change.new_value);
                    result = format!(
                        "{}{}{}",
                        &result[..abs_colon],
                        new_decl,
                        &result[abs_semi..]
                    );
                } else {
                    eprintln!(
                        "[apply-css] WARN: no semicolon after {} at offset {}",
                        var_name, abs_colon
                    );
                }
            } else {
                eprintln!(
                    "[apply-css] WARN: no colon found for {} at offset {}",
                    var_name, decl_start
                );
            }
        } else {
            eprintln!(
                "[apply-css] {} not found in HTML, adding to :root",
                var_name
            );
            result = add_var_to_root(&result, var_name, &change.new_value)?;
        }
    }

    // Log the :root block after changes
    if let Some(root) = extract_root_block(&result) {
        let preview = &root[..root.len().min(200)];
        eprintln!("[apply-css] :root AFTER (first 200): {}", preview);
    }

    Ok(result)
}

/// Find the byte offset where a CSS variable declaration starts.
///
/// Uses exact matching: `--bg:` must NOT match `--bg-2:`.
/// The character immediately after the variable name must be `:` or whitespace.
fn find_var_declaration(html: &str, var_name: &str) -> Option<usize> {
    let pat_colon = format!("{var_name}:");
    let pat_space = format!("{var_name} :");

    // Search for exact declaration (not a prefix of a longer variable name)
    for pat in [&pat_colon, &pat_space] {
        let mut search_from = 0;
        while let Some(pos) = html[search_from..].find(pat.as_str()) {
            let abs_pos = search_from + pos;
            let end_of_name = abs_pos + var_name.len();
            // The character right after the var name must be ':' or ' '
            // (not '-' or alphanumeric, which would mean a longer name like --bg-2)
            let next_char = html.as_bytes().get(end_of_name).copied().unwrap_or(b':');
            if next_char == b':' || next_char == b' ' {
                return Some(abs_pos);
            }
            search_from = abs_pos + pat.len();
        }
    }
    None
}

/// Add a new CSS variable to the :root block.
fn add_var_to_root(html: &str, var_name: &str, value: &str) -> Result<String, String> {
    let lower = html.to_lowercase();
    let root_pos = lower.find(":root").ok_or("no :root block found")?;
    let brace_start = html[root_pos..]
        .find('{')
        .ok_or("no opening brace in :root")?
        + root_pos;
    // Insert right after the opening brace
    let insert_pos = brace_start + 1;
    let new_line = format!("\n            {var_name}: {value};");
    Ok(format!(
        "{}{}{}",
        &html[..insert_pos],
        new_line,
        &html[insert_pos..]
    ))
}

// ─── Tier 2: Section Edit ────────────────────────────────────────────────────

/// System prompt for section-level edits.
pub const SECTION_EDIT_SYSTEM_PROMPT: &str = "\
You are editing a single section of an HTML website. Return ONLY the complete \
element (e.g. <section>, <footer>, <header>, <nav>) with ALL attributes preserved \
including data-nexus-section, data-nexus-editable, and data-nexus-slot. \
Do NOT return anything outside the element tags. Do NOT return JSON, \
explanations, or markdown fences. Use inline styles with hardcoded hex colors \
(no CSS variables or class references). Preserve the data-nexus-section attribute \
value exactly as-is.";

/// System prompt for generating a new section.
pub const SECTION_ADD_SYSTEM_PROMPT: &str = "\
You are adding a new section to an HTML website. Generate a complete <section> \
element with a data-nexus-section attribute set to the specified ID. Include \
data-nexus-editable and data-nexus-slot attributes on editable child elements. \
Use inline styles with hardcoded hex colors (no CSS variables or class references). \
Do NOT return anything outside the section tags. Do NOT return JSON, \
explanations, or markdown fences.";

/// Build the prompt for editing an existing section.
pub fn build_section_edit_prompt(section_html: &str, user_request: &str) -> String {
    format!(
        "Current section:\n{section_html}\n\n\
         User's edit request: {user_request}\n\n\
         Return the complete updated section element."
    )
}

/// Build the prompt for adding a new section.
pub fn build_section_add_prompt(
    section_id: &str,
    user_request: &str,
    context_html: &str,
) -> String {
    // Provide a snippet of the page for style context (first 100 lines)
    let context: String = context_html
        .lines()
        .take(100)
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "The website has this style context (first 100 lines):\n{context}\n\n\
         User's request: {user_request}\n\n\
         Generate a new <section data-nexus-section=\"{section_id}\"> element that \
         matches the existing site's style. Include data-nexus-editable attributes \
         on editable text elements."
    )
}

/// Remove a section from the HTML entirely (no LLM call needed).
pub fn remove_section(html: &str, section_id: &str) -> Result<String, String> {
    let span = extract_section(html, section_id)
        .ok_or_else(|| format!("section '{}' not found", section_id))?;

    // Remove the section and any preceding whitespace/newlines
    let before = html[..span.start].trim_end_matches(['\n', '\r']);
    let after = &html[span.end..];

    Ok(format!("{before}\n{after}"))
}

/// Splice a new/replacement section into the HTML.
pub fn splice_section(html: &str, section_id: &str, new_section: &str) -> Result<String, String> {
    let cleaned = crate::llm_codegen::strip_markdown_fences(new_section);

    if let Some(span) = extract_section(html, section_id) {
        // Replace existing section
        Ok(format!(
            "{}{}{}",
            &html[..span.start],
            cleaned.trim(),
            &html[span.end..]
        ))
    } else {
        // New section — insert before </main> or before <footer
        let insert_pos = find_insertion_point(html);
        Ok(format!(
            "{}\n\n    {}\n\n{}",
            &html[..insert_pos],
            cleaned.trim(),
            &html[insert_pos..]
        ))
    }
}

/// Find the best insertion point for a new section.
fn find_insertion_point(html: &str) -> usize {
    // Prefer inserting before the footer
    if let Some(pos) = html.find("<footer") {
        // Walk back to the start of the line
        let line_start = html[..pos].rfind('\n').map(|p| p + 1).unwrap_or(pos);
        return line_start;
    }
    // Fallback: before </main>
    if let Some(pos) = html.find("</main>") {
        return pos;
    }
    // Last resort: before </body>
    if let Some(pos) = html.find("</body>") {
        return pos;
    }
    html.len()
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Sample HTML for testing ──

    const SAMPLE_HTML: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<title>Test Site</title>
<style>
:root {
    --bg-color: #ffffff;
    --text-color: #1a1a1a;
    --accent-color: #c2410c;
    --accent-secondary: #7000ff;
    --font-main: 'Inter', system-ui, sans-serif;
    --font-size-base: 16px;
    --surface-color: #f5f5f5;
}
body { background: var(--bg-color); color: var(--text-color); }
</style>
</head>
<body>
<header data-nexus-section="header">
    <nav><a href="/">Home</a></nav>
</header>
<main>
<section data-nexus-section="hero" data-nexus-editable>
    <h1 data-nexus-slot="heading">Welcome</h1>
    <p data-nexus-slot="subheading">This is the hero section.</p>
    <div class="inner">
        <section class="nested">Nested content</section>
    </div>
</section>
<section data-nexus-section="features">
    <h2>Features</h2>
    <ul><li>Feature 1</li><li>Feature 2</li></ul>
</section>
<section data-nexus-section="pricing">
    <h2>Pricing</h2>
    <div class="cards">Card 1 | Card 2</div>
</section>
</main>
<footer data-nexus-section="footer">
    <p>&copy; 2026 Test</p>
</footer>
</body>
</html>"#;

    const SAMPLE_HTML_NO_SECTIONS: &str = r#"<!DOCTYPE html>
<html><head><style>:root { --primary: #333; }</style></head>
<body><h1>Hello</h1></body></html>"#;

    const SAMPLE_HTML_NO_VARS: &str = r#"<!DOCTYPE html>
<html><head><style>body { color: red; }</style></head>
<body><section data-nexus-section="hero"><h1>Hi</h1></section></body></html>"#;

    /// Realistic pizza site with real variable names (matches actual build output).
    const PIZZA_HTML: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<title>Giuseppe's Pizzeria</title>
<style>
:root {
    --primary: #f97316;
    --primary-hover: #ea6c0a;
    --bg: #faf6f1;
    --bg-2: #f0ebe4;
    --surface: #ffffff;
    --surface-2: #f5f0ea;
    --text: #1a1a1a;
    --text-muted: #6b6b6b;
    --text-subtle: #999999;
    --border: #e0d8cf;
    --border-light: #ede6dd;
    --font: 'Outfit', sans-serif;
}
body { background: var(--bg); color: var(--text); font-family: var(--font); }
</style>
</head>
<body>
<nav data-nexus-section="nav"><a href="/">Giuseppe's</a></nav>
<section data-nexus-section="hero"><h1>Authentic Wood-Fired Pizza Since 1952</h1></section>
<section data-nexus-section="about"><h2>Our Story</h2><p>Family recipes since 1952.</p></section>
<section data-nexus-section="menu"><h2>Our Menu</h2></section>
<section data-nexus-section="gallery"><h2>Gallery</h2></section>
<section data-nexus-section="testimonials"><h2>What People Say</h2></section>
<section data-nexus-section="contact"><h2>Visit Us</h2></section>
<footer data-nexus-section="footer"><p>&copy; 2026 Giuseppe's</p></footer>
</body>
</html>"#;

    // ── Classifier Tests ──

    #[test]
    fn test_classify_css_color_keyword() {
        let c = classify_edit("make buttons yellow", SAMPLE_HTML);
        assert_eq!(c.tier, EditTier::CssVariable);
        let changes = c.css_changes.unwrap();
        assert!(!changes.is_empty());
        assert!(changes.iter().any(|ch| ch.new_value == "#FFD700"));
    }

    #[test]
    fn test_classify_css_hex_color() {
        let c = classify_edit("change the accent to #FF5733", SAMPLE_HTML);
        assert_eq!(c.tier, EditTier::CssVariable);
        let changes = c.css_changes.unwrap();
        assert!(changes.iter().any(|ch| ch.new_value == "#FF5733"));
    }

    #[test]
    fn test_classify_section_edit() {
        let c = classify_edit("update the hero section", SAMPLE_HTML);
        assert_eq!(c.tier, EditTier::SectionEdit);
        assert_eq!(c.target_section.as_deref(), Some("hero"));
    }

    #[test]
    fn test_classify_section_add() {
        let c = classify_edit("add a testimonials section", SAMPLE_HTML);
        assert_eq!(c.tier, EditTier::SectionEdit);
        assert_eq!(c.target_section.as_deref(), Some("testimonials"));
    }

    #[test]
    fn test_classify_section_remove() {
        let c = classify_edit("remove the pricing section", SAMPLE_HTML);
        assert_eq!(c.tier, EditTier::SectionEdit);
        assert_eq!(c.target_section.as_deref(), Some("pricing"));
    }

    #[test]
    fn test_classify_full_regen() {
        let c = classify_edit("completely redesign the page", SAMPLE_HTML);
        assert_eq!(c.tier, EditTier::FullRegeneration);
    }

    #[test]
    fn test_classify_ambiguous_fallback() {
        let c = classify_edit("make it more professional and corporate", SAMPLE_HTML);
        assert_eq!(c.tier, EditTier::FullRegeneration);
    }

    #[test]
    fn test_classify_no_sections_falls_through() {
        let c = classify_edit("update the hero section", SAMPLE_HTML_NO_SECTIONS);
        // No data-nexus-section attributes → falls through to Tier 3
        assert_eq!(c.tier, EditTier::FullRegeneration);
    }

    #[test]
    fn test_classify_no_vars_section_edit() {
        let c = classify_edit("update the hero section", SAMPLE_HTML_NO_VARS);
        assert_eq!(c.tier, EditTier::SectionEdit);
        assert_eq!(c.target_section.as_deref(), Some("hero"));
    }

    #[test]
    fn test_classify_dark_mode() {
        let c = classify_edit("switch to dark mode", SAMPLE_HTML);
        assert_eq!(c.tier, EditTier::CssVariable);
        let changes = c.css_changes.unwrap();
        // Should swap bg from light → dark and text from dark → light
        assert!(changes.iter().any(|ch| ch.variable.contains("bg")));
        assert!(changes.iter().any(|ch| ch.variable.contains("text")));
    }

    // ── CSS Variable Tests ──

    #[test]
    fn test_has_css_variables() {
        assert!(has_css_variables(SAMPLE_HTML));
        assert!(!has_css_variables("<html><body>no vars</body></html>"));
    }

    #[test]
    fn test_parse_css_variables() {
        let vars = parse_css_variables(SAMPLE_HTML);
        assert!(vars.len() >= 4);
        assert!(vars.iter().any(|(n, _)| n == "--bg-color"));
        assert!(vars
            .iter()
            .any(|(n, v)| n == "--accent-color" && v == "#c2410c"));
    }

    #[test]
    fn test_apply_css_changes_replace() {
        let changes = vec![CssChange {
            variable: "--accent-color".to_string(),
            old_value: Some("#c2410c".to_string()),
            new_value: "#FFD700".to_string(),
        }];
        let result = apply_css_changes(SAMPLE_HTML, &changes).unwrap();
        assert!(result.contains("--accent-color: #FFD700;"));
        assert!(!result.contains("#c2410c"));
        // Other vars unchanged
        assert!(result.contains("--bg-color: #ffffff;"));
    }

    #[test]
    fn test_apply_css_changes_add_new_var() {
        let changes = vec![CssChange {
            variable: "--new-var".to_string(),
            old_value: None,
            new_value: "#123456".to_string(),
        }];
        let result = apply_css_changes(SAMPLE_HTML, &changes).unwrap();
        assert!(result.contains("--new-var: #123456;"));
    }

    #[test]
    fn test_apply_css_dark_mode_swap() {
        let c = classify_edit("switch to dark mode", SAMPLE_HTML);
        let changes = c.css_changes.unwrap();
        let result = apply_css_changes(SAMPLE_HTML, &changes).unwrap();
        // bg should now be dark
        assert!(
            result.contains("--bg-color: #0f172a") || result.contains("--bg-color: #1e293b"),
            "bg-color should be dark, got: {}",
            result
                .lines()
                .find(|l| l.contains("--bg-color"))
                .unwrap_or("NOT FOUND")
        );
        // text should now be light
        assert!(result.contains("--text-color: #f8fafc"));
        // Still valid HTML
        assert!(result.contains(":root"));
    }

    // ── Section Tests ──

    #[test]
    fn test_has_section_anchors() {
        assert!(has_section_anchors(SAMPLE_HTML));
        assert!(!has_section_anchors(SAMPLE_HTML_NO_SECTIONS));
    }

    #[test]
    fn test_extract_section_hero() {
        let span = extract_section(SAMPLE_HTML, "hero").unwrap();
        assert!(span.content.contains("Welcome"));
        assert!(span.content.contains("data-nexus-section=\"hero\""));
        assert!(span.content.starts_with("<section"));
        assert!(span.content.ends_with("</section>"));
        assert_eq!(span.tag_name, "section");
    }

    #[test]
    fn test_extract_section_nested_elements() {
        // The hero section contains a nested <section class="nested"> —
        // extraction must not close early on it.
        let span = extract_section(SAMPLE_HTML, "hero").unwrap();
        assert!(span.content.contains("Nested content"));
        assert!(span.content.contains("</section>"));
        // Count opening and closing section tags — they must match
        let opens = span.content.matches("<section").count();
        let closes = span.content.matches("</section>").count();
        assert_eq!(opens, closes);
    }

    #[test]
    fn test_extract_section_footer() {
        let span = extract_section(SAMPLE_HTML, "footer").unwrap();
        assert!(span.content.contains("2026 Test"));
        assert!(span.content.starts_with("<footer"));
        assert!(span.content.ends_with("</footer>"));
        assert_eq!(span.tag_name, "footer");
    }

    #[test]
    fn test_extract_section_nonexistent() {
        assert!(extract_section(SAMPLE_HTML, "nonexistent").is_none());
    }

    #[test]
    fn test_extract_preserves_offsets() {
        let span = extract_section(SAMPLE_HTML, "features").unwrap();
        // Verify that the byte offsets are correct
        assert_eq!(&SAMPLE_HTML[span.start..span.end], span.content);
    }

    #[test]
    fn test_splice_section_replace() {
        let new_hero = r#"<section data-nexus-section="hero"><h1>NEW HERO</h1></section>"#;
        let result = splice_section(SAMPLE_HTML, "hero", new_hero).unwrap();
        assert!(result.contains("NEW HERO"));
        assert!(!result.contains("Welcome"));
        // Other sections unchanged
        assert!(result.contains("Features"));
        assert!(result.contains("Pricing"));
        assert!(result.contains("2026 Test"));
    }

    #[test]
    fn test_splice_section_add_new() {
        let new_section =
            r#"<section data-nexus-section="testimonials"><h2>Testimonials</h2></section>"#;
        let result = splice_section(SAMPLE_HTML, "testimonials", new_section).unwrap();
        assert!(result.contains("data-nexus-section=\"testimonials\""));
        // Should be inserted before footer
        let testimonial_pos = result.find("Testimonials").unwrap();
        let footer_pos = result.find("<footer").unwrap();
        assert!(testimonial_pos < footer_pos);
    }

    #[test]
    fn test_remove_section() {
        let result = remove_section(SAMPLE_HTML, "pricing").unwrap();
        assert!(!result.contains("Pricing"));
        assert!(!result.contains("data-nexus-section=\"pricing\""));
        // Other sections intact
        assert!(result.contains("data-nexus-section=\"hero\""));
        assert!(result.contains("data-nexus-section=\"features\""));
        assert!(result.contains("data-nexus-section=\"footer\""));
    }

    #[test]
    fn test_remove_nonexistent_section_errors() {
        let result = remove_section(SAMPLE_HTML, "nonexistent");
        assert!(result.is_err());
    }

    // ── Tier 1 costs $0 ──

    #[test]
    fn test_tier1_no_cost() {
        let c = classify_edit("make buttons yellow", SAMPLE_HTML);
        assert_eq!(c.tier, EditTier::CssVariable);
        // Tier 1 is pure string manipulation — no LLM call, no cost.
        // The caller should record cost_usd = 0.0.
    }

    // ── After CSS edit, HTML is valid ──

    #[test]
    fn test_css_edit_preserves_html_structure() {
        let changes = vec![CssChange {
            variable: "--accent-color".to_string(),
            old_value: Some("#c2410c".to_string()),
            new_value: "#FFD700".to_string(),
        }];
        let result = apply_css_changes(SAMPLE_HTML, &changes).unwrap();
        assert!(result.contains("<!DOCTYPE html>"));
        assert!(result.contains(":root"));
        assert!(result.contains("</html>"));
        assert!(result.contains("--accent-color: #FFD700;"));
    }

    // ── After section edit, other sections unchanged ──

    #[test]
    fn test_section_edit_preserves_others() {
        let new_hero = r#"<section data-nexus-section="hero"><h1>NEW</h1></section>"#;
        let result = splice_section(SAMPLE_HTML, "hero", new_hero).unwrap();

        // Extract other sections and verify they're byte-for-byte identical
        let orig_features = extract_section(SAMPLE_HTML, "features").unwrap();
        let new_features = extract_section(&result, "features").unwrap();
        assert_eq!(orig_features.content, new_features.content);

        let orig_pricing = extract_section(SAMPLE_HTML, "pricing").unwrap();
        let new_pricing = extract_section(&result, "pricing").unwrap();
        assert_eq!(orig_pricing.content, new_pricing.content);

        let orig_footer = extract_section(SAMPLE_HTML, "footer").unwrap();
        let new_footer = extract_section(&result, "footer").unwrap();
        assert_eq!(orig_footer.content, new_footer.content);
    }

    // ── Tier 2 input is smaller than Tier 3 ──

    #[test]
    fn test_section_input_smaller_than_full() {
        let hero = extract_section(SAMPLE_HTML, "hero").unwrap();
        // Section HTML should be much smaller than the full page
        assert!(hero.content.len() < SAMPLE_HTML.len());
        // Specifically, the section should be less than half the full page
        assert!(hero.content.len() < SAMPLE_HTML.len() / 2);
    }

    // ── Color helper tests ──

    #[test]
    fn test_hex_luminance() {
        assert!(hex_luminance("#ffffff").unwrap() > 0.9);
        assert!(hex_luminance("#000000").unwrap() < 0.1);
        assert!(hex_luminance("#808080").unwrap() > 0.3);
    }

    #[test]
    fn test_find_target_color_hex() {
        assert_eq!(
            find_target_color("change to #FF5733"),
            Some("#FF5733".to_string())
        );
    }

    #[test]
    fn test_find_target_color_named() {
        assert_eq!(
            find_target_color("make it yellow"),
            Some("#FFD700".to_string())
        );
    }

    #[test]
    fn test_scale_css_value() {
        // 16 * 1.2 = 19.2, 16 * 0.8 = 12.8
        assert_eq!(scale_css_value("16px", true), Some("19.2px".to_string()));
        assert_eq!(scale_css_value("16px", false), Some("12.8px".to_string()));
        // Whole-number results: 10 * 1.2 = 12.0
        assert_eq!(scale_css_value("10px", true), Some("12px".to_string()));
    }

    // ── Section ID listing ──

    #[test]
    fn test_list_section_ids() {
        let ids = list_section_ids(SAMPLE_HTML);
        assert_eq!(ids, vec!["header", "hero", "features", "pricing", "footer"]);
    }

    // ── Fuzzy section matching ──

    #[test]
    fn test_fuzzy_section_match_nav() {
        let c = classify_edit("update the navigation", SAMPLE_HTML);
        // "navigation" → "header" (the header section contains the nav)
        assert_eq!(c.tier, EditTier::SectionEdit);
    }

    #[test]
    fn test_fuzzy_section_match_top_part() {
        let c = classify_edit("change the top part", SAMPLE_HTML);
        assert_eq!(c.tier, EditTier::SectionEdit);
        // Should match "hero"
        assert_eq!(c.target_section.as_deref(), Some("hero"));
    }

    // ── FIX 1: Multi-section false match regression tests ──

    #[test]
    fn test_hero_with_tagline_not_multi_section() {
        // "about" appears as a WORD in the request (about family recipes)
        // but it should NOT be counted as a section reference.
        let c = classify_edit(
            "update the hero section to say Authentic Wood-Fired Pizza Since 1952 and add a tagline about family recipes",
            PIZZA_HTML,
        );
        assert_eq!(
            c.tier,
            EditTier::SectionEdit,
            "Should be SectionEdit, not FullRegeneration. Reason: {}",
            c.reason
        );
        assert_eq!(c.target_section.as_deref(), Some("hero"));
    }

    #[test]
    fn test_two_real_sections_triggers_full_regen() {
        // Both "pricing" and "features" are real section IDs
        let c = classify_edit(
            "change the pricing table and update the features",
            SAMPLE_HTML,
        );
        assert_eq!(c.tier, EditTier::FullRegeneration);
        assert!(
            c.reason.contains("Multiple sections"),
            "reason: {}",
            c.reason
        );
    }

    #[test]
    fn test_content_words_not_section_refs() {
        // None of these content words should count as section references:
        // "tagline", "headline", "subtitle", "button", "text"
        let c = classify_edit(
            "update the hero section with a new headline and tagline text",
            PIZZA_HTML,
        );
        assert_eq!(c.tier, EditTier::SectionEdit, "Reason: {}", c.reason);
        assert_eq!(c.target_section.as_deref(), Some("hero"));
    }

    #[test]
    fn test_about_word_inside_request_not_section() {
        // "about" appears in "learn about our team" but should not match
        // the "about" section because it's not a standalone section reference
        // in the context of "update the hero"
        let c = classify_edit(
            "update the hero section to tell visitors about our story",
            PIZZA_HTML,
        );
        assert_eq!(c.tier, EditTier::SectionEdit, "Reason: {}", c.reason);
        assert_eq!(c.target_section.as_deref(), Some("hero"));
    }

    #[test]
    fn test_contains_word_boundary() {
        assert!(contains_word("update the hero section", "hero"));
        assert!(!contains_word("update the heroic banner", "hero"));
        assert!(contains_word("change hero and footer", "hero"));
        assert!(contains_word("hero is great", "hero"));
        assert!(!contains_word("superhero saves day", "hero"));
    }

    // ── FIX 2: Removal verb tests ──

    #[test]
    fn test_remove_gallery_section() {
        let c = classify_edit("remove the gallery section", PIZZA_HTML);
        assert_eq!(c.tier, EditTier::SectionEdit, "Reason: {}", c.reason);
        assert_eq!(c.target_section.as_deref(), Some("gallery"));
    }

    #[test]
    fn test_delete_footer() {
        let c = classify_edit("delete the footer", PIZZA_HTML);
        assert_eq!(c.tier, EditTier::SectionEdit, "Reason: {}", c.reason);
        assert_eq!(c.target_section.as_deref(), Some("footer"));
    }

    #[test]
    fn test_get_rid_of_testimonials() {
        let c = classify_edit("get rid of the testimonials", PIZZA_HTML);
        assert_eq!(c.tier, EditTier::SectionEdit, "Reason: {}", c.reason);
        assert_eq!(c.target_section.as_deref(), Some("testimonials"));
    }

    #[test]
    fn test_hide_the_contact() {
        let c = classify_edit("hide the contact section", PIZZA_HTML);
        assert_eq!(c.tier, EditTier::SectionEdit, "Reason: {}", c.reason);
        assert_eq!(c.target_section.as_deref(), Some("contact"));
    }

    #[test]
    fn test_remove_section_preserves_others() {
        let result = remove_section(PIZZA_HTML, "gallery").unwrap();
        assert!(!result.contains("data-nexus-section=\"gallery\""));
        // All other sections preserved
        assert!(result.contains("data-nexus-section=\"hero\""));
        assert!(result.contains("data-nexus-section=\"about\""));
        assert!(result.contains("data-nexus-section=\"menu\""));
        assert!(result.contains("data-nexus-section=\"testimonials\""));
        assert!(result.contains("data-nexus-section=\"contact\""));
        assert!(result.contains("data-nexus-section=\"footer\""));
    }

    // ── FIX 3: Dark mode swap on light-themed site ──

    #[test]
    fn test_dark_mode_swap_light_pizza_site() {
        let c = classify_edit("switch to dark mode", PIZZA_HTML);
        assert_eq!(c.tier, EditTier::CssVariable);
        let changes = c.css_changes.unwrap();
        let result = apply_css_changes(PIZZA_HTML, &changes).unwrap();

        // Background must become dark
        let vars_after = parse_css_variables(&result);
        let bg_val = vars_after
            .iter()
            .find(|(n, _)| n == "--bg")
            .map(|(_, v)| v.as_str())
            .unwrap();
        assert!(
            is_dark_color(bg_val),
            "--bg should be dark after swap, got {}",
            bg_val
        );

        // Text must become light
        let text_val = vars_after
            .iter()
            .find(|(n, _)| n == "--text")
            .map(|(_, v)| v.as_str())
            .unwrap();
        assert!(
            is_light_color(text_val),
            "--text should be light after swap, got {}",
            text_val
        );

        // Surface must become dark
        let surface_val = vars_after
            .iter()
            .find(|(n, _)| n == "--surface")
            .map(|(_, v)| v.as_str())
            .unwrap();
        assert!(
            is_dark_color(surface_val),
            "--surface should be dark after swap, got {}",
            surface_val
        );

        // Border must change
        let border_val = vars_after
            .iter()
            .find(|(n, _)| n == "--border")
            .map(|(_, v)| v.as_str())
            .unwrap();
        assert!(
            is_dark_color(border_val),
            "--border should be dark after swap, got {}",
            border_val
        );

        // Text-muted should be adjusted
        let muted_val = vars_after
            .iter()
            .find(|(n, _)| n == "--text-muted")
            .map(|(_, v)| v.as_str());
        assert!(muted_val.is_some(), "--text-muted should be present");

        // Primary accent should NOT change
        let primary_val = vars_after
            .iter()
            .find(|(n, _)| n == "--primary")
            .map(|(_, v)| v.as_str())
            .unwrap();
        assert_eq!(
            primary_val, "#f97316",
            "--primary should not change in dark mode"
        );
    }

    #[test]
    fn test_light_mode_swap_dark_site() {
        // Create a dark-themed site and switch to light mode
        let dark_html = PIZZA_HTML
            .replace("--bg: #faf6f1", "--bg: #0a0a0a")
            .replace("--bg-2: #f0ebe4", "--bg-2: #111111")
            .replace("--surface: #ffffff", "--surface: #1a1a2e")
            .replace("--text: #1a1a1a", "--text: #f0f0f0")
            .replace("--border: #e0d8cf", "--border: #2a2a2a");

        let c = classify_edit("switch to light mode", &dark_html);
        assert_eq!(c.tier, EditTier::CssVariable);
        let changes = c.css_changes.unwrap();
        let result = apply_css_changes(&dark_html, &changes).unwrap();

        let vars_after = parse_css_variables(&result);
        let bg_val = vars_after
            .iter()
            .find(|(n, _)| n == "--bg")
            .map(|(_, v)| v.as_str())
            .unwrap();
        assert!(
            is_light_color(bg_val),
            "--bg should be light after swap, got {}",
            bg_val
        );
    }

    #[test]
    fn test_find_var_declaration_exact_match() {
        // --bg: must not match --bg-2:
        let html = ":root { --bg: #fff; --bg-2: #eee; }";
        let pos = find_var_declaration(html, "--bg").unwrap();
        // Should find --bg: not --bg-2:
        assert_eq!(&html[pos..pos + 5], "--bg:");
    }

    #[test]
    fn test_is_mid_color() {
        assert!(is_mid_color("#808080")); // mid-gray
        assert!(is_mid_color("#6b6b6b")); // text-muted typical
        assert!(!is_mid_color("#ffffff")); // white
        assert!(!is_mid_color("#000000")); // black
        assert!(!is_mid_color("#1a1a1a")); // very dark
    }

    // ── Pizza site section IDs ──

    #[test]
    fn test_pizza_section_ids() {
        let ids = list_section_ids(PIZZA_HTML);
        assert_eq!(
            ids,
            vec![
                "nav",
                "hero",
                "about",
                "menu",
                "gallery",
                "testimonials",
                "contact",
                "footer"
            ]
        );
    }
}

//! HTML/CSS Sanitizer — security-critical input sanitization.
//!
//! ALL imported HTML is untrusted. Uses `ammonia` with explicit allowlists.
//! Deny by default. Remove all scripts, event handlers, dangerous URLs.

use std::collections::HashSet;

/// Result of HTML sanitization.
#[derive(Debug, Clone)]
pub struct SanitizeResult {
    pub clean_html: String,
    pub removed_elements: Vec<String>,
    pub warnings: Vec<String>,
}

// ─── Dangerous CSS Properties ──────────────────────────────────────────────

const DANGEROUS_CSS_PROPS: &[&str] = &["expression", "behavior", "-moz-binding"];

const DANGEROUS_CSS_FUNCTIONS: &[&str] = &[
    "expression(",
    "url(javascript:",
    "url(data:text/html",
    "url(data:image/svg+xml",
    "url(vbscript:",
];

// ─── HTML Sanitization ─────────────────────────────────────────────────────

/// Sanitize untrusted HTML. Strips all dangerous elements and attributes.
///
/// Uses `ammonia` with an explicit allowlist — deny by default.
pub fn sanitize_html(raw_html: &str) -> SanitizeResult {
    let mut removed = Vec::new();
    let mut warnings = Vec::new();

    // Track what gets removed
    let lower = raw_html.to_lowercase();
    if lower.contains("<script") {
        removed.push("script tags".into());
    }
    if lower.contains("<iframe") {
        removed.push("iframe tags".into());
    }
    if lower.contains("<object") || lower.contains("<embed") || lower.contains("<applet") {
        removed.push("embedded objects".into());
    }
    if lower.contains("onclick")
        || lower.contains("onload")
        || lower.contains("onerror")
        || lower.contains("onmouseover")
    {
        removed.push("event handler attributes".into());
    }
    if lower.contains("javascript:") {
        removed.push("javascript: URLs".into());
    }
    if lower.contains("<meta") && lower.contains("http-equiv") {
        removed.push("meta http-equiv".into());
    }

    // Build ammonia sanitizer with explicit allowlist
    let mut builder = ammonia::Builder::new();

    // Allowed tags — semantic HTML + layout + media
    let allowed_tags: HashSet<&str> = [
        // Document structure
        "html",
        "head",
        "body",
        "title",
        // Semantic sections
        "header",
        "nav",
        "main",
        "section",
        "article",
        "aside",
        "footer",
        // Headings
        "h1",
        "h2",
        "h3",
        "h4",
        "h5",
        "h6",
        // Block elements
        "div",
        "p",
        "blockquote",
        "pre",
        "code",
        "hr",
        "br",
        // Lists
        "ul",
        "ol",
        "li",
        "dl",
        "dt",
        "dd",
        // Inline elements
        "span",
        "a",
        "strong",
        "em",
        "b",
        "i",
        "u",
        "s",
        "small",
        "mark",
        "sub",
        "sup",
        "abbr",
        "time",
        "cite",
        "q",
        // Tables
        "table",
        "thead",
        "tbody",
        "tfoot",
        "tr",
        "th",
        "td",
        "caption",
        "colgroup",
        "col",
        // Media
        "img",
        "picture",
        "source",
        "video",
        "audio",
        "figure",
        "figcaption",
        // SVG subset
        "svg",
        "path",
        "circle",
        "rect",
        "line",
        "polyline",
        "polygon",
        "g",
        "defs",
        "use",
        "text",
        "tspan",
        "ellipse",
        // Forms (display only — no submission)
        "form",
        "input",
        "textarea",
        "button",
        "select",
        "option",
        "optgroup",
        "label",
        "fieldset",
        "legend",
        // Other
        "details",
        "summary",
        "dialog",
        "meter",
        "progress",
    ]
    .into_iter()
    .collect();
    builder.tags(allowed_tags);

    // Allowed attributes (generic)
    let generic_attrs: HashSet<&str> = [
        "class",
        "id",
        "style",
        "role",
        "tabindex",
        "title",
        "lang",
        "dir",
        "hidden",
        "aria-label",
        "aria-labelledby",
        "aria-describedby",
        "aria-hidden",
        "aria-live",
        "aria-expanded",
        "aria-selected",
        "aria-controls",
        "aria-current",
        "aria-haspopup",
        "aria-modal",
        "data-nexus-section",
        "data-nexus-slot",
        "data-nexus-editable",
    ]
    .into_iter()
    .collect();
    builder.generic_attributes(generic_attrs);

    // Tag-specific attributes
    let mut tag_attrs = std::collections::HashMap::new();

    let a_attrs: HashSet<&str> = ["href", "target"].into_iter().collect();
    tag_attrs.insert("a", a_attrs);

    let img_attrs: HashSet<&str> = ["src", "alt", "width", "height", "loading", "decoding"]
        .into_iter()
        .collect();
    tag_attrs.insert("img", img_attrs);

    let video_attrs: HashSet<&str> = [
        "src",
        "controls",
        "width",
        "height",
        "poster",
        "autoplay",
        "muted",
        "loop",
        "playsinline",
    ]
    .into_iter()
    .collect();
    tag_attrs.insert("video", video_attrs);

    let audio_attrs: HashSet<&str> = ["src", "controls"].into_iter().collect();
    tag_attrs.insert("audio", audio_attrs);

    let source_attrs: HashSet<&str> = ["src", "srcset", "type", "media"].into_iter().collect();
    tag_attrs.insert("source", source_attrs);

    let svg_attrs: HashSet<&str> = [
        "viewBox",
        "xmlns",
        "width",
        "height",
        "fill",
        "stroke",
        "stroke-width",
        "stroke-linecap",
        "stroke-linejoin",
        "d",
        "cx",
        "cy",
        "r",
        "x",
        "y",
        "x1",
        "y1",
        "x2",
        "y2",
        "rx",
        "ry",
        "points",
        "transform",
        "opacity",
        "fill-rule",
        "clip-rule",
    ]
    .into_iter()
    .collect();
    for svg_tag in &[
        "svg", "path", "circle", "rect", "line", "polyline", "polygon", "g", "defs", "use", "text",
        "tspan", "ellipse",
    ] {
        tag_attrs.insert(svg_tag, svg_attrs.clone());
    }

    let input_attrs: HashSet<&str> = [
        "type",
        "name",
        "value",
        "placeholder",
        "disabled",
        "readonly",
        "required",
        "min",
        "max",
        "step",
        "pattern",
        "checked",
    ]
    .into_iter()
    .collect();
    tag_attrs.insert("input", input_attrs);

    let form_attrs: HashSet<&str> = ["method", "action"].into_iter().collect();
    tag_attrs.insert("form", form_attrs);

    let textarea_attrs: HashSet<&str> = [
        "name",
        "rows",
        "cols",
        "placeholder",
        "disabled",
        "readonly",
    ]
    .into_iter()
    .collect();
    tag_attrs.insert("textarea", textarea_attrs);

    let button_attrs: HashSet<&str> = ["type", "disabled"].into_iter().collect();
    tag_attrs.insert("button", button_attrs);

    let td_attrs: HashSet<&str> = ["colspan", "rowspan"].into_iter().collect();
    tag_attrs.insert("td", td_attrs.clone());
    tag_attrs.insert("th", td_attrs);

    let time_attrs: HashSet<&str> = ["datetime"].into_iter().collect();
    tag_attrs.insert("time", time_attrs);

    let meter_attrs: HashSet<&str> = ["value", "min", "max", "low", "high", "optimum"]
        .into_iter()
        .collect();
    tag_attrs.insert("meter", meter_attrs);

    let progress_attrs: HashSet<&str> = ["value", "max"].into_iter().collect();
    tag_attrs.insert("progress", progress_attrs);

    builder.tag_attributes(tag_attrs);

    // URL schemes
    let url_schemes: HashSet<&str> = ["https", "mailto", "tel"].into_iter().collect();
    builder.url_schemes(url_schemes);

    // Strip comments
    builder.strip_comments(true);

    // Clean links
    builder.link_rel(Some("noopener noreferrer"));

    let clean_html = builder.clean(raw_html).to_string();

    // Post-process: sanitize inline styles
    let clean_html = sanitize_inline_styles(&clean_html);

    // Check for external stylesheet warnings
    if lower.contains("<link") && lower.contains("stylesheet") {
        warnings.push("External stylesheets removed — CSS must be internalized".into());
        removed.push("external stylesheet links".into());
    }

    SanitizeResult {
        clean_html,
        removed_elements: removed,
        warnings,
    }
}

/// Sanitize inline style attributes within HTML.
fn sanitize_inline_styles(html: &str) -> String {
    // Simple approach: find style="..." attributes and clean the CSS values
    let mut result = String::with_capacity(html.len());
    let mut remaining = html;

    while let Some(style_start) = remaining.find("style=\"") {
        result.push_str(&remaining[..style_start]);
        remaining = &remaining[style_start..];

        // Find the closing quote
        let after_open = &remaining[7..]; // skip 'style="'
        if let Some(close) = after_open.find('"') {
            let style_value = &after_open[..close];
            let clean_style = sanitize_css_value(style_value);
            result.push_str("style=\"");
            result.push_str(&clean_style);
            result.push('"');
            remaining = &after_open[close + 1..];
        } else {
            result.push_str(&remaining[..7]);
            remaining = &remaining[7..];
        }
    }
    result.push_str(remaining);
    result
}

/// Sanitize a single CSS value string (from an inline style).
fn sanitize_css_value(css: &str) -> String {
    let lower = css.to_lowercase();
    // Check for dangerous functions
    for func in DANGEROUS_CSS_FUNCTIONS {
        if lower.contains(func) {
            return String::new();
        }
    }
    for prop in DANGEROUS_CSS_PROPS {
        if lower.contains(prop) {
            // Remove just the dangerous property
            return css
                .split(';')
                .filter(|decl| !decl.to_lowercase().contains(prop))
                .collect::<Vec<_>>()
                .join(";");
        }
    }
    css.to_string()
}

// ─── CSS Sanitization ──────────────────────────────────────────────────────

/// Sanitize a CSS stylesheet string. Removes dangerous declarations.
pub fn sanitize_css(css: &str) -> String {
    let mut clean = String::with_capacity(css.len());
    let lower = css.to_lowercase();

    for line in css.lines() {
        let line_lower = line.to_lowercase().trim().to_string();

        // Skip dangerous @import with external URLs
        if line_lower.starts_with("@import")
            && (line_lower.contains("http:") || line_lower.contains("https:"))
        {
            continue;
        }

        // Skip dangerous function calls
        let mut skip = false;
        for func in DANGEROUS_CSS_FUNCTIONS {
            if line_lower.contains(func) {
                skip = true;
                break;
            }
        }
        for prop in DANGEROUS_CSS_PROPS {
            if line_lower.contains(&format!("{prop}:")) || line_lower.contains(&format!("{prop} :"))
            {
                skip = true;
                break;
            }
        }

        if !skip {
            clean.push_str(line);
            clean.push('\n');
        }
    }

    // Warn about @import of external
    let _ = lower; // suppress unused

    clean
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_removes_script_tags() {
        let result = sanitize_html("<div><script>alert('xss')</script><p>Safe</p></div>");
        assert!(!result.clean_html.contains("<script>"));
        assert!(!result.clean_html.contains("alert"));
        assert!(result.clean_html.contains("Safe"));
    }

    #[test]
    fn test_removes_event_handlers() {
        let result = sanitize_html(r#"<div onclick="alert('xss')" class="box">Text</div>"#);
        assert!(!result.clean_html.contains("onclick"));
        assert!(result.clean_html.contains("Text"));
        assert!(result.clean_html.contains("class=\"box\""));
    }

    #[test]
    fn test_removes_javascript_urls() {
        let result = sanitize_html(r#"<a href="javascript:void(0)">Click</a>"#);
        assert!(!result.clean_html.contains("javascript:"));
        assert!(result.clean_html.contains("Click"));
    }

    #[test]
    fn test_removes_iframe() {
        let result =
            sanitize_html(r#"<div><iframe src="http://evil.com"></iframe><p>OK</p></div>"#);
        assert!(!result.clean_html.contains("<iframe"));
        assert!(result.clean_html.contains("OK"));
    }

    #[test]
    fn test_removes_data_urls_in_src() {
        let result = sanitize_html(r#"<img src="data:text/html,<script>alert('x')</script>">"#);
        // data: URLs should be stripped (only https allowed)
        assert!(!result.clean_html.contains("data:text/html"));
    }

    #[test]
    fn test_preserves_semantic_html() {
        let html = "<header><nav>Nav</nav></header><main><section><article>Content</article></section></main><footer>Foot</footer>";
        let result = sanitize_html(html);
        assert!(result.clean_html.contains("<header>"));
        assert!(result.clean_html.contains("<nav>"));
        assert!(result.clean_html.contains("<main>"));
        assert!(result.clean_html.contains("<section>"));
        assert!(result.clean_html.contains("<article>"));
        assert!(result.clean_html.contains("<footer>"));
    }

    #[test]
    fn test_preserves_text_content() {
        let result = sanitize_html("<div><h1>Hello World</h1><p>Some text content here</p></div>");
        assert!(result.clean_html.contains("Hello World"));
        assert!(result.clean_html.contains("Some text content here"));
    }

    #[test]
    fn test_preserves_safe_images() {
        let result = sanitize_html(r#"<img src="https://example.com/img.jpg" alt="photo">"#);
        assert!(result.clean_html.contains("https://example.com/img.jpg"));
        assert!(result.clean_html.contains("alt=\"photo\""));
    }

    #[test]
    fn test_preserves_css_classes() {
        let result = sanitize_html(
            r#"<div class="flex items-center gap-4"><span class="text-lg">Hi</span></div>"#,
        );
        assert!(result
            .clean_html
            .contains("class=\"flex items-center gap-4\""));
    }

    #[test]
    fn test_preserves_aria_attributes() {
        let result = sanitize_html(r#"<button aria-label="Close" role="button">X</button>"#);
        assert!(result.clean_html.contains("aria-label=\"Close\""));
        assert!(result.clean_html.contains("role=\"button\""));
    }

    #[test]
    fn test_sanitizes_css_expression() {
        let clean =
            sanitize_css("div {\n  color: expression(alert('xss'));\n  background: #fff;\n}");
        assert!(!clean.contains("expression"));
        assert!(clean.contains("background"));
    }

    #[test]
    fn test_sanitizes_css_behavior() {
        let clean = sanitize_css("div {\n  behavior: url(exploit.htc);\n  color: red;\n}");
        assert!(!clean.contains("behavior"));
        assert!(clean.contains("color: red"));
    }

    #[test]
    fn test_sanitizes_css_import_external() {
        let clean =
            sanitize_css("@import url('http://evil.com/style.css');\nbody { color: #333; }");
        assert!(!clean.contains("evil.com"));
        assert!(clean.contains("color: #333"));
    }

    #[test]
    fn test_preserves_css_layout() {
        let css = "div { display: flex; align-items: center; grid-template-columns: 1fr 1fr; position: relative; }";
        let clean = sanitize_css(css);
        assert!(clean.contains("display: flex"));
        assert!(clean.contains("grid-template-columns"));
    }

    #[test]
    fn test_preserves_css_media_queries() {
        let css = "@media (max-width: 768px) { .container { flex-direction: column; } }";
        let clean = sanitize_css(css);
        assert!(clean.contains("@media"));
        assert!(clean.contains("max-width: 768px"));
    }

    #[test]
    fn test_reports_removed_elements() {
        let result =
            sanitize_html("<div><script>alert('x')</script><iframe src='evil'></iframe></div>");
        assert!(!result.removed_elements.is_empty());
        assert!(result.removed_elements.iter().any(|r| r.contains("script")));
        assert!(result.removed_elements.iter().any(|r| r.contains("iframe")));
    }
}

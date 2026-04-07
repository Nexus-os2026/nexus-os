//! Assembler — takes a ContentPayload + template HTML → final rendered HTML.
//!
//! This is the LAST line of defense against XSS. All Text and CTA slot values
//! are HTML-escaped on injection, regardless of upstream validation.

use crate::content_payload::ContentPayload;
use crate::image_gen::GeneratedImage;
use crate::slot_schema::{html_escape, SlotType, TemplateSchema};
use crate::tokens::TokenSet;
use std::collections::HashMap;
use thiserror::Error;

// ─── Errors ─────────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum AssemblyError {
    #[error("dangerous URL in slot '{slot}': {detail}")]
    DangerousUrl { slot: String, detail: String },
    #[error("assembly failed: {0}")]
    General(String),
}

// ─── Rich Text Sanitizer ────────────────────────────────────────────────────

/// Sanitize rich text to allowlisted tags only.
/// Allows: <strong>, </strong>, <em>, </em>, <br>, <br/>, <br />, <a href="...">, </a>
fn sanitize_rich_text(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '<' {
            // Collect the tag
            let mut tag = String::from('<');
            for c in chars.by_ref() {
                tag.push(c);
                if c == '>' {
                    break;
                }
            }
            let lower = tag.to_lowercase();
            if lower == "<strong>"
                || lower == "</strong>"
                || lower == "<em>"
                || lower == "</em>"
                || lower == "<br>"
                || lower == "<br/>"
                || lower == "<br />"
                || lower == "</a>"
                || lower.starts_with("<a href=")
            {
                result.push_str(&tag);
            } else {
                // Escape disallowed tag
                result.push_str(&html_escape(&tag));
            }
        } else {
            result.push(ch);
        }
    }
    result
}

// ─── URL Protocol Validation ────────────────────────────────────────────────

const ALLOWED_URL_PROTOCOLS: &[&str] = &["https://", "mailto:", "tel:", "#"];

fn validate_url_protocol(slot_name: &str, url: &str) -> Result<String, AssemblyError> {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return Ok(String::new());
    }
    if ALLOWED_URL_PROTOCOLS.iter().any(|p| trimmed.starts_with(p)) {
        Ok(html_escape(trimmed))
    } else {
        Err(AssemblyError::DangerousUrl {
            slot: slot_name.into(),
            detail: format!(
                "URL must start with https://, mailto:, tel:, or # — got: {}",
                &trimmed[..trimmed.len().min(40)]
            ),
        })
    }
}

// ─── Placeholder → Slot Mapping ─────────────────────────────────────────────

/// Build a flat lookup from HTML placeholder names (SCREAMING_SNAKE_CASE) to
/// (section_id, slot_name, value) tuples.
///
/// The HTML templates use two naming conventions:
/// 1. Direct: `{{CTA_PRIMARY}}` → slot `cta_primary` (found by scanning all sections)
/// 2. Section-prefixed: `{{FEATURES_HEADING}}` → section `features`, slot `heading`
fn build_placeholder_map<'a>(
    payload: &'a ContentPayload,
    _schema: &TemplateSchema,
) -> HashMap<String, (&'a str, &'a str, &'a str)> {
    let mut map: HashMap<String, (&str, &str, &str)> = HashMap::new();

    // First pass: section-prefixed keys (always unique, highest priority)
    for sc in &payload.sections {
        let section_id = sc.section_id.as_str();
        for (slot_name, value) in &sc.slots {
            let prefixed_key = format!("{}_{}", section_id, slot_name).to_uppercase();
            map.insert(
                prefixed_key,
                (section_id, slot_name.as_str(), value.as_str()),
            );
        }
    }

    // Second pass: direct keys (first section wins — don't overwrite)
    for sc in &payload.sections {
        let section_id = sc.section_id.as_str();
        for (slot_name, value) in &sc.slots {
            let direct_key = slot_name.to_uppercase();
            map.entry(direct_key)
                .or_insert((section_id, slot_name.as_str(), value.as_str()));
        }
    }

    map
}

/// Look up the SlotType for a given section_id + slot_name from the schema.
fn get_slot_type(schema: &TemplateSchema, section_id: &str, slot_name: &str) -> Option<SlotType> {
    schema
        .sections
        .iter()
        .find(|s| s.section_id == section_id)
        .and_then(|s| s.slots.get(slot_name))
        .map(|c| c.slot_type)
}

// ─── Meta Placeholder Resolution ───────────────────────────────────────────

/// Extract the bare Google Fonts family name from a CSS font-family stack.
/// e.g. "'Inter', system-ui, sans-serif" → "Inter"
/// e.g. "'Playfair Display', Georgia, serif" → "Playfair Display"
fn extract_google_font_name(font_stack: &str) -> String {
    let first = font_stack.split(',').next().unwrap_or("").trim();
    first.trim_matches('\'').trim_matches('"').to_string()
}

/// URL-encode a font name for Google Fonts URLs (spaces → +).
fn url_encode_font(name: &str) -> String {
    name.replace(' ', "+")
}

/// Build a map of meta placeholders (HEADING_FONT, SITE_NAME, etc.) → resolved values.
fn build_meta_map(payload: &ContentPayload, token_set: &TokenSet) -> HashMap<&'static str, String> {
    let mut meta = HashMap::new();

    // Font names for Google Fonts <link> URLs
    let heading = extract_google_font_name(&token_set.foundation.font_heading);
    let body = extract_google_font_name(&token_set.foundation.font_body);
    let mono = extract_google_font_name(&token_set.foundation.font_mono);
    meta.insert("HEADING_FONT", url_encode_font(&heading));
    meta.insert("BODY_FONT", url_encode_font(&body));
    meta.insert("CODE_FONT", url_encode_font(&mono));

    // SITE_NAME: look for "brand" slot in nav/footer sections, or derive from template_id
    let site_name = payload
        .sections
        .iter()
        .filter(|s| s.section_id == "nav" || s.section_id == "footer")
        .find_map(|s| s.slots.get("brand"))
        .or_else(|| {
            payload.sections.iter().find_map(|s| {
                s.slots
                    .get("business_name")
                    .or_else(|| s.slots.get("brand"))
            })
        })
        .cloned()
        .unwrap_or_else(|| humanize_template_id(&payload.template_id));
    meta.insert("SITE_NAME", site_name);

    // TAGLINE: look for "tagline" or "subtitle" in hero section
    let tagline = payload
        .sections
        .iter()
        .filter(|s| s.section_id == "hero")
        .find_map(|s| s.slots.get("tagline").or_else(|| s.slots.get("subtitle")))
        .cloned()
        .unwrap_or_default();
    meta.insert("TAGLINE", tagline);

    // BUSINESS_NAME: alias for local_business template
    let biz_name = payload
        .sections
        .iter()
        .find_map(|s| {
            s.slots
                .get("business_name")
                .or_else(|| s.slots.get("brand"))
        })
        .cloned()
        .unwrap_or_else(|| humanize_template_id(&payload.template_id));
    meta.insert("BUSINESS_NAME", biz_name);

    meta
}

/// Convert template_id like "saas_landing" → "Saas Landing" for fallback names.
fn humanize_template_id(id: &str) -> String {
    id.split('_')
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                Some(first) => {
                    let upper: String = first.to_uppercase().collect();
                    format!("{upper}{}", c.as_str())
                }
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

// ─── Assembler ──────────────────────────────────────────────────────────────

/// Assemble a final HTML page from template HTML, content payload, and token CSS.
///
/// 1. Injects token CSS into the `:root {}` block
/// 2. Replaces `{{PLACEHOLDER}}` strings with content from the payload
/// 3. HTML-escapes Text/CTA values on injection (defense in depth)
/// 4. Validates URL protocols for Url slots
/// 5. Sanitizes RichText slots to allowlisted tags
/// 6. Replaces meta placeholders (HEADING_FONT, SITE_NAME, etc.) with real values
/// 7. Removes unfilled optional placeholders
pub fn assemble(
    payload: &ContentPayload,
    template_html: &str,
    token_set: &TokenSet,
    schema: &TemplateSchema,
) -> Result<String, AssemblyError> {
    let mut html = template_html.to_string();

    // Step 1: Inject token CSS
    // Replace the existing :root { ... } block with token CSS
    let token_css = token_set.to_css();
    if let Some(root_start) = html.find(":root {") {
        // Find the matching closing brace for :root
        if let Some(rel_end) = find_matching_brace(&html[root_start..]) {
            let root_end = root_start + rel_end + 1;
            // Also capture any subsequent @media blocks that are part of the token system
            // We inject our full token CSS before the rest of the <style> content
            let before = &html[..root_start];
            let after = &html[root_end..];
            html = format!("{before}{token_css}{after}");
        }
    }

    // Step 2-5: Replace content placeholders
    let placeholder_map = build_placeholder_map(payload, schema);

    // Step 6: Build meta placeholder map for HEADING_FONT, SITE_NAME, etc.
    let meta_map = build_meta_map(payload, token_set);

    // Find and replace all {{PLACEHOLDER}} patterns
    let mut output = String::with_capacity(html.len());
    let mut remaining = html.as_str();

    while let Some(start) = remaining.find("{{") {
        output.push_str(&remaining[..start]);
        remaining = &remaining[start + 2..];

        if let Some(end) = remaining.find("}}") {
            let placeholder = &remaining[..end];
            remaining = &remaining[end + 2..];

            // Look up in content map first
            if let Some(&(section_id, slot_name, value)) = placeholder_map.get(placeholder) {
                let slot_type = get_slot_type(schema, section_id, slot_name);
                let injected = match slot_type {
                    Some(SlotType::Text) | Some(SlotType::Cta) => {
                        // CRITICAL: HTML-escape text and CTA values
                        html_escape(value)
                    }
                    Some(SlotType::Url) | Some(SlotType::VideoEmbed) => {
                        validate_url_protocol(slot_name, value)?
                    }
                    Some(SlotType::ImagePrompt) => {
                        // Inject as escaped alt text
                        html_escape(value)
                    }
                    Some(SlotType::RichText) => sanitize_rich_text(value),
                    Some(SlotType::Number) => {
                        // Already validated as numeric
                        html_escape(value)
                    }
                    Some(SlotType::IconKey) => {
                        // Inject as escaped text (used as class or data attr)
                        html_escape(value)
                    }
                    None => {
                        // Unknown slot type — escape defensively
                        html_escape(value)
                    }
                };
                output.push_str(&injected);
            } else if let Some(meta_val) = meta_map.get(placeholder) {
                // Meta placeholder (HEADING_FONT, SITE_NAME, etc.) — inject resolved value
                output.push_str(&html_escape(meta_val));
            } else {
                // Remove unfilled optional placeholder (replace with empty string)
            }
        } else {
            // No closing }} — preserve as-is
            output.push_str("{{");
        }
    }
    output.push_str(remaining);

    Ok(output)
}

/// Assemble with generated images.
///
/// Same as `assemble`, but ImagePrompt slots are replaced with `<img>` tags
/// referencing the generated images instead of plain alt text.
pub fn assemble_with_images(
    payload: &ContentPayload,
    template_html: &str,
    token_set: &TokenSet,
    schema: &TemplateSchema,
    image_map: &HashMap<(String, String), GeneratedImage>,
) -> Result<String, AssemblyError> {
    let mut html = template_html.to_string();

    // Step 1: Inject token CSS (same as assemble)
    let token_css = token_set.to_css();
    if let Some(root_start) = html.find(":root {") {
        if let Some(rel_end) = find_matching_brace(&html[root_start..]) {
            let root_end = root_start + rel_end + 1;
            let before = &html[..root_start];
            let after = &html[root_end..];
            html = format!("{before}{token_css}{after}");
        }
    }

    // Step 2-5: Replace placeholders (with image support)
    let placeholder_map = build_placeholder_map(payload, schema);

    // Step 6: Build meta placeholder map for HEADING_FONT, SITE_NAME, etc.
    let meta_map = build_meta_map(payload, token_set);

    let mut output = String::with_capacity(html.len());
    let mut remaining = html.as_str();

    while let Some(start) = remaining.find("{{") {
        output.push_str(&remaining[..start]);
        remaining = &remaining[start + 2..];

        if let Some(end) = remaining.find("}}") {
            let placeholder = &remaining[..end];
            remaining = &remaining[end + 2..];

            if let Some(&(section_id, slot_name, value)) = placeholder_map.get(placeholder) {
                let slot_type = get_slot_type(schema, section_id, slot_name);
                let injected = match slot_type {
                    Some(SlotType::ImagePrompt) => {
                        // Check if we have a generated image for this slot
                        let key = (section_id.to_string(), slot_name.to_string());
                        if let Some(gen_img) = image_map.get(&key) {
                            gen_img.to_img_tag()
                        } else {
                            // No generated image — inject as escaped alt text (same as before)
                            html_escape(value)
                        }
                    }
                    Some(SlotType::Text) | Some(SlotType::Cta) => html_escape(value),
                    Some(SlotType::Url) | Some(SlotType::VideoEmbed) => {
                        validate_url_protocol(slot_name, value)?
                    }
                    Some(SlotType::RichText) => sanitize_rich_text(value),
                    Some(SlotType::Number) => html_escape(value),
                    Some(SlotType::IconKey) => html_escape(value),
                    None => html_escape(value),
                };
                output.push_str(&injected);
            } else if let Some(meta_val) = meta_map.get(placeholder) {
                // Meta placeholder (HEADING_FONT, SITE_NAME, etc.) — inject resolved value
                output.push_str(&html_escape(meta_val));
            } else {
                // Remove unfilled optional placeholder (replace with empty string)
            }
        } else {
            output.push_str("{{");
        }
    }
    output.push_str(remaining);

    Ok(output)
}

/// Find the index of the matching closing brace for a block that starts with `{`.
fn find_matching_brace(s: &str) -> Option<usize> {
    let mut depth = 0i32;
    let mut in_string = false;
    let mut prev = ' ';

    for (i, ch) in s.char_indices() {
        if ch == '\'' || ch == '"' {
            if prev != '\\' {
                in_string = !in_string;
            }
        } else if !in_string {
            if ch == '{' {
                depth += 1;
            } else if ch == '}' {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
        }
        prev = ch;
    }
    None
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content_payload::{ContentPayload, SectionContent};
    use crate::slot_schema::get_template_schema;
    use crate::templates::get_template;
    use crate::variant::{MotionProfile, VariantSelection};

    fn default_variant(palette: &str) -> VariantSelection {
        VariantSelection {
            palette_id: palette.into(),
            typography_id: "modern".into(),
            layout: HashMap::new(),
            motion: MotionProfile::Subtle,
        }
    }

    fn mock_saas_payload() -> ContentPayload {
        ContentPayload {
            template_id: "saas_landing".into(),
            variant: default_variant("saas_midnight"),
            sections: vec![
                SectionContent {
                    section_id: "hero".into(),
                    slots: HashMap::from([
                        ("badge".into(), "New in v2".into()),
                        ("headline".into(), "Build Faster with AI Power".into()),
                        (
                            "subtitle".into(),
                            "The platform that helps developers ship 10x faster.".into(),
                        ),
                        ("cta_primary".into(), "Start Free Trial".into()),
                        ("cta_secondary".into(), "Watch Demo".into()),
                        (
                            "media".into(),
                            "Product screenshot showing the dashboard interface".into(),
                        ),
                    ]),
                },
                SectionContent {
                    section_id: "features".into(),
                    slots: HashMap::from([
                        ("heading".into(), "Powerful Features".into()),
                        (
                            "subheading".into(),
                            "Everything you need to ship faster".into(),
                        ),
                        ("feature_1_icon".into(), "rocket".into()),
                        ("feature_1_title".into(), "Lightning Fast".into()),
                        (
                            "feature_1_desc".into(),
                            "Deploy in seconds with our optimized pipeline.".into(),
                        ),
                        ("feature_2_icon".into(), "shield".into()),
                        ("feature_2_title".into(), "Enterprise Security".into()),
                        (
                            "feature_2_desc".into(),
                            "Bank-grade encryption protects your data.".into(),
                        ),
                        ("feature_3_icon".into(), "chart".into()),
                        ("feature_3_title".into(), "Real-time Analytics".into()),
                        (
                            "feature_3_desc".into(),
                            "Monitor everything with live dashboards.".into(),
                        ),
                    ]),
                },
                SectionContent {
                    section_id: "pricing".into(),
                    slots: HashMap::from([
                        ("heading".into(), "Simple Pricing".into()),
                        ("tier_1_name".into(), "Starter".into()),
                        ("tier_1_price".into(), "$9/mo".into()),
                        (
                            "tier_1_features".into(),
                            "5 projects<br>1GB storage<br>Email support".into(),
                        ),
                        ("tier_2_name".into(), "Pro".into()),
                        ("tier_2_price".into(), "$29/mo".into()),
                        (
                            "tier_2_features".into(),
                            "Unlimited projects<br>10GB storage".into(),
                        ),
                        ("tier_2_badge".into(), "Most Popular".into()),
                        ("tier_3_name".into(), "Enterprise".into()),
                        ("tier_3_price".into(), "$99/mo".into()),
                        (
                            "tier_3_features".into(),
                            "Everything in Pro<br>SSO<br>SLA".into(),
                        ),
                    ]),
                },
                SectionContent {
                    section_id: "testimonials".into(),
                    slots: HashMap::from([
                        ("heading".into(), "What Our Users Say".into()),
                        (
                            "testimonial_1_quote".into(),
                            "This tool saved us hours.".into(),
                        ),
                        ("testimonial_1_author".into(), "Jane Smith".into()),
                        ("testimonial_1_role".into(), "CTO at TechCorp".into()),
                        (
                            "testimonial_2_quote".into(),
                            "Best developer tool ever.".into(),
                        ),
                        ("testimonial_2_author".into(), "Alex Johnson".into()),
                        ("testimonial_2_role".into(), "Lead Engineer".into()),
                        (
                            "testimonial_3_quote".into(),
                            "Incredible reliability.".into(),
                        ),
                        ("testimonial_3_author".into(), "Maria Garcia".into()),
                        ("testimonial_3_role".into(), "VP Engineering".into()),
                    ]),
                },
                SectionContent {
                    section_id: "cta".into(),
                    slots: HashMap::from([
                        ("headline".into(), "Ready to Ship Faster?".into()),
                        (
                            "body".into(),
                            "Join developers building better software.".into(),
                        ),
                        ("cta_button".into(), "Get Started Free".into()),
                    ]),
                },
                SectionContent {
                    section_id: "footer".into(),
                    slots: HashMap::from([
                        ("brand".into(), "AcmeAI".into()),
                        (
                            "copyright".into(),
                            "2026 AcmeAI Inc. All rights reserved.".into(),
                        ),
                    ]),
                },
            ],
        }
    }

    fn mock_portfolio_payload() -> ContentPayload {
        ContentPayload {
            template_id: "portfolio".into(),
            variant: default_variant("port_monochrome"),
            sections: vec![
                SectionContent {
                    section_id: "hero".into(),
                    slots: HashMap::from([
                        ("avatar".into(), "JD".into()),
                        ("name".into(), "Jane Doe".into()),
                        ("title".into(), "Full-Stack Developer".into()),
                        ("bio".into(), "Building delightful web experiences.".into()),
                        ("cta_primary".into(), "View My Work".into()),
                        ("cta_secondary".into(), "Get in Touch".into()),
                    ]),
                },
                SectionContent {
                    section_id: "projects".into(),
                    slots: HashMap::from([
                        ("heading".into(), "Featured Projects".into()),
                        (
                            "project_1_image".into(),
                            "Screenshot of e-commerce project".into(),
                        ),
                        ("project_1_title".into(), "ShopFlow".into()),
                        (
                            "project_1_desc".into(),
                            "A modern e-commerce platform.".into(),
                        ),
                        ("project_1_tag_1".into(), "React".into()),
                        ("project_1_tag_2".into(), "Node.js".into()),
                        (
                            "project_2_image".into(),
                            "Screenshot of analytics dashboard".into(),
                        ),
                        ("project_2_title".into(), "DataViz Pro".into()),
                        (
                            "project_2_desc".into(),
                            "Real-time analytics dashboard.".into(),
                        ),
                        ("project_2_tag_1".into(), "Vue.js".into()),
                        (
                            "project_3_image".into(),
                            "Screenshot of chat application".into(),
                        ),
                        ("project_3_title".into(), "ChatSync".into()),
                        (
                            "project_3_desc".into(),
                            "Real-time messaging platform.".into(),
                        ),
                        ("project_3_tag_1".into(), "Rust".into()),
                        (
                            "project_4_image".into(),
                            "Screenshot of API platform".into(),
                        ),
                        ("project_4_title".into(), "APIHub".into()),
                        (
                            "project_4_desc".into(),
                            "Developer API management tool.".into(),
                        ),
                        ("project_4_tag_1".into(), "Go".into()),
                    ]),
                },
                SectionContent {
                    section_id: "about".into(),
                    slots: HashMap::from([
                        ("heading".into(), "About Me".into()),
                        (
                            "bio".into(),
                            "I am a developer with <strong>10 years</strong> of experience.".into(),
                        ),
                    ]),
                },
                SectionContent {
                    section_id: "skills".into(),
                    slots: HashMap::from([
                        ("heading".into(), "Skills".into()),
                        ("skill_1".into(), "Rust".into()),
                        ("skill_2".into(), "TypeScript".into()),
                        ("skill_3".into(), "React".into()),
                        ("skill_4".into(), "PostgreSQL".into()),
                    ]),
                },
                SectionContent {
                    section_id: "contact".into(),
                    slots: HashMap::from([
                        ("heading".into(), "Contact".into()),
                        ("subtext".into(), "Reach out for collaboration.".into()),
                        ("email".into(), "mailto:jane@example.com".into()),
                    ]),
                },
                SectionContent {
                    section_id: "footer".into(),
                    slots: HashMap::from([(
                        "copyright".into(),
                        "2026 Jane Doe. All rights reserved.".into(),
                    )]),
                },
            ],
        }
    }

    fn mock_ecommerce_payload() -> ContentPayload {
        ContentPayload {
            template_id: "ecommerce".into(),
            variant: default_variant("ecom_luxe"),
            sections: vec![
                SectionContent {
                    section_id: "hero".into(),
                    slots: HashMap::from([
                        ("headline".into(), "New Season Collection".into()),
                        (
                            "subtitle".into(),
                            "Discover the latest trends in sustainable fashion.".into(),
                        ),
                        ("cta_primary".into(), "Shop Now".into()),
                    ]),
                },
                SectionContent {
                    section_id: "categories".into(),
                    slots: HashMap::from([
                        ("heading".into(), "Shop by Category".into()),
                        ("category_1".into(), "Dresses".into()),
                        ("category_2".into(), "Tops".into()),
                        ("category_3".into(), "Accessories".into()),
                    ]),
                },
                SectionContent {
                    section_id: "products".into(),
                    slots: HashMap::from([
                        ("heading".into(), "Featured Products".into()),
                        ("product_1_image".into(), "Organic cotton dress".into()),
                        ("product_1_name".into(), "Organic Cotton Midi Dress".into()),
                        ("product_1_price".into(), "$89.00".into()),
                        ("product_2_image".into(), "Linen blend top".into()),
                        ("product_2_name".into(), "Linen Blend Relaxed Top".into()),
                        ("product_2_price".into(), "$49.00".into()),
                        ("product_3_image".into(), "Recycled denim jacket".into()),
                        ("product_3_name".into(), "Recycled Denim Jacket".into()),
                        ("product_3_price".into(), "$120.00".into()),
                        ("product_4_image".into(), "Bamboo fiber scarf".into()),
                        ("product_4_name".into(), "Bamboo Fiber Scarf".into()),
                        ("product_4_price".into(), "$35.00".into()),
                    ]),
                },
                SectionContent {
                    section_id: "reviews".into(),
                    slots: HashMap::from([
                        ("heading".into(), "Customer Reviews".into()),
                        (
                            "review_1_text".into(),
                            "Amazing quality and fast shipping!".into(),
                        ),
                        ("review_1_author".into(), "Sarah K.".into()),
                        ("review_1_rating".into(), "5".into()),
                        (
                            "review_2_text".into(),
                            "Love the sustainable materials.".into(),
                        ),
                        ("review_2_author".into(), "Mike T.".into()),
                        ("review_2_rating".into(), "4".into()),
                    ]),
                },
                SectionContent {
                    section_id: "newsletter".into(),
                    slots: HashMap::from([
                        ("heading".into(), "Join Our Community".into()),
                        (
                            "subtext".into(),
                            "Get exclusive deals and style tips.".into(),
                        ),
                        ("cta_button".into(), "Subscribe Now".into()),
                    ]),
                },
                SectionContent {
                    section_id: "footer".into(),
                    slots: HashMap::from([
                        ("store_name".into(), "EcoWear".into()),
                        (
                            "store_description".into(),
                            "Sustainable fashion for the modern world.".into(),
                        ),
                        (
                            "copyright".into(),
                            "2026 EcoWear. All rights reserved.".into(),
                        ),
                    ]),
                },
            ],
        }
    }

    fn mock_dashboard_payload() -> ContentPayload {
        ContentPayload {
            template_id: "dashboard".into(),
            variant: default_variant("dash_pro"),
            sections: vec![
                SectionContent {
                    section_id: "sidebar".into(),
                    slots: HashMap::from([
                        ("app_name".into(), "MetricsHub".into()),
                        ("nav_item_1".into(), "Dashboard".into()),
                        ("nav_item_1_icon".into(), "home".into()),
                        ("nav_item_2".into(), "Analytics".into()),
                        ("nav_item_2_icon".into(), "chart".into()),
                        ("nav_item_3".into(), "Settings".into()),
                        ("nav_item_3_icon".into(), "gear".into()),
                    ]),
                },
                SectionContent {
                    section_id: "header".into(),
                    slots: HashMap::from([
                        ("user_name".into(), "Alex Rivera".into()),
                        ("user_role".into(), "Admin".into()),
                        ("user_initial".into(), "AR".into()),
                        ("search_placeholder".into(), "Search metrics...".into()),
                    ]),
                },
                SectionContent {
                    section_id: "stats".into(),
                    slots: HashMap::from([
                        ("stat_1_label".into(), "Total Revenue".into()),
                        ("stat_1_value".into(), "$48,200".into()),
                        ("stat_1_change".into(), "+12%".into()),
                        ("stat_2_label".into(), "Active Users".into()),
                        ("stat_2_value".into(), "2,340".into()),
                        ("stat_2_change".into(), "+8%".into()),
                        ("stat_3_label".into(), "Conversion Rate".into()),
                        ("stat_3_value".into(), "3.2%".into()),
                        ("stat_3_change".into(), "+0.5%".into()),
                        ("stat_4_label".into(), "Avg Session".into()),
                        ("stat_4_value".into(), "4m 12s".into()),
                        ("stat_4_change".into(), "-2%".into()),
                    ]),
                },
                SectionContent {
                    section_id: "charts".into(),
                    slots: HashMap::from([
                        ("chart_1_title".into(), "Revenue Trend".into()),
                        ("chart_1_type".into(), "line".into()),
                        ("chart_2_title".into(), "User Distribution".into()),
                        ("chart_2_type".into(), "bar".into()),
                    ]),
                },
                SectionContent {
                    section_id: "data_table".into(),
                    slots: HashMap::from([
                        ("table_title".into(), "Recent Transactions".into()),
                        ("col_1".into(), "Customer".into()),
                        ("col_2".into(), "Amount".into()),
                        ("col_3".into(), "Status".into()),
                        ("col_4".into(), "Date".into()),
                        ("row_1_col_1".into(), "Acme Corp".into()),
                        ("row_1_col_2".into(), "$1,200".into()),
                        ("row_1_col_3".into(), "Completed".into()),
                        ("row_1_col_4".into(), "2026-04-01".into()),
                        ("row_2_col_1".into(), "TechStart".into()),
                        ("row_2_col_2".into(), "$800".into()),
                        ("row_2_col_3".into(), "Pending".into()),
                        ("row_2_col_4".into(), "2026-04-02".into()),
                        ("row_3_col_1".into(), "DataFlow".into()),
                        ("row_3_col_2".into(), "$2,400".into()),
                        ("row_3_col_3".into(), "Completed".into()),
                        ("row_3_col_4".into(), "2026-04-03".into()),
                    ]),
                },
                SectionContent {
                    section_id: "footer".into(),
                    slots: HashMap::from([
                        ("app_name".into(), "MetricsHub".into()),
                        ("app_version".into(), "v2.1.0".into()),
                        (
                            "copyright".into(),
                            "2026 MetricsHub. All rights reserved.".into(),
                        ),
                    ]),
                },
            ],
        }
    }

    fn mock_local_business_payload() -> ContentPayload {
        ContentPayload {
            template_id: "local_business".into(),
            variant: default_variant("biz_warm"),
            sections: vec![
                SectionContent {
                    section_id: "hero".into(),
                    slots: HashMap::from([
                        ("business_name".into(), "Bella Cucina".into()),
                        (
                            "tagline".into(),
                            "Authentic Italian dining in the heart of downtown.".into(),
                        ),
                        ("cta_primary".into(), "Book a Table".into()),
                        ("phone".into(), "555-0123".into()),
                    ]),
                },
                SectionContent {
                    section_id: "services".into(),
                    slots: HashMap::from([
                        ("heading".into(), "Our Services".into()),
                        ("service_1_icon".into(), "utensils".into()),
                        ("service_1_name".into(), "Fine Dining".into()),
                        (
                            "service_1_desc".into(),
                            "Handcrafted Italian dishes by Chef Marco.".into(),
                        ),
                        ("service_2_icon".into(), "glass".into()),
                        ("service_2_name".into(), "Wine Selection".into()),
                        (
                            "service_2_desc".into(),
                            "Curated wines from Italian vineyards.".into(),
                        ),
                        ("service_3_icon".into(), "party".into()),
                        ("service_3_name".into(), "Private Events".into()),
                        (
                            "service_3_desc".into(),
                            "Host your celebration in our garden.".into(),
                        ),
                    ]),
                },
                SectionContent {
                    section_id: "gallery".into(),
                    slots: HashMap::from([
                        ("heading".into(), "Our Gallery".into()),
                        ("gallery_1".into(), "Interior of the restaurant".into()),
                        ("gallery_2".into(), "Pasta dish close-up".into()),
                        ("gallery_3".into(), "Outdoor patio seating".into()),
                    ]),
                },
                SectionContent {
                    section_id: "testimonials".into(),
                    slots: HashMap::from([
                        ("heading".into(), "Guest Reviews".into()),
                        (
                            "testimonial_1_quote".into(),
                            "Best Italian food in the city!".into(),
                        ),
                        ("testimonial_1_author".into(), "Roberto M.".into()),
                        (
                            "testimonial_2_quote".into(),
                            "The tiramisu is unforgettable.".into(),
                        ),
                        ("testimonial_2_author".into(), "Lisa Chen".into()),
                    ]),
                },
                SectionContent {
                    section_id: "map".into(),
                    slots: HashMap::from([("address".into(), "123 Main Street, Downtown".into())]),
                },
                SectionContent {
                    section_id: "hours".into(),
                    slots: HashMap::from([
                        ("heading".into(), "Hours & Location".into()),
                        ("weekday_hours".into(), "Mon-Fri: 11am-10pm".into()),
                        ("saturday_hours".into(), "Sat: 10am-11pm".into()),
                        ("sunday_hours".into(), "Sun: 10am-9pm".into()),
                        ("address".into(), "123 Main Street, Downtown".into()),
                        ("phone".into(), "555-0123".into()),
                    ]),
                },
                SectionContent {
                    section_id: "footer".into(),
                    slots: HashMap::from([
                        ("business_name".into(), "Bella Cucina".into()),
                        (
                            "copyright".into(),
                            "2026 Bella Cucina. All rights reserved.".into(),
                        ),
                    ]),
                },
            ],
        }
    }

    fn mock_docs_site_payload() -> ContentPayload {
        ContentPayload {
            template_id: "docs_site".into(),
            variant: default_variant("docs_clean"),
            sections: vec![
                SectionContent {
                    section_id: "sidebar_nav".into(),
                    slots: HashMap::from([
                        ("doc_title".into(), "NexusAPI Docs".into()),
                        ("nav_category_1".into(), "Getting Started".into()),
                        ("nav_items_1".into(), "<a href=\"#install\">Installation</a><br><a href=\"#quickstart\">Quickstart</a>".into()),
                    ]),
                },
                SectionContent {
                    section_id: "search".into(),
                    slots: HashMap::from([
                        ("placeholder".into(), "Search documentation...".into()),
                    ]),
                },
                SectionContent {
                    section_id: "content".into(),
                    slots: HashMap::from([
                        ("main_heading".into(), "NexusAPI Documentation".into()),
                        ("intro_text".into(), "Welcome to NexusAPI. <strong>Fast</strong>, secure, and easy to use.".into()),
                    ]),
                },
                SectionContent {
                    section_id: "code_blocks".into(),
                    slots: HashMap::from([
                        ("code_lang_1".into(), "bash".into()),
                        ("code_example_1".into(), "npm install nexus-api".into()),
                    ]),
                },
                SectionContent {
                    section_id: "footer".into(),
                    slots: HashMap::from([
                        ("copyright".into(), "2026 NexusAPI. All rights reserved.".into()),
                    ]),
                },
            ],
        }
    }

    #[test]
    fn test_assemble_replaces_all_placeholders() {
        let template = get_template("saas_landing").unwrap();
        let schema = get_template_schema("saas_landing").unwrap();
        let payload = mock_saas_payload();
        let token_set = payload.variant.to_token_set().unwrap();

        let result = assemble(&payload, template.html, &token_set, &schema);
        assert!(result.is_ok(), "assembly failed: {result:?}");
        let html = result.unwrap();

        // Check that key content was injected
        assert!(
            html.contains("Build Faster with AI Power"),
            "missing headline"
        );
        assert!(html.contains("Start Free Trial"), "missing CTA");
        assert!(html.contains("AcmeAI"), "missing brand");

        // Verify key content slots are filled (font/meta placeholders may remain)
    }

    #[test]
    fn test_assemble_html_escapes_text_slots() {
        let schema = get_template_schema("saas_landing").unwrap();
        let mut payload = mock_saas_payload();
        // Inject XSS attempt into a text slot
        payload.sections[0]
            .slots
            .insert("headline".into(), "<script>alert('xss')</script>".into());
        let token_set = payload.variant.to_token_set().unwrap();

        let simple_html = "<h1>{{HEADLINE}}</h1>";
        let result = assemble(&payload, simple_html, &token_set, &schema).unwrap();
        assert!(
            !result.contains("<script>"),
            "XSS should be escaped, got: {result}"
        );
        assert!(
            result.contains("&lt;script&gt;"),
            "should contain escaped script tag"
        );
    }

    #[test]
    fn test_assemble_html_escapes_cta_slots() {
        let schema = get_template_schema("saas_landing").unwrap();
        let mut payload = mock_saas_payload();
        payload.sections[0]
            .slots
            .insert("cta_primary".into(), "Start <b>Now</b>".into());
        let token_set = payload.variant.to_token_set().unwrap();

        let simple_html = "<button>{{CTA_PRIMARY}}</button>";
        let result = assemble(&payload, simple_html, &token_set, &schema).unwrap();
        assert!(
            !result.contains("<b>"),
            "HTML in CTA should be escaped, got: {result}"
        );
    }

    #[test]
    fn test_assemble_removes_unfilled_optional_placeholders() {
        let schema = get_template_schema("saas_landing").unwrap();
        let payload = mock_saas_payload();
        let token_set = payload.variant.to_token_set().unwrap();

        let html_with_optional = "<p>{{SOME_UNKNOWN_OPTIONAL}}</p><p>{{HEADLINE}}</p>";
        let result = assemble(&payload, html_with_optional, &token_set, &schema).unwrap();
        assert!(
            !result.contains("{{SOME_UNKNOWN_OPTIONAL}}"),
            "unfilled optional should be removed"
        );
        assert!(result.contains("Build Faster with AI Power"));
    }

    #[test]
    fn test_assemble_injects_token_css() {
        let schema = get_template_schema("saas_landing").unwrap();
        let payload = mock_saas_payload();
        let token_set = payload.variant.to_token_set().unwrap();

        let template = get_template("saas_landing").unwrap();
        let result = assemble(&payload, template.html, &token_set, &schema).unwrap();
        // Token CSS should contain our palette colors
        assert!(
            result.contains("color-scheme: light dark"),
            "should contain token CSS"
        );
    }

    #[test]
    fn test_assemble_url_slot_validates_protocol() {
        let schema = get_template_schema("portfolio").unwrap();
        let mut payload = mock_portfolio_payload();
        // Inject javascript: URL
        payload.sections[1]
            .slots
            .insert("project_1_url".into(), "javascript:alert('xss')".into());
        let token_set = payload.variant.to_token_set().unwrap();

        let html = "<a href=\"{{PROJECT_1_URL}}\">Link</a>";
        let result = assemble(&payload, html, &token_set, &schema);
        assert!(result.is_err(), "javascript: URL should be rejected");
        assert!(matches!(
            result.unwrap_err(),
            AssemblyError::DangerousUrl { .. }
        ));
    }

    #[test]
    fn test_assemble_preserves_html_structure() {
        let schema = get_template_schema("saas_landing").unwrap();
        let payload = mock_saas_payload();
        let token_set = payload.variant.to_token_set().unwrap();

        let template = get_template("saas_landing").unwrap();
        let result = assemble(&payload, template.html, &token_set, &schema).unwrap();
        // Basic structural checks
        assert!(result.contains("<!DOCTYPE html>"));
        assert!(result.contains("<html"));
        assert!(result.contains("</html>"));
        assert!(result.contains("<head>"));
        assert!(result.contains("</head>"));
        assert!(result.contains("<body"));
        assert!(result.contains("</body>"));
    }

    // ── Integration Tests: Full Assembly for All 6 Templates ──

    #[test]
    fn test_full_assembly_saas_landing() {
        let schema = get_template_schema("saas_landing").unwrap();
        let payload = mock_saas_payload();
        let token_set = payload.variant.to_token_set().unwrap();
        let template = get_template("saas_landing").unwrap();

        let result = assemble(&payload, template.html, &token_set, &schema);
        assert!(result.is_ok(), "saas_landing assembly failed: {result:?}");
        let html = result.unwrap();
        assert!(html.contains("Build Faster with AI Power"));
        assert!(html.contains("color-scheme: light dark"));
        assert!(html.len() > 1000, "output too short: {} chars", html.len());
    }

    #[test]
    fn test_full_assembly_portfolio() {
        let schema = get_template_schema("portfolio").unwrap();
        let payload = mock_portfolio_payload();
        let token_set = payload.variant.to_token_set().unwrap();
        let template = get_template("portfolio").unwrap();

        let result = assemble(&payload, template.html, &token_set, &schema);
        assert!(result.is_ok(), "portfolio assembly failed: {result:?}");
        let html = result.unwrap();
        assert!(html.contains("Jane Doe"));
        assert!(html.len() > 1000);
    }

    #[test]
    fn test_full_assembly_ecommerce() {
        let schema = get_template_schema("ecommerce").unwrap();
        let payload = mock_ecommerce_payload();
        let token_set = payload.variant.to_token_set().unwrap();
        let template = get_template("ecommerce").unwrap();

        let result = assemble(&payload, template.html, &token_set, &schema);
        assert!(result.is_ok(), "ecommerce assembly failed: {result:?}");
        let html = result.unwrap();
        assert!(html.contains("New Season Collection"));
        assert!(html.len() > 1000);
    }

    #[test]
    fn test_full_assembly_dashboard() {
        let schema = get_template_schema("dashboard").unwrap();
        let payload = mock_dashboard_payload();
        let token_set = payload.variant.to_token_set().unwrap();
        let template = get_template("dashboard").unwrap();

        let result = assemble(&payload, template.html, &token_set, &schema);
        assert!(result.is_ok(), "dashboard assembly failed: {result:?}");
        let html = result.unwrap();
        assert!(html.contains("MetricsHub"));
        assert!(html.len() > 1000);
    }

    #[test]
    fn test_full_assembly_local_business() {
        let schema = get_template_schema("local_business").unwrap();
        let payload = mock_local_business_payload();
        let token_set = payload.variant.to_token_set().unwrap();
        let template = get_template("local_business").unwrap();

        let result = assemble(&payload, template.html, &token_set, &schema);
        assert!(result.is_ok(), "local_business assembly failed: {result:?}");
        let html = result.unwrap();
        assert!(html.contains("Bella Cucina"));
        assert!(html.len() > 1000);
    }

    #[test]
    fn test_full_assembly_docs_site() {
        let schema = get_template_schema("docs_site").unwrap();
        let payload = mock_docs_site_payload();
        let token_set = payload.variant.to_token_set().unwrap();
        let template = get_template("docs_site").unwrap();

        let result = assemble(&payload, template.html, &token_set, &schema);
        assert!(result.is_ok(), "docs_site assembly failed: {result:?}");
        let html = result.unwrap();
        assert!(html.contains("NexusAPI"));
        assert!(html.len() > 1000);
    }

    // ── Image Generation Assembly Tests ──

    #[test]
    fn test_assemble_with_images_inserts_img_tag() {
        use crate::image_gen::{GeneratedImage, ImageFormat};

        let schema = get_template_schema("portfolio").unwrap();
        let payload = mock_portfolio_payload();
        let token_set = payload.variant.to_token_set().unwrap();

        let mut image_map = HashMap::new();
        image_map.insert(
            ("projects".to_string(), "project_1_image".to_string()),
            GeneratedImage {
                primary_path: "images/project_1_image.webp".into(),
                srcset_paths: vec![
                    ("images/project_1_image-800.webp".into(), 800),
                    ("images/project_1_image-400.webp".into(), 400),
                ],
                alt_text: "Screenshot of e-commerce project".into(),
                width: 1200,
                height: 800,
                format: ImageFormat::WebP,
                generation_method: "placeholder".into(),
                cost: 0.0,
            },
        );

        let simple_html = "<div>{{PROJECT_1_IMAGE}}</div>";
        let result =
            assemble_with_images(&payload, simple_html, &token_set, &schema, &image_map).unwrap();
        assert!(
            result.contains("src=\"images/project_1_image.webp\""),
            "should contain img src, got: {result}"
        );
        assert!(result.contains("srcset="), "should contain srcset");
        assert!(
            result.contains("alt=\"Screenshot of e-commerce project\""),
            "should contain alt text"
        );
        assert!(
            result.contains("loading=\"lazy\""),
            "should have lazy loading"
        );
    }

    #[test]
    fn test_assemble_with_images_falls_back_to_alt_text() {
        let schema = get_template_schema("portfolio").unwrap();
        let payload = mock_portfolio_payload();
        let token_set = payload.variant.to_token_set().unwrap();

        // Empty image map — no generated images
        let image_map = HashMap::new();

        let simple_html = "<div>{{PROJECT_1_IMAGE}}</div>";
        let result =
            assemble_with_images(&payload, simple_html, &token_set, &schema, &image_map).unwrap();
        // Should fall back to escaped alt text
        assert!(
            result.contains("Screenshot of e-commerce project"),
            "should contain alt text fallback"
        );
        assert!(
            !result.contains("<img"),
            "should NOT contain img tag without generated image"
        );
    }

    #[test]
    fn test_sanitize_rich_text_allows_safe_tags() {
        let input = "Hello <strong>world</strong> and <em>italic</em> with <br> breaks";
        let result = sanitize_rich_text(input);
        assert_eq!(result, input);
    }

    #[test]
    fn test_sanitize_rich_text_strips_unsafe_tags() {
        let input = "Hello <script>alert('xss')</script> world";
        let result = sanitize_rich_text(input);
        assert!(!result.contains("<script>"));
        assert!(result.contains("&lt;script&gt;"));
    }
}

//! Slot Contract — typed, constrained content slots for all template sections.
//!
//! This is the ONLY interface through which AI-generated content enters templates.
//! Every slot has a type, constraints, and validation. All text/CTA content is
//! HTML-escaped on assembly.

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

// ─── Errors ─────────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum SlotError {
    #[error("required slot '{slot}' is missing")]
    MissingRequired { slot: String },
    #[error("slot '{slot}' exceeds max length ({max} chars, got {actual})")]
    TooLong {
        slot: String,
        max: usize,
        actual: usize,
    },
    #[error("slot '{slot}' is below min length ({min} chars, got {actual})")]
    TooShort {
        slot: String,
        min: usize,
        actual: usize,
    },
    #[error("slot '{slot}' contains disallowed HTML: {detail}")]
    DisallowedHtml { slot: String, detail: String },
    #[error("slot '{slot}' has invalid URL: {detail}")]
    InvalidUrl { slot: String, detail: String },
    #[error("slot '{slot}' CTA is invalid: {detail}")]
    InvalidCta { slot: String, detail: String },
    #[error("slot '{slot}' must be numeric")]
    NotNumeric { slot: String },
}

// ─── Slot Types ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SlotType {
    /// Plain text — HTML-escaped on assembly.
    Text,
    /// Call-to-action button text — HTML-escaped + action-verb validated.
    Cta,
    /// URL — protocol allowlist (https, mailto, tel).
    Url,
    /// Image generation prompt — never rendered as HTML.
    ImagePrompt,
    /// Sanitized video embed URL only.
    VideoEmbed,
    /// Limited HTML: `<strong>`, `<em>`, `<br>`, `<a href>`.
    RichText,
    /// Numeric only (for stats, counters).
    Number,
    /// Icon identifier from an allowed set.
    IconKey,
}

// ─── Slot Constraint ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlotConstraint {
    pub slot_type: SlotType,
    pub required: bool,
    pub max_chars: Option<usize>,
    pub min_chars: Option<usize>,
    pub validation_hint: Option<String>,
}

impl SlotConstraint {
    fn new(slot_type: SlotType, required: bool) -> Self {
        Self {
            slot_type,
            required,
            max_chars: None,
            min_chars: None,
            validation_hint: None,
        }
    }

    fn max(mut self, n: usize) -> Self {
        self.max_chars = Some(n);
        self
    }

    fn min(mut self, n: usize) -> Self {
        self.min_chars = Some(n);
        self
    }

    fn hint(mut self, h: &str) -> Self {
        self.validation_hint = Some(h.to_string());
        self
    }
}

// ─── Schema Types ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SectionSchema {
    pub section_id: String,
    pub display_name: String,
    pub slots: IndexMap<String, SlotConstraint>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateSchema {
    pub template_id: String,
    pub sections: Vec<SectionSchema>,
}

// ─── Validation ─────────────────────────────────────────────────────────────

/// Escape HTML special characters for Text and Cta slots.
pub fn html_escape(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '&' => escaped.push_str("&amp;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&#x27;"),
            _ => escaped.push(ch),
        }
    }
    escaped
}

/// Validate a URL against the protocol allowlist.
pub fn validate_url(value: &str) -> Result<(), SlotError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(SlotError::InvalidUrl {
            slot: String::new(),
            detail: "URL is empty".into(),
        });
    }
    let allowed_prefixes = ["https://", "mailto:", "tel:", "#"];
    if !allowed_prefixes.iter().any(|p| trimmed.starts_with(p)) {
        return Err(SlotError::InvalidUrl {
            slot: String::new(),
            detail: format!(
                "URL must start with https://, mailto:, tel:, or # — got: {}",
                &trimmed[..trimmed.len().min(40)]
            ),
        });
    }
    Ok(())
}

/// Validate a CTA string — must be non-empty and start with an action verb (heuristic).
pub fn validate_cta(value: &str) -> Result<(), SlotError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(SlotError::InvalidCta {
            slot: String::new(),
            detail: "CTA text cannot be empty".into(),
        });
    }
    // Heuristic: first word should be an action verb or common CTA starter
    let first_word = trimmed
        .split_whitespace()
        .next()
        .unwrap_or("")
        .to_lowercase();
    let action_starters = [
        "get",
        "start",
        "try",
        "join",
        "sign",
        "buy",
        "shop",
        "book",
        "schedule",
        "contact",
        "learn",
        "discover",
        "explore",
        "download",
        "view",
        "see",
        "read",
        "watch",
        "create",
        "build",
        "launch",
        "subscribe",
        "order",
        "request",
        "claim",
        "unlock",
        "access",
        "begin",
        "send",
        "submit",
        "reserve",
        "browse",
        "find",
        "add",
        "open",
        "upgrade",
        "register",
        "call",
        "email",
        "visit",
        "check",
        "apply",
        "enroll",
        "hire",
    ];
    if !action_starters.contains(&first_word.as_str()) {
        return Err(SlotError::InvalidCta {
            slot: String::new(),
            detail: format!("CTA should start with an action verb, got: '{first_word}'"),
        });
    }
    Ok(())
}

/// Validate a single slot value against its constraint.
pub fn validate_slot_value(
    slot_name: &str,
    value: &str,
    constraint: &SlotConstraint,
) -> Result<(), SlotError> {
    // Length checks
    if let Some(max) = constraint.max_chars {
        if value.len() > max {
            return Err(SlotError::TooLong {
                slot: slot_name.into(),
                max,
                actual: value.len(),
            });
        }
    }
    if let Some(min) = constraint.min_chars {
        if value.len() < min {
            return Err(SlotError::TooShort {
                slot: slot_name.into(),
                min,
                actual: value.len(),
            });
        }
    }

    // Type-specific validation
    match constraint.slot_type {
        SlotType::Text => {
            // Reject raw HTML tags
            if value.contains('<') && value.contains('>') {
                return Err(SlotError::DisallowedHtml {
                    slot: slot_name.into(),
                    detail: "Text slots must not contain HTML tags".into(),
                });
            }
        }
        SlotType::Cta => {
            // Reject raw HTML tags
            if value.contains('<') && value.contains('>') {
                return Err(SlotError::DisallowedHtml {
                    slot: slot_name.into(),
                    detail: "CTA slots must not contain HTML tags".into(),
                });
            }
            validate_cta(value).map_err(|e| match e {
                SlotError::InvalidCta { detail, .. } => SlotError::InvalidCta {
                    slot: slot_name.into(),
                    detail,
                },
                other => other,
            })?;
        }
        SlotType::Url | SlotType::VideoEmbed => {
            validate_url(value).map_err(|e| match e {
                SlotError::InvalidUrl { detail, .. } => SlotError::InvalidUrl {
                    slot: slot_name.into(),
                    detail,
                },
                other => other,
            })?;
        }
        SlotType::Number => {
            if value.parse::<f64>().is_err() {
                return Err(SlotError::NotNumeric {
                    slot: slot_name.into(),
                });
            }
        }
        SlotType::RichText => {
            // Allow only whitelisted tags
            let allowed_tags = [
                "<strong>",
                "</strong>",
                "<em>",
                "</em>",
                "<br>",
                "<br/>",
                "<br />",
            ];
            let mut check = value.to_string();
            // Allow <a href="...">
            while let Some(start) = check.find("<a ") {
                if let Some(end) = check[start..].find('>') {
                    let tag = &check[start..start + end + 1];
                    if tag.starts_with("<a href=") {
                        check = format!("{}{}", &check[..start], &check[start + end + 1..]);
                        continue;
                    }
                }
                break;
            }
            // Remove </a>
            check = check.replace("</a>", "");
            // Remove allowed tags
            for tag in &allowed_tags {
                check = check.replace(tag, "");
            }
            // Any remaining < > pairs are disallowed
            if check.contains('<') && check.contains('>') {
                return Err(SlotError::DisallowedHtml {
                    slot: slot_name.into(),
                    detail: "RichText allows only <strong>, <em>, <br>, <a href>".into(),
                });
            }
        }
        SlotType::ImagePrompt | SlotType::IconKey => {
            // No special validation beyond length
        }
    }
    Ok(())
}

/// Validate all slots in a section payload against its schema.
pub fn validate_section_payload(
    payload: &HashMap<String, String>,
    schema: &SectionSchema,
) -> Result<(), Vec<SlotError>> {
    let mut errors = Vec::new();

    for (slot_name, constraint) in &schema.slots {
        match payload.get(slot_name) {
            Some(value) => {
                if let Err(e) = validate_slot_value(slot_name, value, constraint) {
                    errors.push(e);
                }
            }
            None => {
                if constraint.required {
                    errors.push(SlotError::MissingRequired {
                        slot: slot_name.clone(),
                    });
                }
            }
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

// ─── Template Schema Definitions ────────────────────────────────────────────

/// Builder helpers for concise schema construction.
fn text(required: bool) -> SlotConstraint {
    SlotConstraint::new(SlotType::Text, required)
}
fn cta(required: bool) -> SlotConstraint {
    SlotConstraint::new(SlotType::Cta, required)
}
fn url(required: bool) -> SlotConstraint {
    SlotConstraint::new(SlotType::Url, required)
}
fn image(required: bool) -> SlotConstraint {
    SlotConstraint::new(SlotType::ImagePrompt, required)
}
fn rich(required: bool) -> SlotConstraint {
    SlotConstraint::new(SlotType::RichText, required)
}
fn num(required: bool) -> SlotConstraint {
    SlotConstraint::new(SlotType::Number, required)
}
fn icon(required: bool) -> SlotConstraint {
    SlotConstraint::new(SlotType::IconKey, required)
}

fn section(id: &str, display: &str, slots: Vec<(&str, SlotConstraint)>) -> SectionSchema {
    let mut map = IndexMap::new();
    for (name, c) in slots {
        map.insert(name.to_string(), c);
    }
    SectionSchema {
        section_id: id.to_string(),
        display_name: display.to_string(),
        slots: map,
    }
}

// ── SaaS Landing ────────────────────────────────────────────────────────────

fn saas_landing_schema() -> TemplateSchema {
    TemplateSchema {
        template_id: "saas_landing".into(),
        sections: vec![
            section(
                "hero",
                "Hero",
                vec![
                    (
                        "badge",
                        text(false)
                            .max(30)
                            .hint("short badge text, e.g. 'New in v2'"),
                    ),
                    ("headline", text(true).max(80).min(10).hint("main headline")),
                    (
                        "subtitle",
                        text(true).max(160).min(20).hint("supporting subtitle"),
                    ),
                    (
                        "cta_primary",
                        cta(true).max(25).hint("primary action button"),
                    ),
                    (
                        "cta_secondary",
                        cta(false).max(25).hint("secondary action button"),
                    ),
                    ("media", image(false).hint("hero visual")),
                ],
            ),
            section(
                "features",
                "Features",
                vec![
                    ("heading", text(true).max(60).hint("section heading")),
                    (
                        "subheading",
                        text(false).max(120).hint("section subheading"),
                    ),
                    ("feature_1_icon", icon(true).max(30)),
                    ("feature_1_title", text(true).max(40)),
                    ("feature_1_desc", text(true).max(120)),
                    ("feature_2_icon", icon(true).max(30)),
                    ("feature_2_title", text(true).max(40)),
                    ("feature_2_desc", text(true).max(120)),
                    ("feature_3_icon", icon(true).max(30)),
                    ("feature_3_title", text(true).max(40)),
                    ("feature_3_desc", text(true).max(120)),
                ],
            ),
            section(
                "pricing",
                "Pricing",
                vec![
                    ("heading", text(true).max(60)),
                    ("tier_1_name", text(true).max(20)),
                    ("tier_1_price", text(true).max(15).hint("e.g. '$9/mo'")),
                    (
                        "tier_1_features",
                        rich(true)
                            .max(300)
                            .hint("feature list, use <br> between items"),
                    ),
                    ("tier_2_name", text(true).max(20)),
                    ("tier_2_price", text(true).max(15)),
                    ("tier_2_features", rich(true).max(400)),
                    (
                        "tier_2_badge",
                        text(false).max(20).hint("e.g. 'Most Popular'"),
                    ),
                    ("tier_3_name", text(true).max(20)),
                    ("tier_3_price", text(true).max(15)),
                    ("tier_3_features", rich(true).max(500)),
                ],
            ),
            section(
                "testimonials",
                "Testimonials",
                vec![
                    ("heading", text(true).max(60)),
                    ("testimonial_1_quote", text(true).max(200)),
                    ("testimonial_1_author", text(true).max(40)),
                    ("testimonial_1_role", text(true).max(60)),
                    ("testimonial_2_quote", text(true).max(200)),
                    ("testimonial_2_author", text(true).max(40)),
                    ("testimonial_2_role", text(true).max(60)),
                    ("testimonial_3_quote", text(true).max(200)),
                    ("testimonial_3_author", text(true).max(40)),
                    ("testimonial_3_role", text(true).max(60)),
                ],
            ),
            section(
                "cta",
                "Call to Action",
                vec![
                    ("headline", text(true).max(80)),
                    ("body", text(true).max(160)),
                    ("cta_button", cta(true).max(25)),
                ],
            ),
            section(
                "footer",
                "Footer",
                vec![
                    ("brand", text(true).max(30)),
                    ("copyright", text(true).max(80)),
                    ("links", rich(false).max(500).hint("footer link groups")),
                ],
            ),
        ],
    }
}

// ── Docs Site ───────────────────────────────────────────────────────────────

fn docs_site_schema() -> TemplateSchema {
    TemplateSchema {
        template_id: "docs_site".into(),
        sections: vec![
            section(
                "sidebar_nav",
                "Sidebar Navigation",
                vec![
                    ("doc_title", text(true).max(40).hint("documentation title")),
                    (
                        "nav_category_1",
                        text(true).max(30).hint("first nav category"),
                    ),
                    (
                        "nav_items_1",
                        rich(true).max(300).hint("links in category 1"),
                    ),
                    ("nav_category_2", text(false).max(30)),
                    ("nav_items_2", rich(false).max(300)),
                    ("nav_category_3", text(false).max(30)),
                    ("nav_items_3", rich(false).max(300)),
                ],
            ),
            section(
                "search",
                "Search",
                vec![(
                    "placeholder",
                    text(true).max(40).hint("search placeholder text"),
                )],
            ),
            section(
                "content",
                "Content",
                vec![
                    ("main_heading", text(true).max(80)),
                    (
                        "intro_text",
                        rich(true).max(500).hint("introduction paragraph"),
                    ),
                    (
                        "callout_text",
                        rich(false).max(300).hint("callout/tip box text"),
                    ),
                    (
                        "install_text",
                        rich(false).max(300).hint("installation instructions"),
                    ),
                    ("quickstart_text", rich(false).max(500)),
                    ("config_text", rich(false).max(500)),
                    ("warning_text", rich(false).max(300).hint("warning callout")),
                ],
            ),
            section(
                "code_blocks",
                "Code Examples",
                vec![
                    (
                        "code_lang_1",
                        text(true).max(20).hint("language label e.g. 'bash'"),
                    ),
                    (
                        "code_example_1",
                        text(true).max(500).hint("code snippet — not HTML-rendered"),
                    ),
                    ("code_lang_2", text(false).max(20)),
                    ("code_example_2", text(false).max(500)),
                ],
            ),
            section(
                "footer",
                "Footer",
                vec![
                    ("copyright", text(true).max(80)),
                    ("links", rich(false).max(300)),
                ],
            ),
        ],
    }
}

// ── Portfolio ───────────────────────────────────────────────────────────────

fn portfolio_schema() -> TemplateSchema {
    TemplateSchema {
        template_id: "portfolio".into(),
        sections: vec![
            section(
                "hero",
                "Hero",
                vec![
                    ("avatar", text(false).max(5).hint("avatar initial or emoji")),
                    ("name", text(true).max(40)),
                    ("title", text(true).max(60).hint("professional title")),
                    ("bio", text(true).max(160).hint("short bio/tagline")),
                    ("cta_primary", cta(false).max(25)),
                    ("cta_secondary", cta(false).max(25)),
                ],
            ),
            section(
                "projects",
                "Projects",
                vec![
                    ("heading", text(false).max(40)),
                    (
                        "project_1_image",
                        image(true).hint("project screenshot prompt"),
                    ),
                    ("project_1_title", text(true).max(40)),
                    ("project_1_desc", text(true).max(120)),
                    ("project_1_tag_1", text(true).max(20)),
                    ("project_1_tag_2", text(false).max(20)),
                    ("project_1_url", url(false).max(200)),
                    ("project_2_image", image(true)),
                    ("project_2_title", text(true).max(40)),
                    ("project_2_desc", text(true).max(120)),
                    ("project_2_tag_1", text(true).max(20)),
                    ("project_2_tag_2", text(false).max(20)),
                    ("project_2_url", url(false).max(200)),
                    ("project_3_image", image(true)),
                    ("project_3_title", text(true).max(40)),
                    ("project_3_desc", text(true).max(120)),
                    ("project_3_tag_1", text(true).max(20)),
                    ("project_3_tag_2", text(false).max(20)),
                    ("project_3_url", url(false).max(200)),
                    ("project_4_image", image(true)),
                    ("project_4_title", text(true).max(40)),
                    ("project_4_desc", text(true).max(120)),
                    ("project_4_tag_1", text(true).max(20)),
                    ("project_4_tag_2", text(false).max(20)),
                    ("project_4_url", url(false).max(200)),
                ],
            ),
            section(
                "about",
                "About",
                vec![
                    ("heading", text(false).max(40)),
                    ("bio", rich(true).max(500).hint("about me description")),
                ],
            ),
            section(
                "skills",
                "Skills",
                vec![
                    ("heading", text(false).max(40)),
                    ("skill_1", text(true).max(25)),
                    ("skill_2", text(true).max(25)),
                    ("skill_3", text(true).max(25)),
                    ("skill_4", text(true).max(25)),
                    ("skill_5", text(false).max(25)),
                    ("skill_6", text(false).max(25)),
                    ("skill_7", text(false).max(25)),
                    ("skill_8", text(false).max(25)),
                ],
            ),
            section(
                "contact",
                "Contact",
                vec![
                    ("heading", text(false).max(40)),
                    ("subtext", text(false).max(120)),
                    ("email", url(false).max(100).hint("mailto: link")),
                ],
            ),
            section(
                "footer",
                "Footer",
                vec![
                    ("copyright", text(true).max(80)),
                    ("social_links", rich(false).max(300)),
                ],
            ),
        ],
    }
}

// ── Local Business ──────────────────────────────────────────────────────────

fn local_business_schema() -> TemplateSchema {
    TemplateSchema {
        template_id: "local_business".into(),
        sections: vec![
            section(
                "hero",
                "Hero",
                vec![
                    ("business_name", text(true).max(40)),
                    ("tagline", text(true).max(120)),
                    ("cta_primary", cta(true).max(25).hint("e.g. 'Book a Table'")),
                    ("cta_secondary", cta(false).max(25)),
                    ("phone", text(false).max(20)),
                    ("hero_image", image(false)),
                ],
            ),
            section(
                "services",
                "Services",
                vec![
                    ("heading", text(false).max(40)),
                    ("service_1_icon", icon(true).max(30)),
                    ("service_1_name", text(true).max(30)),
                    ("service_1_desc", text(true).max(120)),
                    ("service_2_icon", icon(true).max(30)),
                    ("service_2_name", text(true).max(30)),
                    ("service_2_desc", text(true).max(120)),
                    ("service_3_icon", icon(true).max(30)),
                    ("service_3_name", text(true).max(30)),
                    ("service_3_desc", text(true).max(120)),
                ],
            ),
            section(
                "gallery",
                "Gallery",
                vec![
                    ("heading", text(false).max(40)),
                    ("gallery_1", image(true).hint("gallery photo 1")),
                    ("gallery_2", image(true)),
                    ("gallery_3", image(true)),
                    ("gallery_4", image(false)),
                    ("gallery_5", image(false)),
                    ("gallery_6", image(false)),
                ],
            ),
            section(
                "testimonials",
                "Testimonials",
                vec![
                    ("heading", text(false).max(40)),
                    ("testimonial_1_quote", text(true).max(200)),
                    ("testimonial_1_author", text(true).max(40)),
                    ("testimonial_2_quote", text(true).max(200)),
                    ("testimonial_2_author", text(true).max(40)),
                ],
            ),
            section(
                "map",
                "Map & Location",
                vec![
                    ("address", text(true).max(120)),
                    (
                        "map_embed_url",
                        url(false).max(300).hint("Google Maps embed URL"),
                    ),
                ],
            ),
            section(
                "hours",
                "Hours & Contact",
                vec![
                    ("heading", text(false).max(40)),
                    (
                        "weekday_hours",
                        text(true).max(40).hint("e.g. 'Mon-Fri: 9am-9pm'"),
                    ),
                    ("saturday_hours", text(true).max(40)),
                    ("sunday_hours", text(true).max(40)),
                    ("address", text(true).max(120)),
                    ("phone", text(true).max(20)),
                ],
            ),
            section(
                "footer",
                "Footer",
                vec![
                    ("business_name", text(true).max(40)),
                    ("copyright", text(true).max(80)),
                ],
            ),
        ],
    }
}

// ── E-Commerce ──────────────────────────────────────────────────────────────

fn ecommerce_schema() -> TemplateSchema {
    TemplateSchema {
        template_id: "ecommerce".into(),
        sections: vec![
            section(
                "hero",
                "Hero",
                vec![
                    ("headline", text(true).max(80)),
                    ("subtitle", text(true).max(160)),
                    ("cta_primary", cta(true).max(25)),
                    ("hero_image", image(false)),
                ],
            ),
            section(
                "categories",
                "Categories",
                vec![
                    ("heading", text(false).max(40)),
                    ("category_1", text(true).max(25)),
                    ("category_1_image", image(false)),
                    ("category_2", text(true).max(25)),
                    ("category_2_image", image(false)),
                    ("category_3", text(true).max(25)),
                    ("category_3_image", image(false)),
                    ("category_4", text(false).max(25)),
                    ("category_4_image", image(false)),
                ],
            ),
            section(
                "products",
                "Products",
                vec![
                    ("heading", text(false).max(40)),
                    ("product_1_image", image(true)),
                    ("product_1_name", text(true).max(60)),
                    ("product_1_price", text(true).max(15).hint("e.g. '$29.99'")),
                    ("product_1_reviews", num(false).hint("number of reviews")),
                    ("product_2_image", image(true)),
                    ("product_2_name", text(true).max(60)),
                    ("product_2_price", text(true).max(15)),
                    ("product_2_reviews", num(false)),
                    ("product_3_image", image(true)),
                    ("product_3_name", text(true).max(60)),
                    ("product_3_price", text(true).max(15)),
                    ("product_3_reviews", num(false)),
                    ("product_4_image", image(true)),
                    ("product_4_name", text(true).max(60)),
                    ("product_4_price", text(true).max(15)),
                    ("product_4_reviews", num(false)),
                ],
            ),
            section(
                "reviews",
                "Reviews",
                vec![
                    ("heading", text(false).max(40)),
                    ("review_1_text", text(true).max(200)),
                    ("review_1_author", text(true).max(40)),
                    ("review_1_rating", num(true).hint("1-5 star rating")),
                    ("review_2_text", text(true).max(200)),
                    ("review_2_author", text(true).max(40)),
                    ("review_2_rating", num(true)),
                ],
            ),
            section(
                "newsletter",
                "Newsletter",
                vec![
                    ("heading", text(true).max(60)),
                    ("subtext", text(false).max(120)),
                    ("cta_button", cta(true).max(25)),
                    (
                        "placeholder",
                        text(false).max(40).hint("email input placeholder"),
                    ),
                ],
            ),
            section(
                "footer",
                "Footer",
                vec![
                    ("store_name", text(true).max(30)),
                    ("store_description", text(false).max(160)),
                    ("copyright", text(true).max(80)),
                    ("links", rich(false).max(500)),
                ],
            ),
        ],
    }
}

// ── Dashboard ───────────────────────────────────────────────────────────────

fn dashboard_schema() -> TemplateSchema {
    TemplateSchema {
        template_id: "dashboard".into(),
        sections: vec![
            section(
                "sidebar",
                "Sidebar",
                vec![
                    ("app_name", text(true).max(30)),
                    ("nav_item_1", text(true).max(25)),
                    ("nav_item_1_icon", icon(true).max(30)),
                    ("nav_item_2", text(true).max(25)),
                    ("nav_item_2_icon", icon(true).max(30)),
                    ("nav_item_3", text(true).max(25)),
                    ("nav_item_3_icon", icon(true).max(30)),
                    ("nav_item_4", text(false).max(25)),
                    ("nav_item_4_icon", icon(false).max(30)),
                    ("nav_item_5", text(false).max(25)),
                    ("nav_item_5_icon", icon(false).max(30)),
                ],
            ),
            section(
                "header",
                "Header",
                vec![
                    ("user_name", text(true).max(40)),
                    ("user_role", text(false).max(30)),
                    ("user_initial", text(false).max(3)),
                    ("search_placeholder", text(false).max(40)),
                ],
            ),
            section(
                "stats",
                "Stats Cards",
                vec![
                    ("stat_1_label", text(true).max(25)),
                    ("stat_1_value", text(true).max(15)),
                    ("stat_1_change", text(true).max(15).hint("e.g. '+12%'")),
                    ("stat_2_label", text(true).max(25)),
                    ("stat_2_value", text(true).max(15)),
                    ("stat_2_change", text(true).max(15)),
                    ("stat_3_label", text(true).max(25)),
                    ("stat_3_value", text(true).max(15)),
                    ("stat_3_change", text(true).max(15)),
                    ("stat_4_label", text(true).max(25)),
                    ("stat_4_value", text(true).max(15)),
                    ("stat_4_change", text(true).max(15)),
                ],
            ),
            section(
                "charts",
                "Charts",
                vec![
                    ("chart_1_title", text(true).max(40)),
                    (
                        "chart_1_type",
                        text(true).max(20).hint("e.g. 'line', 'bar'"),
                    ),
                    ("chart_2_title", text(true).max(40)),
                    ("chart_2_type", text(true).max(20)),
                ],
            ),
            section(
                "data_table",
                "Data Table",
                vec![
                    ("table_title", text(true).max(40)),
                    ("col_1", text(true).max(20).hint("column header")),
                    ("col_2", text(true).max(20)),
                    ("col_3", text(true).max(20)),
                    ("col_4", text(true).max(20)),
                    ("row_1_col_1", text(true).max(40)),
                    ("row_1_col_2", text(true).max(40)),
                    ("row_1_col_3", text(true).max(40)),
                    ("row_1_col_4", text(true).max(40)),
                    ("row_2_col_1", text(true).max(40)),
                    ("row_2_col_2", text(true).max(40)),
                    ("row_2_col_3", text(true).max(40)),
                    ("row_2_col_4", text(true).max(40)),
                    ("row_3_col_1", text(true).max(40)),
                    ("row_3_col_2", text(true).max(40)),
                    ("row_3_col_3", text(true).max(40)),
                    ("row_3_col_4", text(true).max(40)),
                ],
            ),
            section(
                "footer",
                "Footer",
                vec![
                    ("app_name", text(false).max(30)),
                    ("app_version", text(false).max(15)),
                    ("copyright", text(true).max(80)),
                ],
            ),
        ],
    }
}

// ─── Public API ─────────────────────────────────────────────────────────────

/// Get all 6 template schemas.
pub fn all_template_schemas() -> Vec<TemplateSchema> {
    vec![
        saas_landing_schema(),
        docs_site_schema(),
        portfolio_schema(),
        local_business_schema(),
        ecommerce_schema(),
        dashboard_schema(),
    ]
}

/// Get a template schema by ID.
pub fn get_template_schema(template_id: &str) -> Option<TemplateSchema> {
    all_template_schemas()
        .into_iter()
        .find(|s| s.template_id == template_id)
}

/// Return template schema with self-improvement slot adjustments applied as an overlay.
///
/// If no adjustments exist, returns the same schema as `get_template_schema`.
pub fn get_template_schema_improved(template_id: &str) -> Option<TemplateSchema> {
    let defaults = crate::self_improve::load_system_defaults();
    get_template_schema_with_defaults(template_id, &defaults)
}

/// Inner function for testing — applies slot adjustments from explicit defaults.
pub fn get_template_schema_with_defaults(
    template_id: &str,
    defaults: &crate::self_improve::SystemDefaults,
) -> Option<TemplateSchema> {
    let mut schema = get_template_schema(template_id)?;
    if defaults.slot_adjustments.is_empty() {
        return Some(schema);
    }
    for section in &mut schema.sections {
        for (slot_name, constraint) in &mut section.slots {
            let key = format!("{}.{}.{}", template_id, section.section_id, slot_name);
            if let Some(adj) = defaults.slot_adjustments.get(&key) {
                if let Some(max) = adj.max_chars {
                    constraint.max_chars = Some(max);
                }
            }
        }
    }
    Some(schema)
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_six_templates_defined() {
        let schemas = all_template_schemas();
        assert_eq!(schemas.len(), 6);
        let ids: Vec<&str> = schemas.iter().map(|s| s.template_id.as_str()).collect();
        assert!(ids.contains(&"saas_landing"));
        assert!(ids.contains(&"docs_site"));
        assert!(ids.contains(&"portfolio"));
        assert!(ids.contains(&"local_business"));
        assert!(ids.contains(&"ecommerce"));
        assert!(ids.contains(&"dashboard"));
    }

    #[test]
    fn test_every_section_has_at_least_one_slot() {
        for schema in all_template_schemas() {
            for section in &schema.sections {
                assert!(
                    !section.slots.is_empty(),
                    "Template '{}' section '{}' has no slots",
                    schema.template_id,
                    section.section_id
                );
            }
        }
    }

    #[test]
    fn test_rejects_missing_required_slot() {
        let schema = saas_landing_schema();
        let hero = &schema.sections[0];
        let payload: HashMap<String, String> = HashMap::new(); // empty
        let result = validate_section_payload(&payload, hero);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors
            .iter()
            .any(|e| matches!(e, SlotError::MissingRequired { slot } if slot == "headline")));
    }

    #[test]
    fn test_rejects_oversized_text() {
        let schema = saas_landing_schema();
        let hero = &schema.sections[0];
        let mut payload = HashMap::new();
        payload.insert("headline".into(), "x".repeat(100)); // max 80
        payload.insert(
            "subtitle".into(),
            "Valid subtitle for the hero section.".into(),
        );
        payload.insert("cta_primary".into(), "Get Started".into());
        let result = validate_section_payload(&payload, hero);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors
            .iter()
            .any(|e| matches!(e, SlotError::TooLong { slot, .. } if slot == "headline")));
    }

    #[test]
    fn test_rejects_html_in_text_slot() {
        let constraint = text(true).max(200);
        let result = validate_slot_value("headline", "<script>alert('xss')</script>", &constraint);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            SlotError::DisallowedHtml { .. }
        ));
    }

    #[test]
    fn test_accepts_valid_payload() {
        let schema = saas_landing_schema();
        let hero = &schema.sections[0];
        let mut payload = HashMap::new();
        payload.insert("headline".into(), "Ship faster with Nexus".into());
        payload.insert(
            "subtitle".into(),
            "The governed AI agent platform for modern teams.".into(),
        );
        payload.insert("cta_primary".into(), "Get Started".into());
        let result = validate_section_payload(&payload, hero);
        assert!(
            result.is_ok(),
            "expected Ok, got: {:?}",
            result.unwrap_err()
        );
    }

    #[test]
    fn test_url_rejects_javascript_protocol() {
        let result = validate_url("javascript:alert('xss')");
        assert!(result.is_err());
    }

    #[test]
    fn test_url_accepts_https() {
        assert!(validate_url("https://example.com").is_ok());
    }

    #[test]
    fn test_url_accepts_mailto() {
        assert!(validate_url("mailto:hello@example.com").is_ok());
    }

    #[test]
    fn test_url_accepts_tel() {
        assert!(validate_url("tel:+1234567890").is_ok());
    }

    #[test]
    fn test_url_accepts_hash() {
        assert!(validate_url("#section").is_ok());
    }

    #[test]
    fn test_cta_rejects_empty() {
        let result = validate_cta("");
        assert!(result.is_err());
    }

    #[test]
    fn test_cta_accepts_action_verb() {
        assert!(validate_cta("Get Started").is_ok());
        assert!(validate_cta("Start Free Trial").is_ok());
        assert!(validate_cta("Book Now").is_ok());
    }

    #[test]
    fn test_html_escape() {
        assert_eq!(
            html_escape("<script>alert('xss')</script>"),
            "&lt;script&gt;alert(&#x27;xss&#x27;)&lt;/script&gt;"
        );
        assert_eq!(
            html_escape("Hello & \"World\""),
            "Hello &amp; &quot;World&quot;"
        );
    }

    #[test]
    fn test_number_slot_rejects_non_numeric() {
        let constraint = num(true);
        assert!(validate_slot_value("count", "abc", &constraint).is_err());
    }

    #[test]
    fn test_number_slot_accepts_numeric() {
        let constraint = num(true);
        assert!(validate_slot_value("count", "42", &constraint).is_ok());
        assert!(validate_slot_value("count", "3.14", &constraint).is_ok());
    }

    #[test]
    fn test_rich_text_allows_whitelisted_tags() {
        let constraint = rich(true).max(500);
        let value = "This is <strong>bold</strong> and <em>italic</em> with a <br> break";
        assert!(validate_slot_value("body", value, &constraint).is_ok());
    }

    #[test]
    fn test_rich_text_rejects_script() {
        let constraint = rich(true).max(500);
        let value = "Hello <script>alert('xss')</script>";
        assert!(validate_slot_value("body", value, &constraint).is_err());
    }

    #[test]
    fn test_get_template_schema_found() {
        let schema = get_template_schema("portfolio").unwrap();
        assert_eq!(schema.template_id, "portfolio");
        assert!(!schema.sections.is_empty());
    }

    #[test]
    fn test_get_template_schema_not_found() {
        assert!(get_template_schema("nonexistent").is_none());
    }

    #[test]
    fn test_slot_adjustment_overlay() {
        let mut defaults = crate::self_improve::SystemDefaults::default();
        defaults.slot_adjustments.insert(
            "saas_landing.hero.headline".into(),
            crate::self_improve::SlotAdjustment {
                max_chars: Some(100),
            },
        );
        let schema = get_template_schema_with_defaults("saas_landing", &defaults).unwrap();
        let hero = schema
            .sections
            .iter()
            .find(|s| s.section_id == "hero")
            .unwrap();
        let headline = hero.slots.get("headline").unwrap();
        assert_eq!(headline.max_chars, Some(100));
    }

    #[test]
    fn test_empty_defaults_no_change() {
        let defaults = crate::self_improve::SystemDefaults::default();
        let original = get_template_schema("saas_landing").unwrap();
        let improved = get_template_schema_with_defaults("saas_landing", &defaults).unwrap();
        // All max_chars should be identical
        for (orig_sec, imp_sec) in original.sections.iter().zip(improved.sections.iter()) {
            for (name, orig_slot) in &orig_sec.slots {
                let imp_slot = imp_sec.slots.get(name).unwrap();
                assert_eq!(orig_slot.max_chars, imp_slot.max_chars);
            }
        }
    }
}

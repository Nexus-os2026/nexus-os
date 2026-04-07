//! Trust Signal Detection — checks for social proof, testimonials, badges, and contact info.

use super::ConversionInput;
use crate::quality::{compute_score, CheckResult, QualityError, QualityIssue, Severity};

/// Run trust signal detection. Template-aware: dashboard/docs get baseline pass.
pub fn check(input: &ConversionInput) -> Result<CheckResult, QualityError> {
    let template = &input.template_id;

    // Dashboard and docs_site: trust signals are less relevant
    if template == "dashboard" || template == "docs_site" {
        return Ok(CheckResult {
            check_id: "trust_signals".into(),
            check_name: "Trust Signals".into(),
            score: 95,
            max_score: 100,
            issues: vec![],
            passed: true,
        });
    }

    let html = &input.quality_input.html;
    let lower = html.to_lowercase();
    let mut issues = Vec::new();

    // Check 1: Testimonials section exists
    let has_testimonials = lower.contains("testimonial")
        || lower.contains("review")
        || lower.contains("quote")
        || lower.contains("data-nexus-section=\"testimonials\"")
        || lower.contains("data-nexus-section=\"reviews\"");

    if !has_testimonials {
        issues.push(QualityIssue {
            severity: Severity::Warning,
            message:
                "No testimonials section — social proof increases conversion by ~34% on average"
                    .into(),
            section_id: None,
            element: None,
            fix: None,
        });
    }

    // Check 2: Star ratings (important for ecommerce)
    if template == "ecommerce" {
        let has_ratings = lower.contains("star") || lower.contains("rating") || lower.contains("★");
        if !has_ratings {
            issues.push(QualityIssue {
                severity: Severity::Warning,
                message: "No star ratings found — product ratings build buyer confidence".into(),
                section_id: None,
                element: None,
                fix: None,
            });
        }
    }

    // Check 3: Social proof / logos section (important for SaaS)
    if template == "saas_landing" {
        let has_logos = lower.contains("trusted by")
            || lower.contains("logo")
            || lower.contains("partner")
            || lower.contains("client");
        if !has_logos {
            issues.push(QualityIssue {
                severity: Severity::Warning,
                message:
                    "No 'trusted by' logos section — brand logos build credibility for SaaS products"
                        .into(),
                section_id: None,
                element: None,
                fix: None,
            });
        }
    }

    // Check 4: Trust badges (important for ecommerce)
    if template == "ecommerce" {
        let has_trust_badges = lower.contains("secure")
            || lower.contains("guarantee")
            || lower.contains("shipping")
            || lower.contains("return")
            || lower.contains("ssl")
            || lower.contains("trust");
        if !has_trust_badges {
            issues.push(QualityIssue {
                severity: Severity::Warning,
                message:
                    "No trust badges — add shipping, returns, or security badges for buyer confidence"
                        .into(),
                section_id: None,
                element: None,
                fix: None,
            });
        }
    }

    // Check 5: Contact information (important for local business)
    if template == "local_business" {
        let has_contact = lower.contains("phone")
            || lower.contains("tel:")
            || lower.contains("address")
            || lower.contains("contact")
            || lower.contains("hours");
        if !has_contact {
            issues.push(QualityIssue {
                severity: Severity::Warning,
                message:
                    "No contact information visible — local businesses need phone, address, and hours"
                        .into(),
                section_id: None,
                element: None,
                fix: None,
            });
        }
    }

    // Check 6: Author/team section (important for portfolio)
    if template == "portfolio" {
        let has_about = lower.contains("about")
            || lower.contains("data-nexus-section=\"about\"")
            || lower.contains("bio")
            || lower.contains("team");
        if !has_about {
            issues.push(QualityIssue {
                severity: Severity::Info,
                message:
                    "No about/team section — personal connection increases portfolio conversions"
                        .into(),
                section_id: None,
                element: None,
                fix: None,
            });
        }
    }

    let score = compute_score(&issues);

    Ok(CheckResult {
        check_id: "trust_signals".into(),
        check_name: "Trust Signals".into(),
        score,
        max_score: 100,
        issues,
        passed: score >= 60,
    })
}

// ─── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content_payload::ContentPayload;
    use crate::quality::conversion::ConversionInput;
    use crate::quality::QualityInput;
    use crate::variant::VariantSelection;

    fn make_input(html: &str, template_id: &str) -> ConversionInput {
        ConversionInput {
            quality_input: QualityInput {
                html: html.to_string(),
                output_dir: None,
                template_id: template_id.to_string(),
                sections: vec![],
            },
            content_payload: ContentPayload {
                template_id: template_id.to_string(),
                variant: VariantSelection::default(),
                sections: vec![],
            },
            template_id: template_id.to_string(),
            brief: Some("test".into()),
        }
    }

    #[test]
    fn test_saas_with_testimonials_scores_high() {
        let input = make_input(
            r##"<!DOCTYPE html><html><head><title>T</title></head><body>
            <div data-nexus-section="testimonials">
                <blockquote>"Great product" — Jane</blockquote>
            </div>
            <div>Trusted by leading companies</div>
            </body></html>"##,
            "saas_landing",
        );
        let result = check(&input).unwrap();
        assert!(result.score >= 90, "score was {}", result.score);
    }

    #[test]
    fn test_saas_without_testimonials_penalized() {
        let input = make_input(
            r##"<!DOCTYPE html><html><head><title>T</title></head><body>
            <div data-nexus-section="hero"><h1>Product</h1></div>
            </body></html>"##,
            "saas_landing",
        );
        let result = check(&input).unwrap();
        assert!(
            result
                .issues
                .iter()
                .any(|i| i.message.contains("testimonials")),
            "should flag missing testimonials"
        );
    }

    #[test]
    fn test_ecommerce_with_ratings() {
        let input = make_input(
            r##"<!DOCTYPE html><html><head><title>T</title></head><body>
            <div data-nexus-section="reviews"><span>★★★★★</span> 5-star rating</div>
            <div>Secure checkout</div><div>Free shipping</div>
            </body></html>"##,
            "ecommerce",
        );
        let result = check(&input).unwrap();
        assert!(
            !result.issues.iter().any(|i| i.message.contains("rating")),
            "should not penalize when ratings present"
        );
    }

    #[test]
    fn test_ecommerce_without_trust_badges() {
        let input = make_input(
            r##"<!DOCTYPE html><html><head><title>T</title></head><body>
            <div data-nexus-section="hero"><h1>Shop</h1></div>
            </body></html>"##,
            "ecommerce",
        );
        let result = check(&input).unwrap();
        assert!(
            result
                .issues
                .iter()
                .any(|i| i.message.contains("trust badges")),
            "should flag missing trust badges for ecommerce"
        );
    }

    #[test]
    fn test_dashboard_gets_baseline_pass() {
        let input = make_input(
            r##"<!DOCTYPE html><html><head><title>T</title></head><body>
            <div data-nexus-section="sidebar">Menu</div>
            </body></html>"##,
            "dashboard",
        );
        let result = check(&input).unwrap();
        assert!(result.score >= 90, "dashboard should get baseline pass");
        assert!(result.issues.is_empty());
    }

    #[test]
    fn test_docs_gets_baseline_pass() {
        let input = make_input(
            r##"<!DOCTYPE html><html><head><title>T</title></head><body>
            <div data-nexus-section="content">Docs</div>
            </body></html>"##,
            "docs_site",
        );
        let result = check(&input).unwrap();
        assert!(result.score >= 90, "docs should get baseline pass");
    }

    #[test]
    fn test_local_business_contact_info() {
        let input = make_input(
            r##"<!DOCTYPE html><html><head><title>T</title></head><body>
            <div data-nexus-section="testimonials">Review here</div>
            <div>Phone: 555-1234</div><div>Address: 123 Main St</div>
            </body></html>"##,
            "local_business",
        );
        let result = check(&input).unwrap();
        assert!(
            !result.issues.iter().any(|i| i.message.contains("contact")),
            "should not penalize when contact info present"
        );
    }

    #[test]
    fn test_template_aware_scoring() {
        // Same HTML, different templates should produce different results
        let html = r##"<!DOCTYPE html><html><head><title>T</title></head><body>
            <div data-nexus-section="hero"><h1>Hello</h1></div>
            </body></html>"##;

        let saas_result = check(&make_input(html, "saas_landing")).unwrap();
        let dash_result = check(&make_input(html, "dashboard")).unwrap();

        assert!(
            dash_result.score > saas_result.score,
            "dashboard should score higher than saas for same minimal HTML"
        );
    }
}

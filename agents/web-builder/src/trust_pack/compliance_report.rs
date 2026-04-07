//! Compliance Report — styled HTML report for printing to PDF.
//!
//! Generates a professional, self-contained HTML document that covers:
//! provenance, quality, costs, dependencies, deploys, and signature.
//! Print to PDF from any browser for a SOC 2 audit-ready report.

use super::audit_trail::AuditEvent;
use super::build_manifest::BuildManifest;

/// Generate a self-contained HTML compliance report.
///
/// Uses inline styles (no external CSS) so it renders correctly in any browser.
pub fn generate_compliance_html(
    manifest: &BuildManifest,
    audit_events: &[AuditEvent],
    template_id: &str,
) -> String {
    let models_rows = manifest
        .models_used
        .iter()
        .map(|m| {
            format!(
                "<tr><td style=\"padding:8px;border-bottom:1px solid #e2e8f0;\">{}</td>\
                 <td style=\"padding:8px;border-bottom:1px solid #e2e8f0;\">{}</td>\
                 <td style=\"padding:8px;border-bottom:1px solid #e2e8f0;\">{}</td>\
                 <td style=\"padding:8px;border-bottom:1px solid #e2e8f0;\">${:.4}</td></tr>",
                m.model_name, m.purpose, m.invocation_count, m.cost_usd,
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    let audit_rows = audit_events
        .iter()
        .map(|e| {
            let color = event_color(&e.event_type);
            format!(
                "<tr><td style=\"padding:6px 8px;border-bottom:1px solid #f1f5f9;\">\
                 <span style=\"display:inline-block;width:8px;height:8px;border-radius:50%;background:{color};margin-right:6px;\"></span>\
                 {}</td>\
                 <td style=\"padding:6px 8px;border-bottom:1px solid #f1f5f9;font-family:monospace;font-size:12px;\">{}</td>\
                 <td style=\"padding:6px 8px;border-bottom:1px solid #f1f5f9;\">{}</td></tr>",
                e.event_type, e.timestamp, e.description,
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    let qs = &manifest.quality_scores;
    let signature_section = if let Some(ref sig) = manifest.signature {
        let pk = manifest.signer_public_key.as_deref().unwrap_or("N/A");
        format!(
            "<h2 style=\"color:#1e293b;margin-top:32px;\">Digital Signature</h2>\
             <p style=\"color:#059669;\"><strong>Status: Signed (Ed25519)</strong></p>\
             <table style=\"width:100%;border-collapse:collapse;margin-top:8px;\">\
             <tr><td style=\"padding:6px 8px;font-weight:600;width:160px;\">Public Key</td>\
             <td style=\"padding:6px 8px;font-family:monospace;font-size:11px;word-break:break-all;\">{pk}</td></tr>\
             <tr><td style=\"padding:6px 8px;font-weight:600;\">Signature</td>\
             <td style=\"padding:6px 8px;font-family:monospace;font-size:11px;word-break:break-all;\">{}</td></tr>\
             </table>\
             <p style=\"color:#64748b;font-size:13px;margin-top:8px;\">Verify: reconstruct canonical JSON (excluding signature fields), then verify Ed25519 signature against the public key.</p>",
            &sig[..sig.len().min(64)],
        )
    } else {
        "<h2 style=\"color:#1e293b;margin-top:32px;\">Digital Signature</h2>\
         <p style=\"color:#94a3b8;\">Signature: pending (Ed25519 key not configured)</p>"
            .to_string()
    };

    format!(
        r##"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>Nexus Builder Compliance Report — {project_name}</title>
<style>
  @media print {{ body {{ font-size: 11px; }} h1 {{ font-size: 20px; }} }}
  body {{ font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; max-width: 900px; margin: 0 auto; padding: 40px 24px; color: #1e293b; line-height: 1.6; }}
  .header {{ border-bottom: 3px solid #6366f1; padding-bottom: 16px; margin-bottom: 24px; }}
  .score-bar {{ display: inline-block; height: 8px; border-radius: 4px; margin-right: 8px; }}
  table {{ width: 100%; border-collapse: collapse; }}
  th {{ text-align: left; padding: 8px; border-bottom: 2px solid #e2e8f0; color: #64748b; font-size: 13px; text-transform: uppercase; letter-spacing: 0.05em; }}
</style>
</head>
<body>

<div class="header">
  <h1 style="margin:0;color:#6366f1;">Nexus Builder Compliance Report</h1>
  <p style="margin:4px 0 0;color:#64748b;">Project: <strong>{project_name}</strong> | Build: <code>{build_id}</code> | Date: {timestamp}</p>
  <p style="margin:2px 0 0;color:#64748b;font-family:monospace;font-size:12px;">SHA-256: {build_hash}</p>
</div>

<h2 style="color:#1e293b;">Executive Summary</h2>
<p>This report documents the complete build provenance for <strong>{project_name}</strong>, built using the <code>{template_id}</code> template in <code>{output_mode}</code> mode. {model_count} AI model(s) were used across the build pipeline at a total cost of <strong>${total_cost:.4}</strong>. The quality score is <strong>{overall_score}/100</strong> with {issues_found} issues found and {issues_fixed} auto-fixed.</p>

<h2 style="color:#1e293b;margin-top:32px;">Quality Report Card</h2>
<table style="width:100%;border-collapse:collapse;">
  <tr><td style="padding:6px 0;width:140px;">Accessibility</td><td><span class="score-bar" style="width:{acc_w}px;background:#6366f1;"></span> {accessibility}/100</td></tr>
  <tr><td style="padding:6px 0;">SEO</td><td><span class="score-bar" style="width:{seo_w}px;background:#6366f1;"></span> {seo}/100</td></tr>
  <tr><td style="padding:6px 0;">Performance</td><td><span class="score-bar" style="width:{perf_w}px;background:#6366f1;"></span> {performance}/100</td></tr>
  <tr><td style="padding:6px 0;">Security</td><td><span class="score-bar" style="width:{sec_w}px;background:#6366f1;"></span> {security}/100</td></tr>
  <tr><td style="padding:6px 0;">HTML Validity</td><td><span class="score-bar" style="width:{html_w}px;background:#6366f1;"></span> {html_validity}/100</td></tr>
  <tr><td style="padding:6px 0;">Responsive</td><td><span class="score-bar" style="width:{resp_w}px;background:#6366f1;"></span> {responsive}/100</td></tr>
</table>

<h2 style="color:#1e293b;margin-top:32px;">Conversion Report Card</h2>
<table style="width:100%;border-collapse:collapse;">
  <tr><td style="padding:6px 0;width:140px;">CTA Placement</td><td><span class="score-bar" style="width:{cta_w}px;background:#a855f7;"></span> {cta_placement}/100</td></tr>
  <tr><td style="padding:6px 0;">Above the Fold</td><td><span class="score-bar" style="width:{fold_w}px;background:#a855f7;"></span> {above_fold}/100</td></tr>
  <tr><td style="padding:6px 0;">Trust Signals</td><td><span class="score-bar" style="width:{trust_w}px;background:#a855f7;"></span> {trust_signals}/100</td></tr>
  <tr><td style="padding:6px 0;">Copy Clarity</td><td><span class="score-bar" style="width:{copy_w}px;background:#a855f7;"></span> {copy_clarity}/100</td></tr>
</table>

<h2 style="color:#1e293b;margin-top:32px;">Model Attribution</h2>
<table>
  <thead><tr><th>Model</th><th>Purpose</th><th>Invocations</th><th>Cost</th></tr></thead>
  <tbody>{models_rows}</tbody>
</table>
<p style="color:#64748b;font-size:13px;margin-top:8px;"><strong>Total cost: ${total_cost:.4}</strong></p>

<h2 style="color:#1e293b;margin-top:32px;">Audit Trail ({event_count} events)</h2>
<table>
  <thead><tr><th>Event</th><th>Timestamp</th><th>Description</th></tr></thead>
  <tbody>{audit_rows}</tbody>
</table>

{deploy_section}

{signature_section}

<hr style="margin-top:40px;border:none;border-top:1px solid #e2e8f0;">
<p style="color:#94a3b8;font-size:12px;text-align:center;">Generated by Nexus Builder v{version} | Verification: nexus-os.dev/verify</p>

</body>
</html>"##,
        project_name = html_escape(&manifest.project_name),
        build_id = &manifest.build_id[..manifest.build_id.len().min(8)],
        timestamp = &manifest.timestamp,
        build_hash = &manifest.build_hash[..manifest.build_hash.len().min(16)],
        template_id = template_id,
        output_mode = &manifest.output_mode,
        model_count = manifest.models_used.len(),
        total_cost = manifest.total_cost_usd,
        overall_score = qs.overall,
        issues_found = manifest.issues_found,
        issues_fixed = manifest.issues_fixed,
        accessibility = qs.accessibility,
        seo = qs.seo,
        performance = qs.performance,
        security = qs.security,
        html_validity = qs.html_validity,
        responsive = qs.responsive,
        acc_w = qs.accessibility * 2,
        seo_w = qs.seo * 2,
        perf_w = qs.performance * 2,
        sec_w = qs.security * 2,
        html_w = qs.html_validity * 2,
        resp_w = qs.responsive * 2,
        cta_placement = manifest.conversion_scores.cta_placement,
        above_fold = manifest.conversion_scores.above_fold,
        trust_signals = manifest.conversion_scores.trust_signals,
        copy_clarity = manifest.conversion_scores.copy_clarity,
        cta_w = manifest.conversion_scores.cta_placement * 2,
        fold_w = manifest.conversion_scores.above_fold * 2,
        trust_w = manifest.conversion_scores.trust_signals * 2,
        copy_w = manifest.conversion_scores.copy_clarity * 2,
        event_count = audit_events.len(),
        deploy_section = build_deploy_section(manifest),
        version = env!("CARGO_PKG_VERSION"),
    )
}

fn build_deploy_section(manifest: &BuildManifest) -> String {
    if let Some(ref url) = manifest.deploy_url {
        let provider = manifest.deploy_provider.as_deref().unwrap_or("Unknown");
        format!(
            "<h2 style=\"color:#1e293b;margin-top:32px;\">Deployment</h2>\
             <table style=\"width:100%;border-collapse:collapse;\">\
             <tr><td style=\"padding:6px 8px;font-weight:600;width:120px;\">Provider</td>\
             <td style=\"padding:6px 8px;\">{provider}</td></tr>\
             <tr><td style=\"padding:6px 8px;font-weight:600;\">URL</td>\
             <td style=\"padding:6px 8px;\">{url}</td></tr>\
             <tr><td style=\"padding:6px 8px;font-weight:600;\">Build Hash</td>\
             <td style=\"padding:6px 8px;font-family:monospace;font-size:12px;\">{hash}</td></tr>\
             </table>",
            hash = manifest.deploy_hash.as_deref().unwrap_or("N/A"),
        )
    } else {
        "<h2 style=\"color:#1e293b;margin-top:32px;\">Deployment</h2>\
         <p style=\"color:#94a3b8;\">Not yet deployed.</p>"
            .to_string()
    }
}

fn event_color(event_type: &super::audit_trail::AuditEventType) -> &'static str {
    use super::audit_trail::AuditEventType;
    match event_type {
        AuditEventType::BuildStarted | AuditEventType::VisualEdit | AuditEventType::TextEdit => {
            "#3b82f6"
        }
        AuditEventType::BuildCompleted | AuditEventType::QualityCheck => "#22c55e",
        AuditEventType::AutoFix | AuditEventType::ThemeChange => "#eab308",
        AuditEventType::Deployed => "#8b5cf6",
        AuditEventType::Rollback => "#ef4444",
        _ => "#64748b",
    }
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

// ─── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trust_pack::build_manifest::{
        BuildManifest, ConversionScoresSummary, ModelUsage, QualityScoresSummary,
    };

    fn sample_manifest() -> BuildManifest {
        BuildManifest {
            project_id: "proj-001".into(),
            project_name: "AI Writer".into(),
            build_id: "build-abc-123".into(),
            build_hash: "deadbeef12345678".into(),
            timestamp: "2026-04-04T12:00:00Z".into(),
            template_id: "saas_landing".into(),
            output_mode: "Html".into(),
            models_used: vec![ModelUsage {
                model_name: "claude-sonnet-4-6".into(),
                purpose: "content_generation".into(),
                cost_usd: 0.15,
                invocation_count: 1,
            }],
            total_cost_usd: 0.15,
            quality_scores: QualityScoresSummary {
                accessibility: 95,
                seo: 90,
                performance: 88,
                security: 100,
                html_validity: 92,
                responsive: 85,
                overall: 91,
            },
            conversion_scores: ConversionScoresSummary {
                cta_placement: 88,
                above_fold: 95,
                trust_signals: 72,
                copy_clarity: 80,
                overall: 83,
            },
            issues_found: 3,
            issues_fixed: 3,
            external_dependency_count: 5,
            backend_provider: None,
            schema_hash: None,
            rls_policy_hash: None,
            deploy_provider: None,
            deploy_url: None,
            deploy_hash: None,
            signer_public_key: Some("abcd1234".into()),
            signature: Some("sig5678abcd".into()),
        }
    }

    fn sample_events() -> Vec<AuditEvent> {
        vec![AuditEvent {
            id: "evt-0001".into(),
            timestamp: "2026-04-04T12:00:00Z".into(),
            event_type: super::super::audit_trail::AuditEventType::BuildCompleted,
            description: "Build completed".into(),
            details: serde_json::json!({}),
        }]
    }

    #[test]
    fn test_compliance_html_valid() {
        let html = generate_compliance_html(&sample_manifest(), &sample_events(), "saas_landing");
        assert!(html.contains("<!DOCTYPE html>"));
        assert!(html.contains("</html>"));
        assert!(html.contains("AI Writer"));
    }

    #[test]
    fn test_compliance_html_has_quality_scores() {
        let html = generate_compliance_html(&sample_manifest(), &sample_events(), "saas_landing");
        assert!(html.contains("95/100")); // accessibility
        assert!(html.contains("90/100")); // seo
        assert!(html.contains("91/100")); // overall
    }

    #[test]
    fn test_compliance_html_has_model_attribution() {
        let html = generate_compliance_html(&sample_manifest(), &sample_events(), "saas_landing");
        assert!(html.contains("claude-sonnet-4-6"));
        assert!(html.contains("content_generation"));
    }

    #[test]
    fn test_compliance_html_has_signature() {
        let html = generate_compliance_html(&sample_manifest(), &sample_events(), "saas_landing");
        assert!(html.contains("Signed (Ed25519)"));
        assert!(html.contains("abcd1234")); // public key
    }

    #[test]
    fn test_compliance_html_no_deploy() {
        let html = generate_compliance_html(&sample_manifest(), &[], "saas_landing");
        assert!(html.contains("Not yet deployed"));
    }

    #[test]
    fn test_compliance_html_escapes_project_name() {
        let mut m = sample_manifest();
        m.project_name = "Test <script>alert('xss')</script>".into();
        let html = generate_compliance_html(&m, &[], "saas_landing");
        assert!(!html.contains("<script>"));
        assert!(html.contains("&lt;script&gt;"));
    }
}

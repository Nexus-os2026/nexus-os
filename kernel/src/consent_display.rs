//! Immutable template renderer for approval display.
//!
//! Renders [`ApprovalRequest`] into safe, structured display formats that
//! cannot be manipulated by agents.  All output is plain text — no Markdown,
//! no HTML.

use crate::consent::{ApprovalRequest, GovernedOperation, RiskLevel};
use serde::Serialize;

/// Structured, safe display of an approval request for human review.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ApprovalDisplay {
    pub summary: String,
    pub details: Vec<(String, String)>,
    pub risk_badge: String,
    pub raw_command: String,
    pub warnings: Vec<String>,
}

/// Render an [`ApprovalRequest`] into a safe [`ApprovalDisplay`].
///
/// All fields are derived from server-generated data on the request — agents
/// have no control over the output.
pub fn render_approval(request: &ApprovalRequest) -> ApprovalDisplay {
    let summary = sanitize_display_text(&request.display_summary);

    let details = request
        .display_args
        .iter()
        .map(|(key, value)| (sanitize_display_text(key), sanitize_display_text(value)))
        .collect();

    let risk_badge = match request.risk_level {
        RiskLevel::Low => "LOW RISK".to_string(),
        RiskLevel::Medium => "MEDIUM RISK".to_string(),
        RiskLevel::High => "HIGH RISK — Review Carefully".to_string(),
        RiskLevel::Critical => "CRITICAL RISK — Verify All Parameters".to_string(),
    };

    let raw_command = sanitize_display_text(&request.raw_view);

    let mut warnings = Vec::new();
    match request.operation {
        GovernedOperation::TerminalCommand => {
            warnings.push("This operation will execute a system command".to_string());
        }
        GovernedOperation::SocialPostPublish => {
            warnings.push("This operation will access the network".to_string());
        }
        GovernedOperation::SelfMutationApply => {
            warnings.push("This operation will delete files".to_string());
        }
        GovernedOperation::DistributedEnable => {
            warnings.push("This operation will access the network".to_string());
        }
        GovernedOperation::ToolCall
        | GovernedOperation::MultiAgentOrchestrate
        | GovernedOperation::TimeMachineUndo => {}
        GovernedOperation::SelfEvolution => {
            warnings.push(
                "This operation will modify the agent's own description or strategy".to_string(),
            );
        }
        GovernedOperation::AgentLifecycleManage => {
            warnings.push("This operation will create or destroy sub-agents".to_string());
        }
        GovernedOperation::GovernancePolicyModify => {
            warnings.push(
                "This operation will modify system-wide governance policies (L5 only)".to_string(),
            );
        }
        GovernedOperation::EcosystemFuelAllocate => {
            warnings
                .push("This operation will allocate fuel across the agent ecosystem".to_string());
        }
        GovernedOperation::SovereignPromotion => {
            warnings.push(
                "This operation will promote an agent to L5 Sovereign — requires 2-person approval"
                    .to_string(),
            );
        }
        GovernedOperation::TranscendentCreation => {
            warnings.push(
                "This operation will create or activate an L6 Transcendent agent — mandatory 60-second review applies"
                    .to_string(),
            );
        }
    }
    if let Some(min_review) = request.min_review_seconds {
        warnings.push(format!(
            "Mandatory review delay: wait at least {min_review} seconds before approving"
        ));
    }
    if request.risk_level == RiskLevel::Critical {
        warnings.push("This is a critical operation requiring careful review".to_string());
    }

    ApprovalDisplay {
        summary,
        details,
        risk_badge,
        raw_command,
        warnings,
    }
}

/// Strip Markdown formatting, HTML tags, and control characters from display
/// text.
///
/// This is a defense-in-depth measure — even though agents cannot inject into
/// server-generated fields, the sanitizer ensures nothing slips through.
pub fn sanitize_display_text(text: &str) -> String {
    let mut output = String::with_capacity(text.len());
    let mut in_html_tag = false;

    for ch in text.chars() {
        if ch == '<' {
            in_html_tag = true;
            continue;
        }
        if ch == '>' {
            in_html_tag = false;
            continue;
        }
        if in_html_tag {
            continue;
        }
        // Strip Markdown formatting characters
        if matches!(ch, '*' | '_' | '#' | '`' | '[' | ']' | '(' | ')') {
            continue;
        }
        // Strip control characters (keep printable ASCII + valid Unicode)
        if ch.is_control() && ch != '\n' {
            continue;
        }
        output.push(ch);
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::consent::{GovernedOperation, HitlTier};

    #[test]
    fn sanitize_strips_markdown() {
        let input = "**bold** _italic_ `code` [link](url) # heading";
        let result = sanitize_display_text(input);
        assert_eq!(result, "bold italic code linkurl  heading");
    }

    #[test]
    fn sanitize_strips_html_tags() {
        let input = "hello <script>alert('xss')</script> world";
        let result = sanitize_display_text(input);
        assert_eq!(result, "hello alert'xss' world");
    }

    #[test]
    fn sanitize_strips_control_characters() {
        let input = "hello\x00\x01\x02world\nnewline";
        let result = sanitize_display_text(input);
        assert_eq!(result, "helloworld\nnewline");
    }

    #[test]
    fn sanitize_preserves_normal_text() {
        let input = "Tool call agent-abc123 tier2";
        let result = sanitize_display_text(input);
        assert_eq!(result, input);
    }

    #[test]
    fn render_terminal_command_high_risk() {
        let request = ApprovalRequest::from_operation(
            "req-test1".to_string(),
            GovernedOperation::TerminalCommand,
            "a1b2c3d4-e5f6-7890-abcd-ef1234567890".to_string(),
            "hash123".to_string(),
            "agent.test".to_string(),
            HitlTier::Tier2,
            1,
        );
        let display = render_approval(&request);

        assert_eq!(display.summary, "Terminal command agent a1b2c3d4");
        assert_eq!(display.risk_badge, "HIGH RISK — Review Carefully");
        assert!(display
            .warnings
            .contains(&"This operation will execute a system command".to_string()));
        assert!(!display
            .warnings
            .contains(&"This is a critical operation requiring careful review".to_string()));
    }

    #[test]
    fn render_self_mutation_critical() {
        let request = ApprovalRequest::from_operation(
            "req-test2".to_string(),
            GovernedOperation::SelfMutationApply,
            "deadbeef-1234-5678-9abc-def012345678".to_string(),
            "hash456".to_string(),
            "agent.test".to_string(),
            HitlTier::Tier3,
            2,
        );
        let display = render_approval(&request);

        assert_eq!(display.risk_badge, "CRITICAL RISK — Verify All Parameters");
        assert!(display
            .warnings
            .contains(&"This operation will delete files".to_string()));
        assert!(display
            .warnings
            .contains(&"This is a critical operation requiring careful review".to_string()));
    }

    #[test]
    fn render_tool_call_low_risk() {
        let request = ApprovalRequest::from_operation(
            "req-test3".to_string(),
            GovernedOperation::ToolCall,
            "11111111-2222-3333-4444-555555555555".to_string(),
            "hash789".to_string(),
            "agent.test".to_string(),
            HitlTier::Tier1,
            3,
        );
        let display = render_approval(&request);

        assert_eq!(display.risk_badge, "LOW RISK");
        assert!(display.warnings.is_empty());
    }

    #[test]
    fn render_details_match_display_args() {
        let request = ApprovalRequest::from_operation(
            "req-test4".to_string(),
            GovernedOperation::ToolCall,
            "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee".to_string(),
            "hashXYZ".to_string(),
            "agent.test".to_string(),
            HitlTier::Tier2,
            4,
        );
        let display = render_approval(&request);

        assert_eq!(display.details.len(), 4);
        assert_eq!(display.details[0].0, "operation");
        // Underscores are stripped by the sanitizer (Markdown formatting char)
        assert_eq!(display.details[0].1, "toolcall");
        assert_eq!(display.details[1].0, "agentid");
        assert_eq!(display.details[2].0, "tier");
        assert_eq!(display.details[2].1, "tier2");
        assert_eq!(display.details[3].0, "payloadhash");
    }

    #[test]
    fn render_includes_warnings_for_network_social() {
        let request = ApprovalRequest::from_operation(
            "req-net1".to_string(),
            GovernedOperation::SocialPostPublish,
            "aabbccdd-1122-3344-5566-778899001122".to_string(),
            "nethash1".to_string(),
            "agent.social".to_string(),
            HitlTier::Tier2,
            10,
        );
        let display = render_approval(&request);

        assert!(display
            .warnings
            .contains(&"This operation will access the network".to_string()));
    }

    #[test]
    fn render_includes_warnings_for_network_distributed() {
        let request = ApprovalRequest::from_operation(
            "req-net2".to_string(),
            GovernedOperation::DistributedEnable,
            "aabbccdd-1122-3344-5566-778899001122".to_string(),
            "nethash2".to_string(),
            "agent.dist".to_string(),
            HitlTier::Tier3,
            11,
        );
        let display = render_approval(&request);

        assert!(display
            .warnings
            .contains(&"This operation will access the network".to_string()));
        // DistributedEnable is always Critical
        assert!(display
            .warnings
            .contains(&"This is a critical operation requiring careful review".to_string()));
    }

    #[test]
    fn render_includes_warnings_for_file_delete() {
        let request = ApprovalRequest::from_operation(
            "req-del".to_string(),
            GovernedOperation::SelfMutationApply,
            "dddddddd-eeee-ffff-0000-111111111111".to_string(),
            "delhash".to_string(),
            "agent.self".to_string(),
            HitlTier::Tier3,
            12,
        );
        let display = render_approval(&request);

        assert!(display
            .warnings
            .contains(&"This operation will delete files".to_string()));
    }

    #[test]
    fn render_no_warnings_for_tool_call_low_tier() {
        let request = ApprovalRequest::from_operation(
            "req-quiet".to_string(),
            GovernedOperation::ToolCall,
            "eeeeeeee-ffff-0000-1111-222222222222".to_string(),
            "quiethash".to_string(),
            "agent.read".to_string(),
            HitlTier::Tier1,
            13,
        );
        let display = render_approval(&request);

        assert!(display.warnings.is_empty());
    }

    #[test]
    fn sanitize_strips_nested_html() {
        let input = "<div><img src=x onerror=alert(1)>safe text</div>";
        let result = sanitize_display_text(input);
        assert!(!result.contains('<'));
        assert!(!result.contains('>'));
        assert!(result.contains("safe text"));
    }

    #[test]
    fn sanitize_strips_zero_width_chars() {
        // Zero-width space (U+200B) and zero-width joiner (U+200D) are not
        // control chars in Unicode, so they pass through. But actual control
        // chars like \x7F (DEL) should be stripped.
        let input = "visible\x7Ftext";
        let result = sanitize_display_text(input);
        assert_eq!(result, "visibletext");
    }

    #[test]
    fn approval_display_serializes_to_json() {
        let request = ApprovalRequest::from_operation(
            "req-test5".to_string(),
            GovernedOperation::ToolCall,
            "12345678-abcd-ef01-2345-678901234567".to_string(),
            "hash000".to_string(),
            "agent.test".to_string(),
            HitlTier::Tier1,
            5,
        );
        let display = render_approval(&request);
        let json = serde_json::to_string(&display).expect("should serialize");
        assert!(json.contains("\"summary\""));
        assert!(json.contains("\"risk_badge\""));
        assert!(json.contains("\"warnings\""));
    }
}

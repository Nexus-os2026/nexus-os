use crate::generator::SocialPlatform;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ComplianceDecision {
    Allowed,
    Blocked(String),
}

pub fn check_compliance(platform: SocialPlatform, recent_posts: usize) -> ComplianceDecision {
    let (limit, reason) = match platform {
        SocialPlatform::X => (300, "3-hour posting limit reached"),
        SocialPlatform::Instagram => (25, "daily limit"),
        SocialPlatform::Facebook => (50, "daily limit reached"),
    };

    if recent_posts >= limit {
        ComplianceDecision::Blocked(reason.to_string())
    } else {
        ComplianceDecision::Allowed
    }
}

#[cfg(test)]
mod tests {
    use super::{check_compliance, ComplianceDecision};
    use crate::generator::SocialPlatform;

    #[test]
    fn test_tos_rate_limit() {
        let decision = check_compliance(SocialPlatform::Instagram, 25);
        assert_eq!(decision, ComplianceDecision::Blocked("daily limit".to_string()));
    }
}

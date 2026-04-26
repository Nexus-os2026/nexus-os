//! Channel identity for per-channel post-count tracking (Bug W).
//!
//! `ChannelKey { platform, account_id }` is the composite key the
//! `PublishStateHandle` keys all reads and writes by. Single-tenant
//! deployments collapse to `(platform, "default")` until multi-account
//! support arrives — `ChannelKey::default_account` is the helper for
//! that path.

use nexus_content::generator::SocialPlatform;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChannelKey {
    pub platform: SocialPlatform,
    pub account_id: String,
}

impl ChannelKey {
    pub fn new(platform: SocialPlatform, account_id: impl Into<String>) -> Self {
        Self {
            platform,
            account_id: account_id.into(),
        }
    }

    /// Single-tenant placeholder until multi-account lands. Maps to
    /// `account_id = "default"`.
    pub fn default_account(platform: SocialPlatform) -> Self {
        Self::new(platform, "default")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn equal_when_platform_and_account_match() {
        let a = ChannelKey::new(SocialPlatform::X, "acct-1");
        let b = ChannelKey::new(SocialPlatform::X, "acct-1");
        assert_eq!(a, b);
        let mut h: HashMap<ChannelKey, u32> = HashMap::new();
        h.insert(a, 7);
        assert_eq!(h.get(&b), Some(&7));
    }

    #[test]
    fn unequal_when_platform_differs() {
        let a = ChannelKey::new(SocialPlatform::X, "acct-1");
        let b = ChannelKey::new(SocialPlatform::Instagram, "acct-1");
        assert_ne!(a, b);
    }

    #[test]
    fn unequal_when_account_differs() {
        let a = ChannelKey::new(SocialPlatform::X, "acct-1");
        let b = ChannelKey::new(SocialPlatform::X, "acct-2");
        assert_ne!(a, b);
    }

    #[test]
    fn default_account_uses_default_string() {
        let k = ChannelKey::default_account(SocialPlatform::X);
        assert_eq!(k.account_id, "default");
        assert_eq!(k.platform, SocialPlatform::X);
    }
}

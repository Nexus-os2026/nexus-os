use serde::{Deserialize, Serialize};

pub const TOOL_CAPABILITY_PREFIX: &str = "external_tool";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolGovernancePolicy {
    pub min_autonomy_level: u8,
    pub side_effects_require_approval: bool,
    pub url_denylist: Vec<String>,
    pub max_body_size_bytes: u64,
}

impl Default for ToolGovernancePolicy {
    fn default() -> Self {
        Self {
            min_autonomy_level: 2,
            side_effects_require_approval: false,
            url_denylist: vec![
                "localhost".into(),
                "127.0.0.1".into(),
                "0.0.0.0".into(),
                "169.254.".into(),
                "metadata.google".into(),
            ],
            max_body_size_bytes: 1024 * 1024,
        }
    }
}

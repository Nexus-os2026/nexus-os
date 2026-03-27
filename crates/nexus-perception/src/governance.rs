use serde::{Deserialize, Serialize};

pub const PERCEPTION_CAPABILITY: &str = "multimodal_perception";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerceptionPolicy {
    pub min_autonomy_level: u8,
    pub max_image_size_bytes: u64,
    pub max_perception_calls_per_minute: u32,
    pub allowed_sources: Vec<String>,
    pub cost_per_perception: u64,
}

impl Default for PerceptionPolicy {
    fn default() -> Self {
        Self {
            min_autonomy_level: 2,
            max_image_size_bytes: 20 * 1024 * 1024,
            max_perception_calls_per_minute: 30,
            allowed_sources: Vec::new(),
            cost_per_perception: 5_000_000,
        }
    }
}

impl PerceptionPolicy {
    pub fn check_authorization(&self, autonomy_level: u8) -> Result<(), String> {
        if autonomy_level < self.min_autonomy_level {
            return Err(format!(
                "Perception requires L{}+, agent is L{}",
                self.min_autonomy_level, autonomy_level
            ));
        }
        Ok(())
    }

    pub fn check_image_size(&self, size_bytes: u64) -> Result<(), String> {
        if size_bytes > self.max_image_size_bytes {
            return Err(format!(
                "Image too large: {} bytes (max {})",
                size_bytes, self.max_image_size_bytes
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_governance_min_autonomy() {
        let policy = PerceptionPolicy::default();
        assert!(policy.check_authorization(1).is_err());
        assert!(policy.check_authorization(2).is_ok());
        assert!(policy.check_authorization(3).is_ok());
    }

    #[test]
    fn test_governance_image_size() {
        let policy = PerceptionPolicy::default();
        assert!(policy.check_image_size(1024).is_ok());
        assert!(policy.check_image_size(21 * 1024 * 1024).is_err());
    }
}

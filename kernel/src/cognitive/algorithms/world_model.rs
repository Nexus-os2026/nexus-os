use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WorldModel;

impl WorldModel {
    pub fn simulate_action(&self, decision_id: &str, alternative: &str) -> serde_json::Value {
        json!({
            "decision_id": decision_id,
            "alternative": alternative,
            "predicted_outcome": format!("simulated outcome for '{alternative}'"),
            "confidence": 0.5,
        })
    }
}

//! Token economy integration — browser actions burn compute tokens.

use crate::actions::BrowserAction;

/// Calculate the token burn for a browser action.
/// Browser actions use a 1.5x multiplier over raw LLM cost.
pub fn calculate_burn(action: &BrowserAction, base_cost_per_token: f64) -> f64 {
    let tokens = action.estimated_tokens() as f64;
    tokens * base_cost_per_token * 1.5
}

/// Pre-check: does the agent have enough balance?
pub fn check_balance(balance: f64, estimated_burn: f64) -> Result<(), String> {
    if balance < estimated_burn {
        Err(format!(
            "Insufficient balance: need {estimated_burn:.2}, have {balance:.2}"
        ))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_browser_economy_burn() {
        let action = BrowserAction::ExecuteTask {
            task: "test".into(),
            max_steps: Some(10),
        };
        let base_cost = 0.001; // $0.001 per token
        let browser_burn = calculate_burn(&action, base_cost);
        let raw_llm_burn = action.estimated_tokens() as f64 * base_cost;
        assert!(
            browser_burn > raw_llm_burn,
            "Browser burn ({browser_burn}) should be > raw LLM ({raw_llm_burn})"
        );
        // 1.5x multiplier
        assert!((browser_burn - raw_llm_burn * 1.5).abs() < 0.001);
    }
}

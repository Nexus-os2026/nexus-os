use crate::registry::ExternalTool;

pub struct ToolEconomy;

impl ToolEconomy {
    pub fn cost(tool: &ExternalTool) -> u64 {
        tool.cost_per_call
    }

    /// Side-effect tools cost 50% more.
    pub fn adjusted_cost(tool: &ExternalTool) -> u64 {
        if tool.has_side_effects {
            (tool.cost_per_call as f64 * 1.5) as u64
        } else {
            tool.cost_per_call
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::ToolRegistry;

    #[test]
    fn test_tool_cost_side_effects_premium() {
        let reg = ToolRegistry::default_registry();
        let github = reg.get("github").unwrap();
        assert!(github.has_side_effects);
        let adjusted = ToolEconomy::adjusted_cost(github);
        assert_eq!(adjusted, (github.cost_per_call as f64 * 1.5) as u64);
    }

    #[test]
    fn test_tool_cost_read_only() {
        let reg = ToolRegistry::default_registry();
        let ws = reg.get("web_search").unwrap();
        assert!(!ws.has_side_effects);
        assert_eq!(ToolEconomy::adjusted_cost(ws), ws.cost_per_call);
    }
}

use crate::pipeline::PipelineStage;

pub struct FactoryEconomy;

impl FactoryEconomy {
    pub fn estimate_total_cost() -> u64 {
        PipelineStage::all().iter().map(|s| s.base_cost()).sum()
    }

    pub fn stage_cost(stage: &PipelineStage) -> u64 {
        stage.base_cost()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_economy_estimate() {
        let total = FactoryEconomy::estimate_total_cost();
        let manual: u64 = PipelineStage::all().iter().map(|s| s.base_cost()).sum();
        assert_eq!(total, manual);
        // 5 + 10 + 20 + 15 + 5 + 10 + 5 = 70 NXC
        assert_eq!(total, 70_000_000);
    }
}

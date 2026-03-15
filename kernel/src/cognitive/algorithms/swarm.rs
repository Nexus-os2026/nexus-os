use crate::cognitive::types::AgentStep;

#[derive(Debug, Clone, Default)]
pub struct SwarmCoordinator;

impl SwarmCoordinator {
    pub fn prepare_parallel_step(&self, step: &mut AgentStep) {
        if step.max_retries < 3 {
            step.max_retries = 3;
        }
    }
}

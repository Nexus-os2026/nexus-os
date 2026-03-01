use std::time::Instant;

use crate::{capability::FuelPolicy, NexusCoreError};

#[derive(Debug)]
pub struct FuelGauge {
    policy: FuelPolicy,
    llm_calls_used: u64,
    tool_calls_used: u64,
    output_bytes_used: u64,
    started_at: Instant,
    agent_id: String,
}

impl FuelGauge {
    pub fn new(agent_id: String, policy: FuelPolicy) -> Self {
        Self {
            policy,
            llm_calls_used: 0,
            tool_calls_used: 0,
            output_bytes_used: 0,
            started_at: Instant::now(),
            agent_id,
        }
    }

    pub fn charge_llm_call(&mut self) -> Result<(), NexusCoreError> {
        self.llm_calls_used += 1;
        if self.llm_calls_used > self.policy.max_llm_calls {
            return Err(NexusCoreError::FuelExhausted {
                agent_id: self.agent_id.clone(),
                limit_type: "llm_calls".to_string(),
                limit_value: self.policy.max_llm_calls,
            });
        }
        self.check_wall_clock()
    }

    pub fn charge_tool_call(&mut self) -> Result<(), NexusCoreError> {
        self.tool_calls_used += 1;
        if self.tool_calls_used > self.policy.max_tool_calls {
            return Err(NexusCoreError::FuelExhausted {
                agent_id: self.agent_id.clone(),
                limit_type: "tool_calls".to_string(),
                limit_value: self.policy.max_tool_calls,
            });
        }
        self.check_wall_clock()
    }

    pub fn charge_output(&mut self, bytes: u64) -> Result<(), NexusCoreError> {
        self.output_bytes_used += bytes;
        if self.output_bytes_used > self.policy.max_output_bytes {
            return Err(NexusCoreError::FuelExhausted {
                agent_id: self.agent_id.clone(),
                limit_type: "output_bytes".to_string(),
                limit_value: self.policy.max_output_bytes,
            });
        }
        Ok(())
    }

    fn check_wall_clock(&self) -> Result<(), NexusCoreError> {
        let elapsed = self.started_at.elapsed().as_secs();
        if elapsed > self.policy.max_wall_clock_seconds {
            return Err(NexusCoreError::FuelExhausted {
                agent_id: self.agent_id.clone(),
                limit_type: "wall_clock_seconds".to_string(),
                limit_value: self.policy.max_wall_clock_seconds,
            });
        }
        Ok(())
    }

    pub fn snapshot(&self) -> FuelSnapshot {
        FuelSnapshot {
            llm_calls_used: self.llm_calls_used,
            llm_calls_remaining: self.policy.max_llm_calls.saturating_sub(self.llm_calls_used),
            tool_calls_used: self.tool_calls_used,
            tool_calls_remaining: self.policy.max_tool_calls.saturating_sub(self.tool_calls_used),
            wall_clock_seconds: self.started_at.elapsed().as_secs(),
            output_bytes_used: self.output_bytes_used,
        }
    }
}

#[derive(Debug, serde::Serialize)]
pub struct FuelSnapshot {
    pub llm_calls_used: u64,
    pub llm_calls_remaining: u64,
    pub tool_calls_used: u64,
    pub tool_calls_remaining: u64,
    pub wall_clock_seconds: u64,
    pub output_bytes_used: u64,
}

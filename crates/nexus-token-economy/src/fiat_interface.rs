use crate::coin::NexusCoin;

/// Fiat conversion interface — defines how Nexus coins could map to external value.
/// This trait is STUB ONLY — implementation requires regulatory compliance
/// and is not needed for launch.
pub trait FiatConversion: Send + Sync {
    /// Get the current exchange rate (NXC per USD)
    fn exchange_rate(&self) -> Result<f64, String>;

    /// Estimate fiat value of a coin amount
    fn estimate_fiat_value(&self, amount: NexusCoin) -> Result<f64, String>;

    /// Request a withdrawal (would require KYC, compliance, etc.)
    fn request_withdrawal(&self, agent_id: &str, amount: NexusCoin) -> Result<String, String>;
}

/// Stub implementation — always returns "not available"
pub struct FiatConversionStub;

impl FiatConversion for FiatConversionStub {
    fn exchange_rate(&self) -> Result<f64, String> {
        Err("Fiat conversion not available — regulatory compliance pending".into())
    }

    fn estimate_fiat_value(&self, _amount: NexusCoin) -> Result<f64, String> {
        Err("Fiat conversion not available".into())
    }

    fn request_withdrawal(&self, _agent_id: &str, _amount: NexusCoin) -> Result<String, String> {
        Err("Withdrawals not available — regulatory compliance pending".into())
    }
}

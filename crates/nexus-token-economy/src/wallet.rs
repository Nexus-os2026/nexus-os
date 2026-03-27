use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::coin::NexusCoin;
use crate::EconomyError;

/// Agent wallet — cryptographically sealed, governance-controlled.
/// Follows the same pattern as CapabilityBudget in the Governance Oracle.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentWallet {
    /// Agent this wallet belongs to
    pub agent_id: String,
    /// Current balance
    pub balance: NexusCoin,
    /// Total coins ever earned (minted + rewards)
    pub lifetime_earned: NexusCoin,
    /// Total coins ever burned (compute + spawn)
    pub lifetime_burned: NexusCoin,
    /// Total coins transferred to others (delegation payments)
    pub lifetime_transferred: NexusCoin,
    /// Total coins received from others (delegation earnings)
    pub lifetime_received: NexusCoin,
    /// Amount currently locked in escrow
    pub escrowed: NexusCoin,
    /// Wallet version (monotonically increasing with each transaction)
    pub version: u64,
    /// Hash of current state (for integrity verification)
    pub state_hash: String,
    /// Governance authority signature over the state hash
    pub authority_signature: Vec<u8>,
    /// Agent autonomy level (determines gating behavior)
    pub autonomy_level: u8,
}

impl AgentWallet {
    /// Create a new wallet with an initial allocation
    pub fn new(
        agent_id: String,
        initial_balance: NexusCoin,
        autonomy_level: u8,
        signing_key: &ed25519_dalek::SigningKey,
    ) -> Self {
        let mut wallet = Self {
            agent_id,
            balance: initial_balance,
            lifetime_earned: initial_balance,
            lifetime_burned: NexusCoin::ZERO,
            lifetime_transferred: NexusCoin::ZERO,
            lifetime_received: NexusCoin::ZERO,
            escrowed: NexusCoin::ZERO,
            version: 0,
            state_hash: String::new(),
            authority_signature: Vec::new(),
            autonomy_level,
        };
        wallet.rehash_and_sign(signing_key);
        wallet
    }

    /// Credit coins to this wallet (mint or reward)
    pub fn credit(
        &mut self,
        amount: NexusCoin,
        signing_key: &ed25519_dalek::SigningKey,
    ) -> Result<(), EconomyError> {
        self.balance = self
            .balance
            .checked_add(amount)
            .ok_or(EconomyError::Overflow)?;
        self.lifetime_earned = self
            .lifetime_earned
            .checked_add(amount)
            .ok_or(EconomyError::Overflow)?;
        self.version += 1;
        self.rehash_and_sign(signing_key);
        Ok(())
    }

    /// Burn coins from this wallet (compute cost or spawn cost)
    /// Coins are destroyed — they leave the total supply permanently
    pub fn burn(
        &mut self,
        amount: NexusCoin,
        signing_key: &ed25519_dalek::SigningKey,
    ) -> Result<(), EconomyError> {
        let available = self.available_balance();
        if available < amount {
            return Err(EconomyError::InsufficientBalance {
                requested: amount,
                available,
            });
        }
        self.balance =
            self.balance
                .checked_sub(amount)
                .ok_or(EconomyError::InsufficientBalance {
                    requested: amount,
                    available,
                })?;
        self.lifetime_burned = self
            .lifetime_burned
            .checked_add(amount)
            .ok_or(EconomyError::Overflow)?;
        self.version += 1;
        self.rehash_and_sign(signing_key);
        Ok(())
    }

    /// Lock coins in escrow for delegation
    pub fn lock_escrow(
        &mut self,
        amount: NexusCoin,
        signing_key: &ed25519_dalek::SigningKey,
    ) -> Result<(), EconomyError> {
        let available = self.available_balance();
        if available < amount {
            return Err(EconomyError::InsufficientBalance {
                requested: amount,
                available,
            });
        }
        self.escrowed = self
            .escrowed
            .checked_add(amount)
            .ok_or(EconomyError::Overflow)?;
        self.version += 1;
        self.rehash_and_sign(signing_key);
        Ok(())
    }

    /// Release escrowed coins (transfer to provider)
    pub fn release_escrow(
        &mut self,
        amount: NexusCoin,
        signing_key: &ed25519_dalek::SigningKey,
    ) -> Result<(), EconomyError> {
        if self.escrowed < amount {
            return Err(EconomyError::EscrowError(
                "Insufficient escrowed funds".into(),
            ));
        }
        self.escrowed = self
            .escrowed
            .checked_sub(amount)
            .ok_or(EconomyError::EscrowError("Escrow underflow".into()))?;
        self.balance =
            self.balance
                .checked_sub(amount)
                .ok_or(EconomyError::InsufficientBalance {
                    requested: amount,
                    available: self.balance,
                })?;
        self.lifetime_transferred = self
            .lifetime_transferred
            .checked_add(amount)
            .ok_or(EconomyError::Overflow)?;
        self.version += 1;
        self.rehash_and_sign(signing_key);
        Ok(())
    }

    /// Refund escrowed coins (delegation cancelled or failed)
    pub fn refund_escrow(
        &mut self,
        amount: NexusCoin,
        signing_key: &ed25519_dalek::SigningKey,
    ) -> Result<(), EconomyError> {
        if self.escrowed < amount {
            return Err(EconomyError::EscrowError(
                "Insufficient escrowed funds".into(),
            ));
        }
        self.escrowed = self
            .escrowed
            .checked_sub(amount)
            .ok_or(EconomyError::EscrowError("Escrow underflow".into()))?;
        self.version += 1;
        self.rehash_and_sign(signing_key);
        Ok(())
    }

    /// Receive coins from another wallet (delegation payment)
    pub fn receive(
        &mut self,
        amount: NexusCoin,
        signing_key: &ed25519_dalek::SigningKey,
    ) -> Result<(), EconomyError> {
        self.balance = self
            .balance
            .checked_add(amount)
            .ok_or(EconomyError::Overflow)?;
        self.lifetime_received = self
            .lifetime_received
            .checked_add(amount)
            .ok_or(EconomyError::Overflow)?;
        self.version += 1;
        self.rehash_and_sign(signing_key);
        Ok(())
    }

    /// Available balance (total minus escrowed)
    pub fn available_balance(&self) -> NexusCoin {
        self.balance
            .checked_sub(self.escrowed)
            .unwrap_or(NexusCoin::ZERO)
    }

    /// Verify wallet integrity
    pub fn verify(&self, verifying_key: &ed25519_dalek::VerifyingKey) -> Result<(), EconomyError> {
        let expected_hash = self.compute_hash();
        if expected_hash != self.state_hash {
            return Err(EconomyError::IntegrityViolation("Hash mismatch".into()));
        }
        use ed25519_dalek::Verifier;
        let signature = ed25519_dalek::Signature::from_bytes(
            self.authority_signature
                .as_slice()
                .try_into()
                .map_err(|_| EconomyError::IntegrityViolation("Invalid signature".into()))?,
        );
        verifying_key
            .verify(self.state_hash.as_bytes(), &signature)
            .map_err(|_| {
                EconomyError::IntegrityViolation("Signature verification failed".into())
            })?;
        Ok(())
    }

    /// Burn rate: lifetime_burned / lifetime_earned (0.0-1.0+)
    pub fn burn_rate(&self) -> f64 {
        if self.lifetime_earned.micro() == 0 {
            return 0.0;
        }
        self.lifetime_burned.as_f64() / self.lifetime_earned.as_f64()
    }

    fn rehash_and_sign(&mut self, signing_key: &ed25519_dalek::SigningKey) {
        self.state_hash = self.compute_hash();
        use ed25519_dalek::Signer;
        let signature = signing_key.sign(self.state_hash.as_bytes());
        self.authority_signature = signature.to_bytes().to_vec();
    }

    fn compute_hash(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(self.agent_id.as_bytes());
        hasher.update(self.balance.micro().to_le_bytes());
        hasher.update(self.escrowed.micro().to_le_bytes());
        hasher.update(self.version.to_le_bytes());
        hasher.update(self.lifetime_earned.micro().to_le_bytes());
        hasher.update(self.lifetime_burned.micro().to_le_bytes());
        format!("{:x}", hasher.finalize())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_signing_key() -> ed25519_dalek::SigningKey {
        ed25519_dalek::SigningKey::from_bytes(&[42u8; 32])
    }

    #[test]
    fn test_wallet_credit_and_burn() {
        let sk = test_signing_key();
        let mut wallet = AgentWallet::new("agent-1".into(), NexusCoin::from_coins(100), 3, &sk);

        assert_eq!(wallet.balance, NexusCoin::from_coins(100));

        wallet.credit(NexusCoin::from_coins(50), &sk).unwrap();
        assert_eq!(wallet.balance, NexusCoin::from_coins(150));
        assert_eq!(wallet.lifetime_earned, NexusCoin::from_coins(150));

        wallet.burn(NexusCoin::from_coins(30), &sk).unwrap();
        assert_eq!(wallet.balance, NexusCoin::from_coins(120));
        assert_eq!(wallet.lifetime_burned, NexusCoin::from_coins(30));
    }

    #[test]
    fn test_wallet_insufficient_burn() {
        let sk = test_signing_key();
        let mut wallet = AgentWallet::new("agent-1".into(), NexusCoin::from_coins(10), 3, &sk);
        let result = wallet.burn(NexusCoin::from_coins(20), &sk);
        assert!(result.is_err());
    }

    #[test]
    fn test_wallet_escrow_lock_release() {
        let sk = test_signing_key();
        let mut wallet = AgentWallet::new("agent-1".into(), NexusCoin::from_coins(100), 3, &sk);

        wallet.lock_escrow(NexusCoin::from_coins(30), &sk).unwrap();
        assert_eq!(wallet.available_balance(), NexusCoin::from_coins(70));
        assert_eq!(wallet.escrowed, NexusCoin::from_coins(30));

        wallet
            .release_escrow(NexusCoin::from_coins(30), &sk)
            .unwrap();
        assert_eq!(wallet.balance, NexusCoin::from_coins(70));
        assert_eq!(wallet.escrowed, NexusCoin::ZERO);
        assert_eq!(wallet.lifetime_transferred, NexusCoin::from_coins(30));
    }

    #[test]
    fn test_wallet_integrity_verification() {
        let sk = test_signing_key();
        let vk = ed25519_dalek::VerifyingKey::from(&sk);

        let wallet = AgentWallet::new("agent-1".into(), NexusCoin::from_coins(100), 3, &sk);
        assert!(wallet.verify(&vk).is_ok());

        // Tamper with the balance
        let mut tampered = wallet;
        tampered.balance = NexusCoin::from_coins(999);
        assert!(tampered.verify(&vk).is_err());
    }

    #[test]
    fn test_wallet_burn_rate() {
        let sk = test_signing_key();
        let mut wallet = AgentWallet::new("agent-1".into(), NexusCoin::from_coins(100), 3, &sk);
        wallet.burn(NexusCoin::from_coins(50), &sk).unwrap();
        let rate = wallet.burn_rate();
        assert!((rate - 0.5).abs() < 0.001);
    }
}

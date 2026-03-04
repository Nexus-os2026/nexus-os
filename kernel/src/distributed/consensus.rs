use super::DistError;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ProposalId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Proposal {
    pub kind: String,
    pub payload_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Vote {
    Approve,
    Reject,
    Abstain,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum QuorumStatus {
    Pending,
    Reached,
    Rejected,
}

pub trait ConsensusProtocol {
    fn propose(&mut self, proposal: Proposal) -> Result<ProposalId, DistError>;
    fn vote(&mut self, proposal_id: &ProposalId, vote: Vote) -> Result<(), DistError>;
    fn check_quorum(&self, proposal_id: &ProposalId) -> Result<QuorumStatus, DistError>;
}

#[derive(Debug, Clone, Default)]
pub struct SingleNodeConsensus;

impl SingleNodeConsensus {
    pub fn new() -> Self {
        Self
    }
}

impl ConsensusProtocol for SingleNodeConsensus {
    fn propose(&mut self, proposal: Proposal) -> Result<ProposalId, DistError> {
        let encoded = serde_json::to_vec(&proposal)
            .map_err(|error| DistError::InvalidProposal(error.to_string()))?;
        let mut hasher = Sha256::new();
        hasher.update(encoded);
        let digest = format!("{:x}", hasher.finalize());
        Ok(ProposalId(format!("prop-{}", &digest[..16])))
    }

    fn vote(&mut self, _proposal_id: &ProposalId, _vote: Vote) -> Result<(), DistError> {
        Ok(())
    }

    fn check_quorum(&self, _proposal_id: &ProposalId) -> Result<QuorumStatus, DistError> {
        Ok(QuorumStatus::Reached)
    }
}

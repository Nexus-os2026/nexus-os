pub mod consensus;
pub mod economy;
pub mod governance;
pub mod message;
pub mod patterns;
pub mod protocol;
pub mod roles;
pub mod session;
pub mod tauri_commands;

pub use consensus::{ConsensusDetector, ConsensusState};
pub use governance::{CollaborationPolicy, COLLABORATION_CAPABILITY};
pub use message::{CollaborationMessage, MessageContent, MessageType};
pub use patterns::CollaborationPattern;
pub use protocol::CollaborationProtocol;
pub use roles::{CollaborationRole, Participant};
pub use session::{
    ActiveVote, CollabError, CollaborationOutcome, CollaborationSession, ConsensusMethod, Dissent,
    SessionStatus, VoteChoice, VoteRecord,
};
pub use tauri_commands::CollabState;

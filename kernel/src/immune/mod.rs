//! Biological immune system for Nexus OS.
//!
//! Provides layered defense inspired by biological immune systems:
//!
//! | Layer | Module | Role |
//! |-------|--------|------|
//! | Detection | [`detector`] | Scan agent I/O for threats |
//! | Response | [`antibody`] | Spawn specialized defense agents |
//! | Memory | [`memory`] | Store threat signatures (virus definitions) |
//! | Collective | [`hive`] | Propagate defenses across all agents |
//! | Privacy | [`privacy`] | Deep PII/secret scanning with Luhn checks |
//! | Training | [`arena`] | Red-team attacker vs defender sessions |
//! | Dashboard | [`status`] | Overall immune health at a glance |

pub mod antibody;
pub mod arena;
pub mod detector;
pub mod hive;
pub mod memory;
pub mod privacy;
pub mod status;

pub use antibody::{Antibody, AntibodySpawner};
pub use arena::{AdversarialArena, ArenaSession, RoundResult};
pub use detector::{ThreatDetector, ThreatEvent, ThreatSeverity, ThreatType};
pub use hive::{HiveImmunity, ImmunityUpdate};
pub use memory::{ImmuneMemory, ThreatSignature};
pub use privacy::{PrivacyCategory, PrivacyRule, PrivacyScanner, PrivacyViolation, ScanResult};
pub use status::{ImmuneStatus, ThreatLevel};

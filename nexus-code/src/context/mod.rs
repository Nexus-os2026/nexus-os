//! Context engine — NEXUSCODE.md parsing and context window measurement.

pub mod compaction;
pub mod measurement;
pub mod nexuscode;

pub use measurement::ContextMeasurement;
pub use nexuscode::NexusCodeMd;

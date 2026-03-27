//! Request queue management.
//!
//! Thin wrapper over the tokio mpsc channel that connects the oracle to the
//! decision engine.

/// Create a bounded request channel for oracle → engine communication.
pub fn request_channel(
    capacity: usize,
) -> (
    tokio::sync::mpsc::Sender<crate::oracle::OracleRequest>,
    tokio::sync::mpsc::Receiver<crate::oracle::OracleRequest>,
) {
    tokio::sync::mpsc::channel(capacity)
}

//! Lightweight cooperative cancel token.
//!
//! Built on `tokio::sync::watch::channel(bool)` to avoid pulling in
//! `tokio-util` for a trivial flag. `Clone` shares the underlying
//! sender/receiver so any holder can flip cancellation and any other
//! holder observes it.

use tokio::sync::watch;

#[derive(Clone)]
pub struct CancelToken {
    tx: watch::Sender<bool>,
    rx: watch::Receiver<bool>,
}

impl CancelToken {
    pub fn new() -> Self {
        let (tx, rx) = watch::channel(false);
        Self { tx, rx }
    }

    pub fn is_cancelled(&self) -> bool {
        *self.rx.borrow()
    }

    pub fn cancel(&self) {
        let _ = self.tx.send(true);
    }
}

impl Default for CancelToken {
    fn default() -> Self {
        Self::new()
    }
}

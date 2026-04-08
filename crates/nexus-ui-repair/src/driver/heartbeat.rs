//! Phase 1.4 Deliverable 7 — driver heartbeat.
//!
//! A human reading `~/.nexus/ui-repair/heartbeat.json` during a live
//! autonomous scout run needs to know, at a glance, (1) that the driver
//! is alive, (2) which page it is working, and (3) which state inside
//! the per-element loop it is in. If the loop hangs in `Acting`, the
//! heartbeat must still reflect the last recorded `(page, state)` pair
//! so the operator can reach in and kill the right subprocess.
//!
//! # Lifecycle
//!
//! [`Heartbeat::spawn`] starts a background [`tokio::task`] that ticks
//! on an interval and rewrites the heartbeat file atomically. The state
//! the task observes lives behind an `Arc<Mutex<HeartbeatState>>` so
//! the driver loop can update `current_page` / `current_state` on every
//! state transition without blocking.
//!
//! # Shutdown
//!
//! [`Heartbeat::shutdown`] cancels the background task via a
//! [`tokio::sync::oneshot`] channel and awaits the task to finish. The
//! oneshot means cancellation is observed **immediately**, not up to
//! `interval_ms` later (a polled `AtomicBool` would leak a full tick).
//!
//! `Drop` intentionally does **not** block on async cleanup. If the
//! heartbeat is dropped without an explicit `shutdown().await`, the
//! oneshot sender fires in `Drop` but the task is detached — it will
//! observe the cancel on its next `select!` tick and exit on its own.
//! This is a conservative choice: `Drop` cannot run async code, and we
//! would rather leak a single idle tokio task than block the drop site.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use tokio::sync::oneshot;
use tokio::task::JoinHandle;

/// Mutable state the driver updates at every page / state transition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeartbeatState {
    /// The page the driver is currently exercising (descriptor route
    /// or fixture name). Updated at the top of every page.
    pub current_page: String,
    /// The driver state within the per-element loop, e.g.
    /// `"Enumerate"`, `"Plan"`, `"Act"`, `"Observe"`, `"Classify"`,
    /// `"Report"`. Updated at every state transition so a hang in
    /// `Act` still shows `"Act"` in the file.
    pub current_state: String,
    /// Monotonic tick count written by the background task. Exposed
    /// so an operator can tell a fresh hang ("tick stuck at 12") from
    /// a slow loop ("tick still advancing, just slowly").
    pub tick: u64,
    /// RFC3339 timestamp of the most recent write.
    pub updated_at: String,
}

impl HeartbeatState {
    /// Initial state before the driver has entered the loop.
    pub fn initial() -> Self {
        Self {
            current_page: "<pre-loop>".to_string(),
            current_state: "<pre-loop>".to_string(),
            tick: 0,
            updated_at: chrono::Utc::now().to_rfc3339(),
        }
    }
}

/// Handle to a running heartbeat background task.
pub struct Heartbeat {
    state: Arc<Mutex<HeartbeatState>>,
    path: PathBuf,
    cancel_tx: Option<oneshot::Sender<()>>,
    task: Option<JoinHandle<()>>,
}

impl Heartbeat {
    /// Spawn the background heartbeat task.
    ///
    /// The task writes to `path` every `interval_ms` milliseconds until
    /// [`Heartbeat::shutdown`] cancels it. An initial write happens
    /// before the first tick so a very-short-lived run still leaves a
    /// file behind.
    ///
    /// Must be called from inside a tokio runtime.
    pub fn spawn(path: PathBuf, interval_ms: u64) -> std::io::Result<Self> {
        let state = Arc::new(Mutex::new(HeartbeatState::initial()));
        let (cancel_tx, mut cancel_rx) = oneshot::channel::<()>();

        // Make sure the parent directory exists so the first write
        // succeeds. We do NOT create the file itself here — the
        // background task does via the atomic rename.
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() && !parent.exists() {
                std::fs::create_dir_all(parent)?;
            }
        }

        // Initial write so a very short run still produces a file.
        write_state_atomic(&path, &state.lock().expect("heartbeat mutex"))?;

        let task_state = state.clone();
        let task_path = path.clone();
        let task = tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_millis(interval_ms));
            // Skip the first immediate tick — we already wrote above.
            interval.tick().await;
            loop {
                tokio::select! {
                    biased;
                    _ = &mut cancel_rx => break,
                    _ = interval.tick() => {
                        // Advance tick counter and stamp.
                        {
                            let mut guard = match task_state.lock() {
                                Ok(g) => g,
                                Err(_) => break,
                            };
                            guard.tick = guard.tick.wrapping_add(1);
                            guard.updated_at = chrono::Utc::now().to_rfc3339();
                        }
                        let snapshot = match task_state.lock() {
                            Ok(g) => g.clone(),
                            Err(_) => break,
                        };
                        if let Err(e) = write_state_atomic(&task_path, &snapshot) {
                            tracing::warn!(error = %e, "heartbeat write failed");
                        }
                    }
                }
            }
        });

        Ok(Self {
            state,
            path,
            cancel_tx: Some(cancel_tx),
            task: Some(task),
        })
    }

    /// Update the current `(page, state)` pair. Called by the driver
    /// loop at every page boundary and every state transition.
    pub fn set_position(&self, page: impl Into<String>, state: impl Into<String>) {
        if let Ok(mut guard) = self.state.lock() {
            guard.current_page = page.into();
            guard.current_state = state.into();
            guard.updated_at = chrono::Utc::now().to_rfc3339();
        }
    }

    /// Snapshot of the current heartbeat state (test introspection).
    pub fn snapshot(&self) -> HeartbeatState {
        self.state
            .lock()
            .map(|g| g.clone())
            .unwrap_or_else(|_| HeartbeatState::initial())
    }

    /// Path of the heartbeat file.
    pub fn path(&self) -> &std::path::Path {
        &self.path
    }

    /// Cancel the background task and await its completion.
    ///
    /// Safe to call exactly once. After `shutdown` returns, the tokio
    /// task is joined — no handle leaks. Subsequent calls are no-ops.
    pub async fn shutdown(mut self) {
        if let Some(tx) = self.cancel_tx.take() {
            let _ = tx.send(());
        }
        if let Some(task) = self.task.take() {
            let _ = task.await;
        }
    }
}

impl Drop for Heartbeat {
    fn drop(&mut self) {
        // Non-blocking cancel: fire the oneshot if nobody called
        // shutdown(). The task will observe the cancel on its next
        // select! iteration and exit on its own. We do NOT await the
        // task here — Drop cannot run async code.
        if let Some(tx) = self.cancel_tx.take() {
            let _ = tx.send(());
        }
        // We intentionally leave the JoinHandle detached if shutdown
        // was not called. tokio treats a dropped JoinHandle as detach.
    }
}

/// Write `state` to `path` atomically (write-to-temp + rename).
fn write_state_atomic(path: &std::path::Path, state: &HeartbeatState) -> std::io::Result<()> {
    let bytes = serde_json::to_vec_pretty(state)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, &bytes)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

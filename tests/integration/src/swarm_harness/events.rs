//! Broadcast event capture utility for swarm-harness scenario tests.
//!
//! Subscribes-before-run is the caller's responsibility — get the
//! receiver via `scenario.events_tx.subscribe()` BEFORE invoking
//! `scenario.run()`, then hand it to [`drain_events_until_terminal`]
//! after run starts.

use nexus_swarm::events::SwarmEvent;
use std::time::Duration;
use tokio::sync::broadcast;

/// Errors the capture utility can surface to a scenario test.
#[derive(Debug, PartialEq, Eq)]
pub enum EventCaptureError {
    /// No terminal event arrived in the configured window.
    Timeout,
    /// The broadcast channel overflowed before the receiver caught up.
    Lagged,
}

/// Drain `rx` into a `Vec<SwarmEvent>` until either a terminal event
/// (`SwarmCompleted` or `SwarmCancelled`) is observed or `timeout`
/// elapses. The terminal event itself is included in the returned vec.
///
/// Returns `Err(Timeout)` if no terminal event arrives in the window;
/// `Err(Lagged)` on broadcast lag (the test should bump the channel
/// depth or drain faster).
pub async fn drain_events_until_terminal(
    mut rx: broadcast::Receiver<SwarmEvent>,
    timeout: Duration,
) -> Result<Vec<SwarmEvent>, EventCaptureError> {
    let outcome = tokio::time::timeout(timeout, async {
        let mut captured = Vec::new();
        loop {
            match rx.recv().await {
                Ok(event) => {
                    let is_terminal = matches!(
                        event,
                        SwarmEvent::SwarmCompleted { .. } | SwarmEvent::SwarmCancelled { .. }
                    );
                    captured.push(event);
                    if is_terminal {
                        return Ok::<_, EventCaptureError>(captured);
                    }
                }
                Err(broadcast::error::RecvError::Closed) => {
                    return Ok(captured);
                }
                Err(broadcast::error::RecvError::Lagged(_)) => {
                    return Err(EventCaptureError::Lagged);
                }
            }
        }
    })
    .await;

    match outcome {
        Ok(Ok(captured)) => Ok(captured),
        Ok(Err(e)) => Err(e),
        Err(_elapsed) => Err(EventCaptureError::Timeout),
    }
}

/// Drain `rx` for at most `duration` and return whatever events
/// arrived. Unlike [`drain_events_until_terminal`], this never errors
/// — a timeout with zero events is the success signal for scenarios
/// that assert "no events should arrive" (e.g. plan denial paths
/// where the coordinator is never reached).
pub async fn drain_for_duration(
    mut rx: broadcast::Receiver<SwarmEvent>,
    duration: Duration,
) -> Vec<SwarmEvent> {
    let mut captured = Vec::new();
    let _ = tokio::time::timeout(duration, async {
        loop {
            match rx.recv().await {
                Ok(event) => captured.push(event),
                Err(broadcast::error::RecvError::Closed) => return,
                Err(broadcast::error::RecvError::Lagged(_)) => return,
            }
        }
    })
    .await;
    captured
}

/// Stable string label per SwarmEvent variant. Useful for sequence
/// assertions without pattern-matching every variant inline.
pub fn event_kind_str(event: &SwarmEvent) -> &'static str {
    match event {
        SwarmEvent::PlanProposed { .. } => "PlanProposed",
        SwarmEvent::PlanApproved { .. } => "PlanApproved",
        SwarmEvent::PlanRejected { .. } => "PlanRejected",
        SwarmEvent::NodeStarted { .. } => "NodeStarted",
        SwarmEvent::NodeEvent { .. } => "NodeEvent",
        SwarmEvent::NodeCompleted { .. } => "NodeCompleted",
        SwarmEvent::NodeFailed { .. } => "NodeFailed",
        SwarmEvent::RouteDenied { .. } => "RouteDenied",
        SwarmEvent::BudgetUpdate { .. } => "BudgetUpdate",
        SwarmEvent::ProviderHealthUpdate { .. } => "ProviderHealthUpdate",
        SwarmEvent::SwarmCompleted { .. } => "SwarmCompleted",
        SwarmEvent::SwarmCancelled { .. } => "SwarmCancelled",
        SwarmEvent::OracleTicketIssued { .. } => "OracleTicketIssued",
        SwarmEvent::OracleRuntimeCheck { .. } => "OracleRuntimeCheck",
        SwarmEvent::OracleRuntimeDenial { .. } => "OracleRuntimeDenial",
    }
}

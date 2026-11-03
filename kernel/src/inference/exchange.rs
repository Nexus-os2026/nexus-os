//! One cancellable wrapper for every request to the local runtime, and the
//! absolute deadline kept on two clocks.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::sync::OnceLock;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::emergency_stop::{HaltArea, HaltOutcome, StopWatch};

use super::endpoint::LoopbackEndpoint;
use super::ollama::ApiPath;
use super::{CallCancel, CallContext, CallDeadline, CallObserver, ModelError};

/// The backstop on any one request, whatever the caller's deadline.
pub const CALL_HARD_TIMEOUT: Duration = Duration::from_secs(180);

/// How often a request in flight polls its cancellation and deadlines.
const POLL: Duration = Duration::from_millis(20);

/// The bound on establishing a connection.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

/// The two clocks a deadline is kept on, in milliseconds: wall time (which
/// advances across a suspend) and monotonic time (which cannot be set
/// back).
pub trait DeadlineClock: Send + Sync {
    fn wall_ms(&self) -> u64;
    fn monotonic_ms(&self) -> u64;
}

/// The system's clocks: `SystemTime` since the Unix epoch, and `Instant`
/// since the first use in this process.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClocks;

impl DeadlineClock for SystemClocks {
    fn wall_ms(&self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, millis)
    }

    fn monotonic_ms(&self) -> u64 {
        static ANCHOR: OnceLock<Instant> = OnceLock::new();
        millis(ANCHOR.get_or_init(Instant::now).elapsed())
    }
}

fn millis(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

/// A deadline fixed once, as an instant on each of two clocks; it has
/// expired when either clock reaches its instant. It is never rebuilt from
/// the time left, so a delay before or between calls never extends it, and
/// a suspend (which stops the monotonic clock but not the wall clock) ends
/// it on time.
#[derive(Debug, Clone)]
pub struct AbsoluteDeadline<C: DeadlineClock = SystemClocks> {
    clock: C,
    wall_at_ms: u64,
    monotonic_at_ms: u64,
}

impl<C: DeadlineClock> AbsoluteDeadline<C> {
    /// The deadline at these instants of `clock`.
    pub fn at(clock: C, wall_at_ms: u64, monotonic_at_ms: u64) -> Self {
        Self {
            clock,
            wall_at_ms,
            monotonic_at_ms,
        }
    }

    /// The deadline `budget` from now, read once on each clock.
    pub fn within(clock: C, budget: Duration) -> Self {
        let budget = millis(budget);
        let wall_at_ms = clock.wall_ms().saturating_add(budget);
        let monotonic_at_ms = clock.monotonic_ms().saturating_add(budget);
        Self::at(clock, wall_at_ms, monotonic_at_ms)
    }

    pub fn wall_at_ms(&self) -> u64 {
        self.wall_at_ms
    }

    pub fn monotonic_at_ms(&self) -> u64 {
        self.monotonic_at_ms
    }
}

impl<C: DeadlineClock> CallDeadline for AbsoluteDeadline<C> {
    fn expired(&self) -> bool {
        self.clock.wall_ms() >= self.wall_at_ms || self.clock.monotonic_ms() >= self.monotonic_at_ms
    }
}

/// What ends a request: the caller's cancellation, deadline and stop watch.
pub(super) struct Guard<'a> {
    pub(super) cancel: &'a dyn CallCancel,
    pub(super) deadline: &'a dyn CallDeadline,
    pub(super) stop: &'a StopWatch,
    /// The caller's dispatch observer (M2e-A1b interface stub: carried,
    /// never notified).
    #[allow(dead_code)]
    pub(super) observer: Option<&'a dyn CallObserver>,
}

impl<'a> Guard<'a> {
    pub(super) fn of(ctx: &CallContext<'a>) -> Self {
        Self {
            cancel: ctx.cancel,
            deadline: ctx.deadline,
            stop: ctx.stop,
            observer: ctx.observer,
        }
    }

    /// Whether the work may go on: not stopped, not cancelled, not expired.
    pub(super) fn check(&self) -> Result<(), ModelError> {
        if self.stop.check().is_err() {
            return Err(ModelError::Stopped);
        }
        if self.cancel.cancelled().is_some() {
            return Err(ModelError::Cancelled);
        }
        if self.deadline.expired() {
            return Err(ModelError::Timeout);
        }
        Ok(())
    }

    /// Record that the owner's stop ended this model call.
    pub(super) fn record_stop(&self) {
        self.stop
            .record_halt(HaltArea::ModelCall, HaltOutcome::Ended);
    }
}

/// A runtime reply: its status and its body (at most the request's cap).
pub(super) struct Reply {
    pub(super) status: u16,
    pub(super) body: Vec<u8>,
}

/// One request to the local runtime, ended by the guard. It runs on its
/// own thread and its own current-thread runtime, as a select over the
/// request, the stop watch, a 20 ms poll of the cancellation and both
/// deadlines (the caller's and the [`CALL_HARD_TIMEOUT`] backstop, each on
/// two clocks). Ending the select drops the request and then the runtime,
/// so the connection closes; the thread is joined before this returns.
pub(super) fn exchange(
    endpoint: &LoopbackEndpoint,
    path: ApiPath,
    body: Option<Vec<u8>>,
    cap: usize,
    guard: &Guard<'_>,
) -> Result<Reply, ModelError> {
    guard.check()?;
    let addr = endpoint.addr();
    // The address is loopback by construction; checked again before every
    // connection all the same.
    if !addr.ip().is_loopback() {
        return Err(ModelError::NotLocal);
    }
    let local = match addr.ip() {
        IpAddr::V4(_) => IpAddr::V4(Ipv4Addr::LOCALHOST),
        IpAddr::V6(_) => IpAddr::V6(Ipv6Addr::LOCALHOST),
    };
    let url = format!("http://{addr}/{}", path.as_str());
    let hard = AbsoluteDeadline::within(SystemClocks, CALL_HARD_TIMEOUT);
    #[cfg(test)]
    let probe = seams::probe();
    let outcome = std::thread::scope(|scope| {
        let thread = std::thread::Builder::new()
            .name("nexus-model-exchange".to_string())
            .spawn_scoped(scope, move || {
                #[cfg(test)]
                let _probe = probe.inspect(|count| {
                    count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                });
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .map_err(|_| Ended::Error(ModelError::Unavailable))?;
                let outcome = runtime.block_on(async {
                    tokio::select! {
                        biased;
                        () = guard.stop.halted_async() => Err(Ended::Stopped),
                        error = poll(guard, &hard) => Err(Ended::Error(error)),
                        reply = request(local, url, body, cap) => reply.map_err(Ended::Error),
                    }
                });
                // The connection ends with the runtime.
                drop(runtime);
                outcome
            });
        match thread {
            Ok(thread) => thread
                .join()
                .unwrap_or(Err(Ended::Error(ModelError::Unavailable))),
            Err(_) => Err(Ended::Error(ModelError::Unavailable)),
        }
    });
    match outcome {
        Ok(reply) => Ok(reply),
        Err(Ended::Stopped) => {
            guard.record_stop();
            Err(ModelError::Stopped)
        }
        Err(Ended::Error(error)) => Err(error),
    }
}

enum Ended {
    Stopped,
    Error(ModelError),
}

/// Resolves when the caller cancels or either deadline expires.
async fn poll(guard: &Guard<'_>, hard: &AbsoluteDeadline) -> ModelError {
    loop {
        if guard.cancel.cancelled().is_some() {
            return ModelError::Cancelled;
        }
        if guard.deadline.expired() || hard.expired() {
            return ModelError::Timeout;
        }
        tokio::time::sleep(POLL).await;
    }
}

/// The request itself: no proxy, no redirect, the local address on
/// loopback, a bounded connect and a bounded body.
async fn request(
    local: IpAddr,
    url: String,
    body: Option<Vec<u8>>,
    cap: usize,
) -> Result<Reply, ModelError> {
    let client = reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .local_address(local)
        .connect_timeout(CONNECT_TIMEOUT)
        .pool_max_idle_per_host(0)
        .build()
        .map_err(|_| ModelError::Unavailable)?;
    let request = match body {
        Some(body) => client
            .post(url)
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .body(body),
        None => client.get(url),
    };
    let mut response = request.send().await.map_err(request_error)?;
    let status = response.status().as_u16();
    if response
        .content_length()
        .is_some_and(|length| length > cap as u64)
    {
        return Err(ModelError::ResponseTooLarge);
    }
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(request_error)? {
        if body.len().saturating_add(chunk.len()) > cap {
            return Err(ModelError::ResponseTooLarge);
        }
        body.extend_from_slice(&chunk);
    }
    Ok(Reply { status, body })
}

fn request_error(error: reqwest::Error) -> ModelError {
    if error.is_timeout() {
        ModelError::Timeout
    } else {
        ModelError::Unavailable
    }
}

/// Test seams: a probe each exchange thread holds while it runs (so a test
/// sees how many ran and that none is left), and a hook run on the calling
/// thread right after the chat reply arrives.
#[cfg(test)]
pub(super) mod seams {
    use std::cell::RefCell;
    use std::sync::atomic::AtomicUsize;
    use std::sync::Arc;

    thread_local! {
        static PROBE: RefCell<Option<Arc<AtomicUsize>>> = const { RefCell::new(None) };
        static AFTER_REPLY: RefCell<Option<Box<dyn Fn()>>> = const { RefCell::new(None) };
    }

    pub(crate) fn set_probe(probe: Arc<AtomicUsize>) {
        PROBE.with(|slot| *slot.borrow_mut() = Some(probe));
    }

    pub(crate) fn clear_probe() {
        PROBE.with(|slot| *slot.borrow_mut() = None);
    }

    pub(super) fn probe() -> Option<Arc<AtomicUsize>> {
        PROBE.with(|slot| slot.borrow().clone())
    }

    pub(crate) fn set_after_reply(hook: Box<dyn Fn()>) {
        AFTER_REPLY.with(|slot| *slot.borrow_mut() = Some(hook));
    }

    pub(crate) fn after_reply() {
        AFTER_REPLY.with(|slot| {
            if let Some(hook) = slot.borrow().as_ref() {
                hook();
            }
        });
    }
}

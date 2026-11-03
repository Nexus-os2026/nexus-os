//! One wrapper for every request to the local runtime, and the absolute
//! deadline kept on two clocks.
//!
//! M2e-A1 interface stub: the inherited behaviour. The coding-run transport
//! runs each request on its own thread under the process's emergency-stop
//! authority (`StopAuthority::global()`), with no caller cancellation and a
//! relative timeout (F4); this stub does the same and ignores the caller's
//! cancellation, deadline and stop watch.

use std::sync::OnceLock;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::emergency_stop::{HaltArea, HaltOutcome, StopAuthority, StopWatch};

use super::endpoint::LoopbackEndpoint;
use super::ollama::ApiPath;
use super::{CallCancel, CallContext, CallDeadline, ModelError};

/// The backstop on any one request, whatever the caller's deadline.
pub const CALL_HARD_TIMEOUT: Duration = Duration::from_secs(180);

/// The inherited relative timeout of a metadata request.
const LIST_TIMEOUT: Duration = Duration::from_secs(5);

/// The two clocks a deadline is kept on, in milliseconds.
pub trait DeadlineClock: Send + Sync {
    fn wall_ms(&self) -> u64;
    fn monotonic_ms(&self) -> u64;
}

/// The system's clocks.
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

/// A deadline fixed once, as an instant on each of two clocks.
#[derive(Debug, Clone)]
pub struct AbsoluteDeadline<C: DeadlineClock = SystemClocks> {
    clock: C,
    wall_at_ms: u64,
    monotonic_at_ms: u64,
}

impl<C: DeadlineClock> AbsoluteDeadline<C> {
    pub fn at(clock: C, wall_at_ms: u64, monotonic_at_ms: u64) -> Self {
        Self {
            clock,
            wall_at_ms,
            monotonic_at_ms,
        }
    }

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

/// The caller's cancellation, deadline and stop watch (not consulted by
/// this stub).
#[allow(dead_code)]
pub(super) struct Guard<'a> {
    pub(super) cancel: &'a dyn CallCancel,
    pub(super) deadline: &'a dyn CallDeadline,
    pub(super) stop: &'a StopWatch,
}

impl<'a> Guard<'a> {
    pub(super) fn of(ctx: &CallContext<'a>) -> Self {
        Self {
            cancel: ctx.cancel,
            deadline: ctx.deadline,
            stop: ctx.stop,
        }
    }
}

/// A runtime reply.
pub(super) struct Reply {
    pub(super) status: u16,
    pub(super) body: Vec<u8>,
}

/// One request, as the inherited transport makes it: on its own thread,
/// admitted under the process's stop authority, with a relative timeout.
pub(super) fn exchange(
    endpoint: &LoopbackEndpoint,
    path: ApiPath,
    body: Option<Vec<u8>>,
    cap: usize,
    _guard: &Guard<'_>,
) -> Result<Reply, ModelError> {
    let watch = StopAuthority::global()
        .admit()
        .map_err(|_| ModelError::Stopped)?;
    let url = format!("http://{}/{}", endpoint.addr(), path.as_str());
    let timeout = if body.is_some() && matches!(path, ApiPath::Chat) {
        CALL_HARD_TIMEOUT
    } else {
        LIST_TIMEOUT
    };
    #[cfg(test)]
    let probe = seams::probe();
    std::thread::Builder::new()
        .name("nexus-coding-model".to_string())
        .spawn(move || {
            #[cfg(test)]
            let _probe = probe.inspect(|count| {
                count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            });
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|_| ModelError::Unavailable)?;
            let stopped = watch.clone();
            let result = runtime.block_on(async move {
                tokio::select! {
                    result = request(url, body, cap, timeout) => result,
                    () = stopped.halted_async() => Err(ModelError::Stopped),
                }
            });
            drop(runtime);
            if matches!(result, Err(ModelError::Stopped)) {
                watch.record_halt(HaltArea::ModelCall, HaltOutcome::Ended);
            }
            result
        })
        .map_err(|_| ModelError::Unavailable)?
        .join()
        .map_err(|_| ModelError::Unavailable)?
}

async fn request(
    url: String,
    body: Option<Vec<u8>>,
    cap: usize,
    timeout: Duration,
) -> Result<Reply, ModelError> {
    let client = reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(timeout)
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
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(request_error)? {
        body.extend_from_slice(&chunk);
        if body.len() > cap {
            return Err(ModelError::ResponseTooLarge);
        }
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

/// Test seams: a probe each exchange thread holds while it runs, and a hook
/// run on the calling thread right after the chat reply arrives.
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

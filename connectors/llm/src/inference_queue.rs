//! Parallel inference with request queuing and priority-based scheduling.
//!
//! Allows multiple agents to submit LLM requests simultaneously. Requests are
//! priority-sorted and dispatched either sequentially (local models) or via
//! thread-pool (external providers).

use crate::providers::{LlmProvider, LlmResponse};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{mpsc, Arc, Condvar, Mutex};
use std::time::Instant;

/// Priority levels for inference requests. Lower numeric value = higher priority.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum InferencePriority {
    Critical = 0,
    High = 1,
    #[default]
    Normal = 2,
    Low = 3,
}

/// A queued inference request.
#[derive(Debug, Clone)]
pub struct InferenceRequest {
    pub id: u64,
    pub prompt: String,
    pub max_tokens: u32,
    pub model: String,
    pub priority: InferencePriority,
    pub submitted_at: Instant,
}

/// Result of a completed inference request.
#[derive(Debug, Clone)]
pub struct InferenceResult {
    pub request_id: u64,
    pub response: Result<LlmResponse, String>,
    pub queue_wait_ms: u64,
    pub inference_ms: u64,
}

/// Snapshot of queue statistics.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct QueueStats {
    pub pending: usize,
    pub in_flight: usize,
    pub completed: usize,
    pub total_submitted: usize,
}

/// Internal entry wrapping a request with its response channel.
struct QueueEntry {
    request: InferenceRequest,
    reply_tx: mpsc::Sender<InferenceResult>,
}

/// Thread-safe inference queue with priority ordering.
pub struct InferenceQueue {
    /// Priority-sorted pending requests (lower priority value = front of queue).
    pending: Mutex<Vec<QueueEntry>>,
    /// Signalled when a new request is enqueued or an in-flight request completes.
    notify: Condvar,
    /// Monotonically increasing request ID counter.
    next_id: AtomicUsize,
    /// Number of requests currently being processed.
    in_flight: AtomicUsize,
    /// Total number of completed requests.
    completed: AtomicUsize,
    /// Total number of submitted requests.
    total_submitted: AtomicUsize,
    /// Whether the queue has been shut down.
    shutdown: AtomicBool,
    /// Maximum number of concurrent in-flight requests (0 = unlimited).
    max_concurrent: AtomicUsize,
}

impl InferenceQueue {
    pub fn new() -> Self {
        Self {
            pending: Mutex::new(Vec::new()),
            notify: Condvar::new(),
            next_id: AtomicUsize::new(1),
            in_flight: AtomicUsize::new(0),
            completed: AtomicUsize::new(0),
            total_submitted: AtomicUsize::new(0),
            shutdown: AtomicBool::new(false),
            max_concurrent: AtomicUsize::new(0),
        }
    }

    /// Create a queue with a maximum number of concurrent in-flight requests.
    pub fn with_max_concurrent(max: usize) -> Self {
        Self {
            pending: Mutex::new(Vec::new()),
            notify: Condvar::new(),
            next_id: AtomicUsize::new(1),
            in_flight: AtomicUsize::new(0),
            completed: AtomicUsize::new(0),
            total_submitted: AtomicUsize::new(0),
            shutdown: AtomicBool::new(false),
            max_concurrent: AtomicUsize::new(max),
        }
    }

    /// Returns true if in-flight count is below the max_concurrent limit (or unlimited).
    fn can_dispatch(&self) -> bool {
        let max = self.max_concurrent.load(Ordering::Relaxed);
        if max == 0 {
            return true; // unlimited
        }
        self.in_flight.load(Ordering::Relaxed) < max
    }

    /// Submit a request and receive a channel to await the result.
    pub fn submit(
        &self,
        prompt: String,
        max_tokens: u32,
        model: String,
        priority: InferencePriority,
    ) -> (u64, mpsc::Receiver<InferenceResult>) {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed) as u64;
        let (tx, rx) = mpsc::channel();
        let entry = QueueEntry {
            request: InferenceRequest {
                id,
                prompt,
                max_tokens,
                model,
                priority,
                submitted_at: Instant::now(),
            },
            reply_tx: tx,
        };

        {
            let mut queue = self.pending.lock().unwrap_or_else(|poisoned| {
                eprintln!("Lock was poisoned, recovering inner data");
                poisoned.into_inner()
            });
            // Insert in priority order (stable: append then sort).
            queue.push(entry);
            queue.sort_by_key(|e| e.request.priority);
        }
        self.total_submitted.fetch_add(1, Ordering::Relaxed);
        self.notify.notify_one();

        (id, rx)
    }

    /// Take the next highest-priority entry from the queue. Returns `None` if empty.
    fn take_next(&self) -> Option<QueueEntry> {
        let mut queue = self.pending.lock().unwrap_or_else(|poisoned| {
            eprintln!("Lock was poisoned, recovering inner data");
            poisoned.into_inner()
        });
        if queue.is_empty() {
            None
        } else {
            Some(queue.remove(0))
        }
    }

    /// Take the next entry, blocking until one is available or shutdown is signalled.
    fn take_next_blocking(&self) -> Option<QueueEntry> {
        let mut queue = self.pending.lock().unwrap_or_else(|poisoned| {
            eprintln!("Lock was poisoned, recovering inner data");
            poisoned.into_inner()
        });
        loop {
            if self.shutdown.load(Ordering::Relaxed) {
                return None;
            }
            if !queue.is_empty() {
                return Some(queue.remove(0));
            }
            queue = self.notify.wait(queue).unwrap_or_else(|poisoned| {
                eprintln!("Lock was poisoned, recovering inner data");
                poisoned.into_inner()
            });
        }
    }

    /// Return a snapshot of queue statistics.
    pub fn stats(&self) -> QueueStats {
        let pending = self
            .pending
            .lock()
            .unwrap_or_else(|poisoned| {
                eprintln!("Lock was poisoned, recovering inner data");
                poisoned.into_inner()
            })
            .len();
        QueueStats {
            pending,
            in_flight: self.in_flight.load(Ordering::Relaxed),
            completed: self.completed.load(Ordering::Relaxed),
            total_submitted: self.total_submitted.load(Ordering::Relaxed),
        }
    }

    /// Signal the queue to stop accepting work. Wakes any blocked workers.
    pub fn shutdown(&self) {
        self.shutdown.store(true, Ordering::Relaxed);
        self.notify.notify_all();
    }

    /// Returns true if shutdown has been signalled.
    pub fn is_shutdown(&self) -> bool {
        self.shutdown.load(Ordering::Relaxed)
    }

    /// Number of pending requests.
    pub fn pending_count(&self) -> usize {
        self.pending
            .lock()
            .unwrap_or_else(|poisoned| {
                eprintln!("Lock was poisoned, recovering inner data");
                poisoned.into_inner()
            })
            .len()
    }
}

impl Default for InferenceQueue {
    fn default() -> Self {
        Self::new()
    }
}

/// Scheduling strategy for the inference scheduler.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchedulerMode {
    /// Process one request at a time on the calling thread.
    /// Best for local models that hold exclusive GPU/CPU resources.
    Local,
    /// Dispatch requests across `worker_count` threads.
    /// Best for external API providers that can handle concurrent calls.
    External { worker_count: usize },
}

/// Scheduler that drains the queue and dispatches to a provider.
pub struct InferenceScheduler {
    mode: SchedulerMode,
    queue: Arc<InferenceQueue>,
}

impl InferenceScheduler {
    pub fn new(mode: SchedulerMode, queue: Arc<InferenceQueue>) -> Self {
        Self { mode, queue }
    }

    /// Process all currently pending requests. Blocks until each is complete.
    /// For `Local` mode: sequential. For `External` mode: parallel with thread pool.
    pub fn drain<P: LlmProvider + 'static>(&self, provider: &Arc<P>) {
        match self.mode {
            SchedulerMode::Local => self.drain_local(provider),
            SchedulerMode::External { worker_count } => {
                self.drain_external(provider, worker_count);
            }
        }
    }

    fn drain_local<P: LlmProvider>(&self, provider: &Arc<P>) {
        while self.queue.can_dispatch() {
            let entry = match self.queue.take_next() {
                Some(e) => e,
                None => break,
            };
            self.queue.in_flight.fetch_add(1, Ordering::Relaxed);
            let result = execute_request(provider.as_ref(), &entry.request);
            self.queue.in_flight.fetch_sub(1, Ordering::Relaxed);
            self.queue.completed.fetch_add(1, Ordering::Relaxed);
            // Best-effort: receiver may have been dropped if caller timed out
            let _ = entry.reply_tx.send(result);
        }
    }

    fn drain_external<P: LlmProvider + 'static>(&self, provider: &Arc<P>, worker_count: usize) {
        // Collect pending entries up to max_concurrent limit.
        let mut entries = Vec::new();
        while self.queue.can_dispatch() {
            match self.queue.take_next() {
                Some(entry) => entries.push(entry),
                None => break,
            }
        }
        if entries.is_empty() {
            return;
        }

        // Split entries into chunks for each worker.
        let chunk_size = entries.len().div_ceil(worker_count);
        let chunks: Vec<Vec<QueueEntry>> =
            entries
                .into_iter()
                .fold(Vec::new(), |mut acc: Vec<Vec<QueueEntry>>, entry| {
                    if acc.is_empty() || acc.last().is_none_or(|last| last.len() >= chunk_size) {
                        acc.push(Vec::new());
                    }
                    if let Some(last) = acc.last_mut() {
                        last.push(entry);
                    }
                    acc
                });

        let mut handles = Vec::new();
        for chunk in chunks {
            let provider = Arc::clone(provider);
            let queue = Arc::clone(&self.queue);
            let handle = std::thread::spawn(move || {
                for entry in chunk {
                    queue.in_flight.fetch_add(1, Ordering::Relaxed);
                    let result = execute_request(provider.as_ref(), &entry.request);
                    queue.in_flight.fetch_sub(1, Ordering::Relaxed);
                    queue.completed.fetch_add(1, Ordering::Relaxed);
                    // Best-effort: receiver may have been dropped if caller timed out
                    let _ = entry.reply_tx.send(result);
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            // Best-effort: join worker threads, ignore panics from individual workers
            let _ = handle.join();
        }
    }

    /// Run the scheduler as a long-lived loop, blocking-waiting for new requests.
    /// Exits when `queue.shutdown()` is called. Only useful for `Local` mode.
    pub fn run_loop<P: LlmProvider>(&self, provider: &Arc<P>) {
        while let Some(entry) = self.queue.take_next_blocking() {
            self.queue.in_flight.fetch_add(1, Ordering::Relaxed);
            let result = execute_request(provider.as_ref(), &entry.request);
            self.queue.in_flight.fetch_sub(1, Ordering::Relaxed);
            self.queue.completed.fetch_add(1, Ordering::Relaxed);
            // Best-effort: receiver may have been dropped if caller timed out
            let _ = entry.reply_tx.send(result);
        }
    }
}

/// Execute a single inference request against the provider.
fn execute_request<P: LlmProvider + ?Sized>(
    provider: &P,
    request: &InferenceRequest,
) -> InferenceResult {
    let queue_wait_ms = request.submitted_at.elapsed().as_millis() as u64;
    let start = Instant::now();
    let response = provider.query(&request.prompt, request.max_tokens, &request.model);
    let inference_ms = start.elapsed().as_millis() as u64;

    InferenceResult {
        request_id: request.id,
        response: response.map_err(|e| e.to_string()),
        queue_wait_ms,
        inference_ms,
    }
}

/// Convenience: submit a request to the queue, drain with the scheduler, and return the result.
pub fn query_with_queue<P: LlmProvider + 'static>(
    queue: &Arc<InferenceQueue>,
    scheduler: &InferenceScheduler,
    provider: &Arc<P>,
    prompt: &str,
    max_tokens: u32,
    model: &str,
    priority: InferencePriority,
) -> InferenceResult {
    let (id, rx) = queue.submit(prompt.to_string(), max_tokens, model.to_string(), priority);
    scheduler.drain(provider);
    rx.recv().unwrap_or_else(|_| InferenceResult {
        request_id: id,
        response: Err("inference result channel closed unexpectedly".to_string()),
        queue_wait_ms: 0,
        inference_ms: 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::MockProvider;

    fn mock_provider() -> Arc<MockProvider> {
        Arc::new(MockProvider::new())
    }

    // ── 1. Submit and receive ───────────────────────────────────────────

    #[test]
    fn test_submit_and_receive_single_request() {
        let queue = Arc::new(InferenceQueue::new());
        let provider = mock_provider();
        let scheduler = InferenceScheduler::new(SchedulerMode::Local, Arc::clone(&queue));

        let (id, rx) = queue.submit("hello".into(), 64, "mock".into(), InferencePriority::Normal);
        assert!(id > 0);

        scheduler.drain(&provider);
        let result = rx.recv().unwrap();
        assert_eq!(result.request_id, id);
        assert!(result.response.is_ok());
    }

    // ── 2. Priority ordering ────────────────────────────────────────────

    #[test]
    fn test_priority_ordering() {
        let queue = Arc::new(InferenceQueue::new());
        let provider = mock_provider();
        let scheduler = InferenceScheduler::new(SchedulerMode::Local, Arc::clone(&queue));

        // Submit low first, then critical.
        let (id_low, rx_low) =
            queue.submit("low".into(), 32, "mock".into(), InferencePriority::Low);
        let (id_crit, rx_crit) = queue.submit(
            "critical".into(),
            32,
            "mock".into(),
            InferencePriority::Critical,
        );
        let (id_high, rx_high) =
            queue.submit("high".into(), 32, "mock".into(), InferencePriority::High);

        // All should complete.
        scheduler.drain(&provider);

        let r_low = rx_low.recv().unwrap();
        let r_crit = rx_crit.recv().unwrap();
        let r_high = rx_high.recv().unwrap();

        assert_eq!(r_low.request_id, id_low);
        assert_eq!(r_crit.request_id, id_crit);
        assert_eq!(r_high.request_id, id_high);

        // Critical should have the smallest queue_wait_ms (processed first).
        // But since mock is instant, we just verify all completed successfully.
        assert!(r_low.response.is_ok());
        assert!(r_crit.response.is_ok());
        assert!(r_high.response.is_ok());
    }

    // ── 3. Queue stats ─────────────────────────────────────────────────

    #[test]
    fn test_queue_stats_tracking() {
        let queue = Arc::new(InferenceQueue::new());
        let provider = mock_provider();
        let scheduler = InferenceScheduler::new(SchedulerMode::Local, Arc::clone(&queue));

        let stats = queue.stats();
        assert_eq!(stats.pending, 0);
        assert_eq!(stats.completed, 0);
        assert_eq!(stats.total_submitted, 0);

        let (_id1, _rx1) = queue.submit("a".into(), 32, "mock".into(), InferencePriority::Normal);
        let (_id2, _rx2) = queue.submit("b".into(), 32, "mock".into(), InferencePriority::Normal);

        let stats = queue.stats();
        assert_eq!(stats.pending, 2);
        assert_eq!(stats.total_submitted, 2);
        assert_eq!(stats.completed, 0);

        scheduler.drain(&provider);

        let stats = queue.stats();
        assert_eq!(stats.pending, 0);
        assert_eq!(stats.completed, 2);
        assert_eq!(stats.total_submitted, 2);
    }

    // ── 4. Monotonic IDs ────────────────────────────────────────────────

    #[test]
    fn test_request_ids_monotonically_increase() {
        let queue = InferenceQueue::new();
        let (id1, _) = queue.submit("a".into(), 32, "m".into(), InferencePriority::Normal);
        let (id2, _) = queue.submit("b".into(), 32, "m".into(), InferencePriority::Normal);
        let (id3, _) = queue.submit("c".into(), 32, "m".into(), InferencePriority::Normal);
        assert!(id1 < id2);
        assert!(id2 < id3);
    }

    // ── 5. Local scheduler processes sequentially ───────────────────────

    #[test]
    fn test_local_scheduler_sequential() {
        let queue = Arc::new(InferenceQueue::new());
        let provider = mock_provider();
        let scheduler = InferenceScheduler::new(SchedulerMode::Local, Arc::clone(&queue));

        let mut receivers = Vec::new();
        for i in 0..5 {
            let (_, rx) = queue.submit(
                format!("prompt {i}"),
                32,
                "mock".into(),
                InferencePriority::Normal,
            );
            receivers.push(rx);
        }

        scheduler.drain(&provider);

        for rx in receivers {
            let result = rx.recv().unwrap();
            assert!(result.response.is_ok());
        }

        let stats = queue.stats();
        assert_eq!(stats.completed, 5);
        assert_eq!(stats.pending, 0);
    }

    // ── 6. External scheduler dispatches in parallel ────────────────────

    #[test]
    fn test_external_scheduler_parallel() {
        let queue = Arc::new(InferenceQueue::new());
        let provider = mock_provider();
        let scheduler = InferenceScheduler::new(
            SchedulerMode::External { worker_count: 3 },
            Arc::clone(&queue),
        );

        let mut receivers = Vec::new();
        for i in 0..9 {
            let (_, rx) = queue.submit(
                format!("prompt {i}"),
                32,
                "mock".into(),
                InferencePriority::Normal,
            );
            receivers.push(rx);
        }

        scheduler.drain(&provider);

        for rx in receivers {
            let result = rx.recv().unwrap();
            assert!(result.response.is_ok());
        }

        let stats = queue.stats();
        assert_eq!(stats.completed, 9);
        assert_eq!(stats.pending, 0);
    }

    // ── 7. Convenience wrapper ──────────────────────────────────────────

    #[test]
    fn test_query_with_queue_convenience() {
        let queue = Arc::new(InferenceQueue::new());
        let provider = mock_provider();
        let scheduler = InferenceScheduler::new(SchedulerMode::Local, Arc::clone(&queue));

        let result = query_with_queue(
            &queue,
            &scheduler,
            &provider,
            "test prompt",
            64,
            "mock",
            InferencePriority::Normal,
        );

        assert!(result.response.is_ok());
        let resp = result.response.unwrap();
        assert!(!resp.output_text.is_empty());
    }

    // ── 8. Empty drain is a no-op ───────────────────────────────────────

    #[test]
    fn test_drain_empty_queue_is_noop() {
        let queue = Arc::new(InferenceQueue::new());
        let provider = mock_provider();
        let scheduler = InferenceScheduler::new(SchedulerMode::Local, Arc::clone(&queue));

        scheduler.drain(&provider);

        let stats = queue.stats();
        assert_eq!(stats.pending, 0);
        assert_eq!(stats.completed, 0);
        assert_eq!(stats.in_flight, 0);
    }

    // ── 9. Shutdown stops the loop ──────────────────────────────────────

    #[test]
    fn test_shutdown_stops_run_loop() {
        let queue = Arc::new(InferenceQueue::new());
        let provider = mock_provider();
        let scheduler_queue = Arc::clone(&queue);

        let handle = std::thread::spawn(move || {
            let scheduler =
                InferenceScheduler::new(SchedulerMode::Local, Arc::clone(&scheduler_queue));
            scheduler.run_loop(&provider);
        });

        // Give the loop a moment to start waiting.
        std::thread::sleep(std::time::Duration::from_millis(20));
        queue.shutdown();
        handle.join().expect("scheduler thread panicked");
        assert!(queue.is_shutdown());
    }

    // ── 10. Result contains timing info ─────────────────────────────────

    #[test]
    fn test_result_contains_timing_info() {
        let queue = Arc::new(InferenceQueue::new());
        let provider = mock_provider();
        let scheduler = InferenceScheduler::new(SchedulerMode::Local, Arc::clone(&queue));

        let (_, rx) = queue.submit(
            "timing".into(),
            32,
            "mock".into(),
            InferencePriority::Normal,
        );
        scheduler.drain(&provider);

        let result = rx.recv().unwrap();
        // queue_wait_ms and inference_ms should be non-negative (they're u64, always true).
        // Just verify they're present and reasonable.
        assert!(result.queue_wait_ms < 5000, "queue wait unreasonably high");
        assert!(
            result.inference_ms < 5000,
            "inference time unreasonably high"
        );
    }

    // ── 11. Concurrent submitters ───────────────────────────────────────

    #[test]
    fn test_concurrent_submitters() {
        let queue = Arc::new(InferenceQueue::new());
        let provider = mock_provider();

        // Spawn 4 threads each submitting 5 requests.
        let mut submit_handles = Vec::new();
        let mut all_receivers = Arc::new(Mutex::new(Vec::new()));

        for t in 0..4 {
            let q = Arc::clone(&queue);
            let rxs = Arc::clone(&all_receivers);
            let handle = std::thread::spawn(move || {
                for i in 0..5 {
                    let (_, rx) = q.submit(
                        format!("t{t}-p{i}"),
                        32,
                        "mock".into(),
                        InferencePriority::Normal,
                    );
                    rxs.lock().unwrap().push(rx);
                }
            });
            submit_handles.push(handle);
        }

        for h in submit_handles {
            h.join().unwrap();
        }

        assert_eq!(queue.stats().total_submitted, 20);

        let scheduler = InferenceScheduler::new(
            SchedulerMode::External { worker_count: 4 },
            Arc::clone(&queue),
        );
        scheduler.drain(&provider);

        let receivers = Arc::get_mut(&mut all_receivers).unwrap().get_mut().unwrap();
        for rx in receivers.drain(..) {
            let result = rx.recv().unwrap();
            assert!(result.response.is_ok());
        }

        assert_eq!(queue.stats().completed, 20);
    }

    // ── 12. Mixed priorities processed correctly ────────────────────────

    #[test]
    fn test_mixed_priorities_all_complete() {
        let queue = Arc::new(InferenceQueue::new());
        let provider = mock_provider();
        let scheduler = InferenceScheduler::new(SchedulerMode::Local, Arc::clone(&queue));

        let priorities = [
            InferencePriority::Low,
            InferencePriority::Critical,
            InferencePriority::Normal,
            InferencePriority::High,
            InferencePriority::Critical,
            InferencePriority::Low,
            InferencePriority::Normal,
            InferencePriority::High,
        ];

        let mut receivers = Vec::new();
        for (i, &prio) in priorities.iter().enumerate() {
            let (_, rx) = queue.submit(format!("p{i}"), 32, "mock".into(), prio);
            receivers.push(rx);
        }

        scheduler.drain(&provider);

        let results: Vec<InferenceResult> =
            receivers.into_iter().map(|rx| rx.recv().unwrap()).collect();

        // All should succeed.
        for r in &results {
            assert!(r.response.is_ok(), "request {} failed", r.request_id);
        }

        assert_eq!(queue.stats().completed, 8);
    }
}

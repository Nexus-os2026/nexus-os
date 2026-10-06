//! A minimal DevTools protocol client over the browser's pipe.
//!
//! The browser reads commands on its descriptor 3 and writes replies and
//! events on its descriptor 4, as NUL-terminated JSON. There is no
//! debugging port: nothing else on the machine can drive this browser.

use crate::authority::run::CancelToken;
use serde_json::{json, Value};
use std::collections::{HashMap, VecDeque};
use std::io::{Read, Write};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const MAX_MESSAGE: usize = 16 * 1024 * 1024;
/// Events kept: only `Target.*` events (the session closes pages it did not
/// open), at most this many and this many bytes, the oldest dropped first.
const MAX_EVENTS: usize = 1024;
const MAX_EVENT_BYTES: usize = 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum CdpError {
    Closed,
    Timeout,
    Cancelled,
    Protocol(String),
}

type Pending = Arc<Mutex<HashMap<u64, mpsc::Sender<Value>>>>;

pub(crate) struct Cdp {
    writer: Mutex<std::io::PipeWriter>,
    next: AtomicU64,
    pending: Pending,
    events: Arc<Mutex<VecDeque<(usize, Value)>>>,
}

impl Cdp {
    pub(crate) fn new(writer: std::io::PipeWriter, mut reader: std::io::PipeReader) -> Self {
        let pending: Pending = Arc::new(Mutex::new(HashMap::new()));
        let events = Arc::new(Mutex::new(VecDeque::new()));
        {
            let (pending, events) = (pending.clone(), events.clone());
            std::thread::Builder::new()
                .name("nexus-p3-cdp".into())
                .spawn(move || {
                    let mut buffer: Vec<u8> = Vec::new();
                    // Bytes already searched for a terminator.
                    let mut scanned = 0;
                    let mut chunk = vec![0u8; 64 * 1024];
                    loop {
                        let n = match reader.read(&mut chunk) {
                            Ok(0) | Err(_) => break,
                            Ok(n) => n,
                        };
                        buffer.extend_from_slice(&chunk[..n]);
                        while let Some(found) = buffer[scanned..].iter().position(|b| *b == 0) {
                            let end = scanned + found;
                            scanned = 0;
                            let message: Vec<u8> = buffer.drain(..=end).collect();
                            let Ok(value) = serde_json::from_slice::<Value>(&message[..end]) else {
                                continue;
                            };
                            match value.get("id").and_then(Value::as_u64) {
                                Some(id) => {
                                    if let Some(reply) =
                                        pending.lock().expect("pending").remove(&id)
                                    {
                                        let _ = reply.send(value);
                                    }
                                }
                                None => {
                                    let target = value
                                        .get("method")
                                        .and_then(Value::as_str)
                                        .is_some_and(|method| method.starts_with("Target."));
                                    if !target {
                                        continue;
                                    }
                                    let mut queue = events.lock().expect("events");
                                    queue.push_back((end, value));
                                    let mut bytes: usize = queue.iter().map(|(size, _)| size).sum();
                                    while queue.len() > MAX_EVENTS || bytes > MAX_EVENT_BYTES {
                                        match queue.pop_front() {
                                            Some((size, _)) => bytes -= size,
                                            None => break,
                                        }
                                    }
                                }
                            }
                        }
                        scanned = buffer.len();
                        if buffer.len() > MAX_MESSAGE {
                            break;
                        }
                    }
                    // The browser is gone: fail everything still waiting.
                    pending.lock().expect("pending").clear();
                })
                .expect("the devtools reader starts");
        }
        Self {
            writer: Mutex::new(writer),
            next: AtomicU64::new(1),
            pending,
            events,
        }
    }

    /// Send one command and wait for its reply, observing `cancel`. Nothing
    /// is sent once the session is cancelled, and a reply that arrives after
    /// the cancellation is not used: no further step runs after a stop.
    pub(crate) fn call(
        &self,
        method: &str,
        params: Value,
        session: Option<&str>,
        timeout: Duration,
        cancel: &CancelToken,
    ) -> Result<Value, CdpError> {
        if cancel.is_cancelled() {
            return Err(CdpError::Cancelled);
        }
        let id = self.next.fetch_add(1, Ordering::SeqCst);
        let mut message = json!({ "id": id, "method": method, "params": params });
        if let Some(session) = session {
            message["sessionId"] = Value::String(session.to_string());
        }
        let (sender, receiver) = mpsc::channel();
        self.pending.lock().expect("pending").insert(id, sender);
        {
            let mut bytes =
                serde_json::to_vec(&message).map_err(|e| CdpError::Protocol(e.to_string()))?;
            bytes.push(0);
            let mut writer = self.writer.lock().expect("writer");
            if writer
                .write_all(&bytes)
                .and_then(|()| writer.flush())
                .is_err()
            {
                self.pending.lock().expect("pending").remove(&id);
                return Err(CdpError::Closed);
            }
        }
        let deadline = Instant::now() + timeout;
        loop {
            match receiver.recv_timeout(Duration::from_millis(50)) {
                Ok(_) if cancel.is_cancelled() => return Err(CdpError::Cancelled),
                Ok(reply) => {
                    if let Some(error) = reply.get("error") {
                        let text = error
                            .get("message")
                            .and_then(Value::as_str)
                            .unwrap_or("devtools error");
                        return Err(CdpError::Protocol(text.chars().take(200).collect()));
                    }
                    return Ok(reply.get("result").cloned().unwrap_or(Value::Null));
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => return Err(CdpError::Closed),
                Err(mpsc::RecvTimeoutError::Timeout) => {}
            }
            if cancel.is_cancelled() {
                self.pending.lock().expect("pending").remove(&id);
                return Err(CdpError::Cancelled);
            }
            if Instant::now() >= deadline {
                self.pending.lock().expect("pending").remove(&id);
                return Err(CdpError::Timeout);
            }
        }
    }

    /// Take the events received so far (only `Target.*` events are kept).
    pub(crate) fn drain_events(&self) -> Vec<Value> {
        self.events
            .lock()
            .expect("events")
            .drain(..)
            .map(|(_, event)| event)
            .collect()
    }
}

#[cfg(test)]
mod tests;

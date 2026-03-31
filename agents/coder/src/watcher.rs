use nexus_sdk::errors::AgentError;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, TryRecvError};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, UNIX_EPOCH};

const POLL_INTERVAL: Duration = Duration::from_millis(100);
const DEFAULT_DEBOUNCE: Duration = Duration::from_millis(500);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileEvent {
    Created(String),
    Modified(String),
    Deleted(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WatchTrigger {
    AutoRecompile,
    AutoTest,
    AutoLint,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FileFingerprint {
    modified_nanos: u128,
    size: u64,
}

#[derive(Debug, Clone)]
struct PendingEvent {
    event: FileEvent,
    observed_at: std::time::Instant,
}

pub struct FileWatcher {
    receiver: Receiver<FileEvent>,
    stop: Arc<AtomicBool>,
    handle: Option<thread::JoinHandle<()>>,
}

impl FileWatcher {
    pub fn recv_timeout(&self, timeout: Duration) -> Result<FileEvent, RecvTimeoutError> {
        self.receiver.recv_timeout(timeout)
    }

    pub fn try_recv(&self) -> Result<FileEvent, TryRecvError> {
        self.receiver.try_recv()
    }
}

impl Iterator for FileWatcher {
    type Item = FileEvent;

    fn next(&mut self) -> Option<Self::Item> {
        // Optional: return None when sender is disconnected (watcher stopped)
        self.receiver.recv().ok()
    }
}

impl Drop for FileWatcher {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            // Best-effort: join watcher thread on drop, ignore panics
            let _ = handle.join();
        }
    }
}

pub fn watch(directory: impl AsRef<Path>, patterns: &[&str]) -> Result<FileWatcher, AgentError> {
    let root = directory.as_ref().to_path_buf();
    if !root.exists() {
        return Err(AgentError::SupervisorError(format!(
            "watch path '{}' does not exist",
            root.display()
        )));
    }
    if !root.is_dir() {
        return Err(AgentError::SupervisorError(format!(
            "watch path '{}' must be a directory",
            root.display()
        )));
    }

    let watched_patterns = if patterns.is_empty() {
        vec!["*".to_string()]
    } else {
        patterns.iter().map(|value| value.to_string()).collect()
    };

    let stop = Arc::new(AtomicBool::new(false));
    let stop_token = Arc::clone(&stop);
    let (sender, receiver) = mpsc::channel::<FileEvent>();
    let handle = thread::spawn(move || {
        run_watch_loop(root, watched_patterns, stop_token, sender);
    });

    Ok(FileWatcher {
        receiver,
        stop,
        handle: Some(handle),
    })
}

pub fn suggested_triggers(event: &FileEvent) -> Vec<WatchTrigger> {
    let path = match event {
        FileEvent::Created(path) | FileEvent::Modified(path) | FileEvent::Deleted(path) => path,
    };

    let lowered = path.to_ascii_lowercase();
    if lowered.ends_with(".rs") || lowered.ends_with(".go") {
        return vec![
            WatchTrigger::AutoRecompile,
            WatchTrigger::AutoTest,
            WatchTrigger::AutoLint,
        ];
    }
    if lowered.ends_with(".ts")
        || lowered.ends_with(".tsx")
        || lowered.ends_with(".js")
        || lowered.ends_with(".jsx")
        || lowered.ends_with(".py")
    {
        return vec![WatchTrigger::AutoTest, WatchTrigger::AutoLint];
    }
    vec![WatchTrigger::AutoLint]
}

fn run_watch_loop(
    root: PathBuf,
    patterns: Vec<String>,
    stop: Arc<AtomicBool>,
    sender: mpsc::Sender<FileEvent>,
) {
    let mut previous = snapshot(root.as_path(), patterns.as_slice()).unwrap_or_default();
    let mut pending = HashMap::<String, PendingEvent>::new();

    while !stop.load(Ordering::Relaxed) {
        thread::sleep(POLL_INTERVAL);

        let current = match snapshot(root.as_path(), patterns.as_slice()) {
            Ok(value) => value,
            Err(_) => continue,
        };

        let now = std::time::Instant::now();
        for (path, fingerprint) in &current {
            match previous.get(path) {
                None => update_pending(&mut pending, FileEvent::Created(path.clone()), now),
                Some(previous_fingerprint) if previous_fingerprint != fingerprint => {
                    update_pending(&mut pending, FileEvent::Modified(path.clone()), now)
                }
                Some(_) => {}
            }
        }

        for path in previous.keys() {
            if !current.contains_key(path) {
                update_pending(&mut pending, FileEvent::Deleted(path.clone()), now);
            }
        }

        previous = current;
        flush_debounced(&mut pending, now, &sender);
    }

    let now = std::time::Instant::now();
    flush_debounced_immediately(&mut pending, now, &sender);
}

fn snapshot(
    root: &Path,
    patterns: &[String],
) -> Result<HashMap<String, FileFingerprint>, AgentError> {
    let mut stack = vec![root.to_path_buf()];
    let mut map = HashMap::new();

    while let Some(dir) = stack.pop() {
        let entries = fs::read_dir(&dir).map_err(|error| {
            AgentError::SupervisorError(format!(
                "failed reading directory '{}': {error}",
                dir.display()
            ))
        })?;

        for entry in entries {
            let entry = entry.map_err(|error| {
                AgentError::SupervisorError(format!(
                    "failed reading entry in '{}': {error}",
                    dir.display()
                ))
            })?;
            let path = entry.path();
            let file_type = entry.file_type().map_err(|error| {
                AgentError::SupervisorError(format!(
                    "failed reading file type '{}': {error}",
                    path.display()
                ))
            })?;

            if file_type.is_dir() {
                let name = entry.file_name().to_string_lossy().to_string();
                if is_ignored_dir(name.as_str()) {
                    continue;
                }
                stack.push(path);
                continue;
            }
            if !file_type.is_file() {
                continue;
            }

            let relative = match path.strip_prefix(root) {
                Ok(value) => value.to_string_lossy().replace('\\', "/"),
                Err(_) => continue,
            };
            if !matches_patterns(relative.as_str(), patterns) {
                continue;
            }

            let metadata = entry.metadata().map_err(|error| {
                AgentError::SupervisorError(format!(
                    "failed reading metadata '{}': {error}",
                    path.display()
                ))
            })?;
            let modified_nanos = metadata
                .modified()
                // Optional: modification time may not be available on all filesystems
                .ok()
                // Optional: system time may be before UNIX_EPOCH on misconfigured clocks
                .and_then(|value| value.duration_since(UNIX_EPOCH).ok())
                .map(|value| value.as_nanos())
                .unwrap_or(0);

            map.insert(
                relative,
                FileFingerprint {
                    modified_nanos,
                    size: metadata.len(),
                },
            );
        }
    }

    Ok(map)
}

fn update_pending(
    pending: &mut HashMap<String, PendingEvent>,
    next: FileEvent,
    now: std::time::Instant,
) {
    let path = match &next {
        FileEvent::Created(path) | FileEvent::Modified(path) | FileEvent::Deleted(path) => path,
    }
    .clone();

    let merged = match pending.get(path.as_str()) {
        Some(existing) => merge_events(existing.event.clone(), next),
        None => next,
    };
    pending.insert(
        path,
        PendingEvent {
            event: merged,
            observed_at: now,
        },
    );
}

fn flush_debounced(
    pending: &mut HashMap<String, PendingEvent>,
    now: std::time::Instant,
    sender: &mpsc::Sender<FileEvent>,
) {
    let mut ready = Vec::new();
    for (path, event) in pending.iter() {
        if now.duration_since(event.observed_at) >= DEFAULT_DEBOUNCE {
            ready.push(path.clone());
        }
    }
    for path in ready {
        if let Some(pending_event) = pending.remove(path.as_str()) {
            // Best-effort: send debounced file event, receiver may have been dropped
            let _ = sender.send(pending_event.event);
        }
    }
}

fn flush_debounced_immediately(
    pending: &mut HashMap<String, PendingEvent>,
    _now: std::time::Instant,
    sender: &mpsc::Sender<FileEvent>,
) {
    let mut keys = pending.keys().cloned().collect::<Vec<_>>();
    keys.sort();
    for key in keys {
        if let Some(pending_event) = pending.remove(key.as_str()) {
            // Best-effort: flush pending file event immediately, receiver may have been dropped
            let _ = sender.send(pending_event.event);
        }
    }
}

fn merge_events(previous: FileEvent, next: FileEvent) -> FileEvent {
    match (previous, next) {
        (FileEvent::Created(path), FileEvent::Modified(_)) => FileEvent::Created(path),
        (FileEvent::Created(path), FileEvent::Deleted(_)) => FileEvent::Deleted(path),
        (FileEvent::Modified(path), FileEvent::Deleted(_)) => FileEvent::Deleted(path),
        (FileEvent::Deleted(path), FileEvent::Created(_)) => FileEvent::Modified(path),
        (_, event) => event,
    }
}

fn matches_patterns(path: &str, patterns: &[String]) -> bool {
    patterns
        .iter()
        .any(|pattern| wildcard_match(pattern.as_str(), path))
}

fn wildcard_match(pattern: &str, text: &str) -> bool {
    let pattern = pattern.replace('\\', "/");
    if pattern == "*" || pattern == "**/*" {
        return true;
    }
    if let Some(ext) = pattern.strip_prefix("*.") {
        return text.ends_with(format!(".{ext}").as_str());
    }
    if !pattern.contains('*') {
        return text == pattern;
    }

    let anchors_start = !pattern.starts_with('*');
    let anchors_end = !pattern.ends_with('*');
    let parts = pattern.split('*').collect::<Vec<_>>();
    let mut cursor = 0_usize;

    for (index, part) in parts.iter().enumerate() {
        if part.is_empty() {
            continue;
        }
        if index == 0 && anchors_start {
            if !text.starts_with(part) {
                return false;
            }
            cursor = part.len();
            continue;
        }

        if let Some(position) = text[cursor..].find(part) {
            cursor += position + part.len();
        } else {
            return false;
        }
    }

    if anchors_end {
        if let Some(last) = parts.last() {
            return text.ends_with(last);
        }
    }
    true
}

fn is_ignored_dir(name: &str) -> bool {
    matches!(
        name,
        ".git" | "target" | "node_modules" | "__pycache__" | ".venv"
    )
}

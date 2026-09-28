//! Bounded frontend error reports (Final Gate resource bound).
//!
//! `log_frontend_error` is callable by any script in the webview, and the
//! interface's global `error` and `unhandledrejection` handlers call it for
//! every uncaught error. So each field is cut to
//! [`MAX_FRONTEND_ERROR_FIELD_BYTES`] with a visible marker, on stderr and in
//! the log, and `frontend_errors.log` stops growing at
//! [`MAX_FRONTEND_ERROR_LOG_BYTES`]: a record that would pass the cap is not
//! written, and one notice goes to stderr per process. Records are only ever
//! appended; nothing is rotated, truncated or deleted.

use std::borrow::Cow;
use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

/// Longest message, stack or component stack kept from one report, in bytes.
pub(crate) const MAX_FRONTEND_ERROR_FIELD_BYTES: usize = 8 * 1024;
/// `frontend_errors.log` is never appended past this size, in bytes.
pub(crate) const MAX_FRONTEND_ERROR_LOG_BYTES: u64 = 4 * 1024 * 1024;

/// Keep at most [`MAX_FRONTEND_ERROR_FIELD_BYTES`] of `text`, cut on a UTF-8
/// character boundary, followed by a marker naming how many bytes were cut.
pub(crate) fn frontend_error_field(text: &str) -> Cow<'_, str> {
    if text.len() <= MAX_FRONTEND_ERROR_FIELD_BYTES {
        return Cow::Borrowed(text);
    }
    let end = text.floor_char_boundary(MAX_FRONTEND_ERROR_FIELD_BYTES);
    Cow::Owned(format!(
        "{}…[truncated {} bytes]",
        &text[..end],
        text.len() - end
    ))
}

/// What happened to one record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FrontendErrorAppend {
    Appended,
    /// Appending would take the log past its cap, so nothing was written.
    /// `first` is true only for the first such record of this `notice_sent`.
    DroppedAtCap {
        first: bool,
    },
    /// The log could not be opened, measured or written.
    Unavailable,
}

/// Append `record` to `path` unless the file would then exceed `cap` bytes.
/// The size is taken from the handle that is appended to. Callers that may
/// run concurrently serialize through one lock.
pub(crate) fn append_frontend_error_record(
    path: &Path,
    record: &str,
    cap: u64,
    notice_sent: &AtomicBool,
) -> FrontendErrorAppend {
    let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    else {
        return FrontendErrorAppend::Unavailable;
    };
    let Ok(metadata) = file.metadata() else {
        return FrontendErrorAppend::Unavailable;
    };
    if metadata.len().saturating_add(record.len() as u64) > cap {
        return FrontendErrorAppend::DroppedAtCap {
            first: !notice_sent.swap(true, Ordering::SeqCst),
        };
    }
    match file.write_all(record.as_bytes()) {
        Ok(()) => FrontendErrorAppend::Appended,
        Err(_) => FrontendErrorAppend::Unavailable,
    }
}

/// One log record, in the format the log has always used.
pub(crate) fn frontend_error_record(
    timestamp: &str,
    message: &str,
    stack: &str,
    component_stack: &str,
) -> String {
    format!("[{timestamp}] {message}\n{stack}\n{component_stack}\n---\n")
}

/// Report one interface error on stderr and in
/// `<identity home>/.nexus/frontend_errors.log`, both bounded.
pub(crate) fn record_frontend_error(message: &str, stack: &str, component_stack: &str) {
    static APPEND: Mutex<()> = Mutex::new(());
    static CAP_NOTICE_SENT: AtomicBool = AtomicBool::new(false);

    let message = frontend_error_field(message);
    let stack = frontend_error_field(stack);
    let component_stack = frontend_error_field(component_stack);
    eprintln!("[FRONTEND ERROR] {message}");
    if !stack.is_empty() {
        eprintln!("[FRONTEND STACK] {stack}");
    }
    if !component_stack.is_empty() {
        eprintln!("[COMPONENT STACK] {component_stack}");
    }

    // Also append to a log file for post-mortem debugging.
    let Ok(log_dir) = nexus_kernel::identity_home::nexus_state_dir() else {
        return;
    };
    let _ = std::fs::create_dir_all(&log_dir);
    let record = frontend_error_record(
        &chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        &message,
        &stack,
        &component_stack,
    );
    let outcome = {
        let _serialized = APPEND.lock().unwrap_or_else(|p| p.into_inner());
        append_frontend_error_record(
            &log_dir.join("frontend_errors.log"),
            &record,
            MAX_FRONTEND_ERROR_LOG_BYTES,
            &CAP_NOTICE_SENT,
        )
    };
    if outcome == (FrontendErrorAppend::DroppedAtCap { first: true }) {
        eprintln!(
            "[FRONTEND ERROR] frontend_errors.log has reached {MAX_FRONTEND_ERROR_LOG_BYTES} bytes; \
             further reports go to stderr only"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fresh scratch directory, removed when the test ends.
    struct Scratch(std::path::PathBuf);

    impl Scratch {
        fn new() -> Self {
            let dir =
                std::env::temp_dir().join(format!("nexus-fg-k-errlog-{}", uuid::Uuid::new_v4()));
            std::fs::create_dir(&dir).unwrap();
            Self(dir)
        }
        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn p0_fg_k_fields_are_cut_on_a_char_boundary_with_a_marker() {
        let short = "boom";
        assert!(matches!(frontend_error_field(short), Cow::Borrowed("boom")));
        let exact = "a".repeat(MAX_FRONTEND_ERROR_FIELD_BYTES);
        assert_eq!(frontend_error_field(&exact), exact.as_str());

        let long = "a".repeat(MAX_FRONTEND_ERROR_FIELD_BYTES + 100);
        let cut = frontend_error_field(&long);
        assert!(cut.starts_with(&"a".repeat(MAX_FRONTEND_ERROR_FIELD_BYTES)));
        assert!(
            cut.ends_with("…[truncated 100 bytes]"),
            "{}",
            &cut[cut.len() - 40..]
        );

        // A multi-byte character straddling the limit is dropped whole.
        let mut straddle = "a".repeat(MAX_FRONTEND_ERROR_FIELD_BYTES - 1);
        straddle.push('é');
        straddle.push_str("tail");
        let cut = frontend_error_field(&straddle);
        assert!(cut.starts_with(&"a".repeat(MAX_FRONTEND_ERROR_FIELD_BYTES - 1)));
        assert!(cut.ends_with("…[truncated 6 bytes]"));

        let huge = "x".repeat(1 << 20);
        assert!(frontend_error_field(&huge).len() < MAX_FRONTEND_ERROR_FIELD_BYTES + 64);
    }

    #[test]
    fn p0_fg_k_the_log_stops_at_its_cap_and_keeps_every_byte() {
        let dir = Scratch::new();
        let path = dir.path().join("frontend_errors.log");
        let notice = AtomicBool::new(false);
        let record = frontend_error_record("t", "message", "stack", "");
        let cap = (record.len() * 3) as u64;

        for _ in 0..3 {
            assert_eq!(
                append_frontend_error_record(&path, &record, cap, &notice),
                FrontendErrorAppend::Appended
            );
        }
        let full = std::fs::read(&path).unwrap();
        assert_eq!(full.len() as u64, cap);

        assert_eq!(
            append_frontend_error_record(&path, &record, cap, &notice),
            FrontendErrorAppend::DroppedAtCap { first: true }
        );
        assert_eq!(
            append_frontend_error_record(&path, "x", cap, &notice),
            FrontendErrorAppend::DroppedAtCap { first: false }
        );
        assert_eq!(
            std::fs::read(&path).unwrap(),
            full,
            "nothing appended or cut"
        );
    }

    #[test]
    fn p0_fg_k_an_oversized_existing_log_is_kept_as_it_is() {
        let dir = Scratch::new();
        let path = dir.path().join("frontend_errors.log");
        let legacy = vec![b'L'; 64];
        std::fs::write(&path, &legacy).unwrap();
        let notice = AtomicBool::new(false);
        assert_eq!(
            append_frontend_error_record(&path, "new\n", 32, &notice),
            FrontendErrorAppend::DroppedAtCap { first: true }
        );
        assert_eq!(std::fs::read(&path).unwrap(), legacy);
    }

    #[test]
    fn p0_fg_k_a_record_is_written_whole_or_not_at_all() {
        let dir = Scratch::new();
        let path = dir.path().join("frontend_errors.log");
        let notice = AtomicBool::new(false);
        assert_eq!(
            append_frontend_error_record(&path, "12345", 8, &notice),
            FrontendErrorAppend::Appended
        );
        // 5 + 5 > 8: the second record would fit only in part.
        assert_eq!(
            append_frontend_error_record(&path, "67890", 8, &notice),
            FrontendErrorAppend::DroppedAtCap { first: true }
        );
        assert_eq!(std::fs::read(&path).unwrap(), b"12345");
    }

    #[test]
    fn p0_fg_k_an_unwritable_log_writes_nothing() {
        let dir = Scratch::new();
        let missing_parent = dir.path().join("absent").join("frontend_errors.log");
        let notice = AtomicBool::new(false);
        assert_eq!(
            append_frontend_error_record(&missing_parent, "r", 1024, &notice),
            FrontendErrorAppend::Unavailable
        );
        assert!(!dir.path().join("absent").exists());
    }

    #[test]
    fn p0_fg_k_the_record_format_is_unchanged() {
        assert_eq!(
            frontend_error_record("2026-09-28 18:00:00", "m", "s", "c"),
            "[2026-09-28 18:00:00] m\ns\nc\n---\n"
        );
    }
}

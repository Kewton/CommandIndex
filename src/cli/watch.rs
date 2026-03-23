pub const WATCH_AFTER_HELP: &str = "\
When to use:
  Keep index automatically up to date during development.
  Runs as a daemon watching for file changes.

Examples:
  commandindexdev watch --path .
  commandindexdev watch --debounce 3 --with-embedding";

use std::fmt;
use std::path::Path;
use std::time::{Duration, Instant};

use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;

use crate::cli::index::{IndexError, IndexOptions};
use crate::indexer::manifest::FileType;

// ---------------------------------------------------------------------------
// WatchError
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub enum WatchError {
    Notify(notify::Error),
    Index(IndexError),
    Io(std::io::Error),
    Signal(ctrlc::Error),
}

impl WatchError {
    pub fn is_recoverable(&self) -> bool {
        match self {
            WatchError::Index(e) => !matches!(
                e,
                IndexError::IndexNotFound
                    | IndexError::SchemaVersionMismatch
                    | IndexError::IndexCorrupted(_)
                    | IndexError::Config(_)
            ),
            WatchError::Notify(_) => false,
            WatchError::Io(_) => true,
            WatchError::Signal(_) => false,
        }
    }
}

impl fmt::Display for WatchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WatchError::Notify(e) => write!(f, "File watcher error: {e}"),
            WatchError::Index(e) => write!(f, "Index error: {e}"),
            WatchError::Io(e) => write!(f, "IO error: {e}"),
            WatchError::Signal(e) => write!(f, "Signal handler error: {e}"),
        }
    }
}

impl std::error::Error for WatchError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            WatchError::Notify(e) => Some(e),
            WatchError::Index(e) => Some(e),
            WatchError::Io(e) => Some(e),
            WatchError::Signal(e) => Some(e),
        }
    }
}

impl From<IndexError> for WatchError {
    fn from(e: IndexError) -> Self {
        WatchError::Index(e)
    }
}

impl From<notify::Error> for WatchError {
    fn from(e: notify::Error) -> Self {
        WatchError::Notify(e)
    }
}

impl From<std::io::Error> for WatchError {
    fn from(e: std::io::Error) -> Self {
        WatchError::Io(e)
    }
}

impl From<ctrlc::Error> for WatchError {
    fn from(e: ctrlc::Error) -> Self {
        WatchError::Signal(e)
    }
}

// ---------------------------------------------------------------------------
// is_relevant_event
// ---------------------------------------------------------------------------

/// Determine whether a filesystem event path is relevant for watch mode.
///
/// Returns `true` when all of the following hold:
/// 1. The path is inside `base_dir`.
/// 2. The path is **not** inside the `.commandindex/` directory.
/// 3. The file extension is one of the supported types.
pub fn is_relevant_event(event_path: &Path, base_dir: &Path) -> bool {
    // Must be inside base_dir
    if !event_path.starts_with(base_dir) {
        return false;
    }

    // Ignore .commandindex/ directory
    if let Ok(rel) = event_path.strip_prefix(base_dir) {
        for component in rel.components() {
            if let std::path::Component::Normal(s) = component
                && s == ".commandindex"
            {
                return false;
            }
        }
    }

    // Check extension
    let ext = match event_path.extension().and_then(|e| e.to_str()) {
        Some(e) => e,
        None => return false,
    };

    FileType::all_extensions().contains(&ext)
}

// ---------------------------------------------------------------------------
// Debouncer
// ---------------------------------------------------------------------------

pub(crate) struct Debouncer {
    first_event_time: Option<Instant>,
    last_event_time: Option<Instant>,
    debounce_duration: Duration,
    max_wait: Duration,
    pending: bool,
}

impl Debouncer {
    pub fn new(debounce_secs: u64) -> Self {
        Self {
            first_event_time: None,
            last_event_time: None,
            debounce_duration: Duration::from_secs(debounce_secs),
            max_wait: Duration::from_secs(debounce_secs.saturating_mul(5)),
            pending: false,
        }
    }

    pub fn notify_event(&mut self) {
        let now = Instant::now();
        if self.first_event_time.is_none() {
            self.first_event_time = Some(now);
        }
        self.last_event_time = Some(now);
        self.pending = true;
    }

    pub fn should_trigger(&self) -> bool {
        if !self.pending {
            return false;
        }
        let now = Instant::now();
        // Trigger if debounce_duration elapsed since last event
        if let Some(last) = self.last_event_time
            && now.duration_since(last) >= self.debounce_duration
        {
            return true;
        }
        // Also trigger if max_wait exceeded since first event
        if let Some(first) = self.first_event_time
            && now.duration_since(first) >= self.max_wait
        {
            return true;
        }
        false
    }

    pub fn reset(&mut self) {
        self.first_event_time = None;
        self.last_event_time = None;
        self.pending = false;
    }

    #[cfg(test)]
    pub fn is_pending(&self) -> bool {
        self.pending
    }
}

// ---------------------------------------------------------------------------
// run
// ---------------------------------------------------------------------------

/// Main watch loop: monitors `path` for file changes and runs incremental
/// index updates after a debounce period.
pub fn run(path: &Path, debounce_secs: u64, with_embedding: bool) -> Result<(), WatchError> {
    let base_dir = path.canonicalize()?;

    // Check that .commandindex/ exists
    let index_dir = base_dir.join(".commandindex");
    if !index_dir.is_dir() {
        return Err(WatchError::Index(IndexError::IndexNotFound));
    }

    // Graceful shutdown flag
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    ctrlc::set_handler(move || {
        r.store(false, Ordering::SeqCst);
    })?;

    // Channel for notify events
    let (tx, rx) = mpsc::channel();

    let (err_tx, err_rx) = mpsc::channel::<notify::Error>();
    let mut watcher = RecommendedWatcher::new(
        move |res: Result<notify::Event, notify::Error>| match res {
            Ok(event) => {
                let _ = tx.send(event);
            }
            Err(e) => {
                let _ = err_tx.send(e);
            }
        },
        Config::default(),
    )?;

    watcher.watch(&base_dir, RecursiveMode::Recursive)?;

    println!(
        "Watching {} for changes (debounce: {}s) ...",
        base_dir.display(),
        debounce_secs
    );
    println!("Press Ctrl+C to stop.");

    let mut debouncer = Debouncer::new(debounce_secs);
    let poll_interval = Duration::from_millis(200);

    while running.load(Ordering::SeqCst) {
        // Check for watcher errors
        if let Ok(watcher_err) = err_rx.try_recv() {
            eprintln!("[watch] Watcher error: {watcher_err}");
            return Err(WatchError::Notify(watcher_err));
        }

        // Drain all pending events (non-blocking)
        while let Ok(event) = rx.try_recv() {
            for p in &event.paths {
                if is_relevant_event(p, &base_dir) {
                    debouncer.notify_event();
                    break;
                }
            }
        }

        // Check whether we should trigger an update
        if debouncer.should_trigger() {
            debouncer.reset();
            println!("[watch] Change detected, running incremental update ...");
            let options = IndexOptions { with_embedding };
            let commandindex_dir = base_dir.join(".commandindex");
            match crate::cli::index::run_incremental(&base_dir, &commandindex_dir, &options) {
                Ok(summary) => {
                    println!(
                        "[watch] Update done: +{} ~{} -{} (unchanged {}, skipped {}) in {:.2}s",
                        summary.added_files,
                        summary.modified_files,
                        summary.deleted_files,
                        summary.unchanged,
                        summary.skipped,
                        summary.duration.as_secs_f64(),
                    );
                }
                Err(e) => {
                    let watch_err = WatchError::Index(e);
                    if watch_err.is_recoverable() {
                        eprintln!("[watch] Recoverable error: {watch_err}");
                    } else {
                        return Err(watch_err);
                    }
                }
            }
        }

        std::thread::sleep(poll_interval);
    }

    println!("\n[watch] Shutting down gracefully.");
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- WatchError::is_recoverable tests --

    #[test]
    fn test_is_recoverable_index_not_found() {
        let err = WatchError::Index(IndexError::IndexNotFound);
        assert!(!err.is_recoverable());
    }

    #[test]
    fn test_is_recoverable_schema_version_mismatch() {
        let err = WatchError::Index(IndexError::SchemaVersionMismatch);
        assert!(!err.is_recoverable());
    }

    #[test]
    fn test_is_recoverable_index_corrupted() {
        let err = WatchError::Index(IndexError::IndexCorrupted("bad data".to_string()));
        assert!(!err.is_recoverable());
    }

    #[test]
    fn test_is_recoverable_config() {
        let err = WatchError::Index(IndexError::Config("missing key".to_string()));
        assert!(!err.is_recoverable());
    }

    #[test]
    fn test_is_recoverable_io_index() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let err = WatchError::Index(IndexError::Io(io_err));
        assert!(err.is_recoverable());
    }

    #[test]
    fn test_is_recoverable_notify() {
        let err = WatchError::Notify(notify::Error::generic("test"));
        assert!(!err.is_recoverable());
    }

    #[test]
    fn test_is_recoverable_io() {
        let io_err = std::io::Error::other("disk full");
        let err = WatchError::Io(io_err);
        assert!(err.is_recoverable());
    }

    // -- is_relevant_event tests --

    #[test]
    fn test_relevant_markdown_file() {
        let base = Path::new("/project");
        let path = Path::new("/project/docs/readme.md");
        assert!(is_relevant_event(path, base));
    }

    #[test]
    fn test_relevant_typescript_file() {
        let base = Path::new("/project");
        let path = Path::new("/project/src/index.ts");
        assert!(is_relevant_event(path, base));
    }

    #[test]
    fn test_relevant_tsx_file() {
        let base = Path::new("/project");
        let path = Path::new("/project/src/App.tsx");
        assert!(is_relevant_event(path, base));
    }

    #[test]
    fn test_relevant_python_file() {
        let base = Path::new("/project");
        let path = Path::new("/project/scripts/main.py");
        assert!(is_relevant_event(path, base));
    }

    #[test]
    fn test_irrelevant_rust_file() {
        let base = Path::new("/project");
        let path = Path::new("/project/src/main.rs");
        assert!(!is_relevant_event(path, base));
    }

    #[test]
    fn test_irrelevant_no_extension() {
        let base = Path::new("/project");
        let path = Path::new("/project/Makefile");
        assert!(!is_relevant_event(path, base));
    }

    #[test]
    fn test_ignore_commandindex_dir() {
        let base = Path::new("/project");
        let path = Path::new("/project/.commandindex/state.json");
        assert!(!is_relevant_event(path, base));
    }

    #[test]
    fn test_ignore_nested_commandindex_dir() {
        let base = Path::new("/project");
        let path = Path::new("/project/.commandindex/tantivy/meta.json");
        assert!(!is_relevant_event(path, base));
    }

    #[test]
    fn test_outside_base_dir() {
        let base = Path::new("/project");
        let path = Path::new("/other/docs/readme.md");
        assert!(!is_relevant_event(path, base));
    }

    // -- Debouncer tests --

    #[test]
    fn test_debouncer_initial_state() {
        let d = Debouncer::new(1);
        assert!(!d.is_pending());
        assert!(!d.should_trigger());
    }

    #[test]
    fn test_debouncer_pending_after_event() {
        let mut d = Debouncer::new(1);
        d.notify_event();
        assert!(d.is_pending());
    }

    #[test]
    fn test_debouncer_does_not_trigger_immediately() {
        let mut d = Debouncer::new(1);
        d.notify_event();
        // Right after event, should_trigger returns false because
        // debounce_duration has not elapsed
        assert!(!d.should_trigger());
    }

    #[test]
    fn test_debouncer_triggers_after_duration() {
        let mut d = Debouncer::new(0); // 0-second debounce
        d.notify_event();
        // With 0 debounce, should trigger immediately (>= 0 elapsed)
        // We need a tiny sleep to ensure some time passes
        std::thread::sleep(Duration::from_millis(1));
        assert!(d.should_trigger());
    }

    #[test]
    fn test_debouncer_reset() {
        let mut d = Debouncer::new(0);
        d.notify_event();
        std::thread::sleep(Duration::from_millis(1));
        assert!(d.should_trigger());
        d.reset();
        assert!(!d.is_pending());
        assert!(!d.should_trigger());
    }

    #[test]
    fn test_debouncer_max_wait() {
        // debounce_secs = 1 => max_wait = 5s
        // We'll use a debouncer with 0 debounce to test max_wait reset
        let mut d = Debouncer::new(0);
        d.notify_event();
        std::thread::sleep(Duration::from_millis(1));
        assert!(d.should_trigger());
        d.reset();
        // After reset, no trigger
        assert!(!d.should_trigger());
    }

    // -- Display / Error trait tests --

    #[test]
    fn test_watch_error_display() {
        let err = WatchError::Index(IndexError::IndexNotFound);
        let msg = format!("{err}");
        assert!(msg.contains("Index error"));
    }

    #[test]
    fn test_watch_error_io_display() {
        let io_err = std::io::Error::other("test");
        let err = WatchError::Io(io_err);
        let msg = format!("{err}");
        assert!(msg.contains("IO error"));
    }

    #[test]
    fn test_watch_error_notify_display() {
        let err = WatchError::Notify(notify::Error::generic("watcher failed"));
        let msg = format!("{err}");
        assert!(msg.contains("File watcher error"));
    }

    #[test]
    fn test_from_index_error() {
        let idx_err = IndexError::IndexNotFound;
        let watch_err: WatchError = idx_err.into();
        assert!(matches!(
            watch_err,
            WatchError::Index(IndexError::IndexNotFound)
        ));
    }

    #[test]
    fn test_from_io_error() {
        let io_err = std::io::Error::other("test");
        let watch_err: WatchError = io_err.into();
        assert!(matches!(watch_err, WatchError::Io(_)));
    }

    #[test]
    fn test_from_notify_error() {
        let notify_err = notify::Error::generic("test");
        let watch_err: WatchError = notify_err.into();
        assert!(matches!(watch_err, WatchError::Notify(_)));
    }

    #[test]
    fn test_error_source() {
        let io_err = std::io::Error::other("test");
        let watch_err = WatchError::Io(io_err);
        assert!(std::error::Error::source(&watch_err).is_some());
    }
}

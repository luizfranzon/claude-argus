use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc;
use std::sync::Mutex;
use std::time::Duration;

use argus_application::ports::{FileWatcherPort, WatchCallback, WatchError, WatchHandle};
use notify_debouncer_mini::notify::RecursiveMode;
use notify_debouncer_mini::{new_debouncer, Debouncer};

/// Debounce window before `on_change` fires. Long enough to coalesce a whole
/// `claude` edit burst (many small writes) into one refresh, short enough
/// that the File Explorer/GitPanel still feel live.
const DEBOUNCE: Duration = Duration::from_millis(400);

type RecommendedDebouncer = Debouncer<notify_debouncer_mini::notify::RecommendedWatcher>;

/// `FileWatcherPort` backed by `notify-debouncer-mini`. Each `watch()` call
/// owns one OS watcher + one background thread draining its debounced event
/// channel into `on_change`; `unwatch()` drops the watcher, which stops the
/// OS subscription and lets that thread exit once the channel closes.
pub struct NotifyWatcherAdapter {
    watchers: Mutex<HashMap<WatchHandle, RecommendedDebouncer>>,
}

impl NotifyWatcherAdapter {
    pub fn new() -> Self {
        Self {
            watchers: Mutex::new(HashMap::new()),
        }
    }
}

impl Default for NotifyWatcherAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl FileWatcherPort for NotifyWatcherAdapter {
    fn watch(&self, root: PathBuf, on_change: WatchCallback) -> Result<WatchHandle, WatchError> {
        let (tx, rx) = mpsc::channel();
        let mut debouncer =
            new_debouncer(DEBOUNCE, tx).map_err(|e| WatchError::Failed(e.to_string()))?;
        debouncer
            .watcher()
            .watch(&root, RecursiveMode::Recursive)
            .map_err(|e| WatchError::Failed(e.to_string()))?;

        std::thread::spawn(move || {
            // One `on_change` per debounced batch is enough — the caller
            // just re-lists/re-statuses, it doesn't need per-file detail.
            while rx.recv().is_ok() {
                on_change();
            }
        });

        let handle = WatchHandle::new();
        self.watchers.lock().unwrap().insert(handle, debouncer);
        Ok(handle)
    }

    fn unwatch(&self, handle: WatchHandle) {
        self.watchers.lock().unwrap().remove(&handle);
    }
}

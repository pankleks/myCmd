use notify::{Config, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};

// Below the frontend's 180 ms debounce, so a continuous burst still refreshes
// on its 1 s max delay (src/utils/directoryRefresh.ts) instead of every tick.
const COALESCE: Duration = Duration::from_millis(100);

/// Runs `emit` once, `COALESCE` after the first of a burst of calls; a call
/// arriving after the flag is cleared schedules a new one, so the last change
/// is always followed by an emit.
fn coalesce(pending: &Arc<AtomicBool>, emit: impl FnOnce() + Send + 'static) {
    if pending.swap(true, Ordering::SeqCst) {
        return;
    }
    let pending = pending.clone();
    std::thread::spawn(move || {
        std::thread::sleep(COALESCE);
        pending.store(false, Ordering::SeqCst);
        emit();
    });
}
use tauri::{AppHandle, Emitter, Manager};

#[derive(Default)]
pub struct DirectoryWatchers {
    watchers: Mutex<HashMap<PathBuf, RecommendedWatcher>>,
    latest: AtomicU64,
}

// Async so a stalled network mount blocks a worker thread, not the UI thread.
#[tauri::command]
pub async fn watch_directories(
    app: AppHandle,
    paths: Vec<String>,
) -> std::result::Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || watch(&app, paths))
        .await
        .map_err(|error| error.to_string())?
}

fn watch(app: &AppHandle, paths: Vec<String>) -> std::result::Result<(), String> {
    let state = app.state::<DirectoryWatchers>();
    let request = state.latest.fetch_add(1, Ordering::SeqCst) + 1;
    let mut requested = HashSet::new();
    for path in paths {
        if let Ok(path) = fs::canonicalize(path) {
            if path.is_dir() {
                requested.insert(path);
            }
        }
    }

    let mut watchers = state
        .watchers
        .lock()
        .map_err(|_| "Directory watcher state is unavailable".to_string())?;
    // Calls now run in parallel: an older request must not replace a newer one.
    if state.latest.load(Ordering::SeqCst) != request {
        return Ok(());
    }
    watchers.retain(|path, _| requested.contains(path));

    let mut first_error = None;
    for path in requested {
        if watchers.contains_key(&path) {
            continue;
        }

        let event_path = path.to_string_lossy().into_owned();
        let event_app = app.clone();
        let pending = Arc::new(AtomicBool::new(false));
        let watcher = RecommendedWatcher::new(
            move |event: notify::Result<notify::Event>| match event {
                Ok(event) if !matches!(event.kind, EventKind::Access(_)) => {
                    let (app, path) = (event_app.clone(), event_path.clone());
                    coalesce(&pending, move || {
                        let _ = app.emit("filesystem-changed", &path);
                    });
                }
                Err(error) => eprintln!("Filesystem watcher error: {error}"),
                _ => {}
            },
            Config::default(),
        )
        .map_err(|error| format!("Could not create filesystem watcher: {error}"))
        .and_then(|mut watcher| {
            watcher
                .watch(&path, RecursiveMode::NonRecursive)
                .map_err(|error| format!("Could not watch {}: {error}", path.display()))
                .map(|()| watcher)
        });
        // One failing panel (e.g. inotify limit) must not leave the other unwatched.
        match watcher {
            Ok(watcher) => {
                watchers.insert(path, watcher);
            }
            Err(error) => {
                first_error.get_or_insert(error);
            }
        }
    }
    first_error.map_or(Ok(()), Err)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coalesces_a_burst_into_one_trailing_emit() {
        let pending = Arc::new(AtomicBool::new(false));
        let emits = Arc::new(AtomicU64::new(0));
        let count = || {
            let emits = emits.clone();
            move || {
                emits.fetch_add(1, Ordering::SeqCst);
            }
        };
        for _ in 0..1000 {
            coalesce(&pending, count());
        }
        std::thread::sleep(COALESCE * 3);
        assert_eq!(emits.load(Ordering::SeqCst), 1);
        // A later change gets its own emit.
        coalesce(&pending, count());
        std::thread::sleep(COALESCE * 3);
        assert_eq!(emits.load(Ordering::SeqCst), 2);
    }
}

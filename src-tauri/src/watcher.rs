use notify::{Config, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex,
    },
};
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
        let watcher = RecommendedWatcher::new(
            move |event: notify::Result<notify::Event>| match event {
                Ok(event) if !matches!(event.kind, EventKind::Access(_)) => {
                    let _ = event_app.emit("filesystem-changed", &event_path);
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

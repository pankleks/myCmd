use notify::{Config, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::PathBuf,
    sync::Mutex,
};
use tauri::{AppHandle, Emitter, State};

#[derive(Default)]
pub struct DirectoryWatchers {
    watchers: Mutex<HashMap<PathBuf, RecommendedWatcher>>,
}

#[tauri::command]
pub fn watch_directories(
    app: AppHandle,
    state: State<'_, DirectoryWatchers>,
    paths: Vec<String>,
) -> std::result::Result<(), String> {
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
    watchers.retain(|path, _| requested.contains(path));

    for path in requested {
        if watchers.contains_key(&path) {
            continue;
        }

        let event_path = path.to_string_lossy().into_owned();
        let event_app = app.clone();
        let mut watcher = RecommendedWatcher::new(
            move |event: notify::Result<notify::Event>| match event {
                Ok(event) if !matches!(event.kind, EventKind::Access(_)) => {
                    let _ = event_app.emit("filesystem-changed", &event_path);
                }
                Err(error) => eprintln!("Filesystem watcher error: {error}"),
                _ => {}
            },
            Config::default(),
        )
        .map_err(|error| format!("Could not create filesystem watcher: {error}"))?;
        watcher
            .watch(&path, RecursiveMode::NonRecursive)
            .map_err(|error| format!("Could not watch {}: {error}", path.display()))?;
        watchers.insert(path, watcher);
    }
    Ok(())
}

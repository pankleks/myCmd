use crate::{
    cancellation::Registry,
    error::{FsError, Result},
    filesystem::{self, Entry},
};
use serde::Serialize;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter, State};

#[derive(Default)]
pub struct SearchManager(pub Registry);
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
    pub root: String,
    pub entries: Vec<Entry>,
    pub skipped: usize,
    pub limited: bool,
    pub cancelled: bool,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Batch {
    request_id: String,
    entries: Vec<Entry>,
}

fn search(
    root: &Path,
    pattern: &str,
    check: impl Fn() -> Result<()>,
    mut batch: impl FnMut(Vec<Entry>),
) -> Result<SearchResult> {
    let root = filesystem::absolute(root)?;
    if !root.is_dir() {
        return Err(FsError::new(
            "invalid_path",
            "Search folder must be a directory",
        ));
    }
    let pattern = glob::Pattern::new(if pattern == "*.*" { "*" } else { pattern })
        .map_err(|e| FsError::new("invalid_pattern", e.to_string()))?;
    let options = glob::MatchOptions {
        case_sensitive: false,
        require_literal_separator: true,
        require_literal_leading_dot: false,
    };
    let mut result = SearchResult {
        root: filesystem::text(&root),
        entries: vec![],
        skipped: 0,
        limited: false,
        cancelled: false,
    };
    let mut stack: Vec<PathBuf> = vec![root];
    let mut pending = Vec::new();
    let mut last_batch = std::time::Instant::now();
    while let Some(directory) = stack.pop() {
        if check().is_err() {
            result.cancelled = true;
            break;
        }
        let entries = match std::fs::read_dir(&directory) {
            Ok(entries) => entries,
            Err(_) => {
                result.skipped += 1;
                continue;
            }
        };
        for item in entries {
            if check().is_err() {
                result.cancelled = true;
                break;
            }
            let item = match item {
                Ok(item) => item,
                Err(_) => {
                    result.skipped += 1;
                    continue;
                }
            };
            let metadata = match std::fs::symlink_metadata(item.path()) {
                Ok(metadata) => metadata,
                Err(_) => {
                    result.skipped += 1;
                    continue;
                }
            };
            if metadata.is_dir() && !metadata.file_type().is_symlink() {
                stack.push(item.path());
            }
            if pattern.matches_with(&item.file_name().to_string_lossy(), options) {
                match filesystem::entry(&item.path()) {
                    Ok(entry) => {
                        pending.push(entry.clone());
                        result.entries.push(entry);
                    }
                    Err(_) => result.skipped += 1,
                }
            }
            if pending.len() >= 128
                || (!pending.is_empty() && last_batch.elapsed().as_millis() >= 200)
            {
                batch(std::mem::take(&mut pending));
                last_batch = std::time::Instant::now();
            }
            if result.entries.len() >= 50_000 {
                result.limited = true;
                break;
            }
        }
        if result.cancelled || result.limited {
            break;
        }
    }
    if !pending.is_empty() {
        batch(pending);
    }
    Ok(result)
}

#[tauri::command]
pub async fn search_files(
    app: AppHandle,
    manager: State<'_, SearchManager>,
    root: String,
    pattern: String,
    request_id: String,
) -> Result<SearchResult> {
    let request = manager.0.register(request_id.clone())?;
    tauri::async_runtime::spawn_blocking(move || {
        search(
            Path::new(&root),
            &pattern,
            || request.check(),
            |entries| {
                let _ = app.emit(
                    "search-results",
                    Batch {
                        request_id: request_id.clone(),
                        entries,
                    },
                );
            },
        )
    })
    .await
    .map_err(|e| FsError::new("io_error", e.to_string()))?
}
#[tauri::command]
pub fn cancel_search(manager: State<'_, SearchManager>, request_id: String) -> Result<()> {
    manager.0.cancel(&[request_id])
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn searches_names_recursively_without_inspecting_archives() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::create_dir(temp.path().join("tools.exe")).unwrap();
        std::fs::write(temp.path().join("tools.exe/app.EXE"), b"file").unwrap();
        std::fs::write(temp.path().join("test.zip"), b"not a zip").unwrap();
        let result = search(temp.path(), "*.exe", || Ok(()), |_| {}).unwrap();
        assert_eq!(result.entries.len(), 2);
        assert!(!result.cancelled);
        let all = search(temp.path(), "*.*", || Ok(()), |_| {}).unwrap();
        assert_eq!(all.entries.len(), 3);
        assert!(
            search(
                temp.path(),
                "*",
                || Err(FsError::new("cancelled", "cancelled")),
                |_| {}
            )
            .unwrap()
            .cancelled
        );
    }
}

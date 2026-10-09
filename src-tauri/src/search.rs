use crate::{
    cancellation::Registry,
    error::{FsError, Result},
    filesystem::{self, Entry},
};
use serde::Serialize;
use std::{
    io::Read,
    path::{Path, PathBuf},
};
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

fn contains_text(path: &Path, needle: &str, check: &impl Fn() -> Result<()>) -> Result<bool> {
    let mut file = std::fs::File::open(path).map_err(|e| FsError::io(e, path))?;
    let mut buffer = Vec::with_capacity(64 * 1024 + 4);
    let mut first = true;
    let mut utf16 = None;
    let mut tail = String::new();
    let mut found = false;
    loop {
        check()?;
        let read = file
            .by_ref()
            .take(64 * 1024)
            .read_to_end(&mut buffer)
            .map_err(|e| FsError::io(e, path))?;
        check()?;
        let eof = read == 0;
        if first {
            utf16 = filesystem::utf16_encoding(&buffer);
            if buffer.starts_with(&[0xff, 0xfe]) || buffer.starts_with(&[0xfe, 0xff]) {
                buffer.drain(..2);
            }
            first = false;
        }
        let (text, consumed) = if let Some(little_endian) = utf16 {
            if eof && buffer.len() % 2 != 0 {
                return Ok(false);
            }
            let mut units: Vec<u16> = buffer
                .as_chunks::<2>()
                .0
                .iter()
                .map(|p| {
                    let bytes = [p[0], p[1]];
                    if little_endian {
                        u16::from_le_bytes(bytes)
                    } else {
                        u16::from_be_bytes(bytes)
                    }
                })
                .collect();
            // Keep a split surrogate pair for the next chunk.
            if !eof && units.last().is_some_and(|u| (0xd800..=0xdbff).contains(u)) {
                units.pop();
            }
            let text = match String::from_utf16(&units) {
                Ok(text) => text,
                Err(_) => return Ok(false),
            };
            (text, units.len() * 2)
        } else {
            let consumed = match std::str::from_utf8(&buffer) {
                Ok(_) => buffer.len(),
                Err(error) if error.error_len().is_none() && !eof => error.valid_up_to(),
                Err(_) => return Ok(false),
            };
            (
                std::str::from_utf8(&buffer[..consumed]).unwrap().to_owned(),
                consumed,
            )
        };
        if text.chars().any(|c| {
            c.is_control() && !matches!(c, '\n' | '\r' | '\t' | '\u{1b}' | '\u{8}' | '\u{c}')
        }) {
            return Ok(false);
        }
        if !found {
            tail.push_str(&text.to_lowercase());
            found = tail.contains(needle);
            let mut start = tail.len().saturating_sub(needle.len());
            while !tail.is_char_boundary(start) {
                start -= 1;
            }
            tail.drain(..start);
        }
        buffer.drain(..consumed);
        if eof {
            return Ok(found);
        }
    }
}

fn search(
    root: &Path,
    pattern: &str,
    text: Option<&str>,
    check: impl Fn() -> Result<()>,
    mut batch: impl FnMut(Vec<Entry>),
) -> Result<SearchResult> {
    if text == Some("") {
        return Err(FsError::new(
            "invalid_pattern",
            "Search text must not be empty",
        ));
    }
    let needle = text.map(str::to_lowercase);
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
                if let Some(needle) = &needle {
                    // Only regular files: do not follow links or read devices/pipes.
                    if !metadata.is_file() {
                        continue;
                    }
                    match contains_text(&item.path(), needle, &check) {
                        Ok(true) => {}
                        Ok(false) => continue,
                        Err(_) => {
                            if check().is_err() {
                                result.cancelled = true;
                                break;
                            }
                            result.skipped += 1;
                            continue;
                        }
                    }
                }
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
    text: Option<String>,
    request_id: String,
) -> Result<SearchResult> {
    let request = manager.0.register(request_id.clone())?;
    tauri::async_runtime::spawn_blocking(move || {
        search(
            Path::new(&root),
            &pattern,
            text.as_deref(),
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
    fn searches_text_recursively_and_skips_binary_files_and_directories() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::create_dir(temp.path().join("nested")).unwrap();
        for name in ["README", "nested/code.rs", "data.json", "unknown.custom"] {
            std::fs::write(temp.path().join(name), "A literal NÉEDLE.* here").unwrap();
        }
        std::fs::write(temp.path().join("other.txt"), "not a match").unwrap();
        std::fs::write(temp.path().join("binary.txt"), b"N\xc3\xa9edle.*\0binary").unwrap();
        std::fs::write(temp.path().join("invalid.txt"), b"needle.*\xff").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(temp.path().join("README"), temp.path().join("link.txt"))
            .unwrap();
        let result = search(temp.path(), "*.*", Some("néedle.*"), || Ok(()), |_| {}).unwrap();
        assert_eq!(result.entries.len(), 4);
        assert!(result.entries.iter().all(|e| e.r#type == "file"));
        let filtered = search(temp.path(), "*.rs", Some("néedle.*"), || Ok(()), |_| {}).unwrap();
        assert_eq!(filtered.entries.len(), 1);
        assert_eq!(filtered.entries[0].name, "code.rs");
        assert!(search(temp.path(), "*", Some(""), || Ok(()), |_| {}).is_err());
    }

    #[test]
    fn handles_chunk_boundaries_utf16_and_files_larger_than_preview_limit() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("text");
        let mut utf8 = vec![b'x'; 64 * 1024 - 1];
        utf8.extend_from_slice("Ą.CASE".as_bytes());
        std::fs::write(&path, utf8).unwrap();
        assert!(contains_text(&path, "ą.case", &|| Ok(())).unwrap());
        for little_endian in [true, false] {
            let mut bytes = if little_endian {
                vec![0xff, 0xfe]
            } else {
                vec![0xfe, 0xff]
            };
            for unit in ("x".repeat(32765) + "A😀B").encode_utf16() {
                bytes.extend_from_slice(&if little_endian {
                    unit.to_le_bytes()
                } else {
                    unit.to_be_bytes()
                });
            }
            std::fs::write(&path, bytes).unwrap();
            assert!(contains_text(&path, "a😀b", &|| Ok(())).unwrap());
            let bytes: Vec<u8> = "BOM-less NEEDLE"
                .encode_utf16()
                .flat_map(|u| {
                    if little_endian {
                        u.to_le_bytes()
                    } else {
                        u.to_be_bytes()
                    }
                })
                .collect();
            std::fs::write(&path, bytes).unwrap();
            assert!(contains_text(&path, "needle", &|| Ok(())).unwrap());
        }
        let mut large = vec![b'x'; 9 * 1024 * 1024];
        large.extend_from_slice(b"NEEDLE");
        std::fs::write(&path, large).unwrap();
        assert!(contains_text(&path, "needle", &|| Ok(())).unwrap());
        let checks = std::cell::Cell::new(0);
        let result = search(
            temp.path(),
            "*",
            Some("needle"),
            || {
                checks.set(checks.get() + 1);
                if checks.get() > 6 {
                    Err(FsError::new("cancelled", "cancelled"))
                } else {
                    Ok(())
                }
            },
            |_| {},
        )
        .unwrap();
        assert!(result.cancelled);
        assert!(result.entries.is_empty());
        assert!(checks.get() < 12);
    }

    #[test]
    fn searches_names_recursively_without_inspecting_archives() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::create_dir(temp.path().join("tools.exe")).unwrap();
        std::fs::write(temp.path().join("tools.exe/app.EXE"), b"file").unwrap();
        std::fs::write(temp.path().join("test.zip"), b"not a zip").unwrap();
        let result = search(temp.path(), "*.exe", None, || Ok(()), |_| {}).unwrap();
        assert_eq!(result.entries.len(), 2);
        assert!(!result.cancelled);
        let all = search(temp.path(), "*.*", None, || Ok(()), |_| {}).unwrap();
        assert_eq!(all.entries.len(), 3);
        assert!(
            search(
                temp.path(),
                "*",
                None,
                || Err(FsError::new("cancelled", "cancelled")),
                |_| {}
            )
            .unwrap()
            .cancelled
        );
    }
}

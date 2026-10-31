use crate::error::{FsError, Result};
use clipboard_rs::{Clipboard, ClipboardContent, ClipboardContext};
use std::{
    path::Path,
    sync::{Arc, Mutex},
};

// X11 serves clipboard requests from the owning context. Keep it alive for the
// lifetime of the application, rather than dropping it after each command.
#[derive(Clone, Default)]
pub struct FileClipboard(Arc<Mutex<Option<ClipboardContext>>>);

#[derive(Default, serde::Serialize)]
pub struct ClipboardFiles {
    pub paths: Vec<String>,
    pub cut: bool,
}

#[cfg(any(windows, test))]
fn windows_cut(data: &[u8]) -> bool {
    data.get(..4) == Some(&2u32.to_le_bytes())
}

#[cfg(windows)]
fn clipboard_sequence() -> u32 {
    #[link(name = "user32")]
    extern "system" {
        fn GetClipboardSequenceNumber() -> u32;
    }
    // This read-only API has no arguments or pointer preconditions.
    unsafe { GetClipboardSequenceNumber() }
}

#[cfg(any(target_os = "linux", test))]
fn gnome_files(data: &[u8]) -> Result<(Vec<String>, bool)> {
    let text = std::str::from_utf8(data).map_err(clipboard_error)?;
    let mut lines = text.lines();
    let cut = lines.next() == Some("cut");
    Ok((
        lines
            .filter(|line| !line.is_empty())
            .map(str::to_owned)
            .collect(),
        cut,
    ))
}

fn existing_paths(files: Vec<String>) -> Result<Vec<String>> {
    let mut paths = Vec::new();
    for file in files {
        let path = local_path(&file)?;
        match std::fs::symlink_metadata(&path) {
            Ok(_) => {
                if !paths.contains(&path) {
                    paths.push(path);
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(FsError::io(e, Path::new(&path))),
        }
    }
    Ok(paths)
}

fn clipboard_error(error: impl std::fmt::Display) -> FsError {
    FsError::new("clipboard_error", error.to_string())
}

impl FileClipboard {
    fn with<T>(&self, action: impl FnOnce(&ClipboardContext) -> Result<T>) -> Result<T> {
        let mut context = self.0.lock().map_err(clipboard_error)?;
        if context.is_none() {
            *context = Some(ClipboardContext::new().map_err(clipboard_error)?);
        }
        action(context.as_ref().unwrap())
    }
}

fn local_path(value: &str) -> Result<String> {
    let path = if value.starts_with("file:") {
        url::Url::parse(value)
            .ok()
            .filter(|url| url.host_str().is_none_or(|host| host == "localhost"))
            .and_then(|url| url.to_file_path().ok())
            .ok_or_else(|| {
                FsError::new(
                    "invalid_path",
                    "Clipboard contains an invalid or non-local file URL",
                )
            })?
    } else {
        Path::new(value).to_path_buf()
    };
    if !path.is_absolute() || path.to_string_lossy().contains('\0') {
        return Err(FsError::new(
            "invalid_path",
            "Clipboard file paths must be absolute",
        ));
    }
    path.into_os_string()
        .into_string()
        .map_err(|_| FsError::new("invalid_path", "Clipboard file path is not valid Unicode"))
}

impl FileClipboard {
    pub fn copy_text(&self, text: String) -> Result<()> {
        self.with(|context| context.set_text(text).map_err(clipboard_error))
    }

    pub fn copy_files(&self, paths: Vec<String>, cut: bool) -> Result<()> {
        if paths.is_empty() {
            return Err(FsError::new("invalid_path", "No files selected"));
        }
        let paths = paths
            .iter()
            .map(|path| local_path(path))
            .collect::<Result<Vec<_>>>()?;
        for path in &paths {
            std::fs::symlink_metadata(path).map_err(|error| FsError::io(error, Path::new(path)))?;
        }
        self.with(|context| {
            #[cfg(target_os = "linux")]
            {
                let uris = paths
                    .iter()
                    .map(|path| {
                        url::Url::from_file_path(path)
                            .map(|url| url.to_string())
                            .map_err(|_| {
                                FsError::new("invalid_path", "Cannot encode clipboard file URL")
                            })
                    })
                    .collect::<Result<Vec<_>>>()?;
                // Publish the URI list and both GNOME/KDE copy or cut markers.
                // Encode URLs ourselves: literal %, #, spaces and newlines in
                // filenames must not change the meaning of the URI list.
                context
                    .set(vec![
                        ClipboardContent::Other(
                            "text/uri-list".into(),
                            uris.join("\r\n").into_bytes(),
                        ),
                        ClipboardContent::Other(
                            "x-special/gnome-copied-files".into(),
                            format!("{}\n{}", if cut { "cut" } else { "copy" }, uris.join("\n"))
                                .into_bytes(),
                        ),
                        ClipboardContent::Other(
                            "application/x-kde-cutselection".into(),
                            if cut { b"1".to_vec() } else { b"0".to_vec() },
                        ),
                    ])
                    .map_err(clipboard_error)
            }
            #[cfg(windows)]
            {
                context
                    .set(vec![
                        ClipboardContent::Files(paths.clone()),
                        ClipboardContent::Other(
                            "Preferred DropEffect".into(),
                            (if cut { 2u32 } else { 1u32 }).to_le_bytes().to_vec(),
                        ),
                    ])
                    .map_err(clipboard_error)?;
                // The backend's multi-format writer can silently skip a failed format.
                if context.get_files().map_err(clipboard_error)? != paths
                    || context
                        .get_buffer("Preferred DropEffect")
                        .map_err(clipboard_error)?
                        != (if cut { 2u32 } else { 1u32 }).to_le_bytes()
                {
                    return Err(FsError::new(
                        "clipboard_error",
                        "Could not publish clipboard files and transfer action",
                    ));
                }
                Ok(())
            }
            #[cfg(target_os = "macos")]
            context
                .set(vec![
                    ClipboardContent::Files(paths),
                    ClipboardContent::Other("org.mycmd.cut".into(), vec![u8::from(cut)]),
                ])
                .map_err(clipboard_error)
        })
    }

    pub fn read_files(&self) -> Result<ClipboardFiles> {
        self.with(|context| {
            #[cfg(windows)]
            let sequence = clipboard_sequence();
            let formats = context.available_formats().map_err(clipboard_error)?;
            #[cfg(target_os = "linux")]
            let (files, cut) = if formats
                .iter()
                .any(|format| format == "x-special/gnome-copied-files")
            {
                let data = context
                    .get_buffer("x-special/gnome-copied-files")
                    .map_err(clipboard_error)?;
                gnome_files(&data)?
            } else if formats.iter().any(|format| format == "text/uri-list") {
                let cut = if formats
                    .iter()
                    .any(|format| format == "application/x-kde-cutselection")
                {
                    context
                        .get_buffer("application/x-kde-cutselection")
                        .map_err(clipboard_error)?
                        == b"1"
                } else {
                    false
                };
                (context.get_files().map_err(clipboard_error)?, cut)
            } else {
                return Ok(ClipboardFiles::default());
            };
            #[cfg(not(target_os = "linux"))]
            let (files, cut) = {
                if !context.has(clipboard_rs::ContentFormat::Files) {
                    return Ok(ClipboardFiles::default());
                }
                #[cfg(windows)]
                let cut = if formats
                    .iter()
                    .any(|format| format == "Preferred DropEffect")
                {
                    windows_cut(
                        &context
                            .get_buffer("Preferred DropEffect")
                            .map_err(clipboard_error)?,
                    )
                } else {
                    false
                };
                #[cfg(target_os = "macos")]
                let cut = if formats.iter().any(|format| format == "org.mycmd.cut") {
                    context
                        .get_buffer("org.mycmd.cut")
                        .map_err(clipboard_error)?
                        == [1]
                } else {
                    false
                };
                (context.get_files().map_err(clipboard_error)?, cut)
            };
            // Never interpret ordinary text as files. Stale cut paths disappear
            // after a successful move; skipped or failed source items remain pasteable.
            let paths = existing_paths(files)?;
            #[cfg(windows)]
            if sequence == 0 || sequence != clipboard_sequence() {
                return Err(FsError::new(
                    "clipboard_error",
                    "Clipboard changed while reading files; try Paste again",
                ));
            }
            Ok(ClipboardFiles { paths, cut })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_only_explicit_windows_move_effects() {
        assert!(windows_cut(&2u32.to_le_bytes()));
        for data in [
            vec![],
            vec![2],
            1u32.to_le_bytes().to_vec(),
            3u32.to_le_bytes().to_vec(),
            4u32.to_le_bytes().to_vec(),
        ] {
            assert!(!windows_cut(&data));
        }
    }

    #[test]
    fn reads_gnome_copy_and_cut_markers_and_ignores_empty_lines() {
        let uri = url::Url::from_file_path(std::env::temp_dir().join("a space # %.txt"))
            .unwrap()
            .to_string();
        let (files, cut) = gnome_files(format!("cut\n{uri}\n\n").as_bytes()).unwrap();
        assert!(cut);
        assert_eq!(files, vec![uri.clone()]);
        assert!(!gnome_files(format!("copy\n{uri}").as_bytes()).unwrap().1);
        assert!(!gnome_files(format!("unknown\n{uri}").as_bytes()).unwrap().1);
        assert!(gnome_files(&[0xff]).is_err());
    }

    #[test]
    fn paste_ignores_moved_sources_but_keeps_remaining_items() {
        let dir = tempfile::tempdir().unwrap();
        let remaining = dir.path().join("remaining");
        let moved = dir.path().join("moved");
        std::fs::write(&remaining, "keep").unwrap();
        let paths = existing_paths(vec![
            remaining.to_str().unwrap().into(),
            moved.to_str().unwrap().into(),
            remaining.to_str().unwrap().into(),
        ])
        .unwrap();
        assert_eq!(paths, vec![remaining.to_str().unwrap()]);
        assert!(existing_paths(vec!["file://remote.example/file".into()]).is_err());
    }

    #[test]
    fn file_urls_round_trip_special_characters() {
        let path = std::env::temp_dir().join("a space # % 日本語\nfile.txt");
        let uri = url::Url::from_file_path(&path).unwrap();
        assert_eq!(local_path(uri.as_str()).unwrap(), path.to_str().unwrap());
    }

    #[test]
    fn rejects_non_local_and_relative_paths() {
        for value in [
            "relative.txt",
            "https://example.com/file",
            "file://remote.example/file",
            "file:///bad%00path",
        ] {
            assert!(local_path(value).is_err(), "{value}");
        }
    }
}

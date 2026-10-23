use crate::error::{FsError, Result};
use clipboard_rs::{Clipboard, ClipboardContext};
use std::{
    path::Path,
    sync::{Arc, Mutex},
};

// X11 serves clipboard requests from the owning context. Keep it alive for the
// lifetime of the application, rather than dropping it after each command.
#[derive(Clone, Default)]
pub struct FileClipboard(Arc<Mutex<Option<ClipboardContext>>>);

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
    pub fn copy_files(&self, paths: Vec<String>) -> Result<()> {
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
                use clipboard_rs::ClipboardContent;
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
                // Publish both the standard URI list and GNOME's copy marker.
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
                            format!("copy\n{}", uris.join("\n")).into_bytes(),
                        ),
                        ClipboardContent::Other(
                            "application/x-kde-cutselection".into(),
                            b"0".to_vec(),
                        ),
                    ])
                    .map_err(clipboard_error)
            }
            #[cfg(not(target_os = "linux"))]
            context.set_files(paths).map_err(clipboard_error)
        })
    }

    pub fn read_files(&self) -> Result<Vec<String>> {
        self.with(|context| {
            let formats = context.available_formats().map_err(clipboard_error)?;
            #[cfg(target_os = "linux")]
            let files = if formats.iter().any(|format| format == "text/uri-list") {
                context.get_files().map_err(clipboard_error)?
            } else if formats
                .iter()
                .any(|format| format == "x-special/gnome-copied-files")
            {
                let data = context
                    .get_buffer("x-special/gnome-copied-files")
                    .map_err(clipboard_error)?;
                let text = String::from_utf8(data).map_err(clipboard_error)?;
                text.lines()
                    .skip(1)
                    .filter(|line| !line.is_empty())
                    .map(str::to_owned)
                    .collect()
            } else {
                return Ok(Vec::new());
            };
            #[cfg(not(target_os = "linux"))]
            let files = {
                let _ = formats;
                if !context.has(clipboard_rs::ContentFormat::Files) {
                    return Ok(Vec::new());
                }
                context.get_files().map_err(clipboard_error)?
            };
            // Never interpret ordinary text as file paths, and never move/delete
            // sources even if another file manager put a cut marker on the clipboard.
            let mut paths = Vec::new();
            for file in files {
                let path = local_path(&file)?;
                if !paths.contains(&path) {
                    paths.push(path);
                }
            }
            Ok(paths)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

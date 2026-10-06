use crate::{
    config::{self, AppConfig},
    error::{FsError, Result},
    filesystem,
    operations::{Manager, Operation, Resolution},
};
use tauri::{AppHandle, State};

pub type DirectorySizing = crate::cancellation::Registry;
#[derive(serde::Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DeleteCounts {
    folders: u64,
    files: u64,
    skipped: u64,
}
fn count_delete_paths(paths: Vec<String>) -> DeleteCounts {
    let mut counts = DeleteCounts::default();
    let mut visited = std::collections::HashSet::new();
    let mut stack: Vec<std::path::PathBuf> =
        paths.into_iter().map(std::path::PathBuf::from).collect();
    while let Some(path) = stack.pop() {
        if std::fs::symlink_metadata(&path).is_ok_and(|metadata| metadata.file_type().is_symlink())
        {
            if visited.insert(path) {
                counts.files += 1;
            }
            continue;
        }
        let path = match filesystem::absolute(&path) {
            Ok(path) => path,
            Err(_) => {
                counts.skipped += 1;
                continue;
            }
        };
        if !visited.insert(path.clone()) {
            continue;
        }
        match std::fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => {
                counts.folders += 1;
                match std::fs::read_dir(&path) {
                    Ok(entries) => {
                        for entry in entries {
                            match entry {
                                Ok(entry) => stack.push(entry.path()),
                                Err(_) => counts.skipped += 1,
                            }
                        }
                    }
                    Err(_) => counts.skipped += 1,
                }
            }
            Ok(_) => counts.files += 1,
            Err(_) => counts.skipped += 1,
        }
    }
    counts
}
#[tauri::command]
pub async fn count_delete_entries(paths: Vec<String>) -> Result<DeleteCounts> {
    tauri::async_runtime::spawn_blocking(move || count_delete_paths(paths))
        .await
        .map_err(|e| FsError::new("io_error", e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn editing_rejects_directories_before_launching_an_application() {
        let directory = tempfile::tempdir().unwrap();
        assert_eq!(
            edit(directory.path(), None).unwrap_err().code,
            "invalid_path"
        );
    }

    #[test]
    fn editor_launch_failure_keeps_the_file() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("file with spaces & symbols.txt");
        std::fs::write(&path, "keep").unwrap();
        let missing_editor = directory.path().join("missing-editor");
        assert_eq!(
            edit(&path, missing_editor.to_str()).unwrap_err().code,
            "not_found"
        );
        assert_eq!(std::fs::read_to_string(path).unwrap(), "keep");
    }

    #[test]
    fn counts_descendants_without_opening_archives_or_double_counting() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::create_dir(temp.path().join("nested")).unwrap();
        std::fs::write(temp.path().join("nested/a.zip"), b"not an archive").unwrap();
        std::fs::write(temp.path().join("b.txt"), b"file").unwrap();
        let counts = count_delete_paths(vec![
            filesystem::text(temp.path()),
            filesystem::text(&temp.path().join("nested")),
        ]);
        assert_eq!(counts.folders, 2);
        assert_eq!(counts.files, 2);
        assert_eq!(counts.skipped, 0);
    }
}
#[tauri::command]
pub async fn list_directory(path: String) -> Result<filesystem::Listing> {
    tauri::async_runtime::spawn_blocking(move || filesystem::list(std::path::Path::new(&path)))
        .await
        .map_err(|e| FsError::new("io_error", e.to_string()))?
}
#[tauri::command]
pub async fn read_text_preview(path: String) -> Result<String> {
    tauri::async_runtime::spawn_blocking(move || {
        filesystem::read_text_preview(std::path::Path::new(&path))
    })
    .await
    .map_err(|e| FsError::new("io_error", e.to_string()))?
}
#[tauri::command]
pub async fn read_pdf_preview(path: String) -> Result<tauri::ipc::Response> {
    tauri::async_runtime::spawn_blocking(move || {
        filesystem::read_pdf_preview(std::path::Path::new(&path)).map(tauri::ipc::Response::new)
    })
    .await
    .map_err(|e| FsError::new("io_error", e.to_string()))?
}
#[tauri::command]
pub async fn read_image_preview(path: String) -> Result<String> {
    tauri::async_runtime::spawn_blocking(move || {
        filesystem::read_image_preview(std::path::Path::new(&path))
    })
    .await
    .map_err(|e| FsError::new("io_error", e.to_string()))?
}
#[tauri::command]
pub async fn read_markdown_image(
    markdown_path: String,
    source: String,
) -> Result<filesystem::PreviewImage> {
    tauri::async_runtime::spawn_blocking(move || {
        filesystem::read_markdown_image(std::path::Path::new(&markdown_path), &source)
    })
    .await
    .map_err(|e| FsError::new("io_error", e.to_string()))?
}
#[tauri::command]
pub async fn list_roots() -> Result<Vec<filesystem::Root>> {
    tauri::async_runtime::spawn_blocking(filesystem::roots)
        .await
        .map_err(|e| FsError::new("io_error", e.to_string()))
}
#[tauri::command]
pub async fn open_file(path: String) -> Result<()> {
    tauri::async_runtime::spawn_blocking(move || {
        let p = filesystem::absolute(std::path::Path::new(&path))?;
        open::that_detached(&p).map_err(|e| FsError::io(e, &p))
    })
    .await
    .map_err(|e| FsError::new("io_error", e.to_string()))?
}
#[tauri::command]
pub fn load_config() -> Result<AppConfig> {
    Ok(config::load())
}

fn edit(path: &std::path::Path, editor: Option<&str>) -> Result<()> {
    let path = filesystem::absolute(path)?;
    if !path.is_file() {
        return Err(FsError::new("invalid_path", "Only files can be edited"));
    }
    if let Some(editor) = editor.map(str::trim).filter(|editor| !editor.is_empty()) {
        let mut child = std::process::Command::new(editor)
            .arg(&path)
            .current_dir(path.parent().unwrap())
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .map_err(|error| FsError::io(error, std::path::Path::new(editor)))?;
        std::thread::spawn(move || {
            let _ = child.wait();
        });
        Ok(())
    } else {
        open::that_detached(&path).map_err(|error| FsError::io(error, &path))
    }
}

#[tauri::command]
pub async fn edit_file(path: String, editor: Option<String>) -> Result<()> {
    tauri::async_runtime::spawn_blocking(move || {
        edit(std::path::Path::new(&path), editor.as_deref())
    })
    .await
    .map_err(|error| FsError::new("io_error", error.to_string()))?
}

#[tauri::command]
pub fn save_config(config: AppConfig) -> Result<()> {
    config::save(&config)
}

#[tauri::command]
pub async fn measure_directory(
    path: String,
    request_id: String,
    sizing: State<'_, DirectorySizing>,
) -> Result<u64> {
    let request = sizing.register(request_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        filesystem::directory_size_checked(std::path::Path::new(&path), || request.check())
    })
    .await
    .map_err(|e| FsError::new("io_error", e.to_string()))?
}

#[tauri::command]
pub fn cancel_directory_sizing(
    request_ids: Vec<String>,
    sizing: State<'_, DirectorySizing>,
) -> Result<()> {
    sizing.cancel(&request_ids)
}

#[tauri::command]
pub fn start_operation(app: AppHandle, manager: State<Manager>, operation: Operation) -> String {
    manager.start(app, operation)
}
#[tauri::command]
pub fn cancel_operation(manager: State<Manager>, operation_id: String) {
    manager.cancel(&operation_id);
}
#[tauri::command]
pub fn resolve_conflict(
    manager: State<Manager>,
    operation_id: String,
    resolution: Resolution,
) -> Result<()> {
    manager.resolve(&operation_id, resolution)
}

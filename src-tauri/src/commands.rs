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
    fn preview_routing_preserves_local_reads_and_reports_disconnected_sftp() {
        let manager = crate::sftp::Manager::default();
        let file = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(file.path(), "preview").unwrap();
        assert_eq!(
            with_preview_path(
                &manager,
                None,
                file.path().to_str().unwrap(),
                8,
                filesystem::read_text_preview
            )
            .unwrap(),
            "preview"
        );
        assert_eq!(
            with_preview_path(
                &manager,
                Some("missing"),
                "/remote.txt",
                8,
                filesystem::read_text_preview
            )
            .unwrap_err()
            .code,
            "sftp_disconnected"
        );
    }

    #[cfg(unix)]
    #[test]
    #[ignore = "starts a private local OpenSSH server; requires /usr/sbin/sshd and ssh-keygen"]
    fn remote_preview_reads_and_limits_files() {
        let fixture = crate::sftp::tests::Fixture::new();
        let manager = crate::sftp::Manager::default();
        fixture.connect(&manager);
        let path = fixture.dir.path().join("preview.svg");
        std::fs::write(&path, "<svg/>").unwrap();
        let read = |limit| {
            with_preview_path(
                &manager,
                Some(&fixture.profile.id),
                path.to_str().unwrap(),
                limit,
                filesystem::read_text_preview,
            )
        };
        assert_eq!(read(8).unwrap(), "<svg/>");
        assert_eq!(read(2).unwrap_err().code, "file_too_large");
    }
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

    #[test]
    fn launch_action_routes_scripts_and_ignored_types_case_insensitively() {
        let rules = config::LaunchRules {
            run_in_terminal: vec!["sh".into(), "bash".into(), "zsh".into()],
            ignore: vec!["exe".into()],
        };
        assert_eq!(
            launch_action(std::path::Path::new("/tmp/deploy.SH"), &rules),
            LaunchAction::RunInTerminal
        );
        assert_eq!(
            launch_action(std::path::Path::new("/tmp/app.Exe"), &rules),
            LaunchAction::Ignore
        );
        assert_eq!(
            launch_action(std::path::Path::new("/tmp/notes.txt"), &rules),
            LaunchAction::Open
        );
        assert_eq!(
            launch_action(std::path::Path::new("/tmp/Makefile"), &rules),
            LaunchAction::Open
        );
        assert_eq!(
            launch_action(std::path::Path::new("/tmp/archive.tar.gz"), &rules),
            LaunchAction::Open
        );
        assert_eq!(
            launch_action(
                std::path::Path::new("/tmp/script.sh"),
                &config::LaunchRules::default()
            ),
            LaunchAction::Open
        );
    }
}
#[tauri::command]
pub async fn list_directory(path: String) -> Result<filesystem::Listing> {
    tauri::async_runtime::spawn_blocking(move || filesystem::list(std::path::Path::new(&path)))
        .await
        .map_err(|e| FsError::new("io_error", e.to_string()))?
}
#[tauri::command]
pub async fn read_text_preview(
    path: String,
    connection_id: Option<String>,
    state: State<'_, std::sync::Arc<crate::sftp::Manager>>,
) -> Result<String> {
    let manager = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        with_preview_path(
            &manager,
            connection_id.as_deref(),
            &path,
            8 * 1024 * 1024,
            filesystem::read_text_preview,
        )
    })
    .await
    .map_err(|e| FsError::new("io_error", e.to_string()))?
}
#[tauri::command]
pub async fn read_pdf_preview(
    path: String,
    connection_id: Option<String>,
    state: State<'_, std::sync::Arc<crate::sftp::Manager>>,
) -> Result<tauri::ipc::Response> {
    let manager = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        with_preview_path(
            &manager,
            connection_id.as_deref(),
            &path,
            64 * 1024 * 1024,
            filesystem::read_pdf_preview,
        )
        .map(tauri::ipc::Response::new)
    })
    .await
    .map_err(|e| FsError::new("io_error", e.to_string()))?
}
#[tauri::command]
pub async fn read_image_preview(
    path: String,
    connection_id: Option<String>,
    state: State<'_, std::sync::Arc<crate::sftp::Manager>>,
) -> Result<String> {
    let manager = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        with_preview_path(
            &manager,
            connection_id.as_deref(),
            &path,
            4 * 1024 * 1024,
            filesystem::read_image_preview,
        )
    })
    .await
    .map_err(|e| FsError::new("io_error", e.to_string()))?
}
fn with_preview_path<T>(
    manager: &crate::sftp::Manager,
    connection_id: Option<&str>,
    path: &str,
    limit: u64,
    read: impl FnOnce(&std::path::Path) -> Result<T>,
) -> Result<T> {
    use std::io::Read;
    let Some(id) = connection_id else {
        return read(std::path::Path::new(path));
    };
    crate::sftp::remote_path(path)?;
    let connection = manager.get(id)?;
    let connection = connection
        .lock()
        .map_err(|e| FsError::new("sftp_error", e.to_string()))?;
    let mut file = connection
        .sftp
        .open(std::path::Path::new(path))
        .map_err(crate::sftp::error)?;
    let stat = file.stat().map_err(crate::sftp::error)?;
    if !stat.is_file() {
        return Err(FsError::new("not_a_file", "Only files can be previewed"));
    }
    if stat.size.is_some_and(|size| size > limit) {
        return Err(FsError::new(
            "file_too_large",
            "File exceeds the preview size limit",
        ));
    }
    let mut temporary = tempfile::Builder::new()
        .suffix(&format!(
            ".{}",
            std::path::Path::new(path)
                .extension()
                .and_then(|s| s.to_str())
                .unwrap_or("")
        ))
        .tempfile()
        .map_err(|e| FsError::new("io_error", e.to_string()))?;
    let copied = std::io::copy(&mut file.take(limit + 1), &mut temporary)
        .map_err(|e| FsError::new("sftp_error", e.to_string()))?;
    if copied > limit {
        return Err(FsError::new(
            "file_too_large",
            "File exceeds the preview size limit",
        ));
    }
    drop(connection);
    read(temporary.path())
}
#[tauri::command]
pub async fn read_markdown_image(
    markdown_path: String,
    source: String,
    connection_id: Option<String>,
    state: State<'_, std::sync::Arc<crate::sftp::Manager>>,
) -> Result<filesystem::PreviewImage> {
    let manager = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        if let Some(id) = connection_id {
            let source = source.split(['?', '#']).next().unwrap_or_default().trim();
            let source = filesystem::decode_uri_path(source)
                .ok_or_else(|| FsError::new("invalid_image_path", "Invalid image path"))?;
            if source.is_empty()
                || source.starts_with('/')
                || source.contains('\\')
                || filesystem::has_uri_scheme(&source)
            {
                return Err(FsError::new(
                    "invalid_image_path",
                    "Only relative remote images can be previewed",
                ));
            }
            let path = format!("{}/{}", crate::sftp::parent(&markdown_path), source);
            return with_preview_path(&manager, Some(&id), &path, 4 * 1024 * 1024, |temporary| {
                filesystem::read_markdown_image(
                    temporary,
                    temporary.file_name().unwrap().to_str().unwrap(),
                )
            });
        }
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
        if p.is_file() {
            match launch_action(&p, &config::load().launch.for_current_os()) {
                LaunchAction::Ignore => return Ok(()),
                LaunchAction::RunInTerminal => return launch_in_terminal(&p),
                LaunchAction::Open => {}
            }
        }
        open::that_detached(&p).map_err(|e| FsError::io(e, &p))
    })
    .await
    .map_err(|e| FsError::new("io_error", e.to_string()))?
}

#[derive(Debug, PartialEq)]
enum LaunchAction {
    Ignore,
    RunInTerminal,
    Open,
}

fn launch_action(path: &std::path::Path, rules: &config::LaunchRules) -> LaunchAction {
    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_lowercase)
        .unwrap_or_default();
    if extension.is_empty() {
        return LaunchAction::Open;
    }
    if rules.run_in_terminal.iter().any(|rule| rule == &extension) {
        LaunchAction::RunInTerminal
    } else if rules.ignore.iter().any(|rule| rule == &extension) {
        LaunchAction::Ignore
    } else {
        LaunchAction::Open
    }
}

#[cfg(target_os = "linux")]
fn launch_in_terminal(path: &std::path::Path) -> Result<()> {
    crate::terminal::run_script(path)
}

// ponytail: script terminals are Linux-only; other systems keep the OS file association.
#[cfg(not(target_os = "linux"))]
fn launch_in_terminal(path: &std::path::Path) -> Result<()> {
    open::that_detached(path).map_err(|e| FsError::io(e, path))
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

use crate::{
    config::{self, AppConfig},
    error::{blocking, FsError, Result},
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
fn count_delete_paths(paths: Vec<String>, check: impl Fn() -> Result<()>) -> Result<DeleteCounts> {
    let mut counts = DeleteCounts::default();
    // Resolve only the selection; a selected symlink is one item and is not followed.
    let mut roots = Vec::new();
    for path in paths.into_iter().map(std::path::PathBuf::from) {
        match std::fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.is_symlink() => roots.push(path),
            _ => match filesystem::absolute(&path) {
                Ok(path) => roots.push(path),
                Err(_) => counts.skipped += 1,
            },
        }
    }
    // Sorted by components, a folder's descendants follow it directly: drop them.
    roots.sort();
    let mut folders = Vec::new();
    let mut last: Option<std::path::PathBuf> = None;
    for root in roots {
        if last.as_ref().is_some_and(|kept| root.starts_with(kept)) {
            continue;
        }
        match std::fs::symlink_metadata(&root) {
            Ok(metadata) if metadata.is_dir() => folders.push(root.clone()),
            Ok(_) => counts.files += 1,
            Err(_) => counts.skipped += 1,
        }
        last = Some(root);
    }
    while let Some(folder) = folders.pop() {
        check()?;
        counts.folders += 1;
        let Ok(entries) = std::fs::read_dir(&folder) else {
            counts.skipped += 1;
            continue;
        };
        for entry in entries {
            // The directory entry type needs no extra stat call per file.
            match entry.and_then(|entry| Ok((entry.file_type()?, entry.path()))) {
                Ok((kind, path)) if kind.is_dir() => folders.push(path),
                Ok(_) => counts.files += 1,
                Err(_) => counts.skipped += 1,
            }
        }
    }
    Ok(counts)
}
#[tauri::command]
pub async fn count_delete_entries(
    paths: Vec<String>,
    request_id: String,
    sizing: State<'_, DirectorySizing>,
) -> Result<DeleteCounts> {
    let request = sizing.register(request_id)?;
    blocking("io_error", move || {
        count_delete_paths(paths, || request.check())
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn previews_fail_fast_while_a_transfer_holds_the_connection() {
        let connection = std::sync::Mutex::new(());
        let wait = std::time::Duration::from_millis(50);
        let held = connection.lock().unwrap();
        let started = std::time::Instant::now();
        assert_eq!(
            lock_for_preview(&connection, wait).unwrap_err().code,
            "sftp_busy"
        );
        assert!(started.elapsed() < std::time::Duration::from_secs(2));
        drop(held);
        assert!(lock_for_preview(&connection, wait).is_ok());
    }
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
        let counts = count_delete_paths(
            vec![
                filesystem::text(temp.path()),
                filesystem::text(&temp.path().join("nested")),
                filesystem::text(&temp.path().join("b.txt")),
                filesystem::text(temp.path()),
            ],
            || Ok(()),
        )
        .unwrap();
        assert_eq!(counts.folders, 2);
        assert_eq!(counts.files, 2);
        assert_eq!(counts.skipped, 0);
    }

    #[cfg(unix)]
    #[test]
    fn counts_a_selected_symlink_once_without_following_it() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::create_dir(temp.path().join("target")).unwrap();
        std::fs::write(temp.path().join("target/inside.txt"), b"file").unwrap();
        std::os::unix::fs::symlink(temp.path().join("target"), temp.path().join("link")).unwrap();
        let counts =
            count_delete_paths(vec![filesystem::text(&temp.path().join("link"))], || Ok(()))
                .unwrap();
        assert_eq!((counts.folders, counts.files), (0, 1));
    }

    #[test]
    fn delete_counting_stops_when_cancelled() {
        let temp = tempfile::tempdir().unwrap();
        let cancelled = count_delete_paths(vec![filesystem::text(temp.path())], || {
            Err(FsError::new("cancelled", "cancelled"))
        });
        assert!(cancelled.is_err_and(|error| error.code == "cancelled"));
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
    blocking("io_error", move || {
        filesystem::list(std::path::Path::new(&path))
    })
    .await
}
#[tauri::command]
pub async fn read_text_preview(
    path: String,
    connection_id: Option<String>,
    state: State<'_, std::sync::Arc<crate::sftp::Manager>>,
) -> Result<String> {
    let manager = state.inner().clone();
    blocking("io_error", move || {
        with_preview_path(
            &manager,
            connection_id.as_deref(),
            &path,
            8 * 1024 * 1024,
            filesystem::read_text_preview,
        )
    })
    .await
}
#[tauri::command]
pub async fn read_pdf_preview(
    path: String,
    connection_id: Option<String>,
    state: State<'_, std::sync::Arc<crate::sftp::Manager>>,
) -> Result<tauri::ipc::Response> {
    let manager = state.inner().clone();
    blocking("io_error", move || {
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
}
#[tauri::command]
pub async fn read_image_preview(
    path: String,
    connection_id: Option<String>,
    state: State<'_, std::sync::Arc<crate::sftp::Manager>>,
) -> Result<String> {
    let manager = state.inner().clone();
    blocking("io_error", move || {
        with_preview_path(
            &manager,
            connection_id.as_deref(),
            &path,
            4 * 1024 * 1024,
            filesystem::read_image_preview,
        )
    })
    .await
}
#[tauri::command]
pub async fn read_image_thumbnail(
    path: String,
    max_side: u32,
    connection_id: Option<String>,
    state: State<'_, std::sync::Arc<crate::sftp::Manager>>,
) -> Result<String> {
    let manager = state.inner().clone();
    blocking("io_error", move || {
        with_preview_path(
            &manager,
            connection_id.as_deref(),
            &path,
            crate::thumbnails::MAX_THUMBNAIL_SOURCE_BYTES,
            |path| crate::thumbnails::read_image_thumbnail(path, max_side),
        )
    })
    .await
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
    let connection = lock_for_preview(&connection, std::time::Duration::from_secs(3))?;
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
/// A transfer holds the connection for its whole run: fail with `sftp_busy`
/// instead of blocking. The short wait lets parallel thumbnails (3 at a time)
/// take turns. shortcut: a preview slower than `wait` also fails the others.
fn lock_for_preview<T>(
    connection: &std::sync::Mutex<T>,
    wait: std::time::Duration,
) -> Result<std::sync::MutexGuard<'_, T>> {
    let deadline = std::time::Instant::now() + wait;
    loop {
        match connection.try_lock() {
            Ok(guard) => return Ok(guard),
            Err(std::sync::TryLockError::Poisoned(e)) => {
                return Err(FsError::new("sftp_error", e.to_string()))
            }
            Err(std::sync::TryLockError::WouldBlock) if std::time::Instant::now() < deadline => {
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
            Err(std::sync::TryLockError::WouldBlock) => {
                return Err(FsError::new(
                    "sftp_busy",
                    "Connection is busy with a transfer. Try again when it finishes.",
                ))
            }
        }
    }
}
#[tauri::command]
pub async fn read_markdown_image(
    markdown_path: String,
    source: String,
    connection_id: Option<String>,
    state: State<'_, std::sync::Arc<crate::sftp::Manager>>,
) -> Result<filesystem::PreviewImage> {
    let manager = state.inner().clone();
    blocking("io_error", move || {
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
}
#[tauri::command]
pub async fn list_roots() -> Result<Vec<filesystem::Root>> {
    blocking("io_error", || Ok(filesystem::roots())).await
}
#[tauri::command]
pub async fn open_file(path: String) -> Result<()> {
    blocking("io_error", move || {
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
/// Why the settings were reset at startup, once; None when they loaded fine.
#[tauri::command]
pub fn take_config_notice() -> Option<String> {
    config::take_reset_notice()
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
    blocking("io_error", move || {
        edit(std::path::Path::new(&path), editor.as_deref())
    })
    .await
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
    blocking("io_error", move || {
        filesystem::directory_size_checked(std::path::Path::new(&path), || request.check())
    })
    .await
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

use crate::{
    error::{FsError, Result},
    filesystem,
};
use std::path::{Path, PathBuf};

#[cfg(target_os = "linux")]
mod linux;
#[cfg(windows)]
mod windows;

fn local_item(path: &Path) -> Result<PathBuf> {
    if !path.is_absolute() || path.to_str().is_none_or(|path| path.contains('\0')) {
        return Err(FsError::new(
            "invalid_path",
            "System actions require one absolute local path",
        ));
    }
    // Resolve the parent only: the shell should receive the link, not its target.
    let path = match (path.parent(), path.file_name()) {
        (Some(parent), Some(name)) => filesystem::absolute(parent)?.join(name),
        _ => filesystem::absolute(path)?,
    };
    let metadata = std::fs::symlink_metadata(&path).map_err(|e| FsError::io(e, &path))?;
    if !metadata.is_file() && !metadata.is_dir() && !metadata.is_symlink() {
        return Err(FsError::new(
            "invalid_path",
            "System actions support files, folders, and symbolic links only",
        ));
    }
    Ok(path)
}

#[tauri::command]
pub async fn show_system_actions(window: tauri::WebviewWindow, path: String) -> Result<()> {
    let path = tauri::async_runtime::spawn_blocking(move || local_item(Path::new(&path)))
        .await
        .map_err(|e| FsError::new("system_actions_error", e.to_string()))??;
    #[cfg(target_os = "linux")]
    {
        let (tx, rx) = std::sync::mpsc::channel();
        let owner = window.clone();
        window
            .run_on_main_thread(move || {
                let result = owner
                    .gtk_window()
                    .map_err(|e| FsError::new("system_actions_error", e.to_string()))
                    .and_then(|parent| linux::show(&parent, &path));
                let _ = tx.send(result);
            })
            .map_err(|e| FsError::new("system_actions_error", e.to_string()))?;
        tauri::async_runtime::spawn_blocking(move || {
            rx.recv()
                .map_err(|e| FsError::new("system_actions_error", e.to_string()))?
        })
        .await
        .map_err(|e| FsError::new("system_actions_error", e.to_string()))?
    }
    #[cfg(windows)]
    {
        let _ = window;
        tauri::async_runtime::spawn_blocking(move || {
            let executable = std::env::current_exe()
                .map_err(|e| FsError::new("system_actions_error", e.to_string()))?;
            let output = std::process::Command::new(executable)
                .arg("--mycmd-system-menu")
                .arg(&path)
                .output()
                .map_err(|e| FsError::io(e, &path))?;
            check_output(output)
        })
        .await
        .map_err(|e| FsError::new("system_actions_error", e.to_string()))?
    }
    #[cfg(target_os = "macos")]
    {
        let _ = window;
        tauri::async_runtime::spawn_blocking(move || {
            let output = std::process::Command::new("/usr/bin/osascript")
                .arg("-e")
                .arg(include_str!("system_actions/macos.applescript"))
                .arg(&path)
                .output()
                .map_err(|e| FsError::io(e, &path))?;
            check_output(output)
        })
        .await
        .map_err(|e| FsError::new("system_actions_error", e.to_string()))?
    }
}

#[cfg(any(windows, target_os = "macos", test))]
fn check_output(output: std::process::Output) -> Result<()> {
    if output.status.success() {
        return Ok(());
    }
    let detail = String::from_utf8_lossy(&output.stderr);
    Err(FsError::new(
        "system_actions_error",
        if detail.trim().is_empty() {
            format!("System actions helper exited with {}", output.status)
        } else {
            detail.trim().to_owned()
        },
    ))
}

#[cfg(windows)]
pub fn run_helper() -> Option<i32> {
    let mut args = std::env::args_os().skip(1);
    if args.next().as_deref() != Some(std::ffi::OsStr::new("--mycmd-system-menu")) {
        return None;
    }
    let result = args
        .next()
        .ok_or_else(|| FsError::new("invalid_path", "Missing system menu path"))
        .and_then(|path| local_item(Path::new(&path)))
        .and_then(|path| {
            windows::show(&path).map_err(|e| FsError::new("system_actions_error", e.to_string()))
        });
    Some(match result {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("{}", error.message);
            1
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn validates_local_items_without_interpreting_names_as_commands() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("quotes ' & spaces 日本語.txt");
        std::fs::write(&path, "keep").unwrap();
        assert_eq!(local_item(&path).unwrap().file_name(), path.file_name());
        assert!(local_item(dir.path()).is_ok());
        for path in [
            Path::new("relative.txt"),
            Path::new("archive:/a.zip!/file"),
            Path::new("search:result"),
            Path::new("/bad\0path"),
        ] {
            assert_eq!(local_item(path).unwrap_err().code, "invalid_path");
        }
        assert!(local_item(&dir.path().join("missing")).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn preserves_symlinks_even_when_the_target_is_missing() {
        let dir = tempfile::tempdir().unwrap();
        let link = dir.path().join("link");
        std::os::unix::fs::symlink("missing", &link).unwrap();
        let path = local_item(&link).unwrap();
        assert!(std::fs::symlink_metadata(path).unwrap().is_symlink());
    }

    #[cfg(unix)]
    #[test]
    fn helper_errors_are_reported_without_claiming_success() {
        use std::os::unix::process::ExitStatusExt;
        let output = std::process::Output {
            status: std::process::ExitStatus::from_raw(256),
            stdout: vec![],
            stderr: b"Native action failed\n".to_vec(),
        };
        assert_eq!(
            check_output(output).unwrap_err().message,
            "Native action failed"
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_system_actions_script_compiles() {
        let dir = tempfile::tempdir().unwrap();
        let script = dir.path().join("system-actions.applescript");
        std::fs::write(&script, include_str!("system_actions/macos.applescript")).unwrap();
        let output = std::process::Command::new("/usr/bin/osacompile")
            .arg("-o")
            .arg(dir.path().join("system-actions.scpt"))
            .arg(&script)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

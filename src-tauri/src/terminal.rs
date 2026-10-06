use crate::{
    error::{FsError, Result},
    filesystem,
};
use std::{
    path::Path,
    process::{Command, Stdio},
};

#[cfg(target_os = "linux")]
fn configure_alternative(command: &mut Command, directory: &Path, name: &str) {
    match name {
        "cosmic-term" | "gnome-terminal" | "kgx" | "xfce4-terminal" | "mate-terminal" | "tilix"
        | "alacritty" => {
            command.arg("--working-directory").arg(directory);
        }
        "konsole" => {
            command.arg("--workdir").arg(directory);
        }
        "kitty" => {
            command.arg("--directory").arg(directory);
        }
        "wezterm" => {
            command.args(["start", "--cwd"]).arg(directory);
        }
        _ => {} // Other terminals inherit the launcher's current directory.
    }
}

fn launchers(directory: &Path) -> Vec<Command> {
    #[cfg(target_os = "linux")]
    let commands = {
        let mut xdg = Command::new("xdg-terminal-exec");
        let mut argument = std::ffi::OsString::from("--dir=");
        argument.push(directory);
        xdg.arg(argument);
        // Debian/Ubuntu's alternatives system selects the user's terminal.
        let mut alternative = Command::new("x-terminal-emulator");
        if let Some(executable) = std::env::var_os("PATH").and_then(|path| {
            std::env::split_paths(&path)
                .find_map(|base| std::fs::canonicalize(base.join("x-terminal-emulator")).ok())
        }) {
            if let Some(name) = executable.file_name().and_then(|name| name.to_str()) {
                configure_alternative(&mut alternative, directory, name);
            }
        }
        vec![xdg, alternative]
    };
    #[cfg(target_os = "macos")]
    let commands = {
        let mut terminal = Command::new("open");
        terminal.args(["-a", "Terminal"]).arg(directory);
        vec![terminal]
    };
    #[cfg(windows)]
    let commands = {
        // A new console uses the default terminal host configured in Windows.
        // The directory is passed as cwd, never interpolated into a shell command.
        let mut terminal = Command::new("cmd.exe");
        terminal.args(["/D", "/C", "start", "", "cmd.exe", "/D"]);
        vec![terminal]
    };
    #[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
    let commands: Vec<Command> = Vec::new();

    commands
        .into_iter()
        .map(|mut command| {
            command
                .current_dir(directory)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null());
            command
        })
        .collect()
}

fn open(path: &Path) -> Result<()> {
    let directory = filesystem::absolute(path)?;
    if !directory.is_dir() {
        return Err(FsError::new(
            "invalid_path",
            "Terminal requires a directory",
        ));
    }
    for mut command in launchers(&directory) {
        match command.spawn() {
            Ok(mut child) => {
                // Do not block the app while the terminal stays open, and reap
                // the child when it exits rather than leaving a zombie process.
                std::thread::spawn(move || {
                    let _ = child.wait();
                });
                return Ok(());
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(FsError::io(error, &directory)),
        }
    }
    Err(FsError::new(
        "terminal_unavailable",
        if cfg!(target_os = "linux") {
            "No system terminal launcher found. Install xdg-terminal-exec or configure x-terminal-emulator."
        } else {
            "No system terminal launcher found."
        },
    ))
}

#[tauri::command]
pub async fn open_terminal(path: String) -> Result<()> {
    tauri::async_runtime::spawn_blocking(move || open(Path::new(&path)))
        .await
        .map_err(|error| FsError::new("io_error", error.to_string()))?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_files_before_launching() {
        let directory = tempfile::tempdir().unwrap();
        let file = directory.path().join("file.txt");
        std::fs::write(&file, "payload").unwrap();
        assert_eq!(open(&file).unwrap_err().code, "invalid_path");
    }

    #[test]
    fn rejects_missing_directories_before_launching() {
        let directory = tempfile::tempdir().unwrap();
        assert_eq!(
            open(&directory.path().join("missing")).unwrap_err().code,
            "not_found"
        );
    }

    #[test]
    fn launchers_use_the_directory_without_shell_interpolation() {
        let directory = Path::new("/directory with spaces & symbols");
        let commands = launchers(directory);
        assert!(!commands.is_empty());
        for command in commands {
            assert_eq!(command.get_current_dir(), Some(directory));
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn prefers_xdg_then_the_system_terminal_alternative() {
        let directory = Path::new("/directory with spaces & symbols");
        let commands = launchers(directory);
        assert_eq!(commands[0].get_program(), "xdg-terminal-exec");
        let arguments: Vec<_> = commands[0].get_args().collect();
        assert_eq!(
            arguments,
            vec![std::ffi::OsStr::new(
                "--dir=/directory with spaces & symbols"
            )]
        );
        assert_eq!(commands[1].get_program(), "x-terminal-emulator");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn passes_an_explicit_directory_to_cosmic_terminal() {
        let directory = Path::new("/directory with spaces & symbols");
        let mut command = Command::new("x-terminal-emulator");
        configure_alternative(&mut command, directory, "cosmic-term");
        let arguments: Vec<_> = command.get_args().collect();
        assert_eq!(
            arguments,
            vec![
                std::ffi::OsStr::new("--working-directory"),
                directory.as_os_str()
            ]
        );
    }
}

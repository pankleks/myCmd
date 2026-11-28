use crate::{
    error::{blocking, FsError, Result},
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

#[cfg(target_os = "linux")]
fn alternative_name() -> Option<String> {
    std::env::var_os("PATH")
        .and_then(|path| {
            std::env::split_paths(&path)
                .find_map(|base| std::fs::canonicalize(base.join("x-terminal-emulator")).ok())
        })?
        .file_name()
        .and_then(|name| name.to_str())
        .map(str::to_owned)
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
        if let Some(name) = alternative_name() {
            configure_alternative(&mut alternative, directory, &name);
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
    // The Windows launcher is cmd.exe, which cannot start in a `\\?\` folder.
    #[cfg(windows)]
    let directory = filesystem::windows_working_directory(&directory)?;
    for mut command in launchers(&directory) {
        match spawn_detached(&mut command) {
            Ok(()) => return Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(FsError::io(error, &directory)),
        }
    }
    Err(terminal_unavailable())
}

fn spawn_detached(command: &mut Command) -> std::io::Result<()> {
    let mut child = command.spawn()?;
    // Do not block the app while the terminal stays open, and reap
    // the child when it exits rather than leaving a zombie process.
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}

fn terminal_unavailable() -> FsError {
    FsError::new(
        "terminal_unavailable",
        if cfg!(target_os = "linux") {
            "No system terminal launcher found. Install xdg-terminal-exec or configure x-terminal-emulator."
        } else {
            "No system terminal launcher found."
        },
    )
}

#[cfg(target_os = "linux")]
fn script_interpreter(script: &Path) -> &'static str {
    match script
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_lowercase)
        .as_deref()
    {
        Some("bash") => "bash",
        Some("zsh") => "zsh",
        _ => "sh",
    }
}

/// The argv that runs the script: an executable file with a shebang runs
/// itself so the kernel picks its interpreter (bash `[[` fails under dash).
#[cfg(target_os = "linux")]
fn script_argv(script: &Path) -> Vec<&std::ffi::OsStr> {
    use std::{io::Read, os::unix::fs::PermissionsExt};
    let executable = std::fs::metadata(script).is_ok_and(|m| m.permissions().mode() & 0o111 != 0);
    let mut magic = [0u8; 2];
    let shebang = executable
        && std::fs::File::open(script).is_ok_and(|mut f| f.read_exact(&mut magic).is_ok())
        && magic == *b"#!";
    if shebang {
        vec![script.as_os_str()]
    } else {
        vec![script_interpreter(script).as_ref(), script.as_os_str()]
    }
}

/// `launchers` with the script's argv appended after each terminal's
/// separator for "run this command".
#[cfg(target_os = "linux")]
fn script_launchers(argv: &[&std::ffi::OsStr], directory: &Path) -> Vec<Command> {
    let mut commands = launchers(directory);
    commands[0].arg("--").args(argv);
    // ponytail: cosmic-term/xterm-style `-e` is best effort; the flag was
    // observed to parse on cosmic-term but cannot be verified headless.
    let separator: &[&str] = match alternative_name().as_deref() {
        Some("gnome-terminal" | "wezterm") => &["--"],
        Some("kitty") => &[],
        _ => &["-e"],
    };
    commands[1].args(separator).args(argv);
    commands
}

/// Open a terminal in the script's directory and run the script inside it.
/// Arguments are passed as argv, never interpolated into a shell command.
#[cfg(target_os = "linux")]
pub fn run_script(path: &Path) -> Result<()> {
    let script = filesystem::absolute(path)?;
    if !script.is_file() {
        return Err(FsError::new(
            "invalid_path",
            "Only files can be run in a terminal",
        ));
    }
    let directory = script.parent().unwrap().to_owned();
    // ponytail: keep trying other launchers on any spawn error; terminal CLI
    // flags vary and a flag mismatch only shows up after a successful spawn.
    for mut command in script_launchers(&script_argv(&script), &directory) {
        if spawn_detached(&mut command).is_ok() {
            return Ok(());
        }
    }
    Err(terminal_unavailable())
}

#[tauri::command]
pub async fn open_terminal(path: String) -> Result<()> {
    blocking("io_error", move || open(Path::new(&path))).await
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

    #[cfg(target_os = "linux")]
    #[test]
    fn script_launchers_pass_interpreter_and_script_as_argv() {
        let directory = Path::new("/directory with spaces & symbols");
        let script = directory.join("deploy me.sh");
        let commands = script_launchers(&script_argv(&script), directory);
        assert_eq!(commands.len(), 2);
        let xdg: Vec<_> = commands[0].get_args().collect();
        assert!(xdg.contains(&std::ffi::OsStr::new("--")));
        assert!(xdg.contains(&std::ffi::OsStr::new("sh")));
        assert!(xdg.iter().any(|arg| *arg == script.as_os_str()));
        let alternative: Vec<_> = commands[1].get_args().collect();
        assert!(alternative.iter().any(|arg| *arg == script.as_os_str()));
        assert!(!format!("{alternative:?}").contains("&&"));
        for command in &commands {
            assert_eq!(command.get_current_dir(), Some(directory));
        }
        assert_eq!(script_interpreter(Path::new("run.BASH")), "bash");
        assert_eq!(script_interpreter(Path::new("run.zsh")), "zsh");
        assert_eq!(script_interpreter(Path::new("run.sh")), "sh");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn executable_scripts_with_a_shebang_run_through_the_kernel() {
        use std::os::unix::fs::PermissionsExt;
        let directory = tempfile::tempdir().unwrap();
        let script = directory.path().join("build; rm -rf ~.sh");
        let mode = |mode| {
            std::fs::set_permissions(&script, std::fs::Permissions::from_mode(mode)).unwrap()
        };
        std::fs::write(&script, "#!/bin/bash\n[[ 1 ]]\n").unwrap();
        mode(0o755);
        assert_eq!(script_argv(&script), [script.as_os_str()]);
        let commands = script_launchers(&script_argv(&script), directory.path());
        let xdg: Vec<_> = commands[0].get_args().collect();
        assert_eq!(xdg[1..], [std::ffi::OsStr::new("--"), script.as_os_str()]);
        // Not executable, or no shebang: fall back to the interpreter.
        mode(0o644);
        assert_eq!(script_argv(&script), ["sh".as_ref(), script.as_os_str()]);
        std::fs::write(&script, "echo hi\n").unwrap();
        mode(0o755);
        assert_eq!(script_argv(&script), ["sh".as_ref(), script.as_os_str()]);
    }
}

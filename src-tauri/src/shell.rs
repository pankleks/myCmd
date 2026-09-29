use serde::Serialize;
use std::{env, process::Command};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandResult {
    pub exit_code: Option<i32>,
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
}

fn run_shell(
    shell: &std::ffi::OsStr,
    interactive: bool,
    command: &str,
    cwd: &str,
) -> std::result::Result<CommandResult, String> {
    #[cfg(windows)]
    let output = {
        let _ = interactive;
        Command::new(shell)
            .arg("/C")
            .arg(command)
            .current_dir(cwd)
            .output()
    };
    #[cfg(not(windows))]
    let output = {
        let mut shell_command = Command::new(shell);
        if interactive {
            shell_command.arg("-i");
        }
        shell_command.arg("-c");
        if interactive {
            // Print a marker first: interactive shells emit startup warnings
            // and rc-file noise on stderr before running the command, and the
            // marker lets us strip all of it locale-independently.
            shell_command.arg(format!("echo {INTERACTIVE_STDERR_MARKER} >&2\n{command}"));
        } else {
            shell_command.arg(command);
        }
        shell_command.current_dir(cwd).output()
    };
    let output = output.map_err(|error| error.to_string())?;
    Ok(CommandResult {
        exit_code: output.status.code(),
        success: output.status.success(),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    })
}

/// Marker printed to stderr before the real command when retrying in an
/// interactive shell. Everything up to and including it (shell startup
/// warnings, rc-file noise) is stripped from the captured stderr.
#[cfg(not(windows))]
const INTERACTIVE_STDERR_MARKER: &str = "__mycmd_stderr_begin__";

#[cfg(not(windows))]
fn strip_interactive_prelude(stderr: &str) -> String {
    match stderr
        .lines()
        .position(|line| line == INTERACTIVE_STDERR_MARKER)
    {
        Some(index) => {
            let mut stripped = stderr
                .lines()
                .skip(index + 1)
                .collect::<Vec<_>>()
                .join("\n");
            if !stripped.is_empty() && stderr.ends_with('\n') {
                stripped.push('\n');
            }
            stripped
        }
        None => stderr.to_string(),
    }
}

fn execute(command: &str, cwd: &str) -> std::result::Result<CommandResult, String> {
    #[cfg(windows)]
    {
        let shell = env::var_os("COMSPEC").unwrap_or_else(|| "cmd.exe".into());
        run_shell(&shell, false, command, cwd)
    }
    #[cfg(not(windows))]
    {
        let shell = env::var_os("SHELL").unwrap_or_else(|| "/bin/sh".into());
        // Load aliases/functions up front. Never replay a command: a failing
        // compound command may already have performed destructive actions.
        let result = run_shell(&shell, true, command, cwd)?;
        Ok(CommandResult {
            stderr: strip_interactive_prelude(&result.stderr),
            ..result
        })
    }
}

#[tauri::command]
pub async fn run_system_command(
    command: String,
    cwd: String,
) -> std::result::Result<CommandResult, String> {
    tauri::async_runtime::spawn_blocking(move || execute(&command, &cwd))
        .await
        .map_err(|error| error.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // execute() reads the SHELL environment variable, so tests that change it
    // must serialize with every other test that runs commands.
    static SHELL_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn executes_in_the_requested_directory_and_captures_output() {
        let _guard = SHELL_LOCK.lock().unwrap();
        let cwd = env::current_dir().unwrap();
        let cwd = cwd.to_str().unwrap();
        let pwd = if cfg!(windows) { "cd" } else { "pwd" };
        let directory = execute(pwd, cwd).unwrap();
        let output = execute("echo mycmd-shell-test", cwd).unwrap();

        assert!(directory.success);
        assert_eq!(directory.stdout.trim(), cwd);
        assert!(output.success);
        assert_eq!(output.stdout.trim(), "mycmd-shell-test");
        assert!(output.stderr.is_empty());
    }

    #[cfg(not(windows))]
    #[test]
    fn strips_shell_startup_noise_before_the_marker() {
        let stderr = "bash: cannot set terminal process group (123): Inappropriate ioctl for device\nbash: no job control in this shell\n__mycmd_stderr_begin__\nboom\n";
        assert_eq!(strip_interactive_prelude(stderr), "boom\n");
        assert_eq!(strip_interactive_prelude("plain\n"), "plain\n");
        assert_eq!(strip_interactive_prelude(""), "");
    }

    #[cfg(not(windows))]
    #[test]
    fn uses_an_interactive_shell_without_retrying() {
        #[cfg(unix)]
        use std::os::unix::fs::PermissionsExt;
        let _guard = SHELL_LOCK.lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let fake = dir.path().join("fake-shell");
        std::fs::write(
            &fake,
            "#!/bin/sh\nif [ \"$1\" = \"-i\" ]; then\n  echo interactive-retry\n  exit 0\nfi\necho \"$2: command not found\" >&2\nexit 127\n",
        )
        .unwrap();
        #[cfg(unix)]
        std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
        let previous = env::var_os("SHELL");
        env::set_var("SHELL", &fake);
        let result = execute("whatever", dir.path().to_str().unwrap());
        match previous {
            Some(value) => env::set_var("SHELL", value),
            None => env::remove_var("SHELL"),
        }
        let result = result.unwrap();
        assert!(result.success);
        assert_eq!(result.stdout.trim(), "interactive-retry");
        assert!(!result.stderr.contains(INTERACTIVE_STDERR_MARKER));
    }

    #[cfg(not(windows))]
    #[test]
    fn failed_compound_command_does_not_repeat_side_effects() {
        let _guard = SHELL_LOCK.lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let previous = env::var_os("SHELL");
        env::set_var("SHELL", "/bin/sh");
        let result = execute(
            "echo once >> count; __mycmd_missing_command__",
            dir.path().to_str().unwrap(),
        );
        match previous {
            Some(value) => env::set_var("SHELL", value),
            None => env::remove_var("SHELL"),
        }
        let result = result.unwrap();
        assert!(!result.success);
        assert_eq!(
            std::fs::read_to_string(dir.path().join("count")).unwrap(),
            "once\n"
        );
    }
}

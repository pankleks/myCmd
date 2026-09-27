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

fn execute(command: &str, cwd: &str) -> std::result::Result<CommandResult, String> {
    #[cfg(windows)]
    let output = {
        let shell = env::var_os("COMSPEC").unwrap_or_else(|| "cmd.exe".into());
        Command::new(shell)
            .arg("/C")
            .arg(command)
            .current_dir(cwd)
            .output()
    };
    #[cfg(not(windows))]
    let output = {
        let shell = env::var_os("SHELL").unwrap_or_else(|| "/bin/sh".into());
        Command::new(shell)
            .arg("-c")
            .arg(command)
            .current_dir(cwd)
            .output()
    };
    let output = output.map_err(|error| error.to_string())?;
    Ok(CommandResult {
        exit_code: output.status.code(),
        success: output.status.success(),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    })
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

    #[test]
    fn executes_in_the_requested_directory_and_captures_output() {
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
}

use serde::Serialize;
use std::{
    collections::VecDeque,
    env,
    io::Read,
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};

const OUTPUT_LIMIT: usize = 1024 * 1024;
const RUNTIME_LIMIT: Duration = Duration::from_secs(120);
type ActiveCommand = Option<(String, Arc<AtomicBool>)>;
#[derive(Default)]
struct ShellState {
    active: ActiveCommand,
    early_cancellations: VecDeque<String>,
}
#[derive(Default, Clone)]
pub struct ShellManager(Arc<Mutex<ShellState>>);
struct CommandLease {
    manager: ShellManager,
    cancel: Arc<AtomicBool>,
}
impl ShellManager {
    fn begin(&self, id: String) -> std::result::Result<CommandLease, String> {
        validate_command_id(&id)?;
        let mut state = self.0.lock().map_err(|e| e.to_string())?;
        if state.active.is_some() {
            return Err("A shell command is already running".into());
        }
        let cancelled = state
            .early_cancellations
            .iter()
            .position(|pending| pending == &id)
            .and_then(|index| state.early_cancellations.remove(index))
            .is_some();
        let cancel = Arc::new(AtomicBool::new(cancelled));
        state.active = Some((id, cancel.clone()));
        Ok(CommandLease {
            manager: self.clone(),
            cancel,
        })
    }
    fn cancel(&self, id: &str) -> std::result::Result<(), String> {
        validate_command_id(id)?;
        let mut state = self.0.lock().map_err(|e| e.to_string())?;
        if let Some((active_id, cancel)) = state.active.as_ref() {
            if active_id == id {
                cancel.store(true, Ordering::Relaxed);
                return Ok(());
            }
        }
        if !state
            .early_cancellations
            .iter()
            .any(|pending| pending == id)
        {
            if state.early_cancellations.len() >= 64 {
                state.early_cancellations.pop_front();
            }
            state.early_cancellations.push_back(id.to_owned());
        }
        Ok(())
    }
}
impl Drop for CommandLease {
    fn drop(&mut self) {
        let mut active = self
            .manager
            .0
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if active
            .active
            .as_ref()
            .is_some_and(|(_, token)| Arc::ptr_eq(token, &self.cancel))
        {
            active.active = None;
        }
    }
}

fn validate_command_id(id: &str) -> std::result::Result<(), String> {
    if id.is_empty() || id.len() > 128 {
        return Err("Command ID must contain 1 to 128 bytes".into());
    }
    Ok(())
}

#[derive(Default)]
struct CapturedOutput {
    bytes: Vec<u8>,
    truncated: bool,
    read_failed: bool,
}
struct Capture {
    output: Arc<Mutex<CapturedOutput>>,
    done: std::sync::mpsc::Receiver<()>,
    stop: Arc<AtomicBool>,
}
impl Capture {
    fn finish(self, timeout: Duration) -> (String, bool, bool, bool) {
        let incomplete = self.done.recv_timeout(timeout).is_err();
        if incomplete {
            self.stop.store(true, Ordering::Relaxed);
        }
        let output = self.output.lock().unwrap();
        (
            String::from_utf8_lossy(&output.bytes).into_owned(),
            output.truncated,
            incomplete || output.read_failed,
            output.read_failed,
        )
    }
}
fn capture(mut pipe: impl Read + Send + 'static) -> Capture {
    let (send, receive) = std::sync::mpsc::channel();
    let output = Arc::new(Mutex::new(CapturedOutput::default()));
    let captured = output.clone();
    let stop = Arc::new(AtomicBool::new(false));
    let stopped = stop.clone();
    std::thread::spawn(move || {
        let mut buffer = [0; 8192];
        loop {
            if stopped.load(Ordering::Relaxed) {
                break;
            }
            match pipe.read(&mut buffer) {
                Ok(0) => break,
                Ok(count) => {
                    let mut output = captured.lock().unwrap();
                    let keep = count.min(OUTPUT_LIMIT - output.bytes.len());
                    output.bytes.extend_from_slice(&buffer[..keep]);
                    output.truncated |= keep < count;
                }
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(20));
                }
                Err(_) => {
                    captured.lock().unwrap().read_failed = true;
                    break;
                }
            }
        }
        let _ = send.send(());
    });
    Capture {
        output,
        done: receive,
        stop,
    }
}

#[cfg(unix)]
fn nonblocking(pipe: &impl std::os::fd::AsRawFd) -> std::io::Result<()> {
    let fd = pipe.as_raw_fd();
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(windows)]
struct PollingPipe<P>(P);
#[cfg(windows)]
impl<P: Read + std::os::windows::io::AsRawHandle> Read for PollingPipe<P> {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        use std::ffi::c_void;
        #[link(name = "kernel32")]
        extern "system" {
            fn PeekNamedPipe(
                handle: *mut c_void,
                buffer: *mut c_void,
                buffer_size: u32,
                bytes_read: *mut u32,
                available: *mut u32,
                remaining: *mut u32,
            ) -> i32;
        }
        if buffer.is_empty() {
            return Ok(0);
        }
        let mut available = 0;
        let ok = unsafe {
            PeekNamedPipe(
                self.0.as_raw_handle(),
                std::ptr::null_mut(),
                0,
                std::ptr::null_mut(),
                &mut available,
                std::ptr::null_mut(),
            )
        };
        if ok == 0 {
            let error = std::io::Error::last_os_error();
            // ERROR_BROKEN_PIPE: every writer has closed its pipe handle.
            return if error.raw_os_error() == Some(109) {
                Ok(0)
            } else {
                Err(error)
            };
        }
        if available == 0 {
            return Err(std::io::ErrorKind::WouldBlock.into());
        }
        let count = buffer.len().min(available as usize);
        self.0.read(&mut buffer[..count])
    }
}

fn terminate(child: &mut std::process::Child) {
    #[cfg(unix)]
    unsafe {
        libc::kill(-(child.id() as i32), libc::SIGKILL);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let cleanup = Command::new("taskkill.exe")
            .args(["/PID", &child.id().to_string(), "/T", "/F"])
            .creation_flags(0x08000000) // CREATE_NO_WINDOW
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
        if let Ok(mut cleanup) = cleanup {
            let deadline = Instant::now() + Duration::from_secs(5);
            loop {
                match cleanup.try_wait() {
                    Ok(Some(_)) => break,
                    Ok(None) if Instant::now() < deadline => {
                        std::thread::sleep(Duration::from_millis(20))
                    }
                    _ => {
                        let _ = cleanup.kill();
                        let _ = cleanup.wait();
                        break;
                    }
                }
            }
        }
    }
    let _ = child.kill();
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandResult {
    pub exit_code: Option<i32>,
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
    pub cancelled: bool,
    pub timed_out: bool,
    pub output_truncated: bool,
    pub output_incomplete: bool,
    pub output_read_failed: bool,
}

#[cfg(test)]
fn run_shell(
    shell: &std::ffi::OsStr,
    interactive: bool,
    command: &str,
    cwd: &str,
) -> std::result::Result<CommandResult, String> {
    run_shell_controlled(
        shell,
        interactive,
        command,
        cwd,
        &AtomicBool::new(false),
        RUNTIME_LIMIT,
    )
}

fn run_shell_controlled(
    shell: &std::ffi::OsStr,
    interactive: bool,
    command: &str,
    cwd: &str,
    cancel: &AtomicBool,
    limit: Duration,
) -> std::result::Result<CommandResult, String> {
    if cancel.load(Ordering::Relaxed) {
        return Ok(CommandResult {
            exit_code: None,
            success: false,
            stdout: String::new(),
            stderr: String::new(),
            cancelled: true,
            timed_out: false,
            output_truncated: false,
            output_incomplete: false,
            output_read_failed: false,
        });
    }
    #[cfg(windows)]
    let mut shell_command = {
        use std::os::windows::process::CommandExt;
        let _ = interactive;
        let mut builder = Command::new(shell);
        builder.arg("/C").arg(command);
        builder.creation_flags(0x08000000); // CREATE_NO_WINDOW
        builder
    };
    #[cfg(not(windows))]
    let mut shell_command = {
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
        shell_command
    };
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        shell_command.process_group(0);
    }
    let mut child = shell_command
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| error.to_string())?;
    let stdout_pipe = child.stdout.take().unwrap();
    let stderr_pipe = child.stderr.take().unwrap();
    #[cfg(unix)]
    if let Err(error) = nonblocking(&stdout_pipe).and_then(|_| nonblocking(&stderr_pipe)) {
        terminate(&mut child);
        let _ = child.wait();
        return Err(error.to_string());
    }
    #[cfg(windows)]
    let stdout_pipe = PollingPipe(stdout_pipe);
    #[cfg(windows)]
    let stderr_pipe = PollingPipe(stderr_pipe);
    let stdout = capture(stdout_pipe);
    let stderr = capture(stderr_pipe);
    let started = Instant::now();
    let mut cancelled;
    let mut timed_out;
    let status = loop {
        cancelled = cancel.load(Ordering::Relaxed);
        timed_out = started.elapsed() >= limit;
        if cancelled || timed_out {
            terminate(&mut child);
            break child.wait().map_err(|e| e.to_string())?;
        }
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => std::thread::sleep(Duration::from_millis(20)),
            Err(error) => {
                terminate(&mut child);
                let _ = child.wait();
                return Err(error.to_string());
            }
        }
    };
    // Descendants may inherit pipes after the shell exits. Never wait forever.
    let (stdout, stdout_truncated, stdout_incomplete, stdout_read_failed) =
        stdout.finish(Duration::from_secs(1));
    let (stderr, stderr_truncated, stderr_incomplete, stderr_read_failed) =
        stderr.finish(Duration::from_secs(1));
    Ok(CommandResult {
        exit_code: status.code(),
        success: status.success() && !cancelled && !timed_out,
        stdout,
        stderr,
        cancelled,
        timed_out,
        output_truncated: stdout_truncated || stderr_truncated,
        output_incomplete: stdout_incomplete || stderr_incomplete,
        output_read_failed: stdout_read_failed || stderr_read_failed,
    })
}

/// Marker printed to stderr before the real command in an
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

#[cfg(test)]
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
    command_id: String,
    manager: tauri::State<'_, ShellManager>,
) -> std::result::Result<CommandResult, String> {
    let lease = manager.begin(command_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        #[cfg(windows)]
        let shell = env::var_os("COMSPEC").unwrap_or_else(|| "cmd.exe".into());
        #[cfg(not(windows))]
        let shell = env::var_os("SHELL").unwrap_or_else(|| "/bin/sh".into());
        let result = run_shell_controlled(
            &shell,
            !cfg!(windows),
            &command,
            &cwd,
            &lease.cancel,
            RUNTIME_LIMIT,
        )?;
        #[cfg(not(windows))]
        let result = CommandResult {
            stderr: strip_interactive_prelude(&result.stderr),
            ..result
        };
        drop(lease);
        Ok(result)
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub fn cancel_system_command(
    command_id: String,
    manager: tauri::State<'_, ShellManager>,
) -> std::result::Result<(), String> {
    manager.cancel(&command_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // execute() reads the SHELL environment variable, so tests that change it
    // must serialize with every other test that runs commands.
    static SHELL_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn manager_rejects_concurrent_commands_and_isolates_cancellation() {
        let manager = ShellManager::default();
        let lease = manager.begin("first".into()).unwrap();
        assert!(manager.begin("second".into()).is_err());
        manager.cancel("unrelated").unwrap();
        assert!(!lease.cancel.load(Ordering::Relaxed));
        manager.cancel("first").unwrap();
        assert!(lease.cancel.load(Ordering::Relaxed));
        drop(lease);
        let next = manager.begin("second".into()).unwrap();
        manager.cancel("first").unwrap();
        assert!(!next.cancel.load(Ordering::Relaxed));
    }

    #[test]
    fn manager_applies_cancellation_received_before_registration() {
        let manager = ShellManager::default();
        manager.cancel("early").unwrap();
        let lease = manager.begin("early".into()).unwrap();
        assert!(lease.cancel.load(Ordering::Relaxed));
        drop(lease);
        let next = manager.begin("different".into()).unwrap();
        assert!(!next.cancel.load(Ordering::Relaxed));
    }

    #[test]
    fn early_cancellations_are_bounded_and_duplicates_do_not_evict_other_ids() {
        let manager = ShellManager::default();
        for index in 0..100 {
            manager.cancel(&format!("id-{index}")).unwrap();
        }
        for _ in 0..100 {
            manager.cancel("id-99").unwrap();
        }
        assert_eq!(manager.0.lock().unwrap().early_cancellations.len(), 64);
        let evicted = manager.begin("id-0".into()).unwrap();
        assert!(!evicted.cancel.load(Ordering::Relaxed));
        drop(evicted);
        let retained = manager.begin("id-36".into()).unwrap();
        assert!(retained.cancel.load(Ordering::Relaxed));
    }

    #[test]
    fn manager_rejects_empty_and_oversized_command_ids() {
        let manager = ShellManager::default();
        for id in [String::new(), "x".repeat(129)] {
            assert!(manager.cancel(&id).is_err());
            assert!(manager.begin(id).is_err());
        }
        assert!(manager.0.lock().unwrap().early_cancellations.is_empty());
        assert!(manager.begin("valid".into()).is_ok());
    }

    #[test]
    fn command_lease_releases_busy_state_when_worker_panics() {
        let manager = ShellManager::default();
        let lease = manager.begin("failing".into()).unwrap();
        let thread = std::thread::spawn(move || {
            let _lease = lease;
            panic!("simulated worker failure");
        });
        assert!(thread.join().is_err());
        assert!(manager.begin("next".into()).is_ok());
    }

    #[test]
    fn command_lease_releases_busy_state_after_spawn_failure() {
        let manager = ShellManager::default();
        let lease = manager.begin("missing-shell".into()).unwrap();
        let temp = tempfile::tempdir().unwrap();
        assert!(run_shell_controlled(
            temp.path().join("missing-shell").as_os_str(),
            false,
            "echo test",
            temp.path().to_str().unwrap(),
            &lease.cancel,
            Duration::from_secs(1)
        )
        .is_err());
        drop(lease);
        assert!(manager.begin("next".into()).is_ok());
    }

    #[test]
    fn already_cancelled_command_never_spawns_its_shell() {
        let temp = tempfile::tempdir().unwrap();
        // A missing executable would fail if spawning were attempted.
        let result = run_shell_controlled(
            temp.path().join("missing-shell").as_os_str(),
            false,
            "echo destructive-side-effect",
            temp.path().to_str().unwrap(),
            &AtomicBool::new(true),
            Duration::from_secs(1),
        )
        .unwrap();
        assert!(result.cancelled);
        assert!(!result.success);
        assert_eq!(result.exit_code, None);
        assert!(result.stdout.is_empty());
        assert!(result.stderr.is_empty());
    }

    #[test]
    fn capture_bounds_output_but_drains_the_stream() {
        let captured = capture(std::io::Cursor::new(vec![b'x'; OUTPUT_LIMIT + 100]));
        let (text, truncated, incomplete, _) = captured.finish(Duration::from_secs(2));
        assert_eq!(text.len(), OUTPUT_LIMIT);
        assert!(truncated);
        assert!(!incomplete);
    }

    #[test]
    fn capture_retains_partial_output_when_a_pipe_remains_open() {
        struct HeldPipe {
            first: bool,
            waiting: std::sync::mpsc::Sender<()>,
            release: std::sync::mpsc::Receiver<()>,
        }
        impl Read for HeldPipe {
            fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
                if self.first {
                    self.first = false;
                    buffer[..7].copy_from_slice(b"partial");
                    return Ok(7);
                }
                let _ = self.waiting.send(());
                let _ = self.release.recv();
                Ok(0)
            }
        }
        let (waiting, ready) = std::sync::mpsc::channel();
        let (release, held) = std::sync::mpsc::channel();
        let capture = capture(HeldPipe {
            first: true,
            waiting,
            release: held,
        });
        ready.recv_timeout(Duration::from_secs(2)).unwrap();
        let (text, truncated, incomplete, read_failed) = capture.finish(Duration::from_millis(10));
        assert!(!read_failed);
        release.send(()).unwrap();
        assert_eq!(text, "partial");
        assert!(!truncated);
        assert!(incomplete);
    }

    #[test]
    fn capture_reports_read_errors_separately_from_truncation() {
        struct BrokenPipe;
        impl Read for BrokenPipe {
            fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
                Err(std::io::Error::other("read failed"))
            }
        }
        let (text, truncated, incomplete, read_failed) =
            capture(BrokenPipe).finish(Duration::from_secs(2));
        assert!(read_failed);
        assert!(text.is_empty());
        assert!(!truncated);
        assert!(incomplete);
    }

    #[cfg(unix)]
    #[test]
    fn nonblocking_capture_releases_reader_with_writer_still_open() {
        use std::io::Write;
        let (reader, mut writer) = std::os::unix::net::UnixStream::pair().unwrap();
        nonblocking(&reader).unwrap();
        writer.write_all(b"retained output").unwrap();
        let capture = capture(reader);
        let output = capture.output.clone();
        let deadline = Instant::now() + Duration::from_secs(2);
        while output.lock().unwrap().bytes.is_empty() {
            assert!(Instant::now() < deadline, "reader did not capture data");
            std::thread::sleep(Duration::from_millis(5));
        }
        let (text, truncated, incomplete, _) = capture.finish(Duration::from_millis(10));
        assert_eq!(text, "retained output");
        assert!(!truncated);
        assert!(incomplete);
        while Arc::strong_count(&output) != 1 {
            assert!(Instant::now() < deadline, "reader did not terminate");
            std::thread::sleep(Duration::from_millis(5));
        }
        // Writer intentionally remains alive until after reader termination.
        drop(writer);
    }

    #[cfg(windows)]
    #[test]
    fn windows_pipe_polling_distinguishes_idle_data_and_eof() {
        use std::io::Write;
        let (reader, mut writer) = std::io::pipe().unwrap();
        let mut reader = PollingPipe(reader);
        let mut buffer = [0; 32];
        assert_eq!(
            reader.read(&mut buffer).unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock
        );
        writer.write_all(b"output").unwrap();
        let count = reader.read(&mut buffer).unwrap();
        assert_eq!(&buffer[..count], b"output");
        drop(writer);
        assert_eq!(reader.read(&mut buffer).unwrap(), 0);
    }

    #[cfg(windows)]
    #[test]
    fn windows_capture_releases_reader_with_writer_still_open() {
        use std::io::Write;
        let (reader, mut writer) = std::io::pipe().unwrap();
        writer.write_all(b"retained output").unwrap();
        let capture = capture(PollingPipe(reader));
        let output = capture.output.clone();
        let deadline = Instant::now() + Duration::from_secs(2);
        while output.lock().unwrap().bytes.is_empty() {
            assert!(Instant::now() < deadline, "reader did not capture data");
            std::thread::sleep(Duration::from_millis(5));
        }
        let (text, truncated, incomplete, _) = capture.finish(Duration::from_millis(10));
        assert_eq!(text, "retained output");
        assert!(!truncated);
        assert!(incomplete);
        while Arc::strong_count(&output) != 1 {
            assert!(Instant::now() < deadline, "reader did not terminate");
            std::thread::sleep(Duration::from_millis(5));
        }
        drop(writer);
    }

    #[cfg(unix)]
    #[test]
    fn timeout_terminates_a_shell_and_its_waited_child() {
        let dir = tempfile::tempdir().unwrap();
        let started = Instant::now();
        let result = run_shell_controlled(
            std::ffi::OsStr::new("/bin/sh"),
            false,
            "sleep 30 & wait",
            dir.path().to_str().unwrap(),
            &AtomicBool::new(false),
            Duration::from_millis(100),
        )
        .unwrap();
        assert!(result.timed_out);
        assert!(!result.success);
        assert!(started.elapsed() < Duration::from_secs(3));
    }

    #[cfg(unix)]
    #[test]
    fn cancellation_terminates_a_running_command() {
        let dir = tempfile::tempdir().unwrap();
        let cancel = Arc::new(AtomicBool::new(false));
        let trigger = cancel.clone();
        let thread = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(100));
            trigger.store(true, Ordering::Relaxed);
        });
        let result = run_shell_controlled(
            std::ffi::OsStr::new("/bin/sh"),
            false,
            "sleep 30",
            dir.path().to_str().unwrap(),
            &cancel,
            Duration::from_secs(5),
        )
        .unwrap();
        thread.join().unwrap();
        assert!(result.cancelled);
        assert!(!result.success);
        assert!(!result.timed_out);
    }

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

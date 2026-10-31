use crate::{
    config,
    error::{FsError, Result},
    filesystem::{Entry, Listing},
};
use base64::{engine::general_purpose::STANDARD_NO_PAD, Engine};
use serde::{Deserialize, Serialize};
use ssh2::{
    CheckResult, FileStat, HostKeyType, KnownHostFileKind, KnownHostKeyFormat, Session, Sftp,
};
use std::{
    collections::HashMap,
    fs,
    net::{TcpStream, ToSocketAddrs},
    path::Path,
    sync::{Arc, Mutex},
    time::Duration,
};

const MAX_ENTRIES: usize = 100_000;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub directory: String,
    pub key_path: Option<String>,
}

impl Profile {
    pub fn validate(&self) -> Result<()> {
        if uuid::Uuid::parse_str(&self.id).is_err()
            || self.name.trim().is_empty()
            || self.name.len() > 200
            || self.host.is_empty()
            || self.host.len() > 253
            || self.host.chars().any(|c| {
                c.is_whitespace() || c.is_control() || matches!(c, '/' | '\\' | '@' | '[' | ']')
            })
            || self.port == 0
            || self.username.is_empty()
            || self.username.len() > 200
            || self.username.chars().any(char::is_control)
        {
            return Err(FsError::new(
                "invalid_connection",
                "Enter a name, hostname or IP, port, and username",
            ));
        }
        if !self.directory.is_empty() {
            remote_path(&self.directory)?;
        }
        if self
            .key_path
            .as_ref()
            .is_some_and(|path| path.contains('\0'))
        {
            return Err(FsError::new(
                "invalid_connection",
                "Invalid private key path",
            ));
        }
        Ok(())
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Auth {
    Agent,
    Key,
    Password,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectRequest {
    pub profile: Profile,
    pub auth: Auth,
    pub password: Option<String>,
    pub passphrase: Option<String>,
    pub trust_fingerprint: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum ConnectResult {
    HostKey {
        fingerprint: String,
    },
    Connected {
        id: String,
        path: String,
        name: String,
    },
}

pub struct Connection {
    pub sftp: Sftp,
    _session: Session,
}

#[cfg(all(test, unix))]
impl Connection {
    pub(crate) fn socket_for_test(&self) -> std::os::fd::RawFd {
        use std::os::fd::AsRawFd;
        self._session.as_raw_fd()
    }
}

#[derive(Default)]
pub struct Manager {
    sessions: Mutex<HashMap<String, Arc<Mutex<Connection>>>>,
    known_hosts_lock: Mutex<()>,
}

impl Manager {
    pub fn get(&self, id: &str) -> Result<Arc<Mutex<Connection>>> {
        self.sessions
            .lock()
            .map_err(lock_error)?
            .get(id)
            .cloned()
            .ok_or_else(|| {
                FsError::new(
                    "sftp_disconnected",
                    "SFTP connection is not open. Use Connections to reconnect.",
                )
            })
    }
    pub(crate) fn connect(
        &self,
        request: ConnectRequest,
        known_path: &Path,
        user_known_path: Option<&Path>,
    ) -> Result<ConnectResult> {
        let profile = &request.profile;
        profile.validate()?;
        let addresses = (profile.host.as_str(), profile.port)
            .to_socket_addrs()
            .map_err(|e| FsError::new("sftp_connect", e.to_string()))?;
        let mut stream = None;
        for address in addresses.take(8) {
            if let Ok(socket) = TcpStream::connect_timeout(&address, Duration::from_secs(10)) {
                stream = Some(socket);
                break;
            }
        }
        let stream =
            stream.ok_or_else(|| FsError::new("sftp_connect", "Unable to reach the SSH server"))?;
        stream
            .set_read_timeout(Some(Duration::from_secs(15)))
            .map_err(|e| FsError::new("sftp_connect", e.to_string()))?;
        stream
            .set_write_timeout(Some(Duration::from_secs(15)))
            .map_err(|e| FsError::new("sftp_connect", e.to_string()))?;
        let mut session = Session::new().map_err(error)?;
        session.set_timeout(15_000);
        session.set_tcp_stream(stream);
        session.handshake().map_err(error)?;
        let fingerprint = format!(
            "SHA256:{}",
            STANDARD_NO_PAD.encode(session.host_key_hash(ssh2::HashType::Sha256).ok_or_else(
                || FsError::new(
                    "sftp_host_key",
                    "Server did not provide a host key fingerprint"
                )
            )?)
        );
        let (key, key_type) = session
            .host_key()
            .ok_or_else(|| FsError::new("sftp_host_key", "Server did not provide a host key"))?;
        let key = key.to_vec();
        let _host_guard = self.known_hosts_lock.lock().map_err(lock_error)?;
        let trusted = verify_host(&session, profile, &key, known_path, user_known_path)?;
        if !trusted && request.trust_fingerprint.as_deref() != Some(&fingerprint) {
            if request.trust_fingerprint.is_some() {
                return Err(FsError::new(
                    "sftp_host_key_changed",
                    "The server key changed after confirmation. Connection blocked.",
                ));
            }
            return Ok(ConnectResult::HostKey { fingerprint });
        }
        match request.auth {
            Auth::Agent => session.userauth_agent(&profile.username).map_err(error)?,
            Auth::Key => {
                let path = profile
                    .key_path
                    .as_ref()
                    .filter(|p| !p.is_empty())
                    .ok_or_else(|| {
                        FsError::new("invalid_connection", "Choose a private key file")
                    })?;
                session
                    .userauth_pubkey_file(
                        &profile.username,
                        None,
                        Path::new(path),
                        request.passphrase.as_deref(),
                    )
                    .map_err(error)?;
            }
            Auth::Password => session
                .userauth_password(&profile.username, request.password.as_deref().unwrap_or(""))
                .map_err(error)?,
        }
        if !session.authenticated() {
            return Err(FsError::new("sftp_auth", "SSH authentication failed"));
        }
        let sftp = session.sftp().map_err(error)?;
        let start = if profile.directory.is_empty() {
            "."
        } else {
            &profile.directory
        };
        let path = sftp.realpath(Path::new(start)).map_err(error)?;
        let path = path
            .to_str()
            .ok_or_else(|| FsError::new("invalid_path", "Remote path is not valid Unicode"))?
            .to_owned();
        remote_path(&path)?;
        if !sftp.stat(Path::new(&path)).map_err(error)?.is_dir() {
            return Err(FsError::new(
                "invalid_path",
                "Starting directory is not a folder",
            ));
        }
        if !trusted {
            save_host(&session, profile, &key, key_type, known_path)?;
        }
        self.sessions.lock().map_err(lock_error)?.insert(
            profile.id.clone(),
            Arc::new(Mutex::new(Connection {
                sftp,
                _session: session,
            })),
        );
        Ok(ConnectResult::Connected {
            id: profile.id.clone(),
            path,
            name: profile.name.clone(),
        })
    }
}

fn verify_host(
    session: &Session,
    profile: &Profile,
    key: &[u8],
    path: &Path,
    user_path: Option<&Path>,
) -> Result<bool> {
    let mut trusted = false;
    for path in user_path.into_iter().chain(std::iter::once(path)) {
        match fs::metadata(path) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => return Err(FsError::io(e, path)),
            Ok(_) => {}
        }
        let mut hosts = session.known_hosts().map_err(error)?;
        hosts
            .read_file(path, KnownHostFileKind::OpenSSH)
            .map_err(error)?;
        match hosts.check_port(&profile.host, profile.port, key) {
            CheckResult::Match => trusted = true,
            CheckResult::NotFound => {},
            _ => return Err(FsError::new("sftp_host_key_changed", "SSH host key does not match known_hosts. Connection blocked. Verify the server key outside myCmd before updating known_hosts.")),
        }
    }
    Ok(trusted)
}

fn save_host(
    session: &Session,
    profile: &Profile,
    key: &[u8],
    kind: HostKeyType,
    path: &Path,
) -> Result<()> {
    let mut hosts = session.known_hosts().map_err(error)?;
    if path.exists() {
        hosts
            .read_file(path, KnownHostFileKind::OpenSSH)
            .map_err(error)?;
    }
    let host = if profile.port == 22 {
        profile.host.clone()
    } else {
        format!("[{}]:{}", profile.host, profile.port)
    };
    let format = match kind {
        HostKeyType::Rsa => KnownHostKeyFormat::SshRsa,
        HostKeyType::Dss => KnownHostKeyFormat::SshDss,
        HostKeyType::Ecdsa256 => KnownHostKeyFormat::Ecdsa256,
        HostKeyType::Ecdsa384 => KnownHostKeyFormat::Ecdsa384,
        HostKeyType::Ecdsa521 => KnownHostKeyFormat::Ecdsa521,
        HostKeyType::Ed25519 => KnownHostKeyFormat::Ed25519,
        _ => {
            return Err(FsError::new(
                "sftp_host_key",
                "Unsupported server host key type",
            ))
        }
    };
    hosts.add(&host, key, "myCmd", format).map_err(error)?;
    let parent = path.parent().unwrap();
    fs::create_dir_all(parent).map_err(|e| FsError::io(e, parent))?;
    let temp = tempfile::NamedTempFile::new_in(parent).map_err(|e| FsError::io(e, path))?;
    hosts
        .write_file(temp.path(), KnownHostFileKind::OpenSSH)
        .map_err(error)?;
    temp.as_file()
        .sync_all()
        .map_err(|e| FsError::io(e, path))?;
    temp.persist(path).map_err(|e| FsError::io(e.error, path))?;
    Ok(())
}

pub fn error(error: ssh2::Error) -> FsError {
    let code = match error.code() {
        ssh2::ErrorCode::SFTP(2 | 10) => "not_found",
        ssh2::ErrorCode::SFTP(3) => "permission_denied",
        _ => "sftp_error",
    };
    FsError::new(code, error.to_string())
}
fn lock_error(error: impl std::fmt::Display) -> FsError {
    FsError::new("sftp_error", error.to_string())
}

pub fn remote_path(path: &str) -> Result<()> {
    if !path.starts_with('/')
        || path.len() > 16_384
        || path.contains(['\0', '\\'])
        || path.split('/').any(|p| p == "." || p == "..")
    {
        return Err(FsError::new(
            "invalid_path",
            "Enter an absolute remote path without dot segments or backslashes",
        ));
    }
    Ok(())
}
pub fn remote_name(name: &str) -> Result<()> {
    if name.is_empty()
        || name.len() > 1024
        || matches!(name, "." | "..")
        || name.contains(['/', '\\', '\0'])
    {
        return Err(FsError::new("invalid_path", "Unsafe remote filename"));
    }
    Ok(())
}
pub fn join(parent: &str, name: &str) -> Result<String> {
    remote_path(parent)?;
    remote_name(name)?;
    Ok(format!("{}/{name}", parent.trim_end_matches('/')))
}
pub fn parent(path: &str) -> String {
    path.trim_end_matches('/')
        .rsplit_once('/')
        .map(|(parent, _)| if parent.is_empty() { "/" } else { parent })
        .unwrap_or("/")
        .to_owned()
}
pub fn name(path: &str) -> Result<&str> {
    remote_path(path)?;
    let name = path.trim_end_matches('/').rsplit('/').next().unwrap_or("");
    remote_name(name)?;
    Ok(name)
}

pub fn directory(
    sftp: &Sftp,
    path: &str,
    mut check: impl FnMut() -> Result<()>,
) -> Result<Vec<(String, FileStat)>> {
    remote_path(path)?;
    let mut handle = sftp.opendir(Path::new(path)).map_err(error)?;
    let mut entries = Vec::new();
    loop {
        check()?;
        let item = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| handle.readdir()))
            .map_err(|_| FsError::new("invalid_path", "Remote directory contains filenames that cannot be represented on this platform"))?;
        match item {
            Ok((path, stat)) => {
                let name = path.to_str().ok_or_else(|| {
                    FsError::new("invalid_path", "Remote filename is not Unicode")
                })?;
                if name == "." || name == ".." {
                    continue;
                }
                remote_name(name)?;
                if entries.len() >= MAX_ENTRIES {
                    return Err(FsError::new(
                        "sftp_limit",
                        "Remote directory exceeds 100,000 entries",
                    ));
                }
                entries.push((name.to_owned(), stat));
            }
            // libssh2 reports FILE (-16) when a directory handle reaches EOF.
            Err(e) if e.code() == ssh2::ErrorCode::Session(-16) => break,
            Err(e) => return Err(error(e)),
        }
    }
    Ok(entries)
}

pub fn entry(path: &str, stat: &FileStat) -> Result<Entry> {
    let name = name(path)?.to_owned();
    let kind = stat.file_type();
    let is_dir = stat.is_dir();
    let is_link = kind == ssh2::FileType::Symlink;
    if !is_dir && !is_link && !stat.is_file() {
        return Err(FsError::new(
            "invalid_path",
            "Unsupported remote filesystem object",
        ));
    }
    Ok(Entry {
        extension: name
            .rsplit_once('.')
            .map(|(_, ext)| ext)
            .unwrap_or("")
            .into(),
        hidden: name.starts_with('.'),
        name,
        path: path.into(),
        r#type: if is_dir {
            "directory"
        } else if is_link {
            "symlink"
        } else {
            "file"
        }
        .into(),
        size: if is_dir { 0 } else { stat.size.unwrap_or(0) },
        modified: stat.mtime,
        directory_target: is_dir,
    })
}

#[tauri::command]
pub async fn connect_sftp(
    request: ConnectRequest,
    state: tauri::State<'_, Arc<Manager>>,
) -> Result<ConnectResult> {
    let manager = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let known = config::config_dir().join("sftp_known_hosts");
        let user_known = dirs::home_dir().map(|home| home.join(".ssh/known_hosts"));
        manager.connect(request, &known, user_known.as_deref())
    })
    .await
    .map_err(|e| FsError::new("sftp_error", e.to_string()))?
}
#[tauri::command]
pub async fn disconnect_sftp(id: String, state: tauri::State<'_, Arc<Manager>>) -> Result<()> {
    let manager = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let session = manager.sessions.lock().map_err(lock_error)?.remove(&id);
        drop(session);
        Ok(())
    })
    .await
    .map_err(|e| FsError::new("sftp_error", e.to_string()))?
}
#[tauri::command]
pub async fn list_sftp(
    id: String,
    path: String,
    state: tauri::State<'_, Arc<Manager>>,
) -> Result<Listing> {
    let connection = state.get(&id)?;
    tauri::async_runtime::spawn_blocking(move || {
        remote_path(&path)?;
        let connection = connection.lock().map_err(lock_error)?;
        let sftp = &connection.sftp;
        let canonical = sftp.realpath(Path::new(&path)).map_err(error)?;
        let path = canonical
            .to_str()
            .ok_or_else(|| FsError::new("invalid_path", "Remote path is not Unicode"))?
            .to_owned();
        remote_path(&path)?;
        let mut entries = Vec::new();
        let mut skipped = 0;
        for (name, stat) in directory(sftp, &path, || Ok(()))? {
            let full = join(&path, &name)?;
            match entry(&full, &stat) {
                Ok(mut item) => {
                    if item.r#type == "symlink" {
                        item.directory_target =
                            sftp.stat(Path::new(&full)).is_ok_and(|stat| stat.is_dir());
                    }
                    entries.push(item);
                }
                Err(_) => skipped += 1,
            }
        }
        Ok(Listing {
            parent: if path == "/" {
                None
            } else {
                Some(parent(&path))
            },
            path,
            entries,
            skipped_entries: skipped,
            warnings: Vec::new(),
        })
    })
    .await
    .map_err(|e| FsError::new("sftp_error", e.to_string()))?
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub fn profile() -> Profile {
        Profile {
            id: uuid::Uuid::new_v4().to_string(),
            name: "Test".into(),
            host: "127.0.0.1".into(),
            port: 22,
            username: "test".into(),
            directory: String::new(),
            key_path: None,
        }
    }

    #[test]
    fn validates_connections_and_remote_paths_at_the_boundary() {
        let mut item = profile();
        item.validate().unwrap();
        item.host = "host\0attacker".into();
        assert!(item.validate().is_err());
        for path in ["relative", "/a/../b", "/a/./b", "/a\\b", "/nul\0"] {
            assert!(remote_path(path).is_err());
        }
        for child in [
            "",
            ".",
            "..",
            "../outside",
            "nested/file",
            "nested\\file",
            "nul\0",
        ] {
            assert!(join("/safe", child).is_err());
        }
        assert_eq!(join("/", "日本語 # %.txt").unwrap(), "/日本語 # %.txt");
        assert_eq!(parent("/a/b"), "/a");
        assert_eq!(parent("/a"), "/");
        assert!(name("/").is_err());
    }

    #[test]
    fn known_hosts_require_explicit_trust_and_block_changed_keys() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("known_hosts");
        let session = Session::new().unwrap();
        let profile = profile();
        let key = b"test host key bytes";
        assert!(!verify_host(&session, &profile, key, &path, None).unwrap());
        save_host(&session, &profile, key, HostKeyType::Ed25519, &path).unwrap();
        assert!(verify_host(&session, &profile, key, &path, None).unwrap());
        assert_eq!(
            verify_host(&session, &profile, b"different", &path, None)
                .unwrap_err()
                .code,
            "sftp_host_key_changed"
        );
        let user_path = dir.path().join("user_known_hosts");
        save_host(
            &session,
            &profile,
            b"conflicting",
            HostKeyType::Ed25519,
            &user_path,
        )
        .unwrap();
        assert_eq!(
            verify_host(&session, &profile, key, &path, Some(&user_path))
                .unwrap_err()
                .code,
            "sftp_host_key_changed"
        );
    }

    #[cfg(unix)]
    pub struct Fixture {
        pub dir: tempfile::TempDir,
        pub profile: Profile,
        pub known: std::path::PathBuf,
        pub server: std::process::Child,
    }
    #[cfg(unix)]
    impl Fixture {
        pub fn new() -> Self {
            let dir = tempfile::tempdir().unwrap();
            for key in ["host", "client"] {
                let output = std::process::Command::new("ssh-keygen")
                    .args(["-q", "-t", "ed25519", "-N", "", "-f"])
                    .arg(dir.path().join(key))
                    .output()
                    .unwrap();
                assert!(
                    output.status.success(),
                    "{}",
                    String::from_utf8_lossy(&output.stderr)
                );
            }
            let username = std::process::Command::new("id")
                .arg("-un")
                .output()
                .unwrap();
            let username = String::from_utf8(username.stdout)
                .unwrap()
                .trim()
                .to_owned();
            let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            let port = listener.local_addr().unwrap().port();
            drop(listener);
            let config = format!("Port {port}\nListenAddress 127.0.0.1\nHostKey {}\nAuthorizedKeysFile {}\nPidFile {}\nStrictModes no\nUsePAM no\nPasswordAuthentication no\nPubkeyAuthentication yes\nPermitRootLogin yes\nAllowUsers {username}\nSubsystem sftp internal-sftp\nLogLevel ERROR\n", dir.path().join("host").display(), dir.path().join("client.pub").display(), dir.path().join("sshd.pid").display());
            let config_path = dir.path().join("sshd_config");
            fs::write(&config_path, config).unwrap();
            let log = fs::File::create(dir.path().join("sshd.log")).unwrap();
            let server = std::process::Command::new("/usr/sbin/sshd")
                .args(["-D", "-e", "-f"])
                .arg(config_path)
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(log)
                .spawn()
                .unwrap();
            let mut fixture = Self {
                known: dir.path().join("known_hosts"),
                profile: Profile {
                    port,
                    username,
                    key_path: Some(dir.path().join("client").to_str().unwrap().into()),
                    ..profile()
                },
                dir,
                server,
            };
            for _ in 0..100 {
                if TcpStream::connect(("127.0.0.1", port)).is_ok() {
                    return fixture;
                }
                if fixture.server.try_wait().unwrap().is_some() {
                    break;
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            panic!(
                "Private sshd did not start: {}",
                fs::read_to_string(fixture.dir.path().join("sshd.log")).unwrap()
            );
        }
        pub fn request(&self, fingerprint: Option<String>) -> ConnectRequest {
            ConnectRequest {
                profile: self.profile.clone(),
                auth: Auth::Key,
                password: None,
                passphrase: None,
                trust_fingerprint: fingerprint,
            }
        }
        pub fn connect(&self, manager: &Manager) {
            let result = manager
                .connect(self.request(None), &self.known, None)
                .unwrap();
            let fingerprint = match result {
                ConnectResult::HostKey { fingerprint } => fingerprint,
                _ => panic!("Unknown server must require explicit trust"),
            };
            assert_eq!(
                manager
                    .connect(self.request(Some("SHA256:wrong".into())), &self.known, None)
                    .unwrap_err()
                    .code,
                "sftp_host_key_changed"
            );
            assert!(!self.known.exists());
            let mut denied = self.request(Some(fingerprint.clone()));
            denied.profile.username = "mycmd-no-such-user".into();
            assert!(manager.connect(denied, &self.known, None).is_err());
            assert!(!self.known.exists());
            assert!(matches!(
                manager
                    .connect(self.request(Some(fingerprint)), &self.known, None)
                    .unwrap(),
                ConnectResult::Connected { .. }
            ));
            assert!(matches!(
                manager
                    .connect(self.request(None), &self.known, None)
                    .unwrap(),
                ConnectResult::Connected { .. }
            ));
        }
    }
    #[cfg(unix)]
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = self.server.kill();
            let _ = self.server.wait();
        }
    }
}

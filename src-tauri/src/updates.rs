//! New-release notification. The backend asks GitHub for the latest published
//! release (never drafts or prereleases), checks every field it uses, and tells
//! the window whether a newer version exists, with its notes and the update
//! command for this installation. Nothing is downloaded or installed here.
//!
//! Trust boundary: the window never supplies a URL. The release page and the
//! download links are rebuilt from the validated version and the fixed
//! repository, and must equal what GitHub returned; commands only embed those
//! exact URLs and fixed file names, quoted for the shell, so nothing GitHub
//! sends reaches a command unchecked.
use crate::error::{FsError, Result};
use serde::Serialize;
use serde_json::Value;
use std::{
    cmp::Ordering,
    sync::Mutex,
    time::{Duration, Instant},
};

const RELEASES: &str = "https://github.com/pankleks/myCmd/releases";
const LATEST_RELEASE_API: &str = "https://api.github.com/repos/pankleks/myCmd/releases/latest";
pub const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
const MAX_NOTES: usize = 20_000;
const MAX_RESPONSE: u64 = 1024 * 1024;
const CACHE: Duration = Duration::from_secs(60 * 60);
const TIMEOUT: Duration = Duration::from_secs(10);

/// `x.y.z` with 1–4 digits per part; anything else (such as `-rc.1`) is not a
/// release version.
fn parse_version(value: &str) -> Option<[u32; 3]> {
    let mut parts = value.split('.');
    let mut version = [0; 3];
    for part in &mut version {
        let text = parts.next()?;
        if text.is_empty() || text.len() > 4 || !text.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        *part = text.parse().ok()?;
    }
    parts.next().is_none().then_some(version)
}

/// Ordering of two `x.y.z` versions; None when either is not one.
pub fn compare_versions(a: &str, b: &str) -> Option<Ordering> {
    Some(parse_version(a)?.cmp(&parse_version(b)?))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Target {
    Deb,
    AppImage,
    WindowsSetup,
    Msi,
    Dmg,
}

const TARGETS: [Target; 5] = [
    Target::Deb,
    Target::AppImage,
    Target::WindowsSetup,
    Target::Msi,
    Target::Dmg,
];

impl Target {
    /// The release file name (see `scripts/release-lib.mjs` collect).
    fn asset_name(self, version: &str) -> String {
        match self {
            Target::Deb => format!("linux-x64--myCmd_{version}_amd64.deb"),
            Target::AppImage => format!("linux-x64--myCmd_{version}_amd64.AppImage"),
            Target::WindowsSetup => format!("windows-x64--myCmd_{version}_x64-setup.exe"),
            Target::Msi => format!("windows-x64--myCmd_{version}_x64_en-US.msi"),
            Target::Dmg => format!("macos-arm64--myCmd_{version}_aarch64.dmg"),
        }
    }
    fn id(self) -> &'static str {
        match self {
            Target::Deb => "deb",
            Target::AppImage => "appimage",
            Target::WindowsSetup => "windows-setup",
            Target::Msi => "msi",
            Target::Dmg => "dmg",
        }
    }
}

#[derive(Clone, Debug)]
struct Release {
    version: String,
    name: String,
    published_at: String,
    notes: String,
    notes_truncated: bool,
    url: String,
    downloads: Vec<(Target, String)>,
}

impl Release {
    fn download(&self, target: Target) -> Option<&str> {
        self.downloads
            .iter()
            .find(|(candidate, _)| *candidate == target)
            .map(|(_, url)| url.as_str())
    }
}

fn text(value: &Value, max: usize) -> (String, bool) {
    let text = value.as_str().unwrap_or_default();
    match text.char_indices().nth(max) {
        Some((end, _)) => (text[..end].to_string(), true),
        None => (text.to_string(), false),
    }
}

/// The newer release described by GitHub's "latest release" answer, or None
/// when it is not newer than `current` or does not pass every check. Download
/// URLs are kept only for the exact expected file names at their exact URLs.
fn parse_release(json: &Value, current: &str) -> Option<Release> {
    if json.get("draft") != Some(&Value::Bool(false))
        || json.get("prerelease") != Some(&Value::Bool(false))
    {
        return None;
    }
    let version = json.get("tag_name")?.as_str()?.strip_prefix('v')?;
    if compare_versions(version, current)? != Ordering::Greater {
        return None;
    }
    let url = format!("{RELEASES}/tag/v{version}");
    if json.get("html_url")?.as_str()? != url {
        return None;
    }
    let mut downloads = Vec::new();
    for asset in json
        .get("assets")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let name = asset.get("name").and_then(Value::as_str);
        let link = asset.get("browser_download_url").and_then(Value::as_str);
        for target in TARGETS {
            let expected = target.asset_name(version);
            let expected_url = format!("{RELEASES}/download/v{version}/{expected}");
            if name == Some(expected.as_str()) && link == Some(expected_url.as_str()) {
                downloads.push((target, expected_url));
            }
        }
    }
    let (name, _) = text(json.get("name").unwrap_or(&Value::Null), 200);
    let (published_at, _) = text(json.get("published_at").unwrap_or(&Value::Null), 40);
    let (notes, notes_truncated) = text(json.get("body").unwrap_or(&Value::Null), MAX_NOTES);
    Some(Release {
        name: if name.trim().is_empty() {
            format!("myCmd {version}")
        } else {
            name
        },
        version: version.to_string(),
        published_at,
        notes,
        notes_truncated,
        url,
        downloads,
    })
}

/// How this copy of myCmd was installed.
// Each platform constructs only its own variants.
#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Install {
    Deb,
    /// Path of the running AppImage file.
    AppImage(String),
    LinuxUnknown,
    WindowsSetup,
    Msi,
    WindowsUnknown,
    /// Path of the `.app` bundle to replace.
    Mac(String),
    Other,
}

/// POSIX shell single-quoting.
fn sh(value: &str) -> String {
    format!("'{}'", value.replace('\'', r"'\''"))
}

/// PowerShell single-quoting.
fn ps(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Instruction {
    target: &'static str,
    label: &'static str,
    note: &'static str,
    command: String,
}

fn update_instructions(release: &Release, install: &Install) -> Vec<Instruction> {
    let entry = |target: Target,
                 label: &'static str,
                 note: &'static str,
                 command: &dyn Fn(&str, &str) -> String| {
        release.download(target).map(|url| Instruction {
            target: target.id(),
            label,
            note,
            command: command(url, &target.asset_name(&release.version)),
        })
    };
    let deb = || {
        entry(
            Target::Deb,
            "Debian / Ubuntu (.deb)",
            "Run in a terminal, then restart myCmd.",
            &|url, name| {
                format!(
                    "curl -fLO {} && sudo apt install {} && rm {}",
                    sh(url),
                    sh(&format!("./{name}")),
                    sh(name)
                )
            },
        )
    };
    let app_image = |path: &str| {
        let path = path.to_string();
        entry(
            Target::AppImage,
            "AppImage",
            "Run in a terminal, then restart myCmd. The new file replaces this AppImage.",
            &move |url, _| {
                let next = format!("{path}.new");
                format!(
                    "curl -fL -o {} {} && chmod +x {} && mv -f {} {}",
                    sh(&next),
                    sh(url),
                    sh(&next),
                    sh(&next),
                    sh(&path)
                )
            },
        )
    };
    let windows = |target: Target, label: &'static str| {
        entry(
            target,
            label,
            "Run in PowerShell. The installer replaces this version; close myCmd when it asks.",
            &|url, name| {
                format!(
                    "$f = Join-Path $env:TEMP {}; Invoke-WebRequest {} -OutFile $f; Start-Process $f",
                    ps(name),
                    ps(url)
                )
            },
        )
    };
    let list = match install {
        Install::Deb => vec![deb()],
        Install::AppImage(path) => vec![app_image(path)],
        Install::LinuxUnknown => vec![deb(), app_image("myCmd.AppImage")],
        Install::WindowsSetup => vec![windows(Target::WindowsSetup, "Windows installer (.exe)")],
        Install::Msi => vec![windows(Target::Msi, "Windows installer (.msi)")],
        Install::WindowsUnknown => vec![
            windows(Target::WindowsSetup, "Windows installer (.exe)"),
            windows(Target::Msi, "Windows installer (.msi)"),
        ],
        Install::Mac(bundle) => {
            let bundle = bundle.clone();
            vec![entry(
                Target::Dmg,
                "macOS",
                "Quit myCmd first, then run in Terminal. The package is unsigned, so the command also clears the quarantine flag and signs the app locally, as in the README.",
                &move |url, _| {
                    let app = sh(&bundle);
                    let next = sh(&format!("{bundle}.new"));
                    // A private temp folder, and the old app is removed only after
                    // the new copy is complete: a failed copy keeps a working app.
                    format!(
                        "d=$(mktemp -d) && curl -fL -o \"$d/myCmd.dmg\" {} && hdiutil attach -nobrowse -quiet -mountpoint \"$d/mnt\" \"$d/myCmd.dmg\" && rm -rf {next} && ditto \"$d/mnt/myCmd.app\" {next} && hdiutil detach -quiet \"$d/mnt\" && rm -rf {app} && mv {next} {app} && rm -rf \"$d\" && xattr -cr {app} && codesign --force --deep --sign - {app}",
                        sh(url),
                    )
                },
            )]
        }
        Install::Other => vec![],
    };
    list.into_iter().flatten().collect()
}

/// The `.app` bundle of a macOS executable path, when it sits in a normal
/// install location (never a mounted DMG or a translocated copy).
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn mac_bundle(executable: &str) -> String {
    let fallback = "/Applications/myCmd.app".to_string();
    let Some(end) = executable.find(".app/") else {
        return fallback;
    };
    let bundle = &executable[..end + 4];
    let name_ok = |name: &str| !name.is_empty() && !name.contains('/') && name.ends_with(".app");
    let installed = match bundle.strip_prefix("/Applications/") {
        Some(name) => name_ok(name),
        None => bundle
            .strip_prefix("/Users/")
            .and_then(|rest| rest.split_once("/Applications/"))
            .is_some_and(|(user, name)| !user.is_empty() && !user.contains('/') && name_ok(name)),
    };
    if installed {
        bundle.to_string()
    } else {
        fallback
    }
}

/// NSIS installs per user under %LOCALAPPDATA%, MSI under Program Files.
#[cfg_attr(not(windows), allow(dead_code))]
fn windows_install(
    executable: &std::path::Path,
    local_app_data: Option<&std::path::Path>,
    program_files: &[std::path::PathBuf],
) -> Install {
    if local_app_data.is_some_and(|folder| executable.starts_with(folder)) {
        Install::WindowsSetup
    } else if program_files
        .iter()
        .any(|folder| executable.starts_with(folder))
    {
        Install::Msi
    } else {
        Install::WindowsUnknown
    }
}

pub fn detect_install() -> Install {
    #[allow(unused_variables)]
    let executable = std::env::current_exe().ok();
    #[cfg(target_os = "linux")]
    {
        if let Some(path) = std::env::var_os("APPIMAGE")
            .map(std::path::PathBuf::from)
            .filter(|path| path.is_absolute())
            .and_then(|path| path.to_str().map(str::to_string))
        {
            return Install::AppImage(path);
        }
        let packaged = executable.is_some_and(|executable| {
            std::process::Command::new("dpkg")
                .arg("-S")
                .arg(executable)
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
                .is_ok_and(|status| status.success())
        });
        if packaged {
            Install::Deb
        } else {
            Install::LinuxUnknown
        }
    }
    #[cfg(windows)]
    {
        let Some(executable) = executable else {
            return Install::WindowsUnknown;
        };
        let local = std::env::var_os("LOCALAPPDATA").map(std::path::PathBuf::from);
        let program_files: Vec<_> = ["ProgramFiles", "ProgramW6432", "ProgramFiles(x86)"]
            .into_iter()
            .filter_map(std::env::var_os)
            .map(std::path::PathBuf::from)
            .collect();
        windows_install(&executable, local.as_deref(), &program_files)
    }
    #[cfg(target_os = "macos")]
    {
        Install::Mac(mac_bundle(
            &executable
                .and_then(|path| path.to_str().map(str::to_string))
                .unwrap_or_default(),
        ))
    }
    #[cfg(not(any(target_os = "linux", windows, target_os = "macos")))]
    {
        Install::Other
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AvailableUpdate {
    version: String,
    name: String,
    published_at: String,
    notes: String,
    notes_truncated: bool,
    instructions: Vec<Instruction>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCheck {
    current: String,
    update: Option<AvailableUpdate>,
}

#[derive(Default)]
struct Cache {
    release: Option<Release>,
    checked_at: Option<Instant>,
}

/// Checker state: the last validated release, cached for an hour.
#[derive(Default)]
pub struct Updates {
    cache: Mutex<Cache>,
    /// Concurrent checks wait for the running one and use its result.
    fetching: Mutex<()>,
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

impl Updates {
    /// `fetch` returns the response body, or None when there is no release.
    fn check_with(
        &self,
        current: &str,
        force: bool,
        now: Instant,
        install: impl FnOnce() -> Install,
        fetch: impl FnOnce() -> Result<Option<String>>,
    ) -> Result<UpdateCheck> {
        let _fetching = lock(&self.fetching);
        let fresh = lock(&self.cache)
            .checked_at
            .is_some_and(|at| now.saturating_duration_since(at) < CACHE);
        if force || !fresh {
            let release = match fetch()? {
                Some(body) => {
                    let json: Value = serde_json::from_str(&body).map_err(|_| {
                        FsError::new(
                            "update_check_failed",
                            "GitHub sent release information that could not be read",
                        )
                    })?;
                    parse_release(&json, current)
                }
                None => None,
            };
            *lock(&self.cache) = Cache {
                release,
                checked_at: Some(now),
            };
        }
        let release = lock(&self.cache).release.clone();
        Ok(UpdateCheck {
            current: current.to_string(),
            update: release.map(|release| AvailableUpdate {
                instructions: update_instructions(&release, &install()),
                version: release.version,
                name: release.name,
                published_at: release.published_at,
                notes: release.notes,
                notes_truncated: release.notes_truncated,
            }),
        })
    }

    pub fn check(&self, force: bool) -> Result<UpdateCheck> {
        self.check_with(
            CURRENT_VERSION,
            force,
            Instant::now(),
            detect_install,
            fetch_latest,
        )
    }

    /// The validated page of the release found by the last check.
    fn release_page(&self) -> Result<String> {
        lock(&self.cache)
            .release
            .as_ref()
            .map(|release| release.url.clone())
            .ok_or_else(|| FsError::new("no_update", "No newer release has been found"))
    }
}

fn fetch_latest() -> Result<Option<String>> {
    let failed = |message: String| FsError::new("update_check_failed", message);
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(TIMEOUT))
        .http_status_as_error(false)
        .user_agent(format!("myCmd/{CURRENT_VERSION}"))
        .build()
        .into();
    let mut response = agent
        .get(LATEST_RELEASE_API)
        .header("Accept", "application/vnd.github+json")
        .call()
        .map_err(|error| failed(format!("Could not reach GitHub: {error}")))?;
    // No published release yet.
    if response.status().as_u16() == 404 {
        return Ok(None);
    }
    if !response.status().is_success() {
        return Err(failed(format!(
            "GitHub answered {}",
            response.status().as_u16()
        )));
    }
    response
        .body_mut()
        .with_config()
        .limit(MAX_RESPONSE)
        .read_to_string()
        .map(Some)
        .map_err(|error| match error {
            ureq::Error::BodyExceedsLimit(_) => {
                failed("The release information is too large".into())
            }
            error => failed(format!("Could not read the answer from GitHub: {error}")),
        })
}

#[tauri::command]
pub async fn check_for_update(
    force: bool,
    state: tauri::State<'_, std::sync::Arc<Updates>>,
) -> Result<UpdateCheck> {
    let updates = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || updates.check(force))
        .await
        .map_err(|e| FsError::new("io_error", e.to_string()))?
}

#[tauri::command]
pub fn open_release_page(state: tauri::State<'_, std::sync::Arc<Updates>>) -> Result<()> {
    let url = state.release_page()?;
    open::that_detached(&url).map_err(|e| FsError::new("io_error", e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn release_json(version: &str) -> Value {
        let asset = |name: String| {
            json!({
                "name": name,
                "browser_download_url": format!("{RELEASES}/download/v{version}/{name}"),
            })
        };
        json!({
            "tag_name": format!("v{version}"),
            "name": format!("myCmd v{version}"),
            "draft": false,
            "prerelease": false,
            "html_url": format!("{RELEASES}/tag/v{version}"),
            "published_at": "2026-12-01T10:00:00Z",
            "body": "# myCmd\n\n- New things",
            "assets": TARGETS
                .iter()
                .map(|target| asset(target.asset_name(version)))
                .chain([asset("SHA256SUMS.txt".into())])
                .collect::<Vec<_>>(),
        })
    }

    #[test]
    fn compares_only_plain_release_versions() {
        assert_eq!(compare_versions("0.2.10", "0.2.9"), Some(Ordering::Greater));
        assert_eq!(compare_versions("1.0.0", "1.0.0"), Some(Ordering::Equal));
        assert_eq!(compare_versions("0.9.9", "1.0.0"), Some(Ordering::Less));
        for invalid in [
            "1.0",
            "1.0.0.0",
            "1.0.0-rc.1",
            "v1.0.0",
            "1..0",
            "12345.0.0",
            "",
        ] {
            assert_eq!(compare_versions(invalid, "1.0.0"), None, "{invalid}");
        }
    }

    #[test]
    fn accepts_a_newer_release_and_keeps_only_expected_downloads() {
        let mut json = release_json("0.3.0");
        json["assets"]
            .as_array_mut()
            .unwrap()
            .push(json!({"name": "linux-x64--myCmd_0.3.0_amd64.deb", "browser_download_url": "https://evil.example/x.deb"}));
        let release = parse_release(&json, "0.2.17").unwrap();
        assert_eq!(release.version, "0.3.0");
        assert_eq!(release.name, "myCmd v0.3.0");
        assert_eq!(release.url, format!("{RELEASES}/tag/v0.3.0"));
        assert_eq!(release.downloads.len(), 5);
        assert_eq!(
            release.download(Target::Deb),
            Some(format!("{RELEASES}/download/v0.3.0/linux-x64--myCmd_0.3.0_amd64.deb").as_str())
        );
        assert!(!release.notes_truncated);
    }

    #[test]
    fn rejects_releases_that_fail_any_check() {
        assert!(parse_release(&release_json("0.2.17"), "0.2.17").is_none());
        assert!(parse_release(&release_json("0.2.16"), "0.2.17").is_none());
        assert!(parse_release(&release_json("0.3.0"), "0.3.0-rc.1").is_none());
        for (field, value) in [
            ("draft", json!(true)),
            ("prerelease", json!(true)),
            ("draft", Value::Null),
            ("tag_name", json!("0.3.0")),
            ("tag_name", json!("v0.3.0-rc.1")),
            ("html_url", json!("https://evil.example/v0.3.0")),
        ] {
            let mut json = release_json("0.3.0");
            json[field] = value;
            assert!(parse_release(&json, "0.2.17").is_none(), "{field}");
        }
        let mut json = release_json("0.3.0");
        json["assets"][0]["browser_download_url"] = json!("https://evil.example/file");
        assert!(parse_release(&json, "0.2.17")
            .unwrap()
            .download(Target::Deb)
            .is_none());
    }

    #[test]
    fn shortens_long_notes_and_names_untitled_releases() {
        let mut json = release_json("0.3.0");
        json["body"] = json!("é".repeat(MAX_NOTES + 5));
        json["name"] = json!(" ");
        let release = parse_release(&json, "0.2.17").unwrap();
        assert_eq!(release.notes.chars().count(), MAX_NOTES);
        assert!(release.notes_truncated);
        assert_eq!(release.name, "myCmd 0.3.0");
    }

    fn commands(install: Install) -> Vec<(String, String)> {
        let release = parse_release(&release_json("0.3.0"), "0.2.17").unwrap();
        update_instructions(&release, &install)
            .into_iter()
            .map(|item| (item.target.to_string(), item.command))
            .collect()
    }

    #[test]
    fn builds_the_command_for_each_installation() {
        let url = |name: &str| format!("'{RELEASES}/download/v0.3.0/{name}'");
        assert_eq!(
            commands(Install::Deb),
            [(
                "deb".into(),
                format!(
                    "curl -fLO {} && sudo apt install './linux-x64--myCmd_0.3.0_amd64.deb' && rm 'linux-x64--myCmd_0.3.0_amd64.deb'",
                    url("linux-x64--myCmd_0.3.0_amd64.deb")
                )
            )]
        );
        assert_eq!(
            commands(Install::AppImage("/home/me/Apps/it's.AppImage".into())),
            [(
                "appimage".into(),
                format!(
                    "curl -fL -o '/home/me/Apps/it'\\''s.AppImage.new' {} && chmod +x '/home/me/Apps/it'\\''s.AppImage.new' && mv -f '/home/me/Apps/it'\\''s.AppImage.new' '/home/me/Apps/it'\\''s.AppImage'",
                    url("linux-x64--myCmd_0.3.0_amd64.AppImage")
                )
            )]
        );
        assert_eq!(
            commands(Install::WindowsSetup),
            [(
                "windows-setup".into(),
                format!(
                    "$f = Join-Path $env:TEMP 'windows-x64--myCmd_0.3.0_x64-setup.exe'; Invoke-WebRequest {} -OutFile $f; Start-Process $f",
                    url("windows-x64--myCmd_0.3.0_x64-setup.exe")
                )
            )]
        );
        let mac = commands(Install::Mac("/Users/o'neil/Applications/myCmd.app".into()));
        assert_eq!(mac.len(), 1);
        assert!(mac[0]
            .1
            .contains(&url("macos-arm64--myCmd_0.3.0_aarch64.dmg")));
        // The old app is removed only after the new copy succeeded.
        assert!(mac[0].1.contains(
            "ditto \"$d/mnt/myCmd.app\" '/Users/o'\\''neil/Applications/myCmd.app.new' && hdiutil detach -quiet \"$d/mnt\" && rm -rf '/Users/o'\\''neil/Applications/myCmd.app' && mv '/Users/o'\\''neil/Applications/myCmd.app.new' '/Users/o'\\''neil/Applications/myCmd.app'"
        ));
        assert!(!mac[0].1.contains("/tmp/"));
        assert!(mac[0].1.ends_with(
            "codesign --force --deep --sign - '/Users/o'\\''neil/Applications/myCmd.app'"
        ));
        let targets = |install| {
            commands(install)
                .into_iter()
                .map(|(target, _)| target)
                .collect::<Vec<_>>()
        };
        assert_eq!(targets(Install::Msi), ["msi"]);
        assert_eq!(targets(Install::LinuxUnknown), ["deb", "appimage"]);
        assert_eq!(targets(Install::WindowsUnknown), ["windows-setup", "msi"]);
        assert!(targets(Install::Other).is_empty());
    }

    #[test]
    fn skips_commands_whose_file_is_missing() {
        let mut json = release_json("0.3.0");
        json["assets"] = json!([]);
        let release = parse_release(&json, "0.2.17").unwrap();
        assert!(update_instructions(&release, &Install::Deb).is_empty());
    }

    #[test]
    fn replaces_only_bundles_in_application_folders() {
        assert_eq!(
            mac_bundle("/Applications/myCmd.app/Contents/MacOS/mycmd"),
            "/Applications/myCmd.app"
        );
        assert_eq!(
            mac_bundle("/Users/me/Applications/My Cmd.app/Contents/MacOS/mycmd"),
            "/Users/me/Applications/My Cmd.app"
        );
        for other in [
            "/Volumes/myCmd/myCmd.app/Contents/MacOS/mycmd",
            "/private/var/folders/x/AppTranslocation/y/d/myCmd.app/Contents/MacOS/mycmd",
            "/Applications/Tools/myCmd.app/Contents/MacOS/mycmd",
            "/usr/local/bin/mycmd",
        ] {
            assert_eq!(mac_bundle(other), "/Applications/myCmd.app", "{other}");
        }
    }

    #[test]
    fn tells_windows_installers_apart_by_folder() {
        use std::path::{Path, PathBuf};
        let local = Path::new(r"C:\Users\me\AppData\Local");
        let program_files = [PathBuf::from(r"C:\Program Files")];
        let install = |exe: &str| windows_install(Path::new(exe), Some(local), &program_files);
        if cfg!(windows) {
            assert_eq!(
                install(r"C:\Users\me\AppData\Local\myCmd\mycmd.exe"),
                Install::WindowsSetup
            );
            assert_eq!(install(r"C:\Program Files\myCmd\mycmd.exe"), Install::Msi);
        }
        assert_eq!(install(r"D:\tools\mycmd.exe"), Install::WindowsUnknown);
        assert_eq!(
            windows_install(
                Path::new("/home/me/.local/myCmd/mycmd"),
                Some(Path::new("/home/me/.local")),
                &[]
            ),
            Install::WindowsSetup
        );
    }

    #[test]
    fn caches_the_answer_for_an_hour_unless_forced() {
        let updates = Updates::default();
        let start = Instant::now();
        let body = release_json("0.3.0").to_string();
        let mut calls = 0;
        let mut check = |force, now| {
            updates
                .check_with(
                    "0.2.17",
                    force,
                    now,
                    || Install::Deb,
                    || {
                        calls += 1;
                        Ok(Some(body.clone()))
                    },
                )
                .unwrap()
        };
        let first = check(false, start);
        assert_eq!(first.current, "0.2.17");
        let update = first.update.unwrap();
        assert_eq!(update.version, "0.3.0");
        assert_eq!(update.instructions.len(), 1);
        check(false, start + Duration::from_secs(60));
        check(true, start + Duration::from_secs(120));
        check(false, start + CACHE + Duration::from_secs(121));
        assert_eq!(calls, 3);
        assert_eq!(
            updates.release_page().unwrap(),
            format!("{RELEASES}/tag/v0.3.0")
        );
    }

    /// `cargo test live_github_release -- --ignored` (needs the network).
    #[test]
    #[ignore]
    fn live_github_release_passes_every_check() {
        let body = fetch_latest().unwrap().expect("a published release");
        let release = parse_release(&serde_json::from_str(&body).unwrap(), "0.0.1").unwrap();
        assert_eq!(release.downloads.len(), TARGETS.len(), "{release:?}");
    }

    #[test]
    fn reports_no_update_without_a_release_and_fails_on_unreadable_answers() {
        let updates = Updates::default();
        let check = |fetch: Result<Option<String>>| {
            updates.check_with("0.2.17", true, Instant::now(), || Install::Deb, || fetch)
        };
        assert!(check(Ok(None)).unwrap().update.is_none());
        assert_eq!(updates.release_page().unwrap_err().code, "no_update");
        assert_eq!(
            check(Ok(Some("not json".into()))).unwrap_err().code,
            "update_check_failed"
        );
        assert_eq!(
            check(Err(FsError::new("update_check_failed", "offline")))
                .unwrap_err()
                .message,
            "offline"
        );
    }
}

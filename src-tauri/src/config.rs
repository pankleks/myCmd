use crate::error::{FsError, Result};
use serde::{Deserialize, Deserializer, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};

pub const CONFIG_VERSION: u32 = 1;
pub const CONFIG_FILE_NAME: &str = "config.json";
pub const CONFIG_ENV_OVERRIDE: &str = "MYCMD_CONFIG_DIR";
pub const MIN_FILE_FONT_SIZE: u32 = 12;
pub const MAX_FILE_FONT_SIZE: u32 = 24;
pub const MIN_COLUMN_WEIGHT: f64 = 20.0;
pub const MAX_COLUMN_WEIGHT: f64 = 2000.0;
pub const COLUMN_COUNT: usize = 4;

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ColumnWeights {
    pub left: Option<[f64; COLUMN_COUNT]>,
    pub right: Option<[f64; COLUMN_COUNT]>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct LaunchRules {
    pub run_in_terminal: Vec<String>,
    pub ignore: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct LaunchConfig {
    pub linux: Option<LaunchRules>,
    pub windows: Option<LaunchRules>,
    pub macos: Option<LaunchRules>,
}

fn default_launch_rules() -> LaunchRules {
    // No fallback between sections: a missing section means these hardcoded
    // per-OS defaults.
    #[cfg(target_os = "linux")]
    {
        LaunchRules {
            run_in_terminal: ["sh", "bash", "zsh"]
                .into_iter()
                .map(str::to_owned)
                .collect(),
            ignore: vec!["exe".to_owned()],
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        LaunchRules::default()
    }
}

impl LaunchConfig {
    pub fn for_current_os(&self) -> LaunchRules {
        let section = {
            #[cfg(target_os = "linux")]
            {
                &self.linux
            }
            #[cfg(windows)]
            {
                &self.windows
            }
            #[cfg(target_os = "macos")]
            {
                &self.macos
            }
            #[cfg(not(any(target_os = "linux", windows, target_os = "macos")))]
            {
                &None
            }
        };
        section.clone().unwrap_or_else(default_launch_rules)
    }
}

const MAX_LAUNCH_EXTENSIONS: usize = 64;

fn sanitize_extensions(values: Vec<String>) -> Vec<String> {
    let mut extensions = Vec::new();
    for value in values {
        let extension = value.trim().trim_start_matches('.').to_lowercase();
        if extension.is_empty()
            || extension.contains(['/', '\\', '.'])
            || extensions.contains(&extension)
        {
            continue;
        }
        extensions.push(extension);
        if extensions.len() >= MAX_LAUNCH_EXTENSIONS {
            break;
        }
    }
    extensions
}

fn sanitize_launch_rules(rules: LaunchRules) -> LaunchRules {
    let run_in_terminal = sanitize_extensions(rules.run_in_terminal);
    let ignore = sanitize_extensions(rules.ignore)
        .into_iter()
        .filter(|extension| !run_in_terminal.contains(extension))
        .collect();
    LaunchRules {
        run_in_terminal,
        ignore,
    }
}

fn sanitize_launch(config: LaunchConfig) -> LaunchConfig {
    let section = |rules: Option<LaunchRules>| {
        rules.map(|rules| {
            let sanitized = sanitize_launch_rules(rules);
            if sanitized.run_in_terminal.is_empty() && sanitized.ignore.is_empty() {
                None
            } else {
                Some(sanitized)
            }
        })
    };
    LaunchConfig {
        // Flatten explicitly emptied sections so a missing section keeps
        // meaning "use the hardcoded OS defaults".
        linux: section(config.linux).flatten(),
        windows: section(config.windows).flatten(),
        macos: section(config.macos).flatten(),
    }
}

fn default_true() -> bool {
    true
}

/// Older files stored a per-panel `{left, right}` object; only an explicit
/// boolean true enables hidden files now, everything else means false.
fn bool_or_legacy<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<bool, D::Error> {
    let value = serde_json::Value::deserialize(deserializer)?;
    Ok(value.as_bool().unwrap_or(false))
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AppConfig {
    pub version: u32,
    pub left_path: Option<String>,
    pub right_path: Option<String>,
    pub column_weights: ColumnWeights,
    pub file_font_size: Option<u32>,
    #[serde(deserialize_with = "bool_or_legacy")]
    pub show_hidden: bool,
    #[serde(default = "default_true")]
    pub show_function_bar: bool,
    pub pinned_directories: Vec<String>,
    pub editor: Option<String>,
    pub launch: LaunchConfig,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            version: CONFIG_VERSION,
            left_path: None,
            right_path: None,
            column_weights: ColumnWeights::default(),
            file_font_size: None,
            show_hidden: false,
            show_function_bar: true,
            pinned_directories: Vec::new(),
            editor: None,
            launch: LaunchConfig::default(),
        }
    }
}

fn sanitize_weights(weights: Option<[f64; COLUMN_COUNT]>) -> Option<[f64; COLUMN_COUNT]> {
    let weights = weights?;
    if weights
        .iter()
        .all(|w| w.is_finite() && *w >= MIN_COLUMN_WEIGHT && *w <= MAX_COLUMN_WEIGHT)
    {
        Some(weights.map(|w| (w * 10.0).round() / 10.0))
    } else {
        None
    }
}

fn sanitize_path(path: Option<String>) -> Option<String> {
    path.map(|p| p.trim().to_owned()).filter(|p| !p.is_empty())
}

impl AppConfig {
    pub fn sanitized(&self) -> Self {
        Self {
            version: CONFIG_VERSION,
            left_path: sanitize_path(self.left_path.clone()),
            right_path: sanitize_path(self.right_path.clone()),
            column_weights: ColumnWeights {
                left: sanitize_weights(self.column_weights.left),
                right: sanitize_weights(self.column_weights.right),
            },
            file_font_size: self
                .file_font_size
                .map(|size| size.clamp(MIN_FILE_FONT_SIZE, MAX_FILE_FONT_SIZE)),
            show_hidden: self.show_hidden,
            show_function_bar: self.show_function_bar,
            editor: sanitize_path(self.editor.clone()),
            launch: sanitize_launch(self.launch.clone()),
            pinned_directories: {
                let mut pins = Vec::new();
                for path in &self.pinned_directories {
                    if let Some(path) = sanitize_path(Some(path.clone())) {
                        if !pins.contains(&path) {
                            pins.push(path);
                        }
                    }
                }
                pins
            },
        }
    }
}

pub fn config_dir() -> PathBuf {
    if let Ok(dir) = std::env::var(CONFIG_ENV_OVERRIDE) {
        if !dir.trim().is_empty() {
            return PathBuf::from(dir);
        }
    }
    dirs::config_dir()
        .map(|dir| dir.join("mycmd"))
        .unwrap_or_else(|| PathBuf::from("."))
}

pub fn config_path() -> PathBuf {
    config_dir().join(CONFIG_FILE_NAME)
}

pub fn load_from(path: &Path) -> AppConfig {
    match fs::read_to_string(path) {
        Ok(contents) => serde_json::from_str::<AppConfig>(&contents).unwrap_or_default(),
        Err(_) => AppConfig::default(),
    }
}

pub fn save_to(path: &Path, config: &AppConfig) -> Result<()> {
    let config = config.sanitized();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| FsError::io(e, parent))?;
    }
    let payload = serde_json::to_string_pretty(&config)
        .map_err(|e| FsError::new("config_encode", e.to_string()))?;
    let tmp_path = path.with_extension(format!("json.tmp-{}", std::process::id()));
    fs::write(&tmp_path, payload).map_err(|e| FsError::io(e, &tmp_path))?;
    fs::rename(&tmp_path, path).map_err(|e| FsError::io(e, path))
}

pub fn load() -> AppConfig {
    load_from(&config_path())
}

pub fn save(config: &AppConfig) -> Result<()> {
    let mut incoming = config.sanitized();
    // The UI has no editor for launch rules; preserve hand-edited per-OS
    // rules so a settings save never wipes config.json changes.
    incoming.launch = load().sanitized().launch;
    save_to(&config_path(), &incoming)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path(name: &str) -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join(name);
        (dir, path)
    }

    #[test]
    fn missing_file_returns_defaults() {
        let (_dir, path) = temp_path("config.json");
        assert_eq!(load_from(&path), AppConfig::default());
    }

    #[test]
    fn corrupt_file_returns_defaults() {
        let (_dir, path) = temp_path("config.json");
        fs::write(&path, "{not valid json").expect("write fixture");
        assert_eq!(load_from(&path), AppConfig::default());
    }

    #[test]
    fn roundtrip_preserves_values() {
        let (_dir, path) = temp_path("config.json");
        let config = AppConfig {
            version: CONFIG_VERSION,
            left_path: Some("C:\\Users\\root".into()),
            right_path: Some("/home/user".into()),
            column_weights: ColumnWeights {
                left: Some([175.0, 78.5, 113.0, 218.0]),
                right: None,
            },
            file_font_size: Some(20),
            show_hidden: true,
            show_function_bar: false,
            pinned_directories: vec!["/home/user/projects".into()],
            editor: Some("/usr/bin/editor".into()),
            launch: LaunchConfig {
                linux: Some(LaunchRules {
                    run_in_terminal: vec!["sh".into()],
                    ignore: vec!["exe".into()],
                }),
                windows: None,
                macos: None,
            },
        };
        save_to(&path, &config).expect("save");
        assert_eq!(load_from(&path), config.sanitized());
    }

    #[test]
    fn legacy_show_hidden_object_loads_as_false() {
        let (_dir, path) = temp_path("config.json");
        fs::write(
            &path,
            r#"{"version":1,"showHidden":{"left":true,"right":false}}"#,
        )
        .expect("write fixture");
        let config = load_from(&path);
        assert!(!config.show_hidden);
        assert!(config.show_function_bar);
    }

    #[test]
    fn sanitize_clamps_and_drops_invalid_values() {
        let config = AppConfig {
            version: 999,
            left_path: Some("   ".into()),
            right_path: Some("  D:\\Backup  ".into()),
            column_weights: ColumnWeights {
                left: Some([10.0, 78.0, 113.0, 218.0]),
                right: Some([f64::NAN, 78.0, 113.0, 218.0]),
            },
            file_font_size: Some(99),
            show_hidden: true,
            show_function_bar: true,
            pinned_directories: Vec::new(),
            editor: Some("   ".into()),
            launch: LaunchConfig::default(),
        };
        let sanitized = config.sanitized();
        assert_eq!(sanitized.version, CONFIG_VERSION);
        assert_eq!(sanitized.left_path, None);
        assert_eq!(sanitized.editor, None);
        assert_eq!(sanitized.right_path.as_deref(), Some("D:\\Backup"));
        assert_eq!(sanitized.column_weights.left, None);
        assert_eq!(sanitized.column_weights.right, None);
        assert_eq!(sanitized.file_font_size, Some(MAX_FILE_FONT_SIZE));
        assert!(sanitized.show_hidden);
        assert!(sanitized.show_function_bar);
    }

    #[test]
    fn launch_rules_missing_section_uses_hardcoded_os_defaults() {
        let rules = LaunchConfig::default().for_current_os();
        #[cfg(target_os = "linux")]
        {
            assert_eq!(rules.run_in_terminal, vec!["sh", "bash", "zsh"]);
            assert_eq!(rules.ignore, vec!["exe"]);
        }
        #[cfg(not(target_os = "linux"))]
        {
            assert!(rules.run_in_terminal.is_empty());
            assert!(rules.ignore.is_empty());
        }
    }

    #[test]
    fn launch_rules_select_only_the_current_os_section_without_merging() {
        let config = LaunchConfig {
            linux: Some(LaunchRules {
                run_in_terminal: vec!["sh".into()],
                ignore: vec![],
            }),
            windows: Some(LaunchRules {
                run_in_terminal: vec![],
                ignore: vec!["lnk".into()],
            }),
            macos: None,
        };
        let rules = config.for_current_os();
        #[cfg(target_os = "linux")]
        assert_eq!(rules.run_in_terminal, vec!["sh"]);
        #[cfg(windows)]
        assert_eq!(rules.ignore, vec!["lnk"]);
        #[cfg(target_os = "macos")]
        assert!(rules.run_in_terminal.is_empty() && rules.ignore.is_empty());
    }

    #[test]
    fn launch_rules_normalize_extensions_and_drop_overlaps() {
        let sanitized = sanitize_launch(LaunchConfig {
            linux: Some(LaunchRules {
                run_in_terminal: vec![
                    " SH ".into(),
                    ".bash".into(),
                    "sh".into(),
                    "a/b".into(),
                    String::new(),
                ],
                ignore: vec!["sh".into(), "EXE".into()],
            }),
            windows: None,
            macos: None,
        });
        assert_eq!(
            sanitized.linux,
            Some(LaunchRules {
                run_in_terminal: vec!["sh".into(), "bash".into()],
                ignore: vec!["exe".into()],
            })
        );
        assert_eq!(
            sanitize_launch(LaunchConfig {
                linux: Some(LaunchRules::default()),
                windows: None,
                macos: None,
            })
            .linux,
            None
        );
    }

    #[test]
    fn launch_rules_survive_unknown_os_keys() {
        let (_dir, path) = temp_path("config.json");
        fs::write(
            &path,
            r#"{"version":1,"launch":{"plan9":{"runInTerminal":["rc"]}}}"#,
        )
        .expect("write fixture");
        assert_eq!(load_from(&path).launch, LaunchConfig::default());
    }
}

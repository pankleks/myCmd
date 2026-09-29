use crate::error::{FsError, Result};
use serde::{Deserialize, Serialize};
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

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ShowHidden {
    pub left: Option<bool>,
    pub right: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AppConfig {
    pub version: u32,
    pub left_path: Option<String>,
    pub right_path: Option<String>,
    pub column_weights: ColumnWeights,
    pub file_font_size: Option<u32>,
    pub show_hidden: ShowHidden,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            version: CONFIG_VERSION,
            left_path: None,
            right_path: None,
            column_weights: ColumnWeights::default(),
            file_font_size: None,
            show_hidden: ShowHidden::default(),
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
    save_to(&config_path(), config)
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
            show_hidden: ShowHidden {
                left: Some(true),
                right: None,
            },
        };
        save_to(&path, &config).expect("save");
        assert_eq!(load_from(&path), config.sanitized());
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
            show_hidden: ShowHidden {
                left: None,
                right: Some(false),
            },
        };
        let sanitized = config.sanitized();
        assert_eq!(sanitized.version, CONFIG_VERSION);
        assert_eq!(sanitized.left_path, None);
        assert_eq!(sanitized.right_path.as_deref(), Some("D:\\Backup"));
        assert_eq!(sanitized.column_weights.left, None);
        assert_eq!(sanitized.column_weights.right, None);
        assert_eq!(sanitized.file_font_size, Some(MAX_FILE_FONT_SIZE));
        assert_eq!(
            sanitized.show_hidden,
            ShowHidden {
                left: None,
                right: Some(false),
            }
        );
    }
}

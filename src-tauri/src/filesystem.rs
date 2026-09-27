use crate::error::{FsError, Result};
use serde::Serialize;
use std::{fs, path::{Component, Path, PathBuf}, time::UNIX_EPOCH};

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub name: String, pub path: String, pub r#type: String, pub extension: String,
    pub size: u64, pub modified: Option<u64>, pub created: Option<u64>, pub hidden: bool,
    pub readonly: bool, pub directory_target: bool,
}
#[derive(Serialize)]
pub struct Listing { pub path: String, pub parent: Option<String>, pub entries: Vec<Entry> }
#[derive(Serialize)]
pub struct Root { pub name: String, pub path: String, pub r#type: String }
pub fn text(p: &Path) -> String { p.to_string_lossy().into_owned() }
pub fn absolute(p: &Path) -> Result<PathBuf> {
    let p = if p == Path::new("~") { dirs::home_dir().ok_or_else(|| FsError::new("not_found", "Home directory unavailable"))? } else { p.to_owned() };
    fs::canonicalize(&p).map_err(|e| FsError::io(e, &p))
}
pub fn entry(p: &Path) -> Result<Entry> {
    let m = fs::symlink_metadata(p).map_err(|e| FsError::io(e, p))?;
    let name = p.file_name().unwrap_or(p.as_os_str()).to_string_lossy().into_owned();
    #[cfg(windows)]
    let hidden = { use std::os::windows::fs::MetadataExt; m.file_attributes() & 6 != 0 };
    #[cfg(not(windows))]
    let hidden = name.starts_with('.');
    Ok(Entry { name, path: text(p), r#type: if m.is_symlink() { "symlink" } else if m.is_dir() { "directory" } else { "file" }.into(),
        extension: p.extension().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default(), size: if m.is_dir() { 0 } else { m.len() },
        modified: m.modified().ok().and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map(|d| d.as_secs()),
        created: m.created().ok().and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map(|d| d.as_secs()),
        hidden, readonly: m.permissions().readonly(), directory_target: p.is_dir() })
}
pub fn list(p: &Path) -> Result<Listing> {
    let p = absolute(p)?;
    let entries = fs::read_dir(&p).map_err(|e| FsError::io(e, &p))?
        .map(|e| e.map_err(|e| FsError::io(e, &p)).and_then(|e| entry(&e.path()))).collect::<Result<Vec<_>>>()?;
    Ok(Listing { path: text(&p), parent: p.parent().map(text), entries })
}
pub fn validate_name(name: &str) -> Result<()> {
    let mut parts = Path::new(name).components();
    if name.is_empty() || name.contains(['/', '\\']) || !matches!(parts.next(), Some(Component::Normal(_))) || parts.next().is_some() {
        return Err(FsError::new("invalid_path", "Enter a single file or directory name"));
    }
    Ok(())
}
pub fn roots() -> Vec<Root> {
    let mut roots = Vec::new();
    if let Some(p) = dirs::home_dir() { roots.push(Root { name: "Home".into(), path: text(&p), r#type: "home".into() }); }
    #[cfg(windows)]
    for letter in b'A'..=b'Z' { let p = format!("{}:\\", letter as char); if Path::new(&p).is_dir() { roots.push(Root { name: p.clone(), path: p, r#type: "drive".into() }); } }
    #[cfg(unix)]
    {
        roots.push(Root { name: "/".into(), path: "/".into(), r#type: "root".into() });
        let mut mounts = Vec::<PathBuf>::new();
        #[cfg(target_os = "linux")]
        if let Ok(contents) = fs::read_to_string("/proc/self/mountinfo") {
            for line in contents.lines() { if let Some(p) = line.split_whitespace().nth(4) {
                let p = p.replace("\\040", " ").replace("\\011", "\t").replace("\\134", "\\");
                if !p.starts_with("/proc") && !p.starts_with("/sys") && !p.starts_with("/dev") { mounts.push(p.into()); }
            } }
        }
        for base in ["/Volumes", "/mnt", "/media", "/run/media"] {
            if let Ok(items) = fs::read_dir(base) { for item in items.flatten() { if item.path().is_dir() { mounts.push(item.path()); } } }
        }
        mounts.sort(); mounts.dedup();
        for p in mounts { let path = text(&p); if !roots.iter().any(|r| r.path == path) { roots.push(Root { name: path.clone(), path, r#type: "mount".into() }); } }
    }
    roots
}

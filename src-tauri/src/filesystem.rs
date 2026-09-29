use crate::error::{FsError, Result};
use base64::{engine::general_purpose::STANDARD, Engine};
use serde::Serialize;
use std::{
    fs,
    io::Read,
    path::{Component, Path, PathBuf},
    time::UNIX_EPOCH,
};

pub const MAX_TEXT_PREVIEW_BYTES: u64 = 2 * 1024 * 1024;
pub const MAX_IMAGE_PREVIEW_BYTES: u64 = 4 * 1024 * 1024;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub name: String,
    pub path: String,
    pub r#type: String,
    pub extension: String,
    pub size: u64,
    pub modified: Option<u64>,
    pub created: Option<u64>,
    pub hidden: bool,
    pub readonly: bool,
    pub directory_target: bool,
}
#[derive(Serialize)]
pub struct Listing {
    pub path: String,
    pub parent: Option<String>,
    pub entries: Vec<Entry>,
}
#[derive(Serialize)]
pub struct Root {
    pub name: String,
    pub path: String,
    pub r#type: String,
}
pub fn text(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

pub fn directory_size(p: &Path) -> Result<u64> {
    let root = absolute(p)?;
    let mut total = 0u64;
    let mut stack = vec![root];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).map_err(|e| FsError::io(e, &dir))? {
            let path = entry.map_err(|e| FsError::io(e, &dir))?.path();
            let meta = fs::symlink_metadata(&path).map_err(|e| FsError::io(e, &path))?;
            if meta.is_dir() && !meta.file_type().is_symlink() {
                stack.push(path);
            } else if meta.is_file() {
                total += meta.len();
            }
        }
    }
    Ok(total)
}
pub fn absolute(p: &Path) -> Result<PathBuf> {
    let p = if p == Path::new("~") {
        dirs::home_dir().ok_or_else(|| FsError::new("not_found", "Home directory unavailable"))?
    } else {
        p.to_owned()
    };
    fs::canonicalize(&p).map_err(|e| FsError::io(e, &p))
}

pub fn read_text_preview(path: &Path) -> Result<String> {
    let path = absolute(path)?;
    let metadata = fs::metadata(&path).map_err(|e| FsError::io(e, &path))?;
    if !metadata.is_file() {
        return Err(FsError::new("not_a_file", "Only files can be previewed"));
    }
    if metadata.len() > MAX_TEXT_PREVIEW_BYTES {
        return Err(FsError::new(
            "file_too_large",
            "Files larger than 2 MiB cannot be previewed",
        ));
    }

    let file = fs::File::open(&path).map_err(|e| FsError::io(e, &path))?;
    let mut content = Vec::with_capacity(metadata.len() as usize);
    file.take(MAX_TEXT_PREVIEW_BYTES + 1)
        .read_to_end(&mut content)
        .map_err(|e| FsError::io(e, &path))?;
    if content.len() as u64 > MAX_TEXT_PREVIEW_BYTES {
        return Err(FsError::new(
            "file_too_large",
            "Files larger than 2 MiB cannot be previewed",
        ));
    }
    if content.contains(&0) {
        return Err(FsError::new(
            "binary_file",
            "Binary files cannot be previewed as text",
        ));
    }
    String::from_utf8(content)
        .map_err(|_| FsError::new("invalid_encoding", "File is not valid UTF-8 text"))
}

pub fn read_markdown_image(markdown_path: &Path, source: &str) -> Result<String> {
    let markdown_path = absolute(markdown_path)?;
    let base = markdown_path
        .parent()
        .ok_or_else(|| FsError::new("invalid_path", "Markdown file has no parent folder"))?;
    let source = source.split(['?', '#']).next().unwrap_or_default().trim();
    let decoded = decode_uri_path(source)
        .ok_or_else(|| FsError::new("invalid_image_path", "Invalid image path"))?;
    let relative = Path::new(&decoded);
    if decoded.is_empty()
        || relative.is_absolute()
        || decoded.starts_with("//")
        || has_uri_scheme(&decoded)
    {
        return Err(FsError::new(
            "invalid_image_path",
            "Only relative local images can be previewed",
        ));
    }

    let path = absolute(&base.join(relative))?;
    image_data_url(&path)
}

pub fn read_image_preview(path: &Path) -> Result<String> {
    image_data_url(&absolute(path)?)
}

fn image_data_url(path: &Path) -> Result<String> {
    let metadata = fs::metadata(&path).map_err(|e| FsError::io(e, &path))?;
    if !metadata.is_file() {
        return Err(FsError::new("not_a_file", "Image source is not a file"));
    }
    let mime = match path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("avif") => "image/avif",
        Some("bmp") => "image/bmp",
        _ => {
            return Err(FsError::new(
                "unsupported_image",
                "This raster image format cannot be previewed",
            ));
        }
    };
    if metadata.len() > MAX_IMAGE_PREVIEW_BYTES {
        return Err(FsError::new(
            "image_too_large",
            "Images larger than 4 MiB cannot be previewed",
        ));
    }

    let file = fs::File::open(&path).map_err(|e| FsError::io(e, &path))?;
    let mut content = Vec::with_capacity(metadata.len() as usize);
    file.take(MAX_IMAGE_PREVIEW_BYTES + 1)
        .read_to_end(&mut content)
        .map_err(|e| FsError::io(e, &path))?;
    if content.len() as u64 > MAX_IMAGE_PREVIEW_BYTES {
        return Err(FsError::new(
            "image_too_large",
            "Images larger than 4 MiB cannot be previewed",
        ));
    }

    Ok(format!("data:{mime};base64,{}", STANDARD.encode(content)))
}

fn decode_uri_path(source: &str) -> Option<String> {
    let bytes = source.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let high = hex_value(*bytes.get(index + 1)?)?;
            let low = hex_value(*bytes.get(index + 2)?)?;
            decoded.push((high << 4) | low);
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(decoded).ok()
}

fn hex_value(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

fn has_uri_scheme(source: &str) -> bool {
    let Some(colon) = source.find(':') else {
        return false;
    };
    let first_separator = source.find(['/', '\\']).unwrap_or(usize::MAX);
    colon < first_separator
}

pub fn entry(p: &Path) -> Result<Entry> {
    if p.to_str().is_none() {
        return Err(FsError::new(
            "invalid_path",
            "This path cannot be represented as Unicode",
        ));
    }
    let m = fs::symlink_metadata(p).map_err(|e| FsError::io(e, p))?;
    let name = p
        .file_name()
        .unwrap_or(p.as_os_str())
        .to_string_lossy()
        .into_owned();
    #[cfg(windows)]
    let hidden = {
        use std::os::windows::fs::MetadataExt;
        m.file_attributes() & 6 != 0
    };
    #[cfg(not(windows))]
    let hidden = name.starts_with('.');
    Ok(Entry {
        name,
        path: text(p),
        r#type: if m.is_symlink() {
            "symlink"
        } else if m.is_dir() {
            "directory"
        } else {
            "file"
        }
        .into(),
        extension: p
            .extension()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default(),
        size: if m.is_dir() { 0 } else { m.len() },
        modified: m
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_secs()),
        created: m
            .created()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_secs()),
        hidden,
        readonly: m.permissions().readonly(),
        directory_target: p.is_dir(),
    })
}
pub fn list(p: &Path) -> Result<Listing> {
    let p = absolute(p)?;
    let entries = fs::read_dir(&p)
        .map_err(|e| FsError::io(e, &p))?
        .map(|e| {
            e.map_err(|e| FsError::io(e, &p))
                .and_then(|e| entry(&e.path()))
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(Listing {
        path: text(&p),
        parent: p.parent().map(text),
        entries,
    })
}
pub fn validate_name(name: &str) -> Result<()> {
    let mut parts = Path::new(name).components();
    if name.is_empty()
        || name.contains(['/', '\\'])
        || !matches!(parts.next(), Some(Component::Normal(_)))
        || parts.next().is_some()
    {
        return Err(FsError::new(
            "invalid_path",
            "Enter a single file or directory name",
        ));
    }
    #[cfg(windows)]
    {
        let stem = name.split('.').next().unwrap_or("").to_ascii_uppercase();
        let reserved = ["CON", "PRN", "AUX", "NUL"].contains(&stem.as_str())
            || (stem.len() == 4
                && (stem.starts_with("COM") || stem.starts_with("LPT"))
                && matches!(stem.as_bytes()[3], b'1'..=b'9'));
        if reserved
            || name.ends_with([' ', '.'])
            || name.chars().any(|c| c < ' ' || "<>:\"|?*".contains(c))
        {
            return Err(FsError::new("invalid_path", "Invalid Windows file name"));
        }
    }
    Ok(())
}
pub fn roots() -> Vec<Root> {
    let mut roots = Vec::new();
    if let Some(p) = dirs::home_dir() {
        roots.push(Root {
            name: "Home".into(),
            path: text(&p),
            r#type: "home".into(),
        });
    }
    #[cfg(windows)]
    for letter in b'A'..=b'Z' {
        let p = format!("{}:\\", letter as char);
        if Path::new(&p).is_dir() {
            roots.push(Root {
                name: p.clone(),
                path: p,
                r#type: "drive".into(),
            });
        }
    }
    #[cfg(unix)]
    {
        roots.push(Root {
            name: "/".into(),
            path: "/".into(),
            r#type: "root".into(),
        });
        let mut mounts = Vec::<PathBuf>::new();
        #[cfg(target_os = "linux")]
        if let Ok(contents) = fs::read_to_string("/proc/self/mountinfo") {
            for line in contents.lines() {
                if let Some(p) = line.split_whitespace().nth(4) {
                    let p = p
                        .replace("\\040", " ")
                        .replace("\\011", "\t")
                        .replace("\\134", "\\");
                    if !p.starts_with("/proc") && !p.starts_with("/sys") && !p.starts_with("/dev") {
                        mounts.push(p.into());
                    }
                }
            }
        }
        for base in ["/Volumes", "/mnt", "/media", "/run/media"] {
            if let Ok(items) = fs::read_dir(base) {
                for item in items.flatten() {
                    if item.path().is_dir() {
                        mounts.push(item.path());
                    }
                }
            }
        }
        mounts.sort();
        mounts.dedup();
        for p in mounts {
            let path = text(&p);
            if !roots.iter().any(|r| r.path == path) {
                roots.push(Root {
                    name: path.clone(),
                    path,
                    r#type: "mount".into(),
                });
            }
        }
    }
    roots
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn directory_size_sums_nested_files() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join("nested")).unwrap();
        let mut a = fs::File::create(dir.path().join("a.bin")).unwrap();
        a.write_all(&[0u8; 100]).unwrap();
        let mut b = fs::File::create(dir.path().join("nested/b.bin")).unwrap();
        b.write_all(&[0u8; 50]).unwrap();
        assert_eq!(directory_size(dir.path()).unwrap(), 150);
    }

    #[test]
    fn directory_size_rejects_missing_paths() {
        let temp = tempfile::tempdir().unwrap();
        let missing = temp.path().join("missing");
        assert!(directory_size(&missing).is_err());
    }

    #[test]
    fn read_text_preview_preserves_text() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("sample.ts");
        fs::write(&path, "const answer = 42;\n").unwrap();
        assert_eq!(read_text_preview(&path).unwrap(), "const answer = 42;\n");
    }

    #[test]
    fn read_text_preview_rejects_directories() {
        let temp = tempfile::tempdir().unwrap();
        let error = read_text_preview(temp.path()).unwrap_err();
        assert_eq!(error.code, "not_a_file");
    }

    #[test]
    fn read_text_preview_rejects_binary_data() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("sample.bin");
        fs::write(&path, [0, 1, 2]).unwrap();
        let error = read_text_preview(&path).unwrap_err();
        assert_eq!(error.code, "binary_file");
    }

    #[test]
    fn read_text_preview_rejects_invalid_utf8() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("sample.txt");
        fs::write(&path, [0xff, 0xfe]).unwrap();
        let error = read_text_preview(&path).unwrap_err();
        assert_eq!(error.code, "invalid_encoding");
    }

    #[test]
    fn read_text_preview_rejects_large_files() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("large.txt");
        let file = fs::File::create(&path).unwrap();
        file.set_len(MAX_TEXT_PREVIEW_BYTES + 1).unwrap();
        let error = read_text_preview(&path).unwrap_err();
        assert_eq!(error.code, "file_too_large");
    }

    #[test]
    fn read_markdown_image_returns_a_data_url_for_relative_raster_image() {
        let temp = tempfile::tempdir().unwrap();
        let markdown = temp.path().join("README.md");
        let image = temp.path().join("diagram.png");
        fs::write(&markdown, "![diagram](diagram.png)").unwrap();
        fs::write(&image, b"png").unwrap();

        assert_eq!(
            read_markdown_image(&markdown, "diagram.png").unwrap(),
            "data:image/png;base64,cG5n"
        );
    }

    #[test]
    fn read_markdown_image_decodes_spaces_in_relative_paths() {
        let temp = tempfile::tempdir().unwrap();
        let markdown = temp.path().join("README.md");
        fs::write(&markdown, "").unwrap();
        fs::write(temp.path().join("my image.jpg"), b"jpg").unwrap();

        assert!(read_markdown_image(&markdown, "my%20image.jpg")
            .unwrap()
            .starts_with("data:image/jpeg;base64,"));
    }

    #[test]
    fn read_markdown_image_rejects_remote_sources_and_non_images() {
        let temp = tempfile::tempdir().unwrap();
        let markdown = temp.path().join("README.md");
        fs::write(&markdown, "").unwrap();
        let remote_error = read_markdown_image(&markdown, "https://example.com/a.png").unwrap_err();
        assert_eq!(remote_error.code, "invalid_image_path");

        fs::write(temp.path().join("page.html"), "<script>alert(1)</script>").unwrap();
        let file_error = read_markdown_image(&markdown, "page.html").unwrap_err();
        assert_eq!(file_error.code, "unsupported_image");
    }

    #[test]
    fn read_markdown_image_rejects_large_images() {
        let temp = tempfile::tempdir().unwrap();
        let markdown = temp.path().join("README.md");
        fs::write(&markdown, "").unwrap();
        let file = fs::File::create(temp.path().join("large.png")).unwrap();
        file.set_len(MAX_IMAGE_PREVIEW_BYTES + 1).unwrap();

        let error = read_markdown_image(&markdown, "large.png").unwrap_err();
        assert_eq!(error.code, "image_too_large");
    }

    #[test]
    fn read_image_preview_returns_a_data_url_for_raster_images() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("picture.jpg");
        fs::write(&path, b"jpeg data").unwrap();

        assert_eq!(
            read_image_preview(&path).unwrap(),
            "data:image/jpeg;base64,anBlZyBkYXRh"
        );
    }

    #[test]
    fn read_image_preview_rejects_unsupported_types_and_large_images() {
        let temp = tempfile::tempdir().unwrap();
        let svg = temp.path().join("picture.svg");
        fs::write(&svg, "<svg></svg>").unwrap();
        assert_eq!(
            read_image_preview(&svg).unwrap_err().code,
            "unsupported_image"
        );

        let large = temp.path().join("large.png");
        let file = fs::File::create(&large).unwrap();
        file.set_len(MAX_IMAGE_PREVIEW_BYTES + 1).unwrap();
        assert_eq!(
            read_image_preview(&large).unwrap_err().code,
            "image_too_large"
        );
    }
}

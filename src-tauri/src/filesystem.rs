use crate::error::{FsError, Result};
use base64::{engine::general_purpose::STANDARD, Engine};
use serde::Serialize;
use std::{
    fs,
    io::Read,
    path::{Component, Path, PathBuf},
    time::UNIX_EPOCH,
};

pub const MAX_TEXT_PREVIEW_BYTES: u64 = 8 * 1024 * 1024;
pub const MAX_IMAGE_PREVIEW_BYTES: u64 = 4 * 1024 * 1024;
pub const MAX_IMAGE_PREVIEW_PIXELS: usize = 16_000_000;
pub const MAX_IMAGE_PREVIEW_DIMENSION: usize = 16_384;
const MAX_ANIMATION_PREVIEW_FRAMES: usize = 100;
const MAX_ANIMATION_PREVIEW_FRAME_PIXELS: usize = 64_000_000;

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
#[serde(rename_all = "camelCase")]
pub struct Listing {
    pub path: String,
    pub parent: Option<String>,
    pub entries: Vec<Entry>,
    pub skipped_entries: usize,
    pub warnings: Vec<FsError>,
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

#[cfg(test)]
pub fn directory_size(p: &Path) -> Result<u64> {
    directory_size_checked(p, || Ok(()))
}

pub fn directory_size_checked(p: &Path, check: impl Fn() -> Result<()>) -> Result<u64> {
    check()?;
    let root = absolute(p)?;
    let mut total = 0u64;
    let mut stack = vec![root];
    while let Some(dir) = stack.pop() {
        check()?;
        for entry in fs::read_dir(&dir).map_err(|e| FsError::io(e, &dir))? {
            check()?;
            let path = entry.map_err(|e| FsError::io(e, &dir))?.path();
            let meta = fs::symlink_metadata(&path).map_err(|e| FsError::io(e, &path))?;
            if meta.is_dir() && !meta.file_type().is_symlink() {
                stack.push(path);
            } else if meta.is_file() {
                total = total.checked_add(meta.len()).ok_or_else(|| {
                    FsError::new("size_overflow", "Total file size exceeds supported range")
                })?;
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
            "Files larger than 8 MiB cannot be previewed",
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
            "Files larger than 8 MiB cannot be previewed",
        ));
    }
    let utf16 = if content.starts_with(&[0xff, 0xfe]) {
        Some((&content[2..], true))
    } else if content.starts_with(&[0xfe, 0xff]) {
        Some((&content[2..], false))
    } else if content.len() >= 4 && content.len() % 2 == 0 {
        // Recognize BOM-less UTF-16 conservatively: ASCII text has a zero
        // high byte in almost every code unit, but no zero low bytes.
        let pairs = content.len() / 2;
        let little = content.chunks_exact(2).filter(|p| p[1] == 0).count();
        let big = content.chunks_exact(2).filter(|p| p[0] == 0).count();
        if little * 10 >= pairs * 9 && content.chunks_exact(2).all(|p| p[0] != 0) {
            Some((&content[..], true))
        } else if big * 10 >= pairs * 9 && content.chunks_exact(2).all(|p| p[1] != 0) {
            Some((&content[..], false))
        } else {
            None
        }
    } else {
        None
    };
    if let Some((bytes, little_endian)) = utf16 {
        if bytes.len() % 2 != 0 {
            return Err(FsError::new(
                "invalid_encoding",
                "File is not valid UTF-16 text",
            ));
        }
        let units: Vec<u16> = bytes
            .chunks_exact(2)
            .map(|p| {
                if little_endian {
                    u16::from_le_bytes([p[0], p[1]])
                } else {
                    u16::from_be_bytes([p[0], p[1]])
                }
            })
            .collect();
        let text = String::from_utf16(&units)
            .map_err(|_| FsError::new("invalid_encoding", "File is not valid UTF-16 text"))?;
        if text.chars().any(|c| {
            c.is_control() && !matches!(c, '\n' | '\r' | '\t' | '\u{1b}' | '\u{8}' | '\u{c}')
        }) {
            return Err(FsError::new(
                "binary_file",
                "Binary files cannot be previewed as text",
            ));
        }
        return Ok(text);
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

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewImage {
    pub data_url: String,
    pub width: usize,
    pub height: usize,
    pub frames: usize,
}

pub fn read_markdown_image(markdown_path: &Path, source: &str) -> Result<PreviewImage> {
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
    Ok(image_data_url(&absolute(path)?)?.data_url)
}

fn image_data_url(path: &Path) -> Result<PreviewImage> {
    let metadata = fs::metadata(path).map_err(|e| FsError::io(e, path))?;
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

    let file = fs::File::open(path).map_err(|e| FsError::io(e, path))?;
    let mut content = Vec::with_capacity(metadata.len() as usize);
    file.take(MAX_IMAGE_PREVIEW_BYTES + 1)
        .read_to_end(&mut content)
        .map_err(|e| FsError::io(e, path))?;
    if content.len() as u64 > MAX_IMAGE_PREVIEW_BYTES {
        return Err(FsError::new(
            "image_too_large",
            "Images larger than 4 MiB cannot be previewed",
        ));
    }

    let (size, frames) = validate_image_dimensions(&content)?;
    Ok(PreviewImage {
        data_url: format!("data:{mime};base64,{}", STANDARD.encode(content)),
        width: size.width,
        height: size.height,
        frames,
    })
}

fn validate_image_dimensions(content: &[u8]) -> Result<(imagesize::ImageSize, usize)> {
    validate_avif_brands(content)?;
    let size = imagesize::blob_size(content)
        .map_err(|_| FsError::new("invalid_image", "Image dimensions could not be read"))?;
    if size.width == 0 || size.height == 0 {
        return Err(FsError::new(
            "invalid_image",
            "Image dimensions must be nonzero",
        ));
    }
    if size.width > MAX_IMAGE_PREVIEW_DIMENSION
        || size.height > MAX_IMAGE_PREVIEW_DIMENSION
        || size
            .width
            .checked_mul(size.height)
            .is_none_or(|pixels| pixels > MAX_IMAGE_PREVIEW_PIXELS)
    {
        return Err(FsError::new(
            "image_too_large",
            "Image exceeds preview limits (16 million pixels or 16384 pixels per side)",
        ));
    }
    let frames = if content.starts_with(b"GIF87a") || content.starts_with(b"GIF89a") {
        validate_gif_frames(content, size.width * size.height)?
    } else {
        1
    };
    if content.starts_with(b"\x89PNG\r\n\x1a\n") {
        reject_chunked_animation(content, true)?;
    } else if content.starts_with(b"RIFF") && content.get(8..12) == Some(b"WEBP") {
        reject_chunked_animation(content, false)?;
    }
    Ok((size, frames))
}

fn validate_avif_brands(content: &[u8]) -> Result<()> {
    if content.get(4..8) == Some(b"ftyp") {
        let end = u32::from_be_bytes(content[..4].try_into().unwrap()) as usize;
        let brands = content
            .get(16..end)
            .ok_or_else(|| FsError::new("invalid_image", "Invalid AVIF brand box"))?;
        if content.get(8..12) == Some(b"avis")
            || brands
                .as_chunks::<4>()
                .0
                .iter()
                .any(|brand| brand == b"avis")
        {
            return Err(FsError::new(
                "unsupported_image",
                "Animated AVIF sequences cannot be previewed; open in the default app",
            ));
        }
    }
    Ok(())
}

fn reject_chunked_animation(content: &[u8], png: bool) -> Result<()> {
    let invalid = || FsError::new("invalid_image", "Invalid animation container metadata");
    let mut offset = if png { 8 } else { 12 };
    let end = if png {
        content.len()
    } else {
        let declared = u32::from_le_bytes(content[4..8].try_into().unwrap()) as usize;
        declared
            .checked_add(8)
            .filter(|end| *end <= content.len())
            .ok_or_else(invalid)?
    };
    while offset < end {
        let header = content.get(offset..offset + 8).ok_or_else(invalid)?;
        let (kind, length) = if png {
            (
                &header[4..8],
                u32::from_be_bytes(header[..4].try_into().unwrap()) as usize,
            )
        } else {
            (
                &header[..4],
                u32::from_le_bytes(header[4..8].try_into().unwrap()) as usize,
            )
        };
        let data_end = offset
            .checked_add(8)
            .and_then(|start| start.checked_add(length))
            .ok_or_else(invalid)?;
        let next = data_end
            .checked_add(if png { 4 } else { length % 2 })
            .filter(|next| *next <= end)
            .ok_or_else(invalid)?;
        if (png && matches!(kind, b"acTL" | b"fcTL" | b"fdAT"))
            || (!png && matches!(kind, b"ANIM" | b"ANMF"))
            || (!png
                && kind == b"VP8X"
                && content
                    .get(offset + 8)
                    .is_some_and(|flags| flags & 0x02 != 0))
        {
            return Err(FsError::new(
                "unsupported_image",
                "Only GIF animation is supported; open this image in the default app",
            ));
        }
        offset = next;
        if png && kind == b"IEND" {
            break;
        }
    }
    Ok(())
}

fn validate_gif_frames(content: &[u8], canvas_pixels: usize) -> Result<usize> {
    let invalid = |_| FsError::new("invalid_image", "GIF frame metadata could not be read");
    let mut options = gif::DecodeOptions::new();
    options.skip_frame_decoding(true);
    options.check_frame_consistency(true);
    let mut decoder = options.read_info(content).map_err(invalid)?;
    let mut frames = 0usize;
    while decoder.read_next_frame().map_err(invalid)?.is_some() {
        frames += 1;
        if frames > MAX_ANIMATION_PREVIEW_FRAMES
            || canvas_pixels
                .checked_mul(frames)
                .is_none_or(|pixels| pixels > MAX_ANIMATION_PREVIEW_FRAME_PIXELS)
        {
            return Err(FsError::new(
                "image_too_large",
                "GIF exceeds preview limits (100 frames or 64 million cumulative canvas pixels)",
            ));
        }
    }
    if frames == 0 {
        return Err(FsError::new("invalid_image", "GIF has no image frames"));
    }
    Ok(frames)
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
    let results = fs::read_dir(&p).map_err(|e| FsError::io(e, &p))?.map(|e| {
        e.map_err(|e| FsError::io(e, &p))
            .and_then(|e| entry(&e.path()))
    });
    let (entries, skipped_entries, warnings) = collect_listing_entries(results);
    Ok(Listing {
        path: text(&p),
        parent: p.parent().map(text),
        entries,
        skipped_entries,
        warnings,
    })
}

fn collect_listing_entries(
    results: impl Iterator<Item = Result<Entry>>,
) -> (Vec<Entry>, usize, Vec<FsError>) {
    let mut entries = Vec::new();
    let mut skipped = 0;
    let mut warnings = Vec::new();
    for result in results {
        match result {
            Ok(entry) => entries.push(entry),
            Err(error) => {
                skipped += 1;
                // Bound diagnostic payloads even when thousands of entries fail.
                if warnings.len() < 20 {
                    warnings.push(error);
                }
            }
        }
    }
    (entries, skipped, warnings)
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
    const PNG_BASE64: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg==";

    fn png() -> Vec<u8> {
        STANDARD.decode(PNG_BASE64).unwrap()
    }

    fn animated_gif(width: u16, height: u16, frames: usize) -> Vec<u8> {
        let mut bytes = Vec::new();
        {
            let mut encoder =
                gif::Encoder::new(&mut bytes, width, height, &[0, 0, 0, 255, 0, 0]).unwrap();
            let frame = gif::Frame {
                width: 1,
                height: 1,
                buffer: std::borrow::Cow::Borrowed(&[0]),
                ..Default::default()
            };
            for _ in 0..frames {
                encoder.write_frame(&frame).unwrap();
            }
        }
        bytes
    }

    #[test]
    fn gif_preview_bounds_frame_counts_without_decoding_pixels() {
        validate_image_dimensions(&animated_gif(1, 1, 100)).unwrap();
        assert_eq!(
            validate_image_dimensions(&animated_gif(1, 1, 101))
                .unwrap_err()
                .code,
            "image_too_large"
        );
    }

    #[test]
    fn gif_preview_bounds_cumulative_canvas_pixels() {
        validate_image_dimensions(&animated_gif(4000, 4000, 4)).unwrap();
        assert_eq!(
            validate_image_dimensions(&animated_gif(4000, 4000, 5))
                .unwrap_err()
                .code,
            "image_too_large"
        );
    }

    #[test]
    fn gif_preview_rejects_missing_frames() {
        assert_eq!(
            validate_image_dimensions(&animated_gif(1, 1, 0))
                .unwrap_err()
                .code,
            "invalid_image"
        );
    }

    #[test]
    fn gif_previews_remain_supported_and_return_frame_cost_metadata() {
        let temp = tempfile::tempdir().unwrap();
        let image = temp.path().join("animated.gif");
        let markdown = temp.path().join("README.md");
        fs::write(&image, animated_gif(1, 1, 2)).unwrap();
        fs::write(&markdown, "![animation](animated.gif)").unwrap();
        assert!(read_image_preview(&image)
            .unwrap()
            .starts_with("data:image/gif;base64,"));
        let preview = read_markdown_image(&markdown, "animated.gif").unwrap();
        assert_eq!((preview.width, preview.height, preview.frames), (1, 1, 2));
        let json = serde_json::to_value(preview).unwrap();
        assert_eq!(json["frames"], 2);
    }

    #[test]
    fn real_animation_fixtures_accept_gif_and_reject_apng() {
        let gif = STANDARD
            .decode(include_str!("../../src/test/fixtures/animated-gif.base64").trim())
            .unwrap();
        let (size, frames) = validate_image_dimensions(&gif).unwrap();
        assert_eq!((size.width, size.height, frames), (1, 1, 2));
        let mut options = gif::DecodeOptions::new();
        options.set_color_output(gif::ColorOutput::RGBA);
        let mut decoder = options.read_info(gif.as_slice()).unwrap();
        assert_eq!(
            decoder.read_next_frame().unwrap().unwrap().buffer.as_ref(),
            &[0, 0, 0, 255]
        );
        assert_eq!(
            decoder.read_next_frame().unwrap().unwrap().buffer.as_ref(),
            &[255, 0, 0, 255]
        );
        assert!(decoder.read_next_frame().unwrap().is_none());
        let png = STANDARD
            .decode(include_str!("../../src/test/fixtures/animated-png.base64").trim())
            .unwrap();
        assert_eq!(
            validate_image_dimensions(&png).unwrap_err().code,
            "unsupported_image"
        );
    }

    fn animation_container(png: bool, frames: usize) -> Vec<u8> {
        fn chunk(bytes: &mut Vec<u8>, png: bool, kind: &[u8; 4], data: &[u8]) {
            let length = data.len() as u32;
            if png {
                bytes.extend(length.to_be_bytes());
                bytes.extend(kind);
            } else {
                bytes.extend(kind);
                bytes.extend(length.to_le_bytes());
            }
            bytes.extend(data);
            if png {
                bytes.extend([0; 4]);
            } else if data.len() % 2 == 1 {
                bytes.push(0);
            }
        }
        let mut bytes = if png {
            b"\x89PNG\r\n\x1a\n".to_vec()
        } else {
            b"RIFF\0\0\0\0WEBP".to_vec()
        };
        if png {
            let mut control = (frames as u32).to_be_bytes().to_vec();
            control.extend([0; 4]);
            chunk(&mut bytes, true, b"acTL", &control);
        } else {
            chunk(&mut bytes, false, b"ANIM", &[0; 6]);
        }
        for _ in 0..frames {
            let mut frame = vec![0; if png { 26 } else { 16 }];
            if png {
                frame[4..8].copy_from_slice(&1u32.to_be_bytes());
                frame[8..12].copy_from_slice(&1u32.to_be_bytes());
            }
            chunk(&mut bytes, png, if png { b"fcTL" } else { b"ANMF" }, &frame);
        }
        if png {
            chunk(&mut bytes, true, b"IEND", &[]);
        } else {
            let length = (bytes.len() - 8) as u32;
            bytes[4..8].copy_from_slice(&length.to_le_bytes());
        }
        bytes
    }

    #[test]
    fn png_and_webp_animations_are_rejected_regardless_of_frame_count() {
        for png in [true, false] {
            for frames in [0, 1, 100, 101] {
                assert_eq!(
                    reject_chunked_animation(&animation_container(png, frames), png)
                        .unwrap_err()
                        .code,
                    "unsupported_image"
                );
            }
        }
    }

    #[test]
    fn static_containers_accept_valid_chunks_and_reject_truncation() {
        reject_chunked_animation(&png(), true).unwrap();
        // A bounded RIFF chunk containing ordinary still-image bytes.
        let webp = b"RIFF\x0c\0\0\0WEBPVP8 \0\0\0\0";
        reject_chunked_animation(webp, false).unwrap();
        for png in [true, false] {
            let bytes = if png {
                b"\x89PNG\r\n\x1a\nIHDR".to_vec()
            } else {
                webp[..webp.len() - 1].to_vec()
            };
            assert_eq!(
                reject_chunked_animation(&bytes, png).unwrap_err().code,
                "invalid_image"
            );
        }
    }

    #[test]
    fn webp_animation_flag_is_rejected_without_frame_chunks() {
        let mut webp = b"RIFF\x16\0\0\0WEBPVP8X\x0a\0\0\0".to_vec();
        webp.extend([0; 10]);
        reject_chunked_animation(&webp, false).unwrap();
        webp[20] = 0x02;
        assert_eq!(
            reject_chunked_animation(&webp, false).unwrap_err().code,
            "unsupported_image"
        );
    }

    #[test]
    fn avif_sequences_are_rejected_for_major_and_compatible_brands() {
        for major in [b"avif", b"avis"] {
            let mut bytes = 20u32.to_be_bytes().to_vec();
            bytes.extend(b"ftyp");
            bytes.extend(major);
            bytes.extend([0; 4]);
            bytes.extend(b"avis");
            assert_eq!(
                validate_avif_brands(&bytes).unwrap_err().code,
                "unsupported_image"
            );
        }
        let mut still = 20u32.to_be_bytes().to_vec();
        still.extend(b"ftypavif\0\0\0\0avif");
        validate_avif_brands(&still).unwrap();
    }

    fn jpeg_header() -> Vec<u8> {
        vec![
            0xff, 0xd8, 0xff, 0xc0, 0, 17, 8, 0, 1, 0, 1, 3, 1, 17, 0, 2, 17, 0, 3, 17, 0, 0xff,
            0xd9,
        ]
    }

    #[test]
    fn image_preview_rejects_small_files_with_huge_dimensions() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("huge.png");
        let mut image = png();
        // PNG IHDR dimensions; no pixel decoding or giant allocation needed.
        image[16..20].copy_from_slice(&5000u32.to_be_bytes());
        image[20..24].copy_from_slice(&5000u32.to_be_bytes());
        fs::write(&path, &image).unwrap();
        assert_eq!(
            read_image_preview(&path).unwrap_err().code,
            "image_too_large"
        );
        let markdown = dir.path().join("README.md");
        fs::write(&markdown, "").unwrap();
        assert_eq!(
            read_markdown_image(&markdown, "huge.png").unwrap_err().code,
            "image_too_large"
        );
    }

    #[test]
    fn image_dimensions_reject_zero_and_excessive_sides() {
        let mut image = png();
        image[16..20].copy_from_slice(&0u32.to_be_bytes());
        assert_eq!(
            validate_image_dimensions(&image).unwrap_err().code,
            "invalid_image"
        );
        image[16..20].copy_from_slice(&16385u32.to_be_bytes());
        assert_eq!(
            validate_image_dimensions(&image).unwrap_err().code,
            "image_too_large"
        );
    }

    #[test]
    fn image_dimensions_accept_the_pixel_limit_boundary() {
        let mut image = png();
        image[16..20].copy_from_slice(&4000u32.to_be_bytes());
        image[20..24].copy_from_slice(&4000u32.to_be_bytes());
        validate_image_dimensions(&image).unwrap();
        image[20..24].copy_from_slice(&4001u32.to_be_bytes());
        assert_eq!(
            validate_image_dimensions(&image).unwrap_err().code,
            "image_too_large"
        );
    }

    #[test]
    fn image_preview_rejects_unreadable_image_headers() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("invalid.png");
        fs::write(&path, "not an image").unwrap();
        assert_eq!(read_image_preview(&path).unwrap_err().code, "invalid_image");
    }

    #[test]
    fn listing_retains_valid_entries_when_another_entry_disappears() {
        let dir = tempfile::tempdir().unwrap();
        let valid = dir.path().join("valid.txt");
        fs::write(&valid, "content").unwrap();
        let (entries, skipped, warnings) = collect_listing_entries(
            vec![entry(&valid), entry(&dir.path().join("removed.txt"))].into_iter(),
        );
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "valid.txt");
        assert_eq!(skipped, 1);
        assert_eq!(warnings[0].code, "not_found");
    }

    #[test]
    fn listing_bounds_warning_samples_without_losing_the_count() {
        let (entries, skipped, warnings) = collect_listing_entries(
            (0..100).map(|_| Err(FsError::new("permission_denied", "Cannot read entry"))),
        );
        assert!(entries.is_empty());
        assert_eq!(skipped, 100);
        assert_eq!(warnings.len(), 20);
        let listing = Listing {
            path: "/example".into(),
            parent: None,
            entries,
            skipped_entries: skipped,
            warnings,
        };
        let json = serde_json::to_value(listing).unwrap();
        assert_eq!(json["skippedEntries"], 100);
        assert!(json.get("skipped_entries").is_none());
    }

    #[test]
    fn listing_still_rejects_directory_level_failures() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("file");
        fs::write(&file, "content").unwrap();
        assert!(list(&file).is_err());
        assert!(list(&dir.path().join("missing")).is_err());
    }

    // Linux filesystems permit arbitrary filename bytes; APFS rejects them.
    #[cfg(target_os = "linux")]
    #[test]
    fn listing_skips_non_unicode_names_without_hiding_valid_files() {
        use std::os::unix::ffi::OsStringExt;
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("valid.txt"), "content").unwrap();
        fs::write(
            dir.path().join(std::ffi::OsString::from_vec(vec![0xff])),
            "content",
        )
        .unwrap();
        let listing = list(dir.path()).unwrap();
        assert_eq!(listing.entries.len(), 1);
        assert_eq!(listing.skipped_entries, 1);
        assert_eq!(listing.warnings[0].code, "invalid_path");
    }

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
    fn directory_size_checks_cancellation_between_entries() {
        use std::cell::Cell;
        let temp = tempfile::tempdir().unwrap();
        for index in 0..10 {
            fs::write(temp.path().join(format!("file-{index}")), b"content").unwrap();
        }
        let checks = Cell::new(0);
        let error = directory_size_checked(temp.path(), || {
            checks.set(checks.get() + 1);
            if checks.get() >= 4 {
                Err(FsError::new("cancelled", "Directory sizing cancelled"))
            } else {
                Ok(())
            }
        })
        .unwrap_err();
        assert_eq!(error.code, "cancelled");
        assert_eq!(checks.get(), 4);
        assert_eq!(directory_size(temp.path()).unwrap(), 70);
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
        fs::write(&path, [0xff, 0x80]).unwrap();
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
    fn read_text_preview_decodes_utf16_logs() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("application.log");
        for little in [true, false] {
            for bom in [true, false] {
                let text = "2026-09-30 \u{1b}[32mINFO\u{1b}[0m Started\r\nNext line\tOK\n";
                let mut bytes = Vec::new();
                if bom {
                    bytes.extend(if little { [0xff, 0xfe] } else { [0xfe, 0xff] });
                }
                for unit in text.encode_utf16() {
                    bytes.extend(if little {
                        unit.to_le_bytes()
                    } else {
                        unit.to_be_bytes()
                    });
                }
                fs::write(&path, bytes).unwrap();
                assert_eq!(read_text_preview(&path).unwrap(), text);
            }
        }
        let text = "Log: café 日本語 😀\n";
        let mut bytes = vec![0xff, 0xfe];
        for unit in text.encode_utf16() {
            bytes.extend(unit.to_le_bytes());
        }
        fs::write(&path, bytes).unwrap();
        assert_eq!(read_text_preview(&path).unwrap(), text);
    }

    #[test]
    fn read_text_preview_rejects_invalid_utf16_and_binary_controls() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("sample.log");
        for bytes in [vec![0xff, 0xfe, 65], vec![0xff, 0xfe, 0, 0xd8]] {
            fs::write(&path, bytes).unwrap();
            assert_eq!(
                read_text_preview(&path).unwrap_err().code,
                "invalid_encoding"
            );
        }
        fs::write(&path, [0xff, 0xfe, 0, 0]).unwrap();
        assert_eq!(read_text_preview(&path).unwrap_err().code, "binary_file");
    }

    #[test]
    fn read_text_preview_accepts_large_utf16_terminal_logs() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("large.log");
        let text = "\u{1b}[32mINFO\u{1b}[0m Log output\r\n".repeat(60_000);
        let mut bytes = vec![0xff, 0xfe];
        for unit in text.encode_utf16() {
            bytes.extend(unit.to_le_bytes());
        }
        assert!(bytes.len() > 2 * 1024 * 1024);
        fs::write(&path, bytes).unwrap();
        assert_eq!(read_text_preview(&path).unwrap(), text);
    }

    #[test]
    #[ignore = "Set MYCMD_LOG_TEST_DIR to validate external log fixtures"]
    fn read_text_preview_external_logs() {
        let directory = std::env::var("MYCMD_LOG_TEST_DIR").expect("Set MYCMD_LOG_TEST_DIR");
        let mut count = 0;
        for item in fs::read_dir(directory).unwrap() {
            let path = item.unwrap().path();
            if path
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("log"))
            {
                let text = read_text_preview(&path)
                    .unwrap_or_else(|e| panic!("{}: {:?}", path.display(), e));
                assert!(!text.is_empty(), "{}", path.display());
                count += 1;
            }
        }
        assert!(count > 0, "No log fixtures found");
    }

    #[test]
    fn read_markdown_image_returns_a_data_url_for_relative_raster_image() {
        let temp = tempfile::tempdir().unwrap();
        let markdown = temp.path().join("README.md");
        let image = temp.path().join("diagram.png");
        fs::write(&markdown, "![diagram](diagram.png)").unwrap();
        fs::write(&image, png()).unwrap();

        let image = read_markdown_image(&markdown, "diagram.png").unwrap();
        assert_eq!((image.width, image.height), (1, 1));
        assert_eq!(
            image.data_url,
            format!("data:image/png;base64,{PNG_BASE64}")
        );
        let json = serde_json::to_value(image).unwrap();
        assert!(json["dataUrl"]
            .as_str()
            .unwrap()
            .starts_with("data:image/png"));
        assert_eq!(json["width"], 1);
        assert_eq!(json["height"], 1);
        assert_eq!(json["frames"], 1);
    }

    #[test]
    fn read_markdown_image_decodes_spaces_in_relative_paths() {
        let temp = tempfile::tempdir().unwrap();
        let markdown = temp.path().join("README.md");
        fs::write(&markdown, "").unwrap();
        fs::write(temp.path().join("my image.jpg"), jpeg_header()).unwrap();

        assert!(read_markdown_image(&markdown, "my%20image.jpg")
            .unwrap()
            .data_url
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
        fs::write(&path, jpeg_header()).unwrap();

        assert_eq!(
            read_image_preview(&path).unwrap(),
            format!("data:image/jpeg;base64,{}", STANDARD.encode(jpeg_header()))
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

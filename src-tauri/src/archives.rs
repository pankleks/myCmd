use crate::{
    error::{FsError, Result},
    filesystem::{self, Entry, Listing},
};
use std::{collections::BTreeMap, fs::File, path::Path};

const MAX_ENTRIES: usize = 100_000;

struct CheckedReader<'a> {
    reader: &'a mut dyn std::io::Read,
    check: &'a dyn Fn() -> Result<()>,
    budget: &'a mut u64,
}
impl std::io::Read for CheckedReader<'_> {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        (self.check)().map_err(|error| std::io::Error::other(error.message))?;
        let size = self.reader.read(buffer)?;
        *self.budget = self.budget.saturating_add(size as u64);
        if *self.budget > 8 * 1024 * 1024 * 1024 {
            return Err(std::io::Error::other(
                "Archive rewrite exceeds the 8 GiB safety limit",
            ));
        }
        Ok(size)
    }
}

pub fn delete_selected(
    path: &Path,
    directory: &str,
    members: &[String],
    check: impl Fn() -> Result<()>,
) -> Result<()> {
    check()?;
    if safe_name(directory).is_none()
        || (!directory.is_empty() && !directory.ends_with('/'))
        || members.is_empty()
    {
        return Err(FsError::new(
            "invalid_path",
            "Invalid archive deletion selection",
        ));
    }
    for member in members {
        filesystem::validate_name(member)?;
    }
    let path = filesystem::absolute(path)?;
    let original = std::fs::symlink_metadata(&path).map_err(|e| FsError::io(e, &path))?;
    if !original.is_file() || original.file_type().is_symlink() || original.permissions().readonly()
    {
        return Err(FsError::new(
            "invalid_path",
            "Archive must be a writable regular file",
        ));
    }
    let error = |e: String| FsError::new("archive_error", e);
    let mut removed = std::collections::BTreeSet::new();
    let mut keep = |name: &str| -> Result<bool> {
        let safe = safe_name(name).ok_or_else(|| error("Unsafe archive member path".into()))?;
        let selected = safe
            .strip_prefix(directory)
            .and_then(|relative| relative.split('/').next())
            .filter(|first| members.iter().any(|member| member == first));
        if let Some(name) = selected {
            removed.insert(name.to_owned());
        }
        Ok(selected.is_none())
    };
    let mut replacement = tempfile::NamedTempFile::new_in(path.parent().unwrap())
        .map_err(|e| FsError::io(e, &path))?;
    if path
        .extension()
        .unwrap_or_default()
        .to_string_lossy()
        .eq_ignore_ascii_case("zip")
    {
        let mut archive =
            zip::ZipArchive::new(File::open(&path).map_err(|e| FsError::io(e, &path))?)
                .map_err(|e| error(e.to_string()))?;
        if archive.len() > MAX_ENTRIES {
            return Err(error("Too many archive entries".into()));
        }
        let mut writer = zip::ZipWriter::new(replacement.as_file_mut());
        writer.set_raw_comment(archive.comment().to_vec().into());
        for i in 0..archive.len() {
            check()?;
            let entry = archive.by_index_raw(i).map_err(|e| error(e.to_string()))?;
            if keep(entry.name())? {
                writer
                    .raw_copy_file(entry)
                    .map_err(|e| error(e.to_string()))?;
            }
        }
        writer.finish().map_err(|e| error(e.to_string()))?;
        // Validate retained members before committing the replacement.
        let mut verify = zip::ZipArchive::new(
            File::open(replacement.path()).map_err(|e| FsError::io(e, &path))?,
        )
        .map_err(|e| error(e.to_string()))?;
        let mut budget = 0;
        for i in 0..verify.len() {
            check()?;
            let mut entry = verify.by_index(i).map_err(|e| error(e.to_string()))?;
            std::io::copy(
                &mut CheckedReader {
                    reader: &mut entry,
                    check: &check,
                    budget: &mut budget,
                },
                &mut std::io::sink(),
            )
            .map_err(|e| error(e.to_string()))?;
        }
    } else if path
        .extension()
        .unwrap_or_default()
        .to_string_lossy()
        .eq_ignore_ascii_case("7z")
    {
        let mut archive = sevenz_rust2::ArchiveReader::open(&path, sevenz_rust2::Password::empty())
            .map_err(|e| error(e.to_string()))?;
        if archive.archive().files.len() > MAX_ENTRIES {
            return Err(error("Too many archive entries".into()));
        }
        let mut writer = sevenz_rust2::ArchiveWriter::new(replacement.as_file_mut())
            .map_err(|e| error(e.to_string()))?;
        let mut budget = 0;
        let mut failure = None;
        archive
            .for_each_entries(|entry, reader| {
                if failure.is_some() {
                    return Ok(false);
                }
                let result = (|| -> Result<()> {
                    check()?;
                    if entry.is_anti_item {
                        return Err(error("7z anti-items are not supported for deletion".into()));
                    }
                    let retained = keep(&entry.name)?;
                    let mut checked = CheckedReader {
                        reader,
                        check: &check,
                        budget: &mut budget,
                    };
                    if retained {
                        writer
                            .push_archive_entry(entry.clone(), Some(&mut checked))
                            .map_err(|e| error(e.to_string()))?;
                    }
                    // Deleted members still need draining in solid blocks.
                    std::io::copy(&mut checked, &mut std::io::sink())
                        .map_err(|e| error(e.to_string()))?;
                    Ok(())
                })();
                if let Err(error) = result {
                    failure = Some(error);
                    return Ok(false);
                }
                Ok(true)
            })
            .map_err(|e| error(e.to_string()))?;
        if let Some(failure) = failure {
            return Err(failure);
        }
        writer.finish().map_err(|e| error(e.to_string()))?;
        let mut verify =
            sevenz_rust2::ArchiveReader::open(replacement.path(), sevenz_rust2::Password::empty())
                .map_err(|e| error(e.to_string()))?;
        budget = 0;
        verify
            .for_each_entries(|_, reader| {
                std::io::copy(
                    &mut CheckedReader {
                        reader,
                        check: &check,
                        budget: &mut budget,
                    },
                    &mut std::io::sink(),
                )?;
                Ok(true)
            })
            .map_err(|e| error(e.to_string()))?;
    } else {
        return Err(error("Only ZIP and 7z are supported".into()));
    }
    if members.iter().any(|member| !removed.contains(member)) {
        return Err(error("Selected archive member no longer exists".into()));
    }
    check()?;
    let current = std::fs::symlink_metadata(&path).map_err(|e| FsError::io(e, &path))?;
    if !current.is_file()
        || current.len() != original.len()
        || current.modified().ok() != original.modified().ok()
    {
        return Err(error(
            "Archive changed during rewrite; original was not replaced".into(),
        ));
    }
    replacement
        .as_file()
        .set_permissions(original.permissions())
        .map_err(|e| FsError::io(e, &path))?;
    replacement
        .as_file()
        .sync_all()
        .map_err(|e| FsError::io(e, &path))?;
    replacement
        .persist(&path)
        .map_err(|e| error(e.to_string()))?;
    Ok(())
}

/// Decode into private staging, then let the regular copy worker handle
/// conflicts and commit files. Never extract archive paths into the target.
pub fn extract_selected(
    archive_path: &Path,
    directory: &str,
    members: &[String],
    staging: &Path,
    check: impl Fn() -> Result<()>,
) -> Result<Vec<std::path::PathBuf>> {
    use std::io::{Read, Write};
    if safe_name(directory).is_none()
        || (!directory.is_empty() && !directory.ends_with('/'))
        || members.is_empty()
    {
        return Err(FsError::new("invalid_path", "Invalid extraction selection"));
    }
    for member in members {
        filesystem::validate_name(member)?;
    }
    let archive_path = filesystem::absolute(archive_path)?;
    let mut count = 0usize;
    let mut written = 0u64;
    let mut found = std::collections::BTreeSet::new();
    let mut paths = std::collections::BTreeSet::new();
    let mut extract = |name: &str, folder: bool, reader: &mut dyn Read| -> Result<()> {
        check()?;
        count += 1;
        if count > MAX_ENTRIES {
            return Err(FsError::new("archive_limit", "Too many archive entries"));
        }
        let Some(name) = safe_name(name) else {
            return Err(FsError::new("invalid_path", "Unsafe archive member path"));
        };
        let Some(relative) = name.strip_prefix(directory) else {
            return Ok(());
        };
        let relative = relative.trim_end_matches('/');
        let first = relative.split('/').next().unwrap_or_default();
        if !members.iter().any(|member| member == first) {
            return Ok(());
        }
        found.insert(first.to_owned());
        if relative.is_empty() || relative.split('/').any(|part| part.is_empty()) {
            return Err(FsError::new(
                "invalid_path",
                "Ambiguous archive member path",
            ));
        }
        let target = staging.join(relative);
        for component in relative.split('/') {
            filesystem::validate_name(component)?;
        }
        if folder {
            std::fs::create_dir_all(&target).map_err(|e| FsError::io(e, &target))?;
        } else {
            if !paths.insert(relative.to_owned()) {
                return Err(FsError::new("invalid_path", "Duplicate archive member"));
            }
            std::fs::create_dir_all(target.parent().unwrap())
                .map_err(|e| FsError::io(e, &target))?;
            let mut output = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&target)
                .map_err(|e| FsError::io(e, &target))?;
            let mut buffer = [0u8; 64 * 1024];
            loop {
                check()?;
                let size = reader
                    .read(&mut buffer)
                    .map_err(|e| FsError::io(e, &target))?;
                if size == 0 {
                    break;
                }
                written += size as u64;
                if written > 8 * 1024 * 1024 * 1024 {
                    return Err(FsError::new(
                        "archive_limit",
                        "Extraction exceeds the 8 GiB safety limit",
                    ));
                }
                output
                    .write_all(&buffer[..size])
                    .map_err(|e| FsError::io(e, &target))?;
            }
        }
        Ok(())
    };
    let archive_error = |e: String| FsError::new("archive_error", e);
    if archive_path
        .extension()
        .unwrap_or_default()
        .to_string_lossy()
        .eq_ignore_ascii_case("zip")
    {
        let mut archive = zip::ZipArchive::new(
            File::open(&archive_path).map_err(|e| FsError::io(e, &archive_path))?,
        )
        .map_err(|e| archive_error(e.to_string()))?;
        if archive.len() > MAX_ENTRIES {
            return Err(archive_error("Too many archive entries".into()));
        }
        for i in 0..archive.len() {
            check()?;
            // Raw metadata allows unselected encrypted files to be skipped.
            let raw = archive
                .by_index_raw(i)
                .map_err(|e| archive_error(e.to_string()))?;
            let name = raw.name().to_owned();
            let folder = raw.is_dir();
            drop(raw);
            let Some(safe) = safe_name(&name) else {
                return Err(archive_error("Unsafe archive member path".into()));
            };
            if !safe.strip_prefix(directory).is_some_and(|relative| {
                members
                    .iter()
                    .any(|m| relative.split('/').next() == Some(m.as_str()))
            }) {
                continue;
            }
            let mut entry = archive
                .by_index(i)
                .map_err(|e| archive_error(e.to_string()))?;
            if entry.is_symlink() {
                return Err(archive_error("Archive symlinks cannot be extracted".into()));
            }
            extract(&name, folder, &mut entry)?;
        }
    } else if archive_path
        .extension()
        .unwrap_or_default()
        .to_string_lossy()
        .eq_ignore_ascii_case("7z")
    {
        let mut archive =
            sevenz_rust2::ArchiveReader::open(&archive_path, sevenz_rust2::Password::empty())
                .map_err(|e| archive_error(e.to_string()))?;
        if archive.archive().files.len() > MAX_ENTRIES {
            return Err(archive_error("Too many archive entries".into()));
        }
        let mut failure = None;
        archive
            .for_each_entries(|entry, reader| {
                if failure.is_some() {
                    return Ok(false);
                }
                if entry.is_anti_item {
                    return Ok(true);
                }
                match extract(&entry.name, entry.is_directory, reader) {
                    Ok(()) => {
                        // Solid blocks share a decoder. Every member must be
                        // consumed, even when it is outside the selection.
                        let mut buffer = [0u8; 64 * 1024];
                        loop {
                            if let Err(error) = check() {
                                failure = Some(error);
                                return Ok(false);
                            }
                            match reader.read(&mut buffer) {
                                Ok(0) => break,
                                Ok(_) => {}
                                Err(error) => {
                                    failure =
                                        Some(FsError::new("archive_error", error.to_string()));
                                    return Ok(false);
                                }
                            }
                        }
                        Ok(true)
                    }
                    Err(error) => {
                        failure = Some(error);
                        Ok(false)
                    }
                }
            })
            .map_err(|e| archive_error(e.to_string()))?;
        if let Some(error) = failure {
            return Err(error);
        }
    } else {
        return Err(archive_error("Only ZIP and 7z are supported".into()));
    }
    if members.iter().any(|member| !found.contains(member)) {
        return Err(archive_error(
            "Selected archive member no longer exists".into(),
        ));
    }
    Ok(found.into_iter().map(|name| staging.join(name)).collect())
}

fn safe_name(name: &str) -> Option<String> {
    let name = name.replace('\\', "/");
    if name.starts_with('/')
        || name.contains(':')
        || name.contains('\0')
        || name.split('/').any(|part| part == ".." || part == ".")
    {
        return None;
    }
    Some(name)
}

fn listing(path: &Path, directory: &str, members: Vec<(String, bool, u64)>) -> Result<Listing> {
    let mut entries = BTreeMap::<String, Entry>::new();
    let mut skipped = 0;
    for (name, is_directory, size) in members {
        let Some(name) = safe_name(&name) else {
            skipped += 1;
            continue;
        };
        let Some(relative) = name.strip_prefix(directory) else {
            continue;
        };
        let Some(first) = relative.split('/').next().filter(|s| !s.is_empty()) else {
            continue;
        };
        let folder = is_directory || relative.contains('/');
        let entry = Entry {
            name: first.to_owned(),
            path: String::new(),
            r#type: if folder { "directory" } else { "file" }.into(),
            extension: if folder {
                String::new()
            } else {
                Path::new(first)
                    .extension()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned()
            },
            size: if folder { 0 } else { size },
            modified: None,
            created: None,
            hidden: first.starts_with('.'),
            readonly: true,
            directory_target: folder,
        };
        match entries.get(first) {
            Some(previous) if previous.directory_target || !folder => {}
            _ => {
                entries.insert(first.to_owned(), entry);
            }
        }
    }
    Ok(Listing {
        path: filesystem::text(path),
        parent: path.parent().map(filesystem::text),
        entries: entries.into_values().collect(),
        skipped_entries: skipped,
        warnings: vec![],
    })
}

#[tauri::command]
pub async fn list_archive(archive_path: String, directory: String) -> Result<Listing> {
    tauri::async_runtime::spawn_blocking(move || {
        let path = filesystem::absolute(Path::new(&archive_path))?;
        if safe_name(&directory).is_none() || (!directory.is_empty() && !directory.ends_with('/')) {
            return Err(FsError::new("invalid_path", "Invalid archive directory"));
        }
        let error = |e: String| {
            FsError::new(
                "archive_error",
                format!("Cannot browse archive (corrupt, encrypted or unsupported): {e}"),
            )
        };
        let mut members = Vec::new();
        if path
            .extension()
            .unwrap_or_default()
            .to_string_lossy()
            .eq_ignore_ascii_case("zip")
        {
            let file = File::open(&path).map_err(|e| FsError::io(e, &path))?;
            let mut archive = zip::ZipArchive::new(file).map_err(|e| error(e.to_string()))?;
            if archive.len() > MAX_ENTRIES {
                return Err(error("Too many archive entries".into()));
            }
            for i in 0..archive.len() {
                let entry = archive.by_index_raw(i).map_err(|e| error(e.to_string()))?;
                members.push((entry.name().to_owned(), entry.is_dir(), entry.size()));
            }
        } else if path
            .extension()
            .unwrap_or_default()
            .to_string_lossy()
            .eq_ignore_ascii_case("7z")
        {
            let reader = sevenz_rust2::ArchiveReader::open(&path, sevenz_rust2::Password::empty())
                .map_err(|e| error(e.to_string()))?;
            if reader.archive().files.len() > MAX_ENTRIES {
                return Err(error("Too many archive entries".into()));
            }
            for entry in &reader.archive().files {
                if !entry.is_anti_item {
                    members.push((entry.name.clone(), entry.is_directory, entry.size));
                }
            }
        } else {
            return Err(error("Only ZIP and 7z are supported".into()));
        }
        listing(&path, &directory, members)
    })
    .await
    .map_err(|e| FsError::new("archive_error", e.to_string()))?
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selectively_extracts_later_members_of_a_solid_sevenz_block() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("solid.7z");
        let mut writer = sevenz_rust2::ArchiveWriter::create(&path).unwrap();
        writer
            .push_archive_entries(
                vec![
                    sevenz_rust2::ArchiveEntry::new_file("skip.txt"),
                    sevenz_rust2::ArchiveEntry::new_file("gulpfile.js"),
                ],
                vec![
                    sevenz_rust2::SourceReader::new(std::io::Cursor::new(
                        b"unselected bytes".to_vec(),
                    )),
                    sevenz_rust2::SourceReader::new(std::io::Cursor::new(
                        b"selected contents".to_vec(),
                    )),
                ],
            )
            .unwrap();
        writer.finish().unwrap();
        let target = tempfile::tempdir().unwrap();
        extract_selected(&path, "", &["gulpfile.js".into()], target.path(), || Ok(())).unwrap();
        assert_eq!(
            std::fs::read(target.path().join("gulpfile.js")).unwrap(),
            b"selected contents"
        );
        assert!(!target.path().join("skip.txt").exists());
        let before = std::fs::read(&path).unwrap();
        assert!(
            delete_selected(&path, "", &["skip.txt".into()], || Err(FsError::new(
                "cancelled",
                "Cancelled"
            )))
            .is_err()
        );
        assert_eq!(std::fs::read(&path).unwrap(), before);
        delete_selected(&path, "", &["skip.txt".into()], || Ok(())).unwrap();
        let mut reader =
            sevenz_rust2::ArchiveReader::open(&path, sevenz_rust2::Password::empty()).unwrap();
        assert_eq!(reader.archive().files.len(), 1);
        assert_eq!(
            reader.read_file("gulpfile.js").unwrap(),
            b"selected contents"
        );
    }
    #[test]
    fn extracts_only_selected_files_and_rejects_unsafe_paths() {
        use std::io::Write;
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("selected.zip");
        let mut zip = zip::ZipWriter::new(File::create(&path).unwrap());
        for name in ["docs/a.txt", "docs/b.txt", "other.txt"] {
            zip.start_file(name, zip::write::SimpleFileOptions::default())
                .unwrap();
            zip.write_all(b"hello").unwrap();
        }
        zip.finish().unwrap();
        let target = tempfile::tempdir().unwrap();
        extract_selected(&path, "docs/", &["a.txt".into()], target.path(), || Ok(())).unwrap();
        assert_eq!(
            std::fs::read(target.path().join("a.txt")).unwrap(),
            b"hello"
        );
        assert!(!target.path().join("b.txt").exists());
        assert!(!target.path().join("other.txt").exists());
        assert!(
            extract_selected(&path, "", &["../escape".into()], target.path(), || Ok(())).is_err()
        );
        assert!(
            extract_selected(&path, "", &["docs".into()], target.path(), || Err(
                FsError::new("cancelled", "Cancelled")
            ))
            .is_err()
        );
    }
    #[test]
    fn browses_real_zip_and_sevenz_archives() {
        use std::io::Write;
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("source");
        std::fs::create_dir_all(source.join("nested")).unwrap();
        std::fs::write(source.join("nested/file.txt"), b"hello").unwrap();
        let zip_path = temp.path().join("sample.zip");
        let mut zip = zip::ZipWriter::new(File::create(&zip_path).unwrap());
        zip.start_file("nested/file.txt", zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(b"hello").unwrap();
        zip.finish().unwrap();
        let sevenz_path = temp.path().join("sample.7z");
        sevenz_rust2::compress_to_path(&source, &sevenz_path).unwrap();
        for path in [zip_path, sevenz_path] {
            let result = tauri::async_runtime::block_on(list_archive(
                filesystem::text(&path),
                String::new(),
            ))
            .unwrap();
            assert!(result.entries.iter().any(|entry| entry.directory_target));
            let target = tempfile::tempdir().unwrap();
            let members: Vec<String> = result
                .entries
                .iter()
                .map(|entry| entry.name.clone())
                .collect();
            let sources = extract_selected(&path, "", &members, target.path(), || Ok(())).unwrap();
            assert!(!sources.is_empty());
            assert!(sources.iter().all(|path| path.exists()));
            delete_selected(&path, "", &members, || Ok(())).unwrap();
            let after = tauri::async_runtime::block_on(list_archive(
                filesystem::text(&path),
                String::new(),
            ))
            .unwrap();
            assert!(after.entries.is_empty());
        }
    }
    #[test]
    fn synthesizes_directories_and_rejects_unsafe_members() {
        let result = listing(
            Path::new("test.zip"),
            "",
            vec![
                ("folder/file.txt".into(), false, 42),
                ("../bad".into(), false, 1),
            ],
        )
        .unwrap();
        assert_eq!(result.entries.len(), 1);
        assert_eq!(result.entries[0].name, "folder");
        assert!(result.entries[0].directory_target);
        assert_eq!(result.skipped_entries, 1);
        let nested = listing(
            Path::new("test.zip"),
            "folder/",
            vec![("folder/file.txt".into(), false, 42)],
        )
        .unwrap();
        assert_eq!(nested.entries[0].size, 42);
    }
}

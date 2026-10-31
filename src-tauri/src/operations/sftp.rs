use super::*;
use crate::sftp::{self as remote, error, join, name, parent, remote_name, remote_path};
use ssh2::{FileStat, OpenFlags, OpenType, RenameFlags, Sftp};

#[derive(Clone, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum SftpOperation {
    Copy {
        sources: Vec<String>,
        destination: String,
        upload: bool,
    },
    Rename {
        path: String,
        name: String,
    },
    CreateDirectory {
        parent: String,
        name: String,
    },
    Delete {
        sources: Vec<String>,
    },
}

struct Node {
    source: String,
    relative: String,
    entry: Entry,
    stat: Option<FileStat>,
}

fn stat(sftp: &Sftp, path: &str) -> Result<Option<FileStat>> {
    match sftp.lstat(Path::new(path)) {
        Ok(stat) => Ok(Some(stat)),
        Err(e) if matches!(e.code(), ssh2::ErrorCode::SFTP(2 | 10)) => Ok(None),
        Err(e) => Err(error(e)),
    }
}

fn destination_entry(sftp: &Sftp, path: &str, upload: bool) -> Result<Option<Entry>> {
    if upload {
        stat(sftp, path)?
            .map(|stat| remote::entry(path, &stat))
            .transpose()
    } else {
        match fs::symlink_metadata(path) {
            Ok(_) => filesystem::entry(Path::new(path)).map(Some),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(FsError::io(e, Path::new(path))),
        }
    }
}

fn target_path(parent: &str, name: &str, upload: bool) -> Result<String> {
    if upload {
        join(parent, name)
    } else {
        filesystem::validate_name(name)?;
        Ok(filesystem::text(&Path::new(parent).join(name)))
    }
}

fn remote_sources(mut sources: Vec<String>) -> Result<Vec<String>> {
    if sources.is_empty() {
        return Err(FsError::new("invalid_path", "No source items"));
    }
    for source in &mut sources {
        remote_path(source)?;
        *source = source.trim_end_matches('/').to_owned();
        name(source)?;
    }
    sources.sort();
    sources.dedup();
    let mut result: Vec<String> = Vec::new();
    for source in sources {
        if !result
            .iter()
            .any(|parent| source.starts_with(&format!("{parent}/")))
        {
            result.push(source);
        }
    }
    Ok(result)
}

impl Worker {
    pub(super) fn run_sftp(&mut self, sftp: &Sftp, operation: SftpOperation) -> Result<()> {
        self.check()?;
        match operation {
            SftpOperation::Copy {
                sources,
                destination,
                upload,
            } => self.copy_sftp(sftp, sources, destination, upload)?,
            SftpOperation::CreateDirectory { parent, name } => {
                let path = join(&parent, &name)?;
                sftp.mkdir(Path::new(&path), 0o755).map_err(error)?;
            }
            SftpOperation::Rename {
                path,
                name: new_name,
            } => {
                name(&path)?;
                let target = join(&parent(&path), &new_name)?;
                if stat(sftp, &target)?.is_some() {
                    return Err(FsError::new(
                        "already_exists",
                        "Remote rename does not overwrite existing items",
                    ));
                }
                sftp.rename(
                    Path::new(&path),
                    Path::new(&target),
                    Some(RenameFlags::empty()),
                )
                .map_err(error)?;
            }
            SftpOperation::Delete { sources } => {
                let mut nodes = Vec::new();
                for source in remote_sources(sources)? {
                    self.remote_nodes(sftp, &source, name(&source)?, &mut nodes, true)?;
                }
                self.progress.total_items = nodes.len() as u64;
                self.emit();
                for node in nodes.into_iter().rev() {
                    self.check()?;
                    // lstat never follows a symbolic link when deleting.
                    let current = sftp.lstat(Path::new(&node.source)).map_err(error)?;
                    if current.is_dir() {
                        sftp.rmdir(Path::new(&node.source))
                    } else {
                        sftp.unlink(Path::new(&node.source))
                    }
                    .map_err(error)?;
                    self.record_progress(Path::new(&node.source), 0, 1);
                }
            }
        }
        Ok(())
    }

    fn remote_nodes(
        &self,
        sftp: &Sftp,
        source: &str,
        relative: &str,
        nodes: &mut Vec<Node>,
        deleting: bool,
    ) -> Result<()> {
        let mut pending = vec![(source.to_owned(), relative.to_owned())];
        while let Some((source, relative)) = pending.pop() {
            self.check()?;
            if nodes.len() >= 100_000 {
                return Err(FsError::new(
                    "sftp_limit",
                    "Operation exceeds 100,000 items",
                ));
            }
            let stat = sftp.lstat(Path::new(&source)).map_err(error)?;
            let entry = remote::entry(&source, &stat)?;
            if entry.r#type == "symlink" && !deleting {
                return Err(FsError::new(
                    "invalid_path",
                    "SFTP transfers do not follow or copy symbolic links",
                ));
            }
            if stat.is_dir() {
                for (child, _) in remote::directory(sftp, &source, || self.check())? {
                    if nodes.len() + pending.len() >= 100_000 {
                        return Err(FsError::new(
                            "sftp_limit",
                            "Operation exceeds 100,000 items",
                        ));
                    }
                    pending.push((join(&source, &child)?, format!("{relative}/{child}")));
                }
            }
            nodes.push(Node {
                source,
                relative,
                entry,
                stat: Some(stat),
            });
        }
        Ok(())
    }

    fn local_nodes(&self, sources: Vec<String>) -> Result<Vec<Node>> {
        let mut nodes = Vec::new();
        let mut pending = Vec::new();
        for path in normalize_sources(sources.into_iter().map(PathBuf::from).collect())? {
            let relative = path
                .file_name()
                .and_then(|name| name.to_str())
                .ok_or_else(|| FsError::new("invalid_path", "SFTP filenames must be Unicode"))?
                .to_owned();
            pending.push((path, relative));
        }
        while let Some((path, relative)) = pending.pop() {
            self.check()?;
            if nodes.len() >= 100_000 {
                return Err(FsError::new(
                    "sftp_limit",
                    "Operation exceeds 100,000 items",
                ));
            }
            let entry = filesystem::entry(&path)?;
            remote_name(&entry.name)?;
            let metadata = fs::symlink_metadata(&path).map_err(|e| FsError::io(e, &path))?;
            if metadata.is_symlink() || (!metadata.is_file() && !metadata.is_dir()) {
                return Err(FsError::new(
                    "invalid_path",
                    "SFTP transfers support regular files and folders only, not symbolic links",
                ));
            }
            if metadata.is_dir() {
                for child in fs::read_dir(&path).map_err(|e| FsError::io(e, &path))? {
                    if nodes.len() + pending.len() >= 100_000 {
                        return Err(FsError::new(
                            "sftp_limit",
                            "Operation exceeds 100,000 items",
                        ));
                    }
                    let child = child.map_err(|e| FsError::io(e, &path))?;
                    let child_name = child.file_name().into_string().map_err(|_| {
                        FsError::new("invalid_path", "SFTP filenames must be Unicode")
                    })?;
                    remote_name(&child_name)?;
                    pending.push((child.path(), format!("{relative}/{child_name}")));
                }
            }
            nodes.push(Node {
                source: filesystem::text(&path),
                relative,
                entry,
                stat: None,
            });
        }
        Ok(nodes)
    }

    fn copy_sftp(
        &mut self,
        sftp: &Sftp,
        sources: Vec<String>,
        destination: String,
        upload: bool,
    ) -> Result<()> {
        let mut nodes = if upload {
            self.local_nodes(sources)?
        } else {
            let mut nodes = Vec::new();
            for source in remote_sources(sources)? {
                self.remote_nodes(sftp, &source, name(&source)?, &mut nodes, false)?;
            }
            nodes
        };
        let destination = if upload {
            remote_path(&destination)?;
            let metadata = sftp.stat(Path::new(&destination)).map_err(error)?;
            if !metadata.is_dir() {
                return Err(FsError::new(
                    "invalid_path",
                    "Destination must be a remote folder",
                ));
            }
            destination
        } else {
            let destination = filesystem::absolute(Path::new(&destination))?;
            if !destination.is_dir() {
                return Err(FsError::new(
                    "invalid_path",
                    "Destination must be a local folder",
                ));
            }
            filesystem::text(&destination)
        };
        let mut roots = std::collections::HashSet::new();
        for node in &nodes {
            if !node.relative.contains('/') && !roots.insert(node.relative.clone()) {
                return Err(FsError::new(
                    "invalid_path",
                    "Selected items have duplicate names",
                ));
            }
            if !upload {
                for part in node.relative.split('/') {
                    filesystem::validate_name(part)?;
                }
            }
            self.progress.total_bytes = self
                .progress
                .total_bytes
                .checked_add(node.entry.size)
                .ok_or_else(|| FsError::new("size_overflow", "Transfer is too large"))?;
        }
        self.progress.total_items = nodes.len() as u64;
        self.emit();
        // Lexical order ensures each parent is handled before its descendants.
        nodes.sort_by(|a, b| a.relative.cmp(&b.relative));
        let mut targets: HashMap<String, String> = HashMap::new();
        let mut skipped = Vec::<String>::new();
        for node in nodes {
            self.check()?;
            if skipped
                .iter()
                .any(|path| node.relative.starts_with(&format!("{path}/")))
            {
                self.tick(Path::new(&node.source), node.entry.size, 1)?;
                continue;
            }
            let (relative_parent, base) = node
                .relative
                .rsplit_once('/')
                .unwrap_or(("", &node.relative));
            let target_parent = targets
                .get(relative_parent)
                .map(String::as_str)
                .unwrap_or(&destination);
            let mut target = target_path(target_parent, base, upload)?;
            let mut overwrite = false;
            let mut skip = false;
            while let Some(existing) = destination_entry(sftp, &target, upload)? {
                self.check()?;
                let answer = self.ask_entries(node.entry.clone(), existing.clone())?;
                match answer.action.as_str() {
                    "skip" => {
                        skip = true;
                        break;
                    }
                    "cancel" => return Err(FsError::new("cancelled", "Transfer cancelled")),
                    "rename" => {
                        if let Some(name) = answer.name {
                            target = target_path(target_parent, &name, upload)?;
                        } else {
                            let mut n = 1;
                            loop {
                                self.check()?;
                                target =
                                    target_path(target_parent, &format!("{base} ({n})"), upload)?;
                                if destination_entry(sftp, &target, upload)?.is_none() {
                                    break;
                                }
                                n += 1;
                            }
                        }
                    }
                    "overwrite" => {
                        if existing.r#type != node.entry.r#type || existing.r#type == "symlink" {
                            return Err(FsError::new(
                                "already_exists",
                                "Cannot overwrite a different item type or a symbolic link",
                            ));
                        }
                        overwrite = true;
                        break;
                    }
                    _ => return Err(FsError::new("invalid_path", "Unknown conflict action")),
                }
            }
            if skip {
                if node.entry.r#type == "directory" {
                    skipped.push(node.relative);
                }
                self.tick(Path::new(&node.source), node.entry.size, 1)?;
                continue;
            }
            if node.entry.r#type == "directory" {
                if !overwrite {
                    if upload {
                        sftp.mkdir(Path::new(&target), 0o700).map_err(error)?;
                    } else {
                        let mut builder = fs::DirBuilder::new();
                        #[cfg(unix)]
                        {
                            use std::os::unix::fs::DirBuilderExt;
                            builder.mode(0o700);
                        }
                        builder
                            .create(&target)
                            .map_err(|e| FsError::io(e, Path::new(&target)))?;
                    }
                }
                targets.insert(node.relative, target);
            } else if upload {
                self.upload(sftp, &node.source, &target, overwrite)?;
            } else {
                self.download(
                    sftp,
                    &node.source,
                    &target,
                    node.stat.as_ref().unwrap(),
                    overwrite,
                )?;
            }
            self.record_progress(Path::new(&node.source), 0, 1);
        }
        Ok(())
    }

    fn upload(&mut self, sftp: &Sftp, source: &str, target: &str, overwrite: bool) -> Result<()> {
        let source = Path::new(source);
        let metadata = fs::symlink_metadata(source).map_err(|e| FsError::io(e, source))?;
        if !metadata.is_file() || metadata.is_symlink() {
            return Err(source_changed(source));
        }
        let mut input = fs::File::open(source).map_err(|e| FsError::io(e, source))?;
        let snapshot = SourceSnapshot::new(&input, source)?;
        if !unchanged_metadata(&metadata, &snapshot.metadata) {
            return Err(source_changed(source));
        }
        let temp = join(
            &parent(target),
            &format!(".mycmd-{}.part", uuid::Uuid::new_v4()),
        )?;
        let mut output = sftp
            .open_mode(
                Path::new(&temp),
                OpenFlags::WRITE | OpenFlags::CREATE | OpenFlags::EXCLUSIVE,
                0o600,
                OpenType::File,
            )
            .map_err(error)?;
        let result = (|| {
            let mut buffer = vec![0; 1024 * 1024];
            let mut bytes = 0u64;
            loop {
                self.check()?;
                let n = input
                    .read(&mut buffer)
                    .map_err(|e| FsError::io(e, source))?;
                if n == 0 {
                    break;
                }
                bytes = bytes
                    .checked_add(n as u64)
                    .ok_or_else(|| source_changed(source))?;
                if bytes > snapshot.metadata.len() {
                    return Err(source_changed(source));
                }
                output
                    .write_all(&buffer[..n])
                    .map_err(|e| FsError::new("sftp_error", e.to_string()))?;
                self.tick(source, n as u64, 0)?;
            }
            let modified = snapshot
                .metadata
                .modified()
                .ok()
                .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|time| time.as_secs());
            #[cfg(unix)]
            let permissions = {
                use std::os::unix::fs::PermissionsExt;
                Some(snapshot.metadata.permissions().mode() & 0o777)
            };
            #[cfg(not(unix))]
            let permissions = None;
            output
                .setstat(FileStat {
                    size: None,
                    uid: None,
                    gid: None,
                    perm: permissions,
                    atime: modified,
                    mtime: modified,
                })
                .map_err(error)?;
            output.close().map_err(error)?;
            snapshot.verify(&input, source)?;
            self.check()?;
            commit_upload(sftp, &temp, target, overwrite)
        })();
        drop(output);
        if let Err(mut failure) = result {
            if let Err(cleanup) = sftp.unlink(Path::new(&temp)).map_err(error) {
                if cleanup.code != "not_found" {
                    failure
                        .message
                        .push_str(&format!(" Temporary upload may remain at {temp}."));
                }
            }
            return Err(failure);
        }
        Ok(())
    }

    fn download(
        &mut self,
        sftp: &Sftp,
        source: &str,
        target: &str,
        expected: &FileStat,
        overwrite: bool,
    ) -> Result<()> {
        let mut input = sftp.open(Path::new(source)).map_err(error)?;
        let before = input.stat().map_err(error)?;
        let expected_size = before.size.ok_or_else(|| {
            FsError::new("sftp_error", "Remote server did not provide a file size")
        })?;
        if !same_remote_file(&before, expected) {
            return Err(FsError::new(
                "source_changed",
                "Remote source changed before download",
            ));
        }
        let target = Path::new(target);
        let mut temp = tempfile::NamedTempFile::new_in(target.parent().unwrap())
            .map_err(|e| FsError::io(e, target))?;
        let mut buffer = vec![0; 1024 * 1024];
        let mut bytes = 0u64;
        loop {
            self.check()?;
            let n = input
                .read(&mut buffer)
                .map_err(|e| FsError::new("sftp_error", e.to_string()))?;
            if n == 0 {
                break;
            }
            bytes = bytes.checked_add(n as u64).ok_or_else(|| {
                FsError::new("size_overflow", "Remote source grew during download")
            })?;
            if bytes > expected_size {
                return Err(FsError::new(
                    "source_changed",
                    "Remote source grew during download",
                ));
            }
            temp.write_all(&buffer[..n])
                .map_err(|e| FsError::io(e, target))?;
            self.tick(Path::new(source), n as u64, 0)?;
        }
        let after = sftp.lstat(Path::new(source)).map_err(error)?;
        if !same_remote_file(&before, &after) || before.size != Some(bytes) {
            return Err(FsError::new(
                "source_changed",
                "Remote source changed during download; destination was not replaced",
            ));
        }
        input.close().map_err(error)?;
        #[cfg(unix)]
        if let Some(mode) = before.perm {
            use std::os::unix::fs::PermissionsExt;
            temp.as_file()
                .set_permissions(fs::Permissions::from_mode(mode & 0o777))
                .map_err(|e| FsError::io(e, target))?;
        }
        if let Some(seconds) = before.mtime {
            let time = std::time::UNIX_EPOCH
                .checked_add(Duration::from_secs(seconds))
                .ok_or_else(|| {
                    FsError::new("invalid_path", "Remote file timestamp is out of range")
                })?;
            temp.as_file()
                .set_times(fs::FileTimes::new().set_modified(time))
                .map_err(|e| FsError::io(e, target))?;
        }
        temp.as_file()
            .sync_all()
            .map_err(|e| FsError::io(e, target))?;
        self.check()?;
        if overwrite {
            temp.persist(target)
        } else {
            temp.persist_noclobber(target)
        }
        .map_err(|e| FsError::io(e.error, target))?;
        Ok(())
    }
}

fn same_remote_file(before: &FileStat, after: &FileStat) -> bool {
    // shortcut: SFTP v3 has no inode identity; use server snapshots for stronger consistency.
    before.is_file() && after.is_file() && before.size == after.size && before.mtime == after.mtime
}

fn commit_upload(sftp: &Sftp, temp: &str, target: &str, overwrite: bool) -> Result<()> {
    if !overwrite {
        return sftp
            .rename(
                Path::new(temp),
                Path::new(target),
                Some(RenameFlags::empty()),
            )
            .map_err(error);
    }
    if let Some(existing) = stat(sftp, target)? {
        if !existing.is_file() {
            return Err(FsError::new(
                "already_exists",
                "Cannot replace a remote directory or symbolic link",
            ));
        }
        // shortcut: SFTP v3 uses a recoverable backup; use POSIX rename when the library exposes it.
        let backup = join(
            &parent(target),
            &format!(".mycmd-{}.backup", uuid::Uuid::new_v4()),
        )?;
        if let Err(e) = sftp.rename(
            Path::new(target),
            Path::new(&backup),
            Some(RenameFlags::empty()),
        ) {
            return Err(FsError::new("sftp_error", format!("{e}. If the connection dropped after the rename, the original destination may be at {backup}; inspect it after reconnecting.")));
        }
        if let Err(e) = sftp.rename(
            Path::new(temp),
            Path::new(target),
            Some(RenameFlags::empty()),
        ) {
            let mut failure = error(e);
            if sftp
                .rename(
                    Path::new(&backup),
                    Path::new(target),
                    Some(RenameFlags::empty()),
                )
                .is_err()
            {
                failure.message.push_str(&format!(" Original destination is preserved at {backup}; restore it after reconnecting."));
            }
            return Err(failure);
        }
        if let Err(e) = sftp.unlink(Path::new(&backup)) {
            return Err(FsError::new(
                "sftp_cleanup",
                format!(
                    "Upload completed, but the previous destination may remain at {backup}: {e}"
                ),
            ));
        }
        Ok(())
    } else {
        sftp.rename(
            Path::new(temp),
            Path::new(target),
            Some(RenameFlags::empty()),
        )
        .map_err(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn remote_selection_is_normalized_without_allowing_root_or_traversal() {
        assert_eq!(
            remote_sources(vec![
                "/a/b".into(),
                "/a".into(),
                "/a".into(),
                "/another".into()
            ])
            .unwrap(),
            vec!["/a", "/another"]
        );
        for sources in [vec![], vec!["/".into()], vec!["/safe/../escape".into()]] {
            assert!(remote_sources(sources).is_err());
        }
    }

    #[cfg(unix)]
    #[test]
    #[ignore = "starts a private local OpenSSH server; requires /usr/sbin/sshd and ssh-keygen"]
    fn sftp_live_round_trip_and_failure_safety() {
        use super::super::tests::worker;
        use std::os::unix::fs::{symlink, PermissionsExt};
        let fixture = crate::sftp::tests::Fixture::new();
        let manager = crate::sftp::Manager::default();
        fixture.connect(&manager);
        let connection = manager.get(&fixture.profile.id).unwrap();
        let connection = connection.lock().unwrap();
        let sftp = &connection.sftp;
        let source = fixture.dir.path().join("local");
        let destination = fixture.dir.path().join("remote");
        let download = fixture.dir.path().join("download");
        fs::create_dir_all(source.join("nested/empty")).unwrap();
        fs::create_dir(&destination).unwrap();
        fs::create_dir(&download).unwrap();
        fs::write(source.join("nested/日本語 # %.txt"), "payload").unwrap();
        fs::set_permissions(
            source.join("nested/日本語 # %.txt"),
            fs::Permissions::from_mode(0o750),
        )
        .unwrap();
        let upload = SftpOperation::Copy {
            sources: vec![filesystem::text(&source)],
            destination: filesystem::text(&destination),
            upload: true,
        };
        let mut w = worker("overwrite");
        w.run_sftp(sftp, upload.clone()).unwrap();
        assert_eq!(w.progress.processed_bytes, 7);
        assert_eq!(w.progress.total_items, w.progress.processed_items);
        assert!(destination.join("local/nested/empty").is_dir());
        assert_eq!(
            fs::read_to_string(destination.join("local/nested/日本語 # %.txt")).unwrap(),
            "payload"
        );
        fs::write(source.join("nested/日本語 # %.txt"), "updated").unwrap();
        worker("overwrite").run_sftp(sftp, upload.clone()).unwrap();
        assert_eq!(
            fs::read_to_string(destination.join("local/nested/日本語 # %.txt")).unwrap(),
            "updated"
        );
        let transfer = SftpOperation::Copy {
            sources: vec![filesystem::text(&destination.join("local"))],
            destination: filesystem::text(&download),
            upload: false,
        };
        let mut w = worker("overwrite");
        w.run_sftp(sftp, transfer.clone()).unwrap();
        assert_eq!(w.progress.processed_bytes, 7);
        assert!(download.join("local/nested/empty").is_dir());
        assert_eq!(
            fs::metadata(download.join("local/nested/日本語 # %.txt"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o750
        );
        for policy in ["skip", "cancel", "rename", "overwrite"] {
            let result = worker(policy).run_sftp(sftp, transfer.clone());
            if policy == "cancel" {
                assert_eq!(result.unwrap_err().code, "cancelled");
            } else {
                result.unwrap();
            }
        }
        let mut cancelled = worker("overwrite");
        cancelled.control.cancel.store(true, Ordering::Relaxed);
        assert_eq!(
            cancelled.run_sftp(sftp, upload.clone()).unwrap_err().code,
            "cancelled"
        );
        symlink("nested", source.join("link")).unwrap();
        assert_eq!(
            worker("overwrite").run_sftp(sftp, upload).unwrap_err().code,
            "invalid_path"
        );
        assert!(destination.join("local/nested/日本語 # %.txt").exists());
        assert!(!destination.join("local/link").exists());
        let large = fixture.dir.path().join("large.bin");
        fs::File::create(&large)
            .unwrap()
            .set_len(32 * 1024 * 1024)
            .unwrap();
        fs::write(destination.join("large.bin"), "original destination").unwrap();
        let mut w = worker("overwrite");
        let control = w.control.clone();
        std::thread::scope(|scope| {
            let destination = &destination;
            let cancelled = scope.spawn(move || {
                for _ in 0..2000 {
                    if fs::read_dir(destination)
                        .unwrap()
                        .flatten()
                        .any(|entry| entry.file_name().to_string_lossy().ends_with(".part"))
                    {
                        control.cancel.store(true, Ordering::Relaxed);
                        return true;
                    }
                    std::thread::sleep(Duration::from_millis(1));
                }
                false
            });
            let result = w.run_sftp(
                sftp,
                SftpOperation::Copy {
                    sources: vec![filesystem::text(&large)],
                    destination: filesystem::text(destination),
                    upload: true,
                },
            );
            assert!(
                cancelled.join().unwrap(),
                "Transfer never created its temporary upload"
            );
            assert_eq!(result.unwrap_err().code, "cancelled");
        });
        assert_eq!(
            fs::read_to_string(destination.join("large.bin")).unwrap(),
            "original destination"
        );
        assert!(!fs::read_dir(&destination)
            .unwrap()
            .flatten()
            .any(|entry| entry.file_name().to_string_lossy().ends_with(".part")));
        worker("overwrite")
            .run_sftp(
                sftp,
                SftpOperation::Delete {
                    sources: vec![filesystem::text(&destination.join("large.bin"))],
                },
            )
            .unwrap();
        let created = destination.join("new folder");
        worker("overwrite")
            .run_sftp(
                sftp,
                SftpOperation::CreateDirectory {
                    parent: filesystem::text(&destination),
                    name: "new folder".into(),
                },
            )
            .unwrap();
        worker("overwrite")
            .run_sftp(
                sftp,
                SftpOperation::Rename {
                    path: filesystem::text(&created),
                    name: "renamed".into(),
                },
            )
            .unwrap();
        worker("overwrite")
            .run_sftp(
                sftp,
                SftpOperation::Delete {
                    sources: vec![
                        filesystem::text(&destination.join("local")),
                        filesystem::text(&destination.join("renamed")),
                    ],
                },
            )
            .unwrap();
        assert_eq!(fs::read_dir(&destination).unwrap().count(), 0);
        assert!(source.join("nested/日本語 # %.txt").exists());
        assert_eq!(
            fs::read_to_string(download.join("local/nested/日本語 # %.txt")).unwrap(),
            "updated"
        );
        fs::write(destination.join("large.bin"), "keep after disconnect").unwrap();
        let socket = connection.socket_for_test();
        std::thread::scope(|scope| {
            let destination = &destination;
            let interrupted = scope.spawn(move || {
                for _ in 0..2000 {
                    if fs::read_dir(destination)
                        .unwrap()
                        .flatten()
                        .any(|entry| entry.file_name().to_string_lossy().ends_with(".part"))
                    {
                        // The connection outlives this scope; shut down, but never close or reuse its descriptor.
                        assert_eq!(unsafe { libc::shutdown(socket, libc::SHUT_RDWR) }, 0);
                        return true;
                    }
                    std::thread::sleep(Duration::from_millis(1));
                }
                false
            });
            let failure = worker("overwrite")
                .run_sftp(
                    sftp,
                    SftpOperation::Copy {
                        sources: vec![filesystem::text(&large)],
                        destination: filesystem::text(destination),
                        upload: true,
                    },
                )
                .unwrap_err();
            assert!(interrupted.join().unwrap());
            assert!(
                failure.message.contains("Temporary upload may remain"),
                "{}",
                failure.message
            );
        });
        assert_eq!(
            fs::read_to_string(destination.join("large.bin")).unwrap(),
            "keep after disconnect"
        );
        assert_eq!(fs::metadata(&large).unwrap().len(), 32 * 1024 * 1024);
    }
}

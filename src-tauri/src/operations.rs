use crate::{
    error::{FsError, Result},
    filesystem::{self, Entry},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc, Mutex,
    },
    time::{Duration, Instant},
};
use tauri::{AppHandle, Emitter};

#[derive(Clone, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Operation {
    #[serde(rename_all = "camelCase")]
    Compress {
        sources: Vec<PathBuf>,
        destination: PathBuf,
        name: String,
        #[serde(default = "crate::config::default_compression_level")]
        compression_level: u8,
    },
    #[serde(rename_all = "camelCase")]
    DeleteArchive {
        archive_path: PathBuf,
        directory: String,
        members: Vec<String>,
    },
    #[serde(rename_all = "camelCase")]
    Extract {
        archive_path: PathBuf,
        directory: String,
        members: Vec<String>,
        destination: PathBuf,
    },
    Copy {
        sources: Vec<PathBuf>,
        destination: PathBuf,
    },
    Move {
        sources: Vec<PathBuf>,
        destination: PathBuf,
    },
    Delete {
        sources: Vec<PathBuf>,
        #[serde(default)]
        permanent: bool,
    },
    Rename {
        path: PathBuf,
        name: String,
    },
    CreateDirectory {
        parent: PathBuf,
        name: String,
    },
    CreateFile {
        parent: PathBuf,
        name: String,
    },
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub operation_id: String,
    pub state: String,
    pub current_item: Option<String>,
    pub processed_bytes: u64,
    pub total_bytes: u64,
    pub processed_items: u64,
    pub total_items: u64,
    pub error: Option<FsError>,
    pub result_path: Option<String>,
}
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Resolution {
    pub action: String,
    pub name: Option<String>,
    pub apply_to_all: bool,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Conflict {
    operation_id: String,
    source: Entry,
    destination: Entry,
}
struct Control {
    cancel: AtomicBool,
    answer: Mutex<Option<mpsc::Sender<Resolution>>>,
}
#[derive(Default)]
pub struct Manager {
    controls: Mutex<HashMap<String, Arc<Control>>>,
}
impl Manager {
    pub fn start(&self, app: AppHandle, operation: Operation) -> String {
        let id = uuid::Uuid::new_v4().to_string();
        let control = Arc::new(Control {
            cancel: AtomicBool::new(false),
            answer: Mutex::new(None),
        });
        self.controls
            .lock()
            .unwrap()
            .insert(id.clone(), control.clone());
        let operation_id = id.clone();
        tauri::async_runtime::spawn_blocking(move || {
            let mut worker = Worker {
                app: Some(app.clone()),
                control,
                policy: None,
                last_emit: Instant::now(),
                progress: Progress {
                    operation_id: operation_id.clone(),
                    state: "running".into(),
                    current_item: None,
                    processed_bytes: 0,
                    total_bytes: 0,
                    processed_items: 0,
                    total_items: 0,
                    error: None,
                    result_path: None,
                },
            };
            worker.emit();
            let result = worker.run(operation);
            worker.progress.state = match &result {
                Ok(_) => "completed",
                Err(e) if e.code == "cancelled" => "cancelled",
                Err(_) => "failed",
            }
            .into();
            worker.progress.error = result.err();
            worker.emit();
            use tauri::Manager as _;
            app.state::<Manager>()
                .controls
                .lock()
                .unwrap()
                .remove(&operation_id);
        });
        id
    }
    pub fn cancel(&self, id: &str) {
        if let Some(c) = self.controls.lock().unwrap().get(id) {
            c.cancel.store(true, Ordering::Relaxed);
        }
    }
    pub fn resolve(&self, id: &str, resolution: Resolution) -> Result<()> {
        if !["skip", "rename", "overwrite", "cancel"].contains(&resolution.action.as_str()) {
            return Err(FsError::new("invalid_path", "Unknown conflict action"));
        }
        if resolution.action == "rename" {
            if let Some(name) = &resolution.name {
                filesystem::validate_name(name)?;
            }
        }
        let controls = self.controls.lock().unwrap();
        let c = controls
            .get(id)
            .ok_or_else(|| FsError::new("not_found", "Operation has ended"))?;
        let sender = c
            .answer
            .lock()
            .unwrap()
            .take()
            .ok_or_else(|| FsError::new("not_found", "No pending conflict"))?;
        sender
            .send(resolution)
            .map_err(|_| FsError::new("cancelled", "Operation has ended"))
    }
}
enum TransferStep {
    Finished(Option<PathBuf>),
    Directory(Box<DirectoryTransfer>),
}
struct DirectoryTransfer {
    source: PathBuf,
    target: PathBuf,
    metadata: fs::Metadata,
    created: bool,
    children: fs::ReadDir,
    complete: bool,
}
struct Worker {
    app: Option<AppHandle>,
    control: Arc<Control>,
    progress: Progress,
    policy: Option<Resolution>,
    last_emit: Instant,
}
impl Worker {
    fn emit(&mut self) {
        if let Some(app) = &self.app {
            let _ = app.emit("operation-progress", &self.progress);
        }
        self.last_emit = Instant::now();
    }
    fn check(&self) -> Result<()> {
        if self.control.cancel.load(Ordering::Relaxed) {
            Err(FsError::new("cancelled", "Operation cancelled"))
        } else {
            Ok(())
        }
    }
    fn tick(&mut self, p: &Path, bytes: u64, items: u64) -> Result<()> {
        self.record_progress(p, bytes, items);
        self.check()
    }
    // Accounting after a committed mutation must not turn it into a failure.
    fn record_progress(&mut self, p: &Path, bytes: u64, items: u64) {
        self.progress.current_item = Some(filesystem::text(p));
        self.progress.processed_bytes += bytes;
        self.progress.processed_items += items;
        if self.last_emit.elapsed() > Duration::from_millis(80) {
            self.emit();
        }
    }
    fn measure(&self, p: &Path) -> Result<(u64, u64)> {
        let mut total = (0u64, 0u64);
        let mut directories: Vec<(PathBuf, fs::ReadDir)> = Vec::new();
        let mut next = Some(p.to_owned());
        while let Some(path) = next.take() {
            self.check()?;
            let metadata = fs::symlink_metadata(&path).map_err(|e| FsError::io(e, &path))?;
            total.0 = total
                .0
                .checked_add(if metadata.is_file() {
                    metadata.len()
                } else {
                    0
                })
                .ok_or_else(|| {
                    FsError::new("size_overflow", "Total file size exceeds supported range")
                })?;
            total.1 = total.1.checked_add(1).ok_or_else(|| {
                FsError::new("size_overflow", "Item count exceeds supported range")
            })?;
            if metadata.is_dir() {
                let entries = fs::read_dir(&path).map_err(|e| FsError::io(e, &path))?;
                directories.push((path, entries));
            }
            // Retain only the active branch, not every sibling's allocated path.
            while let Some((directory, entries)) = directories.last_mut() {
                self.check()?;
                if let Some(entry) = entries.next() {
                    next = Some(entry.map_err(|e| FsError::io(e, directory))?.path());
                    break;
                }
                directories.pop();
            }
        }
        Ok(total)
    }
    fn ask(&mut self, source: &Path, destination: &Path) -> Result<Resolution> {
        if let Some(p) = &self.policy {
            return Ok(p.clone());
        }
        let conflict = Conflict {
            operation_id: self.progress.operation_id.clone(),
            source: filesystem::entry(source)?,
            destination: filesystem::entry(destination)?,
        };
        let (tx, rx) = mpsc::channel();
        *self.control.answer.lock().unwrap() = Some(tx);
        self.app
            .as_ref()
            .ok_or_else(|| FsError::new("io_error", "Conflict requires an event receiver"))?
            .emit("operation-conflict", conflict)
            .map_err(|e| FsError::new("io_error", e.to_string()))?;
        loop {
            self.check()?;
            match rx.recv_timeout(Duration::from_millis(100)) {
                Ok(answer) => {
                    if answer.apply_to_all && answer.action != "cancel" {
                        let mut policy = answer.clone();
                        policy.name = None;
                        self.policy = Some(policy);
                    }
                    return Ok(answer);
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(_) => return Err(FsError::new("cancelled", "Conflict closed")),
            }
        }
    }
    fn transfer(
        &mut self,
        source: &Path,
        destination: &Path,
        moving: bool,
    ) -> Result<Option<PathBuf>> {
        let mut stack = Vec::new();
        match self.transfer_item(source, destination, moving)? {
            TransferStep::Finished(target) => return Ok(target),
            TransferStep::Directory(directory) => stack.push(directory),
        }
        loop {
            self.check()?;
            let directory = stack.last_mut().unwrap();
            if let Some(child) = directory.children.next() {
                let child = child.map_err(|e| FsError::io(e, &directory.source))?;
                let target = directory.target.join(child.file_name());
                match self.transfer_item(&child.path(), &target, moving)? {
                    TransferStep::Finished(target) => {
                        stack.last_mut().unwrap().complete &= target.is_some()
                    }
                    TransferStep::Directory(child) => stack.push(child),
                }
            } else {
                let directory = stack.pop().unwrap();
                drop(directory.children);
                if directory.created {
                    preserve_directory_metadata(
                        &directory.source,
                        &directory.target,
                        &directory.metadata,
                    )
                    .map_err(|e| FsError::io(e, &directory.target))?;
                }
                if moving && directory.complete {
                    fs::remove_dir(&directory.source)
                        .map_err(|e| FsError::io(e, &directory.source))?;
                }
                self.record_progress(&directory.source, 0, 1);
                if let Some(parent) = stack.last_mut() {
                    parent.complete &= directory.complete;
                } else {
                    return Ok(directory.complete.then_some(directory.target));
                }
            }
        }
    }
    fn transfer_item(
        &mut self,
        source: &Path,
        destination: &Path,
        moving: bool,
    ) -> Result<TransferStep> {
        self.check()?;
        let meta = fs::symlink_metadata(source).map_err(|e| FsError::io(e, source))?;
        let mut target = destination.to_owned();
        let mut overwrite = false;
        while fs::symlink_metadata(&target).is_ok() {
            if same_file::is_same_file(source, &target).unwrap_or(false) {
                // Windows resolves case-only names to the same entry, but
                // MoveFileExW without replacement can still update its casing.
                // Canonical paths distinguish separate hard-link names.
                #[cfg(windows)]
                if moving
                    && source.parent() == target.parent()
                    && source.file_name() != target.file_name()
                    && fs::canonicalize(source).map_err(|e| FsError::io(e, source))?
                        == fs::canonicalize(&target).map_err(|e| FsError::io(e, &target))?
                {
                    break;
                }
                return Err(FsError::new(
                    "invalid_path",
                    "Source and destination are the same item",
                ));
            }
            if meta.is_dir()
                && !fs::symlink_metadata(&target)
                    .map_err(|e| FsError::io(e, &target))?
                    .is_symlink()
            {
                let source_path = filesystem::absolute(source)?;
                let target_path = filesystem::absolute(&target)?;
                if source_path.starts_with(&target_path) || target_path.starts_with(&source_path) {
                    return Err(FsError::new(
                        "invalid_path",
                        "Source and destination directories overlap",
                    ));
                }
            }
            let answer = self.ask(source, &target)?;
            match answer.action.as_str() {
                "skip" => {
                    let n = self.measure(source)?;
                    self.tick(source, n.0, n.1)?;
                    return Ok(TransferStep::Finished(None));
                }
                "cancel" => return Err(FsError::new("cancelled", "Operation cancelled")),
                "rename" => {
                    if let Some(name) = answer.name {
                        filesystem::validate_name(&name)?;
                        target = destination.with_file_name(name);
                    } else {
                        let name = destination.file_name().unwrap().to_string_lossy();
                        let mut n = 1;
                        loop {
                            target = destination.with_file_name(format!("{name} ({n})"));
                            if fs::symlink_metadata(&target).is_err() {
                                break;
                            }
                            n += 1;
                        }
                    }
                }
                "overwrite" => {
                    overwrite = true;
                    break;
                }
                _ => unreachable!(),
            }
        }
        let target_meta = fs::symlink_metadata(&target).ok();
        if overwrite
            && (meta.is_dir() != target_meta.as_ref().is_some_and(|m| m.is_dir())
                || target_meta.as_ref().is_some_and(|m| m.is_symlink()))
        {
            return Err(FsError::new(
                "already_exists",
                "Cannot overwrite a different item type or a symbolic link; use Rename or Skip",
            ));
        }
        if moving && !overwrite {
            // Measure before committing, while errors/cancellation can still
            // leave the source untouched.
            let n = self.measure(source)?;
            self.check()?;
            match rename_noreplace(source, &target) {
                Ok(()) => {
                    self.record_progress(&target, n.0, n.1);
                    return Ok(TransferStep::Finished(Some(target)));
                }
                Err(e) if e.kind() == std::io::ErrorKind::CrossesDevices => {}
                Err(e) => return Err(FsError::io(e, &target)),
            }
        }
        if meta.is_dir() {
            if !overwrite {
                fs::create_dir(&target).map_err(|e| FsError::io(e, &target))?;
            }
            return Ok(TransferStep::Directory(Box::new(DirectoryTransfer {
                source: source.to_owned(),
                target,
                metadata: meta,
                created: !overwrite,
                children: fs::read_dir(source).map_err(|e| FsError::io(e, source))?,
                complete: true,
            })));
        }
        if meta.is_symlink() {
            if overwrite {
                return Err(FsError::new(
                    "already_exists",
                    "Use Rename or Skip to preserve an existing link",
                ));
            }
            let link = fs::read_link(source).map_err(|e| FsError::io(e, source))?;
            #[cfg(unix)]
            std::os::unix::fs::symlink(&link, &target).map_err(|e| FsError::io(e, &target))?;
            #[cfg(windows)]
            {
                use std::os::windows::fs::{symlink_dir, symlink_file, FileTypeExt};
                if meta.file_type().is_symlink_dir() {
                    symlink_dir(&link, &target)
                } else {
                    symlink_file(&link, &target)
                }
                .map_err(|e| FsError::io(e, &target))?;
            }
            if moving {
                let current = fs::symlink_metadata(source).map_err(|_| source_changed(source))?;
                if !current.is_symlink()
                    || !unchanged_metadata(&meta, &current)
                    || fs::read_link(source).map_err(|_| source_changed(source))? != link
                {
                    return Err(source_changed(source));
                }
            }
        } else if meta.is_file() {
            let mut input = fs::File::open(source).map_err(|e| FsError::io(e, source))?;
            let snapshot = SourceSnapshot::new(&input, source)?;
            if !unchanged_metadata(&meta, &snapshot.metadata) {
                return Err(source_changed(source));
            }
            let mut temp = tempfile::NamedTempFile::new_in(target.parent().unwrap())
                .map_err(|e| FsError::io(e, &target))?;
            let mut buffer = vec![0; 1024 * 1024];
            loop {
                self.check()?;
                let n = input
                    .read(&mut buffer)
                    .map_err(|e| FsError::io(e, source))?;
                if n == 0 {
                    break;
                }
                temp.write_all(&buffer[..n])
                    .map_err(|e| FsError::io(e, &target))?;
                self.tick(source, n as u64, 0)?;
            }
            preserve_file_metadata(&input, temp.as_file(), &snapshot.metadata)
                .map_err(|e| FsError::io(e, &target))?;
            temp.as_file()
                .sync_all()
                .map_err(|e| FsError::io(e, &target))?;
            self.check()?;
            snapshot.verify(&input, source)?;
            if overwrite {
                temp.persist(&target)
            } else {
                temp.persist_noclobber(&target)
            }
            .map_err(|e| FsError::io(e.error, &target))?;
            if moving {
                // Retain the source if it changed after destination commit.
                snapshot.verify(&input, source)?;
                remove_leaf(source)?;
            }
        } else {
            return Err(FsError::new(
                "invalid_path",
                "Special filesystem objects are not supported",
            ));
        }
        if moving && meta.is_symlink() {
            remove_leaf(source)?;
        }
        self.record_progress(source, 0, 1);
        Ok(TransferStep::Finished(Some(target)))
    }
    fn delete(&mut self, p: &Path) -> Result<()> {
        let mut directories: Vec<(PathBuf, fs::ReadDir)> = Vec::new();
        let mut next = Some(p.to_owned());
        while let Some(path) = next.take() {
            self.check()?;
            let metadata = fs::symlink_metadata(&path).map_err(|e| FsError::io(e, &path))?;
            if metadata.is_dir() {
                let entries = fs::read_dir(&path).map_err(|e| FsError::io(e, &path))?;
                directories.push((path, entries));
            } else {
                remove_leaf(&path)?;
                self.record_progress(
                    &path,
                    if metadata.is_file() {
                        metadata.len()
                    } else {
                        0
                    },
                    1,
                );
            }
            while let Some((directory, entries)) = directories.last_mut() {
                self.check()?;
                if let Some(entry) = entries.next() {
                    next = Some(entry.map_err(|e| FsError::io(e, directory))?.path());
                    break;
                }
                let (directory, entries) = directories.pop().unwrap();
                // Close the iterator before removing its directory (Windows).
                drop(entries);
                self.check()?;
                fs::remove_dir(&directory).map_err(|e| FsError::io(e, &directory))?;
                self.record_progress(&directory, 0, 1);
            }
        }
        Ok(())
    }
    fn run(&mut self, op: Operation) -> Result<()> {
        match op {
            Operation::Compress {
                sources,
                destination,
                name,
                compression_level,
            } => {
                self.compress(sources, destination, name, compression_level)?;
            }
            Operation::DeleteArchive {
                archive_path,
                directory,
                members,
            } => {
                crate::archives::delete_selected(&archive_path, &directory, &members, || {
                    self.check()
                })?;
            }
            Operation::Extract {
                archive_path,
                directory,
                members,
                destination,
            } => {
                let staging =
                    tempfile::tempdir().map_err(|e| FsError::new("io_error", e.to_string()))?;
                let sources = crate::archives::extract_selected(
                    &archive_path,
                    &directory,
                    &members,
                    staging.path(),
                    || self.check(),
                )?;
                self.batch(sources, destination, false)?;
            }
            Operation::CreateFile { parent, name } => {
                filesystem::validate_name(&name)?;
                let path = filesystem::absolute(&parent)?.join(name);
                self.check()?;
                fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&path)
                    .map_err(|error| FsError::io(error, &path))?;
                self.progress.result_path = Some(filesystem::text(&path));
            }
            Operation::CreateDirectory { parent, name } => {
                filesystem::validate_name(&name)?;
                let p = filesystem::absolute(&parent)?.join(name);
                self.check()?;
                fs::create_dir(&p).map_err(|e| FsError::io(e, &p))?;
                self.progress.result_path = Some(filesystem::text(&p));
            }
            Operation::Rename { path, name } => {
                filesystem::validate_name(&name)?;
                let target = path.with_file_name(name);
                protect(&path)?;
                if target == path {
                    self.progress.result_path = Some(filesystem::text(&path));
                    return Ok(());
                }
                let (bytes, items) = self.measure(&path)?;
                self.progress.total_bytes = bytes;
                self.progress.total_items = items;
                self.emit();
                self.progress.result_path = self
                    .transfer(&path, &target, true)?
                    .map(|target| filesystem::text(&target));
            }
            Operation::Delete { sources, permanent } => {
                let sources = normalize_sources(sources)?;
                for p in &sources {
                    protect(p)?;
                    let n = self.measure(p)?;
                    self.progress.total_bytes += n.0;
                    self.progress.total_items += n.1;
                }
                self.emit();
                if permanent {
                    for p in sources {
                        self.delete(&p)?;
                    }
                } else {
                    for p in sources {
                        self.check()?;
                        trash::delete(&p).map_err(|e| {
                            FsError::new("trash_error", format!("{}: {e}", p.display()))
                        })?;
                        self.tick(&p, 0, 1)?;
                    }
                }
            }
            Operation::Copy {
                sources,
                destination,
            } => self.transfer_sources(sources, destination, false)?,
            Operation::Move {
                sources,
                destination,
            } => self.transfer_sources(sources, destination, true)?,
        }
        Ok(())
    }
    fn compress(
        &mut self,
        sources: Vec<PathBuf>,
        destination: PathBuf,
        name: String,
        compression_level: u8,
    ) -> Result<()> {
        if !matches!(compression_level, 1 | 6 | 9) {
            return Err(FsError::new(
                "invalid_path",
                "Compression level must be 1, 6, or 9",
            ));
        }
        let name = zip_name(&name)?;
        let destination = filesystem::absolute(&destination)?;
        if !destination.is_dir() {
            return Err(FsError::new(
                "invalid_path",
                "Destination must be a directory",
            ));
        }
        let mut regular_sources = Vec::new();
        for source in normalize_sources(sources)? {
            self.check()?;
            if !fs::symlink_metadata(&source)
                .map_err(|e| FsError::io(e, &source))?
                .is_symlink()
            {
                regular_sources.push(source);
            }
        }
        let sources = regular_sources;
        if sources.is_empty() {
            return Ok(());
        }
        let mut names = std::collections::HashSet::new();
        for source in &sources {
            if !names.insert(zip_member_name(source)?) {
                return Err(FsError::new(
                    "invalid_path",
                    "Selected items have duplicate names",
                ));
            }
            if source.is_dir() && destination.starts_with(filesystem::absolute(source)?) {
                return Err(FsError::new(
                    "invalid_path",
                    "Cannot create an archive inside a selected folder",
                ));
            }
            let (bytes, items) = self.measure(source)?;
            self.progress.total_bytes += bytes;
            self.progress.total_items += items;
        }
        self.emit();
        let mut target = destination.join(&name);
        let mut temp = tempfile::NamedTempFile::new_in(&destination)
            .map_err(|e| FsError::io(e, &destination))?;
        self.write_zip(&sources, temp.as_file_mut(), compression_level)?;
        temp.as_file()
            .sync_all()
            .map_err(|e| FsError::io(e, &target))?;
        let mut overwrite = false;
        loop {
            self.check()?;
            if sources
                .iter()
                .any(|source| same_file::is_same_file(source, &target).unwrap_or(false))
            {
                return Err(FsError::new(
                    "invalid_path",
                    "Archive cannot replace a source item",
                ));
            }
            match fs::symlink_metadata(&target) {
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => break,
                Err(e) => return Err(FsError::io(e, &target)),
                Ok(_) => {}
            }
            let answer = self.ask(temp.path(), &target)?;
            match answer.action.as_str() {
                "skip" => return Ok(()),
                "cancel" => return Err(FsError::new("cancelled", "Operation cancelled")),
                "rename" => {
                    if let Some(name) = answer.name {
                        target = destination.join(zip_name(&name)?);
                    } else {
                        let mut n = 1;
                        loop {
                            self.check()?;
                            target =
                                destination.join(format!("{} ({n}).zip", &name[..name.len() - 4]));
                            match fs::symlink_metadata(&target) {
                                Err(e) if e.kind() == std::io::ErrorKind::NotFound => break,
                                Err(e) => return Err(FsError::io(e, &target)),
                                Ok(_) => {}
                            }
                            n += 1;
                        }
                    }
                }
                "overwrite" => {
                    let meta =
                        fs::symlink_metadata(&target).map_err(|e| FsError::io(e, &target))?;
                    if !meta.is_file() || meta.is_symlink() {
                        return Err(FsError::new(
                            "invalid_path",
                            "Cannot replace a directory or symbolic link with a ZIP",
                        ));
                    }
                    overwrite = true;
                    break;
                }
                _ => return Err(FsError::new("invalid_path", "Unknown conflict action")),
            }
        }
        self.check()?;
        if overwrite {
            temp.persist(&target)
        } else {
            temp.persist_noclobber(&target)
        }
        .map_err(|e| FsError::io(e.error, &target))?;
        self.progress.result_path = Some(filesystem::text(&target));
        Ok(())
    }
    fn write_zip(
        &mut self,
        sources: &[PathBuf],
        output: &mut fs::File,
        compression_level: u8,
    ) -> Result<()> {
        let error = |e: zip::result::ZipError| FsError::new("archive_error", e.to_string());
        let mut writer = zip::ZipWriter::new(output);
        let mut buffer = vec![0; 1024 * 1024];
        for source in sources {
            let mut directories: Vec<(PathBuf, String, fs::ReadDir)> = Vec::new();
            let mut next = Some((source.clone(), String::new()));
            while let Some((path, prefix)) = next.take() {
                self.check()?;
                let meta = fs::symlink_metadata(&path).map_err(|e| FsError::io(e, &path))?;
                if !meta.is_symlink() {
                    let name = format!("{prefix}{}", zip_member_name(&path)?);
                    let options = zip::write::SimpleFileOptions::default()
                        .compression_method(zip::CompressionMethod::Deflated)
                        .compression_level(Some(i64::from(compression_level)))
                        .large_file(meta.len() > u32::MAX as u64);
                    if meta.is_dir() {
                        writer
                            .add_directory(format!("{name}/"), options)
                            .map_err(error)?;
                        directories.push((
                            path.clone(),
                            name,
                            fs::read_dir(&path).map_err(|e| FsError::io(e, &path))?,
                        ));
                    } else if meta.is_file() {
                        let mut input = fs::File::open(&path).map_err(|e| FsError::io(e, &path))?;
                        let snapshot = SourceSnapshot::new(&input, &path)?;
                        if !unchanged_metadata(&meta, &snapshot.metadata) {
                            return Err(source_changed(&path));
                        }
                        writer.start_file(name, options).map_err(error)?;
                        loop {
                            self.check()?;
                            let size =
                                input.read(&mut buffer).map_err(|e| FsError::io(e, &path))?;
                            if size == 0 {
                                break;
                            }
                            writer
                                .write_all(&buffer[..size])
                                .map_err(|e| FsError::new("archive_error", e.to_string()))?;
                            self.tick(&path, size as u64, 0)?;
                        }
                        snapshot.verify(&input, &path)?;
                    } else {
                        return Err(FsError::new(
                            "invalid_path",
                            "ZIP compression does not support special filesystem objects",
                        ));
                    }
                }
                self.tick(&path, 0, 1)?;
                while let Some((directory, prefix, entries)) = directories.last_mut() {
                    self.check()?;
                    if let Some(entry) = entries.next() {
                        let path = entry.map_err(|e| FsError::io(e, directory))?.path();
                        next = Some((path, format!("{prefix}/")));
                        break;
                    }
                    directories.pop();
                }
            }
        }
        self.check()?;
        writer.finish().map_err(error)?;
        Ok(())
    }
    fn transfer_sources(
        &mut self,
        sources: Vec<PathBuf>,
        destination: PathBuf,
        moving: bool,
    ) -> Result<()> {
        let destination = transfer_destination(&sources, destination)?;
        if destination.is_dir() || destination == Path::new("~") {
            return self.batch(sources, destination, moving);
        }
        if sources.len() != 1 {
            return Err(FsError::new(
                "invalid_path",
                "Destination must be a directory when copying or moving multiple items",
            ));
        }
        let sources = normalize_sources(sources)?;
        let source = &sources[0];
        if fs::symlink_metadata(source)
            .map_err(|e| FsError::io(e, source))?
            .is_dir()
        {
            return Err(FsError::new(
                "invalid_path",
                "Destination must be a directory",
            ));
        }
        protect(&destination)?;
        let parent = destination.parent().unwrap();
        let parent = filesystem::absolute(if parent.as_os_str().is_empty() {
            Path::new(".")
        } else {
            parent
        })?;
        let target = parent.join(destination.file_name().unwrap());
        let (bytes, items) = self.measure(source)?;
        self.progress.total_bytes += bytes;
        self.progress.total_items += items;
        self.emit();
        self.transfer(source, &target, moving)?;
        Ok(())
    }
    fn batch(&mut self, sources: Vec<PathBuf>, destination: PathBuf, moving: bool) -> Result<()> {
        let destination = filesystem::absolute(&destination)?;
        if !destination.is_dir() {
            return Err(FsError::new(
                "invalid_path",
                "Destination must be a directory",
            ));
        }
        let sources = normalize_sources(sources)?;
        for p in &sources {
            protect(p)?;
            let m = fs::symlink_metadata(p).map_err(|e| FsError::io(e, p))?;
            if m.is_dir() && destination.starts_with(filesystem::absolute(p)?) {
                return Err(FsError::new(
                    "invalid_path",
                    "Cannot copy or move a directory into itself",
                ));
            }
            let n = self.measure(p)?;
            self.progress.total_bytes += n.0;
            self.progress.total_items += n.1;
        }
        self.emit();
        for p in sources {
            self.transfer(&p, &destination.join(p.file_name().unwrap()), moving)?;
        }
        Ok(())
    }
}
fn zip_name(name: &str) -> Result<String> {
    filesystem::validate_name(name)?;
    Ok(if name.to_lowercase().ends_with(".zip") {
        name.to_owned()
    } else {
        format!("{name}.zip")
    })
}

fn zip_member_name(path: &Path) -> Result<String> {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| FsError::new("invalid_path", "ZIP member names must be valid UTF-8"))?;
    filesystem::validate_name(name)?;
    if name.contains([':', '\0']) {
        return Err(FsError::new("invalid_path", "Unsafe ZIP member name"));
    }
    Ok(name.to_owned())
}
/// Atomic rename without replacing an entry created after conflict checking.
fn rename_noreplace(source: &Path, destination: &Path) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        let source = std::ffi::CString::new(source.as_os_str().as_bytes())
            .map_err(|_| std::io::Error::from(std::io::ErrorKind::InvalidInput))?;
        let destination = std::ffi::CString::new(destination.as_os_str().as_bytes())
            .map_err(|_| std::io::Error::from(std::io::ErrorKind::InvalidInput))?;
        // Both C strings remain valid for the duration of the syscall.
        #[cfg(target_os = "macos")]
        let result =
            unsafe { libc::renamex_np(source.as_ptr(), destination.as_ptr(), libc::RENAME_EXCL) };
        #[cfg(target_os = "linux")]
        let result = unsafe {
            libc::renameat2(
                libc::AT_FDCWD,
                source.as_ptr(),
                libc::AT_FDCWD,
                destination.as_ptr(),
                libc::RENAME_NOREPLACE,
            )
        };
        #[cfg(not(any(target_os = "macos", target_os = "linux")))]
        return Err(std::io::Error::from(std::io::ErrorKind::Unsupported));
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        if result == 0 {
            Ok(())
        } else {
            Err(std::io::Error::last_os_error())
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        #[link(name = "kernel32")]
        extern "system" {
            fn MoveFileExW(source: *const u16, destination: *const u16, flags: u32) -> i32;
        }
        let source: Vec<u16> = source.as_os_str().encode_wide().chain(Some(0)).collect();
        let destination: Vec<u16> = destination
            .as_os_str()
            .encode_wide()
            .chain(Some(0))
            .collect();
        if source[..source.len() - 1].contains(&0)
            || destination[..destination.len() - 1].contains(&0)
        {
            return Err(std::io::Error::from(std::io::ErrorKind::InvalidInput));
        }
        // No REPLACE_EXISTING flag; buffers are valid, NUL-terminated strings.
        if unsafe { MoveFileExW(source.as_ptr(), destination.as_ptr(), 0) } != 0 {
            Ok(())
        } else {
            Err(std::io::Error::last_os_error())
        }
    }
}

struct SourceSnapshot {
    identity: same_file::Handle,
    metadata: fs::Metadata,
}

impl SourceSnapshot {
    fn new(file: &fs::File, source: &Path) -> Result<Self> {
        Ok(Self {
            identity: same_file::Handle::from_file(
                file.try_clone().map_err(|e| FsError::io(e, source))?,
            )
            .map_err(|e| FsError::io(e, source))?,
            metadata: file.metadata().map_err(|e| FsError::io(e, source))?,
        })
    }

    fn verify(&self, file: &fs::File, source: &Path) -> Result<()> {
        let current = fs::symlink_metadata(source).map_err(|_| source_changed(source))?;
        let opened = file.metadata().map_err(|_| source_changed(source))?;
        let identity = same_file::Handle::from_path(source).map_err(|_| source_changed(source))?;
        if !current.is_file()
            || identity != self.identity
            || !unchanged_metadata(&self.metadata, &current)
            || !unchanged_metadata(&self.metadata, &opened)
        {
            return Err(source_changed(source));
        }
        Ok(())
    }
}

fn source_changed(source: &Path) -> FsError {
    let mut error = FsError::new(
        "source_changed",
        "Source changed during transfer; original was not removed",
    );
    error.path = Some(filesystem::text(source));
    error
}

fn unchanged_metadata(before: &fs::Metadata, after: &fs::Metadata) -> bool {
    let unchanged = before.len() == after.len() && before.modified().ok() == after.modified().ok();
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        unchanged
            && before.dev() == after.dev()
            && before.ino() == after.ino()
            && before.ctime() == after.ctime()
            && before.ctime_nsec() == after.ctime_nsec()
    }
    #[cfg(not(unix))]
    {
        unchanged
    }
}

fn preserve_file_metadata(
    source: &fs::File,
    destination: &fs::File,
    metadata: &fs::Metadata,
) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        use xattr::FileExt;
        for name in source.list_xattr()? {
            if let Some(value) = source.get_xattr(&name)? {
                destination.set_xattr(&name, &value)?;
            }
        }
    }
    #[cfg(not(unix))]
    let _ = source;
    destination.set_permissions(metadata.permissions())?;
    destination.set_times(
        fs::FileTimes::new()
            .set_accessed(metadata.accessed()?)
            .set_modified(metadata.modified()?),
    )
}

fn preserve_directory_metadata(
    source: &Path,
    destination: &Path,
    metadata: &fs::Metadata,
) -> std::io::Result<()> {
    #[cfg(unix)]
    for name in xattr::list(source)? {
        if let Some(value) = xattr::get(source, &name)? {
            xattr::set(destination, &name, &value)?;
        }
    }
    #[cfg(not(unix))]
    let _ = source;
    filetime::set_file_times(
        destination,
        filetime::FileTime::from_last_access_time(metadata),
        filetime::FileTime::from_last_modification_time(metadata),
    )?;
    fs::set_permissions(destination, metadata.permissions())
}

fn transfer_destination(sources: &[PathBuf], destination: PathBuf) -> Result<PathBuf> {
    if let [source] = sources {
        if destination != Path::new("~")
            && destination.parent() == Some(Path::new(""))
            && destination.file_name().is_some()
        {
            protect(source)?;
            return Ok(filesystem::absolute(source.parent().unwrap())?.join(destination));
        }
    }
    Ok(destination)
}

fn protect(p: &Path) -> Result<()> {
    if p.file_name().is_none() || p.parent().is_none() {
        return Err(FsError::new(
            "invalid_path",
            "Filesystem roots cannot be modified",
        ));
    }
    Ok(())
}
fn normalize_sources(sources: Vec<PathBuf>) -> Result<Vec<PathBuf>> {
    if sources.is_empty() {
        return Err(FsError::new("invalid_path", "No source items"));
    }
    let mut paths = Vec::new();
    for p in sources {
        protect(&p)?;
        let parent = filesystem::absolute(p.parent().unwrap())?;
        paths.push(parent.join(p.file_name().unwrap()));
    }
    paths.sort();
    paths.dedup();
    let mut result: Vec<PathBuf> = Vec::new();
    for p in paths {
        if !result.iter().any(|parent| p.starts_with(parent)) {
            result.push(p);
        }
    }
    Ok(result)
}
fn remove_leaf(p: &Path) -> Result<()> {
    #[cfg(windows)]
    {
        use std::os::windows::fs::FileTypeExt;
        if fs::symlink_metadata(p)
            .map_err(|e| FsError::io(e, p))?
            .file_type()
            .is_symlink_dir()
        {
            return fs::remove_dir(p).map_err(|e| FsError::io(e, p));
        }
    }
    fs::remove_file(p).map_err(|e| FsError::io(e, p))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn compress_uses_each_level_and_preserves_contents() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("file.txt");
        let contents: String = (0..10_000)
            .map(|n| {
                format!(
                    "Line {}: a repeated sentence to compress with ZIP.\n",
                    n % 1000
                )
            })
            .collect();
        fs::write(&source, &contents).unwrap();
        let mut sizes = Vec::new();
        for compression_level in [1, 6, 9] {
            let name = format!("level-{compression_level}.zip");
            worker("overwrite")
                .run(Operation::Compress {
                    sources: vec![source.clone()],
                    destination: dir.path().to_owned(),
                    name: name.clone(),
                    compression_level,
                })
                .unwrap();
            let mut archive =
                zip::ZipArchive::new(fs::File::open(dir.path().join(name)).unwrap()).unwrap();
            let mut entry = archive.by_name("file.txt").unwrap();
            sizes.push(entry.compressed_size());
            let mut actual = String::new();
            entry.read_to_string(&mut actual).unwrap();
            assert_eq!(actual, contents);
        }
        assert!(sizes[0] > sizes[2]);
    }

    #[test]
    fn compress_defaults_legacy_requests_and_rejects_invalid_levels_before_writing() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("file.txt");
        fs::write(&source, "payload").unwrap();
        let operation: Operation = serde_json::from_value(serde_json::json!({"type": "compress", "sources": [source], "destination": dir.path(), "name": "Backup.zip"})).unwrap();
        assert!(matches!(
            operation,
            Operation::Compress {
                compression_level: 6,
                ..
            }
        ));
        for level in [0, 2, 10] {
            let operation: Operation = serde_json::from_value(serde_json::json!({"type": "compress", "sources": [source], "destination": dir.path(), "name": "Backup.zip", "compressionLevel": level})).unwrap();
            assert_eq!(
                worker("overwrite").run(operation).unwrap_err().code,
                "invalid_path"
            );
        }
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
    }
    #[test]
    fn compress_preserves_folder_structure_empty_directories_and_file_contents() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("My folder");
        let destination = dir.path().join("out");
        fs::create_dir_all(source.join("nested/empty")).unwrap();
        fs::create_dir(&destination).unwrap();
        fs::write(source.join("nested/hello.txt"), "hello").unwrap();
        let loose = dir.path().join("loose.txt");
        fs::write(&loose, "world").unwrap();
        let mut w = worker("overwrite");
        w.run(Operation::Compress {
            sources: vec![source.clone(), source.join("nested"), loose],
            destination: destination.clone(),
            name: "Bundle".into(),
            compression_level: 6,
        })
        .unwrap();
        let output = destination.join("Bundle.zip");
        let mut zip = zip::ZipArchive::new(fs::File::open(&output).unwrap()).unwrap();
        assert_eq!(zip.len(), 5);
        assert!(zip.by_name("My folder/nested/empty/").unwrap().is_dir());
        let mut contents = String::new();
        zip.by_name("My folder/nested/hello.txt")
            .unwrap()
            .read_to_string(&mut contents)
            .unwrap();
        assert_eq!(contents, "hello");
        assert_eq!(
            zip.by_name("loose.txt").unwrap().compression(),
            zip::CompressionMethod::Deflated
        );
        assert_eq!(w.progress.processed_bytes, 10);
        assert_eq!(w.progress.total_bytes, 10);
        assert_eq!(w.progress.processed_items, 5);
        assert_eq!(w.progress.total_items, 5);
        assert_eq!(
            w.progress.result_path,
            Some(filesystem::text(&filesystem::absolute(&output).unwrap()))
        );
        assert_eq!(fs::read_dir(&destination).unwrap().count(), 1);
        assert_eq!(
            fs::read_to_string(source.join("nested/hello.txt")).unwrap(),
            "hello"
        );
    }

    #[test]
    fn compress_conflicts_skip_rename_overwrite_and_cancel_without_partial_files() {
        for action in ["skip", "rename", "overwrite", "cancel"] {
            let dir = tempfile::tempdir().unwrap();
            let source = dir.path().join("source.txt");
            let destination = dir.path().join("out");
            fs::create_dir(&destination).unwrap();
            fs::write(&source, "payload").unwrap();
            let target = destination.join("Backup.zip");
            fs::write(&target, "keep").unwrap();
            let mut w = worker(action);
            let result = w.run(Operation::Compress {
                sources: vec![source.clone()],
                destination: destination.clone(),
                name: "Backup.zip".into(),
                compression_level: 6,
            });
            if action == "cancel" {
                assert_eq!(result.unwrap_err().code, "cancelled");
            } else {
                result.unwrap();
            }
            if action != "overwrite" {
                assert_eq!(fs::read_to_string(&target).unwrap(), "keep");
            }
            if action == "overwrite" || action == "rename" {
                let archive = if action == "rename" {
                    destination.join("Backup (1).zip")
                } else {
                    target
                };
                let mut zip = zip::ZipArchive::new(fs::File::open(archive).unwrap()).unwrap();
                let mut contents = String::new();
                zip.by_name("source.txt")
                    .unwrap()
                    .read_to_string(&mut contents)
                    .unwrap();
                assert_eq!(contents, "payload");
            }
            assert_eq!(fs::read_to_string(source).unwrap(), "payload");
            assert_eq!(
                fs::read_dir(destination).unwrap().count(),
                if action == "rename" { 2 } else { 1 }
            );
        }
    }

    #[test]
    fn compress_rejects_unsafe_names_empty_sources_and_duplicate_member_names() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join("a")).unwrap();
        fs::create_dir(dir.path().join("b")).unwrap();
        let a = dir.path().join("a/same.txt");
        let b = dir.path().join("b/same.txt");
        fs::write(&a, "a").unwrap();
        fs::write(&b, "b").unwrap();
        for (sources, name) in [
            (vec![a.clone()], "../escape"),
            (vec![a.clone()], ""),
            (vec![], "Empty.zip"),
            (vec![a, b], "Duplicate.zip"),
        ] {
            assert_eq!(
                worker("overwrite")
                    .run(Operation::Compress {
                        sources,
                        destination: dir.path().to_owned(),
                        name: name.into(),
                        compression_level: 6
                    })
                    .unwrap_err()
                    .code,
                "invalid_path"
            );
        }
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 2);
    }

    #[test]
    fn compress_cannot_replace_a_source_or_write_inside_a_selected_folder() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source.zip");
        fs::write(&source, "keep").unwrap();
        assert_eq!(
            worker("overwrite")
                .run(Operation::Compress {
                    sources: vec![source.clone()],
                    destination: dir.path().to_owned(),
                    name: "source.zip".into(),
                    compression_level: 6
                })
                .unwrap_err()
                .code,
            "invalid_path"
        );
        assert_eq!(fs::read_to_string(&source).unwrap(), "keep");
        let folder = dir.path().join("folder");
        fs::create_dir_all(folder.join("nested")).unwrap();
        assert_eq!(
            worker("overwrite")
                .run(Operation::Compress {
                    sources: vec![folder.clone()],
                    destination: folder.join("nested"),
                    name: "Backup.zip".into(),
                    compression_level: 6,
                })
                .unwrap_err()
                .code,
            "invalid_path"
        );
        assert_eq!(fs::read_dir(folder.join("nested")).unwrap().count(), 0);
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 2);
    }

    #[test]
    fn compress_cancellation_before_start_leaves_no_archive() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source");
        fs::write(&source, "keep").unwrap();
        let mut w = worker("overwrite");
        w.control.cancel.store(true, Ordering::Relaxed);
        assert_eq!(
            w.run(Operation::Compress {
                sources: vec![source],
                destination: dir.path().to_owned(),
                name: "Backup.zip".into(),
                compression_level: 6,
            })
            .unwrap_err()
            .code,
            "cancelled"
        );
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[test]
    fn compress_conflict_rename_keeps_zip_extension_and_cannot_replace_source() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source.zip");
        fs::write(&source, "payload").unwrap();
        fs::write(dir.path().join("Backup.zip"), "keep").unwrap();
        let operation = Operation::Compress {
            sources: vec![source.clone()],
            destination: dir.path().to_owned(),
            name: "Backup.zip".into(),
            compression_level: 6,
        };
        let mut w = worker("rename");
        w.policy.as_mut().unwrap().name = Some("Renamed".into());
        w.run(operation.clone()).unwrap();
        assert!(
            zip::ZipArchive::new(fs::File::open(dir.path().join("Renamed.zip")).unwrap()).is_ok()
        );
        let mut w = worker("rename");
        w.policy.as_mut().unwrap().name = Some("source.zip".into());
        assert_eq!(w.run(operation).unwrap_err().code, "invalid_path");
        assert_eq!(fs::read_to_string(source).unwrap(), "payload");
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 3);
    }

    #[test]
    fn compress_cannot_overwrite_an_existing_directory() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source");
        fs::write(&source, "payload").unwrap();
        let target = dir.path().join("Backup.zip");
        fs::create_dir(&target).unwrap();
        assert_eq!(
            worker("overwrite")
                .run(Operation::Compress {
                    sources: vec![source],
                    destination: dir.path().to_owned(),
                    name: "Backup.zip".into(),
                    compression_level: 6,
                })
                .unwrap_err()
                .code,
            "invalid_path"
        );
        assert!(target.is_dir());
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 2);
    }

    #[cfg(unix)]
    #[test]
    fn compress_skips_selected_symlinks_and_creates_no_archive_for_links_only() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("file.txt");
        fs::write(&source, "payload").unwrap();
        let link = dir.path().join("link");
        let broken = dir.path().join("broken");
        std::os::unix::fs::symlink(&source, &link).unwrap();
        std::os::unix::fs::symlink("missing", &broken).unwrap();
        let mut w = worker("overwrite");
        w.run(Operation::Compress {
            sources: vec![source, link.clone(), broken.clone()],
            destination: dir.path().to_owned(),
            name: "Mixed.zip".into(),
            compression_level: 6,
        })
        .unwrap();
        let mut zip =
            zip::ZipArchive::new(fs::File::open(dir.path().join("Mixed.zip")).unwrap()).unwrap();
        assert_eq!(zip.len(), 1);
        assert!(zip.by_name("file.txt").is_ok());
        assert_eq!(w.progress.total_items, 1);
        assert_eq!(w.progress.processed_items, 1);
        let mut w = worker("overwrite");
        w.run(Operation::Compress {
            sources: vec![link, broken],
            destination: dir.path().to_owned(),
            name: "Links.zip".into(),
            compression_level: 6,
        })
        .unwrap();
        assert!(!dir.path().join("Links.zip").exists());
        assert!(w.progress.result_path.is_none());
    }

    #[cfg(unix)]
    #[test]
    fn compress_skips_nested_symlinks_but_rejects_destination_symlinks() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("folder");
        fs::create_dir(&source).unwrap();
        fs::write(source.join("file"), "payload").unwrap();
        std::os::unix::fs::symlink(".", source.join("loop")).unwrap();
        std::os::unix::fs::symlink("missing", source.join("broken:link")).unwrap();
        std::os::unix::fs::symlink("file", source.join("file-link")).unwrap();
        let operation = Operation::Compress {
            sources: vec![source.clone()],
            destination: dir.path().to_owned(),
            name: "Backup.zip".into(),
            compression_level: 6,
        };
        let mut w = worker("overwrite");
        w.run(operation.clone()).unwrap();
        {
            let mut zip =
                zip::ZipArchive::new(fs::File::open(dir.path().join("Backup.zip")).unwrap())
                    .unwrap();
            assert_eq!(zip.len(), 2);
            assert!(zip.by_name("folder/file").is_ok());
        }
        assert_eq!(w.progress.processed_items, w.progress.total_items);
        assert_eq!(w.progress.processed_bytes, 7);
        fs::remove_file(dir.path().join("Backup.zip")).unwrap();
        let untouched = dir.path().join("untouched");
        fs::write(&untouched, "keep").unwrap();
        std::os::unix::fs::symlink(&untouched, dir.path().join("Backup.zip")).unwrap();
        assert_eq!(
            worker("overwrite").run(operation).unwrap_err().code,
            "invalid_path"
        );
        assert_eq!(fs::read_to_string(untouched).unwrap(), "keep");
        assert!(fs::symlink_metadata(dir.path().join("Backup.zip"))
            .unwrap()
            .is_symlink());
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 3);
    }

    #[cfg(unix)]
    #[test]
    fn compress_failure_after_writing_does_not_replace_existing_archive_or_leave_staging_files() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("a.txt");
        fs::write(&source, "payload").unwrap();
        let folder = dir.path().join("z-folder");
        fs::create_dir(&folder).unwrap();
        fs::write(folder.join("unsafe:name"), "not supported").unwrap();
        let destination = dir.path().join("out");
        fs::create_dir(&destination).unwrap();
        let target = destination.join("Backup.zip");
        fs::write(&target, "keep").unwrap();
        let mut w = worker("overwrite");
        assert_eq!(
            w.run(Operation::Compress {
                sources: vec![source, folder],
                destination: destination.clone(),
                name: "Backup.zip".into(),
                compression_level: 6,
            })
            .unwrap_err()
            .code,
            "invalid_path"
        );
        assert_eq!(w.progress.processed_bytes, 7);
        assert_eq!(fs::read_to_string(target).unwrap(), "keep");
        assert_eq!(fs::read_dir(destination).unwrap().count(), 1);
    }

    #[cfg(unix)]
    #[test]
    fn compress_auto_rename_does_not_loop_on_dangling_symlinks() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source");
        fs::write(&source, "payload").unwrap();
        fs::write(dir.path().join("Backup.ZIP"), "keep").unwrap();
        std::os::unix::fs::symlink("missing", dir.path().join("Backup (1).zip")).unwrap();
        let mut w = worker("rename");
        w.run(Operation::Compress {
            sources: vec![source],
            destination: dir.path().to_owned(),
            name: "Backup.ZIP".into(),
            compression_level: 6,
        })
        .unwrap();
        assert_eq!(
            w.progress.result_path,
            Some(filesystem::text(
                &filesystem::absolute(&dir.path().join("Backup (2).zip")).unwrap()
            ))
        );
    }
    #[test]
    fn create_file_is_empty_and_never_overwrites_existing_contents() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("new file & notes.txt");
        let operation = Operation::CreateFile {
            parent: directory.path().to_owned(),
            name: "new file & notes.txt".into(),
        };
        let mut w = worker("overwrite");
        w.run(operation.clone()).unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"");
        assert_eq!(
            w.progress.result_path,
            Some(filesystem::text(&filesystem::absolute(&path).unwrap()))
        );
        fs::write(&path, "keep").unwrap();
        assert_eq!(w.run(operation).unwrap_err().code, "already_exists");
        assert_eq!(fs::read_to_string(&path).unwrap(), "keep");
    }

    #[test]
    fn create_file_rejects_names_that_escape_the_current_directory() {
        let directory = tempfile::tempdir().unwrap();
        for name in ["", ".", "..", "../outside", "nested/file", "nested\\file"] {
            assert!(worker("overwrite")
                .run(Operation::CreateFile {
                    parent: directory.path().to_owned(),
                    name: name.into(),
                })
                .is_err());
        }
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 0);
    }

    #[test]
    fn copy_single_file_to_new_name_in_same_directory() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source.txt");
        let destination = dir.path().join("copy.txt");
        fs::write(&source, "payload").unwrap();
        let mut w = worker("overwrite");
        w.run(Operation::Copy {
            sources: vec![source.clone()],
            destination: destination.clone(),
        })
        .unwrap();
        assert_eq!(fs::read_to_string(source).unwrap(), "payload");
        assert_eq!(fs::read_to_string(destination).unwrap(), "payload");
        assert_eq!(w.progress.total_bytes, 7);
        assert_eq!(w.progress.processed_bytes, 7);
        assert_eq!(w.progress.processed_items, 1);
    }

    #[test]
    fn bare_transfer_filename_uses_source_directory_not_process_working_directory() {
        let dir = tempfile::tempdir().unwrap();
        let canonical = filesystem::absolute(dir.path()).unwrap();
        let source = dir.path().join("pkmain01.sh");
        let target = canonical.join("pkmain02.sh");
        for moving in [false, true] {
            fs::write(&source, "payload").unwrap();
            let destination =
                transfer_destination(std::slice::from_ref(&source), PathBuf::from("pkmain02.sh"))
                    .unwrap();
            assert_eq!(destination, target);
            let mut w = worker("overwrite");
            w.transfer_sources(vec![source.clone()], destination, moving)
                .unwrap();
            assert_eq!(source.exists(), !moving);
            assert_eq!(fs::read_to_string(&target).unwrap(), "payload");
        }
        assert_eq!(
            transfer_destination(std::slice::from_ref(&source), PathBuf::from("src")).unwrap(),
            canonical.join("src")
        );
        #[cfg(unix)]
        {
            let other = dir.path().join("other");
            fs::create_dir(&other).unwrap();
            fs::write(other.join("original.sh"), "target").unwrap();
            let link = dir.path().join("link.sh");
            std::os::unix::fs::symlink(other.join("original.sh"), &link).unwrap();
            assert_eq!(
                transfer_destination(&[link], PathBuf::from("renamed.sh")).unwrap(),
                canonical.join("renamed.sh")
            );
        }
        for destination in [
            dir.path().join("elsewhere.txt"),
            PathBuf::from("~"),
            PathBuf::from("."),
            PathBuf::from("nested/name.txt"),
        ] {
            assert_eq!(
                transfer_destination(std::slice::from_ref(&source), destination.clone()).unwrap(),
                destination
            );
        }
        assert_eq!(
            transfer_destination(&[source.clone(), source], PathBuf::from("out")).unwrap(),
            PathBuf::from("out")
        );
    }

    #[test]
    fn copy_single_file_to_existing_filename_uses_conflict_policy() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source.txt");
        let destination = dir.path().join("copy.txt");
        fs::write(&source, "payload").unwrap();
        fs::write(&destination, "keep").unwrap();
        worker("skip")
            .transfer_sources(vec![source.clone()], destination.clone(), false)
            .unwrap();
        assert_eq!(fs::read_to_string(&destination).unwrap(), "keep");
        worker("overwrite")
            .transfer_sources(vec![source.clone()], destination.clone(), false)
            .unwrap();
        assert_eq!(fs::read_to_string(source).unwrap(), "payload");
        assert_eq!(fs::read_to_string(destination).unwrap(), "payload");
    }

    #[test]
    fn copy_multiple_files_to_filename_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let first = dir.path().join("first.txt");
        let second = dir.path().join("second.txt");
        let destination = dir.path().join("copy.txt");
        fs::write(&first, "first").unwrap();
        fs::write(&second, "second").unwrap();
        let error = worker("overwrite")
            .transfer_sources(
                vec![first.clone(), second.clone()],
                destination.clone(),
                false,
            )
            .unwrap_err();
        assert_eq!(error.code, "invalid_path");
        assert!(error.message.contains("multiple items"));
        assert!(!destination.exists());
        assert_eq!(fs::read_to_string(first).unwrap(), "first");
        assert_eq!(fs::read_to_string(second).unwrap(), "second");
    }

    #[test]
    fn copy_multiple_files_to_directory_still_works() {
        let dir = tempfile::tempdir().unwrap();
        let first = dir.path().join("first.txt");
        let second = dir.path().join("second.txt");
        let destination = dir.path().join("out");
        fs::write(&first, "first").unwrap();
        fs::write(&second, "second").unwrap();
        fs::create_dir(&destination).unwrap();
        worker("overwrite")
            .transfer_sources(vec![first, second], destination.clone(), false)
            .unwrap();
        assert_eq!(
            fs::read_to_string(destination.join("first.txt")).unwrap(),
            "first"
        );
        assert_eq!(
            fs::read_to_string(destination.join("second.txt")).unwrap(),
            "second"
        );
    }

    #[test]
    fn copy_to_same_filename_is_rejected_without_changing_source() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source.txt");
        fs::write(&source, "payload").unwrap();
        let error = worker("overwrite")
            .transfer_sources(vec![source.clone()], source.clone(), false)
            .unwrap_err();
        assert_eq!(error.code, "invalid_path");
        assert_eq!(fs::read_to_string(source).unwrap(), "payload");
    }

    #[test]
    fn move_single_file_to_new_name_in_same_directory() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source.txt");
        let destination = dir.path().join("renamed.txt");
        fs::write(&source, "payload").unwrap();
        let mut w = worker("overwrite");
        w.run(Operation::Move {
            sources: vec![source.clone()],
            destination: destination.clone(),
        })
        .unwrap();
        assert!(!source.exists());
        assert_eq!(fs::read_to_string(destination).unwrap(), "payload");
        assert_eq!(w.progress.total_bytes, 7);
        assert_eq!(w.progress.processed_bytes, 7);
        assert_eq!(w.progress.processed_items, 1);
    }

    #[cfg(windows)]
    #[test]
    fn rename_changes_only_the_case_of_files_and_directories() {
        for directory in [false, true] {
            let dir = tempfile::tempdir().unwrap();
            let source = dir.path().join("pgdev");
            let target = dir.path().join("pgDEV");
            if directory {
                fs::create_dir(&source).unwrap();
                fs::write(source.join("file"), "payload").unwrap();
            } else {
                fs::write(&source, "payload").unwrap();
            }
            let mut w = worker("cancel");
            w.run(Operation::Rename {
                path: source,
                name: "pgDEV".into(),
            })
            .unwrap();
            let names: Vec<_> = fs::read_dir(dir.path())
                .unwrap()
                .map(|entry| entry.unwrap().file_name())
                .collect();
            assert_eq!(names, vec![std::ffi::OsString::from("pgDEV")]);
            let payload = if directory {
                target.join("file")
            } else {
                target.clone()
            };
            assert_eq!(fs::read_to_string(payload).unwrap(), "payload");
            assert_eq!(w.progress.result_path, Some(filesystem::text(&target)));
        }
    }

    #[test]
    fn transfers_preserve_distinct_hard_links() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("original");
        let target = dir.path().join("other-link");
        fs::write(&source, "payload").unwrap();
        fs::hard_link(&source, &target).unwrap();
        for moving in [false, true] {
            assert_eq!(
                worker("overwrite")
                    .transfer(&source, &target, moving)
                    .err()
                    .unwrap()
                    .code,
                "invalid_path"
            );
            assert_eq!(fs::read_to_string(&source).unwrap(), "payload");
            assert_eq!(fs::read_to_string(&target).unwrap(), "payload");
        }
    }

    #[cfg(windows)]
    #[test]
    fn case_only_copy_is_rejected_but_move_updates_the_name() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("pgdev");
        let target = dir.path().join("pgDEV");
        fs::write(&source, "payload").unwrap();
        assert_eq!(
            worker("overwrite")
                .transfer(&source, &target, false)
                .err()
                .unwrap()
                .code,
            "invalid_path"
        );
        let mut w = worker("cancel");
        assert_eq!(
            w.transfer(&source, &target, true).unwrap(),
            Some(target.clone())
        );
        assert_eq!(fs::read_to_string(&target).unwrap(), "payload");
        assert_eq!(
            fs::read_dir(dir.path())
                .unwrap()
                .next()
                .unwrap()
                .unwrap()
                .file_name(),
            "pgDEV"
        );
    }

    #[test]
    fn rename_uses_move_conflict_choices_and_reports_the_actual_result_path() {
        for action in ["skip", "rename", "overwrite", "cancel"] {
            let dir = tempfile::tempdir().unwrap();
            let source = dir.path().join("source.txt");
            let target = dir.path().join("renamed.txt");
            fs::write(&source, "source").unwrap();
            fs::write(&target, "existing").unwrap();
            let mut w = worker(action);
            let result = w.run(Operation::Rename {
                path: source.clone(),
                name: "renamed.txt".into(),
            });
            if action == "cancel" {
                assert_eq!(result.unwrap_err().code, "cancelled");
            } else {
                result.unwrap();
            }
            if action == "rename" || action == "overwrite" {
                let actual = if action == "rename" {
                    dir.path().join("renamed.txt (1)")
                } else {
                    target.clone()
                };
                assert!(!source.exists());
                assert_eq!(fs::read_to_string(&actual).unwrap(), "source");
                assert_eq!(w.progress.result_path, Some(filesystem::text(&actual)));
            } else {
                assert_eq!(fs::read_to_string(&source).unwrap(), "source");
                assert!(w.progress.result_path.is_none());
            }
            if action != "overwrite" {
                assert_eq!(fs::read_to_string(&target).unwrap(), "existing");
            }
        }
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("folder");
        fs::create_dir(&source).unwrap();
        fs::write(source.join("file"), "payload").unwrap();
        let mut w = worker("overwrite");
        w.run(Operation::Rename {
            path: source.clone(),
            name: "folder".into(),
        })
        .unwrap();
        assert_eq!(w.progress.result_path, Some(filesystem::text(&source)));
        w.run(Operation::Rename {
            path: source.clone(),
            name: "new-folder".into(),
        })
        .unwrap();
        assert!(!source.exists());
        let target = dir.path().join("new-folder");
        assert_eq!(fs::read_to_string(target.join("file")).unwrap(), "payload");
        assert_eq!(w.progress.result_path, Some(filesystem::text(&target)));
    }

    #[test]
    fn move_to_existing_filename_only_removes_source_after_overwrite() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source.txt");
        let destination = dir.path().join("renamed.txt");
        fs::write(&source, "payload").unwrap();
        fs::write(&destination, "keep").unwrap();
        worker("skip")
            .transfer_sources(vec![source.clone()], destination.clone(), true)
            .unwrap();
        assert_eq!(fs::read_to_string(&source).unwrap(), "payload");
        assert_eq!(fs::read_to_string(&destination).unwrap(), "keep");
        worker("overwrite")
            .transfer_sources(vec![source.clone()], destination.clone(), true)
            .unwrap();
        assert!(!source.exists());
        assert_eq!(fs::read_to_string(destination).unwrap(), "payload");
    }

    #[test]
    fn move_multiple_files_to_filename_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let first = dir.path().join("first.txt");
        let second = dir.path().join("second.txt");
        let destination = dir.path().join("renamed.txt");
        fs::write(&first, "first").unwrap();
        fs::write(&second, "second").unwrap();
        let error = worker("overwrite")
            .transfer_sources(
                vec![first.clone(), second.clone()],
                destination.clone(),
                true,
            )
            .unwrap_err();
        assert_eq!(error.code, "invalid_path");
        assert!(error.message.contains("multiple items"));
        assert!(!destination.exists());
        assert_eq!(fs::read_to_string(first).unwrap(), "first");
        assert_eq!(fs::read_to_string(second).unwrap(), "second");
    }

    #[test]
    fn move_to_same_filename_is_rejected_without_changing_source() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source.txt");
        fs::write(&source, "payload").unwrap();
        let error = worker("overwrite")
            .transfer_sources(vec![source.clone()], source.clone(), true)
            .unwrap_err();
        assert_eq!(error.code, "invalid_path");
        assert_eq!(fs::read_to_string(source).unwrap(), "payload");
    }

    #[test]
    fn snapshot_rejects_modified_source() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("source");
        fs::write(&path, "old").unwrap();
        let file = fs::File::open(&path).unwrap();
        let snapshot = SourceSnapshot::new(&file, &path).unwrap();
        fs::write(&path, "new content").unwrap();
        assert_eq!(
            snapshot.verify(&file, &path).unwrap_err().code,
            "source_changed"
        );
        assert_eq!(fs::read_to_string(path).unwrap(), "new content");
    }

    #[test]
    fn committed_progress_does_not_fail_on_late_cancellation() {
        let mut w = worker("overwrite");
        w.control.cancel.store(true, Ordering::Relaxed);
        w.record_progress(Path::new("committed"), 10, 1);
        assert_eq!(w.progress.processed_bytes, 10);
        assert_eq!(w.progress.processed_items, 1);
        assert_eq!(w.check().unwrap_err().code, "cancelled");
    }

    #[test]
    fn recursive_copy_preserves_directory_modified_time() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source");
        let destination = dir.path().join("out");
        fs::create_dir(&source).unwrap();
        fs::create_dir(&destination).unwrap();
        fs::write(source.join("file"), "payload").unwrap();
        let time = filetime::FileTime::from_unix_time(1_000_000, 0);
        filetime::set_file_mtime(&source, time).unwrap();
        worker("overwrite")
            .transfer(&source, &destination.join("source"), false)
            .unwrap();
        assert_eq!(
            filetime::FileTime::from_last_modification_time(
                &fs::metadata(destination.join("source")).unwrap()
            ),
            time
        );
    }

    #[test]
    fn snapshot_rejects_replaced_source() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("source");
        let old = dir.path().join("old");
        fs::write(&path, "old").unwrap();
        let file = fs::File::open(&path).unwrap();
        let snapshot = SourceSnapshot::new(&file, &path).unwrap();
        fs::rename(&path, &old).unwrap();
        fs::write(&path, "new").unwrap();
        assert_eq!(
            snapshot.verify(&file, &path).unwrap_err().code,
            "source_changed"
        );
    }

    #[test]
    fn fallback_move_preserves_file_modified_time() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source");
        let destination = dir.path().join("destination");
        fs::write(&source, "payload").unwrap();
        fs::write(&destination, "old").unwrap();
        let time = std::time::UNIX_EPOCH + Duration::from_secs(1_000_000);
        fs::File::options()
            .write(true)
            .open(&source)
            .unwrap()
            .set_times(fs::FileTimes::new().set_modified(time))
            .unwrap();
        // Overwrite intentionally exercises copy-and-remove, even on one volume.
        worker("overwrite")
            .transfer(&source, &destination, true)
            .unwrap();
        assert!(!source.exists());
        assert_eq!(
            fs::metadata(&destination).unwrap().modified().unwrap(),
            time
        );
    }

    #[cfg(unix)]
    #[test]
    fn fallback_move_preserves_extended_attributes() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source");
        let destination = dir.path().join("destination");
        fs::write(&source, "payload").unwrap();
        fs::write(&destination, "old").unwrap();
        xattr::set(&source, "user.mycmd-test", b"metadata").unwrap();
        worker("overwrite")
            .transfer(&source, &destination, true)
            .unwrap();
        assert_eq!(
            xattr::get(&destination, "user.mycmd-test").unwrap(),
            Some(b"metadata".to_vec())
        );
    }
    #[test]
    fn atomic_rename_preserves_existing_destination() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source");
        let destination = dir.path().join("destination");
        fs::write(&source, "new").unwrap();
        fs::write(&destination, "keep").unwrap();
        assert!(rename_noreplace(&source, &destination).is_err());
        assert_eq!(fs::read_to_string(&destination).unwrap(), "keep");
        assert_eq!(fs::read_to_string(&source).unwrap(), "new");
    }
    #[test]
    fn atomic_rename_moves_to_a_missing_destination() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source");
        let destination = dir.path().join("destination");
        fs::write(&source, "payload").unwrap();
        rename_noreplace(&source, &destination).unwrap();
        assert!(!source.exists());
        assert_eq!(fs::read_to_string(&destination).unwrap(), "payload");
    }
    #[test]
    fn iterative_measure_and_delete_handle_nested_directories() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("tree");
        fs::create_dir(&root).unwrap();
        let mut path = root.clone();
        // Keep total path length within Windows limits as well.
        for _ in 0..40 {
            path = path.join("d");
            fs::create_dir(&path).unwrap();
            fs::write(path.join("f"), b"abc").unwrap();
        }
        let mut worker = worker("overwrite");
        assert_eq!(worker.measure(&root).unwrap(), (120, 81));
        worker.delete(&root).unwrap();
        assert!(!root.exists());
        assert_eq!(worker.progress.processed_bytes, 120);
        assert_eq!(worker.progress.processed_items, 81);
    }

    #[test]
    fn streaming_measure_counts_wide_trees_and_empty_directories() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("tree");
        fs::create_dir_all(root.join("empty/nested")).unwrap();
        for index in 0..256 {
            fs::write(root.join(format!("file-{index}")), b"abc").unwrap();
        }
        assert_eq!(worker("overwrite").measure(&root).unwrap(), (768, 259));
        assert_eq!(
            worker("overwrite").measure(&root.join("empty")).unwrap(),
            (0, 2)
        );
        assert_eq!(
            worker("overwrite").measure(&root.join("file-0")).unwrap(),
            (3, 1)
        );
        let mut deleter = worker("overwrite");
        deleter.delete(&root).unwrap();
        assert!(!root.exists());
        assert_eq!(deleter.progress.processed_bytes, 768);
        assert_eq!(deleter.progress.processed_items, 259);
    }

    #[test]
    fn iterative_transfer_copies_and_merges_a_nested_move() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("source");
        let target = temp.path().join("target");
        fs::create_dir(&source).unwrap();
        let mut leaf = source.clone();
        for _ in 0..40 {
            leaf = leaf.join("d");
            fs::create_dir(&leaf).unwrap();
        }
        fs::write(leaf.join("payload"), b"content").unwrap();
        let mut copier = worker("overwrite");
        assert!(copier.transfer(&source, &target, false).unwrap().is_some());
        let relative = leaf.strip_prefix(&source).unwrap();
        assert_eq!(
            fs::read(target.join(relative).join("payload")).unwrap(),
            b"content"
        );
        assert!(source.exists());
        assert_eq!(copier.progress.processed_items, 42);
        // Existing destination directories force the copy-and-remove merge path.
        let mut mover = worker("overwrite");
        assert!(mover.transfer(&source, &target, true).unwrap().is_some());
        assert!(!source.exists());
        assert_eq!(
            fs::read(target.join(relative).join("payload")).unwrap(),
            b"content"
        );
        assert_eq!(mover.progress.processed_items, 42);
    }

    #[cfg(unix)]
    #[test]
    #[ignore = "requires MYCMD_TEST_TRANSFER_VOLUME pointing to a different mounted filesystem"]
    fn cross_device_move_uses_real_copy_and_remove_fallback() {
        use std::os::unix::fs::MetadataExt;
        let volume = std::env::var_os("MYCMD_TEST_TRANSFER_VOLUME")
            .expect("Set MYCMD_TEST_TRANSFER_VOLUME to a writable mounted filesystem");
        let source_temp = tempfile::tempdir().unwrap();
        let destination_temp = tempfile::tempdir_in(volume).unwrap();
        assert_ne!(
            fs::metadata(source_temp.path()).unwrap().dev(),
            fs::metadata(destination_temp.path()).unwrap().dev(),
            "Test requires genuinely different filesystems"
        );
        let source = source_temp.path().join("tree");
        fs::create_dir_all(source.join("nested")).unwrap();
        fs::write(source.join("nested/file"), b"cross-device payload").unwrap();
        std::os::unix::fs::symlink("nested/file", source.join("link")).unwrap();
        let modified = filetime::FileTime::from_unix_time(1_600_000_000, 0);
        filetime::set_file_mtime(source.join("nested/file"), modified).unwrap();
        filetime::set_file_mtime(&source, modified).unwrap();
        let target = destination_temp.path().join("tree");
        let mut mover = worker("overwrite");
        assert!(mover.transfer(&source, &target, true).unwrap().is_some());
        assert!(!source.exists());
        assert_eq!(
            fs::read(target.join("nested/file")).unwrap(),
            b"cross-device payload"
        );
        assert_eq!(
            fs::read_link(target.join("link")).unwrap(),
            PathBuf::from("nested/file")
        );
        assert_eq!(
            filetime::FileTime::from_last_modification_time(
                &fs::metadata(target.join("nested/file")).unwrap()
            ),
            modified
        );
        assert_eq!(
            filetime::FileTime::from_last_modification_time(&fs::metadata(&target).unwrap()),
            modified
        );
        assert_eq!(mover.progress.processed_items, 4);
        assert_eq!(mover.progress.processed_bytes, 20);
    }

    #[cfg(unix)]
    #[test]
    fn iterative_delete_and_measure_do_not_follow_directory_symlinks() {
        let temp = tempfile::tempdir().unwrap();
        let outside = temp.path().join("outside");
        fs::create_dir(&outside).unwrap();
        fs::write(outside.join("keep"), b"safe").unwrap();
        let root = temp.path().join("tree");
        fs::create_dir(&root).unwrap();
        std::os::unix::fs::symlink(&outside, root.join("link")).unwrap();
        let mut worker = worker("overwrite");
        assert_eq!(worker.measure(&root).unwrap(), (0, 2));
        worker.delete(&root).unwrap();
        assert_eq!(fs::read(outside.join("keep")).unwrap(), b"safe");
    }

    #[test]
    fn iterative_walks_honor_cancellation_before_mutation() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("keep");
        fs::write(&path, b"safe").unwrap();
        let mut worker = worker("overwrite");
        worker.control.cancel.store(true, Ordering::Relaxed);
        assert_eq!(worker.measure(&path).unwrap_err().code, "cancelled");
        assert_eq!(worker.delete(&path).unwrap_err().code, "cancelled");
        assert_eq!(fs::read(&path).unwrap(), b"safe");
    }
    fn worker(policy: &str) -> Worker {
        Worker {
            app: None,
            control: Arc::new(Control {
                cancel: AtomicBool::new(false),
                answer: Mutex::new(None),
            }),
            policy: Some(Resolution {
                action: policy.into(),
                name: None,
                apply_to_all: true,
            }),
            last_emit: Instant::now(),
            progress: Progress {
                operation_id: "test".into(),
                state: "running".into(),
                current_item: None,
                processed_bytes: 0,
                total_bytes: 0,
                processed_items: 0,
                total_items: 0,
                error: None,
                result_path: None,
            },
        }
    }
    #[test]
    fn delete_defaults_to_recycle_bin() {
        let operation: Operation = serde_json::from_value(serde_json::json!({
            "type": "delete",
            "sources": ["C:\\temp\\item"],
        }))
        .unwrap();
        assert!(
            matches!(
                operation,
                Operation::Delete {
                    permanent: false,
                    ..
                }
            ),
            "missing flag must mean recycle bin, never permanent delete"
        );
        let operation: Operation = serde_json::from_value(serde_json::json!({
            "type": "delete",
            "sources": ["C:\\temp\\item"],
            "permanent": true,
        }))
        .unwrap();
        assert!(matches!(
            operation,
            Operation::Delete {
                permanent: true,
                ..
            }
        ));
    }
    #[test]
    fn recursive_copy_and_delete() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("source");
        let destination = temp.path().join("destination");
        fs::create_dir_all(source.join("nested")).unwrap();
        fs::create_dir(&destination).unwrap();
        fs::write(source.join("nested/data"), b"payload").unwrap();
        let mut w = worker("overwrite");
        w.run(Operation::Copy {
            sources: vec![source.clone(), source.join("nested")],
            destination: destination.clone(),
        })
        .unwrap();
        assert_eq!(
            fs::read(destination.join("source/nested/data")).unwrap(),
            b"payload"
        );
        assert_eq!(w.progress.processed_bytes, 7);
        assert_eq!(w.progress.processed_items, 3);
        w.run(Operation::Delete {
            sources: vec![destination.join("source")],
            permanent: true,
        })
        .unwrap();
        assert!(!destination.join("source").exists());
        assert!(source.join("nested/data").exists());
    }
    #[test]
    fn copy_into_self_is_rejected_before_writing() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("source");
        fs::create_dir_all(source.join("nested")).unwrap();
        let result = worker("overwrite").run(Operation::Copy {
            sources: vec![source.clone()],
            destination: source.join("nested"),
        });
        assert_eq!(result.unwrap_err().code, "invalid_path");
        assert!(!source.join("nested/source").exists());
    }
    #[test]
    fn conflicts_skip_rename_overwrite_and_cancel() {
        for action in ["skip", "rename", "overwrite", "cancel"] {
            let temp = tempfile::tempdir().unwrap();
            let source = temp.path().join("item");
            let destination = temp.path().join("out");
            fs::create_dir(&destination).unwrap();
            fs::write(&source, b"new").unwrap();
            fs::write(destination.join("item"), b"old").unwrap();
            let result = worker(action).run(Operation::Copy {
                sources: vec![source.clone()],
                destination: destination.clone(),
            });
            if action == "cancel" {
                assert_eq!(result.unwrap_err().code, "cancelled");
            } else {
                result.unwrap();
            }
            assert_eq!(
                fs::read(destination.join("item")).unwrap(),
                if action == "overwrite" {
                    b"new"
                } else {
                    b"old"
                }
            );
            if action == "rename" {
                assert_eq!(fs::read(destination.join("item (1)")).unwrap(), b"new");
            }
            assert_eq!(fs::read(&source).unwrap(), b"new");
        }
    }
    #[test]
    fn skipped_move_keeps_source_and_successful_move_removes_it() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("source");
        let destination = temp.path().join("destination");
        fs::create_dir_all(&source).unwrap();
        fs::create_dir_all(destination.join("source")).unwrap();
        fs::write(source.join("item"), b"new").unwrap();
        fs::write(destination.join("source/item"), b"old").unwrap();
        let mut w = worker("skip");
        w.run(Operation::Move {
            sources: vec![source.clone()],
            destination: destination.clone(),
        })
        .unwrap();
        assert!(source.join("item").exists());
        worker("overwrite")
            .run(Operation::Move {
                sources: vec![source.clone()],
                destination: destination.clone(),
            })
            .unwrap();
        assert!(!source.exists());
        assert_eq!(fs::read(destination.join("source/item")).unwrap(), b"new");
    }
    #[test]
    fn cancellation_and_invalid_names_do_not_modify_files() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("item");
        fs::write(&source, b"keep").unwrap();
        let mut w = worker("overwrite");
        w.control.cancel.store(true, Ordering::Relaxed);
        assert_eq!(
            w.run(Operation::Delete {
                sources: vec![source.clone()],
                permanent: true
            })
            .unwrap_err()
            .code,
            "cancelled"
        );
        assert!(source.exists());
        for name in ["", "..", "../escape", "a/b", "a\\b", "."] {
            assert!(filesystem::validate_name(name).is_err());
        }
        assert!(filesystem::validate_name("valid name.txt").is_ok());
    }
    #[cfg(unix)]
    #[test]
    fn symlink_loops_are_preserved_not_traversed() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("source");
        let destination = temp.path().join("destination");
        fs::create_dir(&source).unwrap();
        fs::create_dir(&destination).unwrap();
        std::os::unix::fs::symlink(".", source.join("loop")).unwrap();
        worker("overwrite")
            .run(Operation::Copy {
                sources: vec![source.clone()],
                destination: destination.clone(),
            })
            .unwrap();
        assert_eq!(
            fs::read_link(destination.join("source/loop")).unwrap(),
            Path::new(".")
        );
        worker("overwrite")
            .run(Operation::Delete {
                sources: vec![destination.join("source")],
                permanent: true,
            })
            .unwrap();
        assert!(source.exists());
    }
}

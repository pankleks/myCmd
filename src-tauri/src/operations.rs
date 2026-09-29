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
        self.progress.current_item = Some(filesystem::text(p));
        self.progress.processed_bytes += bytes;
        self.progress.processed_items += items;
        if self.last_emit.elapsed() > Duration::from_millis(80) {
            self.emit();
        }
        self.check()
    }
    fn measure(&self, p: &Path) -> Result<(u64, u64)> {
        self.check()?;
        let m = fs::symlink_metadata(p).map_err(|e| FsError::io(e, p))?;
        let mut total = (if m.is_file() { m.len() } else { 0 }, 1);
        if m.is_dir() {
            for e in fs::read_dir(p).map_err(|e| FsError::io(e, p))? {
                let child = e.map_err(|e| FsError::io(e, p))?.path();
                let n = self.measure(&child)?;
                total.0 += n.0;
                total.1 += n.1;
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
    fn transfer(&mut self, source: &Path, destination: &Path, moving: bool) -> Result<bool> {
        self.check()?;
        let meta = fs::symlink_metadata(source).map_err(|e| FsError::io(e, source))?;
        let mut target = destination.to_owned();
        let mut overwrite = false;
        while fs::symlink_metadata(&target).is_ok() {
            if same_file::is_same_file(source, &target).unwrap_or(false) {
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
                    return Ok(false);
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
            match rename_noreplace(source, &target) {
                Ok(()) => {
                    let n = self.measure(&target)?;
                    self.tick(&target, n.0, n.1)?;
                    return Ok(true);
                }
                Err(e) if e.kind() == std::io::ErrorKind::CrossesDevices => {}
                Err(e) => return Err(FsError::io(e, &target)),
            }
        }
        if meta.is_dir() {
            if !overwrite {
                fs::create_dir(&target).map_err(|e| FsError::io(e, &target))?;
            }
            let mut complete = true;
            for item in fs::read_dir(source).map_err(|e| FsError::io(e, source))? {
                let item = item.map_err(|e| FsError::io(e, source))?;
                complete &= self.transfer(&item.path(), &target.join(item.file_name()), moving)?;
            }
            if !overwrite {
                fs::set_permissions(&target, meta.permissions())
                    .map_err(|e| FsError::io(e, &target))?;
            }
            if moving && complete {
                fs::remove_dir(source).map_err(|e| FsError::io(e, source))?;
            }
            self.tick(source, 0, 1)?;
            return Ok(complete);
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
            std::os::unix::fs::symlink(link, &target).map_err(|e| FsError::io(e, &target))?;
            #[cfg(windows)]
            {
                use std::os::windows::fs::{symlink_dir, symlink_file, FileTypeExt};
                if meta.file_type().is_symlink_dir() {
                    symlink_dir(link, &target)
                } else {
                    symlink_file(link, &target)
                }
                .map_err(|e| FsError::io(e, &target))?;
            }
        } else if meta.is_file() {
            let mut input = fs::File::open(source).map_err(|e| FsError::io(e, source))?;
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
            temp.as_file()
                .set_permissions(meta.permissions())
                .map_err(|e| FsError::io(e, &target))?;
            temp.as_file()
                .sync_all()
                .map_err(|e| FsError::io(e, &target))?;
            self.check()?;
            if overwrite {
                temp.persist(&target)
            } else {
                temp.persist_noclobber(&target)
            }
            .map_err(|e| FsError::io(e.error, &target))?;
        } else {
            return Err(FsError::new(
                "invalid_path",
                "Special filesystem objects are not supported",
            ));
        }
        if moving {
            remove_leaf(source)?;
        }
        self.tick(source, 0, 1)?;
        Ok(true)
    }
    fn delete(&mut self, p: &Path) -> Result<()> {
        self.check()?;
        let m = fs::symlink_metadata(p).map_err(|e| FsError::io(e, p))?;
        if m.is_dir() {
            for e in fs::read_dir(p).map_err(|e| FsError::io(e, p))? {
                self.delete(&e.map_err(|e| FsError::io(e, p))?.path())?;
            }
            fs::remove_dir(p).map_err(|e| FsError::io(e, p))?;
        } else {
            remove_leaf(p)?;
        }
        self.tick(p, if m.is_file() { m.len() } else { 0 }, 1)
    }
    fn run(&mut self, op: Operation) -> Result<()> {
        match op {
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
                if fs::symlink_metadata(&target).is_ok() {
                    return Err(FsError::new("already_exists", "Destination already exists"));
                }
                self.check()?;
                rename_noreplace(&path, &target).map_err(|e| FsError::io(e, &path))?;
                self.progress.result_path = Some(filesystem::text(&target));
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
            } => self.batch(sources, destination, false)?,
            Operation::Move {
                sources,
                destination,
            } => self.batch(sources, destination, true)?,
        }
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

use serde::Serialize;
use std::{io, path::Path};
#[derive(Debug, Clone, Serialize)]
pub struct FsError { pub code: String, pub message: String, pub path: Option<String> }
impl FsError {
    pub fn new(code: &str, message: impl Into<String>) -> Self { Self { code: code.into(), message: message.into(), path: None } }
    pub fn io(e: io::Error, path: &Path) -> Self {
        let code = match e.kind() {
            io::ErrorKind::NotFound => "not_found", io::ErrorKind::PermissionDenied => "permission_denied",
            io::ErrorKind::AlreadyExists => "already_exists", io::ErrorKind::InvalidInput => "invalid_path",
            io::ErrorKind::StorageFull => "disk_full", io::ErrorKind::ReadOnlyFilesystem => "read_only", _ => "io_error"
        };
        Self { code: code.into(), message: e.to_string(), path: Some(path.to_string_lossy().into()) }
    }
}
pub type Result<T> = std::result::Result<T, FsError>;

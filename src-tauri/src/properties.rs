use crate::{
    error::{FsError, Result},
    filesystem::{self, Entry},
};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileProperties {
    #[serde(flatten)]
    pub entry: Entry,
    pub mode: Option<u32>,
    pub owner: Option<String>,
    pub group: Option<String>,
    pub read_only: bool,
    pub link_target: Option<String>,
    pub link_exists: Option<bool>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionChange {
    pub mode: Option<u32>,
    pub read_only: Option<bool>,
}

fn path_without_following_link(path: &Path) -> Result<PathBuf> {
    match (path.parent(), path.file_name()) {
        (Some(parent), Some(name)) => {
            let parent = if parent.as_os_str().is_empty() {
                Path::new(".")
            } else {
                parent
            };
            Ok(filesystem::absolute(parent)?.join(name))
        }
        _ => filesystem::absolute(path),
    }
}

pub fn read(path: &Path) -> Result<FileProperties> {
    let path = path_without_following_link(path)?;
    let metadata = fs::symlink_metadata(&path).map_err(|e| FsError::io(e, &path))?;
    let entry = filesystem::entry(&path)?;
    #[cfg(unix)]
    let (mode, owner, group) = {
        use std::os::unix::fs::MetadataExt;
        (
            Some(metadata.mode() & 0o777),
            Some(identity(metadata.uid(), false)),
            Some(identity(metadata.gid(), true)),
        )
    };
    #[cfg(not(unix))]
    let (mode, owner, group) = (None, None, None);
    let link_target = if metadata.is_symlink() {
        Some(filesystem::text(
            &fs::read_link(&path).map_err(|e| FsError::io(e, &path))?,
        ))
    } else {
        None
    };
    let link_exists = if metadata.is_symlink() {
        match fs::metadata(&path) {
            Ok(_) => Some(true),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Some(false),
            Err(_) => None,
        }
    } else {
        None
    };
    Ok(FileProperties {
        entry,
        mode,
        owner,
        group,
        read_only: metadata.permissions().readonly(),
        link_target,
        link_exists,
    })
}

#[cfg(unix)]
fn identity(id: u32, group: bool) -> String {
    let mut buffer = vec![0u8; 16 * 1024];
    // Reentrant lookups keep names valid in this owned buffer until copied.
    let name = unsafe {
        if group {
            let mut entry = std::mem::MaybeUninit::<libc::group>::uninit();
            let mut result = std::ptr::null_mut();
            if libc::getgrgid_r(
                id,
                entry.as_mut_ptr(),
                buffer.as_mut_ptr().cast(),
                buffer.len(),
                &mut result,
            ) == 0
                && !result.is_null()
            {
                Some(
                    std::ffi::CStr::from_ptr((*result).gr_name)
                        .to_string_lossy()
                        .into_owned(),
                )
            } else {
                None
            }
        } else {
            let mut entry = std::mem::MaybeUninit::<libc::passwd>::uninit();
            let mut result = std::ptr::null_mut();
            if libc::getpwuid_r(
                id,
                entry.as_mut_ptr(),
                buffer.as_mut_ptr().cast(),
                buffer.len(),
                &mut result,
            ) == 0
                && !result.is_null()
            {
                Some(
                    std::ffi::CStr::from_ptr((*result).pw_name)
                        .to_string_lossy()
                        .into_owned(),
                )
            } else {
                None
            }
        }
    };
    name.map(|name| format!("{name} ({id})"))
        .unwrap_or_else(|| id.to_string())
}

pub fn set_permissions(path: &Path, change: PermissionChange) -> Result<FileProperties> {
    let path = path_without_following_link(path)?;
    let metadata = fs::symlink_metadata(&path).map_err(|e| FsError::io(e, &path))?;
    if metadata.is_symlink() || (!metadata.is_file() && !metadata.is_dir()) {
        return Err(FsError::new(
            "invalid_path",
            "Permissions can only be changed on regular files and folders, not symbolic links",
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::{ffi::OsStrExt, fs::MetadataExt};
        let mode = change
            .mode
            .filter(|mode| *mode <= 0o777)
            .filter(|_| change.read_only.is_none())
            .ok_or_else(|| {
                FsError::new(
                    "invalid_path",
                    "Provide owner, group, and other permissions only",
                )
            })?;
        let path_c = std::ffi::CString::new(path.as_os_str().as_bytes())
            .map_err(|_| FsError::new("invalid_path", "Invalid path"))?;
        // Do not follow a symlink swapped in after the metadata check.
        let result = unsafe {
            libc::fchmodat(
                libc::AT_FDCWD,
                path_c.as_ptr(),
                ((metadata.mode() & 0o7000) | mode) as libc::mode_t,
                libc::AT_SYMLINK_NOFOLLOW,
            )
        };
        if result != 0 {
            return Err(FsError::io(std::io::Error::last_os_error(), &path));
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        let read_only = change
            .read_only
            .filter(|_| change.mode.is_none())
            .ok_or_else(|| FsError::new("invalid_path", "Provide the read-only attribute only"))?;
        let file = fs::OpenOptions::new()
            .access_mode(0x80 | 0x100) // FILE_READ_ATTRIBUTES | FILE_WRITE_ATTRIBUTES
            .custom_flags(0x00200000 | 0x02000000) // OPEN_REPARSE_POINT | BACKUP_SEMANTICS
            .open(&path)
            .map_err(|e| FsError::io(e, &path))?;
        let current = file.metadata().map_err(|e| FsError::io(e, &path))?;
        if current.is_symlink() {
            return Err(FsError::new(
                "invalid_path",
                "Symbolic link permissions cannot be changed",
            ));
        }
        let mut permissions = current.permissions();
        permissions.set_readonly(read_only);
        file.set_permissions(permissions)
            .map_err(|e| FsError::io(e, &path))?;
    }
    read(&path)
}

#[tauri::command]
pub async fn get_file_properties(path: String) -> Result<FileProperties> {
    tauri::async_runtime::spawn_blocking(move || read(Path::new(&path)))
        .await
        .map_err(|e| FsError::new("io_error", e.to_string()))?
}

#[tauri::command]
pub async fn set_file_permissions(
    path: String,
    permissions: PermissionChange,
) -> Result<FileProperties> {
    tauri::async_runtime::spawn_blocking(move || set_permissions(Path::new(&path), permissions))
        .await
        .map_err(|e| FsError::new("io_error", e.to_string()))?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_file_and_directory_metadata() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("notes.txt");
        fs::write(&path, "hello").unwrap();
        let props = read(&path).unwrap();
        assert_eq!(props.entry.name, "notes.txt");
        assert_eq!(props.entry.size, 5);
        assert_eq!(props.entry.r#type, "file");
        assert!(props.entry.modified.is_some());
        assert!(props.link_target.is_none());
        assert_eq!(read(dir.path()).unwrap().entry.r#type, "directory");
        assert!(read(&dir.path().join("missing")).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn updates_rwx_without_recursion_and_preserves_special_bits() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path().join("folder");
        fs::create_dir(&folder).unwrap();
        let file = folder.join("file");
        fs::write(&file, "hello").unwrap();
        fs::set_permissions(&file, fs::Permissions::from_mode(0o640)).unwrap();
        fs::set_permissions(&folder, fs::Permissions::from_mode(0o1755)).unwrap();
        let props = set_permissions(
            &folder,
            PermissionChange {
                mode: Some(0o750),
                read_only: None,
            },
        )
        .unwrap();
        assert_eq!(props.mode, Some(0o750));
        assert!(props.owner.is_some());
        assert!(props.group.is_some());
        assert_eq!(
            fs::metadata(&folder).unwrap().permissions().mode() & 0o7777,
            0o1750
        );
        assert_eq!(
            fs::metadata(&file).unwrap().permissions().mode() & 0o777,
            0o640
        );
        fs::set_permissions(&file, fs::Permissions::from_mode(0o0)).unwrap();
        set_permissions(
            &file,
            PermissionChange {
                mode: Some(0o600),
                read_only: None,
            },
        )
        .unwrap();
        assert_eq!(fs::read_to_string(file).unwrap(), "hello");
    }

    #[cfg(unix)]
    #[test]
    fn symlinks_are_described_without_modifying_targets() {
        use std::os::unix::fs::{symlink, PermissionsExt};
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("file");
        fs::write(&path, "hello").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).unwrap();
        let link = dir.path().join("link");
        symlink("file", &link).unwrap();
        let props = read(&link).unwrap();
        assert_eq!(props.entry.r#type, "symlink");
        assert_eq!(props.entry.name, "link");
        assert_eq!(props.link_target.as_deref(), Some("file"));
        assert_eq!(props.link_exists, Some(true));
        assert!(set_permissions(
            &link,
            PermissionChange {
                mode: Some(0o777),
                read_only: None
            }
        )
        .is_err());
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o640
        );
        fs::remove_file(path).unwrap();
        assert_eq!(read(&link).unwrap().link_exists, Some(false));
    }

    #[cfg(unix)]
    #[test]
    fn invalid_permissions_leave_files_unchanged() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("file");
        fs::write(&path, "hello").unwrap();
        let mode = read(&path).unwrap().mode;
        for change in [
            PermissionChange {
                mode: Some(0o4755),
                read_only: None,
            },
            PermissionChange {
                mode: None,
                read_only: Some(false),
            },
            PermissionChange {
                mode: Some(0o777),
                read_only: Some(false),
            },
        ] {
            assert!(set_permissions(&path, change).is_err());
            assert_eq!(read(&path).unwrap().mode, mode);
        }
    }

    #[cfg(unix)]
    #[test]
    fn size_calculation_skips_links_and_rejects_a_link_as_root() {
        use std::os::unix::fs::symlink;
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path().join("folder");
        fs::create_dir(&folder).unwrap();
        fs::write(folder.join("file"), "hello").unwrap();
        let outside = dir.path().join("outside");
        fs::create_dir(&outside).unwrap();
        fs::write(outside.join("secret"), "not included").unwrap();
        symlink(&outside, folder.join("external")).unwrap();
        symlink(".", folder.join("loop")).unwrap();
        assert_eq!(
            filesystem::directory_size_checked(&folder, || Ok(())).unwrap(),
            5
        );
        assert!(filesystem::directory_size_checked(&folder.join("external"), || Ok(())).is_err());
    }

    #[cfg(windows)]
    #[test]
    fn toggles_windows_readonly_without_affecting_contents() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("file");
        fs::write(&path, "hello").unwrap();
        let props = set_permissions(
            &path,
            PermissionChange {
                mode: None,
                read_only: Some(true),
            },
        )
        .unwrap();
        assert!(props.read_only);
        assert!(props.mode.is_none());
        let props = set_permissions(
            &path,
            PermissionChange {
                mode: None,
                read_only: Some(false),
            },
        )
        .unwrap();
        assert!(!props.read_only);
        assert_eq!(fs::read_to_string(path).unwrap(), "hello");
    }
}

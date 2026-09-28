use crate::{
    config::{self, AppConfig},
    error::{FsError, Result},
    filesystem,
    operations::{Manager, Operation, Resolution},
};
use tauri::{AppHandle, State};
#[tauri::command]
pub async fn list_directory(path: String) -> Result<filesystem::Listing> {
    tauri::async_runtime::spawn_blocking(move || filesystem::list(std::path::Path::new(&path)))
        .await
        .map_err(|e| FsError::new("io_error", e.to_string()))?
}
#[tauri::command]
pub async fn list_roots() -> Result<Vec<filesystem::Root>> {
    tauri::async_runtime::spawn_blocking(filesystem::roots)
        .await
        .map_err(|e| FsError::new("io_error", e.to_string()))
}
#[tauri::command]
pub async fn open_file(path: String) -> Result<()> {
    tauri::async_runtime::spawn_blocking(move || {
        let p = filesystem::absolute(std::path::Path::new(&path))?;
        open::that_detached(&p).map_err(|e| FsError::io(e, &p))
    })
    .await
    .map_err(|e| FsError::new("io_error", e.to_string()))?
}
#[tauri::command]
pub fn load_config() -> Result<AppConfig> {
    Ok(config::load())
}

#[tauri::command]
pub fn save_config(config: AppConfig) -> Result<()> {
    config::save(&config)
}

#[tauri::command]
pub async fn measure_directory(path: String) -> Result<u64> {
    tauri::async_runtime::spawn_blocking(move || {
        filesystem::directory_size(std::path::Path::new(&path))
    })
    .await
    .map_err(|e| FsError::new("io_error", e.to_string()))?
}

#[tauri::command]
pub fn start_operation(app: AppHandle, manager: State<Manager>, operation: Operation) -> String {
    manager.start(app, operation)
}
#[tauri::command]
pub fn cancel_operation(manager: State<Manager>, operation_id: String) {
    manager.cancel(&operation_id);
}
#[tauri::command]
pub fn resolve_conflict(
    manager: State<Manager>,
    operation_id: String,
    resolution: Resolution,
) -> Result<()> {
    manager.resolve(&operation_id, resolution)
}

use crate::{
    clipboard::FileClipboard,
    error::{FsError, Result},
};

#[tauri::command]
pub async fn copy_text_to_clipboard(
    text: String,
    state: tauri::State<'_, FileClipboard>,
) -> Result<()> {
    let clipboard = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || clipboard.copy_text(text))
        .await
        .map_err(|error| FsError::new("clipboard_error", error.to_string()))?
}

#[tauri::command]
pub async fn copy_files_to_clipboard(
    paths: Vec<String>,
    state: tauri::State<'_, FileClipboard>,
) -> Result<()> {
    let clipboard = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || clipboard.copy_files(paths))
        .await
        .map_err(|error| FsError::new("clipboard_error", error.to_string()))?
}

#[tauri::command]
pub async fn read_files_from_clipboard(
    state: tauri::State<'_, FileClipboard>,
) -> Result<Vec<String>> {
    let clipboard = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || clipboard.read_files())
        .await
        .map_err(|error| FsError::new("clipboard_error", error.to_string()))?
}

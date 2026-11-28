use crate::{
    clipboard::{ClipboardFiles, FileClipboard},
    error::{blocking, Result},
};

#[tauri::command]
pub async fn copy_text_to_clipboard(
    text: String,
    state: tauri::State<'_, FileClipboard>,
) -> Result<()> {
    let clipboard = state.inner().clone();
    blocking("clipboard_error", move || clipboard.copy_text(text)).await
}

#[tauri::command]
pub async fn copy_files_to_clipboard(
    paths: Vec<String>,
    cut: Option<bool>,
    state: tauri::State<'_, FileClipboard>,
) -> Result<()> {
    let clipboard = state.inner().clone();
    blocking("clipboard_error", move || {
        clipboard.copy_files(paths, cut.unwrap_or(false))
    })
    .await
}

#[tauri::command]
pub async fn read_files_from_clipboard(
    state: tauri::State<'_, FileClipboard>,
) -> Result<ClipboardFiles> {
    let clipboard = state.inner().clone();
    blocking("clipboard_error", move || clipboard.read_files()).await
}

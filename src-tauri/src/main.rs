#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod commands;
mod error;
mod filesystem;
mod operations;
fn main() {
    tauri::Builder::default()
        .manage(operations::Manager::default())
        .invoke_handler(tauri::generate_handler![
            commands::list_directory,
            commands::list_roots,
            commands::open_file,
            commands::start_operation,
            commands::cancel_operation,
            commands::resolve_conflict
        ])
        .run(tauri::generate_context!())
        .expect("Could not start myCmd");
}

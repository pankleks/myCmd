#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod commands;
mod config;
mod error;
mod filesystem;
mod operations;
mod shell;
mod watcher;
fn main() {
    tauri::Builder::default()
        .manage(operations::Manager::default())
        .manage(watcher::DirectoryWatchers::default())
        .invoke_handler(tauri::generate_handler![
            commands::list_directory,
            commands::list_roots,
            commands::open_file,
            commands::start_operation,
            commands::cancel_operation,
            commands::resolve_conflict,
            commands::load_config,
            commands::save_config,
            shell::run_system_command,
            watcher::watch_directories
        ])
        .run(tauri::generate_context!())
        .expect("Could not start myCmd");
}

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod cancellation;
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
        .manage(shell::ShellManager::default())
        .manage(commands::DirectorySizing::default())
        .manage(watcher::DirectoryWatchers::default())
        .invoke_handler(tauri::generate_handler![
            commands::list_directory,
            commands::read_text_preview,
            commands::read_image_preview,
            commands::read_markdown_image,
            commands::list_roots,
            commands::open_file,
            commands::measure_directory,
            commands::cancel_directory_sizing,
            commands::start_operation,
            commands::cancel_operation,
            commands::resolve_conflict,
            commands::load_config,
            commands::save_config,
            shell::run_system_command,
            shell::cancel_system_command,
            watcher::watch_directories
        ])
        .run(tauri::generate_context!())
        .expect("Could not start myCmd");
}

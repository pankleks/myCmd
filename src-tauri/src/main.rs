#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod archives;
mod cancellation;
mod clipboard;
mod clipboard_commands;
mod commands;
mod config;
mod error;
mod filesystem;
mod operations;
mod properties;
mod search;
mod sftp;
mod shell;
mod system_actions;
mod terminal;
mod watcher;
fn main() {
    #[cfg(windows)]
    if let Some(code) = system_actions::run_helper() {
        std::process::exit(code);
    }
    tauri::Builder::default()
        .manage(clipboard::FileClipboard::default())
        .manage(operations::Manager::default())
        .manage(shell::ShellManager::default())
        .manage(search::SearchManager::default())
        .manage(std::sync::Arc::new(sftp::Manager::default()))
        .manage(commands::DirectorySizing::default())
        .manage(watcher::DirectoryWatchers::default())
        .invoke_handler(tauri::generate_handler![
            clipboard_commands::copy_files_to_clipboard,
            clipboard_commands::read_files_from_clipboard,
            commands::list_directory,
            sftp::connect_sftp,
            sftp::disconnect_sftp,
            sftp::list_sftp,
            properties::get_file_properties,
            properties::set_file_permissions,
            system_actions::show_system_actions,
            clipboard_commands::copy_text_to_clipboard,
            search::search_files,
            search::cancel_search,
            commands::count_delete_entries,
            archives::list_archive,
            commands::read_text_preview,
            commands::read_pdf_preview,
            commands::read_image_preview,
            commands::read_markdown_image,
            commands::list_roots,
            commands::open_file,
            commands::edit_file,
            terminal::open_terminal,
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

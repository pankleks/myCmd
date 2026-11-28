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
mod thumbnails;
mod updates;
mod watcher;
fn main() {
    #[cfg(windows)]
    if let Some(code) = system_actions::run_helper() {
        std::process::exit(code);
    }
    tauri::Builder::default()
        .plugin(
            tauri::plugin::Builder::<tauri::Wry>::new("navigation-guard")
                // A hostile previewed file must never replace the app page with a remote one.
                .on_navigation(|_, url| is_app_url(url))
                .build(),
        )
        .manage(clipboard::FileClipboard::default())
        .manage(operations::Manager::default())
        .manage(shell::ShellManager::default())
        .manage(search::SearchManager::default())
        .manage(std::sync::Arc::new(sftp::Manager::default()))
        .manage(std::sync::Arc::new(updates::Updates::default()))
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
            commands::read_image_thumbnail,
            updates::check_for_update,
            updates::open_release_page,
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

/// The bundled app (tauri://localhost, or http(s)://tauri.localhost on Windows),
/// the Vite dev server in debug builds, and local-only documents.
fn is_app_url(url: &tauri::Url) -> bool {
    matches!(url.scheme(), "tauri" | "about" | "blob" | "data")
        || url.host_str() == Some("tauri.localhost")
        || (cfg!(debug_assertions) && url.host_str() == Some("localhost"))
}

#[cfg(test)]
mod tests {
    #[test]
    fn navigation_stays_inside_the_app() {
        let allowed = |url: &str| super::is_app_url(&url.parse().unwrap());
        assert!(allowed("tauri://localhost/index.html"));
        assert!(allowed("http://tauri.localhost/"));
        assert!(allowed("https://tauri.localhost/"));
        assert!(allowed("about:blank"));
        assert!(!allowed("https://example.com/"));
        assert!(!allowed("https://tauri.localhost.example.com/"));
        assert!(!allowed("file:///etc/passwd"));
    }
}

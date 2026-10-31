use crate::error::{FsError, Result};
use gtk::{gio, glib::variant::ToVariant, prelude::*};
use std::path::Path;

pub fn show(parent: &gtk::ApplicationWindow, path: &Path) -> Result<()> {
    let dialog = gtk::Dialog::with_buttons(
        Some("System actions"),
        Some(parent),
        gtk::DialogFlags::MODAL,
        &[("Close", gtk::ResponseType::Close)],
    );
    let name = gtk::Label::new(path.file_name().and_then(|name| name.to_str()));
    name.set_margin_top(12);
    name.set_margin_bottom(12);
    name.set_margin_start(16);
    name.set_margin_end(16);
    name.set_line_wrap(true);
    name.set_selectable(true);
    dialog.content_area().add(&name);
    dialog.add_button("Open", gtk::ResponseType::Other(1));
    dialog.add_button("Open With…", gtk::ResponseType::Other(2));
    dialog.add_button("Show in File Manager", gtk::ResponseType::Other(3));
    dialog.show_all();
    let response = dialog.run();
    dialog.hide();
    let file = gio::File::for_path(path);
    let context = parent
        .display()
        .app_launch_context()
        .map(|context| context.upcast::<gio::AppLaunchContext>())
        .unwrap_or_default();
    let result = match response {
        gtk::ResponseType::Other(1) => {
            gio::AppInfo::launch_default_for_uri(file.uri().as_str(), Some(&context))
                .map_err(native_error)
        }
        gtk::ResponseType::Other(2) => {
            let chooser = gtk::AppChooserDialog::new(Some(parent), gtk::DialogFlags::MODAL, &file);
            chooser.show_all();
            let result = if chooser.run() == gtk::ResponseType::Ok {
                chooser
                    .app_info()
                    .ok_or_else(|| FsError::new("system_actions_error", "No application selected"))
                    .and_then(|app| app.launch(&[file], Some(&context)).map_err(native_error))
            } else {
                Ok(())
            };
            chooser.close();
            result
        }
        gtk::ResponseType::Other(3) => reveal(&file, &context),
        _ => Ok(()),
    };
    dialog.close();
    result
}

fn native_error(error: impl std::fmt::Display) -> FsError {
    FsError::new("system_actions_error", error.to_string())
}

fn reveal(file: &gio::File, context: &gio::AppLaunchContext) -> Result<()> {
    let result = gio::bus_get_sync(gio::BusType::Session, gio::Cancellable::NONE).and_then(|bus| {
        bus.call_sync(
            Some("org.freedesktop.FileManager1"),
            "/org/freedesktop/FileManager1",
            "org.freedesktop.FileManager1",
            "ShowItems",
            Some(&(vec![file.uri().to_string()], "").to_variant()),
            None,
            gio::DBusCallFlags::NONE,
            3000,
            gio::Cancellable::NONE,
        )
    });
    if result.is_ok() {
        return Ok(());
    }
    // Desktops without FileManager1 can still open the item's containing folder.
    let parent = file.parent().unwrap_or_else(|| file.clone());
    gio::AppInfo::launch_default_for_uri(parent.uri().as_str(), Some(context)).map_err(native_error)
}

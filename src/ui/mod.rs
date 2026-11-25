//! The Print Pictures window and the state behind it.

mod layout_list;
pub mod model;
pub mod options;
pub mod preview;
mod welcome;
mod window;

use crate::integration::Folders;
use gtk4::prelude::*;
use gtk4::{gio, glib};
use std::path::PathBuf;

pub const APPLICATION_ID: &str = "io.github.mihledex.PrintPictures";

pub fn run(files: Vec<PathBuf>) -> glib::ExitCode {
    // Not unique: every right-click gets its own window.
    let app = gtk4::Application::builder()
        .application_id(APPLICATION_ID)
        .flags(gio::ApplicationFlags::NON_UNIQUE)
        .build();
    app.connect_activate(move |app| {
        if files.is_empty() {
            welcome(app);
        } else {
            window::open(app, files.clone());
        }
    });
    // GTK must not try to read the file names itself.
    app.run_with_args::<&str>(&[])
}

/// Started from the application menu: there is nothing to print yet.
fn welcome(app: &gtk4::Application) {
    let home = PathBuf::from(std::env::var_os("HOME").unwrap_or_default());
    let config = Folders::new(&home, std::env::var_os("XDG_CONFIG_HOME"), None).config;
    let thunar = std::env::var_os("PATH")
        .is_some_and(|path| std::env::split_paths(&path).any(|dir| dir.join("thunar").is_file()));
    let exe = std::env::current_exe().unwrap_or_else(|_| "print-pictures".into());
    welcome::show(app, config, exe, thunar);
}

#[cfg(test)]
mod testing {
    use gtk4::prelude::*;

    /// The first widget of type `T` under `root` that `wanted` accepts.
    pub fn find<T: IsA<gtk4::Widget>>(
        root: &gtk4::Widget,
        wanted: &dyn Fn(&T) -> bool,
    ) -> Option<T> {
        if let Some(found) = root.downcast_ref::<T>().filter(|w| wanted(w)) {
            return Some(found.clone());
        }
        let mut child = root.first_child();
        while let Some(widget) = child {
            if let Some(found) = find(&widget, wanted) {
                return Some(found);
            }
            child = widget.next_sibling();
        }
        None
    }
}

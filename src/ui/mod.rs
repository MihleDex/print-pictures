//! The Print Pictures window and the state behind it.

mod layout_list;
pub mod model;
pub mod preview;
mod window;

use gtk4::prelude::*;
use gtk4::{gio, glib};
use std::path::PathBuf;

const APPLICATION_ID: &str = "local.printpictures.PrintPictures";

pub fn run(files: Vec<PathBuf>) -> glib::ExitCode {
    // Not unique: every right-click gets its own window.
    let app = gtk4::Application::builder()
        .application_id(APPLICATION_ID)
        .flags(gio::ApplicationFlags::NON_UNIQUE)
        .build();
    app.connect_activate(move |app| window::open(app, files.clone()));
    // GTK must not try to read the file names itself.
    app.run_with_args::<&str>(&[])
}

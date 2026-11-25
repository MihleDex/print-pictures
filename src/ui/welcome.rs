//! Shown when started without pictures: how to use it, and the Thunar entry.

use crate::integration::{SYSTEM_UCA, add_thunar, has_thunar, remove_thunar};
use gtk4 as gtk;
use gtk4::prelude::*;
use gtk4::{gdk, glib};
use std::path::{Path, PathBuf};

fn thunar_label(config: &Path) -> &'static str {
    if has_thunar(config) {
        "Remove Print from the right-click menu"
    } else {
        "Add Print to the right-click menu"
    }
}

/// `exe` is the program the Thunar entry should run.
pub(super) fn show(
    app: &gtk::Application,
    config: PathBuf,
    exe: PathBuf,
    thunar_installed: bool,
) -> gtk::ApplicationWindow {
    let window = gtk::ApplicationWindow::builder()
        .application(app)
        .title("Print Pictures")
        .default_width(460)
        .resizable(false)
        .icon_name("printer")
        .build();
    let text = gtk::Label::builder()
        .label("Select pictures in your file manager, right-click them and choose Print.")
        .wrap(true)
        .max_width_chars(48)
        .xalign(0.0)
        .build();
    let status = gtk::Label::builder()
        .wrap(true)
        .max_width_chars(48)
        .xalign(0.0)
        .visible(false)
        .build();
    let buttons = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    buttons.set_halign(gtk::Align::End);

    // Thunar has no place for a package to put a menu entry, so it is added here.
    if thunar_installed {
        let toggle = gtk::Button::with_label(thunar_label(&config));
        let status = status.clone();
        toggle.connect_clicked(move |button| {
            let done = if has_thunar(&config) {
                remove_thunar(&config)
            } else {
                add_thunar(&config, &exe, Path::new(SYSTEM_UCA))
            };
            match done {
                Ok(()) => status.set_label("Done. Close and reopen your file manager."),
                Err(e) => status.set_label(&e.to_string()),
            }
            status.set_visible(true);
            button.set_label(thunar_label(&config));
        });
        buttons.append(&toggle);
    }

    let close = gtk::Button::with_label("Close");
    let weak = window.downgrade();
    close.connect_clicked(move |_| {
        if let Some(window) = weak.upgrade() {
            window.close();
        }
    });
    buttons.append(&close);

    let content = gtk::Box::new(gtk::Orientation::Vertical, 12);
    content.set_margin_top(18);
    content.set_margin_bottom(18);
    content.set_margin_start(18);
    content.set_margin_end(18);
    content.append(&text);
    content.append(&status);
    content.append(&buttons);
    window.set_child(Some(&content));

    let keys = gtk::EventControllerKey::new();
    let weak = window.downgrade();
    keys.connect_key_pressed(move |_, key, _, _| {
        if key == gdk::Key::Escape
            && let Some(window) = weak.upgrade()
        {
            window.close();
            glib::Propagation::Stop
        } else {
            glib::Propagation::Proceed
        }
    });
    window.add_controller(keys);
    window.present();
    close.grab_focus();
    window
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::testing::find;
    use std::sync::atomic::{AtomicUsize, Ordering};

    const ADD: &str = "Add Print to the right-click menu";
    const REMOVE: &str = "Remove Print from the right-click menu";

    fn open(config: &Path, thunar: bool) -> gtk::ApplicationWindow {
        // An id can be registered only once in a process.
        static APPS: AtomicUsize = AtomicUsize::new(0);
        let id = format!(
            "local.printpictures.Test{}",
            APPS.fetch_add(1, Ordering::Relaxed)
        );
        let app = gtk::Application::builder()
            .application_id(id)
            .flags(gtk::gio::ApplicationFlags::NON_UNIQUE)
            .build();
        app.register(gtk::gio::Cancellable::NONE).unwrap();
        let exe = PathBuf::from("/usr/bin/print-pictures");
        show(&app, config.to_path_buf(), exe, thunar)
    }

    fn button(window: &gtk::ApplicationWindow, label: &str) -> Option<gtk::Button> {
        find(window.upcast_ref(), &|b: &gtk::Button| {
            b.label().as_deref() == Some(label)
        })
    }

    fn says(window: &gtk::ApplicationWindow, part: &str) -> bool {
        find(window.upcast_ref(), &|l: &gtk::Label| {
            l.is_visible() && l.label().contains(part)
        })
        .is_some()
    }

    #[gtk4::test]
    #[ignore = "needs a display: cargo test -- --ignored welcome"]
    fn welcome_adds_and_removes() {
        let config = tempfile::tempdir().unwrap();
        let window = open(config.path(), true);
        assert!(says(
            &window,
            "Select pictures in your file manager, right-click them and choose Print."
        ));
        assert!(!says(&window, "Done."));

        button(&window, ADD).unwrap().emit_clicked();
        assert!(has_thunar(config.path()));
        assert!(says(&window, "Done. Close and reopen your file manager."));

        button(&window, REMOVE).unwrap().emit_clicked();
        assert!(!has_thunar(config.path()));
        assert!(button(&window, ADD).is_some());
        window.close();
    }

    #[gtk4::test]
    #[ignore = "needs a display: cargo test -- --ignored welcome"]
    fn welcome_without_thunar() {
        let config = tempfile::tempdir().unwrap();
        let window = open(config.path(), false);
        assert!(button(&window, ADD).is_none() && button(&window, REMOVE).is_none());
        assert!(button(&window, "Close").is_some());
        window.close();
    }

    #[gtk4::test]
    #[ignore = "needs a display: cargo test -- --ignored welcome"]
    fn welcome_reports_a_broken_file() {
        let config = tempfile::tempdir().unwrap();
        let uca = config.path().join("Thunar/uca.xml");
        std::fs::create_dir_all(uca.parent().unwrap()).unwrap();
        std::fs::write(&uca, "<broken>").unwrap();
        let window = open(config.path(), true);
        button(&window, ADD).unwrap().emit_clicked();
        assert!(says(&window, "uca.xml"));
        assert_eq!(std::fs::read_to_string(&uca).unwrap(), "<broken>");
        window.close();
    }
}

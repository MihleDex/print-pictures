//! "Print Settings" and "Printer Properties".

use super::model::Model;
use super::window::Picker;
use gtk4 as gtk;
use gtk4::prelude::*;
use gtk4::{gdk, glib};
use std::cell::RefCell;
use std::rc::Rc;

/// A modal window with `content` above OK and Cancel. OK runs `apply`; Cancel
/// and Escape close without it.
fn dialog(
    parent: &gtk::Window,
    title: &str,
    content: &impl IsA<gtk::Widget>,
    apply: impl Fn() + 'static,
) -> gtk::Window {
    let window = gtk::Window::builder()
        .title(title)
        .transient_for(parent)
        .modal(true)
        .destroy_with_parent(true)
        .resizable(false)
        .build();
    let ok = gtk::Button::with_label("OK");
    ok.add_css_class("suggested-action");
    let cancel = gtk::Button::with_label("Cancel");
    let buttons = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    buttons.set_halign(gtk::Align::End);
    buttons.append(&ok);
    buttons.append(&cancel);
    let layout = gtk::Box::new(gtk::Orientation::Vertical, 12);
    layout.set_margin_top(12);
    layout.set_margin_bottom(12);
    layout.set_margin_start(12);
    layout.set_margin_end(12);
    layout.append(content);
    layout.append(&buttons);
    window.set_child(Some(&layout));
    window.set_default_widget(Some(&ok));

    // Weak references, so a closed dialog can be freed.
    let weak = window.downgrade();
    let close = move || {
        if let Some(window) = weak.upgrade() {
            window.close();
        }
    };
    let on_cancel = close.clone();
    cancel.connect_clicked(move |_| on_cancel());
    let on_ok = close.clone();
    ok.connect_clicked(move |_| {
        apply();
        on_ok();
    });
    let keys = gtk::EventControllerKey::new();
    keys.connect_key_pressed(move |_, key, _, _| {
        if key == gdk::Key::Escape {
            close();
            glib::Propagation::Stop
        } else {
            glib::Propagation::Proceed
        }
    });
    window.add_controller(keys);
    window
}

pub fn show_print_settings(
    parent: &gtk::Window,
    model: Rc<RefCell<Model>>,
    on_change: Rc<dyn Fn()>,
) {
    let sharpen = gtk::CheckButton::with_label("Sharpen for printing");
    sharpen.set_active(model.borrow().sharpen());
    let properties = gtk::Button::with_label("Printer Properties...");
    properties.set_halign(gtk::Align::Start);
    properties.set_sensitive(!model.borrow().advanced_options().is_empty());
    let content = gtk::Box::new(gtk::Orientation::Vertical, 8);
    content.append(&sharpen);
    content.append(&properties);

    let (target, changed) = (model.clone(), on_change.clone());
    let window = dialog(parent, "Print Settings", &content, move || {
        target.borrow_mut().set_sharpen(sharpen.is_active());
        changed();
    });
    let weak = window.downgrade();
    properties.connect_clicked(move |_| {
        if let Some(parent) = weak.upgrade() {
            show_properties(&parent, model.clone(), on_change.clone());
        }
    });
    window.present();
}

/// Every printer setting that has no dropdown in the main window.
fn show_properties(parent: &gtk::Window, model: Rc<RefCell<Model>>, on_change: Rc<dyn Fn()>) {
    let list = gtk::Box::new(gtk::Orientation::Vertical, 8);
    let pickers: Vec<(String, Picker)> = {
        let model = model.borrow();
        model
            .advanced_options()
            .into_iter()
            .map(|option| {
                let picker = Picker::new(&option.label);
                let items = option
                    .choices
                    .iter()
                    .map(|c| (c.id.clone(), c.label.clone()));
                picker.show(items.collect(), model.option_value(&option.keyword));
                list.append(&picker.holder);
                (option.keyword.clone(), picker)
            })
            .collect()
    };
    let scroller = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .min_content_width(320)
        .propagate_natural_height(true)
        .max_content_height(420)
        .child(&list)
        .build();

    dialog(parent, "Printer Properties", &scroller, move || {
        {
            let mut model = model.borrow_mut();
            for (keyword, picker) in &pickers {
                // Only what was changed is remembered; the rest stays the printer's default.
                if let Some(value) = picker.selected_id()
                    && model.option_value(keyword) != Some(value.as_str())
                {
                    model.set_option(keyword, &value);
                }
            }
        }
        on_change();
    })
    .present();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cups::Printer;
    use crate::job::Checked;
    use crate::ppd;
    use crate::settings::Settings;
    use crate::ui::testing::find;
    use std::cell::Cell;

    fn model() -> Model {
        let checked = Checked {
            usable: vec!["/p/a.jpg".into()],
            rejected: Vec::new(),
        };
        let printers = vec![Printer {
            name: "Office".into(),
            is_default: true,
            stopped: false,
            saves_to_file: false,
        }];
        let caps =
            |_: &str| ppd::parse(include_str!("../../tests/fixtures/photo-printer.ppd")).unwrap();
        Model::new(checked, printers, Box::new(caps), Settings::default())
    }

    fn open_window(title: &str) -> gtk::Widget {
        gtk::Window::list_toplevels()
            .into_iter()
            .find(|w| {
                w.downcast_ref::<gtk::Window>()
                    .is_some_and(|w| w.is_visible() && w.title().as_deref() == Some(title))
            })
            .unwrap_or_else(|| panic!("no window called {title}"))
    }

    fn press(title: &str, label: &str) {
        find::<gtk::Button>(&open_window(title), &|b| {
            b.label().as_deref() == Some(label)
        })
        .unwrap_or_else(|| panic!("no {label} button in {title}"))
        .emit_clicked();
    }

    fn sharpen_box() -> gtk::CheckButton {
        find::<gtk::CheckButton>(&open_window("Print Settings"), &|c| {
            c.label().as_deref() == Some("Sharpen for printing")
        })
        .unwrap()
    }

    fn colour_dropdown() -> gtk::DropDown {
        find::<gtk::DropDown>(&open_window("Printer Properties"), &|_| true).unwrap()
    }

    #[gtk4::test]
    #[ignore = "needs a display: cargo test -- --ignored dialogs"]
    fn dialogs_apply_on_ok_and_discard_on_cancel() {
        let model = Rc::new(RefCell::new(model()));
        let changes = Rc::new(Cell::new(0));
        let counted = changes.clone();
        let on_change: Rc<dyn Fn()> = Rc::new(move || counted.set(counted.get() + 1));
        let parent = gtk::Window::new();
        let open = || show_print_settings(&parent, model.clone(), on_change.clone());

        open();
        assert!(sharpen_box().is_active());
        sharpen_box().set_active(false);
        press("Print Settings", "Cancel");
        assert!(model.borrow().sharpen());

        open();
        sharpen_box().set_active(false);
        press("Print Settings", "OK");
        assert!(!model.borrow().sharpen());
        assert_eq!(changes.get(), 1);

        open();
        press("Print Settings", "Printer Properties...");
        colour_dropdown().set_selected(1);
        press("Printer Properties", "Cancel");
        assert_eq!(model.borrow().option_value("ColorModel"), Some("RGB"));

        press("Print Settings", "Printer Properties...");
        colour_dropdown().set_selected(1);
        press("Printer Properties", "OK");
        assert_eq!(model.borrow().option_value("ColorModel"), Some("Gray"));
        press("Print Settings", "Cancel");
        assert_eq!(model.borrow().option_value("ColorModel"), Some("Gray"));

        // Pressing OK without touching anything pins no values.
        open();
        press("Print Settings", "Printer Properties...");
        press("Printer Properties", "OK");
        press("Print Settings", "OK");
        let kept = model.borrow().settings();
        assert_eq!(kept.printers["Office"].options.len(), 1);
    }
}

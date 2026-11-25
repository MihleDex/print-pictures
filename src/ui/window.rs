//! The Print Pictures window: widgets over `Model`.

use super::layout_list;
use super::options;
use super::preview::{self, Cache};
use crate::cups::{self, Printer, SubmitRequest, job_title};
use crate::image_source::load;
use crate::job::{Built, Checked, JobError, JobSpec, build_pdf, check_files};
use crate::layout::Paper;
use crate::ppd::{PrinterCaps, PrinterOption};
use crate::settings::Settings;
use crate::ui::model::{Model, preferred_printer, skipped_notice};
use gtk4 as gtk;
use gtk4::prelude::*;
use gtk4::{gdk, gio, glib, pango};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::rc::Rc;

/// `build_pdf` with its failures worded for the user.
fn build_for_user(spec: &JobSpec, pdf: &Path) -> Result<Built, String> {
    build_pdf(spec, pdf).map_err(|e| match e {
        JobError::NothingToPrint(skipped) if !skipped.is_empty() => skipped_notice(&skipped),
        e => e.to_string(),
    })
}

/// Builds the PDF in `dir` and sends it; returns the pictures left out.
/// Runs off the UI thread.
fn print_job(
    spec: JobSpec,
    mut request: SubmitRequest,
    dir: &Path,
) -> Result<Vec<(PathBuf, String)>, String> {
    let pdf = dir.join("job.pdf");
    let built = build_for_user(&spec, &pdf)?;
    let mut printed = spec.files;
    printed.retain(|f| !built.skipped.iter().any(|(s, _)| s == f));
    request.title = job_title(&printed);
    cups::submit(&request, &pdf).map_err(|e| e.to_string())?;
    Ok(built.skipped)
}

/// The file to write for the name picked in "Save As". It always ends in
/// ".pdf", so a picture can never be written over. True when adding that ending
/// gives a file that exists, which the dialog has not asked about.
fn pdf_target(chosen: &Path) -> (PathBuf, bool) {
    if chosen
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("pdf"))
    {
        return (chosen.to_path_buf(), false);
    }
    let mut name = chosen.as_os_str().to_owned();
    name.push(".pdf");
    let path = PathBuf::from(name);
    let exists = path.exists();
    (path, exists)
}

/// Builds the PDF in `work` and puts it at `dest`; returns the pictures left out.
/// Runs off the UI thread.
fn save_job(spec: JobSpec, work: &Path, dest: &Path) -> Result<Vec<(PathBuf, String)>, String> {
    let pdf = work.join("job.pdf");
    let built = build_for_user(&spec, &pdf)?;
    put(&pdf, dest).map_err(|e| format!("{}: {e}", dest.display()))?;
    Ok(built.skipped)
}

/// Copies beside `dest` first, so `dest` is never left half written.
fn put(from: &Path, dest: &Path) -> std::io::Result<()> {
    let mut file = tempfile::NamedTempFile::new_in(dest.parent().unwrap_or(Path::new(".")))?;
    std::io::copy(&mut std::fs::File::open(from)?, &mut file)?;
    // A document the user saves is readable like any other, unlike a temporary
    // file. A network share may refuse the change; the save matters more.
    let _ = file
        .as_file()
        .set_permissions(std::fs::Permissions::from_mode(0o644));
    file.persist(dest).map_err(|e| e.error)?;
    Ok(())
}

/// A titled dropdown that shows labels and reports ids.
pub(super) struct Picker {
    pub(super) holder: gtk::Box,
    dropdown: gtk::DropDown,
    items: RefCell<Vec<(String, String)>>,
}

impl Picker {
    pub(super) fn new(title: &str) -> Picker {
        let dropdown = gtk::DropDown::from_strings(&[]);
        // Long printer and paper names must not widen the window.
        let short = gtk::SignalListItemFactory::new();
        short.connect_setup(|_, item| {
            if let Some(item) = item.downcast_ref::<gtk::ListItem>() {
                let label = gtk::Label::builder()
                    .xalign(0.0)
                    .ellipsize(pango::EllipsizeMode::End)
                    .max_width_chars(14)
                    .build();
                item.set_child(Some(&label));
            }
        });
        short.connect_bind(|_, item| {
            if let Some(item) = item.downcast_ref::<gtk::ListItem>()
                && let Some(text) = item.item().and_downcast::<gtk::StringObject>()
                && let Some(label) = item.child().and_downcast::<gtk::Label>()
            {
                label.set_label(&text.string());
            }
        });
        let full = dropdown.factory();
        dropdown.set_list_factory(full.as_ref());
        dropdown.set_factory(Some(&short));

        let label = gtk::Label::builder().label(title).xalign(0.0).build();
        label.set_mnemonic_widget(Some(&dropdown));
        let holder = gtk::Box::new(gtk::Orientation::Vertical, 2);
        holder.set_hexpand(true);
        holder.append(&label);
        holder.append(&dropdown);
        Picker {
            holder,
            dropdown,
            items: RefCell::new(Vec::new()),
        }
    }

    /// `items` are (id, label) pairs.
    pub(super) fn show(&self, items: Vec<(String, String)>, selected: Option<&str>) {
        if *self.items.borrow() != items {
            let labels: Vec<&str> = items.iter().map(|(_, label)| label.as_str()).collect();
            self.dropdown
                .set_model(Some(&gtk::StringList::new(&labels)));
            *self.items.borrow_mut() = items;
        }
        let position = selected.and_then(|id| self.items.borrow().iter().position(|i| i.0 == id));
        self.dropdown
            .set_selected(position.map_or(gtk::INVALID_LIST_POSITION, |p| p as u32));
    }

    pub(super) fn selected_id(&self) -> Option<String> {
        let at = self.dropdown.selected() as usize;
        self.items.borrow().get(at).map(|(id, _)| id.clone())
    }

    /// Shows a printer option, or hides the dropdown when there is none.
    fn show_option(&self, option: Option<&PrinterOption>, value: Option<&str>) {
        self.holder.set_visible(option.is_some());
        if let Some(option) = option {
            let items = option
                .choices
                .iter()
                .map(|c| (c.id.clone(), c.label.clone()));
            self.show(items.collect(), value);
        }
    }
}

struct Widgets {
    window: gtk::ApplicationWindow,
    /// Everything that is locked while loading or printing.
    body: gtk::Box,
    printer: Picker,
    paper: Picker,
    quality: Picker,
    paper_type: Picker,
    preview: gtk::DrawingArea,
    previous: gtk::Button,
    next: gtk::Button,
    page_label: gtk::Label,
    layouts: gtk::ListBox,
    notice: gtk::Label,
    copies: gtk::SpinButton,
    fill: gtk::CheckButton,
    options: gtk::Button,
    spinner: gtk::Spinner,
    status: gtk::Label,
    print: gtk::Button,
    cancel: gtk::Button,
}

fn row(spacing: i32) -> gtk::Box {
    gtk::Box::new(gtk::Orientation::Horizontal, spacing)
}

fn column(spacing: i32) -> gtk::Box {
    gtk::Box::new(gtk::Orientation::Vertical, spacing)
}

fn page_button(symbol: &str, name: &str) -> gtk::Button {
    let button = gtk::Button::with_label(symbol);
    button.update_property(&[gtk::accessible::Property::Label(name)]);
    button
}

fn build(app: &gtk::Application) -> Widgets {
    let window = gtk::ApplicationWindow::builder()
        .application(app)
        .title("Print Pictures")
        .default_width(735)
        .default_height(550)
        .icon_name("printer")
        .build();

    let header = gtk::Label::builder()
        .label("How do you want to print your pictures?")
        .xalign(0.0)
        .css_classes(["title-4"])
        .build();

    let printer = Picker::new("Printer:");
    let paper = Picker::new("Paper size:");
    let quality = Picker::new("Quality:");
    let paper_type = Picker::new("Paper type:");
    let choices = row(8);
    for choice in [&printer, &paper, &quality, &paper_type] {
        choices.append(&choice.holder);
    }

    let preview = gtk::DrawingArea::builder()
        .hexpand(true)
        .vexpand(true)
        .build();
    let previous = page_button("◀", "Previous page");
    let next = page_button("▶", "Next page");
    let page_label = gtk::Label::new(None);
    let paging = row(8);
    paging.set_halign(gtk::Align::Center);
    paging.append(&previous);
    paging.append(&page_label);
    paging.append(&next);
    let left = column(4);
    left.append(&preview);
    left.append(&paging);

    let layouts = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::Single)
        .build();
    let scroller = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .width_request(150)
        .has_frame(true)
        .child(&layouts)
        .build();
    let middle = row(8);
    middle.set_vexpand(true);
    middle.append(&left);
    middle.append(&scroller);

    let notice = gtk::Label::builder()
        .xalign(0.0)
        .wrap(true)
        .visible(false)
        .build();

    let copies = gtk::SpinButton::with_range(1.0, 99.0, 1.0);
    let copies_label = gtk::Label::new(Some("Copies of each picture:"));
    copies_label.set_mnemonic_widget(Some(&copies));
    let fill = gtk::CheckButton::with_label("Fit picture to frame");
    let job_row = row(8);
    job_row.append(&copies_label);
    job_row.append(&copies);
    job_row.append(&fill);

    let options = gtk::Button::builder()
        .label("Options...")
        .halign(gtk::Align::End)
        .has_frame(false)
        .css_classes(["link"])
        .build();

    let body = column(8);
    body.set_vexpand(true);
    for widget in [header.upcast_ref::<gtk::Widget>(), choices.upcast_ref()] {
        body.append(widget);
    }
    body.append(&middle);
    body.append(&notice);
    body.append(&job_row);
    body.append(&options);

    let spinner = gtk::Spinner::new();
    let status = gtk::Label::builder().xalign(0.0).hexpand(true).build();
    let print = gtk::Button::with_label("Print");
    print.add_css_class("suggested-action");
    let cancel = gtk::Button::with_label("Cancel");
    let bottom = row(8);
    bottom.append(&spinner);
    bottom.append(&status);
    bottom.append(&print);
    bottom.append(&cancel);

    let content = column(8);
    content.set_margin_top(12);
    content.set_margin_bottom(12);
    content.set_margin_start(12);
    content.set_margin_end(12);
    content.append(&body);
    content.append(&bottom);
    window.set_child(Some(&content));

    Widgets {
        window,
        body,
        printer,
        paper,
        quality,
        paper_type,
        preview,
        previous,
        next,
        page_label,
        layouts,
        notice,
        copies,
        fill,
        options,
        spinner,
        status,
        print,
        cancel,
    }
}

/// What has to be read from disk and CUPS before the dialog can be filled in.
struct Loaded {
    checked: Checked,
    printers: Result<Vec<Printer>, String>,
    settings: Settings,
    capabilities: Option<(String, PrinterCaps)>,
}

fn read_everything(files: Vec<PathBuf>) -> Loaded {
    let checked = check_files(&files);
    let printers = cups::printers().map_err(|e| e.to_string());
    let settings = Settings::load_from(&Settings::default_path());
    let capabilities = printers.as_ref().ok().and_then(|printers| {
        let name = &printers[preferred_printer(printers, &settings)?].name;
        Some((name.clone(), cups::capabilities(name)))
    });
    Loaded {
        checked,
        printers,
        settings,
        capabilities,
    }
}

/// The layout rows on screen and the paper they were drawn for.
type Listed = (Vec<(crate::layout::LayoutId, String)>, Option<Paper>);

/// Scrolls the layout list so that `row` can be seen.
fn reveal(layouts: &gtk::ListBox, row: gtk::ListBoxRow) {
    // A row has no position until it has been laid out, a frame or so from now.
    layouts.add_tick_callback(move |list, _| {
        if row.height() == 0 && row.parent().is_some() {
            return glib::ControlFlow::Continue;
        }
        if let Some(viewport) = list.parent().and_downcast::<gtk::Viewport>() {
            viewport.scroll_to(&row, None);
        }
        glib::ControlFlow::Break
    });
}

struct Dialog {
    ui: Widgets,
    model: Rc<RefCell<Model>>,
    capabilities: Rc<RefCell<HashMap<String, PrinterCaps>>>,
    pictures: RefCell<Cache>,
    /// The layout rows on screen, to rebuild them only when they change.
    listed: RefCell<Listed>,
    /// Why the printers could not be listed, if they could not.
    printer_error: Option<String>,
    /// Set while the widgets are being filled in, so their signals are ignored.
    updating: Cell<bool>,
    busy: Cell<bool>,
    /// Holds the PDF while printing; dropping it removes the file.
    workdir: RefCell<Option<tempfile::TempDir>>,
}

impl Dialog {
    fn new(ui: Widgets, loaded: Loaded) -> Rc<Dialog> {
        let capabilities = Rc::new(RefCell::new(HashMap::new()));
        capabilities.borrow_mut().extend(loaded.capabilities);
        let known = capabilities.clone();
        let caps_of = move |name: &str| {
            known
                .borrow()
                .get(name)
                .cloned()
                .unwrap_or_else(PrinterCaps::fallback)
        };
        let (printers, printer_error) = match loaded.printers {
            Ok(printers) => (printers, None),
            Err(e) => (Vec::new(), Some(e)),
        };
        let model = Model::new(loaded.checked, printers, Box::new(caps_of), loaded.settings);
        Rc::new(Dialog {
            ui,
            model: Rc::new(RefCell::new(model)),
            capabilities,
            pictures: RefCell::new(Cache::default()),
            listed: RefCell::new((Vec::new(), None)),
            printer_error,
            updating: Cell::new(false),
            busy: Cell::new(false),
            workdir: RefCell::new(None),
        })
    }

    /// Applies a change the user made, then redraws everything from the model.
    fn change(self: &Rc<Self>, change: impl FnOnce(&mut Model)) {
        if self.updating.get() {
            return;
        }
        change(&mut self.model.borrow_mut());
        self.refresh();
    }

    fn refresh(self: &Rc<Self>) {
        self.updating.set(true);
        let ui = &self.ui;
        {
            let model = self.model.borrow();
            let printers = model
                .printers()
                .iter()
                .map(|p| (p.name.clone(), p.name.clone()));
            ui.printer
                .show(printers.collect(), model.printer().map(|p| p.name.as_str()));
            let caps = model.caps();
            let papers = caps.papers.iter().map(|p| (p.id.clone(), p.label.clone()));
            ui.paper.show(papers.collect(), Some(&model.paper().id));
            for (choice, option) in [
                (&ui.quality, caps.quality()),
                (&ui.paper_type, caps.paper_type()),
            ] {
                let value = option.and_then(|o| model.option_value(&o.keyword));
                choice.show_option(option, value);
            }

            let layouts = (model.layouts(), Some(model.paper().paper));
            let rebuilt = *self.listed.borrow() != layouts;
            if rebuilt {
                ui.layouts.remove_all();
                for (layout, caption) in &layouts.0 {
                    ui.layouts
                        .append(&layout_list::row(*layout, caption, model.paper().paper));
                }
                *self.listed.borrow_mut() = layouts;
            }
            let current = self
                .listed
                .borrow()
                .0
                .iter()
                .position(|l| l.0 == model.layout());
            if let Some(row) = current.and_then(|i| ui.layouts.row_at_index(i as i32)) {
                ui.layouts.select_row(Some(&row));
                if rebuilt {
                    reveal(&ui.layouts, row);
                }
            }

            ui.copies.set_value(f64::from(model.copies()));
            ui.fill.set_active(model.fill());
            ui.page_label.set_label(&model.page_label());
            ui.previous.set_sensitive(model.page_index() > 0);
            ui.next
                .set_sensitive(model.page_index() + 1 < model.pages().len());

            let notice: Vec<String> = self
                .printer_error
                .clone()
                .into_iter()
                .chain(model.notice())
                .collect();
            ui.notice.set_label(&notice.join("\n"));
            ui.notice.set_visible(!notice.is_empty());
            ui.print
                .set_sensitive(model.can_print() && !self.busy.get() && ui.body.is_sensitive());
        }
        self.updating.set(false);
        self.load_pictures();
        ui.preview.queue_draw();
    }

    /// Decodes the pictures of the visible page off the UI thread.
    fn load_pictures(self: &Rc<Self>) {
        let model = self.model.borrow();
        let new = self.pictures.borrow_mut().want(&preview::wanted(&model));
        for key in new {
            let path = model.files()[key.image].clone();
            let dialog = self.clone();
            glib::spawn_future_local(async move {
                let loaded = gio::spawn_blocking(move || load(&path, key.long, key.short)).await;
                // A picture that cannot be read stays grey; printing will name it.
                if let Ok(Ok(raster)) = loaded
                    && dialog.pictures.borrow_mut().deliver(key, raster)
                {
                    dialog.ui.preview.queue_draw();
                }
            });
        }
    }

    /// Locks the dialog while something slow runs; `status` says what, if anything.
    fn wait(&self, waiting: bool, status: &str) {
        self.ui.body.set_sensitive(!waiting);
        self.ui.spinner.set_spinning(waiting);
        self.ui.status.set_label(status);
        if waiting {
            self.ui.print.set_sensitive(false);
        }
    }

    fn choose_printer(self: &Rc<Self>, name: String) {
        if self.updating.get() {
            return;
        }
        if self.capabilities.borrow().contains_key(&name) {
            self.change(|m| m.set_printer(&name));
            return;
        }
        // A network printer can take a while to answer.
        self.wait(true, "");
        let dialog = self.clone();
        glib::spawn_future_local(async move {
            let asked = name.clone();
            let caps = gio::spawn_blocking(move || cups::capabilities(&asked)).await;
            if let Ok(caps) = caps {
                dialog.capabilities.borrow_mut().insert(name.clone(), caps);
                dialog.model.borrow_mut().set_printer(&name);
            }
            dialog.wait(false, "");
            dialog.refresh();
        });
    }

    fn alert(&self, message: &str, then: impl FnOnce() + 'static) {
        let dialog = gtk::AlertDialog::builder()
            .message(message)
            .modal(true)
            .build();
        dialog.choose(Some(&self.ui.window), gio::Cancellable::NONE, move |_| {
            then()
        });
    }

    fn print(self: &Rc<Self>) {
        if !self.ui.print.is_sensitive() {
            return;
        }
        // Take in a number that was typed but not yet confirmed.
        self.ui.copies.update();
        let copies = self.ui.copies.value_as_int();
        self.model.borrow_mut().set_copies(copies.max(1) as u32);
        if self.model.borrow().saves_to_file() {
            return self.ask_where(None);
        }
        let Some((spec, request)) = self.model.borrow().job() else {
            return;
        };
        self.run(move |work| print_job(spec, request, work), None);
    }

    /// "Save As" for a printer that writes files. `existing` reopens it on a file
    /// so that pressing Save asks about replacing it.
    fn ask_where(self: &Rc<Self>, existing: Option<PathBuf>) {
        let chooser = gtk::FileDialog::builder()
            .title("Save As")
            .modal(true)
            .build();
        match existing {
            Some(file) => chooser.set_initial_file(Some(&gio::File::for_path(file))),
            None => {
                let (folder, name) = self.model.borrow().pdf_suggestion();
                chooser.set_initial_folder(Some(&gio::File::for_path(folder)));
                chooser.set_initial_name(Some(&name));
            }
        }
        let dialog = self.clone();
        chooser.save(
            Some(&self.ui.window),
            gio::Cancellable::NONE,
            move |chosen| {
                // An error means the box was closed without choosing.
                let Ok(chosen) = chosen else { return };
                let Some(path) = chosen.path() else {
                    return dialog.alert("Choose a folder on this computer.", || {});
                };
                let (target, ask_again) = pdf_target(&path);
                if ask_again {
                    return dialog.ask_where(Some(target));
                }
                let Some((spec, _)) = dialog.model.borrow().job() else {
                    return;
                };
                let folder = target.parent().map(Path::to_path_buf);
                dialog.run(move |work| save_job(spec, work, &target), folder);
            },
        );
    }

    /// Prints or saves off the UI thread, then closes the window or says what
    /// went wrong. `saved_to` is the folder to remember when saving.
    fn run(
        self: &Rc<Self>,
        job: impl FnOnce(&Path) -> Result<Vec<(PathBuf, String)>, String> + Send + 'static,
        saved_to: Option<PathBuf>,
    ) {
        let dir = match tempfile::tempdir() {
            Ok(dir) => dir,
            Err(e) => return self.alert(&e.to_string(), || {}),
        };
        let path = dir.path().to_path_buf();
        *self.workdir.borrow_mut() = Some(dir);
        self.busy.set(true);
        self.wait(true, "Printing...");

        let dialog = self.clone();
        glib::spawn_future_local(async move {
            let result = gio::spawn_blocking(move || job(&path)).await;
            dialog.workdir.take();
            let result = result.unwrap_or_else(|_| Err("printing stopped unexpectedly".into()));
            match result {
                Ok(skipped) => {
                    if let Some(folder) = saved_to {
                        dialog.model.borrow_mut().set_pdf_folder(folder);
                    }
                    if let Err(e) = dialog
                        .model
                        .borrow()
                        .settings()
                        .save_to(&Settings::default_path())
                    {
                        eprintln!("could not remember these choices: {e}");
                    }
                    let window = dialog.ui.window.clone();
                    if skipped.is_empty() {
                        window.close();
                    } else {
                        dialog.alert(&skipped_notice(&skipped), move || window.close());
                    }
                }
                Err(message) => {
                    dialog.busy.set(false);
                    dialog.wait(false, "");
                    dialog.refresh();
                    dialog.alert(&message, || {});
                }
            }
        });
    }

    fn connect(self: &Rc<Self>) {
        let ui = &self.ui;
        let me = || self.clone();

        let d = me();
        ui.printer.dropdown.connect_selected_notify(move |_| {
            if let Some(name) = d.ui.printer.selected_id() {
                d.choose_printer(name);
            }
        });
        let d = me();
        ui.paper.dropdown.connect_selected_notify(move |_| {
            if let Some(id) = d.ui.paper.selected_id() {
                d.change(|m| m.set_paper(&id));
            }
        });
        let d = me();
        ui.quality.dropdown.connect_selected_notify(move |_| {
            if let Some(value) = d.ui.quality.selected_id() {
                d.change(|m| {
                    if let Some(keyword) = m.caps().quality().map(|o| o.keyword.clone()) {
                        m.set_option(&keyword, &value);
                    }
                });
            }
        });
        let d = me();
        ui.paper_type.dropdown.connect_selected_notify(move |_| {
            if let Some(value) = d.ui.paper_type.selected_id() {
                d.change(|m| {
                    if let Some(keyword) = m.caps().paper_type().map(|o| o.keyword.clone()) {
                        m.set_option(&keyword, &value);
                    }
                });
            }
        });
        let d = me();
        ui.layouts.connect_row_selected(move |_, row| {
            if d.updating.get() {
                return;
            }
            let chosen = row.and_then(|r| d.listed.borrow().0.get(r.index() as usize).map(|l| l.0));
            if let Some(layout) = chosen {
                d.change(|m| m.set_layout(layout));
            }
        });
        let d = me();
        ui.copies.connect_value_changed(move |spin| {
            let copies = spin.value_as_int().max(1) as u32;
            d.change(|m| m.set_copies(copies));
        });
        let d = me();
        ui.fill.connect_toggled(move |check| {
            let on = check.is_active();
            d.change(|m| m.set_fill(on));
        });
        let d = me();
        ui.previous
            .connect_clicked(move |_| d.change(Model::prev_page));
        let d = me();
        ui.next.connect_clicked(move |_| d.change(Model::next_page));
        let d = me();
        ui.options.connect_clicked(move |_| {
            let refresh = d.clone();
            options::show_print_settings(
                d.ui.window.upcast_ref(),
                d.model.clone(),
                Rc::new(move || refresh.refresh()),
            );
        });
        let d = me();
        ui.print.connect_clicked(move |_| d.print());
        let d = me();
        ui.preview.set_draw_func(move |_, cr, w, h| {
            let (model, pictures) = (d.model.borrow(), d.pictures.borrow());
            if let Err(e) = preview::draw(cr, f64::from(w), f64::from(h), &model, &pictures) {
                eprintln!("preview: {e}");
            }
        });
        // Closing while printing abandons the job, so remove its file too.
        let d = me();
        ui.window.connect_close_request(move |_| {
            d.workdir.take();
            glib::Propagation::Proceed
        });

        // Enter prints and the arrows page, except where a control needs them itself.
        let keys = gtk::EventControllerKey::new();
        keys.set_propagation_phase(gtk::PropagationPhase::Capture);
        let d = me();
        keys.connect_key_pressed(move |_, key, _, _| {
            let focus = gtk::prelude::RootExt::focus(&d.ui.window);
            let focus = focus.as_ref();
            if focus.is_some_and(|f| f.ancestor(gtk::Popover::static_type()).is_some()) {
                return glib::Propagation::Proceed;
            }
            match key {
                gdk::Key::Return | gdk::Key::KP_Enter
                    if !focus
                        .is_some_and(|f| f.is::<gtk::Button>() || f.is::<gtk::CheckButton>()) =>
                {
                    d.print();
                }
                gdk::Key::Left | gdk::Key::Right if !focus.is_some_and(|f| f.is::<gtk::Text>()) => {
                    let step = if key == gdk::Key::Left {
                        Model::prev_page
                    } else {
                        Model::next_page
                    };
                    d.change(step);
                }
                _ => return glib::Propagation::Proceed,
            }
            glib::Propagation::Stop
        });
        ui.window.add_controller(keys);
    }
}

/// Opens a window for `files`. It appears at once and fills in when ready.
pub fn open(app: &gtk::Application, files: Vec<PathBuf>) {
    let ui = build(app);

    // Cancel and Escape work from the start, even while still loading.
    let window = ui.window.clone();
    ui.cancel.connect_clicked(move |_| window.close());
    let escape = gtk::EventControllerKey::new();
    let window = ui.window.clone();
    escape.connect_key_pressed(move |_, key, _, _| {
        if key == gdk::Key::Escape {
            window.close();
            glib::Propagation::Stop
        } else {
            glib::Propagation::Proceed
        }
    });
    ui.window.add_controller(escape);

    ui.body.set_sensitive(false);
    ui.print.set_sensitive(false);
    ui.spinner.set_spinning(true);
    ui.window.present();

    glib::spawn_future_local(async move {
        let Ok(loaded) = gio::spawn_blocking(move || read_everything(files)).await else {
            ui.spinner.set_spinning(false);
            ui.status.set_label("Could not read the printers.");
            return;
        };
        let dialog = Dialog::new(ui, loaded);
        dialog.connect();
        dialog.wait(false, "");
        dialog.refresh();
        dialog.ui.print.grab_focus();
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::{LayoutId, Margins, Paper};
    use gdk_pixbuf::{Colorspace, Pixbuf};
    use std::os::unix::fs::PermissionsExt;

    fn job(files: Vec<PathBuf>) -> (JobSpec, SubmitRequest) {
        let spec = JobSpec {
            files,
            layout: LayoutId::FullPage,
            paper: Paper::a4(Margins::default()),
            copies: 1,
            fill: true,
            sharpen: false,
        };
        let request = SubmitRequest {
            printer: "NoSuchPrinter_pp_test".into(),
            title: String::new(),
            paper_id: "A4".into(),
            options: Vec::new(),
        };
        (spec, request)
    }

    fn picture_in(dir: &Path) -> PathBuf {
        let picture = dir.join("ok.png");
        let pixbuf = Pixbuf::new(Colorspace::Rgb, false, 8, 8, 8).unwrap();
        pixbuf.fill(0);
        pixbuf.savev(&picture, "png", &[]).unwrap();
        picture
    }

    #[test]
    fn a_name_without_pdf_gets_it_and_never_replaces_silently() {
        let dir = tempfile::tempdir().unwrap();
        let at = |name: &str| dir.path().join(name);
        assert_eq!(pdf_target(&at("a.pdf")), (at("a.pdf"), false));
        assert_eq!(pdf_target(&at("a.PDF")), (at("a.PDF"), false));
        assert_eq!(
            pdf_target(&at("holiday.jpg")),
            (at("holiday.jpg.pdf"), false)
        );
        std::fs::write(at("report.pdf"), "x").unwrap();
        assert_eq!(pdf_target(&at("report")), (at("report.pdf"), true));
    }

    #[test]
    fn saving_writes_the_pdf_where_asked() {
        let pictures = tempfile::tempdir().unwrap();
        let work = tempfile::tempdir().unwrap();
        let target = tempfile::tempdir().unwrap();
        let dest = target.path().join("My Photos.pdf");
        let gone = pictures.path().join("gone.jpg");

        let (spec, _) = job(vec![gone.clone()]);
        assert_eq!(
            save_job(spec, work.path(), &dest).unwrap_err(),
            "Skipped: gone.jpg"
        );
        assert!(!dest.exists());

        let (spec, _) = job(vec![picture_in(pictures.path()), gone]);
        let skipped = save_job(spec, work.path(), &dest).unwrap();
        assert_eq!(skipped.len(), 1);
        assert!(std::fs::read(&dest).unwrap().starts_with(b"%PDF"));
        let mode = std::fs::metadata(&dest).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o644);
        let files = std::fs::read_dir(target.path()).unwrap().count();
        assert_eq!(files, 1, "no temporary file left");
    }

    #[test]
    fn a_folder_that_cannot_be_written_gives_a_message() {
        let pictures = tempfile::tempdir().unwrap();
        let work = tempfile::tempdir().unwrap();
        let target = tempfile::tempdir().unwrap();
        let (spec, _) = job(vec![picture_in(pictures.path())]);
        let set = |mode| {
            let permissions = std::fs::Permissions::from_mode(mode);
            std::fs::set_permissions(target.path(), permissions).unwrap();
        };
        set(0o500);
        let result = save_job(spec, work.path(), &target.path().join("x.pdf"));
        set(0o700);
        let message = result.unwrap_err();
        assert!(message.contains("x.pdf"), "{message}");
    }

    // A network share can refuse permission changes. To see it refused, run the
    // built test program as
    // `strace -f -qq -o /dev/null -e inject=fchmod:error=EOPNOTSUPP PROGRAM --ignored refused`.
    #[test]
    #[ignore = "only meaningful under strace; see the comment above"]
    fn a_refused_permission_change_still_saves() {
        let pictures = tempfile::tempdir().unwrap();
        let work = tempfile::tempdir().unwrap();
        let target = tempfile::tempdir().unwrap();
        let (spec, _) = job(vec![picture_in(pictures.path())]);
        let dest = target.path().join("x.pdf");
        save_job(spec, work.path(), &dest).unwrap();
        assert!(dest.exists());
    }

    // Neither case sends anything, so no printer is needed.
    #[test]
    fn failures_come_back_as_messages_for_the_user() {
        let dir = tempfile::tempdir().unwrap();
        let (spec, request) = job(vec![dir.path().join("gone.jpg")]);
        let message = print_job(spec, request, dir.path()).unwrap_err();
        assert_eq!(message, "Skipped: gone.jpg");

        let (spec, request) = job(vec![picture_in(dir.path())]);
        let message = print_job(spec, request, dir.path()).unwrap_err();
        assert!(message.contains("does not exist"), "{message}");
    }
}

//! Everything the dialog knows, with no GTK types.

use crate::cups::{Printer, SubmitRequest, job_title};
use crate::job::{Checked, JobSpec};
use crate::layout::{LayoutId, PagePlan, label, paginate, slots};
use crate::ppd::{PaperChoice, PrinterCaps, PrinterOption};
use crate::settings::{PrinterPrefs, Settings};
use std::collections::BTreeMap;
use std::path::PathBuf;

pub struct Model {
    checked: Checked,
    printers: Vec<Printer>,
    caps_of: Box<dyn Fn(&str) -> PrinterCaps>,
    selected: Option<usize>,
    caps: PrinterCaps,
    /// The choices as the user made them. What the printer or paper cannot offer
    /// right now stays here and is simply not used.
    settings: Settings,
    copies: u32,
    page: usize,
}

impl Model {
    pub fn new(
        checked: Checked,
        printers: Vec<Printer>,
        caps_of: Box<dyn Fn(&str) -> PrinterCaps>,
        settings: Settings,
    ) -> Model {
        let selected = settings
            .printer
            .as_deref()
            .and_then(|name| printers.iter().position(|p| p.name == name))
            .or_else(|| printers.iter().position(|p| p.is_default))
            .or_else(|| (!printers.is_empty()).then_some(0));
        let caps = selected.map_or_else(PrinterCaps::fallback, |i| caps_of(&printers[i].name));
        Model {
            checked,
            printers,
            caps_of,
            selected,
            caps,
            settings,
            copies: 1,
            page: 0,
        }
    }

    pub fn files(&self) -> &[PathBuf] {
        &self.checked.usable
    }

    pub fn printers(&self) -> &[Printer] {
        &self.printers
    }

    pub fn printer(&self) -> Option<&Printer> {
        self.selected.map(|i| &self.printers[i])
    }

    pub fn caps(&self) -> &PrinterCaps {
        &self.caps
    }

    fn prefs(&self) -> Option<&PrinterPrefs> {
        self.settings.printers.get(&self.printer()?.name)
    }

    fn prefs_mut(&mut self) -> Option<&mut PrinterPrefs> {
        let name = self.printer()?.name.clone();
        Some(self.settings.printers.entry(name).or_default())
    }

    pub fn paper(&self) -> &PaperChoice {
        self.prefs()
            .and_then(|prefs| self.caps.paper(prefs.paper.as_deref()?))
            .or_else(|| self.caps.paper(&self.caps.default_paper))
            .unwrap_or(&self.caps.papers[0])
    }

    /// The remembered value, if this printer offers it.
    fn chosen<'a>(&'a self, option: &PrinterOption) -> Option<&'a str> {
        let value = self.prefs()?.options.get(&option.keyword)?;
        option
            .choices
            .iter()
            .any(|c| &c.id == value)
            .then_some(value)
    }

    pub fn option_value(&self, keyword: &str) -> Option<&str> {
        let option = self.caps.options.iter().find(|o| o.keyword == keyword)?;
        Some(self.chosen(option).unwrap_or(&option.default))
    }

    pub fn set_printer(&mut self, name: &str) {
        if let Some(i) = self.printers.iter().position(|p| p.name == name) {
            self.selected = Some(i);
            self.caps = (self.caps_of)(name);
        }
    }

    pub fn set_paper(&mut self, id: &str) {
        if self.caps.paper(id).is_some()
            && let Some(prefs) = self.prefs_mut()
        {
            prefs.paper = Some(id.into());
        }
    }

    pub fn set_option(&mut self, keyword: &str, value: &str) {
        let offered = self
            .caps
            .options
            .iter()
            .any(|o| o.keyword == keyword && o.choices.iter().any(|c| c.id == value));
        if offered && let Some(prefs) = self.prefs_mut() {
            prefs.options.insert(keyword.into(), value.into());
        }
    }

    fn available(&self, layout: LayoutId) -> bool {
        !slots(layout, &self.paper().paper).is_empty()
    }

    /// The layouts that fit the paper, with their captions.
    pub fn layouts(&self) -> Vec<(LayoutId, String)> {
        LayoutId::ALL
            .into_iter()
            .filter(|&l| self.available(l))
            .map(|l| (l, label(l, &self.paper().paper)))
            .collect()
    }

    /// The wanted layout, or full page while the paper cannot hold it.
    pub fn layout(&self) -> LayoutId {
        if self.available(self.settings.layout) {
            self.settings.layout
        } else {
            LayoutId::FullPage
        }
    }

    pub fn set_layout(&mut self, layout: LayoutId) {
        if self.available(layout) {
            self.settings.layout = layout;
        }
    }

    pub fn copies(&self) -> u32 {
        self.copies
    }

    pub fn set_copies(&mut self, n: u32) {
        self.copies = n.clamp(1, 99);
    }

    pub fn fill(&self) -> bool {
        self.settings.fit_to_frame
    }

    pub fn set_fill(&mut self, on: bool) {
        self.settings.fit_to_frame = on;
    }

    pub fn sharpen(&self) -> bool {
        self.settings.sharpen
    }

    pub fn set_sharpen(&mut self, on: bool) {
        self.settings.sharpen = on;
    }

    pub fn pages(&self) -> Vec<PagePlan> {
        paginate(
            self.layout(),
            &self.paper().paper,
            self.files().len(),
            self.copies,
        )
    }

    pub fn page_index(&self) -> usize {
        self.page.min(self.pages().len().saturating_sub(1))
    }

    pub fn next_page(&mut self) {
        self.page = (self.page_index() + 1).min(self.pages().len().saturating_sub(1));
    }

    pub fn prev_page(&mut self) {
        self.page = self.page_index().saturating_sub(1);
    }

    pub fn page_label(&self) -> String {
        let total = self.pages().len();
        let current = if total == 0 { 0 } else { self.page_index() + 1 };
        format!("Page {current} of {total}")
    }

    /// The pictures on the current page, each once.
    pub fn visible_images(&self) -> Vec<usize> {
        let pages = self.pages();
        let Some(page) = pages.get(self.page_index()) else {
            return Vec::new();
        };
        let mut images: Vec<usize> = page.iter().map(|p| p.image).collect();
        images.dedup();
        images
    }

    /// Everything that has no dropdown of its own in the main window.
    pub fn advanced_options(&self) -> Vec<&PrinterOption> {
        let main = [self.caps.quality(), self.caps.paper_type()];
        self.caps
            .options
            .iter()
            .filter(|o| !main.iter().flatten().any(|m| m.keyword == o.keyword))
            .collect()
    }

    pub fn can_print(&self) -> bool {
        self.printer().is_some() && !self.files().is_empty()
    }

    pub fn notice(&self) -> Option<String> {
        let mut lines = Vec::new();
        if self.printers.is_empty() {
            lines.push("No printers are installed.".to_string());
        }
        if let Some(printer) = self.printer().filter(|p| p.stopped) {
            lines.push(format!("Printer {} is stopped.", printer.name));
        }
        if !self.checked.rejected.is_empty() {
            lines.push(skipped_notice(&self.checked.rejected));
        }
        (!lines.is_empty()).then(|| lines.join("\n"))
    }

    pub fn job(&self) -> Option<(JobSpec, SubmitRequest)> {
        let printer = self.printer().filter(|_| self.can_print())?;
        let shown = [self.caps.quality(), self.caps.paper_type()];
        // What the visible dropdowns show is always sent; other options only once
        // chosen, so the printer's own defaults keep applying to them.
        let options: BTreeMap<String, String> = self
            .caps
            .options
            .iter()
            .filter_map(|option| {
                let visible = shown.iter().flatten().any(|s| s.keyword == option.keyword);
                let value = self
                    .chosen(option)
                    .or_else(|| visible.then_some(option.default.as_str()))?;
                Some((option.keyword.clone(), value.to_string()))
            })
            .collect();
        let spec = JobSpec {
            files: self.files().to_vec(),
            layout: self.layout(),
            paper: self.paper().paper,
            copies: self.copies,
            fill: self.fill(),
            sharpen: self.sharpen(),
        };
        let request = SubmitRequest {
            printer: printer.name.clone(),
            title: job_title(self.files()),
            paper_id: self.paper().id.clone(),
            options: options.into_iter().collect(),
        };
        Some((spec, request))
    }

    /// What to remember for next time.
    pub fn settings(&self) -> Settings {
        let mut settings = self.settings.clone();
        if let Some(printer) = self.printer() {
            settings.printer = Some(printer.name.clone());
        }
        settings
    }
}

/// "Skipped: a.jpg, b.jpg"
pub fn skipped_notice(skipped: &[(PathBuf, String)]) -> String {
    let names: Vec<String> = skipped
        .iter()
        .map(|(path, _)| {
            path.file_name()
                .unwrap_or(path.as_os_str())
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    format!("Skipped: {}", names.join(", "))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ppd;

    fn photo_caps(name: &str) -> PrinterCaps {
        match name {
            "Office" => ppd::parse(include_str!("../../tests/fixtures/photo-printer.ppd")).unwrap(),
            _ => PrinterCaps::fallback(),
        }
    }

    fn printer(name: &str, is_default: bool) -> Printer {
        Printer {
            name: name.into(),
            is_default,
            stopped: false,
        }
    }

    fn pictures(count: usize) -> Checked {
        let usable = (0..count)
            .map(|i| PathBuf::from(format!("/p/{i}.jpg")))
            .collect();
        Checked {
            usable,
            rejected: Vec::new(),
        }
    }

    fn model_with(count: usize, printers: Vec<Printer>, settings: Settings) -> Model {
        Model::new(pictures(count), printers, Box::new(photo_caps), settings)
    }

    fn model(count: usize) -> Model {
        model_with(
            count,
            vec![printer("Office", true), printer("PDF", false)],
            Settings::default(),
        )
    }

    fn selected(m: &Model) -> &str {
        &m.printer().unwrap().name
    }

    #[test]
    fn printer_choice_is_remembered_then_default_then_first() {
        let both = || vec![printer("Office", false), printer("PDF", true)];
        let remember = |name: &str| Settings {
            printer: Some(name.into()),
            ..Settings::default()
        };
        assert_eq!(
            selected(&model_with(1, both(), remember("Office"))),
            "Office"
        );
        assert_eq!(selected(&model_with(1, both(), remember("Gone"))), "PDF");
        let no_default = vec![printer("Office", false), printer("PDF", false)];
        assert_eq!(
            selected(&model_with(1, no_default, Settings::default())),
            "Office"
        );
    }

    #[test]
    fn no_printers() {
        let remembered = Settings {
            printer: Some("Office".into()),
            ..Settings::default()
        };
        let m = model_with(2, Vec::new(), remembered);
        assert!(!m.can_print());
        assert!(m.job().is_none());
        assert!(m.notice().unwrap().contains("No printers are installed."));
        assert!(!m.pages().is_empty());
        assert_eq!(
            m.settings().printer.as_deref(),
            Some("Office"),
            "not forgotten"
        );
    }

    #[test]
    fn a_stopped_printer_can_still_be_printed_to() {
        let stopped = Printer {
            stopped: true,
            ..printer("Office", true)
        };
        let m = model_with(1, vec![stopped], Settings::default());
        assert!(m.can_print());
        assert_eq!(m.notice().as_deref(), Some("Printer Office is stopped."));
        assert_eq!(model(1).notice(), None);
    }

    #[test]
    fn skipped_files_are_named_and_nothing_usable_means_no_printing() {
        let checked = Checked {
            usable: Vec::new(),
            rejected: vec![
                ("/p/a.txt".into(), "no".into()),
                ("/p/b.jpg".into(), "no".into()),
            ],
        };
        let m = Model::new(
            checked,
            vec![printer("PDF", true)],
            Box::new(photo_caps),
            Settings::default(),
        );
        assert!(!m.can_print());
        assert!(m.job().is_none());
        assert_eq!(m.notice().as_deref(), Some("Skipped: a.txt, b.jpg"));
        assert_eq!(m.page_label(), "Page 0 of 0");
    }

    #[test]
    fn paging_through_250_wallet_pictures() {
        let mut m = model(250);
        m.set_layout(LayoutId::Wallet);
        assert_eq!(m.pages().len(), 28);
        assert_eq!(m.page_label(), "Page 1 of 28");
        assert_eq!(m.visible_images(), (0..9).collect::<Vec<_>>());
        m.prev_page();
        assert_eq!(m.page_index(), 0);
        m.next_page();
        assert_eq!(m.visible_images(), (9..18).collect::<Vec<_>>());
        for _ in 0..40 {
            m.next_page();
        }
        assert_eq!(m.page_label(), "Page 28 of 28");
        m.set_layout(LayoutId::FullPage);
        assert!(m.page_index() < m.pages().len());
        for _ in 0..300 {
            m.next_page();
        }
        m.set_layout(LayoutId::Wallet);
        assert_eq!(m.page_index(), 27);
    }

    #[test]
    fn copies_are_clamped_and_shown_once_per_page() {
        let mut m = model(3);
        assert_eq!(m.copies(), 1);
        m.set_copies(0);
        assert_eq!(m.copies(), 1);
        m.set_copies(500);
        assert_eq!(m.copies(), 99);
        m.set_layout(LayoutId::Wallet);
        assert_eq!(m.visible_images(), [0]);
    }

    #[test]
    fn a_layout_the_paper_cannot_hold_becomes_full_page() {
        let mut m = model(3);
        m.set_layout(LayoutId::P13x18);
        m.set_paper("4x6.Borderless");
        assert_eq!(m.layout(), LayoutId::FullPage);
        let offered: Vec<LayoutId> = m.layouts().iter().map(|(l, _)| *l).collect();
        assert!(!offered.contains(&LayoutId::P13x18));
        assert!(offered.contains(&LayoutId::P10x15));
        m.set_layout(LayoutId::P13x18);
        assert_eq!(m.layout(), LayoutId::FullPage);
        m.set_paper("A4");
        assert_eq!(m.layout(), LayoutId::P13x18, "the wanted layout returns");
    }

    #[test]
    fn paper_and_options_are_remembered_per_printer() {
        let mut m = model(1);
        m.set_paper("4x6.Borderless");
        m.set_option("MediaType", "Glossy");
        m.set_printer("PDF");
        assert_eq!(m.paper().id, "A4");
        assert_eq!(m.option_value("MediaType"), None);
        m.set_printer("Office");
        assert_eq!(m.paper().id, "4x6.Borderless");
        assert_eq!(m.option_value("MediaType"), Some("Glossy"));
        let saved = m.settings();
        assert_eq!(saved.printer.as_deref(), Some("Office"));
        let again = model_with(1, vec![printer("Office", true)], saved);
        assert_eq!(again.option_value("MediaType"), Some("Glossy"));
    }

    #[test]
    fn values_a_printer_does_not_offer_are_ignored_but_kept() {
        let mut settings = Settings::default();
        let prefs = PrinterPrefs {
            paper: Some("A9".into()),
            options: [
                ("MediaType".into(), "Canvas".into()),
                ("Nope".into(), "x".into()),
            ]
            .into(),
        };
        settings.printers.insert("Office".into(), prefs.clone());
        let mut m = model_with(1, vec![printer("Office", true)], settings);
        assert_eq!(m.paper().id, "A4");
        assert_eq!(m.option_value("MediaType"), Some("Plain"));
        m.set_paper("Z1");
        m.set_option("MediaType", "Wood");
        let (_, request) = m.job().unwrap();
        assert!(
            request
                .options
                .iter()
                .all(|(k, v)| k != "Nope" && v != "Canvas" && v != "Wood")
        );
        // A printer that could not be asked just now must not lose its choices.
        assert_eq!(m.settings().printers["Office"], prefs);
    }

    #[test]
    fn the_job_carries_every_choice() {
        let mut m = model(2);
        let kinds: Vec<&str> = m
            .advanced_options()
            .iter()
            .map(|o| o.keyword.as_str())
            .collect();
        assert_eq!(kinds, ["ColorModel"]);
        m.set_layout(LayoutId::P10x15);
        m.set_option("MediaType", "Glossy");
        m.set_option("ColorModel", "Gray");
        m.set_copies(3);
        m.set_fill(false);
        m.set_sharpen(false);
        assert!(!m.fill() && !m.sharpen());

        let (spec, request) = m.job().unwrap();
        assert_eq!(spec.files.len(), 2);
        assert_eq!(
            (spec.layout, spec.copies, spec.fill, spec.sharpen),
            (LayoutId::P10x15, 3, false, false)
        );
        assert_eq!(spec.paper, m.caps().paper("A4").unwrap().paper);
        assert_eq!(
            (request.printer.as_str(), request.paper_id.as_str()),
            ("Office", "A4")
        );
        assert_eq!(request.title, "Print Pictures - 0.jpg (+1)");
        let option = |k: &str, v: &str| (k.to_string(), v.to_string());
        // What the visible dropdowns show is always sent; other options only once chosen.
        assert_eq!(
            request.options,
            [
                option("ColorModel", "Gray"),
                option("MediaType", "Glossy"),
                option("cupsPrintQuality", "Normal")
            ]
        );
        let saved = m.settings();
        assert_eq!(
            (saved.layout, saved.fit_to_frame, saved.sharpen),
            (LayoutId::P10x15, false, false)
        );
        assert_eq!(m.files().len(), 2);
        assert_eq!(m.printers().len(), 2);
    }
}

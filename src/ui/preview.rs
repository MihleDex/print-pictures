//! The page preview: which pictures to load and how the page is drawn.

use crate::image_source::Raster;
use crate::layout::{PagePlan, RectMm, slots};
use crate::render::{Fit, draw_caption, draw_picture, fit, slot_areas};
use crate::ui::model::Model;
use std::collections::HashMap;
use std::f64::consts::FRAC_PI_2;

/// Pixels decoded per millimetre of page; enough for a maximised window.
const PX_PER_MM: f64 = 4.0;
const PADDING: f64 = 8.0;

/// A picture decoded for a particular size on the page.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Key {
    pub image: usize,
    pub long: u32,
    pub short: u32,
}

fn key(image: usize, area: RectMm) -> Key {
    let pixels = |mm: f64| (mm * PX_PER_MM).ceil() as u32;
    Key {
        image,
        long: pixels(area.w.max(area.h)),
        short: pixels(area.w.min(area.h)),
    }
}

/// The pictures of the current page. An entry without a raster is still loading
/// or could not be read.
#[derive(Default)]
pub struct Cache {
    entries: HashMap<Key, Option<Raster>>,
}

impl Cache {
    /// Drops everything else and returns what has to be loaded.
    pub fn want(&mut self, wanted: &[Key]) -> Vec<Key> {
        self.entries.retain(|k, _| wanted.contains(k));
        let new: Vec<Key> = wanted
            .iter()
            .copied()
            .filter(|k| !self.entries.contains_key(k))
            .collect();
        self.entries.extend(new.iter().map(|&k| (k, None)));
        new
    }

    /// False when the page has moved on and the result is no longer wanted.
    pub fn deliver(&mut self, key: Key, raster: Raster) -> bool {
        let entry = self.entries.get_mut(&key);
        let wanted = entry.is_some();
        if let Some(entry) = entry {
            *entry = Some(raster);
        }
        wanted
    }

    pub fn get(&self, key: Key) -> Option<&Raster> {
        self.entries.get(&key)?.as_ref()
    }
}

/// What the current page needs decoded.
pub fn wanted(model: &Model) -> Vec<Key> {
    let pages = model.pages();
    let Some(first) = pages.get(model.page_index()).and_then(|page| page.first()) else {
        return Vec::new();
    };
    // Every slot on a page is the same size.
    let (area, _) = slot_areas(model.layout(), first.slot);
    model
        .visible_images()
        .into_iter()
        .map(|image| key(image, area))
        .collect()
}

/// True when a page holding one picture should be shown on its side.
pub fn turn_view(page: &PagePlan, fits: &[Fit]) -> bool {
    page.len() == 1 && fits.first().is_some_and(|f| f.turn)
}

/// Draws the current page centred in a `width` x `height` area of pixels.
pub fn draw(
    cr: &cairo::Context,
    width: f64,
    height: f64,
    model: &Model,
    cache: &Cache,
) -> Result<(), cairo::Error> {
    let paper = model.paper().paper;
    let layout = model.layout();
    let page: PagePlan = model
        .pages()
        .get(model.page_index())
        .cloned()
        .unwrap_or_default();
    let pictures: Vec<_> = page
        .iter()
        .map(|placement| {
            let (area, strip) = slot_areas(layout, placement.slot);
            let loaded = cache
                .get(key(placement.image, area))
                .map(|r| (r, fit(r.width, r.height, area, model.fill())));
            (placement, area, strip, loaded)
        })
        .collect();
    let fits: Vec<Fit> = pictures.iter().filter_map(|p| Some(p.3?.1)).collect();
    // A single picture on a one-slot layout is easier to judge the right way up.
    let turned = slots(layout, &paper).len() == 1 && turn_view(&page, &fits);

    let (w, h) = (paper.size.w, paper.size.h);
    let (view_w, view_h) = if turned { (h, w) } else { (w, h) };
    let scale = ((width - 2.0 * PADDING) / view_w).min((height - 2.0 * PADDING) / view_h);
    if scale <= 0.0 {
        return Ok(());
    }
    cr.save()?;
    cr.translate(
        (width - view_w * scale) / 2.0,
        (height - view_h * scale) / 2.0,
    );
    cr.scale(scale, scale);
    if turned {
        // The picture is turned clockwise on the paper, so turn the view back.
        cr.translate(0.0, w);
        cr.rotate(-FRAC_PI_2);
    }
    cr.rectangle(0.0, 0.0, w, h);
    cr.set_source_rgb(1.0, 1.0, 1.0);
    cr.fill_preserve()?;
    cr.set_source_rgb(0.5, 0.5, 0.5);
    cr.set_line_width(1.0 / scale);
    cr.stroke()?;

    for (placement, area, strip, loaded) in pictures {
        match loaded {
            Some((raster, fit)) => draw_picture(cr, raster, &fit)?,
            None => {
                cr.rectangle(area.x, area.y, area.w, area.h);
                cr.set_source_rgb(0.75, 0.75, 0.75);
                cr.fill()?;
            }
        }
        if let Some(strip) = strip
            && let Some(file) = model.files().get(placement.image)
        {
            let name = file.file_name().unwrap_or_default().to_string_lossy();
            draw_caption(cr, &name, strip)?;
        }
    }
    cr.restore()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cups::Printer;
    use crate::job::Checked;
    use crate::layout::{LayoutId, Placement};
    use crate::ppd::PrinterCaps;
    use crate::settings::Settings;

    const SLOT: RectMm = RectMm {
        x: 0.0,
        y: 0.0,
        w: 90.0,
        h: 130.0,
    };

    fn one_slot(count: usize) -> PagePlan {
        (0..count)
            .map(|image| Placement { image, slot: SLOT })
            .collect()
    }

    #[test]
    fn the_view_turns_only_for_one_turned_picture() {
        let turned = fit(300, 200, SLOT, true);
        let upright = fit(200, 300, SLOT, true);
        assert!(turn_view(&one_slot(1), &[turned]));
        assert!(!turn_view(&one_slot(1), &[upright]));
        assert!(!turn_view(&one_slot(2), &[turned, turned]));
        assert!(!turn_view(&one_slot(1), &[]), "not loaded yet");
    }

    fn key(image: usize) -> Key {
        Key {
            image,
            long: 10,
            short: 10,
        }
    }

    fn solid(w: u32, h: u32, colour: [u8; 3]) -> Raster {
        Raster {
            width: w,
            height: h,
            rgb: colour.repeat((w * h) as usize),
        }
    }

    #[test]
    fn the_cache_keeps_only_the_wanted_pictures() {
        let mut cache = Cache::default();
        assert_eq!(cache.want(&[key(0), key(1)]), [key(0), key(1)]);
        assert!(cache.deliver(key(1), solid(1, 1, [1, 2, 3])));
        assert_eq!(cache.want(&[key(1), key(2)]), [key(2)]);
        assert!(cache.get(key(1)).is_some());
        assert!(cache.get(key(2)).is_none(), "still loading");
        assert!(
            !cache.deliver(key(0), solid(1, 1, [0, 0, 0])),
            "stale result"
        );
        assert!(cache.get(key(0)).is_none());
    }

    fn model(files: usize) -> Model {
        let checked = Checked {
            usable: (0..files).map(|i| format!("/p/{i}.jpg").into()).collect(),
            rejected: Vec::new(),
        };
        let printers = vec![Printer {
            name: "P".into(),
            is_default: true,
            stopped: false,
        }];
        Model::new(
            checked,
            printers,
            Box::new(|_| PrinterCaps::fallback()),
            Settings::default(),
        )
    }

    /// Draws on a 300 x 300 grey surface and reads pixels back as RGB.
    fn drawn(model: &Model, cache: &Cache) -> impl Fn(usize, usize) -> [u8; 3] + use<> {
        let mut surface = cairo::ImageSurface::create(cairo::Format::Rgb24, 300, 300).unwrap();
        {
            let cr = cairo::Context::new(&surface).unwrap();
            cr.set_source_rgb(0.2, 0.2, 0.2);
            cr.paint().unwrap();
            draw(&cr, 300.0, 300.0, model, cache).unwrap();
        }
        let data = surface.data().unwrap().to_vec();
        move |x, y| {
            let at = (y * 300 + x) * 4;
            let word = u32::from_ne_bytes(data[at..at + 4].try_into().unwrap());
            [(word >> 16) as u8, (word >> 8) as u8, word as u8]
        }
    }

    const BACKGROUND: [u8; 3] = [51, 51, 51];

    #[test]
    fn one_picture_is_decoded_for_its_size_on_the_page() {
        let mut m = model(40);
        m.set_layout(LayoutId::Wallet);
        let keys = wanted(&m);
        assert_eq!(keys.len(), 9);
        assert_eq!((keys[0].long, keys[0].short), (331, 229));
        assert!(wanted(&model(0)).is_empty());
    }

    #[test]
    fn a_loading_picture_shows_as_grey_on_an_upright_page() {
        let pixel = drawn(&model(1), &Cache::default());
        assert_eq!(
            pixel(10, 150),
            BACKGROUND,
            "portrait page leaves the sides free"
        );
        let middle = pixel(150, 150);
        assert!(
            middle[0] == middle[1] && middle[0] > 150 && middle[0] < 240,
            "{middle:?}"
        );
    }

    #[test]
    fn a_turned_full_page_picture_is_shown_upright_on_a_wide_page() {
        let m = model(1);
        let mut cache = Cache::default();
        let key = wanted(&m)[0];
        cache.want(&[key]);
        // Left half red, right half blue: upright means red stays on the left.
        let rgb = (0..200)
            .flat_map(|_| (0..300).flat_map(|x| if x < 150 { [255, 0, 0] } else { [0, 0, 255] }))
            .collect();
        cache.deliver(
            key,
            Raster {
                width: 300,
                height: 200,
                rgb,
            },
        );

        let pixel = drawn(&m, &cache);
        assert_eq!(
            pixel(150, 10),
            BACKGROUND,
            "landscape view leaves top and bottom free"
        );
        assert_eq!(pixel(80, 150), [255, 0, 0]);
        assert_eq!(pixel(220, 150), [0, 0, 255]);
    }
}

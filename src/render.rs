//! Fit rules and cairo drawing, shared by the preview and the print PDF.

use crate::image_source::Raster;
use crate::layout::{LayoutId, RectMm};
use pangocairo::pango;
use std::f64::consts::FRAC_PI_2;

/// Height of the file name strip under a contact sheet picture.
const CAPTION_MM: f64 = 4.0;

/// Where a picture goes: `image` is its rectangle on the page after any turn.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fit {
    pub turn: bool,
    pub image: RectMm,
    pub clip: RectMm,
}

pub fn fit(img_w: u32, img_h: u32, slot: RectMm, fill: bool) -> Fit {
    let turn = (img_w > img_h && slot.h > slot.w) || (img_h > img_w && slot.w > slot.h);
    let (w, h) = if turn { (img_h, img_w) } else { (img_w, img_h) };
    let (sx, sy) = (slot.w / f64::from(w), slot.h / f64::from(h));
    let scale = if fill { sx.max(sy) } else { sx.min(sy) };
    let (w, h) = (f64::from(w) * scale, f64::from(h) * scale);
    let image = RectMm {
        x: slot.x + (slot.w - w) / 2.0,
        y: slot.y + (slot.h - h) / 2.0,
        w,
        h,
    };
    Fit {
        turn,
        image,
        clip: if fill { slot } else { image },
    }
}

/// Splits a contact sheet slot into the picture area and the strip below it.
pub fn split_caption(slot: RectMm) -> (RectMm, RectMm) {
    let picture = RectMm {
        h: slot.h - CAPTION_MM,
        ..slot
    };
    let strip = RectMm {
        y: slot.y + picture.h,
        h: CAPTION_MM,
        ..slot
    };
    (picture, strip)
}

/// Where the picture goes in a slot, and the file name strip if the layout has one.
pub fn slot_areas(layout: LayoutId, slot: RectMm) -> (RectMm, Option<RectMm>) {
    if layout == LayoutId::ContactSheet {
        let (picture, strip) = split_caption(slot);
        (picture, Some(strip))
    } else {
        (slot, None)
    }
}

/// Draws onto a context whose user units are millimetres.
pub fn draw_picture(cr: &cairo::Context, raster: &Raster, fit: &Fit) -> Result<(), cairo::Error> {
    // cairo's Rgb24 is a native-endian 0x00RRGGBB word per pixel.
    let data: Vec<u8> = raster
        .rgb
        .as_chunks::<3>()
        .0
        .iter()
        .flat_map(|p| u32::from_be_bytes([0, p[0], p[1], p[2]]).to_ne_bytes())
        .collect();
    let (w, h) = (raster.width as i32, raster.height as i32);
    let surface = cairo::ImageSurface::create_for_data(data, cairo::Format::Rgb24, w, h, w * 4)?;

    cr.save()?;
    for r in [fit.clip, fit.image] {
        cr.rectangle(r.x, r.y, r.w, r.h);
        cr.clip();
    }
    let (across, down) = if fit.turn {
        // A quarter turn clockwise: the picture's left edge ends up on top.
        cr.translate(fit.image.x + fit.image.w, fit.image.y);
        cr.rotate(FRAC_PI_2);
        (fit.image.h, fit.image.w)
    } else {
        cr.translate(fit.image.x, fit.image.y);
        (fit.image.w, fit.image.h)
    };
    cr.scale(across / f64::from(w), down / f64::from(h));
    cr.set_source_surface(&surface, 0.0, 0.0)?;
    // Repeat the edge pixels so no pale hairline appears around the picture.
    cr.source().set_extend(cairo::Extend::Pad);
    cr.paint()?;
    cr.restore()
}

/// One line of black text, centred in `area` and shortened in the middle.
pub fn draw_caption(cr: &cairo::Context, text: &str, area: RectMm) -> Result<(), cairo::Error> {
    // Pango rounds to whole units, so lay out in twentieths of a millimetre.
    const UNIT: f64 = 0.05;
    let to_pango = |mm: f64| (mm / UNIT * f64::from(pango::SCALE)) as i32;

    cr.save()?;
    cr.translate(area.x, area.y);
    cr.scale(UNIT, UNIT);
    let layout = pangocairo::functions::create_layout(cr);
    let mut font = pango::FontDescription::from_string("Sans");
    font.set_absolute_size(f64::from(to_pango(area.h * 0.7)));
    layout.set_font_description(Some(&font));
    layout.set_width(to_pango(area.w));
    layout.set_ellipsize(pango::EllipsizeMode::Middle);
    layout.set_alignment(pango::Alignment::Center);
    layout.set_single_paragraph_mode(true);
    layout.set_text(text);
    cr.set_source_rgb(0.0, 0.0, 0.0);
    cr.move_to(0.0, 0.0);
    pangocairo::functions::show_layout(cr, &layout);
    cr.restore()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SLOT: RectMm = RectMm {
        x: 10.0,
        y: 10.0,
        w: 90.0,
        h: 130.0,
    };

    fn close(rect: RectMm, x: f64, y: f64, w: f64, h: f64) -> bool {
        [(rect.x, x), (rect.y, y), (rect.w, w), (rect.h, h)]
            .iter()
            .all(|(a, b)| (a - b).abs() < 1e-3)
    }

    #[test]
    fn filling_turns_and_crops() {
        let f = fit(300, 200, SLOT, true);
        assert!(f.turn);
        assert!(close(f.image, 10.0, 7.5, 90.0, 135.0), "{:?}", f.image);
        assert_eq!(f.clip, SLOT);
    }

    #[test]
    fn fitting_shows_the_whole_picture() {
        let f = fit(300, 200, SLOT, false);
        assert!(
            close(f.image, 11.6667, 10.0, 86.6667, 130.0),
            "{:?}",
            f.image
        );
        assert_eq!(f.clip, f.image);
    }

    #[test]
    fn square_pictures_are_never_turned() {
        assert!(!fit(200, 200, SLOT, true).turn);
        assert!(
            !fit(
                200,
                200,
                RectMm {
                    w: 130.0,
                    h: 90.0,
                    ..SLOT
                },
                true
            )
            .turn
        );
    }

    #[test]
    fn a_long_panorama_stays_inside_when_not_filling() {
        let f = fit(4000, 400, SLOT, false);
        assert!(f.image.x >= SLOT.x - 1e-9 && f.image.y >= SLOT.y - 1e-9);
        assert!(f.image.x + f.image.w <= SLOT.x + SLOT.w + 1e-9);
        assert!(f.image.y + f.image.h <= SLOT.y + SLOT.h + 1e-9);
    }

    #[test]
    fn only_the_contact_sheet_has_a_name_strip() {
        assert_eq!(slot_areas(LayoutId::Wallet, SLOT), (SLOT, None));
        let (picture, strip) = split_caption(SLOT);
        assert_eq!(
            slot_areas(LayoutId::ContactSheet, SLOT),
            (picture, Some(strip))
        );
    }

    #[test]
    fn caption_takes_the_bottom_strip() {
        let (picture, strip) = split_caption(SLOT);
        assert!(close(picture, 10.0, 10.0, 90.0, 126.0));
        assert!(close(strip, 10.0, 136.0, 90.0, 4.0));
    }
}

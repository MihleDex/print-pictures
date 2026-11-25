//! The list of layouts on the right, each with a small drawing of the page.

use crate::layout::{LayoutId, Paper, slots};
use gtk4 as gtk;
use gtk4::prelude::*;

const THUMB_W: i32 = 56;
const THUMB_H: i32 = 72;

fn draw_thumbnail(
    cr: &cairo::Context,
    width: f64,
    height: f64,
    layout: LayoutId,
    paper: &Paper,
) -> Result<(), cairo::Error> {
    let (w, h) = (paper.size.w, paper.size.h);
    let scale = ((width - 2.0) / w).min((height - 2.0) / h);
    cr.translate((width - w * scale) / 2.0, (height - h * scale) / 2.0);
    cr.scale(scale, scale);
    cr.rectangle(0.0, 0.0, w, h);
    cr.set_source_rgb(1.0, 1.0, 1.0);
    cr.fill_preserve()?;
    cr.set_source_rgb(0.4, 0.4, 0.4);
    cr.set_line_width(1.0 / scale);
    cr.stroke()?;
    cr.set_source_rgb(0.45, 0.6, 0.75);
    for slot in slots(layout, paper) {
        cr.rectangle(slot.x, slot.y, slot.w, slot.h);
        cr.fill()?;
    }
    Ok(())
}

pub fn row(layout: LayoutId, caption: &str, paper: Paper) -> gtk::ListBoxRow {
    let thumbnail = gtk::DrawingArea::builder()
        .content_width(THUMB_W)
        .content_height(THUMB_H)
        .build();
    thumbnail.set_draw_func(move |_, cr, w, h| {
        // A failed thumbnail leaves the caption, which is enough to choose by.
        let _ = draw_thumbnail(cr, f64::from(w), f64::from(h), layout, &paper);
    });
    let label = gtk::Label::builder()
        .label(caption)
        .wrap(true)
        .justify(gtk::Justification::Center)
        .build();
    let content = gtk::Box::new(gtk::Orientation::Vertical, 2);
    content.set_margin_top(4);
    content.set_margin_bottom(4);
    content.append(&thumbnail);
    content.append(&label);
    gtk::ListBoxRow::builder().child(&content).build()
}

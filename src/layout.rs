//! Paper, layouts, slot geometry and pagination. Everything is in millimetres.

use serde::{Deserialize, Serialize};

pub const NOMINAL_MARGIN_MM: f64 = 5.0;
pub const GAP_MM: f64 = 4.0;
/// How far a picture size may overhang the paper and still count as matching it.
const PAPER_TOLERANCE_MM: f64 = 2.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SizeMm {
    pub w: f64,
    pub h: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RectMm {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Margins {
    pub left: f64,
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Paper {
    pub size: SizeMm,
    pub margins: Margins,
}

impl Paper {
    pub fn a4(margins: Margins) -> Paper {
        Paper {
            size: SizeMm { w: 210.0, h: 297.0 },
            margins,
        }
    }

    pub fn printable(&self) -> RectMm {
        let m = self.margins;
        RectMm {
            x: m.left,
            y: m.top,
            w: self.size.w - m.left - m.right,
            h: self.size.h - m.top - m.bottom,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum LayoutId {
    #[default]
    FullPage,
    P20x25,
    P13x18,
    P10x15,
    P9x13,
    Wallet,
    ContactSheet,
}

impl LayoutId {
    pub const ALL: [LayoutId; 7] = [
        LayoutId::FullPage,
        LayoutId::P20x25,
        LayoutId::P13x18,
        LayoutId::P10x15,
        LayoutId::P9x13,
        LayoutId::Wallet,
        LayoutId::ContactSheet,
    ];

    pub fn id(self) -> &'static str {
        match self {
            LayoutId::FullPage => "full",
            LayoutId::P20x25 => "20x25",
            LayoutId::P13x18 => "13x18",
            LayoutId::P10x15 => "10x15",
            LayoutId::P9x13 => "9x13",
            LayoutId::Wallet => "wallet",
            LayoutId::ContactSheet => "contact",
        }
    }

    pub fn from_id(s: &str) -> Option<LayoutId> {
        LayoutId::ALL.into_iter().find(|l| l.id() == s)
    }
}

/// Slots in reading order; empty means the layout does not fit this paper.
pub fn slots(layout: LayoutId, paper: &Paper) -> Vec<RectMm> {
    let (w, h) = match layout {
        LayoutId::FullPage => return vec![paper.printable()],
        LayoutId::ContactSheet => return contact_sheet(paper),
        LayoutId::P20x25 => (200.0, 250.0),
        LayoutId::P13x18 => (130.0, 180.0),
        LayoutId::P10x15 => (100.0, 150.0),
        LayoutId::P9x13 => (90.0, 130.0),
        LayoutId::Wallet => (57.15, 82.55),
    };

    // Counts come from the nominal margin, never the printer's own margins.
    let avail_w = paper.size.w - 2.0 * NOMINAL_MARGIN_MM;
    let avail_h = paper.size.h - 2.0 * NOMINAL_MARGIN_MM;
    let how_many =
        |avail: f64, size: f64| ((avail + GAP_MM) / (size + GAP_MM)).floor().max(0.0) as usize;
    let upright = (how_many(avail_w, w), how_many(avail_h, h));
    let turned = (how_many(avail_w, h), how_many(avail_h, w));
    let (w, h, cols, rows) = if turned.0 * turned.1 > upright.0 * upright.1 {
        (h, w, turned.0, turned.1)
    } else {
        (w, h, upright.0, upright.1)
    };
    if cols * rows > 0 {
        return grid(w, h, cols, rows, paper.printable());
    }

    // A picture size matching the paper itself goes one to a page.
    let on_paper = |w: f64, h: f64| {
        w <= paper.size.w + PAPER_TOLERANCE_MM && h <= paper.size.h + PAPER_TOLERANCE_MM
    };
    if on_paper(w, h) {
        grid(w, h, 1, 1, paper.printable())
    } else if on_paper(h, w) {
        grid(h, w, 1, 1, paper.printable())
    } else {
        Vec::new()
    }
}

/// A `cols` x `rows` block centred in `area`, shrunk uniformly if it does not fit.
fn grid(w: f64, h: f64, cols: usize, rows: usize, area: RectMm) -> Vec<RectMm> {
    let block_w = cols as f64 * (w + GAP_MM) - GAP_MM;
    let block_h = rows as f64 * (h + GAP_MM) - GAP_MM;
    let s = (area.w / block_w).min(area.h / block_h).min(1.0);
    let x0 = area.x + (area.w - block_w * s) / 2.0;
    let y0 = area.y + (area.h - block_h * s) / 2.0;
    (0..rows)
        .flat_map(|row| (0..cols).map(move |col| (col as f64, row as f64)))
        .map(|(col, row)| RectMm {
            x: x0 + col * (w + GAP_MM) * s,
            y: y0 + row * (h + GAP_MM) * s,
            w: w * s,
            h: h * s,
        })
        .collect()
}

fn contact_sheet(paper: &Paper) -> Vec<RectMm> {
    const COLS: usize = 5;
    const ROWS: usize = 7;
    let p = paper.printable();
    let x = p.x.max(NOMINAL_MARGIN_MM);
    let y = p.y.max(NOMINAL_MARGIN_MM);
    let area = RectMm {
        x,
        y,
        w: (p.x + p.w).min(paper.size.w - NOMINAL_MARGIN_MM) - x,
        h: (p.y + p.h).min(paper.size.h - NOMINAL_MARGIN_MM) - y,
    };
    let w = (area.w - (COLS - 1) as f64 * GAP_MM) / COLS as f64;
    let h = (area.h - (ROWS - 1) as f64 * GAP_MM) / ROWS as f64;
    if w <= 0.0 || h <= 0.0 {
        return Vec::new();
    }
    grid(w, h, COLS, ROWS, area)
}

/// The caption shown under a layout in the sidebar.
pub fn label(layout: LayoutId, paper: &Paper) -> String {
    let name = match layout {
        LayoutId::FullPage => return "Full page photo".into(),
        LayoutId::P20x25 => "20 x 25 cm",
        LayoutId::P13x18 => "13 x 18 cm",
        LayoutId::P10x15 => "10 x 15 cm",
        LayoutId::P9x13 => "9 x 13 cm",
        LayoutId::Wallet => "Wallet",
        LayoutId::ContactSheet => "Contact sheet",
    };
    format!("{name} ({})", slots(layout, paper).len())
}

#[derive(Clone, Debug, PartialEq)]
pub struct Placement {
    pub image: usize,
    pub slot: RectMm,
}

pub type PagePlan = Vec<Placement>;

/// Fills pages in order, with the copies of each picture next to each other.
pub fn paginate(layout: LayoutId, paper: &Paper, image_count: usize, copies: u32) -> Vec<PagePlan> {
    let slots = slots(layout, paper);
    if slots.is_empty() {
        return Vec::new();
    }
    let images: Vec<usize> = (0..image_count)
        .flat_map(|image| std::iter::repeat_n(image, copies as usize))
        .collect();
    images
        .chunks(slots.len())
        .map(|page| {
            page.iter()
                .zip(&slots)
                .map(|(&image, &slot)| Placement { image, slot })
                .collect()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a4() -> Paper {
        Paper::a4(Margins::default())
    }

    fn sheet(w: f64, h: f64) -> Paper {
        Paper {
            size: SizeMm { w, h },
            margins: Margins::default(),
        }
    }

    fn counts(p: &Paper) -> Vec<usize> {
        LayoutId::ALL.iter().map(|&l| slots(l, p).len()).collect()
    }

    fn inside(r: RectMm, outer: RectMm) -> bool {
        r.x >= outer.x - 1e-6
            && r.y >= outer.y - 1e-6
            && r.x + r.w <= outer.x + outer.w + 1e-6
            && r.y + r.h <= outer.y + outer.h + 1e-6
    }

    #[test]
    fn a4_counts_match_windows() {
        assert_eq!(counts(&a4()), [1, 1, 2, 2, 4, 9, 35]);
    }

    #[test]
    fn printer_margins_never_change_counts_and_slots_stay_printable() {
        let p = Paper::a4(Margins {
            left: 3.0,
            top: 3.0,
            right: 3.0,
            bottom: 12.7,
        });
        assert_eq!(counts(&p), [1, 1, 2, 2, 4, 9, 35]);
        for layout in LayoutId::ALL {
            for slot in slots(layout, &p) {
                assert!(inside(slot, p.printable()), "{layout:?} {slot:?}");
            }
        }
    }

    #[test]
    fn captions() {
        assert_eq!(label(LayoutId::FullPage, &a4()), "Full page photo");
        assert_eq!(label(LayoutId::P13x18, &a4()), "13 x 18 cm (2)");
        assert_eq!(label(LayoutId::Wallet, &a4()), "Wallet (9)");
        assert_eq!(label(LayoutId::ContactSheet, &a4()), "Contact sheet (35)");
    }

    #[test]
    fn ids_round_trip() {
        let ids: Vec<&str> = LayoutId::ALL.iter().map(|l| l.id()).collect();
        assert_eq!(
            ids,
            [
                "full", "20x25", "13x18", "10x15", "9x13", "wallet", "contact"
            ]
        );
        for layout in LayoutId::ALL {
            assert_eq!(LayoutId::from_id(layout.id()), Some(layout));
        }
        assert_eq!(LayoutId::from_id("nope"), None);
    }

    #[test]
    fn exact_physical_sizes() {
        let s = slots(LayoutId::P10x15, &a4());
        assert_eq!((s[0].w, s[0].h), (150.0, 100.0)); // turned
        let s = slots(LayoutId::P9x13, &a4());
        assert_eq!((s[0].w, s[0].h), (90.0, 130.0));
    }

    #[test]
    fn a3_fits_four_13x18() {
        assert_eq!(slots(LayoutId::P13x18, &sheet(297.0, 420.0)).len(), 4);
    }

    #[test]
    fn matching_photo_paper() {
        let p = sheet(101.6, 152.4);
        let s = slots(LayoutId::P10x15, &p);
        assert_eq!(s.len(), 1);
        assert!(inside(s[0], p.printable()), "{:?}", s[0]);
        assert!(slots(LayoutId::P13x18, &p).is_empty());
    }

    #[test]
    fn grids_are_centred_without_overlap() {
        for layout in LayoutId::ALL {
            let s = slots(layout, &a4());
            let left = s.iter().map(|r| r.x).fold(f64::MAX, f64::min);
            let right = 210.0 - s.iter().map(|r| r.x + r.w).fold(f64::MIN, f64::max);
            let top = s.iter().map(|r| r.y).fold(f64::MAX, f64::min);
            let bottom = 297.0 - s.iter().map(|r| r.y + r.h).fold(f64::MIN, f64::max);
            assert!((left - right).abs() < 1e-6, "{layout:?} {left} {right}");
            assert!((top - bottom).abs() < 1e-6, "{layout:?} {top} {bottom}");
            for (i, a) in s.iter().enumerate() {
                for b in &s[i + 1..] {
                    let apart = a.x + a.w <= b.x + 1e-6
                        || b.x + b.w <= a.x + 1e-6
                        || a.y + a.h <= b.y + 1e-6
                        || b.y + b.h <= a.y + 1e-6;
                    assert!(apart, "{layout:?} {a:?} overlaps {b:?}");
                }
            }
        }
    }

    #[test]
    fn copies_are_consecutive() {
        let pages = paginate(LayoutId::P13x18, &a4(), 3, 2);
        let ids: Vec<Vec<usize>> = pages
            .iter()
            .map(|p| p.iter().map(|x| x.image).collect())
            .collect();
        assert_eq!(ids, [[0, 0], [1, 1], [2, 2]]);
    }

    #[test]
    fn nothing_to_paginate() {
        assert!(paginate(LayoutId::Wallet, &a4(), 0, 1).is_empty());
    }
}

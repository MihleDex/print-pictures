mod common;

use common::*;
use print_pictures::image_source::load;
use print_pictures::job::{JobError, JobSpec, build_pdf, check_files};
use print_pictures::layout::{LayoutId, Margins, Paper, RectMm, slots};
use std::path::{Path, PathBuf};

fn spec(files: Vec<PathBuf>, layout: LayoutId) -> JobSpec {
    JobSpec {
        files,
        layout,
        paper: Paper::a4(Margins::default()),
        copies: 1,
        fill: true,
        sharpen: true,
    }
}

fn solid(dir: &Path, name: &str, colour: [u8; 3]) -> PathBuf {
    let path = dir.join(name);
    write_png(&path, 60, 90, |_, _| [colour[0], colour[1], colour[2], 255]);
    path
}

/// The colour at a position in millimetres on a 127 dpi page.
fn at(page: &print_pictures::image_source::Raster, x_mm: f64, y_mm: f64) -> [u8; 3] {
    page.pixel((x_mm * 5.0) as u32, (y_mm * 5.0) as u32)
}

fn centre(page: &print_pictures::image_source::Raster, slot: RectMm) -> [u8; 3] {
    at(page, slot.x + slot.w / 2.0, slot.y + slot.h / 2.0)
}

fn near(actual: [u8; 3], expected: [u8; 3]) -> bool {
    actual
        .iter()
        .zip(expected)
        .all(|(&a, e)| a.abs_diff(e) <= 8)
}

#[test]
fn sizes_and_pages() {
    let dir = tempfile::tempdir().unwrap();
    let colours = [[255, 0, 0], [0, 255, 0], [0, 0, 255]];
    let files = ["r.png", "g.png", "b.png"]
        .iter()
        .zip(colours)
        .map(|(name, colour)| solid(dir.path(), name, colour))
        .collect();
    let out = dir.path().join("out.pdf");

    let built = build_pdf(&spec(files, LayoutId::P13x18), &out).unwrap();
    assert_eq!(built.pages, 2);
    assert!(built.skipped.is_empty());

    let info = std::process::Command::new("pdfinfo")
        .arg(&out)
        .output()
        .unwrap();
    let info = String::from_utf8(info.stdout).unwrap();
    assert!(
        info.lines()
            .any(|l| l.starts_with("Pages:") && l.ends_with(" 2")),
        "{info}"
    );
    assert!(info.contains("595.") && info.contains("841."), "{info}");

    let slots = slots(LayoutId::P13x18, &Paper::a4(Margins::default()));
    let first = raster_pdf(&out, 1, 127);
    let second = raster_pdf(&out, 2, 127);
    assert!(near(centre(&first, slots[0]), colours[0]));
    assert!(near(centre(&first, slots[1]), colours[1]));
    assert!(near(centre(&second, slots[0]), colours[2]));
    assert!(near(at(&first, 2.0, 2.0), [255, 255, 255]));
}

#[test]
fn quarter_turn_direction() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("halves.png");
    write_png(&path, 60, 40, |x, _| {
        if x < 30 {
            [255, 0, 0, 255]
        } else {
            [0, 0, 255, 255]
        }
    });
    let out = dir.path().join("out.pdf");

    build_pdf(&spec(vec![path], LayoutId::P9x13), &out).unwrap();

    let slot = slots(LayoutId::P9x13, &Paper::a4(Margins::default()))[0];
    let page = raster_pdf(&out, 1, 127);
    let x = slot.x + slot.w / 2.0;
    assert!(
        near(at(&page, x, slot.y + 10.0), [255, 0, 0]),
        "top should be red"
    );
    assert!(
        near(at(&page, x, slot.y + slot.h - 10.0), [0, 0, 255]),
        "bottom should be blue"
    );
}

#[test]
fn awkward_names() {
    let dir = tempfile::tempdir().unwrap();
    let path = solid(dir.path(), "-Ünï \"quoted\" 100%.png", [1, 2, 3]);
    let out = dir.path().join("out.pdf");

    let built = build_pdf(&spec(vec![path], LayoutId::ContactSheet), &out).unwrap();
    assert_eq!(built.pages, 1);
}

#[test]
fn contact_sheet_prints_file_names() {
    let dir = tempfile::tempdir().unwrap();
    let path = solid(dir.path(), "holiday.png", [255, 0, 0]);
    let out = dir.path().join("out.pdf");

    build_pdf(&spec(vec![path], LayoutId::ContactSheet), &out).unwrap();

    let slot = slots(LayoutId::ContactSheet, &Paper::a4(Margins::default()))[0];
    let page = raster_pdf(&out, 1, 127);
    let strip_has_text = (0..(slot.w * 5.0) as u32).any(|dx| {
        (0..20).any(|dy| {
            let x = (slot.x * 5.0) as u32 + dx;
            let y = ((slot.y + slot.h - 4.0) * 5.0) as u32 + dy;
            page.pixel(x, y)[0] < 100
        })
    });
    assert!(strip_has_text);
}

#[test]
fn bad_files_are_skipped() {
    let dir = tempfile::tempdir().unwrap();
    let good = solid(dir.path(), "good.png", [255, 0, 0]);
    let text = dir.path().join("text.jpg");
    std::fs::write(&text, "not a picture").unwrap();
    let missing = dir.path().join("missing.png");

    let checked = check_files(&[good.clone(), text.clone(), missing.clone()]);
    assert_eq!(checked.usable, [good]);
    let rejected: Vec<&PathBuf> = checked.rejected.iter().map(|(p, _)| p).collect();
    assert_eq!(rejected, [&text, &missing]);

    let out = dir.path().join("out.pdf");
    let result = build_pdf(&spec(Vec::new(), LayoutId::FullPage), &out);
    assert!(
        matches!(&result, Err(JobError::NothingToPrint(s)) if s.is_empty()),
        "{result:?}"
    );
    assert!(!out.exists());
}

#[test]
fn damaged_body_is_skipped() {
    let dir = tempfile::tempdir().unwrap();
    let good = solid(dir.path(), "good.png", [255, 0, 0]);
    let damaged = dir.path().join("damaged.png");
    write_png(&damaged, 200, 200, |x, y| {
        [x as u8, y as u8, (x * y) as u8, 255]
    });
    let mut bytes = std::fs::read(&damaged).unwrap();
    let body = bytes.windows(4).position(|w| w == b"IDAT").unwrap() + 4 + 16;
    bytes[body..].fill(0);
    std::fs::write(&damaged, bytes).unwrap();

    // Without these two the fixture could silently stop testing anything.
    assert_eq!(check_files(std::slice::from_ref(&damaged)).usable.len(), 1);
    assert!(load(&damaged, 100, 100).is_err());

    let out = dir.path().join("out.pdf");
    let files = vec![good.clone(), damaged.clone(), good];
    let built = build_pdf(&spec(files, LayoutId::P13x18), &out).unwrap();
    assert_eq!(built.pages, 1);
    assert_eq!(built.skipped.len(), 1);
    assert_eq!(built.skipped[0].0, damaged);
    assert!(out.exists());

    let alone = dir.path().join("alone.pdf");
    let result = build_pdf(&spec(vec![damaged.clone()], LayoutId::P13x18), &alone);
    assert!(
        matches!(&result, Err(JobError::NothingToPrint(s)) if s.len() == 1 && s[0].0 == damaged),
        "{result:?}"
    );
    assert!(!alone.exists());
}

#[test]
fn render_command_reports_skipped_files() {
    let dir = tempfile::tempdir().unwrap();
    let good = solid(dir.path(), "good.png", [255, 0, 0]);
    let missing = dir.path().join("missing.png");
    let out = dir.path().join("out.pdf");

    let run = std::process::Command::new(env!("CARGO_BIN_EXE_print-pictures"))
        .args([
            "render", "--layout", "wallet", "--paper", "Letter", "--copies", "2", "-o",
        ])
        .arg(&out)
        .arg(&good)
        .arg(&missing)
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&run.stderr);
    assert!(run.status.success(), "{stderr}");
    assert!(stderr.contains("missing.png"), "{stderr}");
    assert!(out.exists());
}

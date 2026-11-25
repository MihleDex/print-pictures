//! Picture fixtures shared by the integration tests.
#![allow(dead_code)] // not every test file uses every helper

use gdk_pixbuf::{Colorspace, Pixbuf};
use print_pictures::image_source::Raster;
use std::path::Path;

fn pixbuf(data: Vec<u8>, w: u32, h: u32, alpha: bool) -> Pixbuf {
    let channels = if alpha { 4 } else { 3 };
    Pixbuf::from_mut_slice(
        data,
        Colorspace::Rgb,
        alpha,
        8,
        w as i32,
        h as i32,
        w as i32 * channels,
    )
}

pub fn write_png(path: &Path, w: u32, h: u32, px: impl Fn(u32, u32) -> [u8; 4]) {
    let data = (0..h)
        .flat_map(|y| (0..w).map(move |x| (x, y)))
        .flat_map(|(x, y)| px(x, y))
        .collect();
    pixbuf(data, w, h, true).savev(path, "png", &[]).unwrap();
}

pub fn jpeg_pixbuf(w: u32, h: u32, px: impl Fn(u32, u32) -> [u8; 3]) -> Pixbuf {
    let data = (0..h)
        .flat_map(|y| (0..w).map(move |x| (x, y)))
        .flat_map(|(x, y)| px(x, y))
        .collect();
    pixbuf(data, w, h, false)
}

pub fn write_jpeg(path: &Path, w: u32, h: u32, px: impl Fn(u32, u32) -> [u8; 3]) {
    jpeg_pixbuf(w, h, px)
        .savev(path, "jpeg", &[("quality", "95")])
        .unwrap();
}

/// Inserts a minimal EXIF block that carries only the orientation tag.
pub fn add_exif_orientation(jpeg: &Path, orientation: u8) {
    #[rustfmt::skip]
    let exif: [u8; 36] = [
        0xFF, 0xE1, 0x00, 0x22, 0x45, 0x78, 0x69, 0x66, 0x00, 0x00, 0x4D, 0x4D, 0x00, 0x2A, 0x00, 0x00,
        0x00, 0x08, 0x00, 0x01, 0x01, 0x12, 0x00, 0x03, 0x00, 0x00, 0x00, 0x01, 0x00, orientation,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];
    let mut bytes = std::fs::read(jpeg).unwrap();
    bytes.splice(2..2, exif);
    std::fs::write(jpeg, bytes).unwrap();
}

/// Rasterises one PDF page with `pdftoppm`; 127 dpi gives 5 px per mm.
pub fn raster_pdf(pdf: &Path, page: u32, dpi: u32) -> Raster {
    let dir = tempfile::tempdir().unwrap();
    let prefix = dir.path().join("page");
    let page = page.to_string();
    let status = std::process::Command::new("pdftoppm")
        .args([
            "-r",
            &dpi.to_string(),
            "-f",
            &page,
            "-l",
            &page,
            "-singlefile",
        ])
        .arg(pdf)
        .arg(&prefix)
        .status()
        .unwrap();
    assert!(status.success());
    let ppm = std::fs::read(prefix.with_extension("ppm")).unwrap();
    // Header: "P6\n<width> <height>\n255\n", then the pixels.
    let mut parts = ppm.splitn(4, |&b| b == b'\n');
    assert_eq!(parts.next(), Some(&b"P6"[..]));
    let size = std::str::from_utf8(parts.next().unwrap()).unwrap();
    let (width, height) = size.split_once(' ').unwrap();
    assert_eq!(parts.next(), Some(&b"255"[..]));
    Raster {
        width: width.parse().unwrap(),
        height: height.parse().unwrap(),
        rgb: parts.next().unwrap().to_vec(),
    }
}

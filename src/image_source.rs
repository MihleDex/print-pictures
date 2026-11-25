//! Decodes a picture with gdk-pixbuf into an upright sRGB raster.

use gdk_pixbuf::{Pixbuf, glib};
use lcms2::{Intent, PixelFormat, Profile, Transform};
use std::path::Path;

/// Upright sRGB pixels, three bytes each, rows packed with no padding.
#[derive(Clone, Debug)]
pub struct Raster {
    pub width: u32,
    pub height: u32,
    pub rgb: Vec<u8>,
}

impl Raster {
    pub fn pixel(&self, x: u32, y: u32) -> [u8; 3] {
        let i = (y as usize * self.width as usize + x as usize) * 3;
        [self.rgb[i], self.rgb[i + 1], self.rgb[i + 2]]
    }
}

#[derive(Clone, Debug, thiserror::Error)]
pub enum LoadError {
    #[error("cannot read the file: {0}")]
    Unreadable(String),
    #[error("not a picture that can be opened: {0}")]
    Unsupported(String),
}

/// The stored (not yet turned) size, from the header only.
pub fn probe(path: &Path) -> Result<(u32, u32), LoadError> {
    std::fs::File::open(path).map_err(|e| LoadError::Unreadable(e.to_string()))?;
    match Pixbuf::file_info(path) {
        Some((_, w, h)) if w > 0 && h > 0 => Ok((w as u32, h as u32)),
        _ => Err(LoadError::Unsupported(
            "unknown format or damaged header".into(),
        )),
    }
}

/// Loads just large enough to cover `cover_long` x `cover_short`, never enlarging.
pub fn load(path: &Path, cover_long: u32, cover_short: u32) -> Result<Raster, LoadError> {
    let (w, h) = probe(path)?;
    let scale = (f64::from(cover_long) / f64::from(w.max(h)))
        .max(f64::from(cover_short) / f64::from(w.min(h)))
        .min(1.0);
    let scaled = |side: u32| (f64::from(side) * scale).round().max(1.0) as i32;
    let pixbuf = Pixbuf::from_file_at_scale(path, scaled(w), scaled(h), true)
        .map_err(|e| LoadError::Unsupported(e.to_string()))?;

    // Read before turning: the turned copy does not carry the options.
    let icc = pixbuf.option("icc-profile");
    let pixbuf = pixbuf.apply_embedded_orientation().unwrap_or(pixbuf);

    let (width, height) = (pixbuf.width() as usize, pixbuf.height() as usize);
    let (channels, stride) = (pixbuf.n_channels() as usize, pixbuf.rowstride() as usize);
    let bytes = pixbuf.read_pixel_bytes();
    // The last row may stop short of `stride`, so slice each row by its real length.
    let rows = (0..height).map(|y| &bytes[y * stride..y * stride + width * channels]);

    let mut rgb: Vec<[u8; 3]> = Vec::with_capacity(width * height);
    let mut alpha = Vec::new();
    for row in rows {
        for px in row.chunks_exact(channels) {
            rgb.push([px[0], px[1], px[2]]);
            if channels == 4 {
                alpha.push(px[3]);
            }
        }
    }

    if let Some(icc) = icc {
        convert_to_srgb(&mut rgb, &glib::base64_decode(&icc));
    }

    // Composite onto white paper.
    for (px, &a) in rgb.iter_mut().zip(&alpha) {
        for c in px {
            *c = ((u32::from(*c) * u32::from(a) + 255 * (255 - u32::from(a)) + 127) / 255) as u8;
        }
    }

    Ok(Raster {
        width: width as u32,
        height: height as u32,
        rgb: rgb.into_flattened(),
    })
}

/// shortcut: a profile lcms cannot apply to RGB pixels (damaged, gray, CMYK) is
/// ignored and the pixels print as sRGB; handle those spaces if colours look off.
fn convert_to_srgb(rgb: &mut [[u8; 3]], icc: &[u8]) {
    let Ok(source) = Profile::new_icc(icc) else {
        return;
    };
    let srgb = Profile::new_srgb();
    let Ok(transform) = Transform::<[u8; 3], [u8; 3]>::new(
        &source,
        PixelFormat::RGB_8,
        &srgb,
        PixelFormat::RGB_8,
        Intent::Perceptual,
    ) else {
        return;
    };
    let original = rgb.to_vec();
    transform.transform_pixels(&original, rgb);
}

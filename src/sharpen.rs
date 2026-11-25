//! A mild unsharp filter for printing.

use crate::image_source::Raster;

const AMOUNT: f32 = 0.5;

/// `v + AMOUNT * (v - mean of the 3x3 around v)`, per channel, edges repeated.
pub fn sharpen(raster: &mut Raster) {
    let (w, h) = (raster.width as usize, raster.height as usize);
    if w == 0 || h == 0 {
        return;
    }
    let original = raster.rgb.clone();
    for y in 0..h {
        let rows = [y.saturating_sub(1), y, (y + 1).min(h - 1)];
        for x in 0..w {
            let cols = [x.saturating_sub(1), x, (x + 1).min(w - 1)];
            for channel in 0..3 {
                let at = |x: usize, y: usize| original[(y * w + x) * 3 + channel];
                let sum: u32 = rows
                    .iter()
                    .flat_map(|&ry| cols.iter().map(move |&cx| (cx, ry)))
                    .map(|(cx, ry)| u32::from(at(cx, ry)))
                    .sum();
                let v = f32::from(at(x, y));
                let sharpened = v + AMOUNT * (v - sum as f32 / 9.0);
                raster.rgb[(y * w + x) * 3 + channel] = sharpened.round().clamp(0.0, 255.0) as u8;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raster(width: u32, height: u32, value: impl Fn(u32) -> u8) -> Raster {
        let rgb = (0..width * height)
            .flat_map(|i| [value(i % width); 3])
            .collect();
        Raster { width, height, rgb }
    }

    #[test]
    fn uniform_grey_is_unchanged() {
        let mut grey = raster(5, 5, |_| 128);
        sharpen(&mut grey);
        assert!(grey.rgb.iter().all(|&v| v == 128));
    }

    #[test]
    fn a_step_gets_more_contrast() {
        let mut step = raster(6, 3, |x| if x < 3 { 100 } else { 200 });
        sharpen(&mut step);
        assert!(step.pixel(2, 1)[0] < 100);
        assert!(step.pixel(3, 1)[0] > 200);
    }
}

mod common;

use common::*;
use gdk_pixbuf::glib;
use lcms2::{CIExyY, CIExyYTRIPLE, Profile, ToneCurve};
use print_pictures::image_source::{LoadError, load, probe};

fn is_red(p: [u8; 3]) -> bool {
    p[0] > 200 && p[2] < 60
}

fn is_blue(p: [u8; 3]) -> bool {
    p[2] > 200 && p[0] < 60
}

#[test]
fn exif_6_is_turned_upright() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("phone.jpg");
    write_jpeg(&path, 40, 20, |x, _| {
        if x < 20 { [255, 0, 0] } else { [0, 0, 255] }
    });
    add_exif_orientation(&path, 6);

    assert_eq!(
        probe(&path).unwrap(),
        (40, 20),
        "probe reports the stored size"
    );
    let raster = load(&path, 4000, 2000).unwrap();
    assert_eq!((raster.width, raster.height), (20, 40));
    assert_eq!(raster.rgb.len(), 20 * 40 * 3);
    assert!(is_red(raster.pixel(10, 5)), "{:?}", raster.pixel(10, 5));
    assert!(is_blue(raster.pixel(10, 35)), "{:?}", raster.pixel(10, 35));
}

#[test]
fn reduced_load_keeps_orientation() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("big.jpg");
    write_jpeg(&path, 4000, 2000, |_, _| [10, 200, 10]);
    add_exif_orientation(&path, 6);

    let raster = load(&path, 400, 200).unwrap();
    assert_eq!((raster.width, raster.height), (200, 400));
}

#[test]
fn never_enlarges() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("small.png");
    write_png(&path, 40, 20, |_, _| [1, 2, 3, 255]);

    let raster = load(&path, 4000, 2000).unwrap();
    assert_eq!((raster.width, raster.height), (40, 20));
    assert_eq!(raster.pixel(39, 19), [1, 2, 3]);
}

#[test]
fn transparency_becomes_white() {
    let dir = tempfile::tempdir().unwrap();
    let clear = dir.path().join("clear.png");
    write_png(&clear, 8, 8, |_, _| [0, 0, 0, 0]);
    assert_eq!(load(&clear, 8, 8).unwrap().pixel(4, 4), [255, 255, 255]);

    let half = dir.path().join("half.png");
    write_png(&half, 8, 8, |_, _| [0, 0, 0, 128]);
    for channel in load(&half, 8, 8).unwrap().pixel(4, 4) {
        assert!((126..=129).contains(&channel), "{channel}");
    }
}

#[test]
fn embedded_profile_is_converted() {
    let xy = |x, y| CIExyY { x, y, Y: 1.0 };
    // sRGB with the red and blue primaries swapped.
    let swapped = CIExyYTRIPLE {
        Red: xy(0.15, 0.06),
        Green: xy(0.30, 0.60),
        Blue: xy(0.64, 0.33),
    };
    let curve = ToneCurve::new(2.2);
    let profile =
        Profile::new_rgb(&xy(0.3127, 0.3290), &swapped, &[&curve, &curve, &curve]).unwrap();
    let encoded = glib::base64_encode(&profile.icc().unwrap());

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("swapped.jpg");
    jpeg_pixbuf(40, 20, |_, _| [255, 0, 0])
        .savev(
            &path,
            "jpeg",
            &[("quality", "95"), ("icc-profile", encoded.as_str())],
        )
        .unwrap();

    for cover in [(40, 20), (20, 10)] {
        let raster = load(&path, cover.0, cover.1).unwrap();
        assert!(
            is_blue(raster.pixel(5, 5)),
            "{cover:?}: {:?}",
            raster.pixel(5, 5)
        );
    }
}

#[test]
fn bad_files() {
    let dir = tempfile::tempdir().unwrap();
    let text = dir.path().join("x.jpg");
    std::fs::write(&text, "this is not a picture").unwrap();
    let missing = dir.path().join("missing.jpg");

    assert!(matches!(probe(&text), Err(LoadError::Unsupported(_))));
    assert!(matches!(
        load(&text, 100, 100),
        Err(LoadError::Unsupported(_))
    ));
    assert!(matches!(probe(&missing), Err(LoadError::Unreadable(_))));
    assert!(matches!(
        load(&missing, 100, 100),
        Err(LoadError::Unreadable(_))
    ));
}

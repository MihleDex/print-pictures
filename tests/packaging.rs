//! The files the package installs beside the program.

use print_pictures::integration::desktop_entry;
use print_pictures::ui::APPLICATION_ID;
use std::path::Path;

fn read(path: &str) -> String {
    let full = Path::new(env!("CARGO_MANIFEST_DIR")).join(path);
    std::fs::read_to_string(&full).unwrap_or_else(|e| panic!("{path}: {e}"))
}

#[test]
fn menu_extensions_behave() {
    let run = std::process::Command::new("python3")
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("data/menus/test_menus.py"))
        .output()
        .unwrap();
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
}

#[test]
fn menu_files_run_the_program_for_pictures() {
    for path in [
        "data/menus/print-pictures.nemo_action",
        "data/menus/print-pictures-dolphin.desktop",
    ] {
        let text = read(path);
        for part in ["Name=Print\n", "print-pictures %F\n", "image/*;\n"] {
            assert!(text.contains(part), "{path} lacks {part:?}");
        }
    }
}

#[test]
fn data_files_carry_the_application_id() {
    assert_eq!(APPLICATION_ID, "io.github.mihledex.PrintPictures");
    let entry = read(&format!("data/{APPLICATION_ID}.desktop"));
    let formats = desktop_entry(Path::new("/x"));
    let formats = formats
        .lines()
        .find(|l| l.starts_with("MimeType="))
        .unwrap();
    for part in [
        "Exec=print-pictures %F",
        formats,
        &format!("Icon={APPLICATION_ID}"),
    ] {
        assert!(entry.lines().any(|l| l == part), "{part} missing:\n{entry}");
    }
    let store = read(&format!("data/{APPLICATION_ID}.metainfo.xml"));
    assert!(store.contains(&format!("<id>{APPLICATION_ID}</id>")));
    assert!(store.contains("<project_license>GPL-3.0-or-later</project_license>"));
    assert!(read(&format!("data/{APPLICATION_ID}.svg")).contains("<svg"));
}

#[test]
fn the_package_lists_files_that_exist() {
    let manifest: toml::Table = toml::from_str(&read("Cargo.toml")).unwrap();
    assert_eq!(
        manifest["package"]["license"].as_str(),
        Some("GPL-3.0-or-later")
    );
    let deb = &manifest["package"]["metadata"]["deb"];
    assert!(deb["depends"].as_str().unwrap().contains("cups-client"));
    let assets = deb["assets"].as_array().unwrap();
    assert_eq!(assets.len(), 8);
    for asset in assets {
        let source = asset[0].as_str().unwrap();
        if !source.starts_with("target/") {
            read(source);
        }
    }
}
